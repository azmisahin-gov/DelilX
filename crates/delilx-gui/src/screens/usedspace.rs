//! Kullanılan alan edinimi ekranı.

use crate::screens::{widgets, State};
use crate::theme;
use crate::worker;
use eframe::egui;
use std::path::PathBuf;

/// Kullanılan alan ekranını çizer.
pub fn draw(ui: &mut egui::Ui, state: &State) {
    ui.label(theme::muted(
        "Yalnızca dosya sisteminin kullandığı sektörleri alır; boş alanı atlar. \
         İmaj boyutunu küçültür.",
    ));
    ui.add_space(10.0);

    widgets::warning_box(
        ui,
        "Uyarı: Kullanılan alan ediniminde yalnızca kullanılan sektörler alınır. Boş alan \
         dışarıda bırakılır; bu nedenle silinmiş dosyalar ve boş alandaki kalıntı veriler \
         imaja dahil edilmez ve kurtarılamaz. Adli incelemede silinmiş veri önemliyse \
         tam disk imajı (RAW veya E01) tercih edilmelidir.",
    );
    ui.add_space(10.0);

    widgets::source_target_picker(ui, state, "Kaynak (cihaz/imaj)", "Hedef dosya", false);

    ui.add_space(10.0);
    let running = state.lock().unwrap().job.running;
    let (source, target) = {
        let s = state.lock().unwrap();
        (s.source_path.clone(), s.target_path.clone())
    };

    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !running && !source.is_empty() && !target.is_empty(),
                egui::Button::new("▶  Kullanılan Alanı Edin"),
            )
            .clicked()
        {
            start(state);
        }
    });

    widgets::progress_bar(ui, state);
    widgets::job_result(ui, state);
    ui.add_space(12.0);
    widgets::log_viewer(ui, state);
}

/// Edinimi başlatır.
fn start(state: &State) {
    let (source, target, bps) = {
        let s = state.lock().unwrap();
        (
            s.source_path.clone(),
            s.target_path.clone(),
            s.settings.bytes_per_sector,
        )
    };
    if source == target {
        state.lock().unwrap().set_status(
            3,
            "Kaynak ve hedef aynı olamaz. Delili korumak için işlem durduruldu.",
        );
        return;
    }
    {
        let mut s = state.lock().unwrap();
        s.log_warn("Kullanılan alan edinimi: boş alan atlanacak; silinmiş veriler kaybolabilir.");
    }

    worker::spawn(state.clone(), "Kullanılan Alan", move |cb, cancel, _st| {
        let mut cfg = delilx_core::usedspace::UsedSpaceConfig::new(
            PathBuf::from(&source),
            PathBuf::from(&target),
        );
        cfg.bytes_per_sector = bps;
        let mut src = delilx_core::source::open_source(std::path::Path::new(&source))
            .map_err(|e| format!("Kaynak açılamadı: {e}"))?;
        let result = delilx_core::usedspace::acquire_used_space(&cfg, &mut *src, Some(cb), &cancel)
            .map_err(|e| format!("{e}"))?;

        let mut msg = String::new();
        msg.push_str(&format!(
            "Dosya sistemi: {}\n",
            result.filesystem.as_deref().unwrap_or("bilinmiyor")
        ));
        msg.push_str(&format!(
            "Kullanılan aralık: {}\n",
            result.used_ranges.len()
        ));
        msg.push_str(&format!("Edinilen: {} bayt\n", result.acquired_bytes));
        msg.push_str(&format!(
            "Atlanan boş alan: {} bayt\n",
            result.skipped_bytes
        ));
        for h in &result.hashes.values {
            msg.push_str(&format!("{}: {}\n", h.algorithm.name(), h.value));
        }
        msg.push_str("\nUYARI: Silinmiş veriler ve boş alan kalıntıları bu imaja dahil edilmedi.");
        Ok(msg)
    });
}
