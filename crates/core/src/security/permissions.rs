//! Session permission model for FreeDesk.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionPermissions {
    pub can_view: bool,
    pub can_control_mouse: bool,
    pub can_control_keyboard: bool,
    pub can_listen_audio: bool,
    pub can_sync_clipboard: bool,
    pub can_transfer_files: bool,
}

impl Default for SessionPermissions {
    fn default() -> Self {
        Self {
            can_view: true,
            can_control_mouse: false,
            can_control_keyboard: false,
            can_listen_audio: true,
            can_sync_clipboard: false,
            can_transfer_files: false,
        }
    }
}

impl SessionPermissions {
    /// Full control preset for authorized users
    pub fn full_control() -> Self {
        Self {
            can_view: true,
            can_control_mouse: true,
            can_control_keyboard: true,
            can_listen_audio: true,
            can_sync_clipboard: true,
            can_transfer_files: true,
        }
    }

    /// Read-only / view-only preset
    pub fn view_only() -> Self {
        Self {
            can_view: true,
            can_control_mouse: false,
            can_control_keyboard: false,
            can_listen_audio: true,
            can_sync_clipboard: false,
            can_transfer_files: false,
        }
    }
}
