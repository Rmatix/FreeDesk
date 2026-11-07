//! High-definition vector-drawn icons for FreeDesk UI.
//!
//! Rather than relying on system emoji fonts (which cause missing glyph "□" boxes
//! on different Windows versions), these vector routines paint crisp, resolution-independent
//! icons using egui's native 2D Painter.

use eframe::egui::{Color32, Painter, Pos2, Rect, Rounding, Stroke};

/// Draws a crisp PC monitor / Desktop icon (AnyDesk style).
pub fn draw_pc_monitor(painter: &Painter, rect: Rect, border_color: Color32, fill_color: Color32) {
    let w = rect.width();
    let h = rect.height();

    // Monitor screen (upper 75% of area)
    let screen_h = h * 0.72;
    let screen_rect = Rect::from_min_size(
        rect.min,
        eframe::egui::Vec2::new(w, screen_h),
    );

    // Screen body & bezel
    painter.rect_filled(screen_rect, Rounding::same(3.5), fill_color);
    painter.rect_stroke(screen_rect, Rounding::same(3.5), Stroke::new(1.5, border_color));

    // Stand neck
    let stem_w = (w * 0.22).max(3.0);
    let stem_x = rect.center().x - stem_w / 2.0;
    let stem_y = screen_rect.max.y;
    let stem_h = h * 0.16;
    let stem_rect = Rect::from_min_size(
        Pos2::new(stem_x, stem_y),
        eframe::egui::Vec2::new(stem_w, stem_h),
    );
    painter.rect_filled(stem_rect, Rounding::same(1.0), border_color);

    // Stand base foot
    let foot_w = (w * 0.55).max(6.0);
    let foot_x = rect.center().x - foot_w / 2.0;
    let foot_y = stem_rect.max.y;
    let foot_h = (h * 0.12).max(2.5);
    let foot_rect = Rect::from_min_size(
        Pos2::new(foot_x, foot_y),
        eframe::egui::Vec2::new(foot_w, foot_h),
    );
    painter.rect_filled(foot_rect, Rounding::same(2.0), border_color);
}

/// Draws a Remote PC connection icon (monitor with connection arrow).
pub fn draw_remote_desk_icon(painter: &Painter, rect: Rect, color: Color32) {
    let sub_w = rect.width() * 0.75;
    let sub_h = rect.height() * 0.75;
    let pc_rect = Rect::from_min_size(rect.min, eframe::egui::Vec2::new(sub_w, sub_h));
    draw_pc_monitor(painter, pc_rect, color, Color32::from_rgb(28, 32, 40));

    // Connection arrow in bottom-right
    let arrow_tip = rect.max;
    let arrow_size = rect.width() * 0.35;
    let p_tip = Pos2::new(arrow_tip.x, arrow_tip.y - 2.0);
    let p_left = Pos2::new(arrow_tip.x - arrow_size, arrow_tip.y - 2.0);
    let p_top = Pos2::new(arrow_tip.x - 2.0, arrow_tip.y - arrow_size);

    painter.add(eframe::egui::Shape::convex_polygon(
        vec![p_tip, p_left, p_top],
        Color32::from_rgb(230, 57, 70), // FreeDesk crimson accent
        Stroke::new(1.0, Color32::WHITE),
    ));
}

/// Draws a security shield icon.
pub fn draw_shield_icon(painter: &Painter, rect: Rect, color: Color32) {
    let top_l = Pos2::new(rect.left(), rect.top() + rect.height() * 0.2);
    let top_m = Pos2::new(rect.center().x, rect.top());
    let top_r = Pos2::new(rect.right(), rect.top() + rect.height() * 0.2);
    let mid_r = Pos2::new(rect.right(), rect.top() + rect.height() * 0.65);
    let bot_tip = Pos2::new(rect.center().x, rect.bottom());
    let mid_l = Pos2::new(rect.left(), rect.top() + rect.height() * 0.65);

    let points = vec![top_m, top_r, mid_r, bot_tip, mid_l, top_l];
    painter.add(eframe::egui::Shape::convex_polygon(
        points.clone(),
        Color32::from_rgba_premultiplied(color.r() / 4, color.g() / 4, color.b() / 4, 180),
        Stroke::new(1.5, color),
    ));
}
