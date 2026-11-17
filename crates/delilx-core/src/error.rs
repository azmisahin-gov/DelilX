//! DelilX hata türleri.
//!
//! Bütün kullanıcıya gösterilen mesajlar Türkçedir.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// DelilX işlemlerinde oluşabilecek hatalar.
#[derive(Debug)]
pub enum DelilXError {
    /// Girdi/çıktı hatası.
    Io {
        /// İşlem açıklaması.
        context: String,
        /// Yol bilgisi (varsa).
        path: Option<PathBuf>,
        /// Kaynak hata.
        source: io::Error,
    },
    /// E01/EWF imaj biçimi hatası.
    Ewf {
        /// Açıklama.
        context: String,
        /// Ayrıntı.
        detail: String,
    },
    /// İmaj veya cihaz biçimi geçersiz.
    Format {
        /// Açıklama.
        context: String,
        /// Ayrıntı.
        detail: String,
    },
    /// Kaynak cihaza yazma girişimi engellendi.
    WriteProtected {
        /// Cihaz yolu.
        device: String,
        /// Açıklama.
        detail: String,
    },
    /// Kaynak ve hedef aynı cihaz.
    SourceEqualsTarget {
        /// Cihaz.
        device: String,
    },
    /// Doğrulama (hash) başarısız.
    VerificationFailed {
        /// Beklenen değer.
        expected: String,
        /// Hesaplanan değer.
        actual: String,
        /// Algoritma.
        algorithm: String,
    },
    /// Okuma hatası eşiği aşıldı.
    TooManyReadErrors {
        /// Sektör numarası.
        sector: u64,
        /// Hata sayısı.
        count: u64,
    },
    /// Kullanıcı işlemi iptal etti.
    Cancelled,
    /// Desteklenmeyen özellik.
    Unsupported {
        /// Açıklama.
        what: String,
    },
    /// Genel hata.
    Other {
        /// Açıklama.
        message: String,
    },
}

impl DelilXError {
    /// Bağlam ekli G/Ç hatası üretir.
    pub fn io(context: impl Into<String>, path: Option<PathBuf>, source: io::Error) -> Self {
        DelilXError::Io {
            context: context.into(),
            path,
            source,
        }
    }

    /// EWF biçim hatası üretir.
    pub fn ewf(context: impl Into<String>, detail: impl Into<String>) -> Self {
        DelilXError::Ewf {
            context: context.into(),
            detail: detail.into(),
        }
    }

    /// Biçim hatası üretir.
    pub fn format(context: impl Into<String>, detail: impl Into<String>) -> Self {
        DelilXError::Format {
            context: context.into(),
            detail: detail.into(),
        }
    }

    /// Genel hata üretir.
    pub fn other(message: impl Into<String>) -> Self {
        DelilXError::Other {
            message: message.into(),
        }
    }

    /// Desteklenmeyen özellik hatası üretir.
    pub fn unsupported(what: impl Into<String>) -> Self {
        DelilXError::Unsupported { what: what.into() }
    }
}

impl fmt::Display for DelilXError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DelilXError::Io {
                context,
                path,
                source,
            } => match path {
                Some(p) => write!(f, "G/Ç hatası: {context} ({}): {source}", p.display()),
                None => write!(f, "G/Ç hatası: {context}: {source}"),
            },
            DelilXError::Ewf { context, detail } => {
                write!(f, "E01 imaj hatası: {context}: {detail}")
            }
            DelilXError::Format { context, detail } => {
                write!(f, "Biçim hatası: {context}: {detail}")
            }
            DelilXError::WriteProtected { device, detail } => write!(
                f,
                "Yazma koruması hatası: {device} cihazına yazma girişimi engellendi: {detail}"
            ),
            DelilXError::SourceEqualsTarget { device } => write!(
                f,
                "Kaynak ve hedef aynı olamaz: {device}. Kaynak delili korumak için işlem durduruldu."
            ),
            DelilXError::VerificationFailed {
                expected,
                actual,
                algorithm,
            } => write!(
                f,
                "Doğrulama başarısız ({algorithm}): beklenen {expected}, hesaplanan {actual}"
            ),
            DelilXError::TooManyReadErrors { sector, count } => write!(
                f,
                "Çok fazla okuma hatası: {sector} sektöründen itibaren {count} hata oluştu"
            ),
            DelilXError::Cancelled => write!(f, "İşlem kullanıcı tarafından iptal edildi"),
            DelilXError::Unsupported { what } => {
                write!(f, "Desteklenmeyen işlem: {what}")
            }
            DelilXError::Other { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for DelilXError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DelilXError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// DelilX sonuç türü.
pub type Result<T> = std::result::Result<T, DelilXError>;
