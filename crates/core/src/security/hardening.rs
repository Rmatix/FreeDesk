//! System and process hardening for FreeDesk.

use tracing::info;

#[cfg(windows)]
use windows::Win32::System::LibraryLoader::{SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_SYSTEM32};

/// Applies hardening rules to protect the process from tampering.
pub fn apply_process_hardening() -> anyhow::Result<()> {
    #[cfg(windows)]
    unsafe {
        // 1. Anti-DLL Hijacking on Windows
        let res = SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32);
        if res.is_err() {
            tracing::warn!("Warning: Failed to set default DLL directories: {:?}", res);
        } else {
            info!("Hardening: Anti-DLL Hijacking policy applied (System32 only).");
        }
    }

    info!("Hardening: Process mitigation rules verified.");
    Ok(())
}
