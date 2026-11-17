//! Arayüz teması ve ortak görsel öğeler (Türkçe).

use egui::{Color32, RichText};

/// DelilX renk paleti (adli/kurumsal mavi-gri).
pub struct Theme;

impl Theme {
    /// Ana koyu renk (başlık çubuğu).
    pub const PRIMARY: Color32 = Color32::from_rgb(21, 44, 74);
    /// İkincil renk (seçili öğe).
    pub const ACCENT: Color32 = Color32::from_rgb(38, 96, 168);
    /// Başarı (yeşil).
    pub const SUCCESS: Color32 = Color32::from_rgb(36, 138, 74);
    /// Uyarı (turuncu).
    pub const WARNING: Color32 = Color32::from_rgb(200, 120, 20);
    /// Hata (kırmızı).
    pub const ERROR: Color32 = Color32::from_rgb(184, 44, 44);
    /// Nötr metin.
    pub const MUTED: Color32 = Color32::from_rgb(120, 128, 140);
    /// Arka plan.
    pub const BACKGROUND: Color32 = Color32::from_rgb(244, 246, 249);
    /// Panel arka planı.
    pub const PANEL: Color32 = Color32::from_rgb(255, 255, 255);

    /// egui görsel stilini uygular.
    pub fn apply(ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.visuals.panel_fill = Self::BACKGROUND;
        style.visuals.window_fill = Self::PANEL;
        style.visuals.extreme_bg_color = Color32::from_rgb(235, 238, 242);
        style.visuals.widgets.active.bg_fill = Self::ACCENT;
        style.visuals.widgets.hovered.bg_fill = Self::ACCENT.gamma_multiply(0.85);
        style.visuals.selection.bg_fill = Self::ACCENT;
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        ctx.set_style(style);
    }
}

/// Bölüm başlığı üretir.
pub fn heading(text: &str) -> RichText {
    RichText::new(text)
        .size(20.0)
        .strong()
        .color(Theme::PRIMARY)
}

/// Alt başlık üretir.
pub fn subheading(text: &str) -> RichText {
    RichText::new(text).size(15.0).strong().color(Theme::ACCENT)
}

/// Açıklama metni üretir.
pub fn muted(text: &str) -> RichText {
    RichText::new(text).size(12.5).color(Theme::MUTED)
}

/// Başarı mesajı üretir.
pub fn success(text: &str) -> RichText {
    RichText::new(text).color(Theme::SUCCESS).strong()
}

/// Uyarı mesajı üretir.
pub fn warning(text: &str) -> RichText {
    RichText::new(text).color(Theme::WARNING)
}

/// Hata mesajı üretir.
pub fn error(text: &str) -> RichText {
    RichText::new(text).color(Theme::ERROR).strong()
}
