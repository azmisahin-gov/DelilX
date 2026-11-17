//! Mantıksal ve seçmeli edinim ekranı.

use crate::screens::{widgets, State};
use crate::theme;
use crate::worker;
use eframe::egui;
use std::path::PathBuf;

/// Mantıksal (`selective=false`) veya seçmeli (`selective=true`) edinim ekranını çizer.
pub fn draw(ui: &mut egui::Ui, state: &State, selective: bool) {
    if selective {
        ui.label(theme::muted(
            "Kullanıcının seçtiği dosya ve klasörleri alır. Dosya envanteri ve \
             SHA-256/SHA-512 hash değerleri oluşturulur.",
        ));
    } else {
        ui.label(theme::muted(
            "Yalnızca mevcut dosyaları alır. Klasör yapısı korunur, dosya metadata'sı \
             ve SHA-256/SHA-512 hash değerleri kaydedilir.",
        ));
    }
    ui.add_space(10.0);

    widgets::warning_box(
        ui,
        "Mantıksal ve seçmeli edinim yalnızca mevcut dosyaları alır. Silinmiş dosyalar, \
         boş alan kalıntıları ve dosya sistemi üst verisi edinilmez. Silinmiş veri \
         önemliyse RAW veya E01 tam disk imajı tercih edilmelidir.",
    );
    ui.add_space(10.0);

    // Kaynak dizin
    let mut source_dir = {
        let s = state.lock().unwrap();
        if selective {
            s.logical.source_dir.clone()
        } else {
            s.source_path.clone()
        }
    };
    let mut target_dir = state.lock().unwrap().target_path.clone();

    egui::Grid::new("mantiksal_grid")
        .num_columns(3)
        .spacing([10.0, 8.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Kaynak dizin").strong());
            ui.add(
                egui::TextEdit::singleline(&mut source_dir)
                    .desired_width(520.0)
                    .hint_text("Kaynak klasör"),
            );
            if ui.button("Gözat…").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    source_dir = p.to_string_lossy().to_string();
                }
            }
            ui.end_row();
            ui.label(egui::RichText::new("Hedef dizin").strong());
            ui.add(
                egui::TextEdit::singleline(&mut target_dir)
                    .desired_width(520.0)
                    .hint_text("Hedef klasör"),
            );
            if ui.button("Gözat…").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    target_dir = p.to_string_lossy().to_string();
                }
            }
            ui.end_row();
        });

    {
        let mut s = state.lock().unwrap();
        if selective {
            s.logical.source_dir = source_dir.clone();
        } else {
            s.source_path = source_dir.clone();
        }
        s.target_path = target_dir.clone();
    }

    ui.add_space(8.0);
    let mut selected_files: Vec<PathBuf> = state.lock().unwrap().logical.selected_files.clone();
    ui.horizontal(|ui| {
        if selective {
            if ui.button("➕  Dosya Ekle").clicked() {
                if let Some(files) = rfd::FileDialog::new().pick_files() {
                    selected_files.extend(files);
                }
            }
            if ui.button("🧹  Listeyi Temizle").clicked() {
                selected_files.clear();
            }
        }
        ui.separator();
        let mut inv_only = state.lock().unwrap().logical.inventory_only;
        ui.checkbox(&mut inv_only, "Yalnızca envanter (kopyalama)");
        let mut exts = state.lock().unwrap().logical.extensions.clone();
        ui.label("Uzantı filtresi:");
        ui.add(
            egui::TextEdit::singleline(&mut exts)
                .desired_width(160.0)
                .hint_text("ör. txt,pdf,jpg"),
        );
        let mut s = state.lock().unwrap();
        s.logical.inventory_only = inv_only;
        s.logical.extensions = exts;
        s.logical.selected_files = selected_files.clone();
    });

    if selective && !selected_files.is_empty() {
        ui.add_space(6.0);
        ui.label(theme::subheading(&format!(
            "Seçili Dosyalar ({})",
            selected_files.len()
        )));
        egui::ScrollArea::vertical()
            .max_height(120.0)
            .show(ui, |ui| {
                for f in &selected_files {
                    ui.label(egui::RichText::new(f.to_string_lossy().to_string()).size(12.0));
                }
            });
    }

    ui.add_space(10.0);
    let running = state.lock().unwrap().job.running;
    let can_start = if selective {
        !selected_files.is_empty() || !source_dir.is_empty()
    } else {
        !source_dir.is_empty()
    };

    ui.horizontal(|ui| {
        let label = if selective {
            "▶  Seçmeli Edinimi Başlat"
        } else {
            "▶  Mantıksal Edinimi Başlat"
        };
        if ui
            .add_enabled(
                !running && can_start && !target_dir.is_empty(),
                egui::Button::new(label),
            )
            .clicked()
        {
            start(state, selective);
        }
    });

    widgets::progress_bar(ui, state);
    widgets::job_result(ui, state);
    ui.add_space(12.0);
    widgets::log_viewer(ui, state);
}

/// Edinimi başlatır.
fn start(state: &State, selective: bool) {
    let (source_dir, selected, target, inv_only, exts) = {
        let s = state.lock().unwrap();
        (
            if selective {
                s.logical.source_dir.clone()
            } else {
                s.source_path.clone()
            },
            s.logical.selected_files.clone(),
            s.target_path.clone(),
            s.logical.inventory_only,
            s.logical.extensions.clone(),
        )
    };

    let mut sources: Vec<PathBuf> = Vec::new();
    if selective && !selected.is_empty() {
        sources.extend(selected);
    } else if !source_dir.is_empty() {
        sources.push(PathBuf::from(&source_dir));
    }

    let name = if selective {
        "Seçmeli Edinim"
    } else {
        "Mantıksal Edinim"
    };
    {
        let mut s = state.lock().unwrap();
        s.log_info(format!("{name} başladı. Kaynak sayısı: {}", sources.len()));
    }

    worker::spawn(state.clone(), name, move |cb, cancel, _st| {
        let mut cfg = delilx_core::logical::LogicalConfig::new(sources, PathBuf::from(&target));
        cfg.copy_files = !inv_only;
        if !exts.trim().is_empty() {
            cfg.include_extensions = exts
                .split(',')
                .map(|e| e.trim().to_string())
                .filter(|e| !e.is_empty())
                .collect();
        }
        let result = delilx_core::logical::acquire_logical(&cfg, Some(cb), &cancel)
            .map_err(|e| format!("{e}"))?;

        // Envanteri yaz
        if cfg.copy_files {
            let inv_path = PathBuf::from(&target).join("envanter.csv");
            let _ = std::fs::write(&inv_path, result.inventory_csv());
            let hash_path = PathBuf::from(&target).join("hash_listesi.csv");
            let _ = std::fs::write(&hash_path, result.hashes_csv());
        }

        let mut msg = String::new();
        msg.push_str(&format!("Dosya sayısı: {}\n", result.inventory.file_count));
        msg.push_str(&format!(
            "Toplam boyut: {}\n",
            delilx_core::progress::format_bytes(result.total_bytes)
        ));
        msg.push_str(&format!("Kopyalanan: {}\n", result.copied_files));
        for w in result
            .log
            .by_level(delilx_core::progress::LogLevel::Warning)
        {
            msg.push_str(&format!("Uyarı: {}\n", w.message));
        }
        Ok(msg)
    });
}
