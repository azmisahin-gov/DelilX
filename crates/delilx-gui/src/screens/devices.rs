//! Cihaz algılama ekranı.

use crate::screens::State;
use crate::theme::{self, Theme};
use eframe::egui;

/// Cihazları yeniden algılar ve durumu günceller.
pub fn refresh_devices(state: &State) {
    match delilx_core::device::detect_devices() {
        Ok(devices) => {
            let count = devices.len();
            let mut s = state.lock().unwrap();
            s.devices = devices;
            s.selected_device = None;
            if count == 0 {
                s.set_status(
                    2,
                    "Depolama aygıtı algılanamadı. Windows dışı ortamlarda cihaz algılama sınırlıdır.",
                );
            } else {
                s.set_status(1, format!("{count} depolama aygıtı algılandı."));
            }
            s.log_info(format!("Cihaz algılama tamamlandı: {count} aygıt."));
        }
        Err(e) => {
            let mut s = state.lock().unwrap();
            s.set_status(3, format!("Cihaz algılama hatası: {e}"));
            s.log_error(format!("Cihaz algılama hatası: {e}"));
        }
    }
}

/// Cihaz ekranını çizer.
pub fn draw(ui: &mut egui::Ui, state: &State) {
    ui.horizontal(|ui| {
        if ui.button("🔄  Cihazları Yeniden Tara").clicked() {
            refresh_devices(state);
        }
        ui.label(theme::muted(
            "USB bellekler ve SD kartlar otomatik algılanır.",
        ));
    });
    ui.add_space(8.0);

    let devices = {
        let s = state.lock().unwrap();
        s.devices.clone()
    };

    if devices.is_empty() {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(theme::warning(
                "Algılanan depolama aygıtı yok veya bu platformda cihaz algılama desteklenmiyor.",
            ));
            ui.add_space(4.0);
            ui.label(theme::muted(
                "Not: Fiziksel disk algılama ve yazma koruması kontrolü Windows üzerinde \
                 tam olarak çalışır. Kaynak olarak bir dosya yolu da girebilirsiniz.",
            ));
        });
        return;
    }

    let mut selected = {
        let s = state.lock().unwrap();
        s.selected_device
    };

    egui::Grid::new("cihaz_grid")
        .num_columns(6)
        .striped(true)
        .spacing([14.0, 8.0])
        .show(ui, |ui| {
            for h in [
                "",
                "Aygıt",
                "Tür",
                "Kapasite",
                "Dosya Sistemi",
                "Yazma Koruması",
            ] {
                ui.label(egui::RichText::new(h).strong().color(Theme::PRIMARY));
            }
            ui.end_row();

            for (i, d) in devices.iter().enumerate() {
                let mut is_sel = selected == Some(i);
                if ui.checkbox(&mut is_sel, "").changed() {
                    selected = if is_sel { Some(i) } else { None };
                }
                ui.label(&d.display_name);
                ui.label(d.kind.label());
                ui.label(d.capacity_text());
                ui.label(d.filesystem.as_deref().unwrap_or("-"));
                let wp = d.write_protection.label();
                let color = if d.write_protection.is_protected() {
                    Theme::SUCCESS
                } else {
                    Theme::WARNING
                };
                ui.label(egui::RichText::new(wp).color(color));
                ui.end_row();
            }
        });

    {
        let mut s = state.lock().unwrap();
        s.selected_device = selected;
    }

    ui.add_space(12.0);

    if let Some(i) = selected {
        let d = &devices[i];
        egui::Frame::group(ui.style())
            .fill(Theme::PANEL)
            .inner_margin(egui::Margin::same(14.0))
            .show(ui, |ui| {
                ui.label(theme::subheading("Seçili Aygıt Ayrıntıları"));
                ui.add_space(4.0);
                egui::Grid::new("detay_grid")
                    .num_columns(2)
                    .spacing([20.0, 5.0])
                    .show(ui, |ui| {
                        let rows = [
                            ("Görünen ad", d.display_name.clone()),
                            ("Tür", d.kind.label().to_string()),
                            ("Kapasite", d.capacity_text()),
                            (
                                "Boş alan",
                                delilx_core::progress::format_bytes(d.free_bytes),
                            ),
                            (
                                "Seri numarası",
                                d.serial.clone().unwrap_or_else(|| "-".to_string()),
                            ),
                            (
                                "Dosya sistemi",
                                d.filesystem.clone().unwrap_or_else(|| "-".to_string()),
                            ),
                            (
                                "Fiziksel disk",
                                d.physical_drive_number
                                    .map(|n| format!("Fiziksel Disk {n}"))
                                    .unwrap_or_else(|| "-".to_string()),
                            ),
                            ("Sektör boyutu", format!("{} bayt", d.bytes_per_sector)),
                            ("Yazma koruması", d.write_protection.label().to_string()),
                            ("Koruma ayrıntısı", d.write_protection_detail.clone()),
                        ];
                        for (k, v) in rows {
                            ui.label(egui::RichText::new(k).strong());
                            ui.label(v);
                            ui.end_row();
                        }
                    });

                if let Some(w) = &d.source_warning {
                    ui.add_space(6.0);
                    ui.label(theme::warning(w));
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Bu Aygıtı Kaynak Olarak Seç").clicked() {
                        let mut s = state.lock().unwrap();
                        if let Some(path) = d.source_path() {
                            s.source_path = path.to_string_lossy().to_string();
                        }
                        s.set_status(1, format!("Kaynak seçildi: {}", d.display_name));
                    }
                });
            });
    }
}
