//! Network bridge connecting egui with the Tokio networking and capture pipelines.

use freedesk_core::capture::ScreenCapture;
use freedesk_core::codec::{TileDecoder, TileEncoder};
use freedesk_core::input::InputSimulator;
use freedesk_core::protocol::{InputEvent, PeerMessage, RelayMessage};
use freedesk_core::security::SessionPermissions;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{error, info};

pub enum GuiCommand {
    Connect {
        target_id: String,
        pin: String,
        perms: SessionPermissions,
    },
    SendInput(InputEvent),
    Disconnect,
}

pub enum NetworkEvent {
    RelayStatus(String),
    IncomingConnection {
        viewer_id: String,
    },
    SessionStarted {
        peer_id: String,
    },
    NewFrame {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
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
    relay_addr: String,
    my_peer_id: String,
    cmd_rx: Receiver<GuiCommand>,
    event_tx: Sender<NetworkEvent>,
) {
    let clean_my_id = my_peer_id.replace(' ', "");

    info!("Connecting to relay at {}", relay_addr);
    let stream = match TcpStream::connect(&relay_addr).await {
        Ok(s) => {
            event_tx
                .send(NetworkEvent::RelayStatus(format!(
                    "🟢 Conectado al Relay ({})",
                    relay_addr
                )))
                .ok();
            s
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

    let (mut read_half, mut write_half) = stream.into_split();

    // 1. Register Host ID with Relay
    let reg_msg = RelayMessage::RegisterHost {
        peer_id: clean_my_id.clone(),
    };
    let payload = match reg_msg.serialize() {
        Ok(p) => p,
        Err(_) => return,
    };
    let len_bytes = (payload.len() as u32).to_be_bytes();
    if write_half.write_all(&len_bytes).await.is_err()
        || write_half.write_all(&payload).await.is_err()
    {
        return;
    }

    // Channel for outbound messages to relay
    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<RelayMessage>();

    // Outbound sender task
    tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if let Ok(bytes) = msg.serialize() {
                let len_bytes = (bytes.len() as u32).to_be_bytes();
                if write_half.write_all(&len_bytes).await.is_err()
                    || write_half.write_all(&bytes).await.is_err()
                {
                    break;
                }
            }
        }
    });

    let is_hosting = Arc::new(AtomicBool::new(false));
    let host_perms = Arc::new(std::sync::Mutex::new(SessionPermissions::full_control()));
    let my_id_clone = clean_my_id.clone();
    let is_hosting_flag = is_hosting.clone();

    // Inbound listener from relay
    let event_tx_clone = event_tx.clone();
    let out_tx_inbound = out_tx.clone();

    tokio::spawn(async move {
        let mut decoder = TileDecoder::new(1920, 1080);
        let mut len_buf = [0u8; 4];

        while read_half.read_exact(&mut len_buf).await.is_ok() {
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut buf = vec![0u8; len];
            if read_half.read_exact(&mut buf).await.is_err() {
                break;
            }

            if let Ok(relay_msg) = RelayMessage::deserialize(&buf) {
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
                        peer_id, connected, ..
                    } => {
                        if connected {
                            event_tx_clone
                                .send(NetworkEvent::SessionStarted {
                                    peer_id: peer_id.clone(),
                                })
                                .ok();

                            // If we are the host, start the screen capture streaming thread
                            if !is_hosting_flag.load(Ordering::SeqCst) {
                                is_hosting_flag.store(true, Ordering::SeqCst);
                                let flag = is_hosting_flag.clone();
                                let target_id = peer_id.clone();
                                let host_out = out_tx_inbound.clone();
                                let my_id = my_id_clone.clone();

                                thread::spawn(move || {
                                    run_host_capture_loop(flag, target_id, my_id, host_out);
                                });
                            }
                        }
                    }
                    RelayMessage::Forward {
                        from_id: _,
                        payload,
                        ..
                    } => {
                        // De-serialize peer message
                        if let Ok(peer_msg) = PeerMessage::deserialize(&payload) {
                            match peer_msg {
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
                                    let perms = host_perms.lock().unwrap().clone();
                                    let _ = InputSimulator::handle_event(&evt, &perms);
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
                out_tx.send(connect_msg).ok();
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
                        out_tx.send(fwd).ok();
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
                        out_tx.send(fwd).ok();
                    }
                }
            }
        }
    }
}

/// Host screen capture streaming loop running at 60 FPS
fn run_host_capture_loop(
    running: Arc<AtomicBool>,
    target_viewer_id: String,
    my_id: String,
    out_tx: tokio::sync::mpsc::UnboundedSender<RelayMessage>,
) {
    info!("Starting screen capture loop for viewer {}", target_viewer_id);

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

        if capture.capture_frame(&mut raw_rgba).is_ok() {
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
                    if out_tx.send(fwd).is_err() {
                        break;
                    }
                }
            }
        }

        // Target ~60 FPS (approx 16ms per frame)
        let elapsed = frame_start.elapsed();
        if elapsed < Duration::from_millis(16) {
            thread::sleep(Duration::from_millis(16) - elapsed);
        }
    }

    info!("Screen capture loop ended for viewer {}", target_viewer_id);
}
