//! Veri edinimi genel bakış ekranı.

use crate::screens::State;
use crate::state::Screen;
use crate::theme::{self, Theme};
use eframe::egui;

/// Veri edinimi ekranını çizer.
pub fn draw(ui: &mut egui::Ui, state: &State) {
    ui.label(theme::muted(
        "DelilX aşağıdaki edinim yöntemlerini destekler. Bir yöntem seçin.",
    ));
    ui.add_space(10.0);

    let mut go = None;
    let methods: [(&str, &str, &str, Screen); 5] = [
        (
            "💽",
            "RAW / DD",
            "USB ve SD kartların tam sektör imajını alır. Sektör sektör okuma, okuma hata kaydı, \
             SHA-256 ve SHA-512 doğrulaması. Silinmiş veriler korunur.",
            Screen::Raw,
        ),
        (
            "🗄",
            "E01 (EnCase)",
            "Gerçek EnCase uyumlu E01 imajı oluşturur. Sıkıştırma, parçalara ayırma, delil ve \
             uzman bilgileri, hash doğrulaması. Silinmiş veriler korunur.",
            Screen::E01,
        ),
        (
            "📁",
            "Mantıksal Edinim",
            "Yalnızca mevcut dosyaları alır. Klasör yapısı korunur, dosya metadata'sı ve \
             SHA-256/SHA-512 hesaplanır.",
            Screen::Logical,
        ),
        (
            "☑",
            "Seçmeli Edinim",
            "Kullanıcının seçtiği dosya ve klasörleri alır. Dosya envanteri oluşturulur.",
            Screen::Selective,
        ),
        (
            "🧩",
            "Kullanılan Alan",
            "Yalnızca kullanılan sektörleri alır. UYARI: boş alan dışarıda bırakıldığı için \
             silinmiş veriler kaybolabilir.",
            Screen::UsedSpace,
        ),
    ];

    for (icon, title, desc, screen) in methods {
        egui::Frame::group(ui.style())
            .fill(Theme::PANEL)
            .inner_margin(egui::Margin::same(14.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon).size(28.0));
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(title)
                                .size(17.0)
                                .strong()
                                .color(Theme::PRIMARY),
                        );
                        ui.label(egui::RichText::new(desc).size(13.0));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Aç").clicked() {
                            go = Some(screen);
                        }
                    });
                });
            });
        ui.add_space(8.0);
    }

    if let Some(s) = go {
        state.lock().unwrap().current = s;
    }
}
