//! Hash doğrulama ekranı.

use crate::screens::{widgets, State};
use crate::theme::{self, Theme};
use crate::worker;
use eframe::egui;

/// Hash doğrulama ekranını çizer.
pub fn draw(ui: &mut egui::Ui, state: &State) {
    ui.label(theme::muted(
        "Bir imaj veya dosyanın hash değerlerini yeniden hesaplayarak kayıtlı \
         değerlerle karşılaştırır.",
    ));
    ui.add_space(10.0);

    let (mut path, mut sha256, mut sha512) = {
        let s = state.lock().unwrap();
        (
            s.verify.path.clone(),
            s.verify.sha256.clone(),
            s.verify.sha512.clone(),
        )
    };

    egui::Grid::new("dogrula_grid")
        .num_columns(3)
        .spacing([10.0, 8.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Dosya / imaj").strong());
            ui.add(
                egui::TextEdit::singleline(&mut path)
                    .desired_width(520.0)
                    .hint_text("Doğrulanacak dosya"),
            );
            if ui.button("Gözat…").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_file() {
                    path = p.to_string_lossy().to_string();
                }
            }
            ui.end_row();

            ui.label(egui::RichText::new("Beklenen SHA-256").strong());
            ui.add(
                egui::TextEdit::singleline(&mut sha256)
                    .desired_width(520.0)
                    .hint_text("Onaltılık SHA-256 (isteğe bağlı)"),
            );
            ui.label("");
            ui.end_row();

            ui.label(egui::RichText::new("Beklenen SHA-512").strong());
            ui.add(
                egui::TextEdit::singleline(&mut sha512)
                    .desired_width(520.0)
                    .hint_text("Onaltılık SHA-512 (isteğe bağlı)"),
            );
            ui.label("");
            ui.end_row();
        });

    {
        let mut s = state.lock().unwrap();
        s.verify.path = path.clone();
        s.verify.sha256 = sha256.clone();
        s.verify.sha512 = sha512.clone();
    }

    ui.add_space(10.0);
    let running = state.lock().unwrap().job.running;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !running && !path.is_empty(),
                egui::Button::new("✓  Doğrula"),
            )
            .clicked()
        {
            start(state, path.clone(), sha256.clone(), sha512.clone());
        }
    });

    widgets::progress_bar(ui, state);
    widgets::job_result(ui, state);

    // Sonuç kutusu
    let result = state.lock().unwrap().verify.result.clone();
    if let Some(r) = result {
        let success = state.lock().unwrap().verify.success;
        ui.add_space(10.0);
        egui::Frame::group(ui.style())
            .fill(if success {
                Theme::SUCCESS.gamma_multiply(0.10)
            } else {
                Theme::ERROR.gamma_multiply(0.10)
            })
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.label(if success {
                    theme::success("Doğrulama BAŞARILI — hash değerleri eşleşti.")
                } else {
                    theme::error("Doğrulama BAŞARISIZ — hash değerleri uyuşmadı.")
                });
                ui.add_space(4.0);
                ui.label(egui::RichText::new(r).size(12.0).monospace());
            });
    }

    ui.add_space(12.0);
    ui.label(theme::muted(
        "Not: Hash değerleri ile metadata bütünlüğü ayrı değerlendirilir. Hash \
         eşleşmesi veri bütünlüğünü gösterir; metadata bütünlüğü ayrıca incelenmelidir.",
    ));

    ui.add_space(12.0);
    widgets::log_viewer(ui, state);
}

/// Doğrulama başlatır.
fn start(state: &State, path: String, sha256: String, sha512: String) {
    worker::spawn(state.clone(), "Hash Doğrulama", move |_cb, _cancel, st| {
        let mut expected = delilx_core::hash::HashSet::new();
        let mut algorithms = Vec::new();
        if !sha256.trim().is_empty() {
            expected.insert(
                delilx_core::hash::HashAlgorithm::Sha256,
                sha256.trim().to_lowercase(),
            );
            algorithms.push(delilx_core::hash::HashAlgorithm::Sha256);
        }
        if !sha512.trim().is_empty() {
            expected.insert(
                delilx_core::hash::HashAlgorithm::Sha512,
                sha512.trim().to_lowercase(),
            );
            algorithms.push(delilx_core::hash::HashAlgorithm::Sha512);
        }
        if algorithms.is_empty() {
            algorithms = vec![
                delilx_core::hash::HashAlgorithm::Sha256,
                delilx_core::hash::HashAlgorithm::Sha512,
            ];
        }

        let report =
            delilx_core::verify::verify_file(std::path::Path::new(&path), &expected, &algorithms)
                .map_err(|e| format!("{e}"))?;

        {
            let mut s = st.lock().unwrap();
            s.verify.result = Some(report.summary_text());
            s.verify.success = report.outcome.success;
        }

        if report.outcome.success {
            Ok("Doğrulama başarılı.".to_string())
        } else {
            Ok("Doğrulama başarısız.".to_string())
        }
    });
}
