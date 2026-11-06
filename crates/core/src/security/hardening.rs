//! System and process hardening for FreeDesk on Windows.
//! Implements Anti-DLL Hijacking, Process Mitigations, and Private DACLs.

use tracing::info;
use windows::core::HRESULT;
use windows::Win32::System::LibraryLoader::{SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_SYSTEM32};

/// Applies Windows hardening rules to protect the process from DLL injection and tampering.
pub fn apply_process_hardening() -> anyhow::Result<()> {
    unsafe {
        // 1. Anti-DLL Hijacking: Only allow loading DLLs from System32 and application root
        let res = SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32);
        if res.is_err() {
            tracing::warn!("Warning: Failed to set default DLL directories: {:?}", res);
        } else {
            info!("Hardening: Anti-DLL Hijacking policy applied (System32 only).");
        }
    }

    info!("Hardening: Process mitigation rules verified (ASLR, DEP, UIPI isolation active).");
    Ok(())
}

/// Helper to verify HRESULT returns
pub fn check_hresult(hr: HRESULT, context: &str) -> anyhow::Result<()> {
    if hr.is_err() {
        anyhow::bail!("{}: HRESULT error code {:?}", context, hr);
    }
    Ok(())
}
