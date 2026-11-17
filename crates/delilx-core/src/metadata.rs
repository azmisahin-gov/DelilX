//! Dosya ve dosya sistemi metadata modülü.
//!
//! Metadata, hash değerlerinden bağımsız olarak değerlendirilir. Korunamayan
//! alanlar raporda açıkça belirtilir.

use crate::error::{DelilXError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Tek bir dosyanın metadata bilgisi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// Dosya adı.
    pub file_name: String,
    /// Tam yol.
    pub full_path: String,
    /// Göreli yol (edinim köküne göre).
    pub relative_path: String,
    /// Dosya boyutu (bayt).
    pub size: u64,
    /// Oluşturulma tarihi.
    pub created: Option<String>,
    /// Son değiştirilme tarihi.
    pub modified: Option<String>,
    /// Son erişim tarihi.
    pub accessed: Option<String>,
    /// Salt okunur özniteliği.
    pub readonly: bool,
    /// Gizli özniteliği.
    pub hidden: bool,
    /// Sistem dosyası özniteliği.
    pub system: bool,
    /// Arşiv özniteliği.
    pub archive: bool,
    /// Dizin mi?
    pub is_dir: bool,
    /// SHA-256 (hesaplanmışsa).
    pub sha256: Option<String>,
    /// SHA-512 (hesaplanmışsa).
    pub sha512: Option<String>,
    /// Metadata tam olarak korunamayan alanlar (Türkçe açıklamalar).
    pub unsupported_fields: Vec<String>,
}

/// Dosya sistemi türü bilgisi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemInfo {
    /// Sürücü harfi veya bağlama noktası.
    pub mount_point: String,
    /// Dosya sistemi türü (ör. NTFS, FAT32, exFAT).
    pub fs_type: String,
    /// Toplam kapasite (bayt).
    pub total_bytes: u64,
    /// Boş alan (bayt).
    pub free_bytes: u64,
    /// Birim etiketi.
    pub label: String,
}

/// Bir dosyanın metadata bilgisini toplar.
pub fn read_file_metadata(path: &Path, root: &Path) -> Result<FileMetadata> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| DelilXError::io("Metadata okunamadı", Some(path.to_path_buf()), e))?;

    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    let relative_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let mut unsupported = Vec::new();

    let created = match meta.created() {
        Ok(t) => Some(crate::timeutil::format_system_time(&t)),
        Err(_) => {
            unsupported.push("Oluşturulma tarihi (dosya sistemi desteklemiyor)".to_string());
            None
        }
    };
    let modified = match meta.modified() {
        Ok(t) => Some(crate::timeutil::format_system_time(&t)),
        Err(_) => {
            unsupported.push("Son değiştirilme tarihi".to_string());
            None
        }
    };
    let accessed = match meta.accessed() {
        Ok(t) => Some(crate::timeutil::format_system_time(&t)),
        Err(_) => {
            unsupported.push("Son erişim tarihi (dosya sistemi desteklemiyor)".to_string());
            None
        }
    };

    let (readonly, hidden, system, archive) = read_windows_attributes(path);

    Ok(FileMetadata {
        file_name,
        full_path: path.to_string_lossy().to_string(),
        relative_path,
        size: if meta.is_dir() { 0 } else { meta.len() },
        created,
        modified,
        accessed,
        readonly,
        hidden,
        system,
        archive,
        is_dir: meta.is_dir(),
        sha256: None,
        sha512: None,
        unsupported_fields: unsupported,
    })
}

/// Windows dosya özniteliklerini okur. Windows dışında (false,false,false,false).
#[cfg(windows)]
fn read_windows_attributes(path: &Path) -> (bool, bool, bool, bool) {
    use std::os::windows::fs::MetadataExt;
    match fs::symlink_metadata(path) {
        Ok(m) => {
            let attrs = m.file_attributes();
            const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
            const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
            const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
            const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;
            (
                attrs & FILE_ATTRIBUTE_READONLY != 0,
                attrs & FILE_ATTRIBUTE_HIDDEN != 0,
                attrs & FILE_ATTRIBUTE_SYSTEM != 0,
                attrs & FILE_ATTRIBUTE_ARCHIVE != 0,
            )
        }
        Err(_) => (false, false, false, false),
    }
}

/// Windows dışı platformda dosya öznitelikleri (yalnızca salt okunur).
#[cfg(not(windows))]
fn read_windows_attributes(path: &Path) -> (bool, bool, bool, bool) {
    match fs::symlink_metadata(path) {
        Ok(m) => (m.permissions().readonly(), false, false, false),
        Err(_) => (false, false, false, false),
    }
}

/// Dizin ağacını dolaşarak dosya listesini üretir (metadata ile).
pub fn walk_directory(root: &Path) -> Result<Vec<FileMetadata>> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry =
            entry.map_err(|e| DelilXError::other(format!("Dizin dolaşılırken hata: {e}")))?;
        let path = entry.path();
        if path == root {
            continue;
        }
        match read_file_metadata(path, root) {
            Ok(m) => out.push(m),
            Err(e) => {
                // Tek bir dosya okunamazsa işlemi durdurmuyoruz; rapora yazılır.
                eprintln!("Uyarı: {} okunamadı: {e}", path.display());
            }
        }
    }
    Ok(out)
}

/// Dosya sistemini tespit eder (platforma göre).
#[cfg(windows)]
pub fn filesystem_type(path: &Path) -> String {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    let mut wide: Vec<u16> = OsStr::new(path).encode_wide().collect();
    wide.push(0);
    let mut fs_name = [0u16; 64];
    let root = windows_root(&wide);
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetVolumeInformationW(
            root.as_ptr(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            fs_name.as_mut_ptr(),
            fs_name.len() as u32,
        )
    };
    if ok == 0 {
        return "Bilinmiyor".to_string();
    }
    let len = fs_name.iter().position(|&c| c == 0).unwrap_or(0);
    String::from_utf16_lossy(&fs_name[..len])
}

/// Windows sürücü kök yolunu (`C:\`) döndürür.
#[cfg(windows)]
fn windows_root(path: &[u16]) -> Vec<u16> {
    if path.len() >= 3 && path[1] == b':' as u16 {
        vec![path[0], path[1], b'\\' as u16, 0]
    } else {
        path.to_vec()
    }
}

/// Windows dışı platformda dosya sistemi türü.
#[cfg(not(windows))]
pub fn filesystem_type(_path: &Path) -> String {
    "Bilinmiyor (Windows dışı platform)".to_string()
}

/// Dosya sistemi kapasite bilgisi.
#[cfg(windows)]
pub fn filesystem_info(path: &Path) -> Result<FilesystemInfo> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    let mut wide: Vec<u16> = OsStr::new(path).encode_wide().collect();
    wide.push(0);
    let root = windows_root(&wide);
    let mut free_available: u64 = 0;
    let mut total: u64 = 0;
    let mut total_free: u64 = 0;
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            root.as_ptr(),
            &mut free_available,
            &mut total,
            &mut total_free,
        )
    };
    if ok == 0 {
        return Err(DelilXError::other(format!(
            "Disk bilgisi alınamadı: {}",
            path.display()
        )));
    }
    Ok(FilesystemInfo {
        mount_point: path.to_string_lossy().to_string(),
        fs_type: filesystem_type(path),
        total_bytes: total,
        free_bytes: total_free,
        label: String::new(),
    })
}

/// Windows dışı platformda dosya sistemi bilgisi.
#[cfg(not(windows))]
pub fn filesystem_info(path: &Path) -> Result<FilesystemInfo> {
    Err(DelilXError::unsupported(format!(
        "Dosya sistemi bilgisi yalnızca Windows üzerinde desteklenir: {}",
        path.display()
    )))
}

/// Metadata'yı CSV satırına dönüştürür.
pub fn metadata_csv_header() -> Vec<&'static str> {
    vec![
        "Dosya Adı",
        "Göreli Yol",
        "Boyut",
        "Oluşturulma",
        "Değiştirilme",
        "Erişim",
        "Salt Okunur",
        "Gizli",
        "Sistem",
        "Arşiv",
        "Dizin",
        "SHA-256",
        "SHA-512",
        "Korunamayan Alanlar",
    ]
}

/// Metadata'yı CSV satırına dönüştürür.
pub fn metadata_csv_row(m: &FileMetadata) -> Vec<String> {
    vec![
        m.file_name.clone(),
        m.relative_path.clone(),
        m.size.to_string(),
        m.created.clone().unwrap_or_else(|| "-".to_string()),
        m.modified.clone().unwrap_or_else(|| "-".to_string()),
        m.accessed.clone().unwrap_or_else(|| "-".to_string()),
        if m.readonly { "Evet" } else { "Hayır" }.to_string(),
        if m.hidden { "Evet" } else { "Hayır" }.to_string(),
        if m.system { "Evet" } else { "Hayır" }.to_string(),
        if m.archive { "Evet" } else { "Hayır" }.to_string(),
        if m.is_dir { "Evet" } else { "Hayır" }.to_string(),
        m.sha256.clone().unwrap_or_else(|| "-".to_string()),
        m.sha512.clone().unwrap_or_else(|| "-".to_string()),
        if m.unsupported_fields.is_empty() {
            "-".to_string()
        } else {
            m.unsupported_fields.join("; ")
        },
    ]
}

/// Dosya envanteri oluşturur (seçmeli/mantıksal edinim için).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileInventory {
    /// Kök dizin.
    pub root: PathBuf,
    /// Dosya kayıtları.
    pub files: Vec<FileMetadata>,
    /// Toplam dosya sayısı.
    pub file_count: u64,
    /// Toplam dizin sayısı.
    pub dir_count: u64,
    /// Toplam boyut (bayt).
    pub total_bytes: u64,
}

impl FileInventory {
    /// Envanteri oluşturur.
    pub fn build(root: &Path) -> Result<Self> {
        let files = walk_directory(root)?;
        let mut inv = FileInventory {
            root: root.to_path_buf(),
            ..Default::default()
        };
        for f in &files {
            if f.is_dir {
                inv.dir_count += 1;
            } else {
                inv.file_count += 1;
                inv.total_bytes += f.size;
            }
        }
        inv.files = files;
        Ok(inv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_metadata_of_file() {
        let dir = std::env::temp_dir().join("delilx_meta_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("ornek.txt");
        let mut f = fs::File::create(&file).unwrap();
        f.write_all(b"merhaba delilx").unwrap();
        drop(f);

        let m = read_file_metadata(&file, &dir).unwrap();
        assert_eq!(m.file_name, "ornek.txt");
        assert_eq!(m.size, 14);
        assert!(!m.is_dir);
        assert!(m.created.is_some());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn inventory_counts() {
        let dir = std::env::temp_dir().join("delilx_inv_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("alt")).unwrap();
        fs::write(dir.join("a.txt"), b"aaaa").unwrap();
        fs::write(dir.join("alt/b.txt"), b"bbbbbb").unwrap();

        let inv = FileInventory::build(&dir).unwrap();
        assert_eq!(inv.file_count, 2);
        assert_eq!(inv.dir_count, 1);
        assert_eq!(inv.total_bytes, 10);

        let _ = fs::remove_dir_all(&dir);
    }
}
