//! E01 (EWF1) imaj yazıcısı.
//!
//! Gerçek EnCase 6 uyumlu E01 dosyaları üretir. Format ayrıntıları libewf
//! kaynak koduna ve referans dosyalarla yapılan bayt düzeyi karşılaştırmaya
//! dayanır. Üretilen dosyalar `ewfinfo` ve `ewfverify` ile doğrulanır.

use super::{
    adler32, compression_level, media_flags, EwfMediaType, SectionType, EWF_SIGNATURE,
    SECTION_DESCRIPTOR_SIZE, TABLE_ENTRY_COMPRESSED,
};
use crate::error::{DelilXError, Result};
use crate::hash::{HashAlgorithm, HashSet};
use crate::progress::{CancelToken, OperationLog, ProgressCallback, ProgressTracker};
use crate::source::SourceReader;
use digest::Digest;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// E01 medya türü seçimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaType {
    /// Sabit disk.
    Fixed,
    /// Çıkarılabilir aygıt (USB, SD).
    Removable,
}

impl MediaType {
    fn to_ewf(self) -> u8 {
        match self {
            MediaType::Fixed => EwfMediaType::Fixed as u8,
            MediaType::Removable => EwfMediaType::Removable as u8,
        }
    }
}

/// E01 sıkıştırma seçimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionMode {
    /// Sıkıştırma yok.
    None,
    /// Hızlı sıkıştırma.
    Fast,
    /// En iyi sıkıştırma.
    Best,
}

impl CompressionMode {
    fn level(self) -> u8 {
        match self {
            CompressionMode::None => compression_level::NONE,
            CompressionMode::Fast => compression_level::FAST,
            CompressionMode::Best => compression_level::BEST,
        }
    }

    fn flate2_level(self) -> Compression {
        match self {
            CompressionMode::None => Compression::none(),
            CompressionMode::Fast => Compression::fast(),
            CompressionMode::Best => Compression::best(),
        }
    }

    /// Türkçe etiket.
    pub fn label(self) -> &'static str {
        match self {
            CompressionMode::None => "Sıkıştırma yok",
            CompressionMode::Fast => "Hızlı sıkıştırma",
            CompressionMode::Best => "En iyi sıkıştırma",
        }
    }
}

/// E01 edinim delil ve uzman bilgileri.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EwfCaseInfo {
    /// Delil numarası.
    pub evidence_number: String,
    /// Vaka/olay numarası.
    pub case_number: String,
    /// Açıklama.
    pub description: String,
    /// İncelemeyi yapan uzman.
    pub examiner_name: String,
    /// Notlar.
    pub notes: String,
    /// Cihaz üretici/model bilgisi.
    pub model: String,
    /// Cihaz seri numarası.
    pub serial_number: String,
    /// Birim etiketi.
    pub device_label: String,
}

impl Default for EwfCaseInfo {
    fn default() -> Self {
        EwfCaseInfo {
            evidence_number: "DELIL-1".to_string(),
            case_number: "VAKA-1".to_string(),
            description: "DelilX edinimi".to_string(),
            examiner_name: String::new(),
            notes: String::new(),
            model: String::new(),
            serial_number: String::new(),
            device_label: String::new(),
        }
    }
}

/// E01 yazıcı ayarları.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EwfWriterConfig {
    /// Hedef ilk segment yolu (ör. `cikti.E01`).
    pub target_path: PathBuf,
    /// Delil ve uzman bilgileri.
    pub case_info: EwfCaseInfo,
    /// Medya türü.
    pub media_type: MediaType,
    /// Fiziksel aygıt görüntüsü mü?
    pub is_physical: bool,
    /// Sektör başına bayt.
    pub bytes_per_sector: u32,
    /// Blok (chunk) başına sektör sayısı.
    pub sectors_per_chunk: u32,
    /// Sıkıştırma modu.
    pub compression: CompressionMode,
    /// Segment dosyası başına en fazla bayt (yaklaşık).
    pub segment_size_bytes: u64,
    /// Hata granülaritesi (sektör).
    pub error_granularity: u32,
    /// Rapor için ayrıca hesaplanacak hash algoritmaları. MD5 ve SHA-1 her
    /// durumda hesaplanır ve E01 biçiminde saklanır.
    pub extra_hashes: Vec<HashAlgorithm>,
}

impl EwfWriterConfig {
    /// Varsayılan ayarlarla yapılandırma oluşturur.
    pub fn new(target: impl Into<PathBuf>) -> Self {
        EwfWriterConfig {
            target_path: target.into(),
            case_info: EwfCaseInfo::default(),
            media_type: MediaType::Fixed,
            is_physical: false,
            bytes_per_sector: 512,
            sectors_per_chunk: 64,
            compression: CompressionMode::Best,
            segment_size_bytes: 1_400 * 1024 * 1024,
            error_granularity: 64,
            extra_hashes: vec![HashAlgorithm::Sha256, HashAlgorithm::Sha512],
        }
    }
}

/// E01 edinim sonucu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EwfWriterResult {
    /// Oluşturulan segment dosyaları.
    pub segment_files: Vec<String>,
    /// Toplam bayt.
    pub total_bytes: u64,
    /// Toplam sektör.
    pub total_sectors: u64,
    /// Toplam blok.
    pub total_chunks: u64,
    /// MD5 (E01 içine yazıldı).
    pub md5: String,
    /// SHA-1 (E01 içine yazıldı).
    pub sha1: String,
    /// Ek hash değerleri (rapor için).
    pub extra_hashes: HashSet,
    /// Başlangıç zamanı.
    pub started_at: String,
    /// Bitiş zamanı.
    pub finished_at: String,
    /// Süre (saniye).
    pub duration_seconds: f64,
    /// İşlem kayıtları.
    pub log: OperationLog,
}

/// Blok tablosu girişi (4 bayt): ofset + sıkıştırma bayrağı.
#[derive(Debug, Clone, Copy)]
struct TableEntry {
    value: u32,
}

/// Segment dosyasına yazan basit tamponlu yazıcı. Yamalama (patch) destekler.
struct SegmentWriter {
    file: File,
    buf: Vec<u8>,
    pos: u64,
}

impl SegmentWriter {
    fn create(path: &Path) -> Result<Self> {
        let file = File::create(path).map_err(|e| {
            DelilXError::io(
                "E01 segment dosyası oluşturulamadı",
                Some(path.to_path_buf()),
                e,
            )
        })?;
        Ok(SegmentWriter {
            file,
            buf: Vec::with_capacity(4 * 1024 * 1024),
            pos: 0,
        })
    }

    #[allow(dead_code)]
    fn open_rw(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map_err(|e| {
                DelilXError::io("E01 segment dosyası açılamadı", Some(path.to_path_buf()), e)
            })?;
        Ok(SegmentWriter {
            file,
            buf: Vec::with_capacity(4 * 1024 * 1024),
            pos: 0,
        })
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.buf.extend_from_slice(data);
        self.pos += data.len() as u64;
        if self.buf.len() >= 4 * 1024 * 1024 {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        if !self.buf.is_empty() {
            self.file
                .write_all(&self.buf)
                .map_err(|e| DelilXError::io("E01 yazma hatası", None, e))?;
            self.buf.clear();
        }
        self.file
            .flush()
            .map_err(|e| DelilXError::io("E01 tamponu boşaltılamadı", None, e))
    }

    fn pos(&self) -> u64 {
        self.pos
    }

    /// Belirtilen mutlak ofsete veri yazar ve yazma konumunu sona döndürür.
    fn patch(&mut self, offset: u64, data: &[u8]) -> Result<()> {
        self.flush()?;
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| DelilXError::io("E01 konumlanamadı", None, e))?;
        self.file
            .write_all(data)
            .map_err(|e| DelilXError::io("E01 yaması yazılamadı", None, e))?;
        self.file
            .seek(SeekFrom::Start(self.pos))
            .map_err(|e| DelilXError::io("E01 konumlanamadı", None, e))?;
        Ok(())
    }
}

/// E01 yazıcı.
pub struct EwfWriter {
    config: EwfWriterConfig,
    writer: SegmentWriter,
    segment_number: u16,
    segment_bytes: u64,
    total_bytes: u64,
    chunks_written: u64,
    predicted_sectors: u64,
    predicted_chunks: u64,
    started_at: String,
    chunk_size: u32,
    sectors_section_offset: u64,
    table_entries: Vec<TableEntry>,
    section_data_bytes: u64,
    md5: md5::Md5,
    sha1: sha1::Sha1,
    extra_hasher: crate::hash::MultiHasher,
    segment_files: Vec<PathBuf>,
    log: OperationLog,
}

impl EwfWriter {
    /// Yeni E01 yazıcı oluşturur ve ilk segment dosyasını açar.
    pub fn create(config: EwfWriterConfig, total_bytes: Option<u64>) -> Result<Self> {
        let chunk_size = config
            .sectors_per_chunk
            .checked_mul(config.bytes_per_sector)
            .ok_or_else(|| DelilXError::other("Blok boyutu hesaplanamadı (taşma)."))?;
        if chunk_size == 0 {
            return Err(DelilXError::other("Blok boyutu sıfır olamaz."));
        }

        let predicted_sectors = total_bytes
            .map(|t| t.div_ceil(config.bytes_per_sector as u64))
            .unwrap_or(0);
        let predicted_chunks = total_bytes
            .map(|t| t.div_ceil(chunk_size as u64))
            .unwrap_or(0);

        let mut writer = EwfWriter {
            config: config.clone(),
            writer: SegmentWriter::create(&config.target_path)?,
            segment_number: 1,
            segment_bytes: 0,
            total_bytes: 0,
            chunks_written: 0,
            predicted_sectors,
            predicted_chunks,
            started_at: crate::timeutil::now_iso8601(),
            chunk_size,
            sectors_section_offset: 0,
            table_entries: Vec::new(),
            section_data_bytes: 0,
            md5: md5::Md5::new(),
            sha1: sha1::Sha1::new(),
            extra_hasher: crate::hash::MultiHasher::new(&config.extra_hashes),
            segment_files: vec![config.target_path.clone()],
            log: OperationLog::new(),
        };
        writer.write_segment_header()?;
        Ok(writer)
    }

    fn write_segment_header(&mut self) -> Result<()> {
        let mut hdr = Vec::with_capacity(13);
        hdr.extend_from_slice(&EWF_SIGNATURE);
        hdr.push(0x01);
        hdr.extend_from_slice(&self.segment_number.to_le_bytes());
        hdr.extend_from_slice(&[0x00, 0x00]);
        self.writer.write(&hdr)?;
        self.segment_bytes += hdr.len() as u64;
        Ok(())
    }

    /// Bölüm tanımlayıcı yazar. `next_offset` bu bölümün başına göre DEĞİL,
    /// dosyadaki mutlak bir sonraki bölüm ofsetidir (libewf/EnCase kuralı).
    fn write_section_descriptor(&mut self, section_type: SectionType, size: u64) -> Result<()> {
        let next_offset = self.writer.pos() + size;
        let desc = build_descriptor(section_type, next_offset, size);
        self.writer.write(&desc)?;
        self.segment_bytes += desc.len() as u64;
        Ok(())
    }

    fn write_compressed_string_section(
        &mut self,
        section_type: SectionType,
        data: &[u8],
    ) -> Result<()> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(data)
            .map_err(|e| DelilXError::other(format!("Metin sıkıştırılamadı: {e}")))?;
        let compressed = encoder
            .finish()
            .map_err(|e| DelilXError::other(format!("Metin sıkıştırılamadı: {e}")))?;

        let size = SECTION_DESCRIPTOR_SIZE + compressed.len() as u64;
        self.write_section_descriptor(section_type, size)?;
        self.writer.write(&compressed)?;
        self.segment_bytes += compressed.len() as u64;
        Ok(())
    }

    fn build_volume_body(&self) -> Vec<u8> {
        let mut body = vec![0u8; 1048];
        body[0] = self.config.media_type.to_ewf();
        body[4..8].copy_from_slice(&(self.predicted_chunks as u32).to_le_bytes());
        body[8..12].copy_from_slice(&self.config.sectors_per_chunk.to_le_bytes());
        body[12..16].copy_from_slice(&self.config.bytes_per_sector.to_le_bytes());
        body[16..24].copy_from_slice(&self.predicted_sectors.to_le_bytes());
        let mut media_flags = 0x01u8;
        if self.config.is_physical {
            media_flags |= media_flags::PHYSICAL;
        }
        body[36] = media_flags;
        body[52] = self.config.compression.level();
        body[56..60].copy_from_slice(&self.config.error_granularity.to_le_bytes());
        let checksum = adler32(&body[0..1048]);
        body.extend_from_slice(&checksum.to_le_bytes());
        body
    }

    fn write_volume_section(&mut self, section_type: SectionType) -> Result<()> {
        let body = self.build_volume_body();
        let size = SECTION_DESCRIPTOR_SIZE + body.len() as u64;
        self.write_section_descriptor(section_type, size)?;
        self.writer.write(&body)?;
        self.segment_bytes += body.len() as u64;
        Ok(())
    }

    fn build_header2(&self) -> Vec<u8> {
        let now = crate::timeutil::now_epoch();
        let c = &self.config.case_info;
        let ver = env!("CARGO_PKG_VERSION");
        let mut s = String::new();
        s.push_str("3\n");
        s.push_str("main\n");
        s.push_str("a\tc\tn\te\tt\tmd\tsn\tav\tov\tm\tu\tp\tdc\n");
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\tWindows\t{}\t{}\t0\t\n",
            c.description,
            c.case_number,
            c.evidence_number,
            c.examiner_name,
            c.notes,
            c.model,
            c.serial_number,
            ver,
            now,
            now
        ));
        s.push('\n');
        s.push_str("srce\n");
        s.push_str("0\t1\n");
        s.push_str("p\tn\tid\tev\ttb\tlo\tpo\tah\tgu\taq\n");
        s.push_str("0\t0\n");
        s.push_str("\t\t\t\t\t-1\t-1\t\t\t\n");
        s.push('\n');
        s.push_str("sub\n");
        s.push_str("0\t1\n");
        s.push_str("p\tn\tid\tnu\tco\tgu\n");
        s.push_str("0\t0\n");
        s.push_str("\t\t\t\t1\t\n");
        s.push('\n');
        s.push('\n');

        let mut out = Vec::with_capacity(s.len() * 2 + 2);
        out.extend_from_slice(&[0xFF, 0xFE]);
        for ch in s.encode_utf16() {
            out.extend_from_slice(&ch.to_le_bytes());
        }
        out
    }

    fn build_header(&self) -> Vec<u8> {
        let now = crate::timeutil::now_readable();
        let c = &self.config.case_info;
        let ver = env!("CARGO_PKG_VERSION");
        let s = format!(
            "1\r\nmain\r\nc\tn\ta\te\tt\tav\tov\tm\tu\tp\r\n\
             {}\t{}\t{}\t{}\t{}\t{}\tWindows\t{}\t{}\t0\r\n\r\n",
            c.case_number,
            c.evidence_number,
            c.description,
            c.examiner_name,
            c.notes,
            ver,
            now,
            now
        );
        s.into_bytes()
    }

    fn write_initial_headers(&mut self) -> Result<()> {
        let h2 = self.build_header2();
        self.write_compressed_string_section(SectionType::Header2, &h2)?;
        self.write_compressed_string_section(SectionType::Header2, &h2)?;
        let h = self.build_header();
        self.write_compressed_string_section(SectionType::Header, &h)?;
        Ok(())
    }

    fn open_chunk_section(&mut self) -> Result<()> {
        self.sectors_section_offset = self.writer.pos();
        self.write_section_descriptor(SectionType::Sectors, 0)?;
        self.section_data_bytes = SECTION_DESCRIPTOR_SIZE;
        self.table_entries.clear();
        Ok(())
    }

    fn finalize_chunk_section(&mut self) -> Result<()> {
        let size = self.writer.pos() - self.sectors_section_offset;
        let next_offset = self.sectors_section_offset + size;
        let desc = build_descriptor(SectionType::Sectors, next_offset, size);
        self.writer.patch(self.sectors_section_offset, &desc)?;
        Ok(())
    }

    fn close_chunk_section(&mut self) -> Result<()> {
        let base_offset = self.sectors_section_offset;
        let number_of_entries = self.table_entries.len() as u32;
        let table_body_len = 24 + 4 * number_of_entries as usize + 4;

        for section_type in [SectionType::Table, SectionType::Table2] {
            let mut body = Vec::with_capacity(table_body_len);
            body.extend_from_slice(&number_of_entries.to_le_bytes());
            body.extend_from_slice(&[0u8; 4]);
            body.extend_from_slice(&base_offset.to_le_bytes());
            body.extend_from_slice(&[0u8; 4]);
            let header_checksum = adler32(&body[0..20]);
            body.extend_from_slice(&header_checksum.to_le_bytes());
            for e in &self.table_entries {
                body.extend_from_slice(&e.value.to_le_bytes());
            }
            let entries_checksum = adler32(&body[24..body.len()]);
            body.extend_from_slice(&entries_checksum.to_le_bytes());

            let size = SECTION_DESCRIPTOR_SIZE + body.len() as u64;
            self.write_section_descriptor(section_type, size)?;
            self.writer.write(&body)?;
            self.segment_bytes += body.len() as u64;
        }
        Ok(())
    }

    fn pack_chunk(&self, data: &[u8]) -> Result<(Vec<u8>, TableEntry)> {
        let relative_offset = self.section_data_bytes as u32;
        let mut value = relative_offset;

        if self.config.compression == CompressionMode::None {
            let mut out = Vec::with_capacity(data.len() + 4);
            out.extend_from_slice(data);
            out.extend_from_slice(&adler32(data).to_le_bytes());
            return Ok((out, TableEntry { value }));
        }

        let mut encoder = ZlibEncoder::new(Vec::new(), self.config.compression.flate2_level());
        encoder
            .write_all(data)
            .map_err(|e| DelilXError::other(format!("Blok sıkıştırılamadı: {e}")))?;
        let compressed = encoder
            .finish()
            .map_err(|e| DelilXError::other(format!("Blok sıkıştırılamadı: {e}")))?;

        if compressed.len() < data.len() {
            value |= TABLE_ENTRY_COMPRESSED;
            Ok((compressed, TableEntry { value }))
        } else {
            let mut out = Vec::with_capacity(data.len() + 4);
            out.extend_from_slice(data);
            out.extend_from_slice(&adler32(data).to_le_bytes());
            Ok((out, TableEntry { value }))
        }
    }

    fn write_chunk(&mut self, data: &[u8]) -> Result<()> {
        self.md5.update(data);
        self.sha1.update(data);
        self.extra_hasher.update(data);

        let (packed, entry) = self.pack_chunk(data)?;
        self.writer.write(&packed)?;
        self.segment_bytes += packed.len() as u64;
        self.section_data_bytes += packed.len() as u64;
        self.table_entries.push(entry);
        Ok(())
    }

    fn start_new_segment(&mut self) -> Result<()> {
        self.finalize_chunk_section()?;
        self.close_chunk_section()?;
        self.write_section_descriptor(SectionType::Next, 0)?;
        self.writer.flush()?;

        self.segment_number += 1;
        let path = segment_path(&self.config.target_path, self.segment_number);
        self.writer = SegmentWriter::create(&path)?;
        self.segment_files.push(path);
        self.segment_bytes = 0;
        self.write_segment_header()?;
        self.write_volume_section(SectionType::Data)?;
        self.open_chunk_section()?;
        Ok(())
    }

    /// Sonlandırır: son blok bölümünü, digest/hash ve done bölümlerini yazar.
    pub fn finish(mut self) -> Result<EwfWriterResult> {
        self.finalize_chunk_section()?;
        self.close_chunk_section()?;

        let md5_val = self.md5.clone().finalize();
        let sha1_val = self.sha1.clone().finalize();

        {
            let mut body = vec![0u8; 80];
            body[0..16].copy_from_slice(&md5_val);
            body[16..36].copy_from_slice(&sha1_val);
            let checksum = adler32(&body[0..76]);
            body[76..80].copy_from_slice(&checksum.to_le_bytes());
            let size = SECTION_DESCRIPTOR_SIZE + body.len() as u64;
            self.write_section_descriptor(SectionType::Digest, size)?;
            self.writer.write(&body)?;
        }

        {
            let mut body = vec![0u8; 36];
            body[0..16].copy_from_slice(&md5_val);
            let checksum = adler32(&body[0..32]);
            body[32..36].copy_from_slice(&checksum.to_le_bytes());
            let size = SECTION_DESCRIPTOR_SIZE + body.len() as u64;
            self.write_section_descriptor(SectionType::Hash, size)?;
            self.writer.write(&body)?;
        }

        self.write_section_descriptor(SectionType::Done, 0)?;
        self.writer.flush()?;

        let extra_hashes = self.extra_hasher.finalize();

        Ok(EwfWriterResult {
            segment_files: self
                .segment_files
                .iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect(),
            total_bytes: self.total_bytes,
            total_sectors: self.total_bytes / self.config.bytes_per_sector as u64,
            total_chunks: self.chunks_written,
            md5: hex::encode(md5_val),
            sha1: hex::encode(sha1_val),
            extra_hashes,
            started_at: self.started_at.clone(),
            finished_at: crate::timeutil::now_iso8601(),
            duration_seconds: 0.0,
            log: self.log,
        })
    }

    /// Kaynaktan veri okuyarak E01 imajı oluşturur.
    pub fn acquire(
        config: EwfWriterConfig,
        source: &mut dyn SourceReader,
        progress: Option<ProgressCallback>,
        cancel: &CancelToken,
    ) -> Result<EwfWriterResult> {
        let start = std::time::Instant::now();
        let total = source.total_bytes();
        let mut w = EwfWriter::create(config, total)?;
        w.log.info("E01 edinimi başladı.");
        w.log.info(format!("Kaynak: {}", source.description()));
        w.log.info(format!(
            "Biçim: EnCase 6 (E01), sıkıştırma: {}",
            w.config.compression.label()
        ));

        w.write_initial_headers()?;
        w.write_volume_section(SectionType::Volume)?;
        w.open_chunk_section()?;

        let chunk_size = w.chunk_size as usize;
        let bps = w.config.bytes_per_sector as usize;
        let mut buf = vec![0u8; chunk_size];
        let tracker = ProgressTracker::new(total.unwrap_or(0));
        // Segment hedefi: en az 64 KiB, üstbilgi payı için %2 düşülür.
        let segment_budget = (w.config.segment_size_bytes * 98 / 100).max(64 * 1024);
        let mut chunks_in_segment: u64 = 0;

        let mut offset: u64 = 0;
        loop {
            cancel.check()?;
            if let Some(t) = total {
                if offset >= t {
                    break;
                }
            }

            let want = match total {
                Some(t) => ((t - offset) as usize).min(chunk_size),
                None => chunk_size,
            };
            let want_sectors = want.div_ceil(bps);
            let want_aligned = (want_sectors * bps).min(chunk_size);
            if want_aligned == 0 {
                break;
            }

            let n = source.read_at(offset, &mut buf[..want_aligned])?;
            if n == 0 {
                break;
            }
            if n < want_aligned {
                for b in buf[n..want_aligned].iter_mut() {
                    *b = 0;
                }
            }

            // Segment dolduysa (en az bir blok yazıldıysa) yeni segmente geç.
            let more = match total {
                Some(t) => offset < t,
                None => true,
            };
            if more && chunks_in_segment > 0 && w.segment_bytes >= segment_budget {
                w.start_new_segment()?;
                chunks_in_segment = 0;
            }

            w.write_chunk(&buf[..want_aligned])?;
            w.total_bytes += want_aligned as u64;
            w.chunks_written += 1;
            chunks_in_segment += 1;
            offset += want_aligned as u64;

            if let Some(cb) = &progress {
                if tracker.should_report() {
                    cb(tracker.progress(
                        w.total_bytes,
                        w.total_bytes / bps as u64,
                        "E01 imajı yazılıyor...",
                    ));
                }
            }
        }

        if let Some(cb) = &progress {
            cb(tracker.progress(
                w.total_bytes,
                w.total_bytes / bps as u64,
                "E01 imajı tamamlandı.",
            ));
        }

        w.log.info(format!(
            "E01 edinimi tamamlandı: {} blok, {} bayt.",
            w.chunks_written, w.total_bytes
        ));

        let mut result = w.finish()?;
        result.duration_seconds = start.elapsed().as_secs_f64();
        Ok(result)
    }
}

/// 76 baytlık bölüm tanımlayıcısı üretir.
fn build_descriptor(section_type: SectionType, next_offset: u64, size: u64) -> [u8; 76] {
    let mut desc = [0u8; SECTION_DESCRIPTOR_SIZE as usize];
    desc[0..16].copy_from_slice(&section_type.to_field());
    desc[16..24].copy_from_slice(&next_offset.to_le_bytes());
    desc[24..32].copy_from_slice(&size.to_le_bytes());
    let checksum = adler32(&desc[0..72]);
    desc[72..76].copy_from_slice(&checksum.to_le_bytes());
    desc
}

/// Segment dosyası adı üretir: `.E01`, `.E02`, ...
pub fn segment_path(base: &Path, segment: u16) -> PathBuf {
    let parent = base.parent().unwrap_or_else(|| Path::new("."));
    let stem = base
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "image".to_string());
    let name = format!("{stem}.E{segment:02}");
    parent.join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_path_generation() {
        let p = segment_path(Path::new("/tmp/out.E01"), 1);
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "out.E01");
        let p = segment_path(Path::new("/tmp/out.E01"), 3);
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "out.E03");
        let p = segment_path(Path::new("/tmp/out.E01"), 12);
        assert_eq!(p.file_name().unwrap().to_string_lossy(), "out.E12");
    }

    #[test]
    fn compression_labels() {
        assert_eq!(CompressionMode::Best.label(), "En iyi sıkıştırma");
        assert_eq!(CompressionMode::None.level(), 0);
        assert_eq!(CompressionMode::Fast.level(), 1);
        assert_eq!(CompressionMode::Best.level(), 2);
    }

    #[test]
    fn descriptor_checksum_is_adler32_of_first_72() {
        let d = build_descriptor(SectionType::Volume, 100, 200);
        let expected = adler32(&d[0..72]);
        assert_eq!(u32::from_le_bytes([d[72], d[73], d[74], d[75]]), expected);
    }
}
