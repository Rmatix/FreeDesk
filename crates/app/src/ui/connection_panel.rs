//! Main connection screen (AnyDesk style) for FreeDesk.

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Rounding, Stroke, Ui, Vec2};
use super::icons::{draw_freedesk_logo, draw_pc_monitor, draw_remote_desk_icon, draw_shield_icon};
use super::theme::{
    ACCENT_RED, BG_INPUT, BORDER_CARD, SUCCESS_GREEN,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, WARNING_AMBER,
};
use freedesk_core::security::SessionPermissions;

#[derive(Clone, Debug)]
pub struct RecentSession {
    pub peer_id: String,
    pub alias: String,
}

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
    pub recent_sessions: Vec<RecentSession>,
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
            relay_status: "● Conectando al Servidor Relay en la nube...".to_string(),
            is_relay_connected: false,
            relay_addr: "freedesk-relay.onrender.com".to_string(),
            requested_perms: SessionPermissions::full_control(),
            is_admin: false,
            copied_recently: false,
            copied_timer: None,
            recent_sessions: Vec::new(),
        }
    }
}

impl ConnectionPanelState {
    pub fn refresh_pin(&mut self) {
        let pin_num: u32 = 100_000 + (rand::random::<u32>() % 900_000);
        self.my_session_pin = format!("{}", pin_num);
    }

    pub fn record_recent_connection(&mut self, peer_id: &str) {
        let clean = peer_id.replace(' ', "");
        if !clean.is_empty() && !self.recent_sessions.iter().any(|s| s.peer_id == clean) {
            self.recent_sessions.insert(0, RecentSession {
                peer_id: clean.clone(),
                alias: format!("Equipo {}", &clean),
            });
            if self.recent_sessions.len() > 4 {
                self.recent_sessions.truncate(4);
            }
        }
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

        ui.add_space(6.0);

        // ── TOP BRAND HEADER BAR ──────────────────────────────────────────────
        ui.horizontal(|ui| {
            // FreeDesk Custom Logo Icon & Title
            ui.horizontal(|ui| {
                let (logo_rect, _) = ui.allocate_exact_size(Vec2::new(26.0, 26.0), egui::Sense::hover());
                draw_freedesk_logo(&ui.painter(), logo_rect);
                ui.heading(RichText::new("FreeDesk").color(ACCENT_RED).strong().size(22.0));

                ui.add_space(8.0);
                ui.colored_label(TEXT_MUTED, "•");
                ui.add_space(4.0);

                // Animated breathing green status radar
                let time = ui.input(|i| i.time);
                let pulse = ((time * 3.0).sin() * 0.5 + 0.5) as f32; // 0..1
                let (dot_rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 14.0), egui::Sense::hover());
                let dot_center = dot_rect.center();
                let pulse_radius = 4.0 + pulse * 4.0;
                let pulse_alpha = ((1.0 - pulse) * 140.0) as u8;
                ui.painter().circle_filled(
                    dot_center,
                    pulse_radius,
                    Color32::from_rgba_premultiplied(46, 204, 113, pulse_alpha),
                );
                ui.painter().circle_filled(dot_center, 4.0, SUCCESS_GREEN);
                ui.colored_label(SUCCESS_GREEN, RichText::new("En línea (60 FPS DXGI)").strong().size(13.5));
            });

            // Right side: Admin Mode Indicator / Button
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.is_admin {
                    // Admin active badge
                    ui.group(|ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 16.0), egui::Sense::hover());
                        draw_shield_icon(&ui.painter(), rect, SUCCESS_GREEN);
                        ui.colored_label(SUCCESS_GREEN, RichText::new("ADMIN ACTIVO").strong().size(13.0));
                    });
                } else {
                    let admin_btn = egui::Button::new(
                        RichText::new("🛡 Iniciar como Admin").color(WARNING_AMBER).strong().size(13.0),
                    );
                    if ui.add(admin_btn).on_hover_text("Reinicia FreeDesk con UAC para controlar ventanas de sistema y Task Manager").clicked() {
                        action = Some(ConnectionAction::RequestLocalElevation);
                    }
                }
            });
        });

        ui.add_space(10.0);

        // ── DUAL MAIN CARDS (AnyDesk Style) ──────────────────────────────────
        ui.columns(2, |columns| {
            // ── LEFT CARD: ESTE PUESTO DE TRABAJO (Host) ─────────────────────
            columns[0].group(|ui| {
                ui.set_min_height(280.0);

                // Card Header with vector PC Monitor icon
                ui.horizontal(|ui| {
                    let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 20.0), egui::Sense::hover());
                    draw_pc_monitor(&ui.painter(), icon_rect, ACCENT_RED, Color32::from_rgb(20, 24, 32));
                    ui.label(RichText::new("Este puesto de trabajo").heading().strong());
                });

                ui.label(RichText::new("Comparte tu ID y PIN para que se conecten a este equipo:").color(TEXT_MUTED).small());
                ui.add_space(8.0);

                // Address Container Box
                ui.label(RichText::new("Tu Dirección FreeDesk:").color(TEXT_SECONDARY).small());
                let (box_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 50.0), egui::Sense::hover());
                ui.painter().rect_filled(box_rect, Rounding::same(8.0), BG_INPUT);
                ui.painter().rect_stroke(box_rect, Rounding::same(8.0), Stroke::new(1.0, BORDER_CARD));

                // Paint ID centered inside container box
                ui.painter().text(
                    Pos2::new(box_rect.left() + 16.0, box_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    &self.my_peer_id,
                    egui::FontId::proportional(24.0),
                    TEXT_PRIMARY,
                );

                // Copy button inside container box right side
                let copy_rect = Rect::from_min_size(
                    Pos2::new(box_rect.right() - 110.0, box_rect.center().y - 16.0),
                    Vec2::new(100.0, 32.0),
                );
                let (btn_text, btn_color) = if self.copied_recently {
                    ("✓ Copiado!", SUCCESS_GREEN)
                } else {
                    ("📋 Copiar", TEXT_PRIMARY)
                };
                if ui.put(copy_rect, egui::Button::new(RichText::new(btn_text).color(btn_color).strong())).clicked() {
                    ui.output_mut(|o| o.copied_text = self.my_peer_id.replace(' ', ""));
                    self.copied_recently = true;
                    self.copied_timer = Some(std::time::Instant::now());
                }

                ui.add_space(10.0);

                // PIN Container
                ui.horizontal(|ui| {
                    ui.label(RichText::new("PIN de Acceso temporal:").color(TEXT_SECONDARY).small());
                    ui.label(RichText::new(&self.my_session_pin).size(18.0).strong().color(ACCENT_RED));
                    if ui.button("🔄 Nuevo").clicked() {
                        self.refresh_pin();
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(4.0);

                // Security badges
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔒 Cifrado E2E X25519 + AES-256-GCM").color(SUCCESS_GREEN).small());
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🛡 Hardening: ASLR, DEP y Anti-DLL Hijacking").color(SUCCESS_GREEN).small());
                });
            });

            // ── RIGHT CARD: OTRO PUESTO (Remote Viewer) ──────────────────────
            columns[1].group(|ui| {
                ui.set_min_height(280.0);

                // Card Header with vector Remote Desk icon
                ui.horizontal(|ui| {
                    let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 20.0), egui::Sense::hover());
                    draw_remote_desk_icon(&ui.painter(), icon_rect, Color32::from_rgb(52, 152, 219));
                    ui.label(RichText::new("Conectarse a otro puesto").heading().strong());
                });

                ui.label(RichText::new("Introduce el ID del puesto remoto al que deseas acceder:").color(TEXT_MUTED).small());
                ui.add_space(8.0);

                ui.label(RichText::new("Dirección remota (9 dígitos):").color(TEXT_SECONDARY).small());
                ui.add(
                    egui::TextEdit::singleline(&mut self.target_peer_id)
                        .hint_text("Ej: 845 964 434")
                        .desired_width(ui.available_width()),
                );

                ui.add_space(6.0);
                ui.label(RichText::new("PIN de acceso del puesto remoto:").color(TEXT_SECONDARY).small());
                ui.add(
                    egui::TextEdit::singleline(&mut self.target_pin)
                        .hint_text("PIN de 6 dígitos")
                        .password(true)
                        .desired_width(ui.available_width()),
                );

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.requested_perms.can_control_mouse, "Mouse");
                    ui.checkbox(&mut self.requested_perms.can_control_keyboard, "Teclado");
                    ui.checkbox(&mut self.requested_perms.can_listen_audio, "Audio");
                });

                ui.add_space(10.0);

                // Main Connect Button
                let connect_btn = egui::Button::new(
                    RichText::new("🚀 CONECTAR AHORA").size(15.0).strong().color(Color32::WHITE),
                )
                .min_size(Vec2::new(ui.available_width(), 40.0))
                .fill(ACCENT_RED);

                if ui.add(connect_btn).clicked() {
                    let clean_id = self.target_peer_id.replace(' ', "");
                    if !clean_id.is_empty() {
                        self.record_recent_connection(&clean_id);
                        action = Some(ConnectionAction::Connect {
                            target_id: clean_id,
                            pin: self.target_pin.clone(),
                            perms: self.requested_perms,
                        });
                    }
                }
            });
        });

        ui.add_space(10.0);

        // ── RECENT SESSIONS SECTION (AnyDesk Style) ───────────────────────────
        if !self.recent_sessions.is_empty() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🕒 Conexiones Recientes").strong());
                });
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    for recent in &self.recent_sessions {
                        let btn = egui::Button::new(format!("🖥 {}", recent.peer_id)).min_size(Vec2::new(120.0, 32.0));
                        if ui.add(btn).clicked() {
                            self.target_peer_id = recent.peer_id.clone();
                        }
                    }
                });
            });
            ui.add_space(10.0);
        }

        // ── RELAY SERVER SETTINGS ─────────────────────────────────────────────
        egui::CollapsingHeader::new("⚙ Servidor Relay de Conexión en la Nube")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Servidor Relay:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.relay_addr)
                            .hint_text("freedesk-relay.onrender.com")
                            .desired_width(260.0),
                    );
                    if ui.button("🔄 Reconectar").clicked() {
                        action = Some(ConnectionAction::ReconnectRelay(self.relay_addr.clone()));
                    }
                });
                ui.label(
                    RichText::new("Servidor en Render: https://freedesk-relay.onrender.com (tráfico cifrado E2E)")
                        .small()
                        .color(TEXT_MUTED),
                );
            });

        ui.add_space(6.0);

        // ── STATUS BAR ────────────────────────────────────────────────────────
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new(&self.relay_status).small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("FreeDesk v0.6.0 • Render Cloud • 60 FPS DXGI • Zero-Knowledge")
                        .small()
                        .color(TEXT_MUTED),
                );
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
