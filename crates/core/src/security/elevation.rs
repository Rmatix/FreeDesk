//! Process elevation and UAC helpers for Windows.
//!
//! Provides inspection of process integrity/elevation levels and requesting
//! elevation via ShellExecuteW with the "runas" verb, matching AnyDesk's elevation flow.

use tracing::{info, warn};

/// Checks if the current process is running with administrative privileges (elevated token).
pub fn is_elevated() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{CloseHandle, HANDLE};
        use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
        use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

        unsafe {
            let mut token = HANDLE::default();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_ok() {
                let mut elevation = TOKEN_ELEVATION::default();
                let mut return_length = 0u32;
                let res = GetTokenInformation(
                    token,
                    TokenElevation,
                    Some(&mut elevation as *mut _ as *mut _),
                    std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                    &mut return_length,
                );
                let _ = CloseHandle(token);
                if res.is_ok() {
                    return elevation.TokenIsElevated != 0;
                }
            }
        }
    }
    false
}

/// Requests UAC elevation by re-launching the current executable with the "runas" verb.
/// An optional command-line parameter can be supplied (e.g. "--elevated").
pub fn request_elevation(extra_args: Option<&str>) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
        use windows::core::HSTRING;

        if let Ok(current_exe) = std::env::current_exe() {
            let exe_str = current_exe.to_string_lossy().to_string();
            let verb = HSTRING::from("runas");
            let file = HSTRING::from(exe_str);
            let params = HSTRING::from(extra_args.unwrap_or("--elevated"));

            info!("🛡️ Requesting UAC elevation via ShellExecuteW...");
            unsafe {
                let res = ShellExecuteW(
                    None,
                    &verb,
                    &file,
                    &params,
                    None,
                    SW_SHOW,
                );
                // Return code > 32 indicates success
                let code = res.0 as isize;
                if code > 32 {
                    info!("✅ Elevation request spawned successfully (code: {})", code);
                    return true;
                } else {
                    warn!("❌ Elevation request was denied or cancelled (code: {})", code);
                }
            }
        }
    }
    false
}
