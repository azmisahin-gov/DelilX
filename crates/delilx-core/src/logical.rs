//! Mantıksal ve seçmeli edinim modülü.
//!
//! Mantıksal edinim mevcut dosyaları alır ve klasör yapısını korur. Seçmeli
//! edinim kullanıcının seçtiği dosya/klasörleri alır. Her iki yöntemde de
//! dosya metadata'sı ve SHA-256/SHA-512 hash değerleri kaydedilir, dosya
//! envanteri oluşturulur.
//!
//! ## Sınırlamalar (raporda açıkça belirtilir)
//!
//! Mantıksal ve seçmeli edinim yalnızca mevcut dosyaları alır. Silinmiş
//! dosyalar, boş alan kalıntıları ve dosya sistemi üst verisi edinilmez.

use crate::error::{DelilXError, Result};
use crate::hash::{hash_file, HashAlgorithm, HashSet};
use crate::metadata::{metadata_csv_header, metadata_csv_row, FileInventory, FileMetadata};
use crate::progress::{CancelToken, OperationLog, ProgressCallback, ProgressTracker};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Mantıksal/seçmeli edinim ayarları.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicalConfig {
    /// Kaynak kök dizin(ler).
    pub sources: Vec<PathBuf>,
    /// Hedef dizin.
    pub target_dir: PathBuf,
    /// Hash algoritmaları.
    pub hash_algorithms: Vec<HashAlgorithm>,
    /// Dosyaları hedefe kopyala mı? (yalnızca envanter isteniyorsa `false`)
    pub copy_files: bool,
    /// Klasör yapısını koru.
    pub preserve_structure: bool,
    /// Dahil edilecek uzantılar (boşsa hepsi).
    pub include_extensions: Vec<String>,
    /// Hariç tutulacak yollar (alt dizin/dosya adı parçaları).
    pub exclude_patterns: Vec<String>,
}

impl LogicalConfig {
    /// Yeni yapılandırma oluşturur.
    pub fn new(sources: Vec<PathBuf>, target: impl Into<PathBuf>) -> Self {
        LogicalConfig {
            sources,
            target_dir: target.into(),
            hash_algorithms: vec![HashAlgorithm::Sha256, HashAlgorithm::Sha512],
            copy_files: true,
            preserve_structure: true,
            include_extensions: Vec::new(),
            exclude_patterns: Vec::new(),
        }
    }

    fn is_excluded(&self, path: &Path) -> bool {
        let s = path.to_string_lossy();
        self.exclude_patterns.iter().any(|p| s.contains(p.as_str()))
    }

    fn is_included(&self, path: &Path) -> bool {
        if self.include_extensions.is_empty() {
            return true;
        }
        match path.extension() {
            Some(ext) => {
                let ext = ext.to_string_lossy().to_lowercase();
                self.include_extensions
                    .iter()
                    .any(|e| e.trim_start_matches('.').to_lowercase() == ext)
            }
            None => false,
        }
    }
}

/// Mantıksal edinim sonucu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicalResult {
    /// Dosya envanteri.
    pub inventory: FileInventory,
    /// Hash değerleri (göreli yol -> hash kümesi).
    pub hashes: Vec<(String, HashSet)>,
    /// Kopyalanan dosya sayısı.
    pub copied_files: u64,
    /// Toplam boyut.
    pub total_bytes: u64,
    /// Hata/uyarı kayıtları.
    pub log: OperationLog,
    /// Başlangıç zamanı.
    pub started_at: String,
    /// Bitiş zamanı.
    pub finished_at: String,
    /// Süre (saniye).
    pub duration_seconds: f64,
}

impl LogicalResult {
    /// Envanteri CSV biçiminde döndürür (başlık + satırlar).
    pub fn inventory_csv(&self) -> String {
        let mut out = String::new();
        out.push_str(&metadata_csv_header().join(","));
        out.push('\n');
        for f in &self.inventory.files {
            let row: Vec<String> = metadata_csv_row(f).iter().map(|c| csv_escape(c)).collect();
            out.push_str(&row.join(","));
            out.push('\n');
        }
        out
    }

    /// Hash listesini CSV biçiminde döndürür.
    pub fn hashes_csv(&self) -> String {
        let mut out = String::from("Dosya,SHA-256,SHA-512\n");
        for (path, hashes) in &self.hashes {
            out.push_str(&format!(
                "{},{},{}\n",
                csv_escape(path),
                hashes.get(HashAlgorithm::Sha256).unwrap_or("-"),
                hashes.get(HashAlgorithm::Sha512).unwrap_or("-")
            ));
        }
        out
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Mantıksal veya seçmeli edinim gerçekleştirir.
pub fn acquire_logical(
    config: &LogicalConfig,
    progress: Option<ProgressCallback>,
    cancel: &CancelToken,
) -> Result<LogicalResult> {
    let mut log = OperationLog::new();
    let started_at = crate::timeutil::now_iso8601();
    let start = std::time::Instant::now();

    if config.copy_files {
        fs::create_dir_all(&config.target_dir).map_err(|e| {
            DelilXError::io(
                "Hedef dizin oluşturulamadı",
                Some(config.target_dir.clone()),
                e,
            )
        })?;
    }

    log.info(format!(
        "Mantıksal edinim başladı. Kaynak sayısı: {}",
        config.sources.len()
    ));

    // Kaynak dosyaları topla
    let mut all_files: Vec<PathBuf> = Vec::new();
    for src in &config.sources {
        if src.is_file() {
            if !config.is_excluded(src) && config.is_included(src) {
                all_files.push(src.clone());
            }
        } else if src.is_dir() {
            for entry in walkdir::WalkDir::new(src).follow_links(false) {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        log.warn(format!("Dizin girdisi okunamadı: {e}"));
                        continue;
                    }
                };
                let p = entry.path();
                if !entry.file_type().is_file() {
                    continue;
                }
                if config.is_excluded(p) || !config.is_included(p) {
                    continue;
                }
                all_files.push(p.to_path_buf());
            }
        } else {
            log.warn(format!("Kaynak bulunamadı: {}", src.display()));
        }
    }

    log.info(format!("Edinilecek dosya sayısı: {}", all_files.len()));

    let total_bytes: u64 = all_files
        .iter()
        .filter_map(|p| fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();
    let tracker = ProgressTracker::new(total_bytes);

    let mut files_meta: Vec<FileMetadata> = Vec::new();
    let mut hashes: Vec<(String, HashSet)> = Vec::new();
    let mut copied = 0u64;
    let mut done_bytes = 0u64;

    for path in &all_files {
        cancel.check()?;

        // Kök dizini belirle (göreli yol için)
        let root = config
            .sources
            .iter()
            .find(|s| path.starts_with(s))
            .map(|s| {
                if s.is_file() {
                    s.parent().unwrap_or(s)
                } else {
                    s.as_path()
                }
            })
            .unwrap_or_else(|| Path::new(""));

        let mut meta = match crate::metadata::read_file_metadata(path, root) {
            Ok(m) => m,
            Err(e) => {
                log.warn(format!("Metadata okunamadı ({}): {e}", path.display()));
                continue;
            }
        };

        // Hash hesapla
        match hash_file(path, &config.hash_algorithms) {
            Ok(h) => {
                meta.sha256 = h.get(HashAlgorithm::Sha256).map(|s| s.to_string());
                meta.sha512 = h.get(HashAlgorithm::Sha512).map(|s| s.to_string());
                hashes.push((meta.relative_path.clone(), h));
            }
            Err(e) => {
                log.error(format!("Hash hesaplanamadı ({}): {e}", path.display()));
                meta.unsupported_fields
                    .push("Hash hesaplanamadı (okuma hatası)".to_string());
            }
        }

        // Kopyala
        if config.copy_files {
            let rel = if config.preserve_structure {
                meta.relative_path.clone()
            } else {
                meta.file_name.clone()
            };
            let dest = config.target_dir.join(&rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    DelilXError::io(
                        "Hedef alt dizin oluşturulamadı",
                        Some(parent.to_path_buf()),
                        e,
                    )
                })?;
            }
            match fs::copy(path, &dest) {
                Ok(_) => {
                    copied += 1;
                    // Kopya doğrulaması: hedef dosya boyutu kaynakla eşit olmalı
                    let src_len = meta.size;
                    let dst_len = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                    if src_len != dst_len {
                        log.warn(format!(
                            "Kopya boyut uyuşmazlığı: {} (kaynak {src_len}, hedef {dst_len})",
                            meta.relative_path
                        ));
                    }
                }
                Err(e) => log.error(format!("Dosya kopyalanamadı ({}): {e}", path.display())),
            }
        }

        done_bytes += meta.size;
        files_meta.push(meta);

        if let Some(cb) = &progress {
            if tracker.should_report() {
                cb(tracker.progress(done_bytes, files_meta.len() as u64, "Dosyalar işleniyor..."));
            }
        }
    }

    let file_count = files_meta.iter().filter(|m| !m.is_dir).count() as u64;
    let dir_count = files_meta.iter().filter(|m| m.is_dir).count() as u64;
    let inventory = FileInventory {
        root: config.sources.first().cloned().unwrap_or_default(),
        files: files_meta,
        file_count,
        dir_count,
        total_bytes,
    };

    log.info(format!(
        "Mantıksal edinim tamamlandı: {file_count} dosya, {copied} kopyalandı."
    ));

    if let Some(cb) = &progress {
        cb(tracker.progress(total_bytes, file_count, "Edinim tamamlandı."));
    }

    Ok(LogicalResult {
        inventory,
        hashes,
        copied_files: copied,
        total_bytes,
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
    fn logical_acquisition_copies_and_hashes() {
        let dir = std::env::temp_dir().join("delilx_logical_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("kaynak/alt")).unwrap();
        fs::create_dir_all(dir.join("hedef")).unwrap();
        fs::write(dir.join("kaynak/a.txt"), b"aaa").unwrap();
        fs::write(dir.join("kaynak/alt/b.txt"), b"bbbb").unwrap();

        let config = LogicalConfig::new(vec![dir.join("kaynak")], dir.join("hedef"));
        let cancel = CancelToken::new();
        let result = acquire_logical(&config, None, &cancel).unwrap();

        assert_eq!(result.inventory.file_count, 2);
        assert_eq!(result.copied_files, 2);
        assert_eq!(result.total_bytes, 7);
        assert_eq!(result.hashes.len(), 2);
        // Klasör yapısı korunmalı
        assert!(dir.join("hedef/a.txt").exists());
        assert!(dir.join("hedef/alt/b.txt").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn logical_extension_filter() {
        let dir = std::env::temp_dir().join("delilx_logical_ext");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("kaynak")).unwrap();
        fs::write(dir.join("kaynak/a.txt"), b"aaa").unwrap();
        fs::write(dir.join("kaynak/b.jpg"), b"jpg").unwrap();

        let mut config = LogicalConfig::new(vec![dir.join("kaynak")], dir.join("hedef"));
        config.copy_files = false;
        config.include_extensions = vec!["txt".to_string()];
        let cancel = CancelToken::new();
        let result = acquire_logical(&config, None, &cancel).unwrap();
        assert_eq!(result.inventory.file_count, 1);
        assert_eq!(result.inventory.files[0].file_name, "a.txt");

        let _ = fs::remove_dir_all(&dir);
    }
}
