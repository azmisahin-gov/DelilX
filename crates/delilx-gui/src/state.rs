//! Uygulama durumu (ekranlar arası paylaşılan veriler).

use delilx_core::device::Device;
use delilx_core::hash::HashAlgorithm;
use delilx_core::progress::{LogEntry, Progress};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

/// Arayüzde seçilebilen ekranlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Ana sayfa.
    Home,
    /// Cihazları algıla.
    Devices,
    /// Veri edinimi (genel bakış).
    Acquisition,
    /// RAW imaj.
    Raw,
    /// E01 imaj.
    E01,
    /// Mantıksal edinim.
    Logical,
    /// Seçmeli edinim.
    Selective,
    /// Kullanılan alan edinimi.
    UsedSpace,
    /// Yazma koruması (yazılımsal write blocker).
    WriteProtect,
    /// İmaj inceleme.
    Inspect,
    /// Hash doğrulama.
    Verify,
    /// Delil raporları.
    Reports,
    /// Ayarlar.
    Settings,
}

impl Screen {
    /// Ekran başlığı (Türkçe).
    pub fn title(&self) -> &'static str {
        match self {
            Screen::Home => "Ana Sayfa",
            Screen::Devices => "Cihazları Algıla",
            Screen::Acquisition => "Veri Edinimi",
            Screen::Raw => "RAW İmaj",
            Screen::E01 => "E01 İmaj",
            Screen::Logical => "Mantıksal Edinim",
            Screen::Selective => "Seçmeli Edinim",
            Screen::UsedSpace => "Kullanılan Alan",
            Screen::WriteProtect => "Yazma Koruması",
            Screen::Inspect => "İmaj İnceleme",
            Screen::Verify => "Hash Doğrulama",
            Screen::Reports => "Delil Raporları",
            Screen::Settings => "Ayarlar",
        }
    }

    /// Ekran simgesi.
    pub fn icon(&self) -> &'static str {
        match self {
            Screen::Home => "🏠",
            Screen::Devices => "🔌",
            Screen::Acquisition => "📥",
            Screen::Raw => "💽",
            Screen::E01 => "🗄",
            Screen::Logical => "📁",
            Screen::Selective => "☑",
            Screen::UsedSpace => "🧩",
            Screen::WriteProtect => "🔒",
            Screen::Inspect => "🔍",
            Screen::Verify => "✓",
            Screen::Reports => "📄",
            Screen::Settings => "⚙",
        }
    }
}

/// Arayüz ayarları.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Varsayılan delil numarası.
    pub default_evidence_number: String,
    /// Varsayılan vaka numarası.
    pub default_case_number: String,
    /// Varsayılan incelemeyi yapan.
    pub default_examiner: String,
    /// Varsayılan sektör boyutu.
    pub bytes_per_sector: u32,
    /// Varsayılan hash algoritmaları.
    pub hash_algorithms: Vec<HashAlgorithm>,
    /// Çıktı dizini.
    pub output_dir: PathBuf,
    /// Doğrulama varsayılan açık mı?
    pub verify_by_default: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            default_evidence_number: "DELIL-1".to_string(),
            default_case_number: "VAKA-1".to_string(),
            default_examiner: String::new(),
            bytes_per_sector: 512,
            hash_algorithms: vec![HashAlgorithm::Sha256, HashAlgorithm::Sha512],
            output_dir: std::env::current_dir().unwrap_or_default(),
            verify_by_default: true,
        }
    }
}

/// Arka planda çalışan işlem durumu.
#[derive(Default)]
pub struct RunningJob {
    /// İşlem çalışıyor mu?
    pub running: bool,
    /// İşlem adı (Türkçe).
    pub name: String,
    /// Son ilerleme bilgisi.
    pub progress: Option<Progress>,
    /// İptal belirteci.
    pub cancel: Option<delilx_core::progress::CancelToken>,
    /// Sonuç mesajı (tamamlandığında).
    pub result: Option<String>,
    /// Sonuç başarılı mı?
    pub success: bool,
    /// İlerleme alıcısı.
    pub receiver: Option<Receiver<Progress>>,
}

/// Uygulamanın bütün durumu.
pub struct AppState {
    /// Geçerli ekran.
    pub current: Screen,
    /// Ayarlar.
    pub settings: Settings,
    /// Algılanan cihazlar.
    pub devices: Vec<Device>,
    /// Seçili kaynak cihaz indeksi.
    pub selected_device: Option<usize>,
    /// Kaynak yolu (manuel).
    pub source_path: String,
    /// Hedef yolu.
    pub target_path: String,
    /// Çalışan işlem.
    pub job: RunningJob,
    /// İşlem kayıtları.
    pub log: Vec<LogEntry>,
    /// Durum çubuğu mesajı.
    pub status: String,
    /// Durum mesajı türü (0 bilgi, 1 başarı, 2 uyarı, 3 hata).
    pub status_kind: u8,
    /// E01 alanları.
    pub e01: E01Form,
    /// Mantıksal alanları.
    pub logical: LogicalForm,
    /// İnceleme sonucu.
    pub inspection: Option<delilx_core::inspect::ImageInspection>,
    /// Seçili imaj yolu (inceleme).
    pub inspect_path: String,
    /// Doğrulama alanları.
    pub verify: VerifyForm,
    /// Üretilen rapor yolları.
    pub report_paths: Vec<PathBuf>,
    /// Rapor oluşturma formu.
    pub report: ReportForm,
    /// RAW okuma hatası politikası (0 sıfırla, 1 durdur).
    pub raw_error_policy: u8,
    /// Yazma koruması ekranı durumu.
    pub write_protect: WriteProtectState,
    /// Son edinim özeti (rapor için).
    pub last_report: Option<delilx_core::report::EvidenceReport>,
}

/// E01 form alanları.
#[derive(Debug, Clone)]
pub struct E01Form {
    /// Delil numarası.
    pub evidence_number: String,
    /// Vaka numarası.
    pub case_number: String,
    /// Açıklama.
    pub description: String,
    /// İncelemeyi yapan.
    pub examiner: String,
    /// Notlar.
    pub notes: String,
    /// Sıkıştırma (0 yok, 1 hızlı, 2 en iyi).
    pub compression: u8,
    /// Segment boyutu (MiB).
    pub segment_mib: u64,
    /// Çıkarılabilir mi?
    pub removable: bool,
}

impl Default for E01Form {
    fn default() -> Self {
        E01Form {
            evidence_number: "DELIL-1".to_string(),
            case_number: "VAKA-1".to_string(),
            description: "DelilX edinimi".to_string(),
            examiner: String::new(),
            notes: String::new(),
            compression: 2,
            segment_mib: 1400,
            removable: false,
        }
    }
}

/// Mantıksal/seçmeli edinim form alanları.
#[derive(Debug, Clone, Default)]
pub struct LogicalForm {
    /// Kaynak dizin.
    pub source_dir: String,
    /// Seçilen dosyalar.
    pub selected_files: Vec<PathBuf>,
    /// Yalnızca envanter.
    pub inventory_only: bool,
    /// Uzantı filtresi (virgülle).
    pub extensions: String,
}

/// Doğrulama form alanları.
#[derive(Debug, Clone, Default)]
pub struct VerifyForm {
    /// İmaj yolu.
    pub path: String,
    /// Beklenen SHA-256.
    pub sha256: String,
    /// Beklenen SHA-512.
    pub sha512: String,
    /// Son sonuç metni.
    pub result: Option<String>,
    /// Başarılı mı?
    pub success: bool,
}

/// Rapor form alanları.
#[derive(Debug, Clone, Default)]
pub struct ReportForm {
    /// İşlem numarası.
    pub operation_number: String,
    /// Delil numarası.
    pub evidence_number: String,
    /// Vaka numarası.
    pub case_number: String,
    /// İncelemeyi yapan.
    pub examiner: String,
    /// Açıklama.
    pub description: String,
}

/// Yazma koruması ekranı durumu.
#[derive(Debug, Clone, Default)]
pub struct WriteProtectState {
    /// Seçili cihaz yolu (fiziksel disk veya birim).
    pub device_path: String,
    /// Son sorgu raporu.
    pub report: Option<delilx_core::writeblock::WriteProtectionReport>,
    /// Son işlem mesajı.
    pub message: Option<String>,
    /// Son işlem başarılı mı?
    pub success: bool,
}

impl AppState {
    /// Yeni uygulama durumu oluşturur.
    pub fn new() -> Self {
        let settings = Settings::default();
        let mut state = AppState {
            current: Screen::Home,
            devices: Vec::new(),
            selected_device: None,
            source_path: String::new(),
            target_path: String::new(),
            job: RunningJob::default(),
            log: Vec::new(),
            status: "DelilX hazır. Başlamak için bir cihaz algılayın veya kaynak seçin."
                .to_string(),
            status_kind: 0,
            e01: E01Form::default(),
            logical: LogicalForm::default(),
            inspection: None,
            inspect_path: String::new(),
            verify: VerifyForm::default(),
            report_paths: Vec::new(),
            report: ReportForm::default(),
            raw_error_policy: 0,
            write_protect: WriteProtectState::default(),
            last_report: None,
            settings,
        };
        state.report.operation_number = delilx_core::report::generate_operation_number();
        state.report.evidence_number = state.settings.default_evidence_number.clone();
        state.report.case_number = state.settings.default_case_number.clone();
        state.e01.evidence_number = state.settings.default_evidence_number.clone();
        state.e01.case_number = state.settings.default_case_number.clone();
        state
    }

    /// RAW okuma hatası politikasını döndürür.
    pub fn raw_error_policy(&self) -> u8 {
        self.raw_error_policy
    }

    /// RAW okuma hatası politikasını ayarlar.
    pub fn set_raw_error_policy(&mut self, v: u8) {
        self.raw_error_policy = v;
    }

    /// Durum mesajını ayarlar.
    pub fn set_status(&mut self, kind: u8, msg: impl Into<String>) {
        self.status_kind = kind;
        self.status = msg.into();
    }

    /// İşlem kaydı ekler.
    pub fn log_info(&mut self, msg: impl Into<String>) {
        self.log.push(LogEntry {
            timestamp: delilx_core::timeutil::now_iso8601(),
            level: delilx_core::progress::LogLevel::Info,
            message: msg.into(),
        });
    }

    /// Uyarı kaydı ekler.
    pub fn log_warn(&mut self, msg: impl Into<String>) {
        self.log.push(LogEntry {
            timestamp: delilx_core::timeutil::now_iso8601(),
            level: delilx_core::progress::LogLevel::Warning,
            message: msg.into(),
        });
    }

    /// Hata kaydı ekler.
    pub fn log_error(&mut self, msg: impl Into<String>) {
        self.log.push(LogEntry {
            timestamp: delilx_core::timeutil::now_iso8601(),
            level: delilx_core::progress::LogLevel::Error,
            message: msg.into(),
        });
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Paylaşılan uygulama durumu türü.
pub type SharedState = Arc<std::sync::Mutex<AppState>>;
