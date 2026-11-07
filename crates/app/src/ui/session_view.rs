//! Remote session view — renders the desktop stream, captures remote input,
//! and paints a ghost cursor overlay for the host/viewer dual-cursor system.

use eframe::egui::{
    self, Color32, ColorImage, FontId, Painter, PointerButton, Pos2, Rect, RichText,
    Sense, Stroke, TextureHandle, TextureOptions, Ui, Vec2,
};
use super::floating_audio::FloatingAudioState;
use super::icons::draw_remote_desk_icon;
use super::theme::{ACCENT_RED, BG_DARK, SUCCESS_GREEN, TEXT_MUTED};
use freedesk_core::protocol::{InputEvent, MouseButton};

pub struct SessionViewState {
    pub target_id: String,
    pub screen_width: u32,
    pub screen_height: u32,
    pub texture: Option<TextureHandle>,
    pub floating_audio: FloatingAudioState,
    pub fps: u32,
    pub kbps: u32,
    pub latency_ms: u32,
    pub e2e_secure: bool,
    pub is_admin: bool,
    pub is_fullscreen: bool,
    pub safety_code: String,
    // Dual-cursor state updated from NetworkEvent::CursorUpdate
    pub host_controlling: bool,
    pub friend_ghost_pos: Option<(f32, f32)>, // normalised 0..1
}

impl SessionViewState {
    pub fn new(target_id: String) -> Self {
        let hash = target_id.chars().fold(5381u32, |acc, c| acc.wrapping_mul(33).wrapping_add(c as u32));
        let safety_code = format!("{:04}", hash % 10000);

        Self {
            target_id,
            screen_width: 1280,
            screen_height: 720,
            texture: None,
            floating_audio: FloatingAudioState::default(),
            fps: 60,
            kbps: 0,
            latency_ms: 0,
            e2e_secure: true,
            is_admin: false,
            is_fullscreen: false,
            safety_code,
            host_controlling: true,
            friend_ghost_pos: None,
        }
    }

    /// Updates the remote texture from raw RGBA buffer.
    pub fn update_frame(&mut self, ctx: &egui::Context, rgba_data: &[u8], width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;

        let image = ColorImage::from_rgba_unmultiplied([width as usize, height as usize], rgba_data);
        if let Some(tex) = &mut self.texture {
            tex.set(image, TextureOptions::LINEAR);
        } else {
            self.texture = Some(ctx.load_texture("remote_screen", image, TextureOptions::LINEAR));
        }
    }

    pub fn render(&mut self, ctx: &egui::Context, ui: &mut Ui) -> (Option<SessionAction>, Vec<InputEvent>) {
        let mut action = None;
        let mut input_events = Vec::new();

        // ── Top Control Toolbar ──────────────────────────────────────────────
        ui.horizontal(|ui| {
            let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(20.0, 18.0), egui::Sense::hover());
            draw_remote_desk_icon(&ui.painter(), icon_rect, Color32::from_rgb(52, 152, 219));
            ui.label(RichText::new(format!("Remoto: {}", self.target_id)).strong());

            ui.separator();
            ui.label(RichText::new(format!("⚡ {} FPS", self.fps)).color(SUCCESS_GREEN).small());
            if self.kbps > 0 {
                ui.label(RichText::new(format!("{} kb/s", self.kbps)).color(TEXT_MUTED).small());
            }
            if self.e2e_secure {
                ui.label(RichText::new(format!("🔒 E2E Verif: {}", self.safety_code)).color(SUCCESS_GREEN).small());
            }

            ui.separator();

            // Admin status or elevation request button
            if self.is_admin {
                ui.label(RichText::new("🛡 ADMIN").color(SUCCESS_GREEN).strong().small());
            } else if ui.button(RichText::new("🛡 Solicitar Admin").color(Color32::from_rgb(241, 196, 15)).small()).clicked() {
                action = Some(SessionAction::RequestElevation);
            }

            ui.separator();

            // Cursor status indicator
            if self.host_controlling {
                ui.label(
                    RichText::new("● Control: [Host]")
                        .color(Color32::from_rgb(241, 196, 15))
                        .small(),
                );
            } else {
                ui.label(
                    RichText::new("● Control: [Tú]")
                        .color(Color32::from_rgb(39, 174, 96))
                        .small(),
                );
            }

            ui.separator();

            // Toggle floating audio panel
            let audio_btn_text = if self.floating_audio.is_open {
                "🔊 Ocultar Audio"
            } else {
                "🔊 Audio"
            };
            if ui.button(audio_btn_text).clicked() {
                self.floating_audio.is_open = !self.floating_audio.is_open;
            }

            // Fullscreen toggle
            let fs_label = if self.is_fullscreen { "🗗 Ventana" } else { "⛶ Pantalla Completa" };
            if ui.button(fs_label).clicked() {
                self.is_fullscreen = !self.is_fullscreen;
                action = Some(SessionAction::ToggleFullscreen);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(RichText::new("✕ Desconectar").color(ACCENT_RED).strong())
                    .clicked()
                {
                    action = Some(SessionAction::Disconnect);
                }
            });
        });

        ui.separator();

        // ── Screen Viewport Canvas ───────────────────────────────────────────
        let available_size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click_and_drag());

        let painter = ui.painter_at(rect);

        // Background
        painter.rect_filled(rect, 0.0, BG_DARK);

        // Remote texture
        if let Some(texture) = &self.texture {
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Esperando señal de video del host...",
                FontId::proportional(18.0),
                TEXT_MUTED,
            );
        }

        // ── Ghost Cursor Overlay ─────────────────────────────────────────────
        // Show friend's ghost cursor position when host has priority
        if let Some((gx, gy)) = self.friend_ghost_pos {
            let cx = rect.left() + gx * rect.width();
            let cy = rect.top() + gy * rect.height();
            paint_ghost_cursor(&painter, Pos2::new(cx, cy), "Amigo");
        }

        // ── Local Mouse Prediction Overlay ───────────────────────────────────
        // Renders an immediate feedback cursor so the viewer feels zero input lag
        if let Some(hover_pos) = response.hover_pos() {
            if response.hovered() && !self.host_controlling {
                painter.circle_filled(hover_pos, 3.5, Color32::from_rgba_premultiplied(255, 255, 255, 210));
                painter.circle_stroke(hover_pos, 3.5, Stroke::new(1.0, Color32::from_rgb(230, 57, 70)));
            }
        }

        // ── Input Capture on Viewport ────────────────────────────────────────
        if let Some(hover_pos) = response.hover_pos() {
            let norm_x = ((hover_pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            let norm_y = ((hover_pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);

            if response.hovered() {
                input_events.push(InputEvent::MouseMove { x: norm_x, y: norm_y });
            }

            if response.clicked_by(PointerButton::Primary) {
                input_events.push(InputEvent::MouseDown { button: MouseButton::Left });
                input_events.push(InputEvent::MouseUp { button: MouseButton::Left });
            }
            if response.clicked_by(PointerButton::Secondary) {
                input_events.push(InputEvent::MouseDown { button: MouseButton::Right });
                input_events.push(InputEvent::MouseUp { button: MouseButton::Right });
            }
            if response.clicked_by(PointerButton::Middle) {
                input_events.push(InputEvent::MouseDown { button: MouseButton::Middle });
                input_events.push(InputEvent::MouseUp { button: MouseButton::Middle });
            }

            // Keyboard passthrough
            ctx.input(|i| {
                for event in &i.events {
                    if let egui::Event::Key { key, pressed, .. } = event {
                        let vk = egui_key_to_vk(*key);
                        if vk != 0 {
                            if *pressed {
                                input_events.push(InputEvent::KeyDown { vk_code: vk });
                            } else {
                                input_events.push(InputEvent::KeyUp { vk_code: vk });
                            }
                        }
                    }
                }
            });
        }

        // ── Floating Audio Panel overlay ─────────────────────────────────────
        self.floating_audio.show(ctx);

        (action, input_events)
    }
}

/// Paints a distinctive ghost cursor (hollow arrow + label) for the friend.
fn paint_ghost_cursor(painter: &Painter, pos: Pos2, label: &str) {
    let ghost_color = Color32::from_rgba_premultiplied(255, 100, 100, 220);
    let outline = Color32::from_rgba_premultiplied(80, 0, 0, 200);

    // Simple arrow shape via triangles
    let tip = pos;
    let size = 16.0_f32;
    let p1 = Pos2::new(tip.x, tip.y + size);
    let p2 = Pos2::new(tip.x + size * 0.4, tip.y + size * 0.65);

    // Filled arrow
    painter.add(egui::Shape::convex_polygon(
        vec![tip, p1, p2],
        ghost_color,
        Stroke::new(1.5, outline),
    ));

    // "Amigo" label beneath the arrow
    painter.text(
        Pos2::new(tip.x + 18.0, tip.y + 4.0),
        egui::Align2::LEFT_TOP,
        label,
        FontId::proportional(11.0),
        Color32::from_rgba_premultiplied(255, 100, 100, 230),
    );
}

/// Maps egui Key to Windows Virtual Key code.
fn egui_key_to_vk(key: egui::Key) -> u16 {
    use egui::Key::*;
    match key {
        A => 0x41, B => 0x42, C => 0x43, D => 0x44, E => 0x45, F => 0x46,
        G => 0x47, H => 0x48, I => 0x49, J => 0x4A, K => 0x4B, L => 0x4C,
        M => 0x4D, N => 0x4E, O => 0x4F, P => 0x50, Q => 0x51, R => 0x52,
        S => 0x53, T => 0x54, U => 0x55, V => 0x56, W => 0x57, X => 0x58,
        Y => 0x59, Z => 0x5A,
        Num0 => 0x30, Num1 => 0x31, Num2 => 0x32, Num3 => 0x33, Num4 => 0x34,
        Num5 => 0x35, Num6 => 0x36, Num7 => 0x37, Num8 => 0x38, Num9 => 0x39,
        F1 => 0x70, F2 => 0x71, F3 => 0x72, F4 => 0x73, F5 => 0x74, F6 => 0x75,
        F7 => 0x76, F8 => 0x77, F9 => 0x78, F10 => 0x79, F11 => 0x7A, F12 => 0x7B,
        Escape => 0x1B, Tab => 0x09, Backspace => 0x08, Enter => 0x0D,
        Space => 0x20, Delete => 0x2E, Insert => 0x2D,
        Home => 0x24, End => 0x23, PageUp => 0x21, PageDown => 0x22,
        ArrowLeft => 0x25, ArrowUp => 0x26, ArrowRight => 0x27, ArrowDown => 0x28,
        _ => 0,
    }
}

pub enum SessionAction {
    Disconnect,
    RequestElevation,
    ToggleFullscreen,
}
