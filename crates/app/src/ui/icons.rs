//! High-definition vector-drawn icons and app logo for FreeDesk UI.
//!
//! Rather than relying on system emoji fonts (which cause missing glyph "□" boxes
//! on different Windows versions), these vector routines paint crisp, resolution-independent
//! icons using egui's native 2D Painter, and generates the native OS window icon.

use eframe::egui::{Color32, IconData, Painter, Pos2, Rect, Rounding, Stroke, Vec2};

/// Draws the official FreeDesk logo emblem (interlocking dual-screen remote desktops).
pub fn draw_freedesk_logo(painter: &Painter, rect: Rect) {
    let w = rect.width();
    let h = rect.height();

    // Back desktop screen (Upper-Left)
    let back_w = w * 0.62;
    let back_h = h * 0.62;
    let back_rect = Rect::from_min_size(rect.min, Vec2::new(back_w, back_h));
    painter.rect_filled(
        back_rect,
        Rounding::same(6.0),
        Color32::from_rgb(215, 38, 56), // Deep crimson
    );
    painter.rect_stroke(
        back_rect,
        Rounding::same(6.0),
        Stroke::new(1.5, Color32::from_rgb(255, 120, 130)),
    );

    // Front desktop screen (Lower-Right)
    let front_w = w * 0.65;
    let front_h = h * 0.65;
    let front_pos = Pos2::new(rect.right() - front_w, rect.bottom() - front_h);
    let front_rect = Rect::from_min_size(front_pos, Vec2::new(front_w, front_h));
    painter.rect_filled(
        front_rect,
        Rounding::same(6.0),
        Color32::from_rgb(240, 68, 85), // Vibrant scarlet
    );
    painter.rect_stroke(
        front_rect,
        Rounding::same(6.0),
        Stroke::new(1.8, Color32::WHITE),
    );

    // Interconnection lightning/bridge core in center
    let center = front_rect.center();
    let core_radius = (w * 0.12).max(3.0);
    painter.circle_filled(center, core_radius, Color32::WHITE);
    painter.circle_filled(
        center,
        core_radius * 0.55,
        Color32::from_rgb(230, 57, 70),
    );
}

/// Draws a crisp PC monitor / Desktop icon (AnyDesk style).
pub fn draw_pc_monitor(painter: &Painter, rect: Rect, border_color: Color32, fill_color: Color32) {
    let w = rect.width();
    let h = rect.height();

    // Monitor screen (upper 72% of area)
    let screen_h = h * 0.72;
    let screen_rect = Rect::from_min_size(rect.min, Vec2::new(w, screen_h));

    // Screen body & bezel
    painter.rect_filled(screen_rect, Rounding::same(4.0), fill_color);
    painter.rect_stroke(screen_rect, Rounding::same(4.0), Stroke::new(1.8, border_color));

    // Stand neck
    let stem_w = (w * 0.22).max(3.0);
    let stem_x = rect.center().x - stem_w / 2.0;
    let stem_y = screen_rect.max.y;
    let stem_h = h * 0.16;
    let stem_rect = Rect::from_min_size(
        Pos2::new(stem_x, stem_y),
        Vec2::new(stem_w, stem_h),
    );
    painter.rect_filled(stem_rect, Rounding::same(1.0), border_color);

    // Stand base foot
    let foot_w = (w * 0.55).max(6.0);
    let foot_x = rect.center().x - foot_w / 2.0;
    let foot_y = stem_rect.max.y;
    let foot_h = (h * 0.12).max(2.5);
    let foot_rect = Rect::from_min_size(
        Pos2::new(foot_x, foot_y),
        Vec2::new(foot_w, foot_h),
    );
    painter.rect_filled(foot_rect, Rounding::same(2.0), border_color);
}

/// Draws a Remote PC connection icon (monitor with connection arrow).
pub fn draw_remote_desk_icon(painter: &Painter, rect: Rect, color: Color32) {
    let sub_w = rect.width() * 0.78;
    let sub_h = rect.height() * 0.78;
    let pc_rect = Rect::from_min_size(rect.min, Vec2::new(sub_w, sub_h));
    draw_pc_monitor(painter, pc_rect, color, Color32::from_rgb(26, 30, 42));

    // Connection arrow in bottom-right
    let arrow_tip = rect.max;
    let arrow_size = rect.width() * 0.38;
    let p_tip = Pos2::new(arrow_tip.x, arrow_tip.y - 1.0);
    let p_left = Pos2::new(arrow_tip.x - arrow_size, arrow_tip.y - 1.0);
    let p_top = Pos2::new(arrow_tip.x - 1.0, arrow_tip.y - arrow_size);

    painter.add(eframe::egui::Shape::convex_polygon(
        vec![p_tip, p_left, p_top],
        Color32::from_rgb(230, 57, 70), // FreeDesk crimson accent
        Stroke::new(1.2, Color32::WHITE),
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
        points,
        Color32::from_rgba_premultiplied(color.r() / 4, color.g() / 4, color.b() / 4, 180),
        Stroke::new(1.6, color),
    ));
}

/// Generates a 64x64 RGBA icon buffer for the native OS window / taskbar.
pub fn generate_app_icon() -> IconData {
    let width = 64u32;
    let height = 64u32;
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            let fx = x as f32;
            let fy = y as f32;

            // Back screen rect: (6..40, 6..40)
            let in_back = fx >= 6.0 && fx <= 40.0 && fy >= 6.0 && fy <= 40.0;
            // Front screen rect: (22..58, 22..58)
            let in_front = fx >= 22.0 && fx <= 58.0 && fy >= 22.0 && fy <= 58.0;

            // Front border
            let is_front_border = in_front && (fx <= 24.0 || fx >= 56.0 || fy <= 24.0 || fy >= 56.0);
            // Center core node: distance to (40, 40)
            let dist_core = ((fx - 40.0).powi(2) + (fy - 40.0).powi(2)).sqrt();

            if dist_core <= 5.0 {
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            } else if is_front_border {
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            } else if in_front {
                rgba[idx] = 230;     // Red
                rgba[idx + 1] = 57;   // Green
                rgba[idx + 2] = 70;   // Blue
                rgba[idx + 3] = 255;
            } else if in_back {
                rgba[idx] = 180;
                rgba[idx + 1] = 30;
                rgba[idx + 2] = 45;
                rgba[idx + 3] = 240;
            }
        }
    }

    IconData {
        rgba,
        width,
        height,
    }
}
