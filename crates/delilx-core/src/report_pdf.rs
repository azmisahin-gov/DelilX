//! PDF rapor üretimi (Türkçe).
//!
//! DejaVu Sans yazı tipi gömülür; Türkçe karakterler doğru görüntülenir.

use crate::error::{DelilXError, Result};
use crate::report::EvidenceReport;
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

/// Gömülü normal yazı tipi (DejaVu Sans).
const FONT_NORMAL: &[u8] = include_bytes!("../../../assets/DejaVuSans.ttf");
/// Gömülü kalın yazı tipi (DejaVu Sans Bold).
const FONT_BOLD: &[u8] = include_bytes!("../../../assets/DejaVuSans-Bold.ttf");

/// Raporu PDF dosyasına yazar.
pub fn write_report_pdf(report: &EvidenceReport, path: &Path) -> Result<()> {
    let (doc, page1, layer1) =
        PdfDocument::new("DelilX Delil Raporu", Mm(210.0), Mm(297.0), "Rapor");

    let font = doc
        .add_external_font(std::io::Cursor::new(FONT_NORMAL))
        .map_err(|e| DelilXError::other(format!("PDF yazı tipi yüklenemedi: {e}")))?;
    let font_bold = doc
        .add_external_font(std::io::Cursor::new(FONT_BOLD))
        .map_err(|e| DelilXError::other(format!("PDF kalın yazı tipi yüklenemedi: {e}")))?;

    let mut cursor_y = 280.0f32;
    let left = 15.0f32;
    let page_height = 297.0f32;
    let bottom_margin = 15.0f32;
    let line_height = 5.5f32;

    let mut current_page = page1;
    let mut current_layer = layer1;

    macro_rules! new_page {
        () => {{
            let (p, l) = doc.add_page(Mm(210.0), Mm(297.0), "Rapor");
            current_page = p;
            current_layer = l;
            cursor_y = 280.0;
        }};
    }

    macro_rules! ensure_space {
        ($needed:expr) => {
            if cursor_y - $needed < bottom_margin {
                new_page!();
            }
        };
    }

    // Başlık
    {
        let layer = doc.get_page(current_page).get_layer(current_layer);
        layer.use_text(
            "DELİLX DELİL RAPORU",
            20.0,
            Mm(left),
            Mm(cursor_y),
            &font_bold,
        );
    }
    cursor_y -= 8.0;
    {
        let layer = doc.get_page(current_page).get_layer(current_layer);
        layer.use_text(
            "Dijital Delili Koru, Doğrula ve İncele.",
            11.0,
            Mm(left),
            Mm(cursor_y),
            &font,
        );
    }
    cursor_y -= 10.0;

    macro_rules! section {
        ($title:expr) => {{
            ensure_space!(line_height + 4.0);
            let layer = doc.get_page(current_page).get_layer(current_layer);
            layer.use_text($title, 13.0, Mm(left), Mm(cursor_y), &font_bold);
            cursor_y -= line_height + 1.0;
        }};
    }

    macro_rules! line {
        ($label:expr, $value:expr) => {{
            ensure_space!(line_height);
            let layer = doc.get_page(current_page).get_layer(current_layer);
            layer.use_text(
                format!("{}: {}", $label, $value),
                10.0,
                Mm(left + 2.0),
                Mm(cursor_y),
                &font,
            );
            cursor_y -= line_height;
        }};
    }

    // Kimlik
    ensure_space!(40.0);
    section!("Rapor Bilgileri");
    line!("İşlem Numarası", &report.operation_number);
    line!("Delil Numarası", &report.evidence_number);
    line!("Vaka Numarası", &report.case_number);
    line!("İncelemeyi Yapan", &report.examiner);
    line!("Açıklama", &report.description);
    cursor_y -= 3.0;

    // Kaynak cihaz
    ensure_space!(50.0);
    section!("Kaynak Cihaz Bilgileri");
    line!("Ad", &report.source_device.name);
    line!("Tür", &report.source_device.kind);
    line!("Kapasite", &report.source_device.capacity);
    line!("Seri Numarası", &report.source_device.serial);
    line!("Dosya Sistemi", &report.source_device.filesystem);
    line!("Yazma Koruması", &report.source_device.write_protection);
    cursor_y -= 3.0;

    // Edinim
    ensure_space!(60.0);
    section!("Edinim Bilgileri");
    line!("Edinim Yöntemi", &report.acquisition_method);
    line!("Hedef", &report.target);
    line!("Başlangıç Zamanı", &report.started_at);
    line!("Bitiş Zamanı", &report.finished_at);
    line!("Süre", &report.duration);
    if let Some(fc) = report.file_count {
        line!("Dosya Sayısı", &fc.to_string());
    }
    line!("Veri Miktarı", &report.data_size);
    cursor_y -= 3.0;

    // Hash
    ensure_space!(40.0);
    section!("Hash Değerleri ve Doğrulama");
    for h in &report.hashes {
        ensure_space!(line_height * 2.0);
        let layer = doc.get_page(current_page).get_layer(current_layer);
        layer.use_text(
            format!("{} ({})", h.algorithm, h.verification),
            10.0,
            Mm(left + 2.0),
            Mm(cursor_y),
            &font_bold,
        );
        cursor_y -= line_height;
        let layer = doc.get_page(current_page).get_layer(current_layer);
        layer.use_text(h.value.clone(), 8.5, Mm(left + 4.0), Mm(cursor_y), &font);
        cursor_y -= line_height;
    }
    line!("Doğrulama Sonucu", &report.verification_result);
    cursor_y -= 3.0;

    // Korunamayan metadata
    if !report.unsupported_metadata.is_empty() {
        ensure_space!(20.0);
        section!("Korunamayan Metadata Alanları");
        for u in &report.unsupported_metadata {
            ensure_space!(line_height);
            let layer = doc.get_page(current_page).get_layer(current_layer);
            layer.use_text(format!("• {u}"), 10.0, Mm(left + 2.0), Mm(cursor_y), &font);
            cursor_y -= line_height;
        }
        cursor_y -= 3.0;
    }

    // Uyarılar
    if !report.warnings.is_empty() {
        ensure_space!(20.0);
        section!("Uyarılar");
        for w in &report.warnings {
            for chunk in wrap_text(w, 95) {
                ensure_space!(line_height);
                let layer = doc.get_page(current_page).get_layer(current_layer);
                layer.use_text(
                    format!("• {chunk}"),
                    10.0,
                    Mm(left + 2.0),
                    Mm(cursor_y),
                    &font,
                );
                cursor_y -= line_height;
            }
        }
        cursor_y -= 3.0;
    }

    // Hatalar
    if !report.errors.is_empty() {
        ensure_space!(20.0);
        section!("Hatalar");
        for e in &report.errors {
            for chunk in wrap_text(e, 95) {
                ensure_space!(line_height);
                let layer = doc.get_page(current_page).get_layer(current_layer);
                layer.use_text(
                    format!("• {chunk}"),
                    10.0,
                    Mm(left + 2.0),
                    Mm(cursor_y),
                    &font,
                );
                cursor_y -= line_height;
            }
        }
        cursor_y -= 3.0;
    }

    // Alt bilgi
    ensure_space!(20.0);
    cursor_y = cursor_y.max(bottom_margin + 8.0);
    {
        let layer = doc.get_page(current_page).get_layer(current_layer);
        layer.use_text(
            format!("Rapor oluşturulma: {}", report.generated_at),
            8.5,
            Mm(left),
            Mm(cursor_y),
            &font,
        );
    }
    cursor_y -= 4.0;
    {
        let layer = doc.get_page(current_page).get_layer(current_layer);
        layer.use_text(
            format!("Uygulama: DelilX {}", report.app_version),
            8.5,
            Mm(left),
            Mm(cursor_y),
            &font,
        );
    }

    let _ = page_height;

    let file = File::create(path)
        .map_err(|e| DelilXError::io("PDF raporu oluşturulamadı", Some(path.to_path_buf()), e))?;
    doc.save(&mut BufWriter::new(file))
        .map_err(|e| DelilXError::other(format!("PDF kaydedilemedi: {e}")))?;
    Ok(())
}

/// Uzun metni belirtilen genişlikte satırlara böler.
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current.clone());
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_text_splits_long_lines() {
        let text = "kelime ".repeat(40);
        let lines = wrap_text(&text, 20);
        assert!(lines.len() > 1);
        for l in &lines {
            assert!(l.chars().count() <= 21);
        }
    }

    #[test]
    fn writes_valid_pdf() {
        let mut report = EvidenceReport::new();
        report.operation_number = "DLX-TEST".to_string();
        report.examiner = "Çağdaş Şölen".to_string();
        report.source_device.name = "Test Cihazı".to_string();
        report.warnings.push("Bu bir uyarıdır.".to_string());

        let path = std::env::temp_dir().join("delilx_report_test.pdf");
        report.write_pdf(&path).unwrap();
        let data = std::fs::read(&path).unwrap();
        assert!(data.starts_with(b"%PDF"));
        assert!(data.len() > 1000);
        let _ = std::fs::remove_file(&path);
    }
}
