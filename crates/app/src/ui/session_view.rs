//! Remote session view rendering the desktop stream and capturing remote input.

use eframe::egui::{self, Color32, ColorImage, PointerButton, Rect, RichText, Sense, TextureHandle, TextureOptions, Ui};
use super::floating_audio::FloatingAudioState;
use super::theme::{ACCENT_RED, BG_DARK, SUCCESS_GREEN, TEXT_MUTED};
use freedesk_core::protocol::{InputEvent, MouseButton};

pub struct SessionViewState {
    pub target_id: String,
    pub screen_width: u32,
    pub screen_height: u32,
    pub texture: Option<TextureHandle>,
    pub floating_audio: FloatingAudioState,
    pub fps: u32,
    pub latency_ms: u32,
    pub e2e_secure: bool,
}

impl SessionViewState {
    pub fn new(target_id: String) -> Self {
        Self {
            target_id,
            screen_width: 1920,
            screen_height: 1080,
            texture: None,
            floating_audio: FloatingAudioState::default(),
            fps: 60,
            latency_ms: 14,
            e2e_secure: true,
        }
    }

    /// Updates the remote texture from raw RGBA buffer
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

        // Top Control Toolbar
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Conectado a: {}", self.target_id)).strong());
            ui.separator();
            ui.label(RichText::new(format!("⚡ {} ms", self.latency_ms)).color(SUCCESS_GREEN).small());
            ui.label(RichText::new(format!("🖥️ {} FPS", self.fps)).color(SUCCESS_GREEN).small());
            if self.e2e_secure {
                ui.label(RichText::new("🔒 E2E Cifrado").color(SUCCESS_GREEN).small());
            }

            ui.separator();

            // Toggle floating audio panel
            let audio_btn_text = if self.floating_audio.is_open {
                "🔊 Ocultar Panel Audio"
            } else {
                "🔊 Mostrar Panel Audio"
            };
            if ui.button(audio_btn_text).clicked() {
                self.floating_audio.is_open = !self.floating_audio.is_open;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("❌ Desconectar").color(ACCENT_RED).strong()).clicked() {
                    action = Some(SessionAction::Disconnect);
                }
            });
        });

        ui.separator();

        // Screen Viewport Canvas
        let available_size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click_and_drag());

        // Paint background and remote texture
        ui.painter().rect_filled(rect, 0.0, BG_DARK);

        if let Some(texture) = &self.texture {
            ui.painter().image(
                texture.id(),
                rect,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Esperando señal de video del host...",
                egui::FontId::proportional(18.0),
                TEXT_MUTED,
            );
        }

        // Input Capture on Viewport
        if let Some(hover_pos) = response.hover_pos() {
            let norm_x = ((hover_pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            let norm_y = ((hover_pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);

            // Mouse Move
            if response.hovered() {
                input_events.push(InputEvent::MouseMove { x: norm_x, y: norm_y });
            }

            // Mouse Clicks
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
        }

        // Render the Movable Draggable Floating Audio Panel overlay
        self.floating_audio.show(ctx);

        (action, input_events)
    }
}

pub enum SessionAction {
    Disconnect,
}
