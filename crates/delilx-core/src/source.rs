//! Kaynak okuma soyutlaması.
//!
//! Kaynak yalnızca okunabilir. Bu modül hiçbir koşulda kaynağa yazmaz.
//! Windows'ta fiziksel cihazlar `FILE_FLAG_NO_BUFFERING` ve yalnızca
//! `GENERIC_READ` erişimiyle açılır.

use crate::error::{DelilXError, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Okuma hatası kaydı.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReadErrorRecord {
    /// Sektör numarası.
    pub sector: u64,
    /// Sektör boyutu.
    pub bytes_per_sector: u32,
    /// Türkçe hata açıklaması.
    pub message: String,
}

/// Bir kaynaktan sektör bazlı okuma yapan soyutlama.
pub trait SourceReader: Send {
    /// Toplam bayt (biliniyorsa).
    fn total_bytes(&self) -> Option<u64>;
    /// Sektör boyutu.
    fn bytes_per_sector(&self) -> u32;
    /// Belirtilen ofsetten tampona okur, okunan bayt sayısını döndürür.
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize>;
    /// Kaynak açıklaması (Türkçe).
    fn description(&self) -> String;
}

/// Normal bir dosyadan okuyan kaynak (imaj dosyaları, test verileri).
pub struct FileSource {
    file: File,
    path: PathBuf,
    size: u64,
    bytes_per_sector: u32,
}

impl FileSource {
    /// Dosyayı salt okunur olarak açar.
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .map_err(|e| DelilXError::io("Kaynak dosya açılamadı", Some(path.to_path_buf()), e))?;
        let size = file
            .metadata()
            .map_err(|e| DelilXError::io("Dosya boyutu okunamadı", Some(path.to_path_buf()), e))?
            .len();
        Ok(FileSource {
            file,
            path: path.to_path_buf(),
            size,
            bytes_per_sector: 512,
        })
    }

    /// Sektör boyutunu ayarlar.
    pub fn with_sector_size(mut self, bps: u32) -> Self {
        self.bytes_per_sector = bps;
        self
    }
}

impl SourceReader for FileSource {
    fn total_bytes(&self) -> Option<u64> {
        Some(self.size)
    }

    fn bytes_per_sector(&self) -> u32 {
        self.bytes_per_sector
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        if offset >= self.size {
            return Ok(0);
        }
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| DelilXError::io("Kaynak konumlanamadı", Some(self.path.clone()), e))?;
        let remaining = (self.size - offset) as usize;
        let to_read = buf.len().min(remaining);
        let mut total = 0;
        while total < to_read {
            match self.file.read(&mut buf[total..to_read]) {
                Ok(0) => break,
                Ok(n) => total += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    return Err(DelilXError::io(
                        "Kaynak okunamadı",
                        Some(self.path.clone()),
                        e,
                    ))
                }
            }
        }
        Ok(total)
    }

    fn description(&self) -> String {
        format!("Dosya: {}", self.path.display())
    }
}

/// Windows fiziksel cihaz kaynağı (yalnızca okuma).
#[cfg(windows)]
pub struct DeviceSource {
    handle: windows_sys::Win32::Foundation::HANDLE,
    path: String,
    size: u64,
    bytes_per_sector: u32,
}

#[cfg(windows)]
unsafe impl Send for DeviceSource {}

#[cfg(windows)]
impl DeviceSource {
    /// Fiziksel cihazı yalnızca okuma erişimiyle açar.
    ///
    /// Yazma erişimi (`GENERIC_WRITE`) hiçbir zaman istenmez. Tamponsuz
    /// (`FILE_FLAG_NO_BUFFERING`) okuma kullanılır.
    pub fn open(path: &str) -> Result<Self> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::GENERIC_READ;
        use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
        use windows_sys::Win32::Storage::FileSystem::{
            CreateFileW, FILE_FLAG_NO_BUFFERING, FILE_FLAG_SEQUENTIAL_SCAN, FILE_SHARE_READ,
            FILE_SHARE_WRITE, OPEN_EXISTING,
        };

        let wide: Vec<u16> = OsStr::new(path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ, // yalnızca okuma; yazma asla istenmez
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_NO_BUFFERING | FILE_FLAG_SEQUENTIAL_SCAN,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            return Err(DelilXError::io(
                format!("Fiziksel cihaz açılamadı (Windows hata kodu {err})"),
                Some(PathBuf::from(path)),
                std::io::Error::from_raw_os_error(err as i32),
            ));
        }

        let bps = query_sector_size(handle).unwrap_or(512);
        let size = query_device_size(handle).unwrap_or(0);

        Ok(DeviceSource {
            handle,
            path: path.to_string(),
            size,
            bytes_per_sector: bps,
        })
    }
}

#[cfg(windows)]
fn query_sector_size(handle: windows_sys::Win32::Foundation::HANDLE) -> Option<u32> {
    let mut info: windows_sys::Win32::System::Ioctl::DISK_GEOMETRY_EX =
        unsafe { std::mem::zeroed() };
    let mut returned: u32 = 0;
    let ok = unsafe {
        windows_sys::Win32::System::IO::DeviceIoControl(
            handle,
            windows_sys::Win32::System::Ioctl::IOCTL_DISK_GET_DRIVE_GEOMETRY_EX,
            std::ptr::null_mut(),
            0,
            &mut info as *mut _ as *mut _,
            std::mem::size_of::<windows_sys::Win32::System::Ioctl::DISK_GEOMETRY_EX>() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        None
    } else {
        Some(info.Geometry.BytesPerSector)
    }
}

#[cfg(windows)]
fn query_device_size(handle: windows_sys::Win32::Foundation::HANDLE) -> Option<u64> {
    let mut length: windows_sys::Win32::System::Ioctl::GET_LENGTH_INFORMATION =
        unsafe { std::mem::zeroed() };
    let mut returned: u32 = 0;
    let ok = unsafe {
        windows_sys::Win32::System::IO::DeviceIoControl(
            handle,
            windows_sys::Win32::System::Ioctl::IOCTL_DISK_GET_LENGTH_INFO,
            std::ptr::null_mut(),
            0,
            &mut length as *mut _ as *mut _,
            std::mem::size_of::<windows_sys::Win32::System::Ioctl::GET_LENGTH_INFORMATION>() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        None
    } else {
        Some(length.Length as u64)
    }
}

#[cfg(windows)]
impl SourceReader for DeviceSource {
    fn total_bytes(&self) -> Option<u64> {
        if self.size == 0 {
            None
        } else {
            Some(self.size)
        }
    }

    fn bytes_per_sector(&self) -> u32 {
        self.bytes_per_sector
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        // Tamponsuz G/Ç için tampon sektör hizasında ve sektör katı olmalı.
        let bps = self.bytes_per_sector as usize;
        if buf.len() % bps != 0 || offset as usize % bps != 0 {
            return Err(DelilXError::other(
                "Tamponsuz okuma için tampon ve ofset sektör hizasında olmalıdır.",
            ));
        }
        let mut overlapped: windows_sys::Win32::System::IO::OVERLAPPED =
            unsafe { std::mem::zeroed() };
        overlapped.Anonymous.Anonymous.Offset = (offset & 0xFFFF_FFFF) as u32;
        overlapped.Anonymous.Anonymous.OffsetHigh = (offset >> 32) as u32;

        let mut read_count: u32 = 0;
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::ReadFile(
                self.handle,
                buf.as_mut_ptr() as *mut _,
                buf.len() as u32,
                &mut read_count,
                &mut overlapped,
            )
        };
        if ok == 0 {
            let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
            // ERROR_HANDLE_EOF = 38 -> dosya sonu
            if err == 38 {
                return Ok(0);
            }
            return Err(DelilXError::io(
                format!("Fiziksel cihaz okunamadı (Windows hata kodu {err})"),
                Some(PathBuf::from(&self.path)),
                std::io::Error::from_raw_os_error(err as i32),
            ));
        }
        Ok(read_count as usize)
    }

    fn description(&self) -> String {
        format!(
            "Fiziksel Cihaz: {} ({} bayt/sektör)",
            self.path, self.bytes_per_sector
        )
    }
}

#[cfg(windows)]
impl Drop for DeviceSource {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}

/// Kaynak yolu verilen bir `SourceReader` üretir.
///
/// Windows'ta `\\.\PhysicalDriveN` biçimindeki yollar fiziksel cihaz olarak,
/// diğerleri dosya olarak açılır.
pub fn open_source(path: &Path) -> Result<Box<dyn SourceReader>> {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if s.starts_with("\\\\.\\") || s.starts_with("//./") {
            return Ok(Box::new(DeviceSource::open(&s)?));
        }
    }
    Ok(Box::new(FileSource::open(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn file_source_reads_correctly() {
        let path = std::env::temp_dir().join("delilx_source_test.bin");
        let mut f = File::create(&path).unwrap();
        let data: Vec<u8> = (0..2048u32).map(|i| (i % 256) as u8).collect();
        f.write_all(&data).unwrap();
        drop(f);

        let mut src = FileSource::open(&path).unwrap();
        assert_eq!(src.total_bytes(), Some(2048));
        let mut buf = vec![0u8; 512];
        let n = src.read_at(0, &mut buf).unwrap();
        assert_eq!(n, 512);
        assert_eq!(buf[0], 0);
        assert_eq!(buf[255], 255);
        let n = src.read_at(1536, &mut buf).unwrap();
        assert_eq!(n, 512);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn file_source_reads_past_end() {
        let path = std::env::temp_dir().join("delilx_source_eof.bin");
        std::fs::write(&path, b"kisa").unwrap();
        let mut src = FileSource::open(&path).unwrap();
        let mut buf = vec![0u8; 512];
        let n = src.read_at(0, &mut buf).unwrap();
        assert_eq!(n, 4);
        let n = src.read_at(1000, &mut buf).unwrap();
        assert_eq!(n, 0);
        let _ = std::fs::remove_file(&path);
    }
}
