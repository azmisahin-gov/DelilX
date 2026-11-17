//! İlerleme bildirimi ve işlem kaydı (log) altyapısı.
//!
//! Uzun süren edinim işlemleri ilerleme geri çağrıları ile kullanıcıya
//! Türkçe durum bilgisi verir.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// İlerleme bilgisi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    /// Tamamlanan bayt.
    pub bytes_done: u64,
    /// Toplam bayt (bilinmiyorsa 0).
    pub bytes_total: u64,
    /// Tamamlanan sektör.
    pub sectors_done: u64,
    /// Toplam sektör (bilinmiyorsa 0).
    pub sectors_total: u64,
    /// Şu anki hız (bayt/saniye).
    pub bytes_per_second: f64,
    /// Türkçe durum mesajı.
    pub message: String,
}

impl Progress {
    /// Tamamlanma oranını (0.0 - 1.0) döndürür. Toplam bilinmiyorsa `None`.
    pub fn fraction(&self) -> Option<f64> {
        if self.bytes_total == 0 {
            None
        } else {
            Some((self.bytes_done as f64 / self.bytes_total as f64).min(1.0))
        }
    }

    /// Türkçe yüzde metni üretir.
    pub fn percent_text(&self) -> String {
        match self.fraction() {
            Some(f) => format!("%{:.1}", f * 100.0),
            None => "Bilinmiyor".to_string(),
        }
    }
}

/// İlerleme geri çağrısı türü.
pub type ProgressCallback = Arc<dyn Fn(Progress) + Send + Sync>;

/// İptal bayrağı. Kullanıcı işlemi durdurabilir.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    /// Yeni iptal belirteci oluşturur.
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// İptal isteğini bildirir.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// İptal istendi mi?
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// İptal edildiyse hata döndürür.
    pub fn check(&self) -> crate::error::Result<()> {
        if self.is_cancelled() {
            Err(crate::error::DelilXError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// İşlem kaydı önem seviyesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Bilgi.
    Info,
    /// Uyarı.
    Warning,
    /// Hata.
    Error,
}

impl LogLevel {
    /// Türkçe etiket.
    pub fn label(&self) -> &'static str {
        match self {
            LogLevel::Info => "Bilgi",
            LogLevel::Warning => "Uyarı",
            LogLevel::Error => "Hata",
        }
    }
}

/// Tek bir işlem kaydı satırı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Zaman damgası (ISO 8601).
    pub timestamp: String,
    /// Önem seviyesi.
    pub level: LogLevel,
    /// Türkçe mesaj.
    pub message: String,
}

/// İşlem kaydı toplayıcı. Edinim sırasında bütün olayları tutar.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OperationLog {
    /// Kayıt satırları.
    pub entries: Vec<LogEntry>,
}

impl OperationLog {
    /// Yeni boş kayıt oluşturur.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bilgi kaydı ekler.
    pub fn info(&mut self, message: impl Into<String>) {
        self.push(LogLevel::Info, message);
    }

    /// Uyarı kaydı ekler.
    pub fn warn(&mut self, message: impl Into<String>) {
        self.push(LogLevel::Warning, message);
    }

    /// Hata kaydı ekler.
    pub fn error(&mut self, message: impl Into<String>) {
        self.push(LogLevel::Error, message);
    }

    /// Kayıt ekler.
    pub fn push(&mut self, level: LogLevel, message: impl Into<String>) {
        self.entries.push(LogEntry {
            timestamp: crate::timeutil::now_iso8601(),
            level,
            message: message.into(),
        });
    }

    /// Belirli seviyedeki kayıtları döndürür.
    pub fn by_level(&self, level: LogLevel) -> Vec<&LogEntry> {
        self.entries.iter().filter(|e| e.level == level).collect()
    }

    /// Toplam uyarı sayısı.
    pub fn warning_count(&self) -> usize {
        self.by_level(LogLevel::Warning).len()
    }

    /// Toplam hata sayısı.
    pub fn error_count(&self) -> usize {
        self.by_level(LogLevel::Error).len()
    }
}

/// İlerleme durumunu izleyen yardımcı. Hız hesabı yapar.
pub struct ProgressTracker {
    start: std::time::Instant,
    total: u64,
    last_report: std::cell::Cell<std::time::Instant>,
    report_interval: std::time::Duration,
    last_bytes: AtomicU64,
}

impl ProgressTracker {
    /// Yeni izleyici oluşturur.
    pub fn new(total: u64) -> Self {
        let now = std::time::Instant::now();
        Self {
            start: now,
            total,
            last_report: std::cell::Cell::new(now),
            report_interval: std::time::Duration::from_millis(200),
            last_bytes: AtomicU64::new(0),
        }
    }

    /// Geçen süreyi saniye olarak döndürür.
    pub fn elapsed_seconds(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// İlerleme nesnesi üretir.
    pub fn progress(&self, bytes_done: u64, sectors_done: u64, message: &str) -> Progress {
        let elapsed = self.elapsed_seconds();
        let bps = if elapsed > 0.0 {
            bytes_done as f64 / elapsed
        } else {
            0.0
        };
        self.last_bytes.store(bytes_done, Ordering::Relaxed);
        Progress {
            bytes_done,
            bytes_total: self.total,
            sectors_done,
            sectors_total: if self.total > 0 { self.total / 512 } else { 0 },
            bytes_per_second: bps,
            message: message.to_string(),
        }
    }

    /// Rapor aralığı geçti mi? (Arayüz güncellemelerini seyreltmek için.)
    pub fn should_report(&self) -> bool {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_report.get()) >= self.report_interval {
            self.last_report.set(now);
            true
        } else {
            false
        }
    }
}

/// İnsan okunur bayt biçimi (Türkçe).
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

/// Saniye cinsinden hızı insan okunur biçime çevirir.
pub fn format_speed(bytes_per_second: f64) -> String {
    format!("{}/sn", format_bytes(bytes_per_second as u64))
}

/// Süreyi Türkçe biçimde döndürür.
pub fn format_duration(seconds: f64) -> String {
    let total = seconds as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h} saat {m} dakika {s} saniye")
    } else if m > 0 {
        format!("{m} dakika {s} saniye")
    } else {
        format!("{s} saniye")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_token_works() {
        let token = CancelToken::new();
        assert!(!token.is_cancelled());
        assert!(token.check().is_ok());
        token.cancel();
        assert!(token.is_cancelled());
        assert!(token.check().is_err());
    }

    #[test]
    fn bytes_formatting() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MiB");
        assert_eq!(format_bytes(1536 * 1024), "1.50 MiB");
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(format_duration(45.0), "45 saniye");
        assert_eq!(format_duration(90.0), "1 dakika 30 saniye");
        assert_eq!(format_duration(3661.0), "1 saat 1 dakika 1 saniye");
    }

    #[test]
    fn progress_fraction() {
        let tracker = ProgressTracker::new(1000);
        let p = tracker.progress(500, 0, "test");
        assert_eq!(p.fraction(), Some(0.5));
        assert_eq!(p.percent_text(), "%50.0");
    }

    #[test]
    fn operation_log_counts() {
        let mut log = OperationLog::new();
        log.info("başladı");
        log.warn("yavaş sektör");
        log.error("okuma hatası");
        assert_eq!(log.warning_count(), 1);
        assert_eq!(log.error_count(), 1);
        assert_eq!(log.entries.len(), 3);
    }
}
