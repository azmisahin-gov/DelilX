//! RAW/DD imaj edinim motoru.
//!
//! Sektör sektör okuma, okuma hatası kaydı, ilerleme göstergesi ve SHA-256 /
//! SHA-512 doğrulaması sağlar. Kaynağa hiçbir koşulda yazmaz.

use crate::error::{DelilXError, Result};
use crate::hash::{hash_file, HashAlgorithm, HashSet};
use crate::progress::{CancelToken, OperationLog, ProgressCallback, ProgressTracker};
use crate::source::SourceReader;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

/// Okuma hatası politikası.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadErrorPolicy {
    /// Hatalı sektörü sıfırlarla doldur ve devam et (bad sector zerofill).
    ZeroFill,
    /// İlk okuma hatasında işlemi durdur.
    Abort,
    /// Belirtilen sayıda hatadan sonra durdur.
    MaxErrors {
        /// İzin verilen en fazla hata sayısı.
        max: u64,
    },
}

/// RAW/DD edinim ayarları.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawAcquisitionConfig {
    /// Kaynak yolu.
    pub source_path: PathBuf,
    /// Hedef imaj dosyası.
    pub target_path: PathBuf,
    /// Sektör boyutu.
    pub bytes_per_sector: u32,
    /// Tek geçişte okunacak sektör sayısı.
    pub sectors_per_read: u32,
    /// Okuma hatası politikası.
    pub error_policy: ReadErrorPolicy,
    /// Her hatalı sektörde kaç kez yeniden denenecek.
    pub read_retries: u32,
    /// Hesaplanacak hash algoritmaları.
    pub hash_algorithms: Vec<HashAlgorithm>,
    /// Edinim bittikten sonra hedef imaj yeniden okunarak doğrulanacak mı?
    pub verify_after: bool,
}

impl RawAcquisitionConfig {
    /// Varsayılan ayarlarla yapılandırma oluşturur.
    pub fn new(source: impl Into<PathBuf>, target: impl Into<PathBuf>) -> Self {
        RawAcquisitionConfig {
            source_path: source.into(),
            target_path: target.into(),
            bytes_per_sector: 512,
            sectors_per_read: 2048,
            error_policy: ReadErrorPolicy::ZeroFill,
            read_retries: 3,
            hash_algorithms: vec![HashAlgorithm::Sha256, HashAlgorithm::Sha512],
            verify_after: true,
        }
    }
}

/// RAW edinim sonucu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawAcquisitionResult {
    /// Kaynak yolu.
    pub source_path: String,
    /// Hedef imaj yolu.
    pub target_path: String,
    /// Okunan toplam bayt.
    pub total_bytes: u64,
    /// Toplam sektör.
    pub total_sectors: u64,
    /// Hatalı okunan sektör sayısı.
    pub error_sector_count: u64,
    /// Okuma hatası kayıtları.
    pub read_errors: Vec<crate::source::ReadErrorRecord>,
    /// Kaynak veri hash değerleri.
    pub source_hashes: HashSet,
    /// Hedef imajdan yeniden hesaplanan hash değerleri.
    pub target_hashes: Option<HashSet>,
    /// Doğrulama başarılı mı? (`None` = doğrulama yapılmadı)
    pub verified: Option<bool>,
    /// Edinim başlangıç zamanı.
    pub started_at: String,
    /// Edinim bitiş zamanı.
    pub finished_at: String,
    /// Toplam süre (saniye).
    pub duration_seconds: f64,
    /// İşlem kayıtları.
    pub log: OperationLog,
}

impl RawAcquisitionResult {
    /// Hata ve uyarı özeti (Türkçe).
    pub fn summary_text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "Kaynak: {}\nHedef: {}\nOkunan: {} bayt ({} sektör)\n",
            self.source_path, self.target_path, self.total_bytes, self.total_sectors
        ));
        s.push_str(&format!(
            "Okuma hatası: {} sektör\n",
            self.error_sector_count
        ));
        for h in &self.source_hashes.values {
            s.push_str(&format!("{}: {}\n", h.algorithm.name(), h.value));
        }
        match self.verified {
            Some(true) => s.push_str("Doğrulama: BAŞARILI (hash değerleri eşleşti)\n"),
            Some(false) => s.push_str("Doğrulama: BAŞARISIZ (hash değerleri eşleşmedi!)\n"),
            None => s.push_str("Doğrulama: yapılmadı\n"),
        }
        s
    }
}

/// RAW/DD imaj edinimi gerçekleştirir.
///
/// Sektör sektör okur, okuma hatalarını kaydeder ve isteğe bağlı olarak hedefi
/// yeniden okuyarak hash doğrulaması yapar.
pub fn acquire_raw(
    config: &RawAcquisitionConfig,
    source: &mut dyn SourceReader,
    progress: Option<ProgressCallback>,
    cancel: &CancelToken,
) -> Result<RawAcquisitionResult> {
    let mut log = OperationLog::new();
    let started_at = crate::timeutil::now_iso8601();
    let start_instant = std::time::Instant::now();

    let bps = config.bytes_per_sector as usize;
    if bps == 0 {
        return Err(DelilXError::other("Sektör boyutu sıfır olamaz."));
    }

    let total = source.total_bytes();
    log.info(format!("Edinim başladı: {}", source.description()));
    log.info(format!("Hedef imaj: {}", config.target_path.display()));
    log.info(format!("Sektör boyutu: {bps} bayt"));
    log.info(format!(
        "Hash algoritmaları: {}",
        config
            .hash_algorithms
            .iter()
            .map(|a| a.name())
            .collect::<Vec<_>>()
            .join(", ")
    ));

    let file = File::create(&config.target_path).map_err(|e| {
        DelilXError::io(
            "Hedef imaj dosyası oluşturulamadı",
            Some(config.target_path.clone()),
            e,
        )
    })?;
    let mut writer = BufWriter::with_capacity(4 * 1024 * 1024, file);

    let mut hasher = crate::hash::MultiHasher::new(&config.hash_algorithms);
    let tracker = ProgressTracker::new(total.unwrap_or(0));
    let mut read_errors: Vec<crate::source::ReadErrorRecord> = Vec::new();
    let mut error_sector_count: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut total_sectors: u64 = 0;
    let mut offset: u64 = 0;

    let chunk_bytes = config.sectors_per_read as usize * bps;
    let mut buf = vec![0u8; chunk_bytes];

    loop {
        cancel.check()?;

        if let Some(t) = total {
            if offset >= t {
                break;
            }
        }

        // Kalan bayt sayısı
        let want = match total {
            Some(t) => ((t - offset) as usize).min(chunk_bytes),
            None => chunk_bytes,
        };
        // Sektör katına yuvarlama (tamponsuz okuma için zorunlu)
        let want_sectors = want.div_ceil(bps);
        let want_aligned = (want_sectors * bps).min(chunk_bytes);
        if want_aligned == 0 {
            break;
        }

        let mut attempt = 0;
        let read_result = loop {
            match source.read_at(offset, &mut buf[..want_aligned]) {
                Ok(n) => break Ok(n),
                Err(e) => {
                    attempt += 1;
                    if attempt > config.read_retries {
                        break Err(e);
                    }
                    log.warn(format!(
                        "Okuma yeniden deneniyor ({} sektör, deneme {}/{}): {e}",
                        offset / bps as u64,
                        attempt,
                        config.read_retries
                    ));
                }
            }
        };

        let n = match read_result {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                // Okuma hatası: politikaya göre davran
                let sector = offset / bps as u64;
                match config.error_policy {
                    ReadErrorPolicy::Abort => {
                        log.error(format!("Okuma hatası, işlem durduruldu: {e}"));
                        writer
                            .flush()
                            .map_err(|e| DelilXError::io("Yazma hatası", None, e))?;
                        return Err(e);
                    }
                    ReadErrorPolicy::MaxErrors { max } if error_sector_count >= max => {
                        log.error(format!(
                            "İzin verilen en fazla hata sayısına ({max}) ulaşıldı, işlem durduruldu."
                        ));
                        writer
                            .flush()
                            .map_err(|e| DelilXError::io("Yazma hatası", None, e))?;
                        return Err(DelilXError::TooManyReadErrors {
                            sector,
                            count: error_sector_count,
                        });
                    }
                    _ => {
                        // ZeroFill: hatalı bölgeyi sıfırla ve devam et
                        log.warn(format!(
                            "Okuma hatası (sektör {sector}): {e}. Bu bölge sıfırlarla dolduruldu."
                        ));
                        read_errors.push(crate::source::ReadErrorRecord {
                            sector,
                            bytes_per_sector: bps as u32,
                            message: e.to_string(),
                        });
                        error_sector_count += 1;
                        for b in buf[..want_aligned].iter_mut() {
                            *b = 0;
                        }
                        want_aligned
                    }
                }
            }
        };

        writer
            .write_all(&buf[..n])
            .map_err(|e| DelilXError::io("İmaj yazılamadı", Some(config.target_path.clone()), e))?;
        hasher.update(&buf[..n]);
        total_bytes += n as u64;
        total_sectors += (n / bps) as u64;
        offset += n as u64;

        if let Some(cb) = &progress {
            if tracker.should_report() {
                cb(tracker.progress(total_bytes, total_sectors, "Veri okunuyor ve yazılıyor..."));
            }
        }

        // Hatalı sektör durumunda ofseti tam sektör hizasına ilerlet
        if n < want_aligned {
            offset += (want_aligned - n) as u64;
        }
    }

    writer
        .flush()
        .map_err(|e| DelilXError::io("İmaj dosyası kapatılamadı", None, e))?;
    drop(writer);

    let source_hashes = hasher.finalize();
    log.info(format!("Edinim tamamlandı: {total_bytes} bayt yazıldı."));
    if error_sector_count > 0 {
        log.warn(format!(
            "{error_sector_count} sektör okuma hatası nedeniyle sıfırlarla dolduruldu."
        ));
    }

    // Hedef doğrulama
    let (target_hashes, verified) = if config.verify_after {
        log.info("Hedef imaj yeniden okunarak doğrulanıyor...");
        let th = hash_file(&config.target_path, &config.hash_algorithms)?;
        let outcome = crate::hash::verify_hashes(&source_hashes, &th);
        let ok = outcome.is_success();
        if ok {
            log.info("Doğrulama başarılı: kaynak ve hedef hash değerleri eşleşti.");
        } else {
            log.error("Doğrulama BAŞARISIZ: kaynak ve hedef hash değerleri eşleşmedi!");
            for (hv, actual) in &outcome.mismatches {
                log.error(format!(
                    "  {}: kaynak {} / hedef {}",
                    hv.algorithm.name(),
                    hv.value,
                    actual
                ));
            }
        }
        (Some(th), Some(ok))
    } else {
        log.info("Hedef doğrulaması istenmedi.");
        (None, None)
    };

    if let Some(cb) = &progress {
        cb(tracker.progress(total_bytes, total_sectors, "Edinim tamamlandı."));
    }

    let finished_at = crate::timeutil::now_iso8601();
    let duration_seconds = start_instant.elapsed().as_secs_f64();

    Ok(RawAcquisitionResult {
        source_path: config.source_path.to_string_lossy().to_string(),
        target_path: config.target_path.to_string_lossy().to_string(),
        total_bytes,
        total_sectors,
        error_sector_count,
        read_errors,
        source_hashes,
        target_hashes,
        verified,
        started_at,
        finished_at,
        duration_seconds,
        log,
    })
}

/// Bir RAW imaj dosyasını okuyucu olarak açar.
pub fn open_raw_image(path: &Path) -> Result<crate::source::FileSource> {
    crate::source::FileSource::open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_source_file(path: &Path, size: usize) -> Vec<u8> {
        let data: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        let mut f = File::create(path).unwrap();
        f.write_all(&data).unwrap();
        drop(f);
        data
    }

    #[test]
    fn raw_acquisition_produces_identical_image() {
        let dir = std::env::temp_dir().join("delilx_raw_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src_path = dir.join("kaynak.bin");
        let tgt_path = dir.join("cikti.dd");
        let data = make_source_file(&src_path, 1024 * 512);

        let config = RawAcquisitionConfig {
            source_path: src_path.clone(),
            target_path: tgt_path.clone(),
            bytes_per_sector: 512,
            sectors_per_read: 256,
            error_policy: ReadErrorPolicy::ZeroFill,
            read_retries: 1,
            hash_algorithms: vec![HashAlgorithm::Sha256, HashAlgorithm::Sha512],
            verify_after: true,
        };

        let mut source = crate::source::FileSource::open(&src_path).unwrap();
        let cancel = CancelToken::new();
        let result = acquire_raw(&config, &mut source, None, &cancel).unwrap();

        assert_eq!(result.total_bytes, data.len() as u64);
        assert_eq!(result.error_sector_count, 0);
        assert_eq!(result.verified, Some(true));

        let written = std::fs::read(&tgt_path).unwrap();
        assert_eq!(written, data);

        // Kaynak hash, bağımsız hesapla eşleşmeli
        let expected = crate::hash::hash_bytes(&data, &[HashAlgorithm::Sha256]);
        assert_eq!(
            result.source_hashes.get(HashAlgorithm::Sha256),
            expected.get(HashAlgorithm::Sha256)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn raw_acquisition_can_be_cancelled() {
        let dir = std::env::temp_dir().join("delilx_raw_cancel");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src_path = dir.join("kaynak.bin");
        let tgt_path = dir.join("cikti.dd");
        make_source_file(&src_path, 1024 * 1024);

        let config = RawAcquisitionConfig::new(&src_path, &tgt_path);
        let mut source = crate::source::FileSource::open(&src_path).unwrap();
        let cancel = CancelToken::new();
        cancel.cancel(); // hemen iptal
        let result = acquire_raw(&config, &mut source, None, &cancel);
        assert!(matches!(result, Err(DelilXError::Cancelled)));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
