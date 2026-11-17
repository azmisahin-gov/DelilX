use delilx_core::report::EvidenceReport;
fn main() -> anyhow::Result<()> {
    let mut r = EvidenceReport::new();
    r.operation_number = "DLX-20261117-210000".into();
    r.evidence_number = "DELIL-2026-001".into();
    r.case_number = "VAKA-2026-042".into();
    r.examiner = "Şükrü Çağdaş Öztürk".into();
    r.description = "USB bellek tam disk imajı - şüpheli dijital delil".into();
    r.source_device.name = "SanDisk Cruzer (E:) - Fiziksel Disk 2".into();
    r.source_device.kind = "Çıkarılabilir Aygıt (USB/SD)".into();
    r.source_device.capacity = "14.51 GiB".into();
    r.source_device.serial = "4C530001".into();
    r.source_device.filesystem = "FAT32".into();
    r.source_device.write_protection = "Donanımsal yazma engelleyici".into();
    r.acquisition_method = "RAW / DD İmaj".into();
    r.target = "D:\\Deliller\\vaka-042\\usb.dd".into();
    r.started_at = "2026-11-17 21:00:00".into();
    r.finished_at = "2026-11-17 21:14:33".into();
    r.duration = "14 dakika 33 saniye".into();
    r.data_bytes = 15575891968;
    r.data_size = "14.51 GiB".into();
    r.warnings
        .push("Okuma sırasında 3 sektör hatalı okundu ve sıfırlarla dolduruldu.".into());
    r.verification_result = "BAŞARILI - kaynak ve hedef hash değerleri eşleşti".into();
    r.verification_success = Some(true);
    let mut hs = delilx_core::hash::HashSet::new();
    hs.insert(
        delilx_core::hash::HashAlgorithm::Sha256,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
    );
    hs.insert(delilx_core::hash::HashAlgorithm::Sha512, "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e".into());
    r.add_hashes(&hs, Some(true));
    r.write_pdf(std::path::Path::new("/workspace/_work/rapor_ornek.pdf"))?;
    println!("yazıldı");
    Ok(())
}
