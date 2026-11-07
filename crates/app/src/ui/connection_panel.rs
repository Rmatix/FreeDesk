//! Main connection screen (AnyDesk style) for FreeDesk.

use eframe::egui::{self, Color32, RichText, Ui, Vec2};
use super::theme::{ACCENT_RED, SUCCESS_GREEN, TEXT_MUTED, TEXT_PRIMARY};
use freedesk_core::security::SessionPermissions;

pub struct ConnectionPanelState {
    pub my_peer_id: String,
    pub my_session_pin: String,
    pub target_peer_id: String,
    pub target_pin: String,
    pub relay_status: String,
    pub is_relay_connected: bool,
    pub relay_addr: String,
    pub requested_perms: SessionPermissions,
    pub is_admin: bool,
    pub copied_recently: bool,
    pub copied_timer: Option<std::time::Instant>,
}

impl Default for ConnectionPanelState {
    fn default() -> Self {
        // Generate initial random 9-digit ID
        let id_num: u32 = 100_000_000 + (rand::random::<u32>() % 900_000_000);
        let id_str = format!("{}", id_num);
        let formatted_id = format!("{} {} {}", &id_str[0..3], &id_str[3..6], &id_str[6..9]);

        let pin_num: u32 = 100_000 + (rand::random::<u32>() % 900_000);
        let pin_str = format!("{}", pin_num);

        Self {
            my_peer_id: formatted_id,
            my_session_pin: pin_str,
            target_peer_id: String::new(),
            target_pin: String::new(),
            relay_status: "● Conectando al Relay en la nube...".to_string(),
            is_relay_connected: false,
            relay_addr: "freedesk-relay.onrender.com".to_string(),
            requested_perms: SessionPermissions::full_control(),
            is_admin: false,
            copied_recently: false,
            copied_timer: None,
        }
    }
}

impl ConnectionPanelState {
    pub fn refresh_pin(&mut self) {
        let pin_num: u32 = 100_000 + (rand::random::<u32>() % 900_000);
        self.my_session_pin = format!("{}", pin_num);
    }

    pub fn render(&mut self, ui: &mut Ui) -> Option<ConnectionAction> {
        let mut action = None;

        // Reset copy notification after 2 seconds
        if self.copied_recently {
            if let Some(timer) = self.copied_timer {
                if timer.elapsed().as_secs() >= 2 {
                    self.copied_recently = false;
                    self.copied_timer = None;
                }
            }
        }

        ui.add_space(10.0);

        // Header Title & Admin Indicator
        ui.horizontal(|ui| {
            ui.heading(RichText::new("FreeDesk").color(ACCENT_RED).strong().size(22.0));
            ui.label(RichText::new("— Escritorio Remoto de Ultra-Rendimiento (60 FPS)").color(TEXT_MUTED));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.is_admin {
                    ui.label(RichText::new("🛡 ADMIN ACTIVADO").color(SUCCESS_GREEN).strong().small());
                } else if ui.button(RichText::new("🛡 Modo Administrador").color(Color32::from_rgb(241, 196, 15)).small()).clicked() {
                    action = Some(ConnectionAction::RequestLocalElevation);
                }
            });
        });

        ui.add_space(15.0);

        // Dual card layout (Host vs Viewer)
        ui.columns(2, |columns| {
            // Left Card: Este puesto (Host)
            columns[0].group(|ui| {
                ui.set_min_height(270.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("📥 Este puesto de trabajo").heading().strong());
                    if self.is_admin {
                        ui.label(RichText::new("[Admin]").color(SUCCESS_GREEN).small());
                    }
                });
                ui.label(RichText::new("Comparte tu ID y PIN para que se conecten a tu equipo:").color(TEXT_MUTED));
                ui.add_space(10.0);

                ui.label(RichText::new("Tu Dirección FreeDesk:").color(TEXT_MUTED));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.my_peer_id).size(26.0).strong().color(TEXT_PRIMARY));
                    let copy_text = if self.copied_recently { "✓ ¡Copiado!" } else { "📋 Copiar" };
                    let copy_color = if self.copied_recently { SUCCESS_GREEN } else { TEXT_PRIMARY };
                    if ui.button(RichText::new(copy_text).color(copy_color)).clicked() {
                        ui.output_mut(|o| o.copied_text = self.my_peer_id.replace(' ', ""));
                        self.copied_recently = true;
                        self.copied_timer = Some(std::time::Instant::now());
                    }
                });

                ui.add_space(8.0);
                ui.label(RichText::new("PIN de Acceso de un solo uso:").color(TEXT_MUTED));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&self.my_session_pin).size(20.0).strong().color(ACCENT_RED));
                    if ui.button("🔄 Nuevo PIN").clicked() {
                        self.refresh_pin();
                    }
                });

                ui.add_space(10.0);
                if !self.is_admin {
                    if ui.button(RichText::new("⚡ Reiniciar con Privilegios de Administrador").color(Color32::from_rgb(241, 196, 15))).clicked() {
                        action = Some(ConnectionAction::RequestLocalElevation);
                    }
                }

                ui.add_space(8.0);
                ui.label(RichText::new("🔒 Cifrado E2E: X25519 + AES-256-GCM + Zeroize Activo").color(SUCCESS_GREEN).small());
                ui.label(RichText::new("🛡 Hardening: ASLR, DEP y Anti-DLL Hijacking activos").color(SUCCESS_GREEN).small());
            });

            // Right Card: Otro puesto (Viewer)
            columns[1].group(|ui| {
                ui.set_min_height(270.0);
                ui.label(RichText::new("📤 Conectarse a otro puesto").heading().strong());
                ui.label(RichText::new("Introduce el ID del puesto remoto al que deseas acceder:").color(TEXT_MUTED));
                ui.add_space(10.0);

                ui.label(RichText::new("Dirección remota (9 dígitos):").color(TEXT_MUTED));
                ui.add(egui::TextEdit::singleline(&mut self.target_peer_id).hint_text("Ej: 492 810 334").desired_width(220.0));

                ui.add_space(8.0);
                ui.label(RichText::new("PIN remoto:").color(TEXT_MUTED));
                ui.add(egui::TextEdit::singleline(&mut self.target_pin).hint_text("PIN de 6 dígitos").password(true).desired_width(220.0));

                ui.add_space(10.0);
                ui.label(RichText::new("Permisos a solicitar:").small().color(TEXT_MUTED));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.requested_perms.can_control_mouse, "Mouse");
                    ui.checkbox(&mut self.requested_perms.can_control_keyboard, "Teclado");
                    ui.checkbox(&mut self.requested_perms.can_listen_audio, "Audio");
                });

                ui.add_space(12.0);
                let connect_btn = egui::Button::new(RichText::new("🚀 Conectar").size(16.0).strong().color(Color32::WHITE))
                    .min_size(Vec2::new(140.0, 36.0));

                if ui.add(connect_btn).clicked() {
                    let clean_id = self.target_peer_id.replace(' ', "");
                    if !clean_id.is_empty() {
                        action = Some(ConnectionAction::Connect {
                            target_id: clean_id,
                            pin: self.target_pin.clone(),
                            perms: self.requested_perms,
                        });
                    }
                }
            });
        });

        ui.add_space(15.0);

        // Network / Relay Configuration (Expanded by default for clarity)
        egui::CollapsingHeader::new("⚙ Servidor Relay de Conexión")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Servidor Relay:");
                    ui.add(egui::TextEdit::singleline(&mut self.relay_addr).hint_text("freedesk-relay.onrender.com").desired_width(240.0));
                    if ui.button("🔄 Reconectar al Servidor").clicked() {
                        action = Some(ConnectionAction::ReconnectRelay(self.relay_addr.clone()));
                    }
                });
                ui.label(RichText::new("Servidor en la nube predeterminado: freedesk-relay.onrender.com (conecta automáticamente al abrir la app)").small().color(TEXT_MUTED));
            });

        ui.add_space(10.0);

        // Status bar
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new(&self.relay_status).small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("FreeDesk v0.5.0 • Render Cloud Active • 60 FPS DXGI • Zero-Knowledge").small().color(TEXT_MUTED));
            });
        });

        action
    }
}

pub enum ConnectionAction {
    Connect {
        target_id: String,
        pin: String,
        perms: SessionPermissions,
    },
    ReconnectRelay(String),
    RequestLocalElevation,
}
