//! Foreground-process inspection.
//!
//! When a clipboard update fires, we want to know which app produced it so
//! we can:
//!   - Drop captures from password managers (excluded_apps filter).
//!   - Stamp the clip with `source_app` for UX.
//!
//! The exposed surface is OS-agnostic; the Windows backend lives below an
//! `cfg(windows)` block and uses `GetForegroundWindow` +
//! `QueryFullProcessImageNameW`. macOS lands with v0.2 (Phase C).

/// What we know about the foreground app at clipboard-event time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceApp {
    /// Bare exe name, e.g. `"chrome.exe"` on Windows or bundle id on macOS.
    pub identifier: String,
    /// Active window title, if accessible. May be empty.
    pub window_title: String,
}

#[cfg(windows)]
mod imp {
    use super::SourceApp;

    use std::path::Path;

    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HWND, MAX_PATH};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    /// Read the foreground window's process exe name + title.
    ///
    /// Returns `None` if no foreground window or if the OS denies access
    /// (rare, e.g. UAC secure desktop).
    pub fn current() -> Option<SourceApp> {
        // SAFETY: Windows API, all returned handles are owned and closed.
        unsafe {
            let hwnd: HWND = GetForegroundWindow();
            if hwnd.0.is_null() {
                return None;
            }

            // Title — best effort.
            let mut title_buf = [0u16; 256];
            let title_len = GetWindowTextW(hwnd, &mut title_buf);
            let window_title = if title_len > 0 {
                String::from_utf16_lossy(&title_buf[..title_len as usize])
            } else {
                String::new()
            };

            // Process id from window.
            let mut pid: u32 = 0;
            let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == 0 {
                return None;
            }

            // Open the process with the minimum required rights so that
            // even integrity-restricted callers usually succeed.
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

            let mut buf = vec![0u16; MAX_PATH as usize];
            let mut size = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_FORMAT(0),
                PWSTR(buf.as_mut_ptr()),
                &mut size,
            );
            let _ = CloseHandle(process);
            if ok.is_err() {
                return None;
            }

            let path = String::from_utf16_lossy(&buf[..size as usize]);
            let identifier = Path::new(&path)
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
                .unwrap_or(path);

            Some(SourceApp {
                identifier,
                window_title,
            })
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::SourceApp;
    /// Non-Windows builds don't have a real implementation yet (v0.2).
    pub fn current() -> Option<SourceApp> {
        None
    }
}

/// OS-agnostic entry point used by the watcher and command layer.
pub fn current() -> Option<SourceApp> {
    imp::current()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn windows_call_returns_some_on_dev_machines() {
        // On a real Windows machine running the test, a foreground window
        // must exist (at minimum, the test runner). Smoke test: we expect
        // Some(_), but tolerate None for headless CI environments where no
        // desktop session exists.
        let result = current();
        if let Some(app) = result {
            assert!(!app.identifier.is_empty());
        }
    }

    #[test]
    fn source_app_construction() {
        let s = SourceApp {
            identifier: "chrome.exe".into(),
            window_title: "Klipo - Github".into(),
        };
        assert_eq!(s.identifier, "chrome.exe");
    }
}
