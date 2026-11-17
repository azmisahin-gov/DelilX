//! DelilX ana uygulama yapısı ve gezinme.

use crate::screens;
use crate::state::{AppState, Screen};
use crate::theme::{self, Theme};
use eframe::egui;
use std::sync::{Arc, Mutex};

/// DelilX uygulaması.
pub struct DelilXApp {
    /// Paylaşılan durum.
    pub state: Arc<Mutex<AppState>>,
    /// İlk açılışta cihaz algılandı mı?
    initialized: bool,
}

impl DelilXApp {
    /// Yeni uygulama oluşturur.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Theme::apply(&cc.egui_ctx);
        DelilXApp {
            state: Arc::new(Mutex::new(AppState::new())),
            initialized: false,
        }
    }
}

impl eframe::App for DelilXApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // İlerleme kanalını boşalt
        {
            let mut s = self.state.lock().unwrap();
            let mut last_progress = None;
            if let Some(rx) = &s.job.receiver {
                while let Ok(p) = rx.try_recv() {
                    last_progress = Some(p);
                }
            }
            if let Some(p) = last_progress {
                s.job.progress = Some(p);
            }
        }

        // İlk açılışta cihazları algıla
        if !self.initialized {
            self.initialized = true;
            screens::devices::refresh_devices(&self.state);
        }

        // İşlem sürüyorsa sürekli yeniden çiz
        {
            let s = self.state.lock().unwrap();
            if s.job.running {
                ctx.request_repaint_after(std::time::Duration::from_millis(150));
            }
        }

        // Üst başlık
        egui::TopBottomPanel::top("ust_baslik").show(ctx, |ui| {
            self.draw_header(ui);
        });

        // Sol menü
        egui::SidePanel::left("yan_menu")
            .resizable(false)
            .exact_width(230.0)
            .show(ctx, |ui| {
                self.draw_sidebar(ui);
            });

        // Alt durum çubuğu
        egui::TopBottomPanel::bottom("durum_cubugu").show(ctx, |ui| {
            self.draw_status_bar(ui);
        });

        // Orta içerik
        egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_content(ui);
        });
    }
}

impl DelilXApp {
    /// Üst başlık çubuğunu çizer.
    fn draw_header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("🛡  DELİLX")
                    .size(24.0)
                    .strong()
                    .color(Theme::PRIMARY),
            );
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new("Dijital Delili Koru, Doğrula ve İncele.")
                    .size(13.0)
                    .color(Theme::MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("Sürüm {}", delilx_core::version()))
                        .size(12.0)
                        .color(Theme::MUTED),
                );
            });
        });
        ui.add_space(4.0);
    }

    /// Sol menüyü çizer.
    fn draw_sidebar(&self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        let current = self.state.lock().unwrap().current;

        // Bölümler
        let groups: [(&str, &[Screen]); 4] = [
            (
                "GENEL",
                &[Screen::Home, Screen::Devices, Screen::Acquisition],
            ),
            (
                "VERİ EDİNİMİ",
                &[
                    Screen::Raw,
                    Screen::E01,
                    Screen::Logical,
                    Screen::Selective,
                    Screen::UsedSpace,
                ],
            ),
            ("İNCELEME", &[Screen::Inspect, Screen::Verify]),
            ("RAPORLAMA", &[Screen::Reports, Screen::Settings]),
        ];

        let mut new_screen = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (group, items) in groups {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(group)
                        .size(11.0)
                        .strong()
                        .color(Theme::MUTED),
                );
                for screen in items {
                    let selected = current == *screen;
                    let text = format!("  {}  {}", screen.icon(), screen.title());
                    let button =
                        egui::SelectableLabel::new(selected, egui::RichText::new(text).size(14.5));
                    if ui.add_sized([210.0, 30.0], button).clicked() {
                        new_screen = Some(*screen);
                    }
                }
            }
        });

        if let Some(s) = new_screen {
            let mut state = self.state.lock().unwrap();
            state.current = s;
        }
    }

    /// İçerik alanını çizer.
    fn draw_content(&self, ui: &mut egui::Ui) {
        let screen = self.state.lock().unwrap().current;
        ui.add_space(6.0);
        ui.label(theme::heading(screen.title()));
        ui.separator();
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match screen {
                Screen::Home => screens::home::draw(ui, &self.state),
                Screen::Devices => screens::devices::draw(ui, &self.state),
                Screen::Acquisition => screens::acquisition::draw(ui, &self.state),
                Screen::Raw => screens::raw::draw(ui, &self.state),
                Screen::E01 => screens::e01::draw(ui, &self.state),
                Screen::Logical => screens::logical::draw(ui, &self.state, false),
                Screen::Selective => screens::logical::draw(ui, &self.state, true),
                Screen::UsedSpace => screens::usedspace::draw(ui, &self.state),
                Screen::Inspect => screens::inspect::draw(ui, &self.state),
                Screen::Verify => screens::verify::draw(ui, &self.state),
                Screen::Reports => screens::reports::draw(ui, &self.state),
                Screen::Settings => screens::settings::draw(ui, &self.state),
            });
    }

    /// Alt durum çubuğunu çizer.
    fn draw_status_bar(&self, ui: &mut egui::Ui) {
        let s = self.state.lock().unwrap();
        ui.horizontal(|ui| {
            let (icon, color) = match s.status_kind {
                1 => ("✔", Theme::SUCCESS),
                2 => ("⚠", Theme::WARNING),
                3 => ("✖", Theme::ERROR),
                _ => ("ℹ", Theme::ACCENT),
            };
            ui.label(egui::RichText::new(icon).color(color).strong());
            ui.label(egui::RichText::new(&s.status).size(12.5));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if s.job.running {
                    ui.label(
                        egui::RichText::new(format!("{} çalışıyor...", s.job.name))
                            .color(Theme::ACCENT),
                    );
                    if let Some(p) = &s.job.progress {
                        if let Some(frac) = p.fraction() {
                            ui.add(
                                egui::ProgressBar::new(frac as f32)
                                    .desired_width(200.0)
                                    .text(p.percent_text()),
                            );
                        }
                    }
                }
            });
        });
    }
}
