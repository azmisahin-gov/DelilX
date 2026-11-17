//! DelilX komut satırı arayüzü.
//!
//! **Dijital Delili Koru, Doğrula ve İncele.**
//!
//! Bu araç, DelilX grafik arayüzünün yaptığı işlemleri komut satırından
//! yapmaya ve otomatik test/doğrulama amacıyla kullanılmaya olanak tanır.
//! Bütün çıktılar Türkçedir.

use clap::{Parser, Subcommand, ValueEnum};
use delilx_core::e01::writer::{
    CompressionMode, EwfCaseInfo, EwfWriter, EwfWriterConfig, MediaType,
};
use delilx_core::hash::HashAlgorithm;
use delilx_core::logical::{acquire_logical, LogicalConfig};
use delilx_core::progress::{format_bytes, format_duration, CancelToken, Progress};
use delilx_core::raw::{acquire_raw, RawAcquisitionConfig, ReadErrorPolicy};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "delilx",
    version,
    about = "DelilX — Adli bilişim veri edinim ve inceleme aracı",
    long_about = "DelilX komut satırı aracı.\n\nDijital Delili Koru, Doğrula ve İncele.\n\n\
                  Kaynak delil hiçbir koşulda değiştirilmez; yalnızca okunur."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, ValueEnum)]
enum CompressionArg {
    /// Sıkıştırma yok
    Yok,
    /// Hızlı sıkıştırma
    Hizli,
    /// En iyi sıkıştırma
    Eniyi,
}

#[derive(Clone, Copy, ValueEnum)]
enum MediaArg {
    /// Sabit disk
    Sabit,
    /// Çıkarılabilir (USB/SD)
    Cikarilabilir,
}

#[derive(Subcommand)]
enum Commands {
    /// Sürüm ve slogan bilgisini gösterir.
    Hakkinda,
    /// Sistemdeki depolama aygıtlarını listeler.
    Cihazlar,
    /// RAW/DD imaj oluşturur.
    Raw {
        /// Kaynak yol.
        #[arg(long)]
        kaynak: PathBuf,
        /// Hedef imaj dosyası.
        #[arg(long)]
        hedef: PathBuf,
        /// Sektör boyutu.
        #[arg(long, default_value_t = 512)]
        sektor: u32,
        /// Okuma hatası politikası: zerofill veya durdur.
        #[arg(long, default_value = "zerofill")]
        hata_politikasi: String,
        /// Doğrulama yapma.
        #[arg(long)]
        dogrulama_yok: bool,
    },
    /// E01 (EnCase) imaj oluşturur.
    E01 {
        /// Kaynak yol.
        #[arg(long)]
        kaynak: PathBuf,
        /// Hedef E01 dosyası (ör. cikti.E01).
        #[arg(long)]
        hedef: PathBuf,
        /// Delil numarası.
        #[arg(long, default_value = "DELIL-1")]
        delil_no: String,
        /// Vaka numarası.
        #[arg(long, default_value = "VAKA-1")]
        vaka_no: String,
        /// İncelemeyi yapan.
        #[arg(long, default_value = "")]
        uzman: String,
        /// Açıklama.
        #[arg(long, default_value = "DelilX edinimi")]
        aciklama: String,
        /// Sıkıştırma.
        #[arg(long, value_enum, default_value = "eniyi")]
        sikistirma: CompressionArg,
        /// Medya türü.
        #[arg(long, value_enum, default_value = "sabit")]
        medya: MediaArg,
        /// Segment boyutu (MiB).
        #[arg(long, default_value_t = 1400)]
        segment_mib: u64,
    },
    /// Mantıksal/seçmeli edinim yapar.
    Mantiksal {
        /// Kaynak dizin veya dosyalar (birden çok verilebilir).
        #[arg(long, num_args = 1..)]
        kaynak: Vec<PathBuf>,
        /// Hedef dizin.
        #[arg(long)]
        hedef: PathBuf,
        /// Yalnızca envanter çıkar, kopyalama.
        #[arg(long)]
        kopyalama_yok: bool,
    },
    /// İmajı inceler (RAW veya E01).
    Incele {
        /// İmaj yolu.
        #[arg(long)]
        imaj: PathBuf,
    },
    /// Hash doğrulaması yapar.
    Dogrula {
        /// İmaj/dosya yolu.
        #[arg(long)]
        imaj: PathBuf,
        /// Beklenen SHA-256.
        #[arg(long)]
        sha256: Option<String>,
        /// Beklenen SHA-512.
        #[arg(long)]
        sha512: Option<String>,
    },
}

fn progress_callback() -> delilx_core::progress::ProgressCallback {
    Arc::new(|p: Progress| {
        let speed = format_bytes(p.bytes_per_second as u64);
        eprint!(
            "\r{} {} / {} ({}/sn)   ",
            p.message,
            format_bytes(p.bytes_done),
            format_bytes(p.bytes_total),
            speed
        );
    })
}

fn main() {
    if let Err(e) = run() {
        eprintln!("\nHata: {e}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Hakkinda => {
            println!("DelilX {}", delilx_core::version());
            println!("{}", delilx_core::APP_SLOGAN);
            println!("\nTürkçe adli bilişim veri edinim ve inceleme uygulaması.");
            println!("Kaynak delil yalnızca okunur; hiçbir koşulda değiştirilmez.");
        }
        Commands::Cihazlar => {
            let devices = delilx_core::device::detect_devices()?;
            if devices.is_empty() {
                println!("Algılanan depolama aygıtı yok.");
                #[cfg(not(windows))]
                println!("(Not: Cihaz algılama yalnızca Windows üzerinde tam çalışır.)");
                return Ok(());
            }
            println!(
                "{:<40} {:<28} {:<12} {:<24} Yazma Koruması",
                "Aygıt", "Tür", "Kapasite", "Dosya Sistemi"
            );
            println!("{}", "-".repeat(120));
            for d in &devices {
                println!(
                    "{:<40} {:<28} {:<12} {:<24} {}",
                    truncate(&d.display_name, 40),
                    truncate(d.kind.label(), 28),
                    d.capacity_text(),
                    truncate(d.filesystem.as_deref().unwrap_or("-"), 24),
                    d.write_protection.label()
                );
                if let Some(w) = &d.source_warning {
                    println!("    Uyarı: {w}");
                }
            }
        }
        Commands::Raw {
            kaynak,
            hedef,
            sektor,
            hata_politikasi,
            dogrulama_yok,
        } => {
            println!("RAW/DD imaj edinimi başlıyor...");
            println!("Kaynak: {}", kaynak.display());
            println!("Hedef : {}", hedef.display());
            let mut cfg = RawAcquisitionConfig::new(&kaynak, &hedef);
            cfg.bytes_per_sector = sektor;
            cfg.error_policy = match hata_politikasi.as_str() {
                "durdur" | "abort" => ReadErrorPolicy::Abort,
                _ => ReadErrorPolicy::ZeroFill,
            };
            cfg.verify_after = !dogrulama_yok;
            let mut source = delilx_core::source::open_source(&kaynak)?;
            let cancel = CancelToken::new();
            let result = acquire_raw(&cfg, &mut *source, Some(progress_callback()), &cancel)?;
            eprintln!();
            println!("{}", result.summary_text());
            println!("Süre: {}", format_duration(result.duration_seconds));
            if result.verified == Some(false) {
                anyhow::bail!("Doğrulama başarısız!");
            }
        }
        Commands::E01 {
            kaynak,
            hedef,
            delil_no,
            vaka_no,
            uzman,
            aciklama,
            sikistirma,
            medya,
            segment_mib,
        } => {
            println!("E01 (EnCase) imaj edinimi başlıyor...");
            println!("Kaynak: {}", kaynak.display());
            println!("Hedef : {}", hedef.display());
            let mut cfg = EwfWriterConfig::new(&hedef);
            cfg.case_info = EwfCaseInfo {
                evidence_number: delil_no,
                case_number: vaka_no,
                description: aciklama,
                examiner_name: uzman,
                ..Default::default()
            };
            cfg.media_type = match medya {
                MediaArg::Sabit => MediaType::Fixed,
                MediaArg::Cikarilabilir => MediaType::Removable,
            };
            cfg.compression = match sikistirma {
                CompressionArg::Yok => CompressionMode::None,
                CompressionArg::Hizli => CompressionMode::Fast,
                CompressionArg::Eniyi => CompressionMode::Best,
            };
            cfg.segment_size_bytes = segment_mib * 1024 * 1024;
            let mut source = delilx_core::source::open_source(&kaynak)?;
            let cancel = CancelToken::new();
            let result = EwfWriter::acquire(cfg, &mut *source, Some(progress_callback()), &cancel)?;
            eprintln!();
            println!("Segment dosyaları: {}", result.segment_files.join(", "));
            println!(
                "Toplam: {} ({} bayt)",
                format_bytes(result.total_bytes),
                result.total_bytes
            );
            println!("Blok sayısı: {}", result.total_chunks);
            println!("MD5 : {}", result.md5);
            println!("SHA1: {}", result.sha1);
            for h in &result.extra_hashes.values {
                println!("{}: {}", h.algorithm.name(), h.value);
            }
            println!("Süre: {}", format_duration(result.duration_seconds));
        }
        Commands::Mantiksal {
            kaynak,
            hedef,
            kopyalama_yok,
        } => {
            println!("Mantıksal edinim başlıyor...");
            let mut cfg = LogicalConfig::new(kaynak, &hedef);
            cfg.copy_files = !kopyalama_yok;
            let cancel = CancelToken::new();
            let result = acquire_logical(&cfg, Some(progress_callback()), &cancel)?;
            eprintln!();
            println!(
                "Dosya sayısı: {}, toplam: {}",
                result.inventory.file_count,
                format_bytes(result.total_bytes)
            );
            println!("Kopyalanan: {}", result.copied_files);
            println!("Süre: {}", format_duration(result.duration_seconds));
            // Envanteri yaz
            let inv_path = hedef.join("envanter.csv");
            if cfg.copy_files {
                std::fs::write(&inv_path, result.inventory_csv())?;
                println!("Envanter: {}", inv_path.display());
            }
        }
        Commands::Incele { imaj } => {
            println!("İmaj inceleniyor: {}", imaj.display());
            let cancel = CancelToken::new();
            let insp = delilx_core::inspect::inspect_image(&imaj, None, &cancel)?;
            println!("Tür         : {}", insp.kind.label());
            println!(
                "Boyut       : {} ({} bayt)",
                format_bytes(insp.total_bytes),
                insp.total_bytes
            );
            println!("Sektör      : {} bayt", insp.bytes_per_sector);
            println!("Bölüm tablosu: {}", insp.layout.table_type.label());
            for p in &insp.layout.partitions {
                println!(
                    "  Bölüm {}: sektör {} + {} ({}), tür: {}, dosya sistemi: {}",
                    p.index,
                    p.start_sector,
                    p.sector_count,
                    format_bytes(p.size_bytes),
                    p.type_code,
                    p.filesystem.as_deref().unwrap_or("bilinmiyor")
                );
            }
            println!(
                "Dosya sistemi: {}",
                insp.filesystem.as_deref().unwrap_or("tanınamadı")
            );
            println!("Dosya sayısı : {}", insp.files.len());
            for f in insp.files.iter().take(50) {
                println!("  {} {}", if f.is_dir { "[D]" } else { "   " }, f.path);
            }
            if insp.files.len() > 50 {
                println!("  ... ({} dosya daha)", insp.files.len() - 50);
            }
            for w in &insp.log.entries {
                if w.level != delilx_core::progress::LogLevel::Info {
                    println!("{}: {}", w.level.label(), w.message);
                }
            }
        }
        Commands::Dogrula {
            imaj,
            sha256,
            sha512,
        } => {
            let mut expected = delilx_core::hash::HashSet::new();
            let mut algorithms = Vec::new();
            if let Some(v) = sha256 {
                expected.insert(HashAlgorithm::Sha256, v.to_lowercase());
                algorithms.push(HashAlgorithm::Sha256);
            }
            if let Some(v) = sha512 {
                expected.insert(HashAlgorithm::Sha512, v.to_lowercase());
                algorithms.push(HashAlgorithm::Sha512);
            }
            if algorithms.is_empty() {
                algorithms = vec![HashAlgorithm::Sha256, HashAlgorithm::Sha512];
            }
            let report = delilx_core::verify::verify_file(&imaj, &expected, &algorithms)?;
            println!("{}", report.summary_text());
            if !report.outcome.success {
                anyhow::bail!("Doğrulama başarısız!");
            }
        }
    }
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}
