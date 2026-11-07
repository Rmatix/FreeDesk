//! Network protocol definition for FreeDesk.

use crate::codec::DeltaFrame;
use crate::security::SessionPermissions;
use serde::{Deserialize, Serialize};

/// Messages routed via the Relay server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelayMessage {
    /// Host announces its peer ID to the relay
    RegisterHost { peer_id: String },
    /// Relay confirms registration
    HostRegistered { peer_id: String, success: bool },
    /// Viewer requests connection to target host
    ConnectToHost {
        target_id: String,
        viewer_id: String,
    },
    /// Notification of peer connection status
    ConnectStatus {
        peer_id: String,
        connected: bool,
        error: Option<String>,
    },
    /// End-to-end encrypted payload routed between Host and Viewer
    Forward {
        target_id: String,
        from_id: String,
        payload: Vec<u8>,
    },
    /// Keep-alive ping
    Ping,
    /// Keep-alive pong
    Pong,
}

/// End-to-end messages sent between Host and Viewer (wrapped inside RelayMessage::Forward)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerMessage {
    /// Handshake step 1: Exchange X25519 public key
    KeyExchange { public_key: [u8; 32] },
    /// Handshake step 2: Auth verification with PIN and requested permissions
    AuthRequest {
        pin: String,
        requested_permissions: SessionPermissions,
    },
    /// Handshake step 3: Host decision on connection
    AuthResponse {
        accepted: bool,
        error_reason: Option<String>,
        granted_permissions: SessionPermissions,
    },
    /// Stream video update (Tile-based Delta frame)
    Video(DeltaFrame),
    /// Stream audio chunk (PCM / Opus-ready)
    Audio(AudioChunk),
    /// Input event from Viewer to Host
    Input(InputEvent),
    /// Remote audio control from Viewer to Host (e.g. from the floating audio panel)
    AudioControl { muted: bool, volume: f32 },
    /// Cursor synchronization and host mouse priority status
    CursorSync {
        host_controlling: bool,
        ghost_x: Option<f32>,
        ghost_y: Option<f32>,
    },
    /// Viewer requests UAC administrator elevation on host
    RequestElevation,
    /// Host decision on elevation request
    ElevationResponse {
        accepted: bool,
        message: String,
    },
    /// In-session chat message between Host and Viewer
    Chat {
        sender: String,
        message: String,
    },
    /// Clipboard text synchronization between Host and Viewer
    ClipboardSync {
        text: String,
    },
    /// Remote Ctrl+Alt+Del injection request
    SendCtrlAltDel,
    /// Terminate session gracefully
    Disconnect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioChunk {
    pub sample_rate: u32,
    pub channels: u16,
    pub pcm_data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEvent {
    MouseMove { x: f32, y: f32 },
    MouseDown { button: MouseButton },
    MouseUp { button: MouseButton },
    MouseWheel { delta_y: f32 },
    KeyDown { vk_code: u16 },
    KeyUp { vk_code: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl RelayMessage {
    pub fn serialize(&self) -> anyhow::Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| anyhow::anyhow!("Relay serialize failed: {:?}", e))
    }

    pub fn deserialize(bytes: &[u8]) -> anyhow::Result<Self> {
        bincode::deserialize(bytes).map_err(|e| anyhow::anyhow!("Relay deserialize failed: {:?}", e))
    }
}

impl PeerMessage {
    pub fn serialize(&self) -> anyhow::Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| anyhow::anyhow!("Peer serialize failed: {:?}", e))
    }

    pub fn deserialize(bytes: &[u8]) -> anyhow::Result<Self> {
        bincode::deserialize(bytes).map_err(|e| anyhow::anyhow!("Peer deserialize failed: {:?}", e))
    }
}
