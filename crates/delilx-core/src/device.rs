//! Cihaz algılama ve kaynak bellek koruma modülü.
//!
//! Bu modül projenin en önemli güvenlik gereksinimini uygular:
//! - Fiziksel diskleri ve çıkarılabilir aygıtları algılar.
//! - Kaynak ve hedef cihazları ayırt eder.
//! - Kaynak cihaza yazma girişimini her koşulda engeller.
//! - Yazma korumasının donanımsal mı yazılımsal mı olduğunu ayrı belirtir.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Cihaz türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    /// Sabit disk (dahili).
    Fixed,
    /// Çıkarılabilir aygıt (USB bellek, SD kart).
    Removable,
    /// Optik sürücü.
    Optical,
    /// Ağ sürücüsü.
    Network,
    /// Sanal disk (VHD vb.).
    Virtual,
    /// Bilinmiyor.
    Unknown,
}

impl DeviceKind {
    /// Türkçe etiket.
    pub fn label(&self) -> &'static str {
        match self {
            DeviceKind::Fixed => "Sabit Disk",
            DeviceKind::Removable => "Çıkarılabilir Aygıt (USB/SD)",
            DeviceKind::Optical => "Optik Sürücü",
            DeviceKind::Network => "Ağ Sürücüsü",
            DeviceKind::Virtual => "Sanal Disk",
            DeviceKind::Unknown => "Bilinmiyor",
        }
    }
}

/// Yazma koruması türü. Yazılımsal korumayı donanımsal ile eşdeğer göstermiyoruz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteProtection {
    /// Donanımsal yazma engelleyici (write blocker) tespit edildi.
    Hardware,
    /// Yalnızca yazılımsal salt okunur (kilit ile açılmış).
    Software,
    /// Yazma koruması doğrulanamadı.
    Unknown,
    /// Yazma koruması yok (kaynak olarak kullanılmamalı).
    None,
}

impl WriteProtection {
    /// Türkçe etiket.
    pub fn label(&self) -> &'static str {
        match self {
            WriteProtection::Hardware => "Donanımsal yazma engelleyici",
            WriteProtection::Software => "Yazılımsal salt okunur (donanımsal değil)",
            WriteProtection::Unknown => "Doğrulanamadı",
            WriteProtection::None => "Yok",
        }
    }

    /// Kaynak olarak kullanılabilir mi? (Yalnızca donanımsal veya yazılımsal korumalı.)
    pub fn is_protected(&self) -> bool {
        matches!(self, WriteProtection::Hardware | WriteProtection::Software)
    }
}

/// Algılanan bir depolama aygıtı / birim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    /// Fiziksel sürücü numarası (varsa). Ör. `\\.\PhysicalDrive2` -> 2.
    pub physical_drive_number: Option<u32>,
    /// Fiziksel cihaz yolu (`\\.\PhysicalDrive2`).
    pub physical_path: Option<String>,
    /// Birim harfi (ör. `E:`) veya bağlama noktası.
    pub drive_letter: Option<String>,
    /// Görünen ad.
    pub display_name: String,
    /// Cihaz türü.
    pub kind: DeviceKind,
    /// Üretici.
    pub vendor: Option<String>,
    /// Model.
    pub model: Option<String>,
    /// Seri numarası.
    pub serial: Option<String>,
    /// Toplam kapasite (bayt).
    pub capacity_bytes: u64,
    /// Boş alan (bayt).
    pub free_bytes: u64,
    /// Dosya sistemi (ör. NTFS, exFAT).
    pub filesystem: Option<String>,
    /// Birim etiketi.
    pub label: Option<String>,
    /// Sektör boyutu (bayt).
    pub bytes_per_sector: u32,
    /// Yazma koruması durumu.
    pub write_protection: WriteProtection,
    /// Yazma koruması ayrıntısı (Türkçe).
    pub write_protection_detail: String,
    /// Bu aygıt kaynak olarak seçilebilir mi?
    pub selectable_as_source: bool,
    /// Kaynak seçilemiyorsa neden (Türkçe).
    pub source_warning: Option<String>,
}

impl Device {
    /// Kaynak için kullanılacak en iyi yol (fiziksel cihaz öncelikli).
    pub fn source_path(&self) -> Option<PathBuf> {
        self.physical_path
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| self.drive_letter.as_ref().map(PathBuf::from))
    }

    /// İnsan okunur kapasite.
    pub fn capacity_text(&self) -> String {
        crate::progress::format_bytes(self.capacity_bytes)
    }
}

/// Kaynak cihazın güvenliğini doğrular.
///
/// Kaynak ile hedef aynı fiziksel aygıt üzerindeyse işlem reddedilir. Bu,
/// delilin üzerine yazılmasını önleyen en kritik kontroldür.
pub fn validate_source_target(
    source: &Device,
    target_path: &std::path::Path,
) -> crate::error::Result<()> {
    // Hedef yol hangi cihaza ait?
    let target_device = device_for_path(target_path);

    if let (Some(src_dev), Some(tgt_dev)) = (source.physical_drive_number, target_device) {
        if src_dev == tgt_dev {
            return Err(crate::error::DelilXError::SourceEqualsTarget {
                device: source.display_name.clone(),
            });
        }
    }

    // Hedef yol, kaynağın birim harfi altındaysa da yasak.
    if let Some(letter) = &source.drive_letter {
        let letter = letter.trim_end_matches('\\');
        let target_str = target_path.to_string_lossy();
        if target_str
            .to_uppercase()
            .starts_with(&letter.to_uppercase())
        {
            return Err(crate::error::DelilXError::SourceEqualsTarget {
                device: source.display_name.clone(),
            });
        }
    }

    if !source.write_protection.is_protected() {
        return Err(crate::error::DelilXError::WriteProtected {
            device: source.display_name.clone(),
            detail: format!(
                "Kaynak cihazda doğrulanmış yazma koruması yok ({}). \
                 Lütfen donanımsal yazma engelleyici kullanın.",
                source.write_protection.label()
            ),
        });
    }

    Ok(())
}

/// Bir dosya yolunun hangi fiziksel sürücüye ait olduğunu bulur.
#[cfg(windows)]
pub fn device_for_path(path: &std::path::Path) -> Option<u32> {
    // Yolun kök harfinden fiziksel sürücü eşlemesi yapılır.
    if let Some(c) = path.to_string_lossy().chars().next() {
        if c.is_ascii_alphabetic() {
            return physical_drive_for_letter(c);
        }
    }
    None
}

/// Windows dışı platformda yol->cihaz eşlemesi yok.
#[cfg(not(windows))]
pub fn device_for_path(_path: &std::path::Path) -> Option<u32> {
    None
}

/// Windows'ta bir sürücü harfinin fiziksel sürücü numarasını bulur.
#[cfg(windows)]
fn physical_drive_for_letter(letter: char) -> Option<u32> {
    // Sürücü harfini `\\.\X:` biçimine çevirip sorgularız.
    let vol_path = format!("\\\\.\\{}:", letter.to_ascii_uppercase());
    let wide: Vec<u16> = vol_path.encode_utf16().chain(std::iter::once(0)).collect();
    // Bu işlem için IOCTL_STORAGE_GET_DEVICE_NUMBER kullanılır.
    query_device_number(&wide)
}

/// IOCTL_STORAGE_GET_DEVICE_NUMBER sorgusu ile cihaz numarasını alır.
#[cfg(windows)]
fn query_device_number(wide_path: &[u16]) -> Option<u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Ioctl::IOCTL_STORAGE_GET_DEVICE_NUMBER;

    let handle = unsafe {
        CreateFileW(
            wide_path.as_ptr(),
            0, // yalnızca sorgu; yazma erişimi istemiyoruz
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return None;
    }

    let mut sdn: windows_sys::Win32::System::Ioctl::STORAGE_DEVICE_NUMBER =
        unsafe { std::mem::zeroed() };
    let mut returned: u32 = 0;
    let ok = unsafe {
        windows_sys::Win32::System::IO::DeviceIoControl(
            handle,
            IOCTL_STORAGE_GET_DEVICE_NUMBER,
            std::ptr::null_mut(),
            0,
            &mut sdn as *mut _ as *mut _,
            std::mem::size_of::<windows_sys::Win32::System::Ioctl::STORAGE_DEVICE_NUMBER>() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    unsafe {
        CloseHandle(handle);
    }
    if ok == 0 {
        None
    } else {
        Some(sdn.DeviceNumber)
    }
}

/// Sistemdeki bütün depolama aygıtlarını algılar.
#[cfg(windows)]
pub fn detect_devices() -> crate::error::Result<Vec<Device>> {
    windows_impl::detect_devices()
}

/// Windows dışı platformda cihaz algılama.
#[cfg(not(windows))]
pub fn detect_devices() -> crate::error::Result<Vec<Device>> {
    Ok(Vec::new())
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW,
    };
    use windows_sys::Win32::System::WindowsProgramming::{
        DRIVE_CDROM, DRIVE_FIXED, DRIVE_NO_ROOT_DIR, DRIVE_RAMDISK, DRIVE_REMOTE, DRIVE_REMOVABLE,
        DRIVE_UNKNOWN,
    };

    pub fn detect_devices() -> crate::error::Result<Vec<Device>> {
        let mut devices = Vec::new();
        let mask = unsafe { GetLogicalDrives() };
        for i in 0..26u32 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let letter = (b'A' + i as u8) as char;
            let root = format!("{letter}:\\");
            let wide: Vec<u16> = OsStr::new(&root)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let drive_type = unsafe { GetDriveTypeW(wide.as_ptr()) };
            if drive_type == DRIVE_NO_ROOT_DIR || drive_type == DRIVE_UNKNOWN {
                continue;
            }
            if let Some(dev) = build_device(letter, &wide, drive_type) {
                devices.push(dev);
            }
        }
        Ok(devices)
    }

    fn build_device(letter: char, wide: &[u16], drive_type: u32) -> Option<Device> {
        let kind = match drive_type {
            DRIVE_REMOVABLE => DeviceKind::Removable,
            DRIVE_FIXED => DeviceKind::Fixed,
            DRIVE_REMOTE => DeviceKind::Network,
            DRIVE_CDROM => DeviceKind::Optical,
            DRIVE_RAMDISK => DeviceKind::Virtual,
            _ => DeviceKind::Unknown,
        };

        // Kapasite bilgisi
        let mut free_available: u64 = 0;
        let mut total: u64 = 0;
        let mut total_free: u64 = 0;
        let ok = unsafe {
            windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                wide.as_ptr(),
                &mut free_available,
                &mut total,
                &mut total_free,
            )
        };
        if ok == 0 {
            return None;
        }

        // Birim etiketi ve dosya sistemi
        let mut label_buf = [0u16; 261];
        let mut fs_buf = [0u16; 64];
        let mut serial: u32 = 0;
        let vok = unsafe {
            GetVolumeInformationW(
                wide.as_ptr(),
                label_buf.as_mut_ptr(),
                label_buf.len() as u32,
                &mut serial,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                fs_buf.as_mut_ptr(),
                fs_buf.len() as u32,
            )
        };
        let label = if vok != 0 {
            let len = label_buf.iter().position(|&c| c == 0).unwrap_or(0);
            let s = String::from_utf16_lossy(&label_buf[..len]);
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        } else {
            None
        };
        let fs = if vok != 0 {
            let len = fs_buf.iter().position(|&c| c == 0).unwrap_or(0);
            let s = String::from_utf16_lossy(&fs_buf[..len]);
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        } else {
            None
        };

        // Fiziksel sürücü numarası (yazma erişimi istemeden)
        let vol_path = format!("\\\\.\\{letter}:");
        let vol_wide: Vec<u16> = vol_path.encode_utf16().chain(std::iter::once(0)).collect();
        let physical_number = super::query_device_number(&vol_wide);
        let physical_path = physical_number.map(|n| format!("\\\\.\\PhysicalDrive{n}"));

        // Yazma koruması kontrolü
        let (wp, wp_detail) = check_write_protection(&vol_wide, physical_number);

        // Sektör boyutu
        let bytes_per_sector = query_bytes_per_sector(&vol_wide).unwrap_or(512);

        let serial_hex = if serial != 0 {
            Some(format!("{serial:08X}"))
        } else {
            None
        };

        let display_name = match (&label, physical_number) {
            (Some(l), Some(n)) => format!("{l} ({letter}:) — Fiziksel Disk {n}"),
            (None, Some(n)) => format!("Yerel Disk ({letter}:) — Fiziksel Disk {n}"),
            (Some(l), None) => format!("{l} ({letter}:)"),
            (None, None) => format!("Birim ({letter}:)"),
        };

        let mut selectable = true;
        let mut warning = None;
        if kind == DeviceKind::Network {
            selectable = false;
            warning = Some("Ağ sürücüleri kaynak delil olarak kullanılamaz.".to_string());
        } else if kind == DeviceKind::Optical {
            selectable = false;
            warning = Some("Optik sürücüler kaynak delil olarak kullanılamaz.".to_string());
        }
        if !wp.is_protected() && selectable {
            warning = Some(format!(
                "Yazma koruması doğrulanamadı ({}). Kaynak olarak kullanmadan önce \
                 donanımsal yazma engelleyici bağlayın.",
                wp.label()
            ));
        }

        Some(Device {
            physical_drive_number: physical_number,
            physical_path,
            drive_letter: Some(format!("{letter}:")),
            display_name,
            kind,
            vendor: None,
            model: None,
            serial: serial_hex,
            capacity_bytes: total,
            free_bytes: total_free,
            filesystem: fs,
            label,
            bytes_per_sector,
            write_protection: wp,
            write_protection_detail: wp_detail,
            selectable_as_source: selectable,
            source_warning: warning,
        })
    }

    /// Yazma koruması durumunu sorgular.
    ///
    /// Fiziksel disk yolu varsa onu, yoksa birim yolunu kullanır. Gerçek
    /// durum `writeblock::query` ile belirlenir: Windows salt-okunur özniteliği
    /// (yazılımsal) ile fiziksel yazma engeli (donanımsal) ayrı değerlendirilir.
    fn check_write_protection(
        vol_wide: &[u16],
        physical_number: Option<u32>,
    ) -> (WriteProtection, String) {
        // Sorgu için fiziksel disk yolu tercih edilir.
        let target = match physical_number {
            Some(n) => format!("\\\\.\\PhysicalDrive{n}"),
            None => String::from_utf16_lossy(
                &vol_wide[..vol_wide
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(vol_wide.len())],
            ),
        };

        match crate::writeblock::query(&target) {
            Ok(report) => {
                let wp = match report.source {
                    crate::writeblock::ProtectionSource::Hardware => WriteProtection::Hardware,
                    crate::writeblock::ProtectionSource::Software => WriteProtection::Software,
                    crate::writeblock::ProtectionSource::None => WriteProtection::None,
                    crate::writeblock::ProtectionSource::Unknown => WriteProtection::Unknown,
                };
                (wp, report.detail)
            }
            Err(e) => (
                WriteProtection::Unknown,
                format!("Yazma koruması sorgulanamadı: {e}"),
            ),
        }
    }

    /// Sektör boyutunu sorgular.
    fn query_bytes_per_sector(vol_wide: &[u16]) -> Option<u32> {
        use windows_sys::Win32::Storage::FileSystem::{
            CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        };
        let handle = unsafe {
            CreateFileW(
                vol_wide.as_ptr(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return None;
        }
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
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(handle);
        }
        if ok == 0 {
            None
        } else {
            Some(info.Geometry.BytesPerSector)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_protection_labels() {
        assert_eq!(
            WriteProtection::Hardware.label(),
            "Donanımsal yazma engelleyici"
        );
        assert!(WriteProtection::Software.is_protected());
        assert!(!WriteProtection::None.is_protected());
        assert!(!WriteProtection::Unknown.is_protected());
    }

    #[test]
    fn source_target_same_device_rejected() {
        let source = Device {
            physical_drive_number: Some(1),
            physical_path: Some("\\\\.\\PhysicalDrive1".to_string()),
            drive_letter: Some("E:".to_string()),
            display_name: "Test Aygıt".to_string(),
            kind: DeviceKind::Removable,
            vendor: None,
            model: None,
            serial: None,
            capacity_bytes: 1024,
            free_bytes: 0,
            filesystem: Some("FAT32".to_string()),
            label: None,
            bytes_per_sector: 512,
            write_protection: WriteProtection::Hardware,
            write_protection_detail: "test".to_string(),
            selectable_as_source: true,
            source_warning: None,
        };
        // Hedef kaynağın birim harfi altında
        let target = std::path::Path::new("E:\\images\\cikti.dd");
        let result = validate_source_target(&source, target);
        assert!(matches!(
            result,
            Err(crate::error::DelilXError::SourceEqualsTarget { .. })
        ));
    }

    #[test]
    fn source_target_unprotected_rejected() {
        let source = Device {
            physical_drive_number: Some(1),
            physical_path: Some("\\\\.\\PhysicalDrive1".to_string()),
            drive_letter: Some("E:".to_string()),
            display_name: "Test Aygıt".to_string(),
            kind: DeviceKind::Removable,
            vendor: None,
            model: None,
            serial: None,
            capacity_bytes: 1024,
            free_bytes: 0,
            filesystem: Some("FAT32".to_string()),
            label: None,
            bytes_per_sector: 512,
            write_protection: WriteProtection::None,
            write_protection_detail: "yok".to_string(),
            selectable_as_source: true,
            source_warning: None,
        };
        let target = std::path::Path::new("C:\\images\\cikti.dd");
        let result = validate_source_target(&source, target);
        assert!(matches!(
            result,
            Err(crate::error::DelilXError::WriteProtected { .. })
        ));
    }
}
