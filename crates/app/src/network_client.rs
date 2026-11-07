//! Network bridge connecting egui with the Tokio networking (WebSocket), capture, and input synchronization.

use freedesk_core::capture::ScreenCapture;
use freedesk_core::codec::{TileDecoder, TileEncoder};
use freedesk_core::input::{CursorControlState, InputSimulator};
use freedesk_core::protocol::{InputEvent, PeerMessage, RelayMessage};
use freedesk_core::security::SessionPermissions;
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info};

pub enum GuiCommand {
    Connect {
        target_id: String,
        pin: String,
        perms: SessionPermissions,
    },
    AcceptIncoming {
        viewer_id: String,
        perms: SessionPermissions,
    },
    RejectIncoming {
        viewer_id: String,
    },
    SendInput(InputEvent),
    AudioControl {
        muted: bool,
        volume: f32,
    },
    Disconnect,
}

pub enum NetworkEvent {
    RelayStatus(String),
    IncomingConnectionPrompt {
        viewer_id: String,
        perms: SessionPermissions,
    },
    SessionStarted {
        peer_id: String,
    },
    NewFrame {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    CursorUpdate {
        host_controlling: bool,
        ghost_pos: Option<(f32, f32)>,
    },
    SessionEnded,
}

pub struct NetworkBridge {
    pub cmd_tx: Sender<GuiCommand>,
    pub event_rx: Receiver<NetworkEvent>,
}

impl NetworkBridge {
    pub fn start(relay_addr: String, my_peer_id: String, _my_pin: String) -> Self {
        let (cmd_tx, cmd_rx) = channel::<GuiCommand>();
        let (event_tx, event_rx) = channel::<NetworkEvent>();

        thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    error!("Failed to create Tokio runtime: {:?}", e);
                    return;
                }
            };

            rt.block_on(async move {
                run_network_loop(relay_addr, my_peer_id, cmd_rx, event_tx).await;
            });
        });

        Self { cmd_tx, event_rx }
    }
}

async fn run_network_loop(
    relay_input: String,
    my_peer_id: String,
    cmd_rx: Receiver<GuiCommand>,
    event_tx: Sender<NetworkEvent>,
) {
    let clean_my_id = my_peer_id.replace(' ', "");

    // Format proper WebSocket URL
    let trimmed = relay_input.trim();
    let stripped = trimmed
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("wss://")
        .trim_start_matches("ws://")
        .trim_end_matches('/');

    let ws_url = if stripped.contains("onrender.com") || stripped.contains(".com") || stripped.contains(".net") || stripped.contains(".org") || stripped.contains(".io") {
        format!("wss://{}", stripped)
    } else {
        format!("ws://{}", stripped)
    };

    info!("Connecting to WebSocket relay at {}", ws_url);
    event_tx
        .send(NetworkEvent::RelayStatus(format!(
            "🟡 Conectando a {}",
            ws_url
        )))
        .ok();

    let (ws_stream, _) = match connect_async(&ws_url).await {
        Ok(res) => {
            event_tx
                .send(NetworkEvent::RelayStatus(format!(
                    "🟢 Conectado al Relay Seguro ({})",
                    ws_url
                )))
                .ok();
            res
        }
        Err(e) => {
            event_tx
                .send(NetworkEvent::RelayStatus(format!(
                    "🔴 Error de conexión con Relay: {:?}",
                    e
                )))
                .ok();
            return;
        }
    };

    let (mut ws_write, mut ws_read) = ws_stream.split();

    // 1. Register Host ID with Relay
    let reg_msg = RelayMessage::RegisterHost {
        peer_id: clean_my_id.clone(),
    };
    if let Ok(payload) = reg_msg.serialize() {
        if ws_write.send(Message::Binary(payload.into())).await.is_err() {
            return;
        }
    }

    // Bounded channel to prevent queue lag: size of 2 drops old frames automatically!
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<RelayMessage>(2);

    // Outbound sender task
    tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if let Ok(bytes) = msg.serialize() {
                if ws_write.send(Message::Binary(bytes.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    let is_hosting = Arc::new(AtomicBool::new(false));
    let host_perms = Arc::new(std::sync::Mutex::new(SessionPermissions::full_control()));
    let input_simulator = Arc::new(InputSimulator::new());
    let my_id_clone = clean_my_id.clone();
    let is_hosting_flag = is_hosting.clone();

    // Inbound listener from relay
    let event_tx_clone = event_tx.clone();
    let out_tx_inbound = out_tx.clone();
    let sim_clone = input_simulator.clone();
    let host_perms_clone = host_perms.clone(); // clone before moving into async block

    tokio::spawn(async move {
        let mut decoder = TileDecoder::new(1280, 720);

        while let Some(msg_result) = ws_read.next().await {
            let ws_msg = match msg_result {
                Ok(m) => m,
                Err(_) => break,
            };

            let payload = match ws_msg {
                Message::Binary(bytes) => bytes,
                Message::Close(_) => break,
                _ => continue,
            };

            if let Ok(relay_msg) = RelayMessage::deserialize(&payload) {
                match relay_msg {
                    RelayMessage::HostRegistered { success, .. } => {
                        if success {
                            event_tx_clone
                                .send(NetworkEvent::RelayStatus(
                                    "🟢 Registrado y Listo para Recibir Conexiones".to_string(),
                                ))
                                .ok();
                        }
                    }
                    RelayMessage::ConnectStatus {
                        peer_id, connected, error,
                    } => {
                        if connected {
                            // Prompt host for incoming connection approval!
                            event_tx_clone
                                .send(NetworkEvent::IncomingConnectionPrompt {
                                    viewer_id: peer_id.clone(),
                                    perms: SessionPermissions::full_control(),
                                })
                                .ok();
                        } else if let Some(err) = error {
                            event_tx_clone
                                .send(NetworkEvent::RelayStatus(format!("🔴 Error: {}", err)))
                                .ok();
                        }
                    }
                    RelayMessage::Forward {
                        from_id,
                        payload,
                        ..
                    } => {
                        if let Ok(peer_msg) = PeerMessage::deserialize(&payload) {
                            match peer_msg {
                                PeerMessage::AuthResponse { accepted, granted_permissions, .. } => {
                                    if accepted {
                                        event_tx_clone
                                            .send(NetworkEvent::SessionStarted { peer_id: from_id })
                                            .ok();
                                    } else {
                                        event_tx_clone
                                            .send(NetworkEvent::RelayStatus("🔴 Conexión rechazada por el anfitrión".to_string()))
                                            .ok();
                                        event_tx_clone.send(NetworkEvent::SessionEnded).ok();
                                    }
                                }
                                PeerMessage::Video(delta) => {
                                    if decoder.apply_delta(&delta).is_ok() {
                                        event_tx_clone
                                            .send(NetworkEvent::NewFrame {
                                                width: delta.width,
                                                height: delta.height,
                                                rgba: decoder.frame_buffer.clone(),
                                            })
                                            .ok();
                                    }
                                }
                                PeerMessage::Input(evt) => {
                                    let perms = host_perms_clone.lock().unwrap().clone();
                                    let state = sim_clone.handle_event(&evt, &perms);

                                    // Check host mouse priority and sync ghost cursor
                                    if let Ok(st) = state {
                                        let (host_ctrl, ghost_pos) = match st {
                                            CursorControlState::HostControlling => {
                                                let ghost = *sim_clone.friend_ghost_cursor.lock().unwrap();
                                                (true, ghost)
                                            }
                                            CursorControlState::FriendControlling => (false, None),
                                        };

                                        // Notify viewer about host priority and ghost cursor
                                        let sync_msg = PeerMessage::CursorSync {
                                            host_controlling: host_ctrl,
                                            ghost_x: ghost_pos.map(|p| p.0),
                                            ghost_y: ghost_pos.map(|p| p.1),
                                        };
                                        if let Ok(sync_bytes) = sync_msg.serialize() {
                                            let fwd = RelayMessage::Forward {
                                                target_id: from_id.clone(),
                                                from_id: my_id_clone.clone(),
                                                payload: sync_bytes,
                                            };
                                            out_tx_inbound.try_send(fwd).ok();
                                        }
                                    }
                                }
                                PeerMessage::CursorSync { host_controlling, ghost_x, ghost_y } => {
                                    let ghost = match (ghost_x, ghost_y) {
                                        (Some(x), Some(y)) => Some((x, y)),
                                        _ => None,
                                    };
                                    event_tx_clone.send(NetworkEvent::CursorUpdate {
                                        host_controlling,
                                        ghost_pos: ghost,
                                    }).ok();
                                }
                                PeerMessage::Disconnect => {
                                    is_hosting_flag.store(false, Ordering::SeqCst);
                                    event_tx_clone.send(NetworkEvent::SessionEnded).ok();
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    // GUI Command loop
    let mut current_target_peer: Option<String> = None;
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            GuiCommand::Connect { target_id, .. } => {
                current_target_peer = Some(target_id.clone());
                let connect_msg = RelayMessage::ConnectToHost {
                    target_id,
                    viewer_id: clean_my_id.clone(),
                };
                let _ = out_tx.send(connect_msg).await;
            }
            GuiCommand::AcceptIncoming { viewer_id, perms } => {
                *host_perms.lock().unwrap() = perms;
                current_target_peer = Some(viewer_id.clone());

                // Send AuthResponse ACCEPTED to viewer
                let resp = PeerMessage::AuthResponse {
                    accepted: true,
                    error_reason: None,
                    granted_permissions: perms,
                };
                if let Ok(p) = resp.serialize() {
                    let fwd = RelayMessage::Forward {
                        target_id: viewer_id.clone(),
                        from_id: clean_my_id.clone(),
                        payload: p,
                    };
                    let _ = out_tx.send(fwd).await;
                }

                // Launch host screen streaming loop
                if !is_hosting.load(Ordering::SeqCst) {
                    is_hosting.store(true, Ordering::SeqCst);
                    let flag = is_hosting.clone();
                    let target_id = viewer_id;
                    let host_out = out_tx.clone();
                    let my_id = clean_my_id.clone();

                    thread::spawn(move || {
                        run_host_capture_loop(flag, target_id, my_id, host_out);
                    });
                }
            }
            GuiCommand::RejectIncoming { viewer_id } => {
                let resp = PeerMessage::AuthResponse {
                    accepted: false,
                    error_reason: Some("Conexión rechazada por el usuario".to_string()),
                    granted_permissions: SessionPermissions::default(),
                };
                if let Ok(p) = resp.serialize() {
                    let fwd = RelayMessage::Forward {
                        target_id: viewer_id,
                        from_id: clean_my_id.clone(),
                        payload: p,
                    };
                    let _ = out_tx.send(fwd).await;
                }
            }
            GuiCommand::SendInput(evt) => {
                if let Some(target) = &current_target_peer {
                    let peer_msg = PeerMessage::Input(evt);
                    if let Ok(payload) = peer_msg.serialize() {
                        let fwd = RelayMessage::Forward {
                            target_id: target.clone(),
                            from_id: clean_my_id.clone(),
                            payload,
                        };
                        // try_send prevents input queue lag!
                        out_tx.try_send(fwd).ok();
                    }
                }
            }
            GuiCommand::AudioControl { muted, volume } => {
                if let Some(target) = &current_target_peer {
                    let peer_msg = PeerMessage::AudioControl { muted, volume };
                    if let Ok(payload) = peer_msg.serialize() {
                        let fwd = RelayMessage::Forward {
                            target_id: target.clone(),
                            from_id: clean_my_id.clone(),
                            payload,
                        };
                        out_tx.try_send(fwd).ok();
                    }
                }
            }
            GuiCommand::Disconnect => {
                if let Some(target) = current_target_peer.take() {
                    let peer_msg = PeerMessage::Disconnect;
                    if let Ok(payload) = peer_msg.serialize() {
                        let fwd = RelayMessage::Forward {
                            target_id: target,
                            from_id: clean_my_id.clone(),
                            payload,
                        };
                        let _ = out_tx.send(fwd).await;
                    }
                }
            }
        }
    }
}

/// Ultra-optimized Host screen capture loop — DXGI preferred, GDI fallback.
///
/// Performance design (derived from AnyDesk REA analysis):
///  • DXGI Desktop Duplication: GPU surface copy, ~0.5 ms latency vs ~8 ms GDI.
///  • DXGI AcquireNextFrame blocks waiting for a new frame, so identical frames
///    are skipped at zero CPU cost (natural frame rate limiting).
///  • Drop policy: try_send drops frames if the WebSocket send buffer is full,
///    keeping the viewer always current with the freshest frame.
///  • Keyframe every 120 frames (~4 s at 30 FPS) to resync after packet loss.
fn run_host_capture_loop(
    running: Arc<AtomicBool>,
    target_viewer_id: String,
    my_id: String,
    out_tx: tokio::sync::mpsc::Sender<RelayMessage>,
) {
    info!("🎥 Starting DXGI/GDI capture loop for viewer {}", target_viewer_id);

    let mut capture = match ScreenCapture::new() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to initialize screen capture: {:?}", e);
            return;
        }
    };

    let mut encoder = TileEncoder::new(capture.width, capture.height);
    let mut raw_rgba = Vec::new();
    let mut frame_count = 0u64;

    while running.load(Ordering::SeqCst) {
        let frame_start = std::time::Instant::now();

        match capture.capture_frame(&mut raw_rgba) {
            Ok(()) => {
                // Keyframe every 120 sent frames (~4 s resync interval)
                let force_keyframe = frame_count % 120 == 0;
                let delta = encoder.encode_frame(&raw_rgba, force_keyframe);
                frame_count += 1;

                if !delta.tiles.is_empty() || force_keyframe {
                    let peer_msg = PeerMessage::Video(delta);
                    if let Ok(payload) = peer_msg.serialize() {
                        let fwd = RelayMessage::Forward {
                            target_id: target_viewer_id.clone(),
                            from_id: my_id.clone(),
                            payload,
                        };
                        // try_send: drops frame if channel full — viewer always gets freshest frame
                        out_tx.try_send(fwd).ok();
                    }
                }
            }
            Err(_) => {
                // DXGI timeout (no desktop change) — normal, not an error.
                // Brief sleep prevents CPU spin while waiting for next frame.
                thread::sleep(Duration::from_millis(5));
                continue;
            }
        }

        // Adaptive pacing: target 33 ms/frame (30 FPS).
        // If encoding was fast, sleep the remainder; if slow, skip sleep entirely.
        let elapsed = frame_start.elapsed();
        let target = Duration::from_millis(33);
        if elapsed < target {
            thread::sleep(target - elapsed);
        }
    }

    info!("🛑 Capture loop ended for viewer {} ({} frames sent)", target_viewer_id, frame_count);
}
