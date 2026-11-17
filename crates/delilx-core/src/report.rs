//! Türkçe delil raporu üretimi (PDF, JSON, CSV).
//!
//! Rapor, yapılan edinim işleminin bütün ayrıntılarını Türkçe olarak içerir:
//! işlem numarası, delil numarası, incelemeyi yapan kişi, kaynak cihaz
//! bilgileri, edinim yöntemi, başlangıç/bitiş zamanı, dosya sayısı, veri
//! miktarı, hash değerleri, doğrulama sonucu ve hata/uyarılar.
//!
//! PDF çıktısı için DejaVu Sans yazı tipi gömülür; böylece Türkçe karakterler
//! (ç, ğ, ı, İ, ö, ş, ü) doğru görüntülenir.

use crate::error::{DelilXError, Result};
use crate::hash::HashSet;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Raporda yer alan tek bir hash kaydı.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportHash {
    /// Algoritma adı.
    pub algorithm: String,
    /// Onaltılık değer.
    pub value: String,
    /// Doğrulama durumu (Türkçe).
    pub verification: String,
}

/// Raporda yer alan bir cihaz/birim bilgisi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportDevice {
    /// Görünen ad.
    pub name: String,
    /// Tür.
    pub kind: String,
    /// Kapasite (insan okunur).
    pub capacity: String,
    /// Seri numarası.
    pub serial: String,
    /// Dosya sistemi.
    pub filesystem: String,
    /// Yazma koruması.
    pub write_protection: String,
}

/// Rapor içeriği. Bütün alanlar Türkçe etiketlidir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceReport {
    /// İşlem numarası.
    pub operation_number: String,
    /// Delil numarası.
    pub evidence_number: String,
    /// İncelemeyi yapan kişi.
    pub examiner: String,
    /// Vaka numarası.
    pub case_number: String,
    /// Açıklama.
    pub description: String,
    /// Kaynak cihaz bilgileri.
    pub source_device: ReportDevice,
    /// Hedef bilgisi.
    pub target: String,
    /// Edinim yöntemi (Türkçe).
    pub acquisition_method: String,
    /// Başlangıç zamanı.
    pub started_at: String,
    /// Bitiş zamanı.
    pub finished_at: String,
    /// Süre (insan okunur).
    pub duration: String,
    /// Dosya sayısı (mantıksal/seçmeli için).
    pub file_count: Option<u64>,
    /// Veri miktarı (bayt).
    pub data_bytes: u64,
    /// Veri miktarı (insan okunur).
    pub data_size: String,
    /// Hash değerleri.
    pub hashes: Vec<ReportHash>,
    /// Doğrulama sonucu (Türkçe).
    pub verification_result: String,
    /// Doğrulama başarılı mı?
    pub verification_success: Option<bool>,
    /// Hatalar.
    pub errors: Vec<String>,
    /// Uyarılar.
    pub warnings: Vec<String>,
    /// Korunamayan metadata alanları (varsa).
    pub unsupported_metadata: Vec<String>,
    /// Rapor oluşturma zamanı.
    pub generated_at: String,
    /// Uygulama sürümü.
    pub app_version: String,
}

impl EvidenceReport {
    /// Yeni boş rapor oluşturur.
    pub fn new() -> Self {
        EvidenceReport {
            operation_number: String::new(),
            evidence_number: String::new(),
            examiner: String::new(),
            case_number: String::new(),
            description: String::new(),
            source_device: ReportDevice {
                name: "Bilinmiyor".to_string(),
                kind: "Bilinmiyor".to_string(),
                capacity: "Bilinmiyor".to_string(),
                serial: "-".to_string(),
                filesystem: "-".to_string(),
                write_protection: "Bilinmiyor".to_string(),
            },
            target: String::new(),
            acquisition_method: String::new(),
            started_at: String::new(),
            finished_at: String::new(),
            duration: String::new(),
            file_count: None,
            data_bytes: 0,
            data_size: String::new(),
            hashes: Vec::new(),
            verification_result: "Yapılmadı".to_string(),
            verification_success: None,
            errors: Vec::new(),
            warnings: Vec::new(),
            unsupported_metadata: Vec::new(),
            generated_at: crate::timeutil::now_iso8601(),
            app_version: crate::APP_VERSION.to_string(),
        }
    }

    /// Hash kümesini rapora ekler.
    pub fn add_hashes(&mut self, hashes: &HashSet, verified: Option<bool>) {
        for h in &hashes.values {
            let verification = match verified {
                Some(true) => "Doğrulandı (eşleşti)".to_string(),
                Some(false) => "DOĞRULANAMADI (uyuşmadı)".to_string(),
                None => "Doğrulama yapılmadı".to_string(),
            };
            self.hashes.push(ReportHash {
                algorithm: h.algorithm.name().to_string(),
                value: h.value.clone(),
                verification,
            });
        }
    }

    /// Raporu JSON biçimine dönüştürür.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| DelilXError::other(format!("JSON üretilemedi: {e}")))
    }

    /// Raporu CSV biçimine dönüştürür (tek kayıt, alan başına satır).
    pub fn to_csv(&self) -> String {
        let mut out = String::from("Alan,Değer\n");
        let rows: Vec<(&str, String)> = vec![
            ("İşlem Numarası", self.operation_number.clone()),
            ("Delil Numarası", self.evidence_number.clone()),
            ("Vaka Numarası", self.case_number.clone()),
            ("İncelemeyi Yapan", self.examiner.clone()),
            ("Açıklama", self.description.clone()),
            ("Kaynak Cihaz", self.source_device.name.clone()),
            ("Cihaz Türü", self.source_device.kind.clone()),
            ("Cihaz Kapasitesi", self.source_device.capacity.clone()),
            ("Cihaz Seri No", self.source_device.serial.clone()),
            ("Dosya Sistemi", self.source_device.filesystem.clone()),
            (
                "Yazma Koruması",
                self.source_device.write_protection.clone(),
            ),
            ("Hedef", self.target.clone()),
            ("Edinim Yöntemi", self.acquisition_method.clone()),
            ("Başlangıç", self.started_at.clone()),
            ("Bitiş", self.finished_at.clone()),
            ("Süre", self.duration.clone()),
            (
                "Dosya Sayısı",
                self.file_count
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "-".into()),
            ),
            ("Veri Miktarı", self.data_size.clone()),
            ("Doğrulama Sonucu", self.verification_result.clone()),
            ("Oluşturulma", self.generated_at.clone()),
            ("Uygulama Sürümü", self.app_version.clone()),
        ];
        for (k, v) in rows {
            out.push_str(&format!("{},{}\n", csv_escape(k), csv_escape(&v)));
        }
        for h in &self.hashes {
            out.push_str(&format!(
                "{},{}\n",
                csv_escape(&h.algorithm),
                csv_escape(&h.value)
            ));
        }
        for w in &self.warnings {
            out.push_str(&format!("Uyarı,{}\n", csv_escape(w)));
        }
        for e in &self.errors {
            out.push_str(&format!("Hata,{}\n", csv_escape(e)));
        }
        out
    }

    /// Raporu düz metin biçimine dönüştürür (Türkçe).
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        s.push_str("========================================\n");
        s.push_str("          DELİLX DELİL RAPORU\n");
        s.push_str("   Dijital Delili Koru, Doğrula ve İncele.\n");
        s.push_str("========================================\n\n");
        s.push_str(&format!("İşlem Numarası : {}\n", self.operation_number));
        s.push_str(&format!("Delil Numarası : {}\n", self.evidence_number));
        s.push_str(&format!("Vaka Numarası  : {}\n", self.case_number));
        s.push_str(&format!("İnceleyen      : {}\n", self.examiner));
        s.push_str(&format!("Açıklama       : {}\n", self.description));
        s.push_str("\n--- Kaynak Cihaz ---\n");
        s.push_str(&format!("Ad             : {}\n", self.source_device.name));
        s.push_str(&format!("Tür            : {}\n", self.source_device.kind));
        s.push_str(&format!(
            "Kapasite       : {}\n",
            self.source_device.capacity
        ));
        s.push_str(&format!("Seri No        : {}\n", self.source_device.serial));
        s.push_str(&format!(
            "Dosya Sistemi  : {}\n",
            self.source_device.filesystem
        ));
        s.push_str(&format!(
            "Yazma Koruması : {}\n",
            self.source_device.write_protection
        ));
        s.push_str("\n--- Edinim ---\n");
        s.push_str(&format!("Yöntem         : {}\n", self.acquisition_method));
        s.push_str(&format!("Hedef          : {}\n", self.target));
        s.push_str(&format!("Başlangıç      : {}\n", self.started_at));
        s.push_str(&format!("Bitiş          : {}\n", self.finished_at));
        s.push_str(&format!("Süre           : {}\n", self.duration));
        if let Some(fc) = self.file_count {
            s.push_str(&format!("Dosya Sayısı   : {fc}\n"));
        }
        s.push_str(&format!("Veri Miktarı   : {}\n", self.data_size));
        s.push_str("\n--- Hash Değerleri ---\n");
        for h in &self.hashes {
            s.push_str(&format!("{:8}: {}\n", h.algorithm, h.value));
        }
        s.push_str(&format!(
            "\nDoğrulama Sonucu: {}\n",
            self.verification_result
        ));
        if !self.unsupported_metadata.is_empty() {
            s.push_str("\n--- Korunamayan Metadata ---\n");
            for u in &self.unsupported_metadata {
                s.push_str(&format!("- {u}\n"));
            }
        }
        if !self.warnings.is_empty() {
            s.push_str("\n--- Uyarılar ---\n");
            for w in &self.warnings {
                s.push_str(&format!("- {w}\n"));
            }
        }
        if !self.errors.is_empty() {
            s.push_str("\n--- Hatalar ---\n");
            for e in &self.errors {
                s.push_str(&format!("- {e}\n"));
            }
        }
        s.push_str(&format!("\nRapor Oluşturulma: {}\n", self.generated_at));
        s.push_str(&format!("Uygulama Sürümü  : DelilX {}\n", self.app_version));
        s
    }

    /// Raporu PDF dosyasına yazar.
    pub fn write_pdf(&self, path: &Path) -> Result<()> {
        crate::report_pdf::write_report_pdf(self, path)
    }

    /// Raporu JSON dosyasına yazar.
    pub fn write_json(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_json()?)
            .map_err(|e| DelilXError::io("JSON raporu yazılamadı", Some(path.to_path_buf()), e))
    }

    /// Raporu CSV dosyasına yazar.
    pub fn write_csv(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_csv())
            .map_err(|e| DelilXError::io("CSV raporu yazılamadı", Some(path.to_path_buf()), e))
    }

    /// Raporu metin dosyasına yazar.
    pub fn write_text(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_text())
            .map_err(|e| DelilXError::io("Metin raporu yazılamadı", Some(path.to_path_buf()), e))
    }

    /// Bütün biçimleri belirtilen dizine yazar ve yolları döndürür.
    pub fn write_all(&self, dir: &Path, base_name: &str) -> Result<Vec<PathBuf>> {
        std::fs::create_dir_all(dir).map_err(|e| {
            DelilXError::io("Rapor dizini oluşturulamadı", Some(dir.to_path_buf()), e)
        })?;
        let pdf = dir.join(format!("{base_name}.pdf"));
        let json = dir.join(format!("{base_name}.json"));
        let csv = dir.join(format!("{base_name}.csv"));
        self.write_pdf(&pdf)?;
        self.write_json(&json)?;
        self.write_csv(&csv)?;
        Ok(vec![pdf, json, csv])
    }
}

impl Default for EvidenceReport {
    fn default() -> Self {
        Self::new()
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// İşlem numarası üretir (tarih + saat).
pub fn generate_operation_number() -> String {
    let now = chrono::Local::now();
    format!("DLX-{}", now.format("%Y%m%d-%H%M%S"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::HashAlgorithm;

    fn sample_report() -> EvidenceReport {
        let mut r = EvidenceReport::new();
        r.operation_number = "DLX-20260101-120000".to_string();
        r.evidence_number = "DELIL-1".to_string();
        r.examiner = "Ahmet Yılmaz".to_string();
        r.acquisition_method = "RAW / DD İmaj".to_string();
        r.data_bytes = 1024;
        r.data_size = "1.00 KiB".to_string();
        let mut hs = HashSet::new();
        hs.insert(HashAlgorithm::Sha256, "abc123".to_string());
        r.add_hashes(&hs, Some(true));
        r.verification_result = "BAŞARILI".to_string();
        r.verification_success = Some(true);
        r
    }

    #[test]
    fn json_contains_turkish() {
        let r = sample_report();
        let json = r.to_json().unwrap();
        assert!(json.contains("Ahmet Yılmaz"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn csv_has_header() {
        let r = sample_report();
        let csv = r.to_csv();
        assert!(csv.starts_with("Alan,Değer"));
        assert!(csv.contains("İşlem Numarası"));
    }

    #[test]
    fn text_has_slogan_and_fields() {
        let r = sample_report();
        let t = r.to_text();
        assert!(t.contains("Dijital Delili Koru"));
        assert!(t.contains("İşlem Numarası"));
        assert!(t.contains("SHA-256"));
    }

    #[test]
    fn operation_number_format() {
        let n = generate_operation_number();
        assert!(n.starts_with("DLX-"));
    }
}
