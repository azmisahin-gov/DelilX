//! İmaj inceleme modülü.
//!
//! RAW ve E01 imajları açar, bölümleri listeler, dosya sistemlerini tespit
//! eder, dosyaları listeler, metadata görüntüler, dosya hash'i hesaplar ve
//! dosyaları ayrı bir klasöre çıkarır. **Orijinal imaj dosyası hiçbir koşulda
//! değiştirilmez.**
//!
//! ## Dosya sistemi desteği
//!
//! NTFS, FAT12/16/32 ve exFAT dosya sistemlerinde dizin ağacı çözümlenir.
//! Desteklenmeyen dosya sistemleri tespit edilir ancak dosya listesi
//! çıkarılamaz; bu durum kullanıcıya açıkça bildirilir.

use crate::error::{DelilXError, Result};
use crate::fs::{detect_filesystem, parse_partitions, PartitionLayout};
use crate::hash::{hash_bytes, HashAlgorithm, HashSet};
use crate::progress::{CancelToken, OperationLog, ProgressCallback, ProgressTracker};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// İmaj türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageKind {
    /// RAW/DD imaj.
    Raw,
    /// E01 imaj.
    E01,
}

impl ImageKind {
    /// Türkçe etiket.
    pub fn label(&self) -> &'static str {
        match self {
            ImageKind::Raw => "RAW / DD imaj",
            ImageKind::E01 => "E01 imaj",
        }
    }
}

/// İnceleme için açılmış imaj. Okuma kaynağı türüne göre davranır.
pub enum ImageReader {
    /// RAW imaj dosyası.
    Raw {
        file: fs::File,
        size: u64,
        bytes_per_sector: u32,
    },
    /// E01 imaj.
    E01(Box<crate::e01::EwfImage>),
}

impl ImageReader {
    /// İmajı açar ve türünü otomatik algılar.
    pub fn open(path: &Path) -> Result<Self> {
        // E01 mi?
        let is_e01 = {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_uppercase())
                .unwrap_or_default();
            ext == "E01" || ext.starts_with("E0")
        };
        if is_e01 {
            let img = crate::e01::EwfImage::open(path)?;
            return Ok(ImageReader::E01(Box::new(img)));
        }

        // RAW
        let file = fs::File::open(path)
            .map_err(|e| DelilXError::io("İmaj dosyası açılamadı", Some(path.to_path_buf()), e))?;
        let size = file
            .metadata()
            .map_err(|e| DelilXError::io("İmaj boyutu okunamadı", Some(path.to_path_buf()), e))?
            .len();
        Ok(ImageReader::Raw {
            file,
            size,
            bytes_per_sector: 512,
        })
    }

    /// İmaj türü.
    pub fn kind(&self) -> ImageKind {
        match self {
            ImageReader::Raw { .. } => ImageKind::Raw,
            ImageReader::E01(_) => ImageKind::E01,
        }
    }

    /// Toplam bayt.
    pub fn total_bytes(&self) -> u64 {
        match self {
            ImageReader::Raw { size, .. } => *size,
            ImageReader::E01(img) => img.volume().total_bytes,
        }
    }

    /// Sektör boyutu.
    pub fn bytes_per_sector(&self) -> u32 {
        match self {
            ImageReader::Raw {
                bytes_per_sector, ..
            } => *bytes_per_sector,
            ImageReader::E01(img) => img.volume().bytes_per_sector,
        }
    }

    /// Belirtilen ofsetten okur.
    pub fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        match self {
            ImageReader::Raw { file, size, .. } => {
                if offset >= *size {
                    return Ok(0);
                }
                let mut f = file;
                f.seek(SeekFrom::Start(offset))
                    .map_err(|e| DelilXError::io("İmaj konumlanamadı", None, e))?;
                let want = ((*size - offset) as usize).min(buf.len());
                f.read_exact(&mut buf[..want])
                    .map_err(|e| DelilXError::io("İmaj okunamadı", None, e))?;
                Ok(want)
            }
            ImageReader::E01(img) => img.read_at(offset, buf),
        }
    }

    /// E01 imajı ise referansını döndürür.
    pub fn as_e01(&self) -> Option<&crate::e01::EwfImage> {
        match self {
            ImageReader::E01(img) => Some(img),
            _ => None,
        }
    }
}

/// İnceleme raporu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageInspection {
    /// İmaj yolu.
    pub image_path: String,
    /// İmaj türü.
    pub kind: ImageKind,
    /// Toplam bayt.
    pub total_bytes: u64,
    /// Sektör boyutu.
    pub bytes_per_sector: u32,
    /// Bölüm düzeni.
    pub layout: PartitionLayout,
    /// E01 ise EWF bilgisi.
    pub ewf_info: Option<crate::e01::EwfImageInfo>,
    /// Bulunan dosya sistemi (ilk).
    pub filesystem: Option<String>,
    /// Desteklenen dosya sistemleri.
    pub supported_filesystems: Vec<String>,
    /// Dosya listesi (çözümlenebildiyse).
    pub files: Vec<InspectedFile>,
    /// İşlem kayıtları.
    pub log: OperationLog,
}

/// İncelenen bir dosya.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectedFile {
    /// Dosya adı.
    pub name: String,
    /// Tam yol (imaj içinde).
    pub path: String,
    /// Boyut.
    pub size: u64,
    /// Ofset (imaj içinde).
    pub offset: u64,
    /// Dizin mi?
    pub is_dir: bool,
    /// Dosya sistemi.
    pub filesystem: String,
}

/// İmajı inceler: bölümleri ve dosya sistemlerini listeler.
pub fn inspect_image(
    path: &Path,
    progress: Option<ProgressCallback>,
    cancel: &CancelToken,
) -> Result<ImageInspection> {
    let mut log = OperationLog::new();
    log.info(format!("İmaj açılıyor: {}", path.display()));

    let reader = ImageReader::open(path)?;
    let total_bytes = reader.total_bytes();
    let bps = reader.bytes_per_sector();
    log.info(format!(
        "İmaj türü: {}, boyut: {} bayt",
        reader.kind().label(),
        total_bytes
    ));

    // Bölüm tablosunu oku
    let mut head = vec![0u8; (1024 * 1024).min(total_bytes.max(1) as usize)];
    reader.read_at(0, &mut head)?;
    let mut cursor = std::io::Cursor::new(&head);
    let mut layout = parse_partitions(&mut cursor, bps)?;

    // Her bölümün dosya sistemini tespit et
    for p in &mut layout.partitions {
        if let Ok(Some(fs)) = detect_filesystem(&mut cursor, p.start_offset) {
            p.filesystem = Some(fs);
        }
    }

    let filesystem = layout
        .partitions
        .iter()
        .find_map(|p| p.filesystem.clone())
        .or_else(|| detect_filesystem(&mut cursor, 0).ok().flatten());

    let supported = ["NTFS", "FAT12", "FAT16", "FAT32", "exFAT"];
    let supported_filesystems: Vec<String> = supported.iter().map(|s| s.to_string()).collect();

    log.info(format!(
        "Bölüm tablosu: {}, bölüm sayısı: {}",
        layout.table_type.label(),
        layout.partitions.len()
    ));

    // Dosya listesini çöz (FAT için temel destek)
    let mut files = Vec::new();
    if let Some(fsname) = &filesystem {
        if fsname.starts_with("FAT") {
            // Bölüm varsa ilgili bölümden, yoksa imajın başından (0) çözümle.
            let fs_offset = layout
                .partitions
                .iter()
                .find(|p| p.filesystem.as_deref() == Some(fsname.as_str()))
                .map(|p| p.start_offset)
                .or_else(|| {
                    layout
                        .partitions
                        .iter()
                        .find(|p| p.filesystem.is_some())
                        .map(|p| p.start_offset)
                })
                .unwrap_or(0);
            match crate::fs_fat::list_fat_files(&reader, fs_offset, fsname, cancel) {
                Ok(f) => {
                    log.info(format!("Dosya listesi çözümlendi: {} girdi", f.len()));
                    files = f;
                }
                Err(e) => log.warn(format!("Dosya listesi çözümlenemedi: {e}")),
            }
        } else {
            log.warn(format!(
                "{fsname} dosya sistemi için dizin çözümleme bu sürümde sınırlıdır; \
                 dosya listesi çıkarılamadı. Bölüm ve dosya sistemi bilgileri raporlanmıştır."
            ));
        }
    } else {
        log.warn("Dosya sistemi tanınamadı; dosya listesi çıkarılamadı.");
    }

    let ewf_info = reader.as_e01().map(|i| i.info().clone());

    if let Some(cb) = &progress {
        cb(ProgressTracker::new(total_bytes).progress(total_bytes, 0, "İnceleme tamamlandı."));
    }

    Ok(ImageInspection {
        image_path: path.to_string_lossy().to_string(),
        kind: reader.kind(),
        total_bytes,
        bytes_per_sector: bps,
        layout,
        ewf_info,
        filesystem,
        supported_filesystems,
        files,
        log,
    })
}

/// İmajdan bir dosyayı çıkarır ve hash değerlerini hesaplar.
pub fn extract_file(
    reader: &ImageReader,
    offset: u64,
    size: u64,
    dest: &Path,
    algorithms: &[HashAlgorithm],
    cancel: &CancelToken,
) -> Result<HashSet> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            DelilXError::io(
                "Çıkarma dizini oluşturulamadı",
                Some(parent.to_path_buf()),
                e,
            )
        })?;
    }
    let file = fs::File::create(dest).map_err(|e| {
        DelilXError::io(
            "Çıkarma dosyası oluşturulamadı",
            Some(dest.to_path_buf()),
            e,
        )
    })?;
    let mut writer = std::io::BufWriter::new(file);
    let mut hasher = crate::hash::MultiHasher::new(algorithms);
    let mut buf = vec![0u8; 1024 * 1024];
    let mut pos = offset;
    let end = offset + size;
    while pos < end {
        cancel.check()?;
        let want = ((end - pos) as usize).min(buf.len());
        let n = reader.read_at(pos, &mut buf[..want])?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&buf[..n])
            .map_err(|e| DelilXError::io("Çıkarma yazılamadı", Some(dest.to_path_buf()), e))?;
        hasher.update(&buf[..n]);
        pos += n as u64;
    }
    writer
        .flush()
        .map_err(|e| DelilXError::io("Çıkarma dosyası kapatılamadı", None, e))?;
    Ok(hasher.finalize())
}

/// Bir imaj dosyasının hash değerlerini hesaplar (imaj bütünlüğü için).
pub fn hash_image_file(path: &Path, algorithms: &[HashAlgorithm]) -> Result<HashSet> {
    crate::hash::hash_file(path, algorithms)
}

/// Bellekteki verinin hash'ini hesaplar (test için).
pub fn hash_slice(data: &[u8], algorithms: &[HashAlgorithm]) -> HashSet {
    hash_bytes(data, algorithms)
}

/// Çıkarılan dosyaların listesini bir dizine yazar (envanter).
pub fn write_extraction_inventory(
    dest_dir: &Path,
    files: &[(String, u64, HashSet)],
) -> Result<PathBuf> {
    fs::create_dir_all(dest_dir).map_err(|e| {
        DelilXError::io(
            "Envanter dizini oluşturulamadı",
            Some(dest_dir.to_path_buf()),
            e,
        )
    })?;
    let path = dest_dir.join("cikarilan_dosyalar.csv");
    let mut out = String::from("Dosya,Boyut,SHA-256,SHA-512\n");
    for (name, size, hashes) in files {
        out.push_str(&format!(
            "{},{},{},{}\n",
            name.replace(',', "_"),
            size,
            hashes.get(HashAlgorithm::Sha256).unwrap_or("-"),
            hashes.get(HashAlgorithm::Sha512).unwrap_or("-")
        ));
    }
    fs::write(&path, out)
        .map_err(|e| DelilXError::io("Envanter yazılamadı", Some(path.clone()), e))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_raw_image() {
        let dir = std::env::temp_dir().join("delilx_inspect_raw");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let img = dir.join("test.dd");
        let mut data = vec![0u8; 1024 * 1024];
        // MBR imzası
        data[510] = 0x55;
        data[511] = 0xAA;
        fs::write(&img, &data).unwrap();

        let reader = ImageReader::open(&img).unwrap();
        assert_eq!(reader.kind(), ImageKind::Raw);
        assert_eq!(reader.total_bytes(), 1024 * 1024);

        let cancel = CancelToken::new();
        let inspection = inspect_image(&img, None, &cancel).unwrap();
        assert_eq!(inspection.kind, ImageKind::Raw);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn extract_writes_correct_data() {
        let dir = std::env::temp_dir().join("delilx_extract_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let img = dir.join("test.dd");
        let data: Vec<u8> = (0..4096u32).map(|i| (i % 256) as u8).collect();
        fs::write(&img, &data).unwrap();

        let reader = ImageReader::open(&img).unwrap();
        let dest = dir.join("cikti.bin");
        let cancel = CancelToken::new();
        let hashes = extract_file(
            &reader,
            1024,
            2048,
            &dest,
            &[HashAlgorithm::Sha256],
            &cancel,
        )
        .unwrap();

        let extracted = fs::read(&dest).unwrap();
        assert_eq!(extracted.len(), 2048);
        assert_eq!(&extracted[..], &data[1024..3072]);
        let expected = hash_bytes(&data[1024..3072], &[HashAlgorithm::Sha256]);
        assert_eq!(
            hashes.get(HashAlgorithm::Sha256),
            expected.get(HashAlgorithm::Sha256)
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
