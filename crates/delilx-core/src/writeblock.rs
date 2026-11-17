//! Yazılımsal yazma koruması (write blocker) modülü.
//!
//! Bu modül, harici bir donanımsal yazma engelleyici kullanmadan, **yalnızca
//! yazılım üzerinden** bir USB belleği veya SD kartı yazmaya karşı korumayı
//! sağlar. Windows'un disk salt-okunur özniteliği
//! (`IOCTL_DISK_SET_DISK_ATTRIBUTES` + `DISK_ATTRIBUTE_READ_ONLY`) kullanılır.
//!
//! ## Önemli güvenlik notları
//!
//! - Bu koruma **yazılımsaldır** ve donanımsal yazma engelleyiciyle **eşdeğer
//!   değildir**. Donanımsal engelleyici, işletim sisteminden bağımsız olarak
//!   çalışır; yazılımsal koruma ise Windows çekirdeği ve sürücü yığınına
//!   dayanır.
//! - Koruma, cihaz çıkarılana veya sistem yeniden başlatılana kadar etkindir
//!   (`Persist = FALSE`). Bu, delil cihazının kalıcı olarak değiştirilmemesi
//!   için bilinçli bir tercihtir.
//! - Koruma **etkinleştirildikten sonra doğrulanır**: öznitelik yeniden
//!   okunur ve cihaz yazma erişimiyle açılamıyorsa koruma "doğrulandı" sayılır.
//! - Koruma sağlanamazsa kullanıcı açıkça bilgilendirilir.

use crate::error::{DelilXError, Result};
use serde::{Deserialize, Serialize};

/// Yazma korumasının kaynağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectionSource {
    /// Donanımsal yazma engelleyici veya salt okunur anahtar (fiziksel cihaz
    /// yazmaya kapalı olarak bildiriyor).
    Hardware,
    /// DelilX tarafından etkinleştirilen yazılımsal salt-okunur özniteliği.
    Software,
    /// Yazma koruması yok; cihaz yazılabilir.
    None,
    /// Durum belirlenemedi.
    Unknown,
}

impl ProtectionSource {
    /// Türkçe etiket.
    pub fn label(&self) -> &'static str {
        match self {
            ProtectionSource::Hardware => "Donanımsal yazma engelleyici",
            ProtectionSource::Software => "Yazılımsal salt okunur (DelilX)",
            ProtectionSource::None => "Yok (yazılabilir)",
            ProtectionSource::Unknown => "Doğrulanamadı",
        }
    }

    /// Cihaz kaynak delil olarak kullanılabilir mi? (Koruma var mı?)
    pub fn is_protected(&self) -> bool {
        matches!(
            self,
            ProtectionSource::Hardware | ProtectionSource::Software
        )
    }
}

/// Yazma koruması durum raporu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteProtectionReport {
    /// Sorgulanan cihaz yolu.
    pub device_path: String,
    /// Korumasının kaynağı.
    pub source: ProtectionSource,
    /// Disk salt-okunur özniteliği (`IOCTL_DISK_GET_DISK_ATTRIBUTES`).
    /// `None` = sorgulanamadı.
    pub read_only_attribute: Option<bool>,
    /// Cihaz yazılabilir mi? (`IOCTL_DISK_IS_WRITABLE`). `None` = sorgulanamadı.
    pub writable: Option<bool>,
    /// Koruma etkinleştirildikten sonra doğrulandı mı? `None` = doğrulama yapılmadı.
    pub verified: Option<bool>,
    /// Türkçe ayrıntı açıklaması.
    pub detail: String,
}

impl WriteProtectionReport {
    /// Koruma etkin ve doğrulanmış mı?
    pub fn is_effective(&self) -> bool {
        self.source.is_protected() && self.verified.unwrap_or(self.source.is_protected())
    }

    /// Türkçe özet metni.
    pub fn summary_text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("Cihaz: {}\n", self.device_path));
        s.push_str(&format!("Koruma kaynağı: {}\n", self.source.label()));
        s.push_str(&format!(
            "Salt-okunur özniteliği: {}\n",
            match self.read_only_attribute {
                Some(true) => "Etkin",
                Some(false) => "Kapalı",
                None => "Sorgulanamadı",
            }
        ));
        s.push_str(&format!(
            "Yazılabilir mi: {}\n",
            match self.writable {
                Some(true) => "Evet (yazılabilir)",
                Some(false) => "Hayır (yazmaya kapalı)",
                None => "Sorgulanamadı",
            }
        ));
        if let Some(v) = self.verified {
            s.push_str(&format!(
                "Doğrulama: {}\n",
                if v {
                    "Koruma etkin ve doğrulandı"
                } else {
                    "KORUMA DOĞRULANAMADI"
                }
            ));
        }
        s.push_str(&format!("\n{}\n", self.detail));
        s
    }
}

/// Yazılımsal korumanın donanımsal ile eşdeğer olmadığını belirten Türkçe uyarı.
pub const SOFTWARE_NOT_EQUIVALENT_WARNING: &str =
    "Uyarı: Yazılımsal salt okunur koruma, donanımsal yazma engelleyiciyle eşdeğer değildir. \
     Donanımsal engelleyici işletim sisteminden bağımsız çalışır; yazılımsal koruma ise Windows \
     çekirdeği ve sürücü yığınına dayanır. Yüksek güvenilirlik gerektiren adli işlemlerde \
     donanımsal yazma engelleyici kullanılmalıdır.";

/// Cihazın sistem diski olup olmadığını belirten uyarı.
pub const SYSTEM_DISK_WARNING: &str =
    "Uyarı: Sistem diski veya önyükleme birimi yazmaya kapatılırsa işletim sistemi kararsız \
     hale gelebilir. DelilX, sistem diskinde yazılımsal koruma etkinleştirmeyi önermez ve \
     reddeder.";

/// Bir cihaz yolunda yazılımsal koruma etkinleştirmenin güvenli olup olmadığını denetler.
///
/// Sistem diski (`%SystemDrive%`) korunamaz.
pub fn ensure_safe_target(device_path: &str) -> Result<()> {
    if is_system_disk(device_path) {
        return Err(DelilXError::other(SYSTEM_DISK_WARNING));
    }
    Ok(())
}

/// Verilen yolun sistem diski olup olmadığını belirler.
#[cfg(windows)]
pub fn is_system_disk(device_path: &str) -> bool {
    // Sistem sürücüsü harfini al (ör. "C:")
    let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    let sys_letter = system_drive.chars().next().map(|c| c.to_ascii_uppercase());

    // Yol bir sürücü harfi içeriyorsa karşılaştır
    if let Some(letter) = device_path
        .chars()
        .find(|c| c.is_ascii_alphabetic() && device_path.contains(':'))
    {
        if Some(letter.to_ascii_uppercase()) == sys_letter {
            return true;
        }
    }

    // Fiziksel disk yolu ise sistem diskinin fiziksel numarasıyla karşılaştır
    if let Some(num) = physical_drive_number_from_path(device_path) {
        if let Some(sys_num) = system_physical_drive_number() {
            return num == sys_num;
        }
    }
    false
}

/// Windows dışı platformda sistem diski kontrolü (her zaman güvenli kabul edilir).
#[cfg(not(windows))]
pub fn is_system_disk(_device_path: &str) -> bool {
    false
}

/// `\\.\PhysicalDriveN` biçimindeki yoldan disk numarasını çıkarır.
#[cfg(windows)]
fn physical_drive_number_from_path(path: &str) -> Option<u32> {
    let lower = path.to_ascii_lowercase();
    if let Some(idx) = lower.find("physicaldrive") {
        let rest = &path[idx + "physicaldrive".len()..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        return digits.parse().ok();
    }
    None
}

/// Sistem diskini işgal eden fiziksel sürücü numarasını bulur.
#[cfg(windows)]
fn system_physical_drive_number() -> Option<u32> {
    let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    let letter = system_drive.chars().next()?;
    crate::device::device_for_path(std::path::Path::new(&format!("{letter}:\\")))
}

/// Bir cihazın yazma koruması durumunu sorgular.
#[cfg(windows)]
pub fn query(device_path: &str) -> Result<WriteProtectionReport> {
    win::query(device_path)
}

/// Windows dışı platformda sorgu.
#[cfg(not(windows))]
pub fn query(_device_path: &str) -> Result<WriteProtectionReport> {
    Err(DelilXError::unsupported(
        "Yazma koruması sorgulama yalnızca Windows üzerinde desteklenir.",
    ))
}

/// Yazılımsal salt-okunur korumayı etkinleştirir veya kaldırır.
///
/// `enable = true` korumayı etkinleştirir, `false` kaldırır.
#[cfg(windows)]
pub fn set_read_only(device_path: &str, enable: bool) -> Result<()> {
    win::set_read_only(device_path, enable)
}

/// Windows dışı platformda koruma ayarı.
#[cfg(not(windows))]
pub fn set_read_only(_device_path: &str, _enable: bool) -> Result<()> {
    Err(DelilXError::unsupported(
        "Yazma koruması ayarlama yalnızca Windows üzerinde desteklenir.",
    ))
}

/// Yazılımsal korumayı etkinleştirir ve gerçekten etkin olduğunu doğrular.
///
/// Doğrulama adımları:
/// 1. Öznitelik etkinleştirilir.
/// 2. Salt-okunur özniteliği yeniden okunarak doğrulanır.
/// 3. Cihaz yazma erişimiyle açılmaya çalışılır; başarısız olması beklenir.
#[cfg(windows)]
pub fn enable_and_verify(device_path: &str) -> Result<WriteProtectionReport> {
    ensure_safe_target(device_path)?;
    win::set_read_only(device_path, true)?;

    // Doğrulama: yeni bir sorgu yap ve yazma erişimini dene
    let mut report = win::query(device_path)?;
    let attribute_ok = report.read_only_attribute == Some(true);
    let write_blocked = win::try_open_for_write(device_path).is_err();
    report.verified = Some(attribute_ok && write_blocked);
    if report.verified == Some(false) {
        report.detail = format!(
            "{} Koruma etkinleştirildi ancak doğrulanamadı (öznitelik: {:?}, yazma engeli: {}).",
            report.detail, report.read_only_attribute, write_blocked
        );
    } else {
        report.detail = format!("{} Koruma etkinleştirildi ve doğrulandı.", report.detail);
    }
    Ok(report)
}

/// Windows dışı platformda etkinleştir-ve-doğrula.
#[cfg(not(windows))]
pub fn enable_and_verify(_device_path: &str) -> Result<WriteProtectionReport> {
    Err(DelilXError::unsupported(
        "Yazma koruması yalnızca Windows üzerinde desteklenir.",
    ))
}

/// Yazılımsal korumayı kaldırır ve durumu doğrular.
#[cfg(windows)]
pub fn disable_and_verify(device_path: &str) -> Result<WriteProtectionReport> {
    ensure_safe_target(device_path)?;
    win::set_read_only(device_path, false)?;
    let mut report = win::query(device_path)?;
    report.verified = Some(report.read_only_attribute == Some(false));
    report.detail = format!("{} Koruma kaldırıldı.", report.detail);
    Ok(report)
}

/// Windows dışı platformda koruma kaldırma.
#[cfg(not(windows))]
pub fn disable_and_verify(_device_path: &str) -> Result<WriteProtectionReport> {
    Err(DelilXError::unsupported(
        "Yazma koruması yalnızca Windows üzerinde desteklenir.",
    ))
}

/// Windows API uygulaması.
#[cfg(windows)]
mod win {
    use super::*;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Ioctl::{
        DISK_ATTRIBUTE_READ_ONLY, GET_DISK_ATTRIBUTES, IOCTL_DISK_GET_DISK_ATTRIBUTES,
        IOCTL_DISK_IS_WRITABLE, IOCTL_DISK_SET_DISK_ATTRIBUTES, SET_DISK_ATTRIBUTES,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    /// Cihazı istenen erişim haklarıyla açar.
    fn open_device(path: &str, desired_access: u32) -> std::result::Result<HANDLE, u32> {
        let wide: Vec<u16> = OsStr::new(path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                desired_access,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            Err(unsafe { windows_sys::Win32::Foundation::GetLastError() })
        } else {
            Ok(handle)
        }
    }

    /// Disk özniteliklerini okur (`IOCTL_DISK_GET_DISK_ATTRIBUTES`).
    fn get_disk_attributes(handle: HANDLE) -> std::result::Result<u64, u32> {
        let mut attrs = GET_DISK_ATTRIBUTES {
            Version: std::mem::size_of::<GET_DISK_ATTRIBUTES>() as u32,
            Reserved1: 0,
            Attributes: 0,
        };
        let mut returned: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_DISK_GET_DISK_ATTRIBUTES,
                std::ptr::null_mut(),
                0,
                &mut attrs as *mut _ as *mut _,
                std::mem::size_of::<GET_DISK_ATTRIBUTES>() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(unsafe { windows_sys::Win32::Foundation::GetLastError() })
        } else {
            Ok(attrs.Attributes)
        }
    }

    /// Salt-okunur özniteliğini ayarlar (`IOCTL_DISK_SET_DISK_ATTRIBUTES`).
    fn set_disk_read_only(handle: HANDLE, read_only: bool) -> std::result::Result<(), u32> {
        // Persist = FALSE: ayar yalnızca çalışma zamanında geçerlidir; cihaz
        // çıkarılana veya sistem yeniden başlatılana kadar etkindir. Bu, delil
        // cihazını kalıcı olarak değiştirmemek için bilinçli bir tercihtir.
        let mut attrs = SET_DISK_ATTRIBUTES {
            Version: std::mem::size_of::<SET_DISK_ATTRIBUTES>() as u32,
            Persist: 0,
            Reserved1: [0u8; 3],
            Attributes: if read_only {
                DISK_ATTRIBUTE_READ_ONLY
            } else {
                0
            },
            AttributesMask: DISK_ATTRIBUTE_READ_ONLY,
            Reserved2: [0u32; 4],
        };
        let mut returned: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_DISK_SET_DISK_ATTRIBUTES,
                &mut attrs as *mut _ as *mut _,
                std::mem::size_of::<SET_DISK_ATTRIBUTES>() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(unsafe { windows_sys::Win32::Foundation::GetLastError() })
        } else {
            Ok(())
        }
    }

    /// Cihaz yazılabilir mi? (`IOCTL_DISK_IS_WRITABLE`).
    fn is_writable(handle: HANDLE) -> Option<bool> {
        let mut returned: u32 = 0;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_DISK_IS_WRITABLE,
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        Some(ok != 0)
    }

    /// Cihazı yazma erişimiyle açmayı dener (doğrulama için).
    pub fn try_open_for_write(path: &str) -> std::result::Result<(), u32> {
        match open_device(path, GENERIC_READ | GENERIC_WRITE) {
            Ok(h) => {
                unsafe { CloseHandle(h) };
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Durum sorgular ve sınıflandırır.
    pub fn query(device_path: &str) -> Result<WriteProtectionReport> {
        // Sorgu için okuma erişimi yeterlidir.
        let handle = match open_device(device_path, GENERIC_READ) {
            Ok(h) => h,
            Err(e) => {
                return Ok(WriteProtectionReport {
                    device_path: device_path.to_string(),
                    source: ProtectionSource::Unknown,
                    read_only_attribute: None,
                    writable: None,
                    verified: None,
                    detail: format!(
                        "Cihaz sorgulanamadı (Windows hata kodu {e}). Yönetici hakları \
                         gerekebilir."
                    ),
                });
            }
        };

        let attrs = get_disk_attributes(handle);
        let writable = is_writable(handle);
        unsafe { CloseHandle(handle) };

        let read_only_attribute = attrs.ok().map(|a| a & DISK_ATTRIBUTE_READ_ONLY != 0);

        let (source, detail) = match (read_only_attribute, writable) {
            (Some(true), _) => (
                ProtectionSource::Software,
                "Windows disk salt-okunur özniteliği etkin. Bu koruma yazılımsaldır; \
                 donanımsal yazma engelleyiciyle eşdeğer değildir."
                    .to_string(),
            ),
            (Some(false), Some(false)) => (
                ProtectionSource::Hardware,
                "Salt-okunur özniteliği kapalı olmasına rağmen cihaz yazmaya kapalı \
                 bildiriyor. Bu genellikle donanımsal yazma engelleyici veya salt okunur \
                 anahtar anlamına gelir."
                    .to_string(),
            ),
            (Some(false), Some(true)) => (
                ProtectionSource::None,
                "Cihaz yazılabilir. Kaynak delil olarak kullanmadan önce yazma koruması \
                 etkinleştirin veya donanımsal yazma engelleyici bağlayın."
                    .to_string(),
            ),
            _ => (
                ProtectionSource::Unknown,
                "Yazma koruması durumu belirlenemedi.".to_string(),
            ),
        };

        Ok(WriteProtectionReport {
            device_path: device_path.to_string(),
            source,
            read_only_attribute,
            writable,
            verified: None,
            detail,
        })
    }

    /// Salt-okunur özniteliğini ayarlar.
    pub fn set_read_only(device_path: &str, enable: bool) -> Result<()> {
        // Ayar için yazma erişimi gerekir.
        let handle = open_device(device_path, GENERIC_READ | GENERIC_WRITE).map_err(|e| {
            DelilXError::other(format!(
                "Cihaz yazma erişimiyle açılamadı (Windows hata kodu {e}). Yazma koruması \
                 ayarlamak için yönetici hakları gerekir ve cihaz kilitli olmamalıdır."
            ))
        })?;
        let result = set_disk_read_only(handle, enable);
        unsafe { CloseHandle(handle) };
        result.map_err(|e| {
            DelilXError::other(format!(
                "Yazma koruması {} (Windows hata kodu {e}). Bu cihaz veya sürücü \
                 salt-okunur özniteliğini desteklemiyor olabilir.",
                if enable {
                    "etkinleştirilemedi"
                } else {
                    "kaldırılamadı"
                }
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protection_source_labels() {
        assert_eq!(
            ProtectionSource::Hardware.label(),
            "Donanımsal yazma engelleyici"
        );
        assert_eq!(
            ProtectionSource::Software.label(),
            "Yazılımsal salt okunur (DelilX)"
        );
        assert!(ProtectionSource::Hardware.is_protected());
        assert!(ProtectionSource::Software.is_protected());
        assert!(!ProtectionSource::None.is_protected());
        assert!(!ProtectionSource::Unknown.is_protected());
    }

    #[test]
    fn software_warning_mentions_not_equivalent() {
        assert!(SOFTWARE_NOT_EQUIVALENT_WARNING.contains("eşdeğer değildir"));
        assert!(SOFTWARE_NOT_EQUIVALENT_WARNING.contains("donanımsal"));
    }

    #[test]
    fn report_summary_is_turkish() {
        let r = WriteProtectionReport {
            device_path: "\\\\.\\PhysicalDrive2".to_string(),
            source: ProtectionSource::Software,
            read_only_attribute: Some(true),
            writable: Some(false),
            verified: Some(true),
            detail: "Koruma etkin.".to_string(),
        };
        let s = r.summary_text();
        assert!(s.contains("Koruma kaynağı"));
        assert!(s.contains("Yazılımsal salt okunur"));
        assert!(s.contains("Doğrulama"));
        assert!(r.is_effective());
    }

    #[test]
    fn report_not_effective_when_verification_failed() {
        let r = WriteProtectionReport {
            device_path: "x".to_string(),
            source: ProtectionSource::Software,
            read_only_attribute: Some(true),
            writable: Some(true),
            verified: Some(false),
            detail: String::new(),
        };
        assert!(!r.is_effective());
    }

    #[test]
    fn system_disk_warning_present() {
        assert!(SYSTEM_DISK_WARNING.contains("Sistem diski"));
    }

    #[cfg(windows)]
    #[test]
    fn system_drive_is_detected() {
        let sys = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
        assert!(is_system_disk(&format!("{sys}\\")));
    }
}
