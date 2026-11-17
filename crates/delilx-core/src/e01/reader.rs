//! E01 (EWF1) imaj okuyucu.
//!
//! Segment dosyalarını açıp bölümleri çözümler, blok tablosunu okuyarak
//! rastgele erişimli okuma sağlar. Orijinal imaj dosyası hiçbir koşulda
//! değiştirilmez (yalnızca okuma).

use super::{
    adler32, SectionType, EWF_SIGNATURE, FILE_HEADER_SIZE, SECTION_DESCRIPTOR_SIZE,
    TABLE_ENTRY_COMPRESSED, TABLE_ENTRY_OFFSET_MASK,
};
use crate::error::{DelilXError, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// E01 segment dosyası bilgisi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EwfSegmentInfo {
    /// Segment dosyası yolu.
    pub path: String,
    /// Segment numarası (1 tabanlı).
    pub segment_number: u16,
    /// Dosya boyutu (bayt).
    pub size_bytes: u64,
    /// Bu segmentteki bölüm türleri (sırayla).
    pub sections: Vec<String>,
}

/// E01 birim (volume) bilgisi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EwfVolumeInfo {
    /// Medya türü (sayısal).
    pub media_type: u8,
    /// Medya türü (Türkçe).
    pub media_type_text: String,
    /// Blok sayısı.
    pub number_of_chunks: u32,
    /// Blok başına sektör.
    pub sectors_per_chunk: u32,
    /// Sektör başına bayt.
    pub bytes_per_sector: u32,
    /// Toplam sektör.
    pub number_of_sectors: u64,
    /// Toplam bayt.
    pub total_bytes: u64,
    /// Medya bayrakları.
    pub media_flags: u8,
    /// Fiziksel aygıt görüntüsü mü?
    pub is_physical: bool,
    /// Sıkıştırma seviyesi.
    pub compression_level: u8,
    /// Hata granülaritesi.
    pub error_granularity: u32,
}

/// E01 imaj özet bilgisi.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EwfImageInfo {
    /// Segment dosyaları.
    pub segments: Vec<EwfSegmentInfo>,
    /// Birim bilgisi.
    pub volume: EwfVolumeInfo,
    /// MD5 (imaj içinden).
    pub md5: Option<String>,
    /// SHA-1 (imaj içinden).
    pub sha1: Option<String>,
    /// header2 alanları (anahtar-değer).
    pub header2_fields: Vec<(String, String)>,
    /// Başlık (header) ham metni.
    pub header_text: Option<String>,
    /// Bulunan bölüm türleri (özet).
    pub section_types: Vec<String>,
}

/// İmaj içindeki bir blok tanımı.
#[derive(Debug, Clone, Copy)]
struct ChunkRef {
    /// Segment indeksi.
    segment_index: usize,
    /// Segment içindeki mutlak ofset.
    offset: u64,
    /// Veri boyutu (sıkıştırılmış ise sıkıştırılmış boyut).
    size: u32,
    /// Sıkıştırılmış mı?
    compressed: bool,
}

/// Açılmış bir E01 imajı.
pub struct EwfImage {
    /// Segment dosyaları (okuma için).
    segments: Vec<SegmentFile>,
    /// Blok referansları (sırayla).
    chunks: Vec<ChunkRef>,
    /// Birim bilgisi.
    volume: EwfVolumeInfo,
    /// MD5.
    md5: Option<String>,
    /// SHA-1.
    sha1: Option<String>,
    /// header2 alanları.
    header2_fields: Vec<(String, String)>,
    /// header metni.
    header_text: Option<String>,
    /// Özet bilgi (segment bilgileri dahil).
    info: EwfImageInfo,
}

/// Açık bir segment dosyası.
struct SegmentFile {
    path: PathBuf,
    file: File,
    size: u64,
    info: EwfSegmentInfo,
}

impl EwfImage {
    /// E01 imajını açar. `path` ilk segment (.E01) yoludur.
    pub fn open(path: &Path) -> Result<Self> {
        let mut segments = Vec::new();
        let mut segment_number = 1u16;

        // Segment dosyalarını sırayla aç
        loop {
            let seg_path = super::writer::segment_path(path, segment_number);
            if !seg_path.exists() {
                break;
            }
            let file = File::open(&seg_path).map_err(|e| {
                DelilXError::io("E01 segment dosyası açılamadı", Some(seg_path.clone()), e)
            })?;
            let size = file
                .metadata()
                .map_err(|e| {
                    DelilXError::io("Segment boyutu okunamadı", Some(seg_path.clone()), e)
                })?
                .len();
            segments.push(SegmentFile {
                path: seg_path.clone(),
                file,
                size,
                info: EwfSegmentInfo {
                    path: seg_path.to_string_lossy().to_string(),
                    segment_number,
                    size_bytes: size,
                    sections: Vec::new(),
                },
            });
            segment_number += 1;
            if segment_number > 10_000 {
                break; // güvenlik sınırı
            }
        }

        if segments.is_empty() {
            return Err(DelilXError::ewf(
                "İmaj açılamadı",
                format!("Segment dosyası bulunamadı: {}", path.display()),
            ));
        }

        let mut chunks = Vec::new();
        let mut volume: Option<EwfVolumeInfo> = None;
        let mut md5 = None;
        let mut sha1 = None;
        let mut header2_fields = Vec::new();
        let mut header_text = None;
        let mut section_types = Vec::new();

        for (seg_idx, seg) in segments.iter_mut().enumerate() {
            let mut current_sectors_body_end: Option<u64> = None;
            // Dosya başlığı doğrulaması
            let mut fh = [0u8; FILE_HEADER_SIZE as usize];
            read_exact_at(&mut seg.file, 0, &mut fh)?;
            if fh[0..8] != EWF_SIGNATURE {
                return Err(DelilXError::ewf(
                    "Geçersiz E01 imzası",
                    format!("{} dosyası E01 imzası içermiyor.", seg.path.display()),
                ));
            }

            // Bölümleri dolaş
            let mut offset = FILE_HEADER_SIZE;
            while offset + SECTION_DESCRIPTOR_SIZE <= seg.size {
                let mut desc = [0u8; SECTION_DESCRIPTOR_SIZE as usize];
                read_exact_at(&mut seg.file, offset, &mut desc)?;

                let section_type = SectionType::from_field(&desc[0..16]);
                if section_type == SectionType::Unknown {
                    // Bilinmeyen bölüm; dolgu olabilir. Kalanı atla.
                    break;
                }
                let next_offset = u64::from_le_bytes(desc[16..24].try_into().unwrap());
                let size = u64::from_le_bytes(desc[24..32].try_into().unwrap());

                // Sağlama doğrulaması
                let stored_checksum = u32::from_le_bytes(desc[72..76].try_into().unwrap());
                let calc_checksum = adler32(&desc[0..72]);
                if stored_checksum != calc_checksum {
                    return Err(DelilXError::ewf(
                        "Bölüm tanımlayıcı sağlaması hatalı",
                        format!(
                            "{} içinde {} ofsetindeki {:?} bölümü bozuk.",
                            seg.path.display(),
                            offset,
                            section_type
                        ),
                    ));
                }

                let name = section_type.as_name().to_string();
                if !section_types.contains(&name) {
                    section_types.push(name.clone());
                }
                seg.info.sections.push(name);

                let body_offset = offset + SECTION_DESCRIPTOR_SIZE;
                let body_len = size.saturating_sub(SECTION_DESCRIPTOR_SIZE);

                match section_type {
                    SectionType::Volume | SectionType::Data => {
                        if volume.is_none() {
                            let mut body = vec![0u8; body_len as usize];
                            read_exact_at(&mut seg.file, body_offset, &mut body)?;
                            volume = Some(parse_volume(&body, section_type)?);
                        }
                    }
                    SectionType::Sectors => {
                        // Blok verisi bölümü. Son blok boyutunu hesaplamak için
                        // bu bölümün gövde bitiş ofsetini saklarız.
                        current_sectors_body_end = Some(body_offset + body_len);
                    }
                    SectionType::Table | SectionType::Table2 => {
                        // Yalnızca ilk tabloyu kullan (table2 kopyadır)
                        let mut body = vec![0u8; body_len as usize];
                        read_exact_at(&mut seg.file, body_offset, &mut body)?;
                        // Table2'yi atla (zaten table işlendi)
                        if section_type == SectionType::Table {
                            let sectors_end = current_sectors_body_end.ok_or_else(|| {
                                DelilXError::ewf(
                                    "Blok tablosu eksik veri bölümü",
                                    "Tablo bölümünden önce sectors bölümü bulunamadı.".to_string(),
                                )
                            })?;
                            let new_chunks = parse_table(&body, seg_idx, sectors_end)?;
                            chunks.extend(new_chunks);
                        }
                    }
                    SectionType::Digest => {
                        let mut body = vec![0u8; body_len as usize];
                        read_exact_at(&mut seg.file, body_offset, &mut body)?;
                        if body.len() >= 36 {
                            md5 = Some(hex::encode(&body[0..16]));
                            sha1 = Some(hex::encode(&body[16..36]));
                        }
                    }
                    SectionType::Hash => {
                        let mut body = vec![0u8; body_len as usize];
                        read_exact_at(&mut seg.file, body_offset, &mut body)?;
                        if body.len() >= 16 && md5.is_none() {
                            md5 = Some(hex::encode(&body[0..16]));
                        }
                    }
                    SectionType::Header2 => {
                        let mut body = vec![0u8; body_len as usize];
                        read_exact_at(&mut seg.file, body_offset, &mut body)?;
                        if let Ok(text) = decompress_string(&body) {
                            if header2_fields.is_empty() {
                                header2_fields = parse_header2(&text);
                            }
                        }
                    }
                    SectionType::Header => {
                        let mut body = vec![0u8; body_len as usize];
                        read_exact_at(&mut seg.file, body_offset, &mut body)?;
                        if let Ok(text) = decompress_string(&body) {
                            if header_text.is_none() {
                                header_text = Some(text);
                            }
                        }
                    }
                    SectionType::Next => break,
                    SectionType::Done => break,
                    SectionType::Unknown => break,
                }

                if next_offset == 0 || next_offset <= offset {
                    break;
                }
                // next_offset dosyadaki mutlak bir sonraki bölüm ofsetidir.
                offset = next_offset;
            }
        }

        let volume = volume.ok_or_else(|| {
            DelilXError::ewf("Birim bilgisi bulunamadı", "volume/data bölümü eksik.")
        })?;

        let mut image = EwfImage {
            segments,
            chunks,
            volume,
            md5,
            sha1,
            header2_fields,
            header_text,
            info: EwfImageInfo {
                segments: Vec::new(),
                volume: EwfVolumeInfo {
                    media_type: 0,
                    media_type_text: String::new(),
                    number_of_chunks: 0,
                    sectors_per_chunk: 0,
                    bytes_per_sector: 512,
                    number_of_sectors: 0,
                    total_bytes: 0,
                    media_flags: 0,
                    is_physical: false,
                    compression_level: 0,
                    error_granularity: 0,
                },
                md5: None,
                sha1: None,
                header2_fields: Vec::new(),
                header_text: None,
                section_types: Vec::new(),
            },
        };

        // info'yu doldur
        image.info.segments = image.segments.iter().map(|s| s.info.clone()).collect();
        image.info.volume = image.volume.clone();
        image.info.md5 = image.md5.clone();
        image.info.sha1 = image.sha1.clone();
        image.info.header2_fields = image.header2_fields.clone();
        image.info.header_text = image.header_text.clone();
        image.info.section_types = section_types;

        Ok(image)
    }

    /// İmaj özet bilgisini döndürür.
    pub fn info(&self) -> &EwfImageInfo {
        &self.info
    }

    /// Birim bilgisi.
    pub fn volume(&self) -> &EwfVolumeInfo {
        &self.volume
    }

    /// İmaj içindeki MD5.
    pub fn md5(&self) -> Option<&str> {
        self.md5.as_deref()
    }

    /// İmaj içindeki SHA-1.
    pub fn sha1(&self) -> Option<&str> {
        self.sha1.as_deref()
    }

    /// Blok sayısı.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// header2 alanlarını döndürür.
    pub fn header2_fields(&self) -> &[(String, String)] {
        &self.header2_fields
    }

    /// Bir bloğu çözerek ham verisini döndürür.
    pub fn read_chunk(&self, index: usize) -> Result<Vec<u8>> {
        let chunk = self
            .chunks
            .get(index)
            .ok_or_else(|| DelilXError::other(format!("Blok bulunamadı: {index}")))?;
        let seg = &self.segments[chunk.segment_index];

        let mut data = vec![0u8; chunk.size as usize];
        let mut file = &seg.file;
        file.seek(SeekFrom::Start(chunk.offset))
            .map_err(|e| DelilXError::io("İmaj okunamadı", Some(seg.path.clone()), e))?;
        file.read_exact(&mut data)
            .map_err(|e| DelilXError::io("İmaj okunamadı", Some(seg.path.clone()), e))?;

        if chunk.compressed {
            let mut decoder = flate2::read::ZlibDecoder::new(&data[..]);
            let mut out = Vec::with_capacity(
                self.volume.bytes_per_sector as usize * self.volume.sectors_per_chunk as usize,
            );
            decoder
                .read_to_end(&mut out)
                .map_err(|e| DelilXError::ewf("Blok çözülemedi", format!("{e}")))?;
            Ok(out)
        } else {
            // Ham blok: son 4 bayt adler32
            if data.len() < 4 {
                return Ok(data);
            }
            let raw_len = data.len() - 4;
            let stored = u32::from_le_bytes(data[raw_len..].try_into().unwrap());
            let calc = adler32(&data[..raw_len]);
            if stored != calc {
                return Err(DelilXError::ewf(
                    "Blok sağlaması hatalı",
                    format!("Blok {index} adler32 sağlaması uyuşmuyor."),
                ));
            }
            data.truncate(raw_len);
            Ok(data)
        }
    }

    /// İmajdan belirtilen ofsetten istenen uzunlukta veri okur.
    pub fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let chunk_size =
            (self.volume.sectors_per_chunk as u64) * (self.volume.bytes_per_sector as u64);
        if chunk_size == 0 {
            return Ok(0);
        }
        let total = self.volume.total_bytes;
        if offset >= total {
            return Ok(0);
        }
        let mut written = 0usize;
        let mut pos = offset;
        while written < buf.len() && pos < total {
            let index = (pos / chunk_size) as usize;
            let within = (pos % chunk_size) as usize;
            let chunk = self.read_chunk(index)?;
            let avail = chunk.len().saturating_sub(within);
            if avail == 0 {
                break;
            }
            let to_copy = avail.min(buf.len() - written);
            buf[written..written + to_copy].copy_from_slice(&chunk[within..within + to_copy]);
            written += to_copy;
            pos += to_copy as u64;
        }
        Ok(written)
    }
}

/// Dosyadan belirtilen ofsetten tam `buf.len()` bayt okur.
fn read_exact_at(file: &mut File, offset: u64, buf: &mut [u8]) -> Result<()> {
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| DelilXError::io("İmaj konumlanamadı", None, e))?;
    file.read_exact(buf)
        .map_err(|e| DelilXError::io("İmaj okunamadı", None, e))
}

/// Volume/Data bölümü gövdesini çözümler.
fn parse_volume(body: &[u8], section_type: SectionType) -> Result<EwfVolumeInfo> {
    if body.len() < 1048 {
        return Err(DelilXError::ewf(
            "Birim bölümü kısa",
            format!(
                "{:?} bölümü {} bayt (en az 1048 gerekli).",
                section_type,
                body.len()
            ),
        ));
    }
    // Sağlama doğrulaması
    let stored = u32::from_le_bytes(body[1048..1052].try_into().unwrap());
    let calc = adler32(&body[0..1048]);
    if stored != calc {
        return Err(DelilXError::ewf(
            "Birim bölümü sağlaması hatalı",
            "volume/data bölümü bozuk.".to_string(),
        ));
    }

    let media_type = body[0];
    let number_of_chunks = u32::from_le_bytes(body[4..8].try_into().unwrap());
    let sectors_per_chunk = u32::from_le_bytes(body[8..12].try_into().unwrap());
    let bytes_per_sector = u32::from_le_bytes(body[12..16].try_into().unwrap());
    let number_of_sectors = u64::from_le_bytes(body[16..24].try_into().unwrap());
    let media_flags = body[36];
    let compression_level = body[52];
    let error_granularity = u32::from_le_bytes(body[56..60].try_into().unwrap());

    let media_type_text = match media_type {
        0x00 => "Çıkarılabilir disk",
        0x01 => "Sabit disk",
        0x03 => "Optik disk",
        0x0e => "Tek dosyalar",
        0x10 => "Bellek",
        _ => "Bilinmiyor",
    }
    .to_string();

    let is_physical = media_flags & super::media_flags::PHYSICAL != 0;

    Ok(EwfVolumeInfo {
        media_type,
        media_type_text,
        number_of_chunks,
        sectors_per_chunk,
        bytes_per_sector,
        number_of_sectors,
        total_bytes: number_of_sectors * bytes_per_sector as u64,
        media_flags,
        is_physical,
        compression_level,
        error_granularity,
    })
}

/// Blok tablosunu çözümler ve blok referanslarını üretir.
fn parse_table(body: &[u8], segment_index: usize, sectors_body_end: u64) -> Result<Vec<ChunkRef>> {
    if body.len() < 28 {
        return Err(DelilXError::ewf(
            "Blok tablosu kısa",
            format!("Tablo bölümü {} bayt.", body.len()),
        ));
    }
    let number_of_entries = u32::from_le_bytes(body[0..4].try_into().unwrap());
    let base_offset = u64::from_le_bytes(body[8..16].try_into().unwrap());

    // Başlık sağlaması
    let stored_header = u32::from_le_bytes(body[20..24].try_into().unwrap());
    let calc_header = adler32(&body[0..20]);
    if stored_header != calc_header {
        return Err(DelilXError::ewf(
            "Blok tablosu başlık sağlaması hatalı",
            "Tablo bozuk.".to_string(),
        ));
    }

    let entries_start = 24usize;
    let needed = entries_start + 4 * number_of_entries as usize + 4;
    if body.len() < needed {
        return Err(DelilXError::ewf(
            "Blok tablosu eksik giriş",
            format!("Beklenen {needed} bayt, bulunan {}.", body.len()),
        ));
    }

    // Giriş sağlaması
    let stored_entries = u32::from_le_bytes(body[needed - 4..needed].try_into().unwrap());
    let calc_entries = adler32(&body[entries_start..needed - 4]);
    if stored_entries != calc_entries {
        return Err(DelilXError::ewf(
            "Blok tablosu giriş sağlaması hatalı",
            "Tablo girişleri bozuk.".to_string(),
        ));
    }

    let mut offsets = Vec::with_capacity(number_of_entries as usize);
    let mut compressed_flags = Vec::with_capacity(number_of_entries as usize);
    for i in 0..number_of_entries as usize {
        let v = u32::from_le_bytes(
            body[entries_start + 4 * i..entries_start + 4 * i + 4]
                .try_into()
                .unwrap(),
        );
        offsets.push(base_offset + (v & TABLE_ENTRY_OFFSET_MASK) as u64);
        compressed_flags.push(v & TABLE_ENTRY_COMPRESSED != 0);
    }

    // Blok boyutları bir sonraki ofsetten hesaplanır
    let mut refs = Vec::with_capacity(offsets.len());
    for i in 0..offsets.len() {
        let start = offsets[i];
        let end = if i + 1 < offsets.len() {
            offsets[i + 1]
        } else {
            sectors_body_end
        };
        let size = if end > start { (end - start) as u32 } else { 0 };
        refs.push(ChunkRef {
            segment_index,
            offset: start,
            size,
            compressed: compressed_flags[i],
        });
    }
    Ok(refs)
}

/// Sıkıştırılmış metin bölümünü açar.
fn decompress_string(body: &[u8]) -> Result<String> {
    let mut decoder = flate2::read::ZlibDecoder::new(body);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| DelilXError::ewf("Metin çözülemedi", format!("{e}")))?;
    // UTF-16LE BOM varsa
    if out.len() >= 2 && out[0] == 0xFF && out[1] == 0xFE {
        let u16s: Vec<u16> = out[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(String::from_utf16_lossy(&u16s))
    } else {
        Ok(String::from_utf8_lossy(&out).to_string())
    }
}

/// header2 metnini anahtar-değer çiftlerine ayırır.
fn parse_header2(text: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.len() >= 4 {
        let keys: Vec<&str> = lines[2].split('\t').collect();
        let values: Vec<&str> = lines[3].split('\t').collect();
        let key_names = [
            "a", "c", "n", "e", "t", "md", "sn", "av", "ov", "m", "u", "p", "dc",
        ];
        let tr = [
            "Açıklama",
            "Vaka Numarası",
            "Delil Numarası",
            "İnceleme Uzmanı",
            "Notlar",
            "Model",
            "Seri Numarası",
            "Yazılım Sürümü",
            "İşletim Sistemi",
            "Edinim Tarihi (epoch)",
            "Sistem Tarihi (epoch)",
            "Şifre",
            "Bilinmeyen",
        ];
        for (i, key) in keys.iter().enumerate() {
            let tr_name = key_names
                .iter()
                .position(|k| k == key)
                .map(|idx| tr[idx].to_string())
                .unwrap_or_else(|| (*key).to_string());
            let value = values.get(i).copied().unwrap_or("").to_string();
            fields.push((tr_name, value));
        }
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_header2_fields() {
        let text = "3\nmain\na\tc\tn\te\tt\naciklama\tVAKA-1\tDELIL-1\tuzman\t\n";
        let fields = parse_header2(text);
        assert!(fields.iter().any(|(k, _)| k == "Vaka Numarası"));
        assert!(fields
            .iter()
            .any(|(k, v)| k == "Delil Numarası" && v == "DELIL-1"));
    }
}
