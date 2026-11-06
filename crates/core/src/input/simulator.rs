//! Remote input simulation using Win32 SendInput.
//! Supports mouse movement, mouse clicks, wheel scrolling, and keyboard keystrokes.

use crate::protocol::{InputEvent, MouseButton};
use crate::security::SessionPermissions;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
    MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_VIRTUALDESK, MOUSEEVENTF_WHEEL, MOUSEINPUT,
    VIRTUAL_KEY,
};

pub struct InputSimulator;

impl InputSimulator {
    /// Executes an incoming remote input event if permissions allow it.
    pub fn handle_event(event: &InputEvent, perms: &SessionPermissions) -> anyhow::Result<()> {
        match event {
            InputEvent::MouseMove { x, y } => {
                if !perms.can_control_mouse {
                    return Ok(());
                }
                Self::send_mouse_move(*x, *y)?;
            }
            InputEvent::MouseDown { button } => {
                if !perms.can_control_mouse {
                    return Ok(());
                }
                Self::send_mouse_button(*button, true)?;
            }
            InputEvent::MouseUp { button } => {
                if !perms.can_control_mouse {
                    return Ok(());
                }
                Self::send_mouse_button(*button, false)?;
            }
            InputEvent::MouseWheel { delta_y } => {
                if !perms.can_control_mouse {
                    return Ok(());
                }
                Self::send_mouse_wheel(*delta_y)?;
            }
            InputEvent::KeyDown { vk_code } => {
                if !perms.can_control_keyboard {
                    return Ok(());
                }
                Self::send_key(*vk_code, true)?;
            }
            InputEvent::KeyUp { vk_code } => {
                if !perms.can_control_keyboard {
                    return Ok(());
                }
                Self::send_key(*vk_code, false)?;
            }
        }
        Ok(())
    }

    fn send_mouse_move(norm_x: f32, norm_y: f32) -> anyhow::Result<()> {
        let clean_x = if norm_x.is_nan() || norm_x.is_infinite() { 0.0 } else { norm_x.clamp(0.0, 1.0) };
        let clean_y = if norm_y.is_nan() || norm_y.is_infinite() { 0.0 } else { norm_y.clamp(0.0, 1.0) };

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
