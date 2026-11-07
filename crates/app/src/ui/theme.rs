//! Visual style and theme for FreeDesk (AnyDesk-inspired dark & red).

use eframe::egui::{Color32, FontId, Rounding, Stroke, TextStyle, Visuals};

pub const ACCENT_RED: Color32 = Color32::from_rgb(230, 57, 70);
pub const ACCENT_RED_HOVER: Color32 = Color32::from_rgb(245, 75, 88);
pub const ACCENT_RED_ACTIVE: Color32 = Color32::from_rgb(215, 45, 58);

pub const BG_DARK: Color32 = Color32::from_rgb(18, 20, 26);
pub const BG_HEADER: Color32 = Color32::from_rgb(24, 27, 36);
pub const BG_PANEL: Color32 = Color32::from_rgb(28, 32, 42);
pub const BG_CARD: Color32 = Color32::from_rgb(34, 39, 52);
pub const BG_CARD_HOVER: Color32 = Color32::from_rgb(42, 48, 64);
pub const BG_INPUT: Color32 = Color32::from_rgb(20, 23, 30);

pub const BORDER_CARD: Color32 = Color32::from_rgb(48, 55, 72);
pub const BORDER_FOCUS: Color32 = Color32::from_rgb(230, 57, 70);

pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(248, 249, 250);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(185, 192, 205);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(130, 138, 155);

pub const SUCCESS_GREEN: Color32 = Color32::from_rgb(46, 204, 113);
pub const WARNING_AMBER: Color32 = Color32::from_rgb(241, 196, 15);

pub fn apply_theme(ctx: &eframe::egui::Context) {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.panel_fill = BG_DARK;
    visuals.window_fill = BG_PANEL;
    visuals.window_stroke = Stroke::new(1.0, BORDER_CARD);
    visuals.window_rounding = Rounding::same(12.0);

    // Button and widget styling
    visuals.widgets.inactive.bg_fill = BG_CARD;
    visuals.widgets.inactive.rounding = Rounding::same(8.0);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_CARD);

    visuals.widgets.hovered.bg_fill = ACCENT_RED_HOVER;
    visuals.widgets.hovered.rounding = Rounding::same(8.0);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_RED);

    visuals.widgets.active.bg_fill = ACCENT_RED_ACTIVE;
    visuals.widgets.active.rounding = Rounding::same(8.0);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);

    visuals.widgets.open.bg_fill = BG_CARD_HOVER;
    visuals.widgets.open.rounding = Rounding::same(8.0);

    // Input fields (TextEdit)
    visuals.extreme_bg_color = BG_INPUT;

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = eframe::egui::Vec2::new(10.0, 10.0);
    style.spacing.window_margin = eframe::egui::Margin::same(14.0);
    style.spacing.button_padding = eframe::egui::Vec2::new(10.0, 6.0);

    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(22.0, eframe::egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(15.0, eframe::egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(14.0, eframe::egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(15.0, eframe::egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(13.0, eframe::egui::FontFamily::Proportional),
    );
    ctx.set_style(style);
}
