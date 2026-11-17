//! DelilX grafik arayüzü (eframe/egui).
//!
//! **Dijital Delili Koru, Doğrula ve İncele.**
//!
//! Modern, anlaşılır ve profesyonel bir Windows masaüstü arayüzü. Bütün
//! menüler, butonlar, açıklamalar ve hata mesajları Türkçedir.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod screens;
mod state;
mod theme;
mod worker;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 780.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("DelilX — Adli Bilişim Veri Edinim ve İnceleme"),
        ..Default::default()
    };
    eframe::run_native(
        "DelilX",
        native_options,
        Box::new(|cc| Ok(Box::new(app::DelilXApp::new(cc)))),
    )
}
