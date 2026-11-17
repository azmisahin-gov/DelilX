//! Ayarlar ekranı.

use crate::screens::State;
use crate::theme::{self, Theme};
use eframe::egui;

/// Ayarlar ekranını çizer.
pub fn draw(ui: &mut egui::Ui, state: &State) {
    ui.label(theme::muted("Varsayılan değerler ve uygulama bilgileri."));
    ui.add_space(10.0);

    ui.label(theme::subheading("Varsayılan Delil Bilgileri"));
    {
        let mut s = state.lock().unwrap();
        egui::Grid::new("ayar_grid")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Varsayılan delil numarası");
                ui.add(
                    egui::TextEdit::singleline(&mut s.settings.default_evidence_number)
                        .desired_width(320.0),
                );
                ui.end_row();
                ui.label("Varsayılan vaka numarası");
                ui.add(
                    egui::TextEdit::singleline(&mut s.settings.default_case_number)
                        .desired_width(320.0),
                );
                ui.end_row();
                ui.label("Varsayılan incelemeyi yapan");
                ui.add(
                    egui::TextEdit::singleline(&mut s.settings.default_examiner)
                        .desired_width(320.0),
                );
                ui.end_row();
                ui.label("Varsayılan sektör boyutu");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut s.settings.bytes_per_sector, 512u32, "512 bayt");
                    ui.radio_value(&mut s.settings.bytes_per_sector, 4096u32, "4096 bayt");
                });
                ui.end_row();
            });
    }

    ui.add_space(12.0);
    ui.label(theme::subheading("Hash Algoritmaları"));
    {
        let mut s = state.lock().unwrap();
        let mut sha256 = s
            .settings
            .hash_algorithms
            .contains(&delilx_core::hash::HashAlgorithm::Sha256);
        let mut sha512 = s
            .settings
            .hash_algorithms
            .contains(&delilx_core::hash::HashAlgorithm::Sha512);
        ui.horizontal(|ui| {
            if ui.checkbox(&mut sha256, "SHA-256").changed()
                || ui.checkbox(&mut sha512, "SHA-512").changed()
            {
                let mut algs = Vec::new();
                if sha256 {
                    algs.push(delilx_core::hash::HashAlgorithm::Sha256);
                }
                if sha512 {
                    algs.push(delilx_core::hash::HashAlgorithm::Sha512);
                }
                if algs.is_empty() {
                    algs.push(delilx_core::hash::HashAlgorithm::Sha256);
                }
                s.settings.hash_algorithms = algs;
            }
        });
        ui.add_space(4.0);
        ui.label(theme::muted(
            "E01 imajları biçim gereği her zaman MD5 ve SHA-1 değerlerini de içerir.",
        ));
    }

    ui.add_space(12.0);
    ui.label(theme::subheading("Doğrulama"));
    {
        let mut s = state.lock().unwrap();
        ui.checkbox(
            &mut s.settings.verify_by_default,
            "Edinim sonrası otomatik doğrulama yap (önerilir)",
        );
    }

    ui.add_space(12.0);
    ui.label(theme::subheading("Uygulama Bilgileri"));
    egui::Frame::group(ui.style())
        .fill(Theme::PANEL)
        .inner_margin(egui::Margin::same(14.0))
        .show(ui, |ui| {
            egui::Grid::new("hakkinda_grid")
                .num_columns(2)
                .spacing([20.0, 5.0])
                .show(ui, |ui| {
                    let rows = [
                        ("Uygulama", "DelilX".to_string()),
                        ("Sürüm", delilx_core::version().to_string()),
                        ("Slogan", delilx_core::APP_SLOGAN.to_string()),
                        ("Platform", platform_name().to_string()),
                        ("Dil", "Türkçe".to_string()),
                        (
                            "E01 uyumluluğu",
                            "EnCase 6 (bağımsız araçlarla doğrulanır)".to_string(),
                        ),
                        (
                            "Kaynak bellek koruma",
                            "Kaynağa yazma yapılmaz; yalnızca okuma erişimi".to_string(),
                        ),
                    ];
                    for (k, v) in rows {
                        ui.label(egui::RichText::new(k).strong());
                        ui.label(v);
                        ui.end_row();
                    }
                });
        });

    ui.add_space(12.0);
    ui.label(theme::subheading("Güvenlik İlkeleri"));
    for text in [
        "Kaynak cihaza hiçbir koşulda yazma işlemi yapılmaz.",
        "Yazılımsal salt okunur koruma, donanımsal yazma engelleyiciyle eşdeğer gösterilmez.",
        "Kaynak ve hedef diskler ayırt edilir; aynı aygıt seçilemez.",
        "Kaynak hiçbir koşulda otomatik biçimlendirilmez veya onarılmaz.",
        "Hash değerleri ile metadata bütünlüğü ayrı değerlendirilir.",
    ] {
        ui.label(theme::muted(&format!("• {text}")));
    }
}

/// Platform adını döndürür.
fn platform_name() -> &'static str {
    if cfg!(windows) {
        "Windows (x64)"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Linux"
    }
}
