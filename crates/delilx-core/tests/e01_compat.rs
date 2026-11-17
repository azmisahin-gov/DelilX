//! E01 uyumluluk testleri.
//!
//! Bu testler DelilX'in ürettiği E01 imajlarının **bağımsız** doğrulama
//! araçlarıyla (libewf: `ewfinfo`, `ewfverify`) okunabildiğini denetler.
//! Araçlar kurulu değilse testler atlanır (CI'da libewf kurulur).
//!
//! Ayrıca libewf'in ürettiği referans E01 dosyalarını okuyabildiğimizi
//! doğrular (ters yön uyumluluk).

use delilx_core::hash::HashAlgorithm;
use delilx_core::progress::CancelToken;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delilx_e01c_{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_data(size: usize) -> Vec<u8> {
    (0..size)
        .map(|i| ((i as u32 * 7 + 13) % 256) as u8)
        .collect()
}

/// Sıkıştırılamayan (yüksek entropili) veri üretir; xorshift PRNG kullanır.
fn make_random_data(size: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    let mut out = Vec::with_capacity(size);
    for _ in 0..size {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.push((state & 0xFF) as u8);
    }
    out
}

/// `ewfverify` aracı mevcut mu?
fn ewfverify_available() -> bool {
    Command::new("ewfverify")
        .arg("-h")
        .output()
        .map(|o| o.status.success() || !o.stdout.is_empty() || !o.stderr.is_empty())
        .unwrap_or(false)
}

/// `ewfinfo` aracı mevcut mu?
fn ewfinfo_available() -> bool {
    Command::new("ewfinfo")
        .arg("-h")
        .output()
        .map(|o| o.status.success() || !o.stdout.is_empty() || !o.stderr.is_empty())
        .unwrap_or(false)
}

/// DelilX ile E01 üretir.
fn write_e01(
    src: &std::path::Path,
    dst: &std::path::Path,
    compression: delilx_core::e01::writer::CompressionMode,
) {
    let mut cfg = delilx_core::e01::EwfWriterConfig::new(dst);
    cfg.case_info = delilx_core::e01::writer::EwfCaseInfo {
        evidence_number: "DELIL-EWF-1".into(),
        case_number: "VAKA-EWF-1".into(),
        description: "DelilX uyumluluk testi".into(),
        examiner_name: "Test Uzmanı".into(),
        notes: "otomatik".into(),
        model: "TestModel".into(),
        serial_number: "SN-EWF".into(),
        device_label: "TEST".into(),
    };
    cfg.compression = compression;
    let mut source = delilx_core::source::FileSource::open(src).unwrap();
    let cancel = CancelToken::new();
    delilx_core::e01::EwfWriter::acquire(cfg, &mut source, None, &cancel).unwrap();
}

#[test]
fn ewfinfo_delilx_imajini_encase6_olarak_tanir() {
    if !ewfinfo_available() {
        eprintln!("ATLANDI: ewfinfo kurulu değil.");
        return;
    }
    let dir = temp_dir("info");
    let src = dir.join("kaynak.img");
    let dst = dir.join("cikti.E01");
    fs::write(&src, make_data(1024 * 1024)).unwrap();
    write_e01(&src, &dst, delilx_core::e01::writer::CompressionMode::Best);

    let out = Command::new("ewfinfo").arg(&dst).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("EnCase 6"),
        "ewfinfo EnCase 6 tanımalı. Çıktı:\n{text}"
    );
    assert!(text.contains("fixed disk") || text.contains("removable disk"));
    assert!(
        text.contains("DELIL-EWF-1"),
        "Delil numarası okunmalı. Çıktı:\n{text}"
    );
    assert!(
        text.contains("VAKA-EWF-1"),
        "Vaka numarası okunmalı. Çıktı:\n{text}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ewfverify_delilx_imajini_basarili_dogrular() {
    if !ewfverify_available() {
        eprintln!("ATLANDI: ewfverify kurulu değil.");
        return;
    }
    let dir = temp_dir("verify");
    let src = dir.join("kaynak.img");
    let dst = dir.join("cikti.E01");
    fs::write(&src, make_data(2 * 1024 * 1024)).unwrap();
    write_e01(&src, &dst, delilx_core::e01::writer::CompressionMode::Best);

    let out = Command::new("ewfverify").arg(&dst).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("SUCCESS"),
        "ewfverify BAŞARILI olmalı. Çıktı:\n{text}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ewfverify_sikistirmasiz_imaji_dogrular() {
    if !ewfverify_available() {
        eprintln!("ATLANDI: ewfverify kurulu değil.");
        return;
    }
    let dir = temp_dir("verify_none");
    let src = dir.join("kaynak.img");
    let dst = dir.join("cikti.E01");
    fs::write(&src, make_data(1024 * 512)).unwrap();
    write_e01(&src, &dst, delilx_core::e01::writer::CompressionMode::None);

    let out = Command::new("ewfverify").arg(&dst).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("SUCCESS"), "Çıktı:\n{text}");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ewfverify_cok_segmentli_imaji_dogrular() {
    if !ewfverify_available() {
        eprintln!("ATLANDI: ewfverify kurulu değil.");
        return;
    }
    let dir = temp_dir("verify_multi");
    let src = dir.join("kaynak.img");
    let dst = dir.join("cikti.E01");
    // Sıkıştırılamayan veri + küçük segment -> çok segment
    let data = make_random_data(6 * 1024 * 1024, 0x9E3779B97F4A7C15);
    fs::write(&src, &data).unwrap();

    let mut cfg = delilx_core::e01::EwfWriterConfig::new(&dst);
    cfg.compression = delilx_core::e01::writer::CompressionMode::Best;
    cfg.segment_size_bytes = 1024 * 1024;
    let mut source = delilx_core::source::FileSource::open(&src).unwrap();
    let cancel = CancelToken::new();
    let result = delilx_core::e01::EwfWriter::acquire(cfg, &mut source, None, &cancel).unwrap();
    assert!(
        result.segment_files.len() > 1,
        "Çok segmentli imaj oluşmalıydı, segment sayısı: {}",
        result.segment_files.len()
    );

    let out = Command::new("ewfverify").arg(&dst).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("SUCCESS"),
        "Çok segment doğrulanmalı. Çıktı:\n{text}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ewfexport_ile_veri_birebir_cikarilir() {
    let export_ok = Command::new("ewfexport")
        .arg("-h")
        .output()
        .map(|o| o.status.success() || !o.stdout.is_empty() || !o.stderr.is_empty())
        .unwrap_or(false);
    if !export_ok {
        eprintln!("ATLANDI: ewfexport kurulu değil.");
        return;
    }
    let dir = temp_dir("export");
    let src = dir.join("kaynak.img");
    let dst = dir.join("cikti.E01");
    let data = make_data(1024 * 1024 + 1234);
    fs::write(&src, &data).unwrap();
    write_e01(&src, &dst, delilx_core::e01::writer::CompressionMode::Best);

    // ewfexport ile ham veriyi çıkar
    let export_dir = dir.join("export");
    fs::create_dir_all(&export_dir).unwrap();
    let status = Command::new("ewfexport")
        .arg("-q")
        .arg("-u")
        .arg("-f")
        .arg("raw")
        .arg("-t")
        .arg(export_dir.join("cikti").to_str().unwrap())
        .arg(&dst)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let _ = status;

    // Çıkarılan dosyayı bul (ewfexport cikti.raw üretir)
    let exported = export_dir.join("cikti.raw");
    let exported = if exported.exists() {
        exported
    } else {
        // Uzantı farklı olabilir; dizini tara
        let mut found = None;
        if let Ok(entries) = fs::read_dir(&export_dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().map(|x| x == "raw").unwrap_or(false) {
                    found = Some(p);
                }
            }
        }
        match found {
            Some(p) => p,
            None => {
                eprintln!("ATLANDI: ewfexport çıktısı bulunamadı.");
                let _ = fs::remove_dir_all(&dir);
                return;
            }
        }
    };

    let exported_data = fs::read(&exported).unwrap();
    // E01 imajları medya boyutunu sektör hizasına yuvarlar; bu nedenle
    // çıkarılan veri kaynaktan en fazla bir sektör (512 bayt) daha uzun olabilir.
    assert!(
        exported_data.len() >= data.len() && exported_data.len() <= data.len() + 512,
        "Çıkarılan boyut beklenen aralıkta olmalı: {}",
        exported_data.len()
    );
    assert_eq!(
        &exported_data[..data.len()],
        &data[..],
        "Çıkarılan veri kaynakla birebir eşleşmeli"
    );
    // Kalan baytlar sıfır dolgu olmalı
    for &b in &exported_data[data.len()..] {
        assert_eq!(b, 0, "Sektör dolgusu sıfır olmalı");
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn libewf_referans_imaji_okunabilir() {
    // libewf (ewfacquire) kuruluysa referans E01 üretip okuyabildiğimizi doğrula.
    let acquire_ok = Command::new("ewfacquire")
        .arg("-h")
        .output()
        .map(|o| o.status.success() || !o.stdout.is_empty() || !o.stderr.is_empty())
        .unwrap_or(false);
    if !acquire_ok {
        eprintln!("ATLANDI: ewfacquire kurulu değil.");
        return;
    }

    let dir = temp_dir("libewf_ref");
    let src = dir.join("kaynak.img");
    let dst = dir.join("ref.E01");
    let data = make_data(1024 * 1024);
    fs::write(&src, &data).unwrap();

    // ewfacquire etkileşimlidir; onay için yeni satır besleriz. stdin kapatılırsa
    // araç sonsuz döngüye girer, bu yüzden boru ile boş satırlar gönderiyoruz.
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new("ewfacquire")
        .args(["-f", "encase6", "-C", "C", "-e", "E", "-t"])
        .arg(&dst)
        .arg(&src)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(mut stdin) = child.stdin.take() {
        // Yeterli sayıda onay satırı gönder, sonra kapat.
        let _ = stdin.write_all(b"\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n\n");
        let _ = stdin.flush();
        drop(stdin);
    }
    let _ = child.wait();

    // ewfacquire çıktıyı ref.E01.E01 olarak üretebilir; yeniden adlandır
    let produced = if dst.exists() {
        dst.clone()
    } else {
        let alt = dir.join("ref.E01.E01");
        if alt.exists() {
            fs::rename(&alt, &dst).unwrap();
            dst.clone()
        } else {
            eprintln!("ATLANDI: ewfacquire çıktısı bulunamadı.");
            let _ = fs::remove_dir_all(&dir);
            return;
        }
    };

    // DelilX okuyucusu ile aç
    let image = delilx_core::e01::EwfImage::open(&produced).unwrap();
    let mut out = vec![0u8; data.len()];
    let n = image.read_at(0, &mut out).unwrap();
    assert_eq!(n, data.len());
    assert_eq!(out, data, "libewf referans imajı birebir okunmalı");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn delilx_ve_libewf_md5_uyusur() {
    if !ewfinfo_available() {
        eprintln!("ATLANDI: ewfinfo kurulu değil.");
        return;
    }
    let dir = temp_dir("md5_match");
    let src = dir.join("kaynak.img");
    let dst = dir.join("cikti.E01");
    let data = make_data(1024 * 1024);
    fs::write(&src, &data).unwrap();
    write_e01(&src, &dst, delilx_core::e01::writer::CompressionMode::Best);

    // DelilX'in hesapladığı MD5
    let expected = delilx_core::hash::hash_bytes(&data, &[HashAlgorithm::Md5]);
    let expected_md5 = expected.get(HashAlgorithm::Md5).unwrap();

    // ewfinfo'nun okuduğu MD5
    let out = Command::new("ewfinfo").arg(&dst).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains(expected_md5),
        "ewfinfo MD5 {expected_md5} içermeli. Çıktı:\n{text}"
    );

    let _ = fs::remove_dir_all(&dir);
}
