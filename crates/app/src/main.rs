#![windows_subsystem = "windows"]

use eframe::egui::{self, CentralPanel};
use freedesk_app::network_client::{GuiCommand, NetworkBridge, NetworkEvent};
use freedesk_app::ui::{apply_theme, ConnectionAction, ConnectionPanelState, SessionAction, SessionViewState};
use freedesk_core::security::apply_process_hardening;
use tracing::info;

enum AppMode {
    Home,
    InSession(SessionViewState),
}

struct FreeDeskApp {
    mode: AppMode,
    conn_panel: ConnectionPanelState,
    bridge: NetworkBridge,
}

impl FreeDeskApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        let conn_panel = ConnectionPanelState::default();

        // Default to local relay or configurable IP
        let relay_addr = std::env::var("FREEDESK_RELAY").unwrap_or_else(|_| "127.0.0.1:7070".to_string());
        let bridge = NetworkBridge::start(
            relay_addr,
            conn_panel.my_peer_id.clone(),
            conn_panel.my_session_pin.clone(),
        );

        Self {
            mode: AppMode::Home,
            conn_panel,
            bridge,
        }
    }
}

impl eframe::App for FreeDeskApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll incoming network events from background bridge
        while let Ok(evt) = self.bridge.event_rx.try_recv() {
            match evt {
                NetworkEvent::RelayStatus(status) => {
                    self.conn_panel.relay_status = status;
                }
                NetworkEvent::IncomingConnection { viewer_id } => {
                    info!("Incoming connection from {}", viewer_id);
                }
                NetworkEvent::SessionStarted { peer_id } => {
                    info!("Session started with peer {}", peer_id);
                    if matches!(self.mode, AppMode::Home) {
                        self.mode = AppMode::InSession(SessionViewState::new(peer_id));
                    }
                }
                NetworkEvent::NewFrame { width, height, rgba } => {
                    if let AppMode::InSession(session) = &mut self.mode {
                        session.update_frame(ctx, &rgba, width, height);
                    }
                }
                NetworkEvent::SessionEnded => {
                    info!("Session closed remotely");
                    self.mode = AppMode::Home;
                }
            }
        }

        CentralPanel::default().show(ctx, |ui| {
            match &mut self.mode {
                AppMode::Home => {
                    if let Some(action) = self.conn_panel.render(ui) {
                        match action {
                            ConnectionAction::Connect { target_id, pin, perms } => {
                                info!("Requesting connection to target ID {}", target_id);
                                self.bridge.cmd_tx.send(GuiCommand::Connect {
                                    target_id: target_id.clone(),
                                    pin,
                                    perms,
                                }).ok();

                                self.mode = AppMode::InSession(SessionViewState::new(target_id));
                            }
                        }
                    }
                }
                AppMode::InSession(session) => {
                    let (action, inputs) = session.render(ctx, ui);

                    // Forward remote user input events to the host
                    for evt in inputs {
                        self.bridge.cmd_tx.send(GuiCommand::SendInput(evt)).ok();
                    }

                    if let Some(SessionAction::Disconnect) = action {
                        info!("Session terminated by user");
                        self.bridge.cmd_tx.send(GuiCommand::Disconnect).ok();
                        self.mode = AppMode::Home;
                    }
                }
            }
        });

        // Repaint at 60 FPS
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

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
