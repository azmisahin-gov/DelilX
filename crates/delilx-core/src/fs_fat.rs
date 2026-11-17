//! FAT12/16/32 dosya sistemi çözümleyicisi (imaj inceleme için).
//!
//! Salt okunur. İmaj içindeki dizin ağacını dolaşarak dosya listesi üretir.
//! Yalnızca kök dizin ve alt dizinlerdeki kısa adlı (8.3) girdiler çözümlenir;
//! uzun dosya adı (LFN) girdileri de desteklenir.

use crate::error::{DelilXError, Result};
use crate::inspect::{ImageReader, InspectedFile};
use crate::progress::CancelToken;

/// Önyükleme kesimi (boot sector) bilgileri.
#[derive(Debug, Clone)]
pub struct FatBoot {
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    num_fats: u8,
    root_entry_count: u16,
    fat_size16: u16,
    fat_size32: u32,
    root_cluster: u32,
    fs_type: FatType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FatType {
    Fat12,
    Fat16,
    Fat32,
}

impl FatBoot {
    fn fat_size(&self) -> u32 {
        if self.fat_size16 != 0 {
            self.fat_size16 as u32
        } else {
            self.fat_size32
        }
    }

    /// Kök dizin bölgesinin başlangıç sektörü.
    fn root_dir_start_sector(&self) -> u32 {
        self.reserved_sectors as u32 + (self.num_fats as u32) * self.fat_size()
    }

    /// Kök dizin bölgesinin sektör sayısı (yalnızca FAT12/16).
    fn root_dir_sectors(&self) -> u32 {
        let bytes = self.root_entry_count as u32 * 32;
        bytes.div_ceil(self.bytes_per_sector as u32)
    }

    /// Veri bölgesinin başlangıç sektörü.
    fn data_start_sector(&self) -> u32 {
        self.root_dir_start_sector() + self.root_dir_sectors()
    }

    /// Küme boyutu (bayt).
    fn cluster_size(&self) -> u64 {
        self.bytes_per_sector as u64 * self.sectors_per_cluster as u64
    }
}

/// FAT önyükleme kesimini okur.
fn parse_boot(reader: &ImageReader, partition_offset: u64) -> Result<FatBoot> {
    let mut buf = vec![0u8; 512];
    reader.read_at(partition_offset, &mut buf)?;

    if buf.len() < 90 {
        return Err(DelilXError::other("FAT önyükleme kesimi çok kısa."));
    }
    // Sıçrama komutu kontrolü
    if buf[0] != 0xEB && buf[0] != 0xE9 {
        return Err(DelilXError::other("Geçerli FAT önyükleme kesimi değil."));
    }

    let bytes_per_sector = u16::from_le_bytes([buf[11], buf[12]]);
    let sectors_per_cluster = buf[13];
    let reserved_sectors = u16::from_le_bytes([buf[14], buf[15]]);
    let num_fats = buf[16];
    let root_entry_count = u16::from_le_bytes([buf[17], buf[18]]);
    let total_sectors16 = u16::from_le_bytes([buf[19], buf[20]]);
    let fat_size16 = u16::from_le_bytes([buf[22], buf[23]]);
    let total_sectors32 = u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]);
    let fat_size32 = u32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]]);
    let root_cluster = u32::from_le_bytes([buf[44], buf[45], buf[46], buf[47]]);

    // FAT türünü toplam küme sayısından belirle
    let root_dir_sectors = (root_entry_count as u32 * 32).div_ceil(bytes_per_sector as u32);
    let fat_size = if fat_size16 != 0 {
        fat_size16 as u32
    } else {
        fat_size32
    };
    let total_sectors = if total_sectors16 != 0 {
        total_sectors16 as u32
    } else {
        total_sectors32
    };
    let _ = total_sectors;
    let data_sectors = total_sectors
        .saturating_sub(reserved_sectors as u32 + (num_fats as u32) * fat_size + root_dir_sectors);
    let count_of_clusters = if sectors_per_cluster > 0 {
        data_sectors / sectors_per_cluster as u32
    } else {
        0
    };

    let fs_type = {
        // Önce önyükleme kesimindeki fs_type metnini kullan (güvenilir).
        let mut from_string = None;
        for off in [82usize, 54] {
            if buf.len() >= off + 8 {
                let s = &buf[off..off + 8];
                if &s[0..5] == b"FAT32" {
                    from_string = Some(FatType::Fat32);
                    break;
                }
                if &s[0..5] == b"FAT16" {
                    from_string = Some(FatType::Fat16);
                    break;
                }
                if &s[0..5] == b"FAT12" {
                    from_string = Some(FatType::Fat12);
                    break;
                }
            }
        }
        from_string.unwrap_or(if count_of_clusters < 4085 {
            FatType::Fat12
        } else if count_of_clusters < 65525 {
            FatType::Fat16
        } else {
            FatType::Fat32
        })
    };

    Ok(FatBoot {
        bytes_per_sector,
        sectors_per_cluster,
        reserved_sectors,
        num_fats,
        root_entry_count,
        fat_size16,
        fat_size32,
        root_cluster,
        fs_type,
    })
}

/// FAT tablosundan bir kümenin sonraki kümesini okur.
fn next_cluster(
    reader: &ImageReader,
    boot: &FatBoot,
    partition_offset: u64,
    cluster: u32,
) -> Result<Option<u32>> {
    let fat_offset = match boot.fs_type {
        FatType::Fat12 => (cluster as u64 * 3) / 2,
        FatType::Fat16 => cluster as u64 * 2,
        FatType::Fat32 => cluster as u64 * 4,
    };
    let fat_start = partition_offset + boot.reserved_sectors as u64 * boot.bytes_per_sector as u64;
    let mut buf = vec![0u8; 4];
    reader.read_at(fat_start + fat_offset, &mut buf)?;

    let value = match boot.fs_type {
        FatType::Fat12 => {
            let v = u16::from_le_bytes([buf[0], buf[1]]);
            if cluster % 2 == 0 {
                (v & 0x0FFF) as u32
            } else {
                (v >> 4) as u32
            }
        }
        FatType::Fat16 => u16::from_le_bytes([buf[0], buf[1]]) as u32,
        FatType::Fat32 => u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) & 0x0FFFFFFF,
    };

    // Zincir sonu işaretleri
    let (eoc, bad) = match boot.fs_type {
        FatType::Fat12 => (0x0FF8u32, 0x0FF7u32),
        FatType::Fat16 => (0xFFF8u32, 0xFFF7u32),
        FatType::Fat32 => (0x0FFFFFF8u32, 0x0FFFFFF7u32),
    };
    if value >= eoc || value == bad || value == 0 {
        // zincir sonu, bozuk küme veya boş küme
        Ok(None)
    } else {
        Ok(Some(value))
    }
}

/// Bir kümenin imaj içindeki ofsetini döndürür.
fn cluster_offset(boot: &FatBoot, partition_offset: u64, cluster: u32) -> u64 {
    let data_start = boot.data_start_sector() as u64;
    let sector = data_start + (cluster as u64 - 2) * boot.sectors_per_cluster as u64;
    partition_offset + sector * boot.bytes_per_sector as u64
}

/// FAT dosya sistemindeki dosyaları listeler.
pub fn list_fat_files(
    reader: &ImageReader,
    partition_offset: u64,
    fs_name: &str,
    cancel: &CancelToken,
) -> Result<Vec<InspectedFile>> {
    let boot = parse_boot(reader, partition_offset)?;
    let mut files = Vec::new();
    let fs_label = if fs_name.is_empty() {
        format!("{:?}", boot.fs_type)
    } else {
        fs_name.to_string()
    };

    // Kök dizin
    if boot.fs_type == FatType::Fat32 {
        let root = boot.root_cluster;
        walk_directory(
            reader,
            &boot,
            partition_offset,
            root,
            "",
            &fs_label,
            &mut files,
            cancel,
            0,
        )?;
    } else {
        // FAT12/16: sabit kök dizin bölgesi
        let root_start =
            partition_offset + boot.root_dir_start_sector() as u64 * boot.bytes_per_sector as u64;
        let root_bytes = boot.root_entry_count as u64 * 32;
        walk_fixed_root(
            reader,
            &boot,
            partition_offset,
            root_start,
            root_bytes,
            &fs_label,
            &mut files,
            cancel,
        )?;
    }

    Ok(files)
}

/// FAT12/16 sabit kök dizinini dolaşır.
#[allow(clippy::too_many_arguments)]
fn walk_fixed_root(
    reader: &ImageReader,
    boot: &FatBoot,
    partition_offset: u64,
    start: u64,
    bytes: u64,
    fs_label: &str,
    files: &mut Vec<InspectedFile>,
    cancel: &CancelToken,
) -> Result<()> {
    let entries = (bytes / 32) as usize;
    let mut buf = vec![0u8; 32 * entries];
    let n = reader.read_at(start, &mut buf)?;
    let entries = n / 32;
    let mut lfn = String::new();
    let mut i = 0;
    while i < entries {
        cancel.check()?;
        let e = &buf[i * 32..i * 32 + 32];
        i += 1;
        if e[0] == 0x00 {
            break; // girdi sonu
        }
        if e[0] == 0xE5 {
            lfn.clear();
            continue; // silinmiş girdi
        }
        let attr = e[11];
        if attr & 0x08 != 0 {
            // Birim etiketi girdisi; atla.
            lfn.clear();
            continue;
        }
        if attr == 0x0F {
            // LFN girdisi: biriktir
            let part = decode_lfn_part(&e[1..11]);
            lfn = format!("{part}{lfn}");
            let part2 = decode_lfn_part(&e[14..26]);
            lfn = format!("{part2}{lfn}");
            let part3 = decode_lfn_part(&e[28..32]);
            lfn = format!("{part3}{lfn}");
            continue;
        }
        // Normal girdi
        let name = if !lfn.is_empty() {
            lfn.clone()
        } else {
            decode_short_name(&e[0..11])
        };
        lfn.clear();

        if name == "." || name == ".." {
            continue;
        }
        let is_dir = attr & 0x10 != 0;
        let high = u16::from_le_bytes([e[20], e[21]]) as u32;
        let low = u16::from_le_bytes([e[26], e[27]]) as u32;
        let cluster = (high << 16) | low;
        let size = u32::from_le_bytes([e[28], e[29], e[30], e[31]]) as u64;

        let offset = if cluster >= 2 {
            cluster_offset(boot, partition_offset, cluster)
        } else {
            0
        };

        files.push(InspectedFile {
            name: name.clone(),
            path: format!("/{name}"),
            size: if is_dir { 0 } else { size },
            offset,
            is_dir,
            filesystem: fs_label.to_string(),
        });

        if is_dir && cluster >= 2 {
            walk_directory(
                reader,
                boot,
                partition_offset,
                cluster,
                &format!("/{name}"),
                fs_label,
                files,
                cancel,
                0,
            )?;
        }
    }
    Ok(())
}

/// Bir dizin kümesini dolaşır (FAT32 ve alt dizinler).
#[allow(clippy::too_many_arguments)]
fn walk_directory(
    reader: &ImageReader,
    boot: &FatBoot,
    partition_offset: u64,
    start_cluster: u32,
    parent_path: &str,
    fs_label: &str,
    files: &mut Vec<InspectedFile>,
    cancel: &CancelToken,
    depth: u32,
) -> Result<()> {
    if depth > 32 {
        return Ok(()); // döngü koruması
    }
    let cluster_size = boot.cluster_size() as usize;
    let mut cluster = start_cluster;
    let mut iterations = 0;
    let mut lfn = String::new();
    while cluster >= 2 && iterations < 1_000_000 {
        cancel.check()?;
        iterations += 1;
        let off = cluster_offset(boot, partition_offset, cluster);
        let mut buf = vec![0u8; cluster_size];
        let n = reader.read_at(off, &mut buf)?;
        if n == 0 {
            break;
        }
        let entries = n / 32;
        for i in 0..entries {
            let e = &buf[i * 32..i * 32 + 32];
            if e[0] == 0x00 {
                return Ok(());
            }
            if e[0] == 0xE5 {
                lfn.clear();
                continue;
            }
            let attr = e[11];
            if attr & 0x08 != 0 {
                lfn.clear();
                continue;
            }
            if attr == 0x0F {
                let part = decode_lfn_part(&e[1..11]);
                lfn = format!("{part}{lfn}");
                let part2 = decode_lfn_part(&e[14..26]);
                lfn = format!("{part2}{lfn}");
                let part3 = decode_lfn_part(&e[28..32]);
                lfn = format!("{part3}{lfn}");
                continue;
            }
            let name = if !lfn.is_empty() {
                lfn.clone()
            } else {
                decode_short_name(&e[0..11])
            };
            lfn.clear();
            if name == "." || name == ".." {
                continue;
            }
            let is_dir = attr & 0x10 != 0;
            let high = u16::from_le_bytes([e[20], e[21]]) as u32;
            let low = u16::from_le_bytes([e[26], e[27]]) as u32;
            let sub_cluster = (high << 16) | low;
            let size = u32::from_le_bytes([e[28], e[29], e[30], e[31]]) as u64;
            let offset = if sub_cluster >= 2 {
                cluster_offset(boot, partition_offset, sub_cluster)
            } else {
                0
            };
            let path = format!("{parent_path}/{name}");
            files.push(InspectedFile {
                name: name.clone(),
                path: path.clone(),
                size: if is_dir { 0 } else { size },
                offset,
                is_dir,
                filesystem: fs_label.to_string(),
            });
            if is_dir && sub_cluster >= 2 {
                walk_directory(
                    reader,
                    boot,
                    partition_offset,
                    sub_cluster,
                    &path,
                    fs_label,
                    files,
                    cancel,
                    depth + 1,
                )?;
            }
        }
        // Sonraki küme
        match next_cluster(reader, boot, partition_offset, cluster)? {
            Some(next) => cluster = next,
            None => break,
        }
    }
    Ok(())
}

/// LFN parçasını (UTF-16LE) çözer.
fn decode_lfn_part(bytes: &[u8]) -> String {
    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|&c| c != 0x0000 && c != 0xFFFF)
        .collect();
    String::from_utf16_lossy(&u16s)
}

/// 8.3 kısa adı çözer.
fn decode_short_name(bytes: &[u8]) -> String {
    let base: Vec<u8> = bytes[0..8]
        .iter()
        .take_while(|&&b| b != b' ' && b != 0)
        .copied()
        .collect();
    let ext: Vec<u8> = bytes[8..11]
        .iter()
        .take_while(|&&b| b != b' ' && b != 0)
        .copied()
        .collect();
    let base = String::from_utf8_lossy(&base).to_string();
    let ext = String::from_utf8_lossy(&ext).to_string();
    // İlk bayt 0x05 ise gerçek karakter 0xE5
    if ext.is_empty() {
        base
    } else {
        format!("{base}.{ext}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_short_name() {
        let mut name = [b' '; 11];
        name[0..4].copy_from_slice(b"TEST");
        name[8..11].copy_from_slice(b"TXT");
        assert_eq!(decode_short_name(&name), "TEST.TXT");
    }

    #[test]
    fn decodes_lfn() {
        let text = "Uzun Dosya Adı.txt";
        let u16s: Vec<u16> = text.encode_utf16().collect();
        let mut bytes = Vec::new();
        for c in &u16s {
            bytes.extend_from_slice(&c.to_le_bytes());
        }
        let decoded = decode_lfn_part(&bytes);
        assert_eq!(decoded, "Uzun Dosya Adı.txt");
    }
}
