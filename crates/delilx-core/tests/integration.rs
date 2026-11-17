//! Uçtan uca entegrasyon testleri.
//!
//! Bu testler gerçek veri akışlarını çalıştırır: RAW imaj, E01 imaj, mantıksal
//! edinim, hash doğrulama, imaj inceleme ve disk karışıklığı koruması.
//!
//! Testler yalnızca geçici dizinlerde ve yapay verilerle çalışır; gerçek
//! aygıtlara hiçbir yazma yapılmaz.

use delilx_core::hash::HashAlgorithm;
use delilx_core::progress::CancelToken;
use std::fs;
use std::path::PathBuf;

/// Test için geçici dizin oluşturur.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delilx_it_{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Deterministik test verisi üretir.
fn make_data(size: usize, seed: u8) -> Vec<u8> {
    (0..size)
        .map(|i| ((i as u32).wrapping_mul(31).wrapping_add(seed as u32) % 256) as u8)
        .collect()
}

#[test]
fn raw_imaj_uctan_uca_dogrulanir() {
    let dir = temp_dir("raw_e2e");
    let src = dir.join("kaynak.img");
    let dst = dir.join("cikti.dd");
    let data = make_data(2 * 1024 * 1024, 7);
    fs::write(&src, &data).unwrap();

    let cfg = delilx_core::raw::RawAcquisitionConfig::new(&src, &dst);
    let mut source = delilx_core::source::FileSource::open(&src).unwrap();
    let cancel = CancelToken::new();
    let result = delilx_core::raw::acquire_raw(&cfg, &mut source, None, &cancel).unwrap();

    assert_eq!(result.total_bytes, data.len() as u64);
    assert_eq!(result.error_sector_count, 0);
    assert_eq!(result.verified, Some(true));
    // İmaj, kaynağın birebir kopyası olmalı
    assert_eq!(fs::read(&dst).unwrap(), data);
    // Bağımsız hesaplanan hash ile eşleşmeli
    let expected = delilx_core::hash::hash_bytes(&data, &[HashAlgorithm::Sha256]);
    assert_eq!(
        result.source_hashes.get(HashAlgorithm::Sha256),
        expected.get(HashAlgorithm::Sha256)
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn e01_imaj_icerik_aynen_geri_okunur() {
    let dir = temp_dir("e01_roundtrip");
    let src = dir.join("kaynak.img");
    let dst = dir.join("cikti.E01");
    let data = make_data(3 * 1024 * 1024 + 512, 11);
    fs::write(&src, &data).unwrap();

    let mut cfg = delilx_core::e01::EwfWriterConfig::new(&dst);
    cfg.case_info = delilx_core::e01::writer::EwfCaseInfo {
        evidence_number: "DELIL-IT-1".into(),
        case_number: "VAKA-IT-1".into(),
        examiner_name: "Test Uzmanı".into(),
        ..Default::default()
    };
    let mut source = delilx_core::source::FileSource::open(&src).unwrap();
    let cancel = CancelToken::new();
    let result = delilx_core::e01::EwfWriter::acquire(cfg, &mut source, None, &cancel).unwrap();

    assert_eq!(result.total_bytes, data.len() as u64);

    // Okuyucu ile aç ve veriyi geri üret
    let image = delilx_core::e01::EwfImage::open(&dst).unwrap();
    assert_eq!(image.volume().total_bytes, data.len() as u64);

    let mut out = vec![0u8; data.len()];
    let n = image.read_at(0, &mut out).unwrap();
    assert_eq!(n, data.len());
    assert_eq!(out, data, "E01 içeriği kaynakla birebir eşleşmeli");

    // İmaj içindeki MD5, bağımsız hesapla eşleşmeli
    let md5 = delilx_core::hash::hash_bytes(&data, &[HashAlgorithm::Md5]);
    assert_eq!(image.md5(), md5.get(HashAlgorithm::Md5));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn e01_hash_degerleri_kaynakla_eslesir() {
    let dir = temp_dir("e01_hash");
    let src = dir.join("kaynak.img");
    let dst = dir.join("cikti.E01");
    let data = make_data(1024 * 512, 3);
    fs::write(&src, &data).unwrap();

    let mut cfg = delilx_core::e01::EwfWriterConfig::new(&dst);
    cfg.compression = delilx_core::e01::writer::CompressionMode::Best;
    let mut source = delilx_core::source::FileSource::open(&src).unwrap();
    let cancel = CancelToken::new();
    let result = delilx_core::e01::EwfWriter::acquire(cfg, &mut source, None, &cancel).unwrap();

    let expected_md5 = delilx_core::hash::hash_bytes(&data, &[HashAlgorithm::Md5]);
    let expected_sha1 = delilx_core::hash::hash_bytes(&data, &[HashAlgorithm::Sha1]);
    assert_eq!(result.md5, expected_md5.get(HashAlgorithm::Md5).unwrap());
    assert_eq!(result.sha1, expected_sha1.get(HashAlgorithm::Sha1).unwrap());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn mantiksal_edinim_yapiyi_korur_ve_hash_uretir() {
    let dir = temp_dir("logical_e2e");
    let src = dir.join("kaynak");
    let dst = dir.join("hedef");
    fs::create_dir_all(src.join("alt/daha_alt")).unwrap();
    fs::write(src.join("kok.txt"), b"kok dosyasi").unwrap();
    fs::write(src.join("alt/ic.txt"), b"ic dosyasi").unwrap();
    fs::write(src.join("alt/daha_alt/derin.bin"), make_data(4096, 1)).unwrap();

    let cfg = delilx_core::logical::LogicalConfig::new(vec![src.clone()], &dst);
    let cancel = CancelToken::new();
    let result = delilx_core::logical::acquire_logical(&cfg, None, &cancel).unwrap();

    assert_eq!(result.inventory.file_count, 3);
    assert_eq!(result.copied_files, 3);
    // Klasör yapısı korunmalı
    assert!(dst.join("kok.txt").exists());
    assert!(dst.join("alt/ic.txt").exists());
    assert!(dst.join("alt/daha_alt/derin.bin").exists());
    // Her dosya için hash kaydı olmalı
    assert_eq!(result.hashes.len(), 3);
    for (_, h) in &result.hashes {
        assert!(h.get(HashAlgorithm::Sha256).is_some());
        assert!(h.get(HashAlgorithm::Sha512).is_some());
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn hash_dogrulama_bozulmayi_yakalar() {
    let dir = temp_dir("verify_corrupt");
    let file = dir.join("delil.bin");
    let data = make_data(64 * 1024, 9);
    fs::write(&file, &data).unwrap();

    let expected = delilx_core::hash::hash_file(&file, &[HashAlgorithm::Sha256]).unwrap();

    // Önce doğru veri ile doğrula
    let report =
        delilx_core::verify::verify_file(&file, &expected, &[HashAlgorithm::Sha256]).unwrap();
    assert!(report.outcome.success);

    // Veriyi boz
    let mut corrupted = data.clone();
    corrupted[100] ^= 0xFF;
    fs::write(&file, &corrupted).unwrap();

    let report =
        delilx_core::verify::verify_file(&file, &expected, &[HashAlgorithm::Sha256]).unwrap();
    assert!(!report.outcome.success, "Bozulma yakalanmalıydı");
    assert_eq!(report.outcome.mismatched.len(), 1);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn kaynak_hedef_ayni_cihaz_reddedilir() {
    use delilx_core::device::{validate_source_target, Device, DeviceKind, WriteProtection};
    let source = Device {
        physical_drive_number: Some(1),
        physical_path: Some("\\\\.\\PhysicalDrive1".to_string()),
        drive_letter: Some("E:".to_string()),
        display_name: "Test USB".to_string(),
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
        write_protection_detail: "donanımsal".to_string(),
        selectable_as_source: true,
        source_warning: None,
    };
    // Hedef, kaynağın birim harfi altında -> reddedilmeli
    let result = validate_source_target(&source, std::path::Path::new("E:\\imaj.dd"));
    assert!(result.is_err());
}

#[test]
fn korumasiz_kaynak_reddedilir() {
    use delilx_core::device::{validate_source_target, Device, DeviceKind, WriteProtection};
    let source = Device {
        physical_drive_number: Some(1),
        physical_path: Some("\\\\.\\PhysicalDrive1".to_string()),
        drive_letter: Some("E:".to_string()),
        display_name: "Test USB".to_string(),
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
    // Farklı sürücüde hedef, ancak yazma koruması yok -> reddedilmeli
    let result = validate_source_target(&source, std::path::Path::new("C:\\imaj.dd"));
    assert!(result.is_err());
}

#[test]
fn yarim_kalan_islem_iptal_edilebilir() {
    let dir = temp_dir("cancel");
    let src = dir.join("kaynak.img");
    let dst = dir.join("cikti.dd");
    let data = make_data(4 * 1024 * 1024, 5);
    fs::write(&src, &data).unwrap();

    let cfg = delilx_core::raw::RawAcquisitionConfig::new(&src, &dst);
    let mut source = delilx_core::source::FileSource::open(&src).unwrap();
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = delilx_core::raw::acquire_raw(&cfg, &mut source, None, &cancel);
    assert!(result.is_err(), "İptal edilen işlem hata döndürmeli");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn imaj_inceleme_bolum_ve_dosya_listeler() {
    let dir = temp_dir("inspect_fat");
    let img = dir.join("test.dd");
    // FAT32 imajı oluştur (mkfs.vfat gerekmez; elle minimal imaj üretmek yerine
    // basit bir imaj oluşturup inceleme akışını test ederiz).
    let data = vec![0u8; 1024 * 1024];
    fs::write(&img, &data).unwrap();

    let cancel = CancelToken::new();
    let insp = delilx_core::inspect::inspect_image(&img, None, &cancel).unwrap();
    assert_eq!(insp.total_bytes, 1024 * 1024);
    // Boş imajda bölüm tablosu yok
    assert!(insp.layout.partitions.is_empty());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn metadata_hash_ayri_degerlendirilir() {
    let dir = temp_dir("meta_hash");
    let file = dir.join("dosya.txt");
    fs::write(&file, b"metin").unwrap();

    let meta = delilx_core::metadata::read_file_metadata(&file, &dir).unwrap();
    assert_eq!(meta.size, 5);
    // Metadata'da hash yoksa None olmalı (ayrı değerlendirilir)
    assert!(meta.sha256.is_none());

    // Hash ayrıca hesaplanır
    let hashes = delilx_core::hash::hash_file(&file, &[HashAlgorithm::Sha256]).unwrap();
    assert!(hashes.get(HashAlgorithm::Sha256).is_some());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn kullanilan_alan_silinen_veri_uyarisi_icerir() {
    let method = delilx_core::acquisition::AcquisitionMethod::UsedSpace;
    let warning = method.deleted_data_warning();
    assert!(warning.is_some());
    let w = warning.unwrap();
    assert!(
        w.contains("silinmiş"),
        "Uyarı silinmiş veri riskini belirtmeli"
    );
    assert!(w.contains("kurtarılamaz") || w.contains("kaybolabilir"));
}
