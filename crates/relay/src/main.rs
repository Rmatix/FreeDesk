//! FreeDesk Relay Server.
//! Zero-Knowledge peer routing, 9-digit peer ID management, and anti-abuse shielding.

use freedesk_core::protocol::RelayMessage;
use freedesk_core::security::RateLimiter;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

type Tx = mpsc::UnboundedSender<Vec<u8>>;
type PeerMap = Arc<Mutex<HashMap<String, Tx>>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let bind_addr = "0.0.0.0:7070";
    let listener = TcpListener::bind(bind_addr).await?;
    info!("🚀 FreeDesk Relay Server running on {}", bind_addr);
    info!("🛡️ Zero-Knowledge architecture: Payloads are End-to-End encrypted between peers.");

    let peers: PeerMap = Arc::new(Mutex::new(HashMap::new()));
    // Max 10 connection attempts per minute per IP
    let rate_limiter = RateLimiter::new(10, Duration::from_secs(60));

    loop {
        let (stream, client_addr) = listener.accept().await?;
        let ip_key = client_addr.ip().to_string();

        if !rate_limiter.check_and_record(&ip_key) {
            warn!("⛔ Rate limit exceeded for IP: {}. Connection dropped.", ip_key);
            drop(stream);
            continue;
        }

        let peers_clone = peers.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, client_addr, peers_clone).await {
                warn!("Client {} disconnected: {:?}", client_addr, e);
            }
        });
    }
}

async fn handle_client(
    stream: TcpStream,
    addr: SocketAddr,
    peers: PeerMap,
) -> anyhow::Result<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let mut current_peer_id: Option<String> = None;

    let (mut read_half, mut write_half) = stream.into_split();

    // Outbound forwarder task
    let outbound_task = tokio::spawn(async move {
        while let Some(msg_bytes) = rx.recv().await {
            let len = (msg_bytes.len() as u32).to_be_bytes();
            if write_half.write_all(&len).await.is_err() {
                break;
            }
            if write_half.write_all(&msg_bytes).await.is_err() {
                break;
            }
        }
    });

    // Inbound frame receiver
    let mut len_buf = [0u8; 4];
    while read_half.read_exact(&mut len_buf).await.is_ok() {
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 10 * 1024 * 1024 {
            // Protect against packet bombing (>10MB packet)
            anyhow::bail!("Packet length exceeded maximum allowable limit: {} bytes", len);
        }

        let mut payload = vec![0u8; len];
        read_half.read_exact(&mut payload).await?;

        let relay_msg = match RelayMessage::deserialize(&payload) {
            Ok(msg) => msg,
            Err(e) => {
                warn!("Malformed packet from {}: {:?}", addr, e);
                continue;
            }
        };

        match relay_msg {
            RelayMessage::RegisterHost { peer_id } => {
                let mut peers_guard = peers.lock().await;
                if peers_guard.contains_key(&peer_id) {
                    warn!("Host registration rejected: ID {} is already active", peer_id);
                    let resp = RelayMessage::HostRegistered {
                        peer_id: peer_id.clone(),
                        success: false,
                    };
                    tx.send(resp.serialize()?).ok();
                } else {
                    info!("Registering host ID: {} from {}", peer_id, addr);
                    current_peer_id = Some(peer_id.clone());
                    peers_guard.insert(peer_id.clone(), tx.clone());

                    let resp = RelayMessage::HostRegistered {
                        peer_id,
                        success: true,
                    };
                    tx.send(resp.serialize()?).ok();
                }
            }
            RelayMessage::ConnectToHost {
                target_id,
                viewer_id,
            } => {
                info!("Viewer {} connecting to Host {}", viewer_id, target_id);
                current_peer_id = Some(viewer_id.clone());
                peers.lock().await.insert(viewer_id.clone(), tx.clone());

                let peers_guard = peers.lock().await;
                if let Some(host_tx) = peers_guard.get(&target_id) {
                    let forward_msg = RelayMessage::ConnectStatus {
                        peer_id: viewer_id.clone(),
                        connected: true,
                        error: None,
                    };
                    host_tx.send(forward_msg.serialize()?).ok();

                    let status_msg = RelayMessage::ConnectStatus {
                        peer_id: target_id,
                        connected: true,
                        error: None,
                    };
                    tx.send(status_msg.serialize()?).ok();
                } else {
                    let err_msg = RelayMessage::ConnectStatus {
                        peer_id: target_id,
                        connected: false,
                        error: Some("Host ID not found or offline".to_string()),
                    };
                    tx.send(err_msg.serialize()?).ok();
                }
            }
            RelayMessage::Forward {
                target_id,
                from_id: _,
                payload,
            } => {
                // SECURITY FIX: Enforce sender identity from socket state to prevent spoofing
                let verified_from_id = match &current_peer_id {
                    Some(id) => id.clone(),
                    None => {
                        warn!("Rejecting forward from unauthenticated client {}", addr);
                        continue;
                    }
                };

                let peers_guard = peers.lock().await;
                if let Some(target_tx) = peers_guard.get(&target_id) {
                    let fwd = RelayMessage::Forward {
                        target_id: target_id.clone(),
                        from_id: verified_from_id,
                        payload,
                    };
                    target_tx.send(fwd.serialize()?).ok();
                }
            }
            RelayMessage::Ping => {
                let pong = RelayMessage::Pong;
                tx.send(pong.serialize()?).ok();
            }
            RelayMessage::Pong => {}
            _ => {}
        }
    }

    // Cleanup on disconnect
    if let Some(id) = current_peer_id {
        info!("Peer {} disconnected, removing from registry", id);
        peers.lock().await.remove(&id);
    }

    outbound_task.abort();
    Ok(())
}
