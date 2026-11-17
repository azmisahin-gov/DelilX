//! # DelilX Çekirdek Kütüphanesi
//!
//! **Dijital Delili Koru, Doğrula ve İncele.**
//!
//! Bu kütüphane DelilX adli bilişim uygulamasının motorunu içerir:
//! veri edinim (RAW/DD, E01, mantıksal, seçmeli), hash doğrulama, metadata
//! inceleme, imaj inceleme ve Türkçe raporlama.
//!
//! ## Temel ilkeler
//!
//! - **Kaynak delili koru:** Kaynak cihazlara hiçbir koşulda yazılmaz.
//! - **Edinimi doğrula:** SHA-256/SHA-512 ve E01 içi MD5/SHA-1 doğrulanır.
//! - **Bütün işlemleri kaydet:** Her işlem zaman damgalı olarak kaydedilir.
//! - **Türkçe anlat:** Bütün kullanıcı mesajları Türkçedir.

pub mod acquisition;
pub mod device;
pub mod e01;
pub mod error;
pub mod fs;
pub mod fs_fat;
pub mod hash;
pub mod inspect;
pub mod logical;
pub mod metadata;
pub mod progress;
pub mod raw;
pub mod report;
pub mod report_pdf;
pub mod source;
pub mod timeutil;
pub mod usedspace;
pub mod verify;
pub mod writeblock;

pub use error::{DelilXError, Result};
pub use hash::{HashAlgorithm, HashSet, HashValue};

/// Uygulama adı.
pub const APP_NAME: &str = "DelilX";

/// Uygulama sürümü.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Uygulama sloganı.
pub const APP_SLOGAN: &str = "Dijital Delili Koru, Doğrula ve İncele.";

/// Kütüphane sürümünü döndürür.
pub fn version() -> &'static str {
    APP_VERSION
}
