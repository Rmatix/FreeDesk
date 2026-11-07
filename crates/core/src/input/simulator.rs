//! Remote input simulation with Host Priority and Dual Cursor support.
//! Prevents cursor fighting and ensures the host PC always has hardware priority.

use crate::protocol::{InputEvent, MouseButton};
use crate::security::SessionPermissions;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
    MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CursorControlState {
    HostControlling,
    FriendControlling,
}

pub struct InputSimulator {
    _last_physical_pos: Mutex<(i32, i32)>,
    last_simulated_pos: Mutex<(i32, i32)>,
    last_host_input_time: Mutex<Instant>,
    pub friend_ghost_cursor: Mutex<Option<(f32, f32)>>,
}

impl Default for InputSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl InputSimulator {
    pub fn new() -> Self {
        let mut pt = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut pt);
        }
        Self {
            _last_physical_pos: Mutex::new((pt.x, pt.y)),
            last_simulated_pos: Mutex::new((pt.x, pt.y)),
            last_host_input_time: Mutex::new(Instant::now() - Duration::from_secs(10)),
            friend_ghost_cursor: Mutex::new(None),
        }
    }

    /// Handles an incoming remote input event.
    /// Returns CursorControlState indicating who currently has control.
    pub fn handle_event(
        &self,
        event: &InputEvent,
        perms: &SessionPermissions,
    ) -> anyhow::Result<CursorControlState> {
        // 1. Check if the local host is physically moving their mouse
        let mut current_pt = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut current_pt);
        }

        let mut sim_pos = self.last_simulated_pos.lock().unwrap();
        let mut last_host_time = self.last_host_input_time.lock().unwrap();

        // Calculate distance from last position that was simulated
        let dx = (current_pt.x - sim_pos.0).abs();
        let dy = (current_pt.y - sim_pos.1).abs();

        // If distance > 4 pixels and didn't come from simulated input, host physically moved mouse!
        if dx > 4 || dy > 4 {
            *last_host_time = Instant::now();
        }

        // Host has priority for 900ms after last physical movement
        let host_has_priority = last_host_time.elapsed() < Duration::from_millis(900);

        if host_has_priority {
            // Under host priority: record friend's position for ghost cursor, but DO NOT move Windows pointer
            if let InputEvent::MouseMove { x, y } = event {
                let mut ghost = self.friend_ghost_cursor.lock().unwrap();
                *ghost = Some((*x, *y));
            }
            return Ok(CursorControlState::HostControlling);
        }

        // Host is idle -> friend can move and click!
        let mut ghost = self.friend_ghost_cursor.lock().unwrap();
        *ghost = None; // Hide ghost cursor because friend moves real pointer

        match event {
            InputEvent::MouseMove { x, y } => {
                if !perms.can_control_mouse {
                    return Ok(CursorControlState::HostControlling);
                }
                Self::send_mouse_move(*x, *y)?;
                // Update simulated position
                let abs_x = (x.clamp(0.0, 1.0) * 65535.0) as i32;
                let abs_y = (y.clamp(0.0, 1.0) * 65535.0) as i32;
                *sim_pos = (abs_x, abs_y);
            }
            InputEvent::MouseDown { button } => {
                if !perms.can_control_mouse {
                    return Ok(CursorControlState::HostControlling);
                }
                Self::send_mouse_button(*button, true)?;
            }
            InputEvent::MouseUp { button } => {
                if !perms.can_control_mouse {
                    return Ok(CursorControlState::HostControlling);
                }
                Self::send_mouse_button(*button, false)?;
            }
            InputEvent::MouseWheel { delta_y } => {
                if !perms.can_control_mouse {
                    return Ok(CursorControlState::HostControlling);
                }
                Self::send_mouse_wheel(*delta_y)?;
            }
            InputEvent::KeyDown { vk_code } => {
                if !perms.can_control_keyboard {
                    return Ok(CursorControlState::HostControlling);
                }
                Self::send_key(*vk_code, true)?;
            }
            InputEvent::KeyUp { vk_code } => {
                if !perms.can_control_keyboard {
                    return Ok(CursorControlState::HostControlling);
                }
                Self::send_key(*vk_code, false)?;
            }
        }

        Ok(CursorControlState::FriendControlling)
    }

    fn send_mouse_move(norm_x: f32, norm_y: f32) -> anyhow::Result<()> {
        let clean_x = if norm_x.is_nan() || norm_x.is_infinite() {
            0.0
        } else {
            norm_x.clamp(0.0, 1.0)
        };
        let clean_y = if norm_y.is_nan() || norm_y.is_infinite() {
            0.0
        } else {
            norm_y.clamp(0.0, 1.0)
        };

        let abs_x = (clean_x * 65535.0) as i32;
        let abs_y = (clean_y * 65535.0) as i32;

        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: abs_x,
                    dy: abs_y,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }

    fn send_mouse_button(button: MouseButton, down: bool) -> anyhow::Result<()> {
        let flag = match (button, down) {
            (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
            (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
            (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
            (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
            (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
            (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
        };

        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: flag,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }

    fn send_mouse_wheel(delta_y: f32) -> anyhow::Result<()> {
        let wheel_delta = (delta_y * 120.0) as i32;

        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: wheel_delta as u32,
                    dwFlags: MOUSEEVENTF_WHEEL,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }

    fn send_key(vk_code: u16, down: bool) -> anyhow::Result<()> {
        let mut flags = windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS(0);
        if !down {
            flags |= KEYEVENTF_KEYUP;
        }

        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk_code),
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }
}
