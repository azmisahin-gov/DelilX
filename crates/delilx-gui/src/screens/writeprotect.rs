//! Yazma koruması (yazılımsal write blocker) ekranı.
//!
//! Kullanıcı bir USB belleği veya SD kartı seçer, yazılımsal yazma korumasını
//! etkinleştirir veya kaldırır ve korumanın gerçekten etkin olup olmadığını
//! görür.

use crate::screens::{widgets, State};
use crate::theme::{self, Theme};
use crate::worker;
use delilx_core::writeblock::{self, ProtectionSource, SOFTWARE_NOT_EQUIVALENT_WARNING};
use eframe::egui;

/// Yazma koruması ekranını çizer.
pub fn draw(ui: &mut egui::Ui, state: &State) {
    ui.label(theme::muted(
        "Harici bir donanımsal yazma engelleyici kullanmadan, yalnızca yazılım üzerinden \
         USB bellek veya SD kartı yazmaya karşı korur. Windows disk salt-okunur özniteliği \
         kullanılır.",
    ));
    ui.add_space(8.0);

    widgets::warning_box(ui, SOFTWARE_NOT_EQUIVALENT_WARNING);
    ui.add_space(10.0);

    // Cihaz seçimi
    ui.label(theme::subheading("Korunacak Aygıt"));
    let devices = state.lock().unwrap().devices.clone();

    let mut device_path = state.lock().unwrap().write_protect.device_path.clone();

    if devices.is_empty() {
        ui.label(theme::warning(
            "Algılanan depolama aygıtı yok. Önce 'Cihazları Algıla' ekranından tarama yapın.",
        ));
    } else {
        egui::ComboBox::from_label("Aygıt seçin")
            .selected_text(if device_path.is_empty() {
                "— seçilmedi —".to_string()
            } else {
                device_path.clone()
            })
            .width(520.0)
            .show_ui(ui, |ui| {
                for d in &devices {
                    let path = d
                        .physical_path
                        .clone()
                        .or_else(|| d.drive_letter.clone())
                        .unwrap_or_default();
                    if path.is_empty() {
                        continue;
                    }
                    let label = format!(
                        "{} — {} ({})",
                        d.display_name,
                        d.kind.label(),
                        d.capacity_text()
                    );
                    ui.selectable_value(&mut device_path, path.clone(), label);
                }
            });
    }

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Aygıt yolu:").strong());
        ui.add(
            egui::TextEdit::singleline(&mut device_path)
                .desired_width(420.0)
                .hint_text("\\\\.\\PhysicalDrive2"),
        );
    });

    state.lock().unwrap().write_protect.device_path = device_path.clone();

    ui.add_space(10.0);
    let running = state.lock().unwrap().job.running;
    let has_device = !device_path.is_empty();

    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !running && has_device,
                egui::Button::new("🔄  Durumu Sorgula"),
            )
            .clicked()
        {
            do_query(state, device_path.clone());
        }
        if ui
            .add_enabled(
                !running && has_device,
                egui::Button::new("🔒  Yazma Korumasını Etkinleştir"),
            )
            .clicked()
        {
            do_set(state, device_path.clone(), true);
        }
        if ui
            .add_enabled(
                !running && has_device,
                egui::Button::new("🔓  Yazma Korumasını Kaldır"),
            )
            .clicked()
        {
            do_set(state, device_path.clone(), false);
        }
    });

    widgets::progress_bar(ui, state);

    // Sonuç raporu
    let report = state.lock().unwrap().write_protect.report.clone();
    if let Some(r) = report {
        ui.add_space(12.0);
        egui::Frame::group(ui.style())
            .fill(Theme::PANEL)
            .inner_margin(egui::Margin::same(14.0))
            .show(ui, |ui| {
                ui.label(theme::subheading("Koruma Durumu"));

                // Durum rozetini renkle göster
                let (icon, color, text) = status_badge(&r);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon).size(22.0).color(color));
                    ui.label(egui::RichText::new(text).size(16.0).strong().color(color));
                });

                ui.add_space(6.0);
                egui::Grid::new("wp_grid")
                    .num_columns(2)
                    .spacing([20.0, 5.0])
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Koruma kaynağı").strong());
                        ui.label(r.source.label());
                        ui.end_row();

                        ui.label(egui::RichText::new("Salt-okunur özniteliği").strong());
                        ui.label(match r.read_only_attribute {
                            Some(true) => "Etkin",
                            Some(false) => "Kapalı",
                            None => "Sorgulanamadı",
                        });
                        ui.end_row();

                        ui.label(egui::RichText::new("Yazılabilir mi").strong());
                        ui.label(match r.writable {
                            Some(true) => "Evet (yazılabilir)",
                            Some(false) => "Hayır (yazmaya kapalı)",
                            None => "Sorgulanamadı",
                        });
                        ui.end_row();

                        if let Some(v) = r.verified {
                            ui.label(egui::RichText::new("Doğrulama").strong());
                            let (vt, vc) = if v {
                                ("Koruma etkin ve doğrulandı", Theme::SUCCESS)
                            } else {
                                ("KORUMA DOĞRULANAMADI", Theme::ERROR)
                            };
                            ui.label(egui::RichText::new(vt).color(vc).strong());
                            ui.end_row();
                        }

                        ui.label(egui::RichText::new("Açıklama").strong());
                        ui.label(&r.detail);
                        ui.end_row();
                    });

                // Donanımsal koruma değilse uyar
                if r.source == ProtectionSource::Software {
                    ui.add_space(6.0);
                    ui.label(theme::warning(SOFTWARE_NOT_EQUIVALENT_WARNING));
                }
            });
    }

    // İşlem mesajı
    let (message, success) = {
        let s = state.lock().unwrap();
        (s.write_protect.message.clone(), s.write_protect.success)
    };
    if let Some(m) = message {
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
                    theme::success("İşlem tamamlandı")
                } else {
                    theme::error("İşlem başarısız")
                });
                ui.add_space(4.0);
                ui.label(egui::RichText::new(m).size(12.5));
            });
    }

    ui.add_space(12.0);
    ui.label(theme::muted(
        "Not: Yazma koruması, cihaz çıkarılana veya sistem yeniden başlatılana kadar etkindir. \
         Bu, delil cihazının kalıcı olarak değiştirilmemesi içindir. Sistem diski korunamaz.",
    ));

    ui.add_space(12.0);
    widgets::log_viewer(ui, state);
}

/// Durum rozetini (simge, renk, metin) döndürür.
fn status_badge(
    r: &writeblock::WriteProtectionReport,
) -> (&'static str, egui::Color32, &'static str) {
    if r.is_effective() {
        ("🔒", Theme::SUCCESS, "Yazmaya kapalı — koruma etkin")
    } else {
        match r.source {
            ProtectionSource::None => ("🔓", Theme::WARNING, "Yazılabilir — koruma yok"),
            ProtectionSource::Unknown => ("❓", Theme::ERROR, "Durum belirlenemedi"),
            _ => ("⚠", Theme::ERROR, "Koruma doğrulanamadı"),
        }
    }
}

/// Durum sorgusu yapar.
fn do_query(state: &State, device: String) {
    {
        let mut s = state.lock().unwrap();
        s.log_info(format!("Yazma koruması sorgulanıyor: {device}"));
    }
    worker::spawn(
        state.clone(),
        "Yazma Koruması Sorgu",
        move |_cb, _cancel, st| {
            let report = writeblock::query(&device).map_err(|e| format!("{e}"))?;
            let mut s = st.lock().unwrap();
            s.write_protect.message = Some(report.detail.clone());
            s.write_protect.success = true;
            s.write_protect.report = Some(report);
            Ok("Durum sorgulandı.".to_string())
        },
    );
}

/// Korumayı etkinleştirir veya kaldırır ve doğrular.
fn do_set(state: &State, device: String, enable: bool) {
    let name = if enable {
        "Yazma Koruması Etkinleştir"
    } else {
        "Yazma Koruması Kaldır"
    };
    {
        let mut s = state.lock().unwrap();
        s.log_info(format!("{name}: {device}"));
    }
    worker::spawn(state.clone(), name, move |_cb, _cancel, st| {
        let report = if enable {
            writeblock::enable_and_verify(&device)
        } else {
            writeblock::disable_and_verify(&device)
        }
        .map_err(|e| format!("{e}"))?;

        let effective = report.is_effective();
        let mut s = st.lock().unwrap();
        s.write_protect.message = Some(report.detail.clone());
        s.write_protect.success = if enable { effective } else { true };
        s.write_protect.report = Some(report);
        if enable && !effective {
            return Err(
                "Koruma etkinleştirildi ancak doğrulanamadı. Cihaz hâlâ yazılabilir olabilir."
                    .to_string(),
            );
        }
        Ok(if enable {
            "Yazma koruması etkinleştirildi ve doğrulandı.".to_string()
        } else {
            "Yazma koruması kaldırıldı.".to_string()
        })
    });
}
