//! FreeDesk Relay Server with WebSocket & HTTP Health Check Support for Render.
//! Zero-Knowledge peer routing, 9-digit peer ID management, and anti-abuse shielding.

use freedesk_core::protocol::RelayMessage;
use freedesk_core::security::RateLimiter;
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::tungstenite::Message;
use tracing::{info, warn};

type Tx = mpsc::UnboundedSender<Vec<u8>>;
type PeerMap = Arc<Mutex<HashMap<String, Tx>>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let port = std::env::var("PORT").unwrap_or_else(|_| "10000".to_string());
    let bind_addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&bind_addr).await?;

    info!("🚀 FreeDesk Relay running on {}", bind_addr);
    info!("🛡️ Zero-Knowledge architecture: Payloads are End-to-End encrypted between peers.");
    info!("☁️ Render Health Check (HTTP 200) & WebSocket (wss://) fully active.");

    let peers: PeerMap = Arc::new(Mutex::new(HashMap::new()));
    // Max 30 connection attempts per minute per public IP
    let rate_limiter = RateLimiter::new(30, Duration::from_secs(60));

    loop {
        let (stream, client_addr) = listener.accept().await?;
        let ip = client_addr.ip();

        // Whitelist internal Render health check addresses and loopback from rate limiting
        let is_internal = ip.is_loopback() || match ip {
            IpAddr::V4(v4) => v4.is_private(),
            IpAddr::V6(_) => false,
        };

        if !is_internal {
            let ip_key = ip.to_string();
            if !rate_limiter.check_and_record(&ip_key) {
                warn!("⛔ Rate limit exceeded for IP: {}. Connection dropped.", ip_key);
                drop(stream);
                continue;
            }
        }

        let peers_clone = peers.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, client_addr, peers_clone).await {
                // Log non-healthcheck errors
                let err_str = format!("{:?}", e);
                if !err_str.contains("Handshake not finished") && !err_str.contains("Connection reset") {
                    warn!("Client {} disconnected: {}", client_addr, err_str);
                }
            }
        });
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    peers: PeerMap,
) -> anyhow::Result<()> {
    // Peek at initial bytes to distinguish Render HTTP health checks (HEAD/GET /healthz) from WebSockets
    let mut peek_buf = [0u8; 4096];
    let peek_len = stream.peek(&mut peek_buf).await?;
    let peek_str = String::from_utf8_lossy(&peek_buf[..peek_len]).to_lowercase();

    // If it's a Render health check (HEAD or GET without "upgrade")
    if peek_str.starts_with("head ") || (peek_str.starts_with("get ") && !peek_str.contains("upgrade")) {
        let mut discard = vec![0u8; peek_len];
        let _ = stream.read(&mut discard).await;

        let response = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";
        stream.write_all(response).await?;
        stream.flush().await?;
        return Ok(());
    }

    // Upgrade TCP stream to WebSocket
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();

    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let mut current_peer_id: Option<String> = None;

    // Outbound forwarder task
    let outbound_task = tokio::spawn(async move {
        while let Some(msg_bytes) = rx.recv().await {
            if ws_write.send(Message::Binary(msg_bytes.into())).await.is_err() {
                break;
            }
        }
    });

    // Inbound frame receiver
    while let Some(msg_result) = ws_read.next().await {
        let ws_msg = match msg_result {
            Ok(m) => m,
            Err(_) => break,
        };

        let payload = match ws_msg {
            Message::Binary(bytes) => bytes,
            Message::Ping(_) => {
                tx.send(RelayMessage::Pong.serialize()?).ok();
                continue;
            }
            Message::Close(_) => break,
            _ => continue,
        };

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
