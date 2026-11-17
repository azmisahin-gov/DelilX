use delilx_core::inspect::ImageReader;
use delilx_core::progress::CancelToken;
fn main() -> anyhow::Result<()> {
    let r = ImageReader::open(std::path::Path::new("/workspace/_work/fat.img"))?;
    let c = CancelToken::new();
    match delilx_core::fs_fat::list_fat_files(&r, 0, "FAT32", &c) {
        Ok(f) => {
            println!("OK {} files", f.len());
            for x in &f {
                println!("  {} {} {}", x.path, x.size, x.is_dir);
            }
        }
        Err(e) => println!("ERR {e}"),
    }
    Ok(())
}
