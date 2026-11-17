//! Ortak arayüz yardımcıları.

use crate::screens::State;
use crate::theme::{self, Theme};
use eframe::egui;

/// Kaynak ve hedef seçim alanlarını çizer.
pub fn source_target_picker(
    ui: &mut egui::Ui,
    state: &State,
    source_label: &str,
    target_label: &str,
    target_is_dir: bool,
) {
    let (mut source, mut target) = {
        let s = state.lock().unwrap();
        (s.source_path.clone(), s.target_path.clone())
    };

    egui::Grid::new("kaynak_hedef")
        .num_columns(3)
        .spacing([10.0, 8.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new(source_label).strong());
            ui.add(
                egui::TextEdit::singleline(&mut source)
                    .desired_width(520.0)
                    .hint_text("Yol girin veya Gözat ile seçin"),
            );
            if ui.button("Gözat…").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_file() {
                    source = p.to_string_lossy().to_string();
                }
            }
            ui.end_row();

            ui.label(egui::RichText::new(target_label).strong());
            ui.add(
                egui::TextEdit::singleline(&mut target)
                    .desired_width(520.0)
                    .hint_text("Hedef yol"),
            );
            if ui.button("Gözat…").clicked() {
                let picked = if target_is_dir {
                    rfd::FileDialog::new().pick_folder()
                } else {
                    rfd::FileDialog::new().save_file()
                };
                if let Some(p) = picked {
                    target = p.to_string_lossy().to_string();
                }
            }
            ui.end_row();
        });

    let mut s = state.lock().unwrap();
    s.source_path = source;
    s.target_path = target;
}

/// Çalışan işlem için ilerleme çubuğu çizer.
pub fn progress_bar(ui: &mut egui::Ui, state: &State) {
    let (running, name, progress) = {
        let s = state.lock().unwrap();
        (s.job.running, s.job.name.clone(), s.job.progress.clone())
    };
    if running {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{name} çalışıyor…")).color(Theme::ACCENT));
            if let Some(p) = &progress {
                ui.label(p.message.clone());
            }
        });
        if let Some(p) = &progress {
            if let Some(frac) = p.fraction() {
                ui.add(
                    egui::ProgressBar::new(frac as f32)
                        .desired_width(ui.available_width().min(600.0))
                        .text(format!(
                            "{} / {} ({})",
                            delilx_core::progress::format_bytes(p.bytes_done),
                            delilx_core::progress::format_bytes(p.bytes_total),
                            p.percent_text()
                        )),
                );
            } else {
                ui.add(
                    egui::ProgressBar::new(0.0)
                        .desired_width(ui.available_width().min(600.0))
                        .animate(true)
                        .text(delilx_core::progress::format_bytes(p.bytes_done)),
                );
            }
        }
        ui.add_space(4.0);
        if ui.button("⏹  İptal Et").clicked() {
            if let Some(c) = &state.lock().unwrap().job.cancel {
                c.cancel();
            }
        }
    }
}

/// İşlem sonucu kutusunu çizer (varsa).
pub fn job_result(ui: &mut egui::Ui, state: &State) {
    let (result, success, running) = {
        let s = state.lock().unwrap();
        (s.job.result.clone(), s.job.success, s.job.running)
    };
    if running {
        return;
    }
    if let Some(r) = result {
        ui.add_space(8.0);
        egui::Frame::group(ui.style())
            .fill(if success {
                Theme::SUCCESS.gamma_multiply(0.10)
            } else {
                Theme::ERROR.gamma_multiply(0.10)
            })
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.label(if success {
                    theme::success("İşlem tamamlandı")
                } else {
                    theme::error("İşlem başarısız")
                });
                ui.add_space(4.0);
                ui.label(egui::RichText::new(r).size(12.5).monospace());
            });
    }
}

/// Uyarı kutusu çizer.
pub fn warning_box(ui: &mut egui::Ui, text: &str) {
    egui::Frame::group(ui.style())
        .fill(Theme::WARNING.gamma_multiply(0.12))
        .inner_margin(egui::Margin::same(10.0))
        .show(ui, |ui| {
            ui.label(theme::warning("⚠  Uyarı"));
            ui.add_space(2.0);
            ui.label(egui::RichText::new(text).size(12.5));
        });
}

/// İşlem kayıtlarını gösterir.
pub fn log_viewer(ui: &mut egui::Ui, state: &State) {
    let entries = {
        let s = state.lock().unwrap();
        s.log.clone()
    };
    ui.label(theme::subheading("İşlem Kayıtları"));
    egui::Frame::group(ui.style()).show(ui, |ui| {
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if entries.is_empty() {
                    ui.label(theme::muted("Henüz işlem kaydı yok."));
                }
                for e in entries.iter().rev() {
                    let color = match e.level {
                        delilx_core::progress::LogLevel::Info => Theme::MUTED,
                        delilx_core::progress::LogLevel::Warning => Theme::WARNING,
                        delilx_core::progress::LogLevel::Error => Theme::ERROR,
                    };
                    ui.label(
                        egui::RichText::new(format!(
                            "[{}] {} {}",
                            e.timestamp,
                            e.level.label(),
                            e.message
                        ))
                        .size(12.0)
                        .color(color),
                    );
                }
            });
    });
}
