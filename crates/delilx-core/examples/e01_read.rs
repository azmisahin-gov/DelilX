//! E01 okuyucu doğrulama örneği: imajı açar ve veriyi yeniden üretir.
use delilx_core::e01::EwfImage;
use sha2::{Digest, Sha256};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = std::path::Path::new(&args[1]);
    let image = EwfImage::open(path)?;
    let info = image.info();
    println!("bölüm_türleri={:?}", info.section_types);
    println!("md5={:?}", info.md5);
    println!("sha1={:?}", info.sha1);
    println!(
        "sektör={} blok={} boyut={}",
        info.volume.number_of_sectors, info.volume.number_of_chunks, info.volume.total_bytes
    );
    for (k, v) in image.header2_fields() {
        println!("header2: {k} = {v}");
    }
    // Tüm veriyi oku ve SHA-256 hesapla
    let mut hasher = Sha256::new();
    let mut offset = 0u64;
    let total = info.volume.total_bytes;
    let mut buf = vec![0u8; 65536];
    while offset < total {
        let want = ((total - offset) as usize).min(buf.len());
        let n = image.read_at(offset, &mut buf[..want])?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        offset += n as u64;
    }
    println!("okunan_bayt={offset}");
    println!("sha256={}", hex::encode(hasher.finalize()));
    Ok(())
}
