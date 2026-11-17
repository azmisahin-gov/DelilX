//! E01 yazıcı doğrulama örneği: deterministik veri ile E01 üretir.
use delilx_core::e01::writer::{
    CompressionMode, EwfCaseInfo, EwfWriter, EwfWriterConfig, MediaType,
};
use delilx_core::progress::CancelToken;
use delilx_core::source::FileSource;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let src = &args[1];
    let dst = &args[2];
    let comp = args.get(3).map(|s| s.as_str()).unwrap_or("best");
    let seg_mb: u64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1400);

    let mut cfg = EwfWriterConfig::new(dst);
    cfg.case_info = EwfCaseInfo {
        evidence_number: "DELIL-TEST-1".into(),
        case_number: "VAKA-TEST-1".into(),
        description: "DelilX E01 test edinimi".into(),
        examiner_name: "Test Uzmanı".into(),
        notes: "otomatik test".into(),
        model: "TestModel".into(),
        serial_number: "SN123456".into(),
        device_label: "TEST".into(),
    };
    cfg.media_type = MediaType::Fixed;
    cfg.compression = match comp {
        "none" => CompressionMode::None,
        "fast" => CompressionMode::Fast,
        _ => CompressionMode::Best,
    };
    cfg.segment_size_bytes = seg_mb * 1024 * 1024;

    let mut source = FileSource::open(std::path::Path::new(src))?;
    let cancel = CancelToken::new();
    let result = EwfWriter::acquire(cfg, &mut source, None, &cancel)?;
    println!("segment_files={:?}", result.segment_files);
    println!("total_bytes={}", result.total_bytes);
    println!("total_chunks={}", result.total_chunks);
    println!("md5={}", result.md5);
    println!("sha1={}", result.sha1);
    Ok(())
}
