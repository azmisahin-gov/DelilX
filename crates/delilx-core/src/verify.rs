//! Hash doğrulama modülü.
//!
//! Bir imaj veya dosyanın hash değerlerini yeniden hesaplayarak kayıtlı
//! değerlerle karşılaştırır. Hash değerleri ile metadata bütünlüğü ayrı
//! değerlendirilir.

use crate::error::{DelilXError, Result};
use crate::hash::{hash_file, HashAlgorithm, HashSet};
use crate::progress::{CancelToken, ProgressCallback, ProgressTracker};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;

/// Doğrulama hedefi türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifyTarget {
    /// RAW/DD imaj dosyası.
    RawImage,
    /// E01 imaj (segmentler).
    E01Image,
    /// Tek dosya.
    File,
}

/// Doğrulama sonucu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyReport {
    /// Hedef yolu.
    pub target_path: String,
    /// Hedef türü.
    pub target_type: VerifyTarget,
    /// Kaynak (beklenen) hash değerleri.
    pub expected: HashSet,
    /// Yeniden hesaplanan hash değerleri.
    pub actual: HashSet,
    /// Karşılaştırma sonucu.
    pub outcome: VerificationOutcomeSummary,
    /// Doğrulama başlangıç zamanı.
    pub started_at: String,
    /// Doğrulama bitiş zamanı.
    pub finished_at: String,
    /// Süre (saniye).
    pub duration_seconds: f64,
}

/// Doğrulama sonucu özeti (serileştirilebilir).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationOutcomeSummary {
    /// Eşleşen algoritmalar.
    pub matched: Vec<String>,
    /// Uyuşmayanlar (algoritma, beklenen, hesaplanan).
    pub mismatched: Vec<(String, String, String)>,
    /// Doğrulanamayanlar.
    pub missing: Vec<String>,
    /// Genel sonuç.
    pub success: bool,
}

impl VerifyReport {
    /// Türkçe özet metni.
    pub fn summary_text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("Hedef: {}\n", self.target_path));
        s.push_str(&format!("Doğrulama zamanı: {}\n", self.finished_at));
        for v in &self.actual.values {
            s.push_str(&format!("{}: {}\n", v.algorithm.name(), v.value));
        }
        if self.outcome.success {
            s.push_str("Sonuç: DOĞRULAMA BAŞARILI — bütün hash değerleri eşleşti.\n");
        } else {
            s.push_str("Sonuç: DOĞRULAMA BAŞARISIZ\n");
            for (alg, exp, act) in &self.outcome.mismatched {
                s.push_str(&format!("  {alg}: beklenen {exp} / hesaplanan {act}\n"));
            }
            if !self.outcome.missing.is_empty() {
                s.push_str(&format!(
                    "  Doğrulanamayan algoritmalar: {}\n",
                    self.outcome.missing.join(", ")
                ));
            }
        }
        s
    }
}

/// Bir dosyanın hash değerlerini yeniden hesaplayıp beklenen değerlerle
/// karşılaştırır.
pub fn verify_file(
    path: &Path,
    expected: &HashSet,
    algorithms: &[HashAlgorithm],
) -> Result<VerifyReport> {
    let started_at = crate::timeutil::now_iso8601();
    let start = std::time::Instant::now();
    let actual = hash_file(path, algorithms)?;
    let outcome = summarize(&expected.diff(&actual), expected, &actual);
    Ok(VerifyReport {
        target_path: path.to_string_lossy().to_string(),
        target_type: VerifyTarget::File,
        expected: expected.clone(),
        actual,
        outcome,
        started_at,
        finished_at: crate::timeutil::now_iso8601(),
        duration_seconds: start.elapsed().as_secs_f64(),
    })
}

/// RAW/DD imaj dosyasını doğrular.
pub fn verify_raw_image(
    path: &Path,
    expected: &HashSet,
    algorithms: &[HashAlgorithm],
) -> Result<VerifyReport> {
    let mut report = verify_file(path, expected, algorithms)?;
    report.target_type = VerifyTarget::RawImage;
    Ok(report)
}

/// Bir E01 imajın bütün segmentlerini okuyarak hash doğrulaması yapar.
pub fn verify_e01_image(
    first_segment: &Path,
    expected: &HashSet,
    algorithms: &[HashAlgorithm],
) -> Result<VerifyReport> {
    let started_at = crate::timeutil::now_iso8601();
    let start = std::time::Instant::now();

    // Bütün segmentleri sırayla oku
    let mut hasher = crate::hash::MultiHasher::new(algorithms);
    let mut seg = 1u16;
    let mut any = false;
    loop {
        let path = crate::e01::writer::segment_path(first_segment, seg);
        if !path.exists() {
            break;
        }
        any = true;
        let mut file = std::fs::File::open(&path)
            .map_err(|e| DelilXError::io("Segment açılamadı", Some(path.clone()), e))?;
        let mut buf = vec![0u8; 4 * 1024 * 1024];
        loop {
            let n = file
                .read(&mut buf)
                .map_err(|e| DelilXError::io("Segment okunamadı", Some(path.clone()), e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        seg += 1;
        if seg > 10_000 {
            break;
        }
    }
    if !any {
        return Err(DelilXError::other(format!(
            "E01 segment dosyası bulunamadı: {}",
            first_segment.display()
        )));
    }

    let actual = hasher.finalize();
    let outcome = summarize(&expected.diff(&actual), expected, &actual);
    Ok(VerifyReport {
        target_path: first_segment.to_string_lossy().to_string(),
        target_type: VerifyTarget::E01Image,
        expected: expected.clone(),
        actual,
        outcome,
        started_at,
        finished_at: crate::timeutil::now_iso8601(),
        duration_seconds: start.elapsed().as_secs_f64(),
    })
}

/// İki hash kümesini karşılaştırarak özet üretir.
fn summarize(
    _diff: &[(HashAlgorithm, String, String)],
    expected: &HashSet,
    actual: &HashSet,
) -> VerificationOutcomeSummary {
    let outcome = crate::hash::verify_hashes(expected, actual);
    VerificationOutcomeSummary {
        matched: outcome
            .matches
            .iter()
            .map(|v| v.algorithm.name().to_string())
            .collect(),
        mismatched: outcome
            .mismatches
            .iter()
            .map(|(hv, act)| {
                (
                    hv.algorithm.name().to_string(),
                    hv.value.clone(),
                    act.clone(),
                )
            })
            .collect(),
        missing: outcome
            .missing
            .iter()
            .map(|a| a.name().to_string())
            .collect(),
        success: outcome.is_success(),
    }
}

/// Bir imajın blok tablosunu doğrulayarak (her bloğun adler32 sağlaması)
/// veri bütünlüğünü denetler. E01 için kullanılır.
pub fn verify_e01_integrity(
    image: &crate::e01::EwfImage,
    progress: Option<ProgressCallback>,
    cancel: &CancelToken,
) -> Result<IntegrityReport> {
    let start = std::time::Instant::now();
    let total = image.chunk_count() as u64;
    let tracker = ProgressTracker::new(total);
    let mut errors = Vec::new();
    let mut ok_count = 0u64;

    for i in 0..image.chunk_count() {
        cancel.check()?;
        match image.read_chunk(i) {
            Ok(_) => ok_count += 1,
            Err(e) => errors.push(format!("Blok {i}: {e}")),
        }
        if let Some(cb) = &progress {
            if tracker.should_report() {
                cb(tracker.progress(i as u64, i as u64, "Blok sağlamaları doğrulanıyor..."));
            }
        }
    }

    Ok(IntegrityReport {
        total_chunks: total,
        ok_chunks: ok_count,
        error_chunks: errors.len() as u64,
        errors,
        duration_seconds: start.elapsed().as_secs_f64(),
    })
}

/// Bütünlük doğrulama raporu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityReport {
    /// Toplam blok.
    pub total_chunks: u64,
    /// Sağlaması doğru blok.
    pub ok_chunks: u64,
    /// Hatalı blok.
    pub error_chunks: u64,
    /// Hata açıklamaları.
    pub errors: Vec<String>,
    /// Süre (saniye).
    pub duration_seconds: f64,
}

impl IntegrityReport {
    /// Bütün bloklar sağlam mı?
    pub fn is_ok(&self) -> bool {
        self.error_chunks == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn verify_file_matches() {
        let path = std::env::temp_dir().join("delilx_verify_test.bin");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"delilx dogrulama testi").unwrap();
        drop(f);

        let expected = crate::hash::hash_file(&path, &[HashAlgorithm::Sha256]).unwrap();
        let report = verify_file(&path, &expected, &[HashAlgorithm::Sha256]).unwrap();
        assert!(report.outcome.success);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn verify_file_detects_corruption() {
        let path = std::env::temp_dir().join("delilx_verify_corrupt.bin");
        std::fs::write(&path, b"orijinal icerik").unwrap();
        let expected = crate::hash::hash_file(&path, &[HashAlgorithm::Sha256]).unwrap();
        // Dosyayı değiştir
        std::fs::write(&path, b"degistirilmis icerik").unwrap();
        let report = verify_file(&path, &expected, &[HashAlgorithm::Sha256]).unwrap();
        assert!(!report.outcome.success);
        assert_eq!(report.outcome.mismatched.len(), 1);

        let _ = std::fs::remove_file(&path);
    }
}
