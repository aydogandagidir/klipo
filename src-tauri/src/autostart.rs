//! "Run on login" toggle.
//!
//! Cross-platform shape:
//!   - Windows: writes a `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//!     value pointing at the absolute path of the running `.exe`. We use
//!     `HKCU` (per-user) intentionally — `HKLM` would require admin
//!     elevation, and this is a per-user clipboard manager.
//!   - macOS: TODO in Faz C — we'll write a `~/Library/LaunchAgents/*.plist`.
//!     For now `is_enabled()` returns `false` and `set_enabled(true)`
//!     returns an error so the UI can show a clear "not implemented yet"
//!     state on Mac.
//!
//! The registry value name is `Klipo` (matches the user-visible product
//! name) — adding or removing this key is idempotent.
//!
//! Errors: registry calls go through Win32 directly. We surface readable
//! `String` errors so the Settings UI can show what happened (most common
//! failure: a corporate roaming policy that denies HKCU writes).

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;

const REGISTRY_VALUE_NAME: &str = "Klipo";
#[cfg(target_os = "windows")]
const REGISTRY_RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

/// Whether autostart is currently enabled on this OS / user.
pub fn is_enabled() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        win::is_value_present()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
}

/// Toggle autostart on / off. `true` writes the registry / plist; `false`
/// removes it.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        if enabled {
            let exe = std::env::current_exe()
                .map_err(|e| format!("failed to read current exe path: {e}"))?;
            // Wrap the path in quotes so spaces don't break Run parsing.
            let value = format!("\"{}\"", exe.display());
            win::write_value(&value)
        } else {
            win::delete_value()
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Err("autostart on this platform is not implemented yet (macOS arrives in v0.2)".to_string())
    }
}

#[cfg(target_os = "windows")]
mod win {
    use super::*;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE,
        REG_CREATE_KEY_DISPOSITION, REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    /// Convert a Rust `&str` into a NUL-terminated UTF-16 buffer for the
    /// Win32 wide-string APIs.
    fn to_wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// Open the `Run` key with the given access mask. Caller must close.
    fn open_run_key(access_mask: u32) -> Result<HKEY, String> {
        let path = to_wide(REGISTRY_RUN_KEY);
        let mut handle = HKEY::default();
        // SAFETY: `path` is a valid NUL-terminated UTF-16 slice owned by us;
        // `handle` is a stack-allocated out-param; the access mask is valid
        // per the windows-rs Registry feature.
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                Some(0),
                windows::Win32::System::Registry::REG_SAM_FLAGS(access_mask),
                &mut handle,
            )
        };
        if status.is_err() {
            return Err(format!("RegOpenKeyExW failed: {status:?}"));
        }
        Ok(handle)
    }

    /// Drop wrapper for an open registry HKEY so we never leak handles even
    /// on the error path.
    struct OwnedKey(HKEY);
    impl Drop for OwnedKey {
        fn drop(&mut self) {
            // SAFETY: we own this handle; close is safe.
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }

    pub(super) fn is_value_present() -> Result<bool, String> {
        let key = match open_run_key(KEY_QUERY_VALUE.0) {
            Ok(k) => OwnedKey(k),
            Err(e) => return Err(e),
        };
        let value_name = to_wide(REGISTRY_VALUE_NAME);
        let mut size: u32 = 0;
        // SAFETY: handle from open_run_key is still alive (held by OwnedKey
        // until end of scope); name buffer is NUL-terminated; out-pointers
        // satisfy the API's nullability rules.
        let status = unsafe {
            RegQueryValueExW(
                key.0,
                PCWSTR(value_name.as_ptr()),
                None,
                None,
                None,
                Some(&mut size),
            )
        };
        if status.is_ok() {
            return Ok(true);
        }
        // ERROR_FILE_NOT_FOUND specifically means "no such value" — every
        // other code is a real error worth surfacing to the user.
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        Err(format!("RegQueryValueExW failed: {status:?}"))
    }

    pub(super) fn write_value(value: &str) -> Result<(), String> {
        // RegCreateKeyExW with the same path is idempotent when the key
        // exists, so this works even on first run before a prior write.
        let path = to_wide(REGISTRY_RUN_KEY);
        let mut handle = HKEY::default();
        let mut disposition = REG_CREATE_KEY_DISPOSITION::default();
        // SAFETY: pointers are owned local buffers; access mask valid.
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(path.as_ptr()),
                Some(0),
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                windows::Win32::System::Registry::REG_SAM_FLAGS(KEY_SET_VALUE.0),
                None,
                &mut handle,
                Some(&mut disposition),
            )
        };
        if status.is_err() {
            return Err(format!("RegCreateKeyExW failed: {status:?}"));
        }
        let key = OwnedKey(handle);

        let value_name = to_wide(REGISTRY_VALUE_NAME);
        let value_wide = to_wide(value);
        let bytes = unsafe {
            std::slice::from_raw_parts(
                value_wide.as_ptr() as *const u8,
                value_wide.len() * std::mem::size_of::<u16>(),
            )
        };
        // SAFETY: `bytes` is a slice into our owned `value_wide` buffer with
        // a length matching its byte size, satisfying RegSetValueExW's "data
        // pointer + length" contract.
        let status = unsafe {
            RegSetValueExW(
                key.0,
                PCWSTR(value_name.as_ptr()),
                Some(0),
                REG_SZ,
                Some(bytes),
            )
        };
        if status.is_err() {
            return Err(format!("RegSetValueExW failed: {status:?}"));
        }
        Ok(())
    }

    pub(super) fn delete_value() -> Result<(), String> {
        let key = match open_run_key(KEY_SET_VALUE.0) {
            Ok(k) => OwnedKey(k),
            Err(e) => return Err(e),
        };
        let value_name = to_wide(REGISTRY_VALUE_NAME);
        // SAFETY: handle valid; name NUL-terminated.
        let status = unsafe { RegDeleteValueW(key.0, PCWSTR(value_name.as_ptr())) };
        if status.is_ok() {
            return Ok(());
        }
        if status == ERROR_FILE_NOT_FOUND {
            // Toggling off when already off is a no-op.
            return Ok(());
        }
        Err(format!("RegDeleteValueW failed: {status:?}"))
    }
}
