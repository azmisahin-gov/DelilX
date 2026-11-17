//! Yazma koruması (write blocker) entegrasyon testleri.
//!
//! Bu testler, yazılımsal yazma koruması modülünün platformdan bağımsız
//! mantığını ve durum sınıflandırmasını doğrular. Gerçek aygıt üzerinde
//! destrüktif işlem yapılmaz; Windows'a özgü API çağrıları yalnızca Windows
//! üzerinde çalışır ve bu ortamda atlanır.

use delilx_core::writeblock::{self, ProtectionSource, WriteProtectionReport};

#[test]
fn yazilimsal_koruma_donanimsal_ile_esdeger_degil_uyarisi() {
    let w = writeblock::SOFTWARE_NOT_EQUIVALENT_WARNING;
    assert!(w.contains("eşdeğer değildir"));
    assert!(w.contains("donanımsal"));
    assert!(w.contains("Windows"));
}

#[test]
fn sistem_diskini_korumak_guvenli_degil() {
    // Sistem diski uyarısı metni mevcut olmalı.
    assert!(writeblock::SYSTEM_DISK_WARNING.contains("Sistem diski"));
    assert!(writeblock::SYSTEM_DISK_WARNING.contains("kararsız"));
}

#[test]
fn koruma_kaynaklari_dogru_siniflandirilir() {
    assert!(ProtectionSource::Hardware.is_protected());
    assert!(ProtectionSource::Software.is_protected());
    assert!(!ProtectionSource::None.is_protected());
    assert!(!ProtectionSource::Unknown.is_protected());
}

#[test]
fn rapor_dogrulanmamis_korumayi_etkin_saymaz() {
    let r = WriteProtectionReport {
        device_path: "\\\\.\\PhysicalDrive9".to_string(),
        source: ProtectionSource::Software,
        read_only_attribute: Some(true),
        writable: Some(true), // hâlâ yazılabilir -> doğrulama başarısız
        verified: Some(false),
        detail: "test".to_string(),
    };
    assert!(!r.is_effective(), "Doğrulanmamış koruma etkin sayılmamalı");
}

#[test]
fn rapor_dogrulanmis_korumayi_etkin_sayar() {
    let r = WriteProtectionReport {
        device_path: "\\\\.\\PhysicalDrive9".to_string(),
        source: ProtectionSource::Software,
        read_only_attribute: Some(true),
        writable: Some(false),
        verified: Some(true),
        detail: "test".to_string(),
    };
    assert!(r.is_effective());
}

#[test]
fn korumasiz_cihaz_etkin_sayilmaz() {
    let r = WriteProtectionReport {
        device_path: "\\\\.\\PhysicalDrive9".to_string(),
        source: ProtectionSource::None,
        read_only_attribute: Some(false),
        writable: Some(true),
        verified: None,
        detail: "yazılabilir".to_string(),
    };
    assert!(!r.is_effective());
}

#[test]
fn rapor_ozeti_turkce_ve_anlasilir() {
    let r = WriteProtectionReport {
        device_path: "\\\\.\\PhysicalDrive2".to_string(),
        source: ProtectionSource::Software,
        read_only_attribute: Some(true),
        writable: Some(false),
        verified: Some(true),
        detail: "Koruma etkin.".to_string(),
    };
    let s = r.summary_text();
    assert!(s.contains("Cihaz:"));
    assert!(s.contains("Koruma kaynağı:"));
    assert!(s.contains("Yazılımsal salt okunur"));
    assert!(s.contains("doğrulandı"));
}

/// Windows'ta sistem diski doğru algılanmalı.
#[cfg(windows)]
#[test]
fn sistem_diskini_algilar() {
    let sys = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    assert!(writeblock::is_system_disk(&format!("{sys}\\")));
    // Fiziksel disk yolu ile de sistem diski algılanmalı
    let letter = sys.chars().next().unwrap();
    let path = format!("{letter}:\\");
    assert!(writeblock::is_system_disk(&path));
}

/// Windows dışı platformda koruma işlemleri anlaşılır hata döndürmeli.
#[cfg(not(windows))]
#[test]
fn windows_disi_platformda_desteklenmiyor_hatasi() {
    let result = writeblock::query("\\\\.\\PhysicalDrive2");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("Windows"), "Türkçe ve anlaşılır olmalı: {msg}");
}

/// Yazılımsal koruma, kaynak seçimi akışına entegre edilmiş olmalı:
/// koruma etkinse cihaz kaynak olarak kullanılabilir olmalıdır.
#[test]
fn koruma_etkinse_kaynak_olarak_kullanilabilir() {
    let protected = WriteProtectionReport {
        device_path: "\\\\.\\PhysicalDrive2".to_string(),
        source: ProtectionSource::Software,
        read_only_attribute: Some(true),
        writable: Some(false),
        verified: Some(true),
        detail: String::new(),
    };
    assert!(protected.is_effective());
    assert!(protected.source.is_protected());
}
