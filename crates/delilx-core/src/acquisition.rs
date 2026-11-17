//! Edinim türleri ve ortak yapılar.

use crate::error::{DelilXError, Result};
use serde::{Deserialize, Serialize};

/// Edinim yöntemi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionMethod {
    /// RAW/DD sektör sektör imaj.
    Raw,
    /// E01 (EnCase) imaj.
    E01,
    /// Mantıksal edinim (mevcut dosyalar).
    Logical,
    /// Seçmeli edinim (kullanıcı seçimi).
    Selective,
    /// Kullanılan alan edinimi.
    UsedSpace,
}

impl AcquisitionMethod {
    /// Türkçe etiket.
    pub fn label(&self) -> &'static str {
        match self {
            AcquisitionMethod::Raw => "RAW / DD İmaj",
            AcquisitionMethod::E01 => "E01 (EnCase) İmaj",
            AcquisitionMethod::Logical => "Mantıksal Edinim",
            AcquisitionMethod::Selective => "Seçmeli Edinim",
            AcquisitionMethod::UsedSpace => "Kullanılan Alan Edinimi",
        }
    }

    /// Kullanılan alan ediniminde silinmiş veri uyarısı.
    pub fn deleted_data_warning(&self) -> Option<&'static str> {
        match self {
            AcquisitionMethod::UsedSpace => Some(
                "Uyarı: Kullanılan alan ediniminde yalnızca dosya sisteminin kullandığı \
                 sektörler alınır. Boş alan dışarıda bırakılır; bu nedenle silinmiş \
                 dosyalar ve boş alandaki kalıntı veriler imaja dahil edilmez ve \
                 kurtarılamaz. Adli incelemede silinmiş veri önemliyse tam disk imajı \
                 (RAW veya E01) tercih edilmelidir.",
            ),
            AcquisitionMethod::Logical | AcquisitionMethod::Selective => Some(
                "Uyarı: Mantıksal ve seçmeli edinim yalnızca mevcut dosyaları alır. \
                 Silinmiş dosyalar, boş alan kalıntıları ve dosya sistemi üst verisi \
                 (journal, $MFT kayıtları) edinilmez.",
            ),
            _ => None,
        }
    }
}

/// Edinim ilerleme aşaması.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionStage {
    /// Hazırlık.
    Preparing,
    /// Kaynak okuma.
    Reading,
    /// Hedefe yazma.
    Writing,
    /// Hash hesaplama.
    Hashing,
    /// Doğrulama.
    Verifying,
    /// Tamamlandı.
    Completed,
    /// İptal edildi.
    Cancelled,
    /// Hata.
    Failed,
}

impl AcquisitionStage {
    /// Türkçe etiket.
    pub fn label(&self) -> &'static str {
        match self {
            AcquisitionStage::Preparing => "Hazırlık yapılıyor",
            AcquisitionStage::Reading => "Kaynak okunuyor",
            AcquisitionStage::Writing => "Hedefe yazılıyor",
            AcquisitionStage::Hashing => "Hash hesaplanıyor",
            AcquisitionStage::Verifying => "Doğrulanıyor",
            AcquisitionStage::Completed => "Tamamlandı",
            AcquisitionStage::Cancelled => "İptal edildi",
            AcquisitionStage::Failed => "Başarısız",
        }
    }
}

/// Edinim öncesi kaynak doğrulaması sonucu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightCheck {
    /// Kontrol adı (Türkçe).
    pub name: String,
    /// Geçti mi?
    pub passed: bool,
    /// Açıklama (Türkçe).
    pub detail: String,
    /// Kritik mi? (kritik kontrol başarısızsa edinim başlatılamaz)
    pub critical: bool,
}

impl PreflightCheck {
    /// Yeni kontrol sonucu oluşturur.
    pub fn new(
        name: impl Into<String>,
        passed: bool,
        detail: impl Into<String>,
        critical: bool,
    ) -> Self {
        PreflightCheck {
            name: name.into(),
            passed,
            detail: detail.into(),
            critical,
        }
    }
}

/// Edinim öncesi bütün güvenlik kontrollerini çalıştırır.
///
/// Kritik kontrollerden biri başarısız olursa edinim başlatılmaz.
pub fn preflight_checks(checks: &[PreflightCheck]) -> Result<()> {
    let failures: Vec<&PreflightCheck> =
        checks.iter().filter(|c| c.critical && !c.passed).collect();
    if !failures.is_empty() {
        let msgs: Vec<String> = failures
            .iter()
            .map(|c| format!("{}: {}", c.name, c.detail))
            .collect();
        return Err(DelilXError::other(format!(
            "Edinim başlatılamadı. Kritik güvenlik kontrolleri başarısız:\n{}",
            msgs.join("\n")
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_space_has_warning() {
        let w = AcquisitionMethod::UsedSpace.deleted_data_warning();
        assert!(w.is_some());
        assert!(w.unwrap().contains("silinmiş"));
    }

    #[test]
    fn raw_has_no_warning() {
        assert!(AcquisitionMethod::Raw.deleted_data_warning().is_none());
    }

    #[test]
    fn preflight_blocks_on_critical_failure() {
        let checks = vec![
            PreflightCheck::new("Yazma koruması", false, "Doğrulanamadı", true),
            PreflightCheck::new("Disk alanı", true, "Yeterli", true),
        ];
        assert!(preflight_checks(&checks).is_err());
    }

    #[test]
    fn preflight_allows_noncritical_failure() {
        let checks = vec![
            PreflightCheck::new("Seri numarası", false, "Okunamadı", false),
            PreflightCheck::new("Yazma koruması", true, "Doğrulandı", true),
        ];
        assert!(preflight_checks(&checks).is_ok());
    }
}
