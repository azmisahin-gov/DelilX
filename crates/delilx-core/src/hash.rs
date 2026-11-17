//! Hash hesaplama ve doğrulama modülü.
//!
//! SHA-256, SHA-512, SHA-1 ve MD5 desteklenir. Hash değerleri ile metadata
//! bütünlüğü birbirinden ayrı değerlendirilir.

use crate::error::{DelilXError, Result};
use digest::Digest;
use md5::Md5;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Desteklenen hash algoritmaları.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HashAlgorithm {
    /// SHA-256 (adli bilişim standardı).
    Sha256,
    /// SHA-512.
    Sha512,
    /// SHA-1 (E01 uyumluluğu için).
    Sha1,
    /// MD5 (E01 uyumluluğu için).
    Md5,
}

impl HashAlgorithm {
    /// Algoritmanın kısa adını döndürür.
    pub fn name(&self) -> &'static str {
        match self {
            HashAlgorithm::Sha256 => "SHA-256",
            HashAlgorithm::Sha512 => "SHA-512",
            HashAlgorithm::Sha1 => "SHA-1",
            HashAlgorithm::Md5 => "MD5",
        }
    }

    /// Algoritmanın küçük harfli kimliğini döndürür.
    pub fn id(&self) -> &'static str {
        match self {
            HashAlgorithm::Sha256 => "sha256",
            HashAlgorithm::Sha512 => "sha512",
            HashAlgorithm::Sha1 => "sha1",
            HashAlgorithm::Md5 => "md5",
        }
    }

    /// Hash özet uzunluğunu (bayt) döndürür.
    pub fn digest_len(&self) -> usize {
        match self {
            HashAlgorithm::Sha256 => 32,
            HashAlgorithm::Sha512 => 64,
            HashAlgorithm::Sha1 => 20,
            HashAlgorithm::Md5 => 16,
        }
    }
}

/// Tek bir hash değeri.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashValue {
    /// Algoritma.
    pub algorithm: HashAlgorithm,
    /// Onaltılık küçük harfli özet.
    pub value: String,
}

/// Bir veri kaynağı için hesaplanan hash kümesi.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashSet {
    /// Hesaplanan değerler.
    pub values: Vec<HashValue>,
}

impl HashSet {
    /// Yeni boş hash kümesi oluşturur.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bir hash değeri ekler.
    pub fn insert(&mut self, algorithm: HashAlgorithm, value: String) {
        self.values.retain(|v| v.algorithm != algorithm);
        self.values.push(HashValue { algorithm, value });
    }

    /// Algoritmaya göre değer döndürür.
    pub fn get(&self, algorithm: HashAlgorithm) -> Option<&str> {
        self.values
            .iter()
            .find(|v| v.algorithm == algorithm)
            .map(|v| v.value.as_str())
    }

    /// Kümeler arasındaki farkları döndürür (beklenen, hesaplanan).
    pub fn diff(&self, other: &HashSet) -> Vec<(HashAlgorithm, String, String)> {
        let mut out = Vec::new();
        for v in &self.values {
            if let Some(o) = other.get(v.algorithm) {
                if o != v.value {
                    out.push((v.algorithm, v.value.clone(), o.to_string()));
                }
            }
        }
        out
    }
}

/// Artımlı (streaming) hash hesaplayıcı.
///
/// Aynı anda birden çok algoritmayı besleyerek tek geçişte tüm özetleri üretir.
pub struct MultiHasher {
    algorithms: Vec<HashAlgorithm>,
    sha256: Option<Sha256>,
    sha512: Option<Sha512>,
    sha1: Option<Sha1>,
    md5: Option<Md5>,
    bytes_processed: u64,
}

impl MultiHasher {
    /// Verilen algoritmalarla yeni bir hesaplayıcı oluşturur.
    pub fn new(algorithms: &[HashAlgorithm]) -> Self {
        let mut has = MultiHasher {
            algorithms: algorithms.to_vec(),
            sha256: None,
            sha512: None,
            sha1: None,
            md5: None,
            bytes_processed: 0,
        };
        for a in algorithms {
            match a {
                HashAlgorithm::Sha256 => has.sha256 = Some(Sha256::new()),
                HashAlgorithm::Sha512 => has.sha512 = Some(Sha512::new()),
                HashAlgorithm::Sha1 => has.sha1 = Some(Sha1::new()),
                HashAlgorithm::Md5 => has.md5 = Some(Md5::new()),
            }
        }
        has
    }

    /// İşlenen bayt sayısını döndürür.
    pub fn bytes_processed(&self) -> u64 {
        self.bytes_processed
    }

    /// Veri bloğunu hesaplayıcıya besler.
    pub fn update(&mut self, data: &[u8]) {
        self.bytes_processed += data.len() as u64;
        if let Some(h) = self.sha256.as_mut() {
            h.update(data);
        }
        if let Some(h) = self.sha512.as_mut() {
            h.update(data);
        }
        if let Some(h) = self.sha1.as_mut() {
            h.update(data);
        }
        if let Some(h) = self.md5.as_mut() {
            h.update(data);
        }
    }

    /// Hesaplanan hash kümesini döndürür.
    pub fn finalize(self) -> HashSet {
        let mut set = HashSet::new();
        if let Some(h) = self.sha256 {
            set.insert(HashAlgorithm::Sha256, hex::encode(h.finalize()));
        }
        if let Some(h) = self.sha512 {
            set.insert(HashAlgorithm::Sha512, hex::encode(h.finalize()));
        }
        if let Some(h) = self.sha1 {
            set.insert(HashAlgorithm::Sha1, hex::encode(h.finalize()));
        }
        if let Some(h) = self.md5 {
            set.insert(HashAlgorithm::Md5, hex::encode(h.finalize()));
        }
        set
    }

    /// Kullanılan algoritmaların listesi.
    pub fn algorithms(&self) -> &[HashAlgorithm] {
        &self.algorithms
    }
}

/// Bir dosyanın hash kümesini hesaplar.
pub fn hash_file(path: &Path, algorithms: &[HashAlgorithm]) -> Result<HashSet> {
    let file = File::open(path)
        .map_err(|e| DelilXError::io("Dosya hash için açılamadı", Some(path.to_path_buf()), e))?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut hasher = MultiHasher::new(algorithms);
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| DelilXError::io("Dosya okunamadı (hash)", Some(path.to_path_buf()), e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize())
}

/// Bellekteki bir tamponun hash kümesini hesaplar.
pub fn hash_bytes(data: &[u8], algorithms: &[HashAlgorithm]) -> HashSet {
    let mut hasher = MultiHasher::new(algorithms);
    hasher.update(data);
    hasher.finalize()
}

/// İki hash kümesini karşılaştırır ve doğrulama sonucu üretir.
pub fn verify_hashes(expected: &HashSet, actual: &HashSet) -> VerificationOutcome {
    let mut matches = Vec::new();
    let mut mismatches = Vec::new();
    let mut missing = Vec::new();

    for v in &expected.values {
        match actual.get(v.algorithm) {
            Some(a) if a == v.value => matches.push(v.clone()),
            Some(a) => mismatches.push((v.clone(), a.to_string())),
            None => missing.push(v.algorithm),
        }
    }

    VerificationOutcome {
        matches,
        mismatches,
        missing,
    }
}

/// Hash doğrulama sonucu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationOutcome {
    /// Eşleşen değerler.
    pub matches: Vec<HashValue>,
    /// Uyuşmayan değerler (beklenen, hesaplanan).
    pub mismatches: Vec<(HashValue, String)>,
    /// Doğrulanamayan (hesaplanamayan) algoritmalar.
    pub missing: Vec<HashAlgorithm>,
}

impl VerificationOutcome {
    /// Bütün beklenen hash değerleri eşleşti mi?
    pub fn is_success(&self) -> bool {
        self.mismatches.is_empty() && self.missing.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vector() {
        // "abc" için bilinen SHA-256 değeri
        let set = hash_bytes(b"abc", &[HashAlgorithm::Sha256]);
        assert_eq!(
            set.get(HashAlgorithm::Sha256).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha512_known_vector() {
        let set = hash_bytes(b"abc", &[HashAlgorithm::Sha512]);
        assert_eq!(
            set.get(HashAlgorithm::Sha512).unwrap(),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
             2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
    }

    #[test]
    fn multi_hasher_consistency() {
        let data = vec![0xABu8; 100_000];
        let one_shot = hash_bytes(
            &data,
            &[
                HashAlgorithm::Sha256,
                HashAlgorithm::Sha512,
                HashAlgorithm::Sha1,
                HashAlgorithm::Md5,
            ],
        );
        let mut hasher = MultiHasher::new(&[
            HashAlgorithm::Sha256,
            HashAlgorithm::Sha512,
            HashAlgorithm::Sha1,
            HashAlgorithm::Md5,
        ]);
        for chunk in data.chunks(4096) {
            hasher.update(chunk);
        }
        let incremental = hasher.finalize();
        assert_eq!(one_shot, incremental);
    }

    #[test]
    fn verify_detects_mismatch() {
        let a = hash_bytes(b"abc", &[HashAlgorithm::Sha256]);
        let b = hash_bytes(b"abd", &[HashAlgorithm::Sha256]);
        let outcome = verify_hashes(&a, &b);
        assert!(!outcome.is_success());
        assert_eq!(outcome.mismatches.len(), 1);
    }

    #[test]
    fn verify_success() {
        let a = hash_bytes(b"abc", &[HashAlgorithm::Sha256, HashAlgorithm::Sha512]);
        let outcome = verify_hashes(&a, &a);
        assert!(outcome.is_success());
        assert_eq!(outcome.matches.len(), 2);
    }
}
