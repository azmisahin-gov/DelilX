//! İmaj inceleme örneği.
use delilx_core::inspect::inspect_image;
use delilx_core::progress::CancelToken;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = std::path::Path::new(&args[1]);
    let cancel = CancelToken::new();
    let insp = inspect_image(path, None, &cancel)?;
    println!(
        "tür={:?} boyut={} bps={}",
        insp.kind, insp.total_bytes, insp.bytes_per_sector
    );
    println!(
        "bölüm tablosu={} bölüm sayısı={}",
        insp.layout.table_type.label(),
        insp.layout.partitions.len()
    );
    for p in &insp.layout.partitions {
        println!(
            "  bölüm {} start={} sektör={} fs={:?} tip={}",
            p.index, p.start_sector, p.sector_count, p.filesystem, p.type_code
        );
    }
    println!("dosya sistemi={:?}", insp.filesystem);
    println!("dosya sayısı={}", insp.files.len());
    for f in insp.files.iter().take(20) {
        println!(
            "  {} boyut={} ofset={} dir={}",
            f.path, f.size, f.offset, f.is_dir
        );
    }
    Ok(())
}
