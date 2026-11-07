#![windows_subsystem = "windows"]

use eframe::egui::{self, CentralPanel, Color32, RichText, Vec2};
use freedesk_app::network_client::{GuiCommand, NetworkBridge, NetworkEvent};
use freedesk_app::ui::{apply_theme, ConnectionAction, ConnectionPanelState, SessionAction, SessionViewState};
use freedesk_core::security::{apply_process_hardening, SessionPermissions};
use tracing::info;

// ─── Incoming connection request (host side) ──────────────────────────────────
struct IncomingRequest {
    viewer_id: String,
    perms: SessionPermissions,
    // permission toggles the host can override before accepting
    allow_mouse: bool,
    allow_keyboard: bool,
    allow_audio: bool,
}

// ─── App mode ────────────────────────────────────────────────────────────────
enum AppMode {
    Home,
    Hosting(SessionViewState),   // We are the host, friend is viewing
    Viewing(SessionViewState),   // We are the viewer, controlling a remote
}

// ─── Main application struct ─────────────────────────────────────────────────
struct FreeDeskApp {
    mode: AppMode,
    conn_panel: ConnectionPanelState,
    bridge: NetworkBridge,
    pending_incoming: Option<IncomingRequest>,
    // Host-side audio bar (only shown when hosting)
    host_audio_muted: bool,
    host_audio_volume: f32,
}

impl FreeDeskApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        let conn_panel = ConnectionPanelState::default();

        let relay_addr = std::env::var("FREEDESK_RELAY")
            .unwrap_or_else(|_| "freedesk-relay.onrender.com".to_string());
        let bridge = NetworkBridge::start(
            relay_addr,
            conn_panel.my_peer_id.clone(),
            conn_panel.my_session_pin.clone(),
        );

        Self {
            mode: AppMode::Home,
            conn_panel,
            bridge,
            pending_incoming: None,
            host_audio_muted: false,
            host_audio_volume: 0.85,
        }
    }
}

impl eframe::App for FreeDeskApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── 1. Drain network events ──────────────────────────────────────────
        while let Ok(evt) = self.bridge.event_rx.try_recv() {
            match evt {
                NetworkEvent::RelayStatus(status) => {
                    self.conn_panel.relay_status = status;
                }
                NetworkEvent::IncomingConnectionPrompt { viewer_id, perms } => {
                    info!("⚠️  Incoming connection request from {}", viewer_id);
                    self.pending_incoming = Some(IncomingRequest {
                        allow_mouse: perms.can_control_mouse,
                        allow_keyboard: perms.can_control_keyboard,
                        allow_audio: perms.can_listen_audio,
                        viewer_id,
                        perms,
                    });
                }
                NetworkEvent::SessionStarted { peer_id } => {
                    info!("Session started with peer {}", peer_id);
                    // Only switch to Viewing mode if we weren't already in Hosting mode
                    if matches!(self.mode, AppMode::Home) {
                        self.mode = AppMode::Viewing(SessionViewState::new(peer_id));
                    }
                }
                NetworkEvent::NewFrame { width, height, rgba } => {
                    if let AppMode::Viewing(session) = &mut self.mode {
                        session.update_frame(ctx, &rgba, width, height);
                    }
                }
                NetworkEvent::CursorUpdate { host_controlling, ghost_pos } => {
                    if let AppMode::Viewing(session) = &mut self.mode {
                        session.host_controlling = host_controlling;
                        session.friend_ghost_pos = ghost_pos;
                    }
                }
                NetworkEvent::SessionEnded => {
                    info!("Session ended");
                    self.mode = AppMode::Home;
                    self.pending_incoming = None;
                }
            }
        }

        // ── 2. Show incoming connection modal (top priority, blocks everything) ──
        if self.pending_incoming.is_some() {
            self.show_incoming_modal(ctx);
        }

        // ── 3. Collect UI actions outside borrow scope ───────────────────────
        enum UiResult {
            None,
            Connect { target_id: String, pin: String, perms: freedesk_core::security::SessionPermissions },
            ReconnectRelay(String),
            ViewerInputs { inputs: Vec<freedesk_core::protocol::InputEvent>, audio_muted: bool, audio_volume: f32, disconnect: bool },
            HostDisconnect,
        }

        let ui_result = CentralPanel::default().show(ctx, |ui| {
            match &mut self.mode {
                AppMode::Home => {
                    if let Some(action) = self.conn_panel.render(ui) {
                        match action {
                            ConnectionAction::Connect { target_id, pin, perms } => {
                                UiResult::Connect { target_id, pin, perms }
                            }
                            ConnectionAction::ReconnectRelay(new_addr) => {
                                UiResult::ReconnectRelay(new_addr)
                            }
                        }
                    } else {
                        UiResult::None
                    }
                }

                AppMode::Viewing(session) => {
                    let (action, inputs) = session.render(ctx, ui);
                    let audio_muted = session.floating_audio.is_muted;
                    let audio_volume = session.floating_audio.volume;
                    let disconnect = matches!(action, Some(SessionAction::Disconnect));
                    UiResult::ViewerInputs { inputs, audio_muted, audio_volume, disconnect }
                }

                AppMode::Hosting(session) => {
                    let (action, _inputs) = session.render(ctx, ui);
                    if matches!(action, Some(SessionAction::Disconnect)) {
                        UiResult::HostDisconnect
                    } else {
                        UiResult::None
                    }
                }
            }
        }).inner;

        // ── 4. Act on collected UI results ───────────────────────────────────
        match ui_result {
            UiResult::Connect { target_id, pin, perms } => {
                info!("Connecting to {}", target_id);
                self.bridge.cmd_tx.send(GuiCommand::Connect {
                    target_id: target_id.clone(),
                    pin,
                    perms,
                }).ok();
                self.mode = AppMode::Viewing(SessionViewState::new(target_id));
            }
            UiResult::ReconnectRelay(new_addr) => {
                info!("Reconnecting to relay: {}", new_addr);
                self.bridge = NetworkBridge::start(
                    new_addr,
                    self.conn_panel.my_peer_id.clone(),
                    self.conn_panel.my_session_pin.clone(),
                );
            }
            UiResult::ViewerInputs { inputs, audio_muted, audio_volume, disconnect } => {
                for evt in inputs {
                    self.bridge.cmd_tx.send(GuiCommand::SendInput(evt)).ok();
                }
                self.bridge.cmd_tx.send(GuiCommand::AudioControl {
                    muted: audio_muted,
                    volume: audio_volume,
                }).ok();
                if disconnect {
                    self.bridge.cmd_tx.send(GuiCommand::Disconnect).ok();
                    self.mode = AppMode::Home;
                }
            }
            UiResult::HostDisconnect => {
                self.bridge.cmd_tx.send(GuiCommand::Disconnect).ok();
                self.mode = AppMode::Home;
            }
            UiResult::None => {}
        }

        // ── 5. Host-side floating audio bar ─────────────────────────────────
        if matches!(self.mode, AppMode::Hosting(_)) {
            self.show_host_audio_bar(ctx);
        }

        // ── 6. Repaint at 60 FPS ─────────────────────────────────────────────
        ctx.request_repaint();
    }
}

impl FreeDeskApp {
    /// AnyDesk-style incoming connection acceptance modal.
    fn show_incoming_modal(&mut self, ctx: &egui::Context) {
        // Semi-transparent overlay
        let screen_rect = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("overlay")))
            .rect_filled(screen_rect, 0.0, Color32::from_rgba_premultiplied(0, 0, 0, 160));

        let Some(req) = &mut self.pending_incoming else { return };

        let viewer_id_clone = req.viewer_id.clone();
        let allow_mouse = req.allow_mouse;
        let allow_keyboard = req.allow_keyboard;
        let allow_audio = req.allow_audio;

        let mut accepted = false;
        let mut rejected = false;

        egui::Window::new("🚨 Solicitud de Conexión Entrante")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_size(Vec2::new(400.0, 300.0))
            .show(ctx, |ui| {
                ui.add_space(8.0);

                // Header
                ui.colored_label(
                    Color32::from_rgb(255, 85, 85),
                    RichText::new("⚠️  ¡Alguien quiere conectarse a tu equipo!")
                        .strong()
                        .size(16.0),
                );
                ui.add_space(6.0);
                ui.label(format!("🖥️  Desde: {}", viewer_id_clone));
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // Permission toggles
                ui.label(RichText::new("🔒 Permisos que otorgarás:").strong());
                ui.add_space(4.0);

                if let Some(req_mut) = &mut self.pending_incoming {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut req_mut.allow_mouse, "🖱️  Control de Mouse");
                        ui.checkbox(&mut req_mut.allow_keyboard, "⌨️  Control de Teclado");
                    });
                    ui.checkbox(&mut req_mut.allow_audio, "🔊 Escuchar Audio");
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                // Action buttons
                ui.horizontal(|ui| {
                    let accept_btn = egui::Button::new(
                        RichText::new("✅  ACEPTAR").strong().color(Color32::WHITE).size(15.0),
                    )
                    .min_size(Vec2::new(140.0, 42.0))
                    .fill(Color32::from_rgb(39, 174, 96));

                    let reject_btn = egui::Button::new(
                        RichText::new("❌  RECHAZAR").strong().color(Color32::WHITE).size(15.0),
                    )
                    .min_size(Vec2::new(140.0, 42.0))
                    .fill(Color32::from_rgb(192, 57, 43));

                    if ui.add(accept_btn).clicked() {
                        accepted = true;
                    }
                    ui.add_space(16.0);
                    if ui.add(reject_btn).clicked() {
                        rejected = true;
                    }
                });
                ui.add_space(8.0);
            });

        if accepted {
            if let Some(req) = self.pending_incoming.take() {
                let granted = SessionPermissions {
                    can_view: true,
                    can_control_mouse: req.allow_mouse,
                    can_control_keyboard: req.allow_keyboard,
                    can_listen_audio: req.allow_audio,
                    can_sync_clipboard: false,
                    can_transfer_files: false,
                };
                info!("✅ Accepted connection from {}", req.viewer_id);
                self.bridge.cmd_tx.send(GuiCommand::AcceptIncoming {
                    viewer_id: req.viewer_id.clone(),
                    perms: granted,
                }).ok();
                // Switch to Hosting mode
                self.mode = AppMode::Hosting(SessionViewState::new(req.viewer_id));
            }
        } else if rejected {
            if let Some(req) = self.pending_incoming.take() {
                info!("❌ Rejected connection from {}", req.viewer_id);
                self.bridge.cmd_tx.send(GuiCommand::RejectIncoming {
                    viewer_id: req.viewer_id,
                }).ok();
            }
        }
    }

    /// Host-side floating audio control bar.
    fn show_host_audio_bar(&mut self, ctx: &egui::Context) {
        let muted = self.host_audio_muted;
        let mut new_muted = muted;
        let mut new_volume = self.host_audio_volume;
        let mut send_update = false;
        let mut disconnect = false;

        egui::Window::new("🎙️ Control de Audio — Estás siendo controlado")
            .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -16.0])
            .resizable(false)
            .collapsible(true)
            .default_size(Vec2::new(300.0, 180.0))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);

                ui.horizontal(|ui| {
                    let status_text = if muted { "🔇 Audio Silenciado" } else { "🟢 Transmitiendo Audio" };
                    let status_color = if muted {
                        Color32::from_rgb(192, 57, 43)
                    } else {
                        Color32::from_rgb(39, 174, 96)
                    };
                    ui.colored_label(status_color, status_text);
                });

                ui.separator();

                ui.label(RichText::new("🎚️ Volumen de salida:").strong());
                ui.horizontal(|ui| {
                    let slider = egui::Slider::new(&mut new_volume, 0.0..=1.0).show_value(false);
                    if ui.add(slider).changed() {
                        send_update = true;
                    }
                    ui.label(format!("{}%", (new_volume * 100.0) as u32));
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    let mute_label = if muted { "🔊 Activar Audio" } else { "🔇 Silenciar" };
                    if ui.button(mute_label).clicked() {
                        new_muted = !muted;
                        send_update = true;
                    }

                    ui.add_space(8.0);

                    if ui.button(RichText::new("⏹ Terminar sesión").color(Color32::from_rgb(192, 57, 43))).clicked() {
                        disconnect = true;
                    }
                });

                ui.add_space(4.0);
                ui.separator();
                ui.label(
                    RichText::new("El control de audio afecta lo que escucha tu amigo.")
                        .small()
                        .color(Color32::from_rgb(120, 120, 120)),
                );
            });

        if send_update {
            self.host_audio_muted = new_muted;
            self.host_audio_volume = new_volume;
            self.bridge.cmd_tx.send(GuiCommand::AudioControl {
                muted: new_muted,
                volume: new_volume,
            }).ok();
        }

        if disconnect {
            self.bridge.cmd_tx.send(GuiCommand::Disconnect).ok();
            self.mode = AppMode::Home;
        }
    }
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    // Install Ring crypto provider for Rustls (required for wss:// connections)
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Apply strict OS-level process hardening (Anti-DLL hijacking, ASLR, DEP)
    if let Err(e) = apply_process_hardening() {
        eprintln!("Warning: Failed to apply process hardening: {:?}", e);
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FreeDesk — Remote Desktop")
            .with_inner_size([960.0, 620.0])
            .with_min_inner_size([700.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "FreeDesk",
        native_options,
        Box::new(|cc| Ok(Box::new(FreeDeskApp::new(cc)))),
    )
}
