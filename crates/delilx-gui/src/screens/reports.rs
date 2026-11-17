//! Delil raporları ekranı.

use crate::screens::{widgets, State};
use crate::theme::{self, Theme};
use eframe::egui;

/// Raporlar ekranını çizer.
pub fn draw(ui: &mut egui::Ui, state: &State) {
    ui.label(theme::muted(
        "Edinim işlemi tamamlandığında Türkçe delil raporu oluşturulur. PDF, JSON ve \
         CSV biçimlerinde çıktı alınabilir.",
    ));
    ui.add_space(10.0);

    ui.label(theme::subheading("Rapor Bilgileri"));
    {
        let mut s = state.lock().unwrap();
        egui::Grid::new("rapor_form")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("İşlem numarası");
                ui.add(
                    egui::TextEdit::singleline(&mut s.report.operation_number).desired_width(360.0),
                );
                ui.end_row();
                ui.label("Delil numarası");
                ui.add(
                    egui::TextEdit::singleline(&mut s.report.evidence_number).desired_width(360.0),
                );
                ui.end_row();
                ui.label("Vaka numarası");
                ui.add(egui::TextEdit::singleline(&mut s.report.case_number).desired_width(360.0));
                ui.end_row();
                ui.label("İncelemeyi yapan");
                ui.add(egui::TextEdit::singleline(&mut s.report.examiner).desired_width(360.0));
                ui.end_row();
                ui.label("Açıklama");
                ui.add(egui::TextEdit::singleline(&mut s.report.description).desired_width(360.0));
                ui.end_row();
            });
    }

    ui.add_space(10.0);
    let has_last = state.lock().unwrap().last_report.is_some();
    let mut output_dir = state.lock().unwrap().settings.output_dir.clone();

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Çıktı dizini:").strong());
        let mut dir_str = output_dir.to_string_lossy().to_string();
        if ui
            .add(egui::TextEdit::singleline(&mut dir_str).desired_width(420.0))
            .changed()
        {
            output_dir = std::path::PathBuf::from(dir_str);
        }
        if ui.button("Gözat…").clicked() {
            if let Some(p) = rfd::FileDialog::new().pick_folder() {
                output_dir = p;
            }
        }
    });
    state.lock().unwrap().settings.output_dir = output_dir.clone();

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                has_last,
                egui::Button::new("📄  Rapor Oluştur (PDF, JSON, CSV)"),
            )
            .clicked()
        {
            generate(state);
        }
        if !has_last {
            ui.label(theme::muted(
                "Önce bir edinim işlemi tamamlayın; rapor bu işlemin verilerinden oluşturulur.",
            ));
        }
    });

    // Üretilen raporlar
    let paths = state.lock().unwrap().report_paths.clone();
    if !paths.is_empty() {
        ui.add_space(10.0);
        egui::Frame::group(ui.style())
            .fill(Theme::SUCCESS.gamma_multiply(0.08))
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.label(theme::success("Raporlar oluşturuldu:"));
                for p in &paths {
                    ui.label(egui::RichText::new(p.to_string_lossy().to_string()).size(12.5));
                }
            });
    }

    // Son rapor önizlemesi
    let preview = state
        .lock()
        .unwrap()
        .last_report
        .as_ref()
        .map(|r| r.to_text());
    if let Some(text) = preview {
        ui.add_space(12.0);
        ui.label(theme::subheading("Rapor Önizlemesi"));
        egui::Frame::group(ui.style()).show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(text).monospace().size(11.5));
                });
        });
    }

    ui.add_space(12.0);
    widgets::log_viewer(ui, state);
}

/// Raporu oluşturur ve diske yazar.
fn generate(state: &State) {
    let (report_form, output_dir, last) = {
        let s = state.lock().unwrap();
        (
            s.report.clone(),
            s.settings.output_dir.clone(),
            s.last_report.clone(),
        )
    };

    let Some(mut r) = last else {
        return;
    };

    // Form alanlarını rapora uygula
    if !report_form.operation_number.trim().is_empty() {
        r.operation_number = report_form.operation_number.clone();
    }
    if !report_form.evidence_number.trim().is_empty() {
        r.evidence_number = report_form.evidence_number.clone();
    }
    if !report_form.case_number.trim().is_empty() {
        r.case_number = report_form.case_number.clone();
    }
    r.examiner = report_form.examiner.clone();
    r.description = report_form.description.clone();

    let base = format!("delil_raporu_{}", sanitize(&r.operation_number));
    match r.write_all(&output_dir, &base) {
        Ok(paths) => {
            let mut s = state.lock().unwrap();
            s.report_paths = paths.clone();
            s.last_report = Some(r);
            s.set_status(1, format!("{} rapor dosyası oluşturuldu.", paths.len()));
            s.log_info(format!("Rapor oluşturuldu: {base}"));
        }
        Err(e) => {
            let mut s = state.lock().unwrap();
            s.set_status(3, format!("Rapor oluşturulamadı: {e}"));
            s.log_error(format!("Rapor oluşturulamadı: {e}"));
            s.last_report = Some(r);
        }
    }
}

/// Dosya adı için güvenli metin üretir.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
