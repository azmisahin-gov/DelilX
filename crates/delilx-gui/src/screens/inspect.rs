//! İmaj inceleme ekranı.

use crate::screens::{widgets, State};
use crate::theme::{self, Theme};
use crate::worker;
use eframe::egui;

/// İmaj inceleme ekranını çizer.
pub fn draw(ui: &mut egui::Ui, state: &State) {
    ui.label(theme::muted(
        "Alınan RAW veya E01 imajları açar; bölümleri, dosya sistemlerini ve dosyaları \
         listeler. Orijinal imaj dosyası hiçbir koşulda değiştirilmez.",
    ));
    ui.add_space(10.0);

    let mut path = state.lock().unwrap().inspect_path.clone();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("İmaj dosyası").strong());
        ui.add(
            egui::TextEdit::singleline(&mut path)
                .desired_width(560.0)
                .hint_text("İncelenecek imaj yolu (.dd, .img, .E01)"),
        );
        if ui.button("Gözat…").clicked() {
            if let Some(p) = rfd::FileDialog::new().pick_file() {
                path = p.to_string_lossy().to_string();
            }
        }
    });
    state.lock().unwrap().inspect_path = path.clone();

    let running = state.lock().unwrap().job.running;
    let mut start_clicked = false;
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !running && !path.is_empty(),
                egui::Button::new("🔍  İmajı İncele"),
            )
            .clicked()
        {
            start_clicked = true;
        }
    });

    if start_clicked {
        start_inspect(state, path);
    }

    widgets::progress_bar(ui, state);
    widgets::job_result(ui, state);

    // Sonuç
    let inspection = state.lock().unwrap().inspection.clone();
    if let Some(insp) = inspection {
        ui.add_space(12.0);
        egui::Frame::group(ui.style())
            .fill(Theme::PANEL)
            .inner_margin(egui::Margin::same(14.0))
            .show(ui, |ui| {
                ui.label(theme::subheading("İmaj Bilgileri"));
                egui::Grid::new("inceleme_bilgi")
                    .num_columns(2)
                    .spacing([20.0, 5.0])
                    .show(ui, |ui| {
                        let rows = [
                            ("Tür", insp.kind.label().to_string()),
                            (
                                "Boyut",
                                format!(
                                    "{} ({} bayt)",
                                    delilx_core::progress::format_bytes(insp.total_bytes),
                                    insp.total_bytes
                                ),
                            ),
                            ("Sektör boyutu", format!("{} bayt", insp.bytes_per_sector)),
                            ("Bölüm tablosu", insp.layout.table_type.label().to_string()),
                            (
                                "Dosya sistemi",
                                insp.filesystem
                                    .clone()
                                    .unwrap_or_else(|| "tanınamadı".into()),
                            ),
                            ("Dosya sayısı", insp.files.len().to_string()),
                        ];
                        for (k, v) in rows {
                            ui.label(egui::RichText::new(k).strong());
                            ui.label(v);
                            ui.end_row();
                        }
                    });

                if !insp.layout.partitions.is_empty() {
                    ui.add_space(8.0);
                    ui.label(theme::subheading("Bölümler"));
                    egui::Grid::new("bolumler")
                        .num_columns(5)
                        .striped(true)
                        .spacing([14.0, 6.0])
                        .show(ui, |ui| {
                            for h in ["#", "Başlangıç", "Sektör", "Boyut", "Dosya Sistemi"] {
                                ui.label(egui::RichText::new(h).strong().color(Theme::PRIMARY));
                            }
                            ui.end_row();
                            for p in &insp.layout.partitions {
                                ui.label(p.index.to_string());
                                ui.label(p.start_sector.to_string());
                                ui.label(p.sector_count.to_string());
                                ui.label(delilx_core::progress::format_bytes(p.size_bytes));
                                ui.label(p.filesystem.as_deref().unwrap_or("-"));
                                ui.end_row();
                            }
                        });
                }

                if !insp.files.is_empty() {
                    ui.add_space(8.0);
                    ui.label(theme::subheading("Dosyalar"));
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .show(ui, |ui| {
                            egui::Grid::new("dosyalar")
                                .num_columns(3)
                                .striped(true)
                                .spacing([14.0, 4.0])
                                .show(ui, |ui| {
                                    for h in ["Yol", "Boyut", "Tür"] {
                                        ui.label(
                                            egui::RichText::new(h).strong().color(Theme::PRIMARY),
                                        );
                                    }
                                    ui.end_row();
                                    for f in &insp.files {
                                        ui.label(&f.path);
                                        ui.label(if f.is_dir {
                                            "-".to_string()
                                        } else {
                                            delilx_core::progress::format_bytes(f.size)
                                        });
                                        ui.label(if f.is_dir { "Dizin" } else { "Dosya" });
                                        ui.end_row();
                                    }
                                });
                        });
                }

                for w in insp.log.by_level(delilx_core::progress::LogLevel::Warning) {
                    ui.add_space(4.0);
                    ui.label(theme::warning(&w.message));
                }
            });
    }

    ui.add_space(12.0);
    widgets::log_viewer(ui, state);
}

/// İnceleme başlatır.
fn start_inspect(state: &State, path: String) {
    {
        let mut s = state.lock().unwrap();
        s.log_info(format!("İmaj inceleme başladı: {path}"));
    }
    worker::spawn(state.clone(), "İmaj İnceleme", move |cb, cancel, st| {
        let insp =
            delilx_core::inspect::inspect_image(std::path::Path::new(&path), Some(cb), &cancel)
                .map_err(|e| format!("{e}"))?;
        let mut msg = String::new();
        msg.push_str(&format!("Tür: {}\n", insp.kind.label()));
        msg.push_str(&format!("Bölüm sayısı: {}\n", insp.layout.partitions.len()));
        msg.push_str(&format!(
            "Dosya sistemi: {}\n",
            insp.filesystem
                .clone()
                .unwrap_or_else(|| "tanınamadı".into())
        ));
        msg.push_str(&format!("Dosya sayısı: {}\n", insp.files.len()));
        // Sonucu duruma kaydet
        st.lock().unwrap().inspection = Some(insp);
        Ok(msg)
    });
}
