//! Ana sayfa ekranı.

use crate::screens::State;
use crate::state::Screen;
use crate::theme::{self, Theme};
use eframe::egui;

/// Ana sayfayı çizer.
pub fn draw(ui: &mut egui::Ui, state: &State) {
    ui.add_space(6.0);

    // Hoş geldin kartı
    egui::Frame::group(ui.style())
        .fill(Theme::PANEL)
        .inner_margin(egui::Margin::same(18.0))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("DelilX'e Hoş Geldiniz")
                    .size(22.0)
                    .strong()
                    .color(Theme::PRIMARY),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(
                    "DelilX, dijital delillerin adli bilişim standartlarına uygun olarak \
                     edinilmesi, doğrulanması ve incelenmesi için geliştirilmiş Türkçe bir \
                     Windows uygulamasıdır.",
                )
                .size(14.0),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "Temel ilke: Kaynak delili koru, edinimi doğrula, bütün işlemleri \
                     kaydet ve kullanıcıya her şeyi Türkçe anlat.",
                )
                .size(13.0)
                .italics()
                .color(Theme::ACCENT),
            );
        });

    ui.add_space(12.0);
    ui.label(theme::subheading("Hızlı İşlemler"));
    ui.add_space(6.0);

    let mut go = None;
    ui.horizontal_wrapped(|ui| {
        for (screen, desc) in [
            (Screen::Devices, "USB ve SD kartları algıla"),
            (Screen::WriteProtect, "Yazılımsal yazma koruması uygula"),
            (Screen::Raw, "RAW/DD tam disk imajı al"),
            (Screen::E01, "EnCase uyumlu E01 imajı oluştur"),
            (Screen::Logical, "Mevcut dosyaları edin"),
            (Screen::Selective, "Seçili dosyaları edin"),
            (Screen::Inspect, "Alınan imajları incele"),
            (Screen::Verify, "Hash doğrulaması yap"),
            (Screen::Reports, "Delil raporu oluştur"),
        ] {
            let button = egui::Button::new(
                egui::RichText::new(format!("{}  {}\n{}", screen.icon(), screen.title(), desc))
                    .size(13.5),
            )
            .min_size(egui::vec2(230.0, 56.0));
            if ui.add(button).clicked() {
                go = Some(screen);
            }
        }
    });

    if let Some(s) = go {
        state.lock().unwrap().current = s;
    }

    ui.add_space(14.0);
    ui.label(theme::subheading("Sistem Durumu"));
    ui.add_space(6.0);

    let (device_count, job_running, log_count) = {
        let s = state.lock().unwrap();
        (s.devices.len(), s.job.running, s.log.len())
    };

    egui::Frame::group(ui.style()).show(ui, |ui| {
        egui::Grid::new("durum_grid")
            .num_columns(2)
            .spacing([24.0, 6.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Algılanan aygıt:").strong());
                ui.label(format!("{device_count}"));
                ui.end_row();
                ui.label(egui::RichText::new("Çalışan işlem:").strong());
                ui.label(if job_running { "Var" } else { "Yok" });
                ui.end_row();
                ui.label(egui::RichText::new("İşlem kaydı:").strong());
                ui.label(format!("{log_count} satır"));
                ui.end_row();
                ui.label(egui::RichText::new("Uygulama sürümü:").strong());
                ui.label(format!("DelilX {}", delilx_core::version()));
                ui.end_row();
            });
    });

    ui.add_space(14.0);
    ui.label(theme::subheading("Güvenlik Uyarıları"));
    ui.add_space(4.0);
    ui.label(theme::muted(
        "• Kaynak cihazlara hiçbir koşulda yazılmaz; yalnızca okuma erişimi kullanılır.",
    ));
    ui.label(theme::muted(
        "• Yazılımsal salt okunur koruma, donanımsal yazma engelleyiciyle eşdeğer değildir.",
    ));
    ui.label(theme::muted(
        "• Yazma koruması doğrulanamıyorsa uygulama sizi uyarır ve edinimi engeller.",
    ));
    ui.label(theme::muted(
        "• Kaynak hiçbir koşulda otomatik biçimlendirilmez veya onarılmaz.",
    ));
}
