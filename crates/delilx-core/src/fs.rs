//! Dosya sistemi çözümleme modülü (imaj inceleme için).
//!
//! Bir imaj içindeki bölümleri listeler ve desteklenen dosya sistemlerini
//! (NTFS, FAT12/16/32, exFAT) yorumlayarak dosya listesi ve metadata sağlar.
//! İmaj dosyası yalnızca okunur; hiçbir koşulda değiştirilmez.

use crate::error::{DelilXError, Result};
use serde::{Deserialize, Serialize};

/// Bölüm tablosu türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartitionTableType {
    /// Ana Önyükleme Kaydı (MBR).
    Mbr,
    /// GUID Bölüm Tablosu (GPT).
    Gpt,
    /// Bölüm tablosu bulunamadı.
    None,
}

impl PartitionTableType {
    /// Türkçe etiket.
    pub fn label(&self) -> &'static str {
        match self {
            PartitionTableType::Mbr => "MBR (Ana Önyükleme Kaydı)",
            PartitionTableType::Gpt => "GPT (GUID Bölüm Tablosu)",
            PartitionTableType::None => "Bölüm tablosu yok",
        }
    }
}

/// Bir bölüm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    /// Bölüm numarası.
    pub index: u32,
    /// Başlangıç sektörü.
    pub start_sector: u64,
    /// Sektör sayısı.
    pub sector_count: u64,
    /// Başlangıç ofseti (bayt).
    pub start_offset: u64,
    /// Boyut (bayt).
    pub size_bytes: u64,
    /// Bölüm türü kodu.
    pub type_code: String,
    /// Dosya sistemi (tespit edilebilirse).
    pub filesystem: Option<String>,
    /// Birim etiketi (tespit edilebilirse).
    pub label: Option<String>,
    /// Önyüklenebilir mi?
    pub bootable: bool,
    /// Ayrılmış/boş mu?
    pub is_empty: bool,
}

/// İmaj içinde bulunan bölümler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionLayout {
    /// Bölüm tablosu türü.
    pub table_type: PartitionTableType,
    /// Sektör boyutu.
    pub bytes_per_sector: u32,
    /// Bölümler.
    pub partitions: Vec<Partition>,
}

/// Sektör boyutu verilen bir imajdan bölüm tablosunu okur.
pub fn parse_partitions<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    bytes_per_sector: u32,
) -> Result<PartitionLayout> {
    // MBR okunur (ilk sektör)
    let mut mbr = vec![0u8; bytes_per_sector as usize];
    reader
        .seek(std::io::SeekFrom::Start(0))
        .map_err(|e| DelilXError::io("Bölüm tablosu okunamadı", None, e))?;
    reader
        .read_exact(&mut mbr)
        .map_err(|e| DelilXError::io("Bölüm tablosu okunamadı", None, e))?;

    // MBR imzası 0x55AA
    let is_mbr = mbr.len() >= 512 && mbr[510] == 0x55 && mbr[511] == 0xAA;
    if !is_mbr {
        return Ok(PartitionLayout {
            table_type: PartitionTableType::None,
            bytes_per_sector,
            partitions: Vec::new(),
        });
    }

    // MBR bölüm girişleri: 0x1BE'den itibaren 4 x 16 bayt
    let mut partitions = Vec::new();
    for i in 0..4 {
        let base = 0x1BE + i * 16;
        if base + 16 > mbr.len() {
            break;
        }
        let entry = &mbr[base..base + 16];
        let bootable = entry[0] == 0x80;
        let type_code = entry[4];
        let start_sector = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as u64;
        let sector_count = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as u64;

        if type_code == 0 && sector_count == 0 {
            continue;
        }
        // Genişletilmiş bölüm (0x05, 0x0F, 0x85) - basitlik için listelenir
        let is_empty = type_code == 0x00;

        partitions.push(Partition {
            index: i as u32 + 1,
            start_sector,
            sector_count,
            start_offset: start_sector * bytes_per_sector as u64,
            size_bytes: sector_count * bytes_per_sector as u64,
            type_code: mbr_type_name(type_code).to_string(),
            filesystem: None,
            label: None,
            bootable,
            is_empty,
        });
    }

    // GPT kontrolü: koruyucu MBR bölümü (0xEE) varsa GPT'yi dene
    let mut table_type = PartitionTableType::Mbr;
    if partitions.iter().any(|p| p.type_code.contains("GPT")) {
        table_type = PartitionTableType::Gpt;
        if let Ok(gpt_parts) = parse_gpt(reader, bytes_per_sector) {
            if !gpt_parts.is_empty() {
                partitions = gpt_parts;
            }
        }
    }

    Ok(PartitionLayout {
        table_type,
        bytes_per_sector,
        partitions,
    })
}

/// GPT bölüm tablosunu okur (LBA 1).
fn parse_gpt<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    bytes_per_sector: u32,
) -> Result<Vec<Partition>> {
    let header_offset = bytes_per_sector as u64;
    let mut header = vec![0u8; bytes_per_sector as usize];
    reader
        .seek(std::io::SeekFrom::Start(header_offset))
        .map_err(|e| DelilXError::io("GPT başlığı okunamadı", None, e))?;
    reader
        .read_exact(&mut header)
        .map_err(|e| DelilXError::io("GPT başlığı okunamadı", None, e))?;

    if header.len() < 92 || &header[0..8] != b"EFI PART" {
        return Ok(Vec::new());
    }

    let partition_entry_lba = u64::from_le_bytes(header[72..80].try_into().unwrap());
    let num_entries = u32::from_le_bytes(header[80..84].try_into().unwrap());
    let entry_size = u32::from_le_bytes(header[84..88].try_into().unwrap());
    let max_entries = num_entries.min(128);

    let entries_offset = partition_entry_lba * bytes_per_sector as u64;
    let mut partitions = Vec::new();
    for i in 0..max_entries {
        let off = entries_offset + (i as u64) * entry_size as u64;
        let mut entry = vec![0u8; entry_size as usize];
        reader
            .seek(std::io::SeekFrom::Start(off))
            .map_err(|e| DelilXError::io("GPT girişi okunamadı", None, e))?;
        if reader.read_exact(&mut entry).is_err() {
            break;
        }
        if entry.len() < 40 {
            break;
        }
        // Bölüm türü GUID'i tamamen sıfırsa boş
        let type_guid = &entry[0..16];
        if type_guid.iter().all(|&b| b == 0) {
            continue;
        }
        let first_lba = u64::from_le_bytes(entry[32..40].try_into().unwrap());
        let last_lba = u64::from_le_bytes(entry[40..48].try_into().unwrap());
        let sector_count = last_lba.saturating_sub(first_lba) + 1;
        // Ad (UTF-16LE, 36 karakter = 72 bayt)
        let label = if entry.len() >= 56 + 72 {
            let name_bytes = &entry[56..128];
            let u16s: Vec<u16> = name_bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .take_while(|&c| c != 0)
                .collect();
            let s = String::from_utf16_lossy(&u16s);
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        } else {
            None
        };

        partitions.push(Partition {
            index: i + 1,
            start_sector: first_lba,
            sector_count,
            start_offset: first_lba * bytes_per_sector as u64,
            size_bytes: sector_count * bytes_per_sector as u64,
            type_code: gpt_type_name(type_guid).to_string(),
            filesystem: None,
            label,
            bootable: false,
            is_empty: false,
        });
    }
    Ok(partitions)
}

/// MBR bölüm türü kodunun Türkçe adını döndürür.
pub fn mbr_type_name(code: u8) -> &'static str {
    match code {
        0x00 => "Boş",
        0x01 => "FAT12",
        0x04 => "FAT16 (<32MB)",
        0x05 => "Genişletilmiş (CHS)",
        0x06 => "FAT16B",
        0x07 => "NTFS/exFAT",
        0x0B => "FAT32 (CHS)",
        0x0C => "FAT32 (LBA)",
        0x0E => "FAT16 (LBA)",
        0x0F => "Genişletilmiş (LBA)",
        0x82 => "Linux takas",
        0x83 => "Linux",
        0x85 => "Linux genişletilmiş",
        0x8E => "Linux LVM",
        0xAF => "macOS HFS+",
        0xEE => "GPT koruyucu",
        0xEF => "EFI Sistem",
        _ => "Bilinmeyen",
    }
}

/// GPT bölüm türü GUID'inin Türkçe adını döndürür.
fn gpt_type_name(guid: &[u8]) -> &'static str {
    // EFI Sistem Bölümü
    const ESP: [u8; 16] = [
        0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9,
        0x3b,
    ];
    // Microsoft Temel Veri
    const MS_BASIC: [u8; 16] = [
        0xa2, 0xa0, 0xd0, 0xeb, 0xe5, 0xb9, 0x33, 0x44, 0x87, 0xc0, 0x68, 0xb6, 0xb7, 0x26, 0x99,
        0xc7,
    ];
    if guid == ESP {
        "EFI Sistem Bölümü"
    } else if guid == MS_BASIC {
        "Microsoft Temel Veri"
    } else {
        "GPT Bölümü"
    }
}

/// Dosya sistemi türünü bir bölümün başındaki imzadan tespit eder.
pub fn detect_filesystem<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    offset: u64,
) -> Result<Option<String>> {
    let mut buf = vec![0u8; 512];
    reader
        .seek(std::io::SeekFrom::Start(offset))
        .map_err(|e| DelilXError::io("Dosya sistemi imzası okunamadı", None, e))?;
    if reader.read_exact(&mut buf).is_err() {
        return Ok(None);
    }

    // NTFS: "NTFS    " @ 3
    if buf.len() >= 11 && &buf[3..7] == b"NTFS" {
        return Ok(Some("NTFS".to_string()));
    }
    // exFAT: "EXFAT   " @ 3
    if buf.len() >= 11 && &buf[3..8] == b"EXFAT" {
        return Ok(Some("exFAT".to_string()));
    }
    // FAT: "FAT12"/"FAT16"/"FAT32" @ 54 veya 82
    for off in [54usize, 82] {
        if buf.len() >= off + 8 {
            let sig = &buf[off..off + 8];
            if &sig[0..5] == b"FAT12" {
                return Ok(Some("FAT12".to_string()));
            }
            if &sig[0..5] == b"FAT16" {
                return Ok(Some("FAT16".to_string()));
            }
            if &sig[0..5] == b"FAT32" {
                return Ok(Some("FAT32".to_string()));
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn detects_ntfs_signature() {
        let mut data = vec![0u8; 1024];
        data[3..7].copy_from_slice(b"NTFS");
        let mut cur = Cursor::new(data);
        assert_eq!(
            detect_filesystem(&mut cur, 0).unwrap().as_deref(),
            Some("NTFS")
        );
    }

    #[test]
    fn detects_fat32_signature() {
        let mut data = vec![0u8; 1024];
        data[82..87].copy_from_slice(b"FAT32");
        let mut cur = Cursor::new(data);
        assert_eq!(
            detect_filesystem(&mut cur, 0).unwrap().as_deref(),
            Some("FAT32")
        );
    }

    #[test]
    fn no_filesystem_returns_none() {
        let data = vec![0u8; 1024];
        let mut cur = Cursor::new(data);
        assert!(detect_filesystem(&mut cur, 0).unwrap().is_none());
    }

    #[test]
    fn parses_mbr_partitions() {
        let mut data = vec![0u8; 1024 * 1024];
        // MBR imzası
        data[510] = 0x55;
        data[511] = 0xAA;
        // Birinci bölüm girişi
        data[0x1BE] = 0x80; // önyüklenebilir
        data[0x1BE + 4] = 0x07; // NTFS
        data[0x1BE + 8..0x1BE + 12].copy_from_slice(&2048u32.to_le_bytes());
        data[0x1BE + 12..0x1BE + 16].copy_from_slice(&1024u32.to_le_bytes());

        let mut cur = Cursor::new(data);
        let layout = parse_partitions(&mut cur, 512).unwrap();
        assert_eq!(layout.table_type, PartitionTableType::Mbr);
        assert_eq!(layout.partitions.len(), 1);
        assert_eq!(layout.partitions[0].start_sector, 2048);
        assert_eq!(layout.partitions[0].sector_count, 1024);
        assert!(layout.partitions[0].bootable);
    }
}
