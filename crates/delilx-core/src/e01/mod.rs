//! E01 (EWF1 / Expert Witness Format) yazıcı ve okuyucu.
//!
//! Bu modül gerçek EnCase uyumlu E01 imajları üretir ve okur. Format,
//! libewf (Joachim Metz) kaynak tanımlarına ve referans dosyalarla yapılan
//! bayt düzeyi doğrulamaya dayanır. Üretilen dosyalar bağımsız `ewfinfo` /
//! `ewfverify` araçlarıyla doğrulanır (bkz. TESTLER.md).
//!
//! ## Bölüm (section) düzeni
//!
//! ```text
//! Dosya başlığı (13 bayt): "EVF\x09\x0d\x0a\xff\x00" + alanlar + segment no
//! Bölüm tanımlayıcı (76 bayt): tür(16) + sonraki(8) + boyut(8) + dolgu(40) + adler32(4)
//! Bölüm gövdesi: tanımlayıcıdan hemen sonra, `boyut - 76` bayt
//! ```
//!
//! ## Segment düzeni
//!
//! - Segment 1: header2, header2, header, volume, [sectors, table, table2]
//! - Sonraki segmentler: data, [sectors, table, table2]
//! - Son segment kapanışı: (data), (digest), hash, done
//! - Ara segment kapanışı: next

pub mod reader;
pub mod writer;

pub use reader::{EwfImage, EwfImageInfo, EwfSegmentInfo, EwfVolumeInfo};
pub use writer::{EwfWriter, EwfWriterConfig, EwfWriterResult};

/// EWF dosya imzası.
pub const EWF_SIGNATURE: [u8; 8] = [0x45, 0x56, 0x46, 0x09, 0x0d, 0x0a, 0xff, 0x00];

/// Bölüm tanımlayıcı boyutu.
pub const SECTION_DESCRIPTOR_SIZE: u64 = 76;

/// Dosya başlığı boyutu.
pub const FILE_HEADER_SIZE: u64 = 13;

/// EWF medya türleri.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EwfMediaType {
    /// Çıkarılabilir disk (USB, SD).
    Removable = 0x00,
    /// Sabit disk.
    Fixed = 0x01,
    /// Optik disk.
    Optical = 0x03,
    /// Tek dosyalar.
    SingleFiles = 0x0e,
    /// Bellek.
    Memory = 0x10,
}

/// Medya bayrakları.
pub mod media_flags {
    /// Fiziksel aygıt görüntüsü.
    pub const PHYSICAL: u8 = 0x02;
    /// Fastbloc yazma engelleyici.
    pub const FASTBLOC: u8 = 0x04;
    /// Tableau yazma engelleyici.
    pub const TABLEAU: u8 = 0x08;
}

/// Sıkıştırma seviyeleri (EnCase 5+).
pub mod compression_level {
    /// Sıkıştırma yok.
    pub const NONE: u8 = 0x00;
    /// Hızlı/iyi sıkıştırma.
    pub const FAST: u8 = 0x01;
    /// En iyi sıkıştırma.
    pub const BEST: u8 = 0x02;
}

/// Tablo girişlerindeki sıkıştırma bayrağı (bit 31).
pub const TABLE_ENTRY_COMPRESSED: u32 = 0x8000_0000;

/// Tablo girişlerindeki ofset maskesi.
pub const TABLE_ENTRY_OFFSET_MASK: u32 = 0x7fff_ffff;

/// adler32 sağlama toplamı hesaplar (RFC 1950).
pub fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65521;
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    // 5552, taşma olmadan işlenebilecek en büyük blok boyutudur.
    for chunk in data.chunks(5552) {
        for &byte in chunk {
            a += byte as u32;
            b += a;
        }
        a %= MOD_ADLER;
        b %= MOD_ADLER;
    }
    (b << 16) | a
}

/// E01/EWF bölüm türleri.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionType {
    /// UTF-16LE başlık.
    Header2,
    /// ASCII başlık.
    Header,
    /// Birim bilgisi (ilk segment).
    Volume,
    /// Birim bilgisi (sonraki segmentler).
    Data,
    /// Veri blokları.
    Sectors,
    /// Blok tablosu.
    Table,
    /// Blok tablosu (kopya).
    Table2,
    /// Özet (MD5 + SHA1).
    Digest,
    /// MD5 hash.
    Hash,
    /// Sonraki segmente işaretçi.
    Next,
    /// Dosya sonu.
    Done,
    /// Bilinmeyen.
    Unknown,
}

impl SectionType {
    /// Bölüm türünün adını döndürür.
    pub fn as_name(&self) -> &'static str {
        match self {
            SectionType::Header2 => "header2",
            SectionType::Header => "header",
            SectionType::Volume => "volume",
            SectionType::Data => "data",
            SectionType::Sectors => "sectors",
            SectionType::Table => "table",
            SectionType::Table2 => "table2",
            SectionType::Digest => "digest",
            SectionType::Hash => "hash",
            SectionType::Next => "next",
            SectionType::Done => "done",
            SectionType::Unknown => "unknown",
        }
    }

    /// 16 baytlık bölüm türü alanı üretir.
    pub fn to_field(&self) -> [u8; 16] {
        let mut field = [0u8; 16];
        let name = self.as_name().as_bytes();
        let n = name.len().min(16);
        field[..n].copy_from_slice(&name[..n]);
        field
    }

    /// Bölüm türü alanından tür çözümler.
    pub fn from_field(field: &[u8]) -> SectionType {
        let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
        match &field[..end] {
            b"header2" => SectionType::Header2,
            b"header" => SectionType::Header,
            b"volume" => SectionType::Volume,
            b"data" => SectionType::Data,
            b"sectors" => SectionType::Sectors,
            b"table" => SectionType::Table,
            b"table2" => SectionType::Table2,
            b"digest" => SectionType::Digest,
            b"hash" => SectionType::Hash,
            b"next" => SectionType::Next,
            b"done" => SectionType::Done,
            _ => SectionType::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adler32_known_vector() {
        // "abc" için bilinen adler32 değeri
        assert_eq!(adler32(b"abc"), 0x024d0127);
        assert_eq!(adler32(b""), 1);
    }

    #[test]
    fn section_type_roundtrip() {
        for t in [
            SectionType::Header2,
            SectionType::Header,
            SectionType::Volume,
            SectionType::Data,
            SectionType::Sectors,
            SectionType::Table,
            SectionType::Table2,
            SectionType::Digest,
            SectionType::Hash,
            SectionType::Next,
            SectionType::Done,
        ] {
            let field = t.to_field();
            assert_eq!(SectionType::from_field(&field), t);
        }
    }
}
