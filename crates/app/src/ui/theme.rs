//! Visual style and theme for FreeDesk (AnyDesk-inspired dark & red).

use eframe::egui::{Color32, FontId, Rounding, Stroke, TextStyle, Visuals};

pub const ACCENT_RED: Color32 = Color32::from_rgb(230, 57, 70);
pub const ACCENT_RED_HOVER: Color32 = Color32::from_rgb(240, 75, 88);
pub const BG_DARK: Color32 = Color32::from_rgb(24, 26, 31);
pub const BG_PANEL: Color32 = Color32::from_rgb(33, 37, 46);
pub const BG_CARD: Color32 = Color32::from_rgb(44, 49, 60);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(245, 245, 247);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(150, 155, 170);
pub const SUCCESS_GREEN: Color32 = Color32::from_rgb(46, 204, 113);

pub fn apply_theme(ctx: &eframe::egui::Context) {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.panel_fill = BG_DARK;
    visuals.window_fill = BG_PANEL;
    visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(60, 65, 80));
    visuals.window_rounding = Rounding::same(10.0);

    // Button styling
    visuals.widgets.inactive.bg_fill = BG_CARD;
    visuals.widgets.inactive.rounding = Rounding::same(6.0);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    visuals.widgets.hovered.bg_fill = ACCENT_RED_HOVER;
    visuals.widgets.hovered.rounding = Rounding::same(6.0);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);

    visuals.widgets.active.bg_fill = ACCENT_RED;
    visuals.widgets.active.rounding = Rounding::same(6.0);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(20.0, eframe::egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(14.0, eframe::egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(13.0, eframe::egui::FontFamily::Monospace),
    );
    ctx.set_style(style);
}
