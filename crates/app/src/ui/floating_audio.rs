//! Movable / Draggable Floating Audio Panel for FreeDesk.
//! Allows monitoring and controlling host audio in real-time.

use eframe::egui::{self, Color32, ProgressBar, Ui, Vec2};
use super::theme::{ACCENT_RED, SUCCESS_GREEN, TEXT_MUTED, TEXT_PRIMARY};

#[derive(Clone, Debug)]
pub struct FloatingAudioState {
    pub is_open: bool,
    pub volume: f32, // 0.0 to 1.0
    pub is_muted: bool,
    pub current_peak_level: f32, // 0.0 to 1.0 (VU meter)
    pub listen_system: bool,
    pub listen_notifications: bool,
}

impl Default for FloatingAudioState {
    fn default() -> Self {
        Self {
            is_open: true,
            volume: 0.85,
            is_muted: false,
            current_peak_level: 0.42,
            listen_system: true,
            listen_notifications: true,
        }
    }
}

impl FloatingAudioState {
    /// Render the draggable floating window
    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.is_open {
            return;
        }

        let mut is_open = self.is_open;
        egui::Window::new("🔊 Audio Remoto del Host")
            .open(&mut is_open)
            .resizable(true)
            .collapsible(true)
            .default_size(Vec2::new(280.0, 240.0))
            .show(ctx, |ui| {
                self.render_contents(ui);
            });
        self.is_open = is_open;
    }

    fn render_contents(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);

        // Header / Status
        ui.horizontal(|ui| {
            if self.is_muted {
                ui.colored_label(ACCENT_RED, "🔇 Audio Silenciado");
            } else {
                ui.colored_label(SUCCESS_GREEN, "🟢 Transmitiendo Audio en Vivo");
            }
        });

        ui.separator();

        // Master Volume
        ui.label(egui::RichText::new("🎚️ Volumen Master:").strong());
        ui.horizontal(|ui| {
            let slider = egui::Slider::new(&mut self.volume, 0.0..=1.0)
                .show_value(false)
                .text("");
            ui.add(slider);
            ui.label(format!("{}%", (self.volume * 100.0) as u32));
        });

        // Live VU Level Meter
        ui.label(egui::RichText::new("📊 Nivel de Sonido del Host:").color(TEXT_MUTED));
        let display_level = if self.is_muted { 0.0 } else { self.current_peak_level * self.volume };
        let meter_color = if display_level > 0.85 {
            ACCENT_RED
        } else if display_level > 0.6 {
            Color32::from_rgb(241, 196, 15) // Warning yellow
        } else {
            SUCCESS_GREEN
        };

        let progress_bar = ProgressBar::new(display_level)
            .fill(meter_color)
            .animate(true);
        ui.add(progress_bar);

        ui.separator();

        // Sound source filters
        ui.label(egui::RichText::new("🎧 Fuentes de Audio activas:").color(TEXT_PRIMARY));
        ui.checkbox(&mut self.listen_system, "Sistema y Reproductores");
        ui.checkbox(&mut self.listen_notifications, "Sonidos de Notificaciones");

        ui.add_space(8.0);

        // Quick Mute / Unmute Button
        ui.horizontal(|ui| {
            let mute_label = if self.is_muted { "🔊 Activar Sonido" } else { "🔇 Silenciar Todo" };
            if ui.button(mute_label).clicked() {
                self.is_muted = !self.is_muted;
            }
        });
    }
}
