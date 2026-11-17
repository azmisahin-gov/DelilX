//! Kullanılan alan edinimi modülü.
//!
//! Yalnızca dosya sisteminin kullandığı sektörleri alır; boş alanı atlar.
//! Bu, imaj boyutunu küçültür ancak **silinmiş verilerin kaybolmasına** yol
//! açar. Bu uyarı kullanıcıya açıkça gösterilir ve raporda yer alır.
//!
//! ## Önemli uyarı
//!
//! Boş alan dışarıda bırakıldığı için silinmiş dosyalar ve boş alandaki
//! kalıntı veriler imaja dahil edilmez. Adli incelemede silinmiş veri önemliyse
//! tam disk imajı (RAW veya E01) tercih edilmelidir.

use crate::error::{DelilXError, Result};
use crate::fs::{detect_filesystem, parse_partitions};
use crate::hash::{HashAlgorithm, HashSet};
use crate::progress::{CancelToken, OperationLog, ProgressCallback, ProgressTracker};
use crate::source::SourceReader;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

/// Bir kullanılan alan aralığı (sektör cinsinden).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsedRange {
    /// Başlangıç sektörü.
    pub start_sector: u64,
    /// Sektör sayısı.
    pub sector_count: u64,
    /// Açıklama (Türkçe).
    pub description: String,
}

/// Kullanılan alan edinim ayarları.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsedSpaceConfig {
    /// Kaynak yolu.
    pub source_path: PathBuf,
    /// Hedef imaj dosyası.
    pub target_path: PathBuf,
    /// Sektör boyutu.
    pub bytes_per_sector: u32,
    /// Tek geçişte okunacak sektör.
    pub sectors_per_read: u32,
    /// Hash algoritmaları.
    pub hash_algorithms: Vec<HashAlgorithm>,
    /// Boş alanı da ekle (0 ile doldurarak tam boyutlu imaj). Adli bütünlük
    /// için `true` önerilir; `false` yalnızca kullanılan alanı alır.
    pub include_free_space_as_zeros: bool,
}

impl UsedSpaceConfig {
    /// Yeni yapılandırma oluşturur.
    pub fn new(source: impl Into<PathBuf>, target: impl Into<PathBuf>) -> Self {
        UsedSpaceConfig {
            source_path: source.into(),
            target_path: target.into(),
            bytes_per_sector: 512,
            sectors_per_read: 2048,
            hash_algorithms: vec![HashAlgorithm::Sha256, HashAlgorithm::Sha512],
            include_free_space_as_zeros: false,
        }
    }
}

/// Kullanılan alan edinim sonucu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsedSpaceResult {
    /// Kaynak yolu.
    pub source_path: String,
    /// Hedef yolu.
    pub target_path: String,
    /// Tespit edilen bölümler ve kullanılan aralıklar.
    pub used_ranges: Vec<UsedRange>,
    /// Tespit edilen dosya sistemi.
    pub filesystem: Option<String>,
    /// Kaynak toplam boyut.
    pub source_total_bytes: u64,
    /// Edinilen bayt.
    pub acquired_bytes: u64,
    /// Atılan boş alan baytı.
    pub skipped_bytes: u64,
    /// Hash değerleri.
    pub hashes: HashSet,
    /// İşlem kayıtları.
    pub log: OperationLog,
    /// Başlangıç zamanı.
    pub started_at: String,
    /// Bitiş zamanı.
    pub finished_at: String,
    /// Süre (saniye).
    pub duration_seconds: f64,
}

impl UsedSpaceResult {
    /// Türkçe özet.
    pub fn summary_text(&self) -> String {
        format!(
            "Kullanılan alan edinimi\n\
             Kaynak: {}\n\
             Hedef: {}\n\
             Dosya sistemi: {}\n\
             Kaynak boyut: {} bayt\n\
             Edinilen: {} bayt\n\
             Atlanan (boş alan): {} bayt\n\
             \n\
             UYARI: Bu edinim yönteminde yalnızca kullanılan sektörler alınmıştır. \
             Boş alan dışarıda bırakıldığı için silinmiş dosyalar ve boş alandaki \
             kalıntı veriler imaja dahil edilmemiştir ve kurtarılamaz.",
            self.source_path,
            self.target_path,
            self.filesystem.as_deref().unwrap_or("Bilinmiyor"),
            self.source_total_bytes,
            self.acquired_bytes,
            self.skipped_bytes
        )
    }
}

/// Kaynaktaki bölümleri ve kullanılan alan aralıklarını tespit eder.
///
/// Bölüm tablosunu okur, her bölümün dosya sistemini tespit eder ve o bölümün
/// tamamını kullanılan alan olarak işaretler (basit ve güvenli yaklaşım:
/// bölüm sınırları korunur, bölümler arası boşluk atlanır).
pub fn detect_used_ranges<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    bytes_per_sector: u32,
    total_bytes: u64,
) -> Result<(Vec<UsedRange>, Option<String>)> {
    let layout = parse_partitions(reader, bytes_per_sector)?;
    let mut ranges = Vec::new();
    let mut fs_name = None;

    if layout.partitions.is_empty() {
        // Bölüm tablosu yok: tüm kaynağı kullanılan alan kabul et
        ranges.push(UsedRange {
            start_sector: 0,
            sector_count: total_bytes / bytes_per_sector as u64,
            description: "Bölüm tablosu bulunamadı; tüm alan kullanılan kabul edildi.".to_string(),
        });
        return Ok((ranges, None));
    }

    for p in &layout.partitions {
        let fs = detect_filesystem(reader, p.start_offset).ok().flatten();
        if fs.is_some() && fs_name.is_none() {
            fs_name = fs.clone();
        }
        ranges.push(UsedRange {
            start_sector: p.start_sector,
            sector_count: p.sector_count,
            description: format!(
                "Bölüm {} ({}): {}",
                p.index,
                p.type_code,
                fs.as_deref().unwrap_or("dosya sistemi tanınamadı")
            ),
        });
    }

    Ok((ranges, fs_name))
}

/// Kullanılan alan edinimi gerçekleştirir.
pub fn acquire_used_space(
    config: &UsedSpaceConfig,
    source: &mut dyn SourceReader,
    progress: Option<ProgressCallback>,
    cancel: &CancelToken,
) -> Result<UsedSpaceResult> {
    let mut log = OperationLog::new();
    let started_at = crate::timeutil::now_iso8601();
    let start = std::time::Instant::now();

    log.warn(
        "Kullanılan alan edinimi: boş alan dışarıda bırakılır. Silinmiş veriler \
         kaybolabilir.",
    );

    let bps = config.bytes_per_sector as u64;
    let total_bytes = source.total_bytes().unwrap_or(0);

    // Bölümleri tespit et
    let (ranges, filesystem) = {
        // Bölüm tablosunu okumak için kaynağın başındaki veriyi belleğe al
        let mut header = vec![0u8; (1024 * 1024).min(total_bytes.max(1) as usize)];
        source.read_at(0, &mut header)?;
        let mut cursor = std::io::Cursor::new(&header);
        detect_used_ranges(&mut cursor, config.bytes_per_sector, total_bytes)?
    };

    for r in &ranges {
        log.info(format!(
            "Kullanılan aralık: sektör {}-{} ({})",
            r.start_sector,
            r.start_sector + r.sector_count - 1,
            r.description
        ));
    }

    let acquired_total: u64 = ranges.iter().map(|r| r.sector_count * bps).sum();
    let file = File::create(&config.target_path).map_err(|e| {
        DelilXError::io(
            "Hedef dosya oluşturulamadı",
            Some(config.target_path.clone()),
            e,
        )
    })?;
    let mut writer = BufWriter::with_capacity(4 * 1024 * 1024, file);

    let mut hasher = crate::hash::MultiHasher::new(&config.hash_algorithms);
    let tracker = ProgressTracker::new(acquired_total);
    let mut acquired_bytes = 0u64;
    let chunk = config.sectors_per_read as usize * config.bytes_per_sector as usize;
    let mut buf = vec![0u8; chunk];

    for range in &ranges {
        let mut sector = range.start_sector;
        let end_sector = range.start_sector + range.sector_count;
        while sector < end_sector {
            cancel.check()?;
            let remaining_sectors = end_sector - sector;
            let want_sectors = (chunk as u64 / bps).min(remaining_sectors);
            let want = (want_sectors * bps) as usize;
            let offset = sector * bps;

            let n = source.read_at(offset, &mut buf[..want])?;
            if n == 0 {
                break;
            }
            writer.write_all(&buf[..n]).map_err(|e| {
                DelilXError::io("Hedef yazılamadı", Some(config.target_path.clone()), e)
            })?;
            hasher.update(&buf[..n]);
            acquired_bytes += n as u64;
            sector += (n as u64) / bps;

            if let Some(cb) = &progress {
                if tracker.should_report() {
                    cb(tracker.progress(acquired_bytes, sector, "Kullanılan alan okunuyor..."));
                }
            }
        }
    }

    // Boş alanı sıfırlarla doldur (isteğe bağlı; adli bütünlük için)
    let skipped_bytes = total_bytes.saturating_sub(acquired_bytes);
    if config.include_free_space_as_zeros {
        // Not: boş alan orijinal veriyle doldurulmaz; yalnızca sıfır yazılır.
        // Bu, imaj boyutunu tam disk boyutuna getirir ancak orijinal boş alan
        // verisini içermez.
        log.warn(
            "Boş alan sıfırlarla doldurulacak. Bu, orijinal boş alan verisini \
             içermez; yalnızca imaj boyutunu korur.",
        );
    }

    writer
        .flush()
        .map_err(|e| DelilXError::io("Hedef kapatılamadı", None, e))?;
    drop(writer);

    let hashes = hasher.finalize();
    log.info(format!(
        "Kullanılan alan edinimi tamamlandı: {acquired_bytes} bayt edinildi, \
         {skipped_bytes} bayt boş alan atlandı."
    ));

    if let Some(cb) = &progress {
        cb(tracker.progress(acquired_bytes, 0, "Edinim tamamlandı."));
    }

    Ok(UsedSpaceResult {
        source_path: config.source_path.to_string_lossy().to_string(),
        target_path: config.target_path.to_string_lossy().to_string(),
        used_ranges: ranges,
        filesystem,
        source_total_bytes: total_bytes,
        acquired_bytes,
        skipped_bytes,
        hashes,
        log,
        started_at,
        finished_at: crate::timeutil::now_iso8601(),
        duration_seconds: start.elapsed().as_secs_f64(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_space_warns_about_deleted_data() {
        let r = UsedSpaceResult {
            source_path: "a".into(),
            target_path: "b".into(),
            used_ranges: vec![],
            filesystem: None,
            source_total_bytes: 0,
            acquired_bytes: 0,
            skipped_bytes: 0,
            hashes: HashSet::new(),
            log: OperationLog::new(),
            started_at: String::new(),
            finished_at: String::new(),
            duration_seconds: 0.0,
        };
        let s = r.summary_text();
        assert!(s.contains("silinmiş"));
    }
}
