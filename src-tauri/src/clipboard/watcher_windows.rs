//! Windows native clipboard watcher.
//!
//! Format priority (richest → simplest), evaluated on every
//! `WM_CLIPBOARDUPDATE`:
//!   1. `CF_HDROP`         → file paths           (kind = `file`)
//!   2. `CF_HBITMAP / CF_DIB` → image (PNG re-encoded) (kind = `image`)
//!   3. `CF_HTML`          → HTML fragment         (kind = `html`)
//!   4. `CF_RTF`           → RTF                   (kind = `rtf`)
//!   5. `CF_UNICODETEXT`   → plain text            (kind = `text`)
//!
//! Single-instance via global `SENDER` static. Architecture:
//!
//! ```text
//!   ┌─────────────────────────┐  WM_CLIPBOARDUPDATE   ┌─────────────────┐
//!   │  Windows OS              │ ────────────────────► │ message-only    │
//!   │  (clipboard cache)       │                       │ window's WndProc │
//!   └─────────────────────────┘                       └────────┬────────┘
//!                                                              │ capture_event
//!                                                              ▼
//!                                            ┌──────────────────────────┐
//!                                            │ tokio::mpsc::UnboundedTx │
//!                                            └────────────┬─────────────┘
//!                                                         │
//!                                  pipeline::run on tokio └─► Storage::insert_clip
//! ```

#![cfg(windows)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// While Klipo itself is writing to the clipboard (paste path), the OS still
/// fires WM_CLIPBOARDUPDATE for our own writes. We don't want the watcher to
/// race with the writer for `OpenClipboard`, so the paste path flips this
/// flag and the WndProc skips capture while it's set.
///
/// Hash-based dedup already prevents inserting duplicate rows, but the
/// race still corrupted multi-format paste (CF_PNG add couldn't open the
/// clipboard while the watcher held it). This flag fixes that completely.
pub(crate) static WATCHER_PAUSED: AtomicBool = AtomicBool::new(false);

pub fn pause_watcher() {
    WATCHER_PAUSED.store(true, Ordering::SeqCst);
}

pub fn resume_watcher() {
    WATCHER_PAUSED.store(false, Ordering::SeqCst);
}

use tokio::sync::mpsc::UnboundedSender;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, OpenClipboard,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
    TranslateMessage, CW_USEDEFAULT, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_CLIPBOARDUPDATE, WNDCLASSW,
};

use super::normalize;
use super::source_app;
use super::{CapturedKind, ClipboardEvent};

static SENDER: OnceLock<UnboundedSender<ClipboardEvent>> = OnceLock::new();

pub fn spawn(tx: UnboundedSender<ClipboardEvent>) -> std::io::Result<()> {
    SENDER.set(tx).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "clipboard watcher already started",
        )
    })?;

    thread::Builder::new()
        .name("klipo-clipboard-watcher".to_string())
        .spawn(|| {
            // SAFETY: this thread owns the message-only window for its
            // entire lifetime; no other thread references the HWND.
            unsafe {
                if let Err(e) = run_message_pump() {
                    tracing::error!(
                        target: "klipo::watcher",
                        error = %e,
                        "clipboard watcher thread terminated"
                    );
                }
            }
        })?;

    tracing::info!(target: "klipo::watcher", "Windows clipboard watcher thread spawned");
    Ok(())
}

unsafe fn run_message_pump() -> windows::core::Result<()> {
    let class_name: PCWSTR = w!("KlipoClipboardWatcher");
    let window_name: PCWSTR = w!("Klipo Clipboard Watcher");

    // SAFETY: GetModuleHandleW(None) is documented null-name behavior.
    let hinstance = unsafe { GetModuleHandleW(None) }?;

    let wnd_class = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: class_name,
        ..Default::default()
    };

    // SAFETY: WNDCLASSW POD with valid pointers; tolerate already-registered.
    let atom = unsafe { RegisterClassW(&wnd_class) };
    if atom == 0 {
        let err = windows::core::Error::from_win32();
        // ERROR_CLASS_ALREADY_EXISTS == 1410 — fine.
        if err.code().0 as u32 != 1410 {
            return Err(err);
        }
    }

    // SAFETY: arguments are all valid; HWND lifetime ends with this thread.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            window_name,
            WINDOW_STYLE(0),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            Some(HWND_MESSAGE),
            None,
            Some(hinstance.into()),
            None,
        )
    }?;

    // SAFETY: hwnd is freshly-created.
    unsafe { AddClipboardFormatListener(hwnd) }?;

    tracing::info!(
        target: "klipo::watcher",
        hwnd = ?hwnd.0,
        "AddClipboardFormatListener attached"
    );

    let mut msg = MSG::default();
    loop {
        // SAFETY: msg is stack-allocated; hwnd is valid.
        let r = unsafe { GetMessageW(&mut msg, Some(hwnd), 0, 0) };
        if r.0 == 0 {
            break;
        }
        if r.0 == -1 {
            tracing::warn!(target: "klipo::watcher", "GetMessageW returned -1");
            break;
        }
        // SAFETY: msg populated.
        let _ = unsafe { TranslateMessage(&msg) };
        unsafe { DispatchMessageW(&msg) };
    }

    Ok(())
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        // Skip our own paste writes — see WATCHER_PAUSED docs.
        if WATCHER_PAUSED.load(Ordering::SeqCst) {
            tracing::debug!(target: "klipo::watcher", "skipping update (watcher paused)");
        } else {
            // SAFETY: capture_event opens/closes clipboard atomically.
            if let Some(event) = unsafe { capture_event() } {
                if let Some(tx) = SENDER.get() {
                    let _ = tx.send(event);
                }
            }
        }
    }
    // SAFETY: documented default fallthrough.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Open the clipboard, probe formats in priority order, build a
/// `ClipboardEvent` from the first hit. Returns `None` if no supported
/// format is present or the clipboard is locked.
unsafe fn capture_event() -> Option<ClipboardEvent> {
    if unsafe { OpenClipboard(None) }.is_err() {
        return None;
    }

    let event = unsafe { capture_inner() };

    // SAFETY: pair with the OpenClipboard above. Always run.
    let _ = unsafe { CloseClipboard() };
    event
}

unsafe fn capture_inner() -> Option<ClipboardEvent> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let source = source_app::current();

    // 1. CF_HDROP — file paths
    if normalize::has_format(normalize::CF_HDROP) {
        if let Some(paths) = unsafe { normalize::read_file_paths() } {
            tracing::info!(target: "klipo::watcher", count = paths.len(), "captured file list");
            // Encode as JSON for storage; paste path will JSON-decode.
            let json = serde_json::to_string(&paths).ok()?;
            let size = json.len() as u64;
            return Some(ClipboardEvent {
                kind: CapturedKind::File,
                text: Some(json),
                bytes: None,
                size_bytes: size,
                source_app: source.as_ref().map(|s| s.identifier.clone()),
                source_window_title: source.as_ref().map(|s| s.window_title.clone()),
                captured_at_ms: now_ms,
            });
        }
    }

    // 2. CF_DIB / CF_DIBV5 — image
    if normalize::has_format(normalize::CF_DIBV5) || normalize::has_format(normalize::CF_DIB) {
        if let Some((png_bytes, _hash)) = unsafe { normalize::read_image_as_png() } {
            let size = png_bytes.len() as u64;
            tracing::info!(target: "klipo::watcher", bytes = size, "captured image");
            return Some(ClipboardEvent {
                kind: CapturedKind::Image,
                text: None,
                bytes: Some(png_bytes),
                size_bytes: size,
                source_app: source.as_ref().map(|s| s.identifier.clone()),
                source_window_title: source.as_ref().map(|s| s.window_title.clone()),
                captured_at_ms: now_ms,
            });
        }
    }

    // 3. CF_HTML — HTML fragment (with Microsoft header)
    let html_id = normalize::html_format_id();
    if normalize::has_format(html_id) {
        if let Some(html) = unsafe { normalize::read_html() } {
            let size = html.len() as u64;
            tracing::info!(target: "klipo::watcher", bytes = size, "captured html");
            return Some(ClipboardEvent {
                kind: CapturedKind::Html,
                text: Some(html),
                bytes: None,
                size_bytes: size,
                source_app: source.as_ref().map(|s| s.identifier.clone()),
                source_window_title: source.as_ref().map(|s| s.window_title.clone()),
                captured_at_ms: now_ms,
            });
        }
    }

    // 4. CF_RTF — Rich Text Format
    let rtf_id = normalize::rtf_format_id();
    if normalize::has_format(rtf_id) {
        if let Some(rtf) = unsafe { normalize::read_rtf() } {
            let size = rtf.len() as u64;
            tracing::info!(target: "klipo::watcher", bytes = size, "captured rtf");
            return Some(ClipboardEvent {
                kind: CapturedKind::Rtf,
                text: Some(rtf),
                bytes: None,
                size_bytes: size,
                source_app: source.as_ref().map(|s| s.identifier.clone()),
                source_window_title: source.as_ref().map(|s| s.window_title.clone()),
                captured_at_ms: now_ms,
            });
        }
    }

    // 5. CF_UNICODETEXT — plain text fallback
    if let Some(text) = unsafe { normalize::read_unicode_text() } {
        let size = text.len() as u64;
        tracing::info!(target: "klipo::watcher", bytes = size, "captured text");
        return Some(ClipboardEvent {
            kind: CapturedKind::Text,
            text: Some(text),
            bytes: None,
            size_bytes: size,
            source_app: source.as_ref().map(|s| s.identifier.clone()),
            source_window_title: source.as_ref().map(|s| s.window_title.clone()),
            captured_at_ms: now_ms,
        });
    }

    None
}
