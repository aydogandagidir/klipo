//! Native paste-out for every clip kind we support.
//!
//! For each kind:
//!   - **Text:** CF_UNICODETEXT → SendInput(Ctrl+V).
//!   - **HTML:** CF_HTML (custom format) + CF_UNICODETEXT fallback for plain
//!     consumers. The HTML payload already carries the Microsoft header.
//!   - **RTF:**  CF_RTF (custom format) + CF_UNICODETEXT fallback.
//!   - **File:** CF_HDROP — DROPFILES struct + concatenated wide paths.
//!     Pasting in Explorer / Outlook / Word inserts the file(s).
//!   - **Image:** CF_DIB — re-encoded PNG decoded back to BMP/DIB pixels.
//!
//! Sequence (called from a tokio command handler after `window.hide()`):
//!   1. `SetForegroundWindow(prev_hwnd)` to push focus to the user's
//!      previously-active app.
//!   2. Sleep ~150 ms — `WM_ACTIVATE` settle time.
//!   3. Open the clipboard, write the new payload.
//!   4. `SendInput(Ctrl+V down, V up, Ctrl up)`.

#![cfg(windows)]

use std::path::Path;
use std::time::Duration;

use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

use crate::storage::clips::{Clip, ClipKind};

const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;

#[derive(Debug, thiserror::Error)]
pub enum PasteError {
    #[error("OpenClipboard failed: {0}")]
    OpenClipboard(windows::core::Error),
    #[error("EmptyClipboard failed: {0}")]
    EmptyClipboard(windows::core::Error),
    #[error("GlobalAlloc failed: {0}")]
    GlobalAlloc(windows::core::Error),
    #[error("GlobalLock returned null")]
    GlobalLockNull,
    #[error("SetClipboardData failed: {0}")]
    SetClipboardData(windows::core::Error),
    #[error("SendInput dispatched 0/4 events")]
    SendInputDropped,
    #[error("clip is missing the payload required to paste this kind: {0}")]
    MissingPayload(&'static str),
    #[error("could not decode image payload: {0}")]
    ImageDecode(String),
    #[error("could not parse file path list as JSON: {0}")]
    FilePathParse(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Public paste entry. `clip` is the row we're pasting; `prev_hwnd_raw` is
/// the user's previous foreground HWND captured at hotkey-press time.
pub async fn paste_clip(
    clip: Clip,
    blob_root: Option<std::path::PathBuf>,
    prev_hwnd_raw: Option<isize>,
) -> Result<(), PasteError> {
    tracing::info!(
        target: "klipo::paste",
        kind = clip.kind.as_str(),
        size = clip.size_bytes,
        prev_hwnd = ?prev_hwnd_raw,
        "paste_clip start"
    );

    if let Some(raw) = prev_hwnd_raw {
        let hwnd = HWND(raw as *mut _);
        // SAFETY: SetForegroundWindow accepts any HWND; stale handle just
        // returns false. No UB.
        let restored = unsafe { SetForegroundWindow(hwnd) };
        tracing::info!(
            target: "klipo::paste",
            hwnd = ?raw,
            ok = restored.as_bool(),
            "SetForegroundWindow"
        );
    }

    tokio::time::sleep(Duration::from_millis(150)).await;

    // Build the platform-specific payload off the Tauri reactor.
    tokio::task::spawn_blocking(move || -> Result<(), PasteError> {
        // Pause our own watcher so it doesn't race with the writer for
        // OpenClipboard. Resumed in the guard's Drop on every exit path.
        let _guard = WatcherPauseGuard::new();

        if matches!(clip.kind, ClipKind::Image) {
            // Image paste delegates to `arboard` (CF_BITMAP + CF_DIB) plus an
            // additive CF_PNG for Chromium-based apps.
            write_image_via_arboard(&clip, blob_root.as_deref())?;
        } else {
            // SAFETY: each kind branch opens/closes the clipboard atomically.
            unsafe { write_for_kind(&clip, blob_root.as_deref())? };
        }

        // Re-assert foreground for the target window. Why: `arboard` (and
        // some Win32 clipboard ops) briefly take focus for their internal
        // owner window, which leaves SendInput pointing at the wrong app.
        // Cheap insurance — costs ~15 ms total.
        if let Some(raw) = prev_hwnd_raw {
            let hwnd = HWND(raw as *mut _);
            // SAFETY: HWND can be stale; SetForegroundWindow returns false in
            // that case. No UB.
            let _ = unsafe { SetForegroundWindow(hwnd) };
        }
        std::thread::sleep(Duration::from_millis(15));

        // SAFETY: SendInput is thread-safe per MSDN.
        unsafe { send_ctrl_v()? };
        tracing::info!(target: "klipo::paste", "paste sequence complete");
        Ok(())
    })
    .await
    .map_err(|_| PasteError::SendInputDropped)?
}

/// RAII guard that pauses the clipboard watcher for the lifetime of a paste.
///
/// Without this, our own `arboard::set_image` triggers `WM_CLIPBOARDUPDATE`
/// in our own watcher window, which then races with our `add_png_format`
/// call for `OpenClipboard` — and the loser silently drops its write.
struct WatcherPauseGuard;

impl WatcherPauseGuard {
    fn new() -> Self {
        crate::clipboard::watcher_windows::pause_watcher();
        // Tiny sleep gives any in-flight WM_CLIPBOARDUPDATE handler from a
        // previous user copy a chance to finish before we start writing.
        std::thread::sleep(Duration::from_millis(20));
        Self
    }
}

impl Drop for WatcherPauseGuard {
    fn drop(&mut self) {
        // Resume after a short delay so the OS event for our paste write
        // arrives at the now-paused watcher and gets discarded. If we
        // resumed immediately, our own write could still be queued and
        // capture-loop-by-bumping, which is harmless but noisy.
        std::thread::sleep(Duration::from_millis(150));
        crate::clipboard::watcher_windows::resume_watcher();
    }
}

/// Write the clip's image (PNG on disk) to the system clipboard.
///
/// Targets a wide compatibility surface:
///   1. **arboard** writes the bitmap formats Paint / Word / Photoshop want
///      (CF_BITMAP + CF_DIB on Windows).
///   2. We then **additively** register "PNG" as a clipboard format and write
///      the raw PNG bytes there. Chromium-based apps (any browser shell,
///      Discord, Slack, Notion, Obsidian) read this MIME-equivalent format
///      on paste.
///
/// "Additive" means we re-`OpenClipboard` after arboard finishes WITHOUT
/// `EmptyClipboard` — that preserves what arboard placed and just appends
/// our PNG record alongside it. Receivers pick the format they prefer.
fn write_image_via_arboard(clip: &Clip, blob_root: Option<&Path>) -> Result<(), PasteError> {
    let rel = clip
        .blob_path
        .as_deref()
        .ok_or(PasteError::MissingPayload("image blob_path"))?;
    let blob_root = blob_root.ok_or(PasteError::MissingPayload(
        "blob_root unavailable for image paste",
    ))?;
    let abs = blob_root.join(rel);
    let png_bytes = std::fs::read(&abs)?;

    // 1. Bitmap formats via arboard — Paint, Word, Photoshop friendly.
    let img = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png)
        .map_err(|e| PasteError::ImageDecode(e.to_string()))?
        .to_rgba8();
    let (width, height) = img.dimensions();
    let rgba_bytes = img.into_raw();

    let image_data = arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: rgba_bytes.into(),
    };

    {
        let mut clipboard =
            arboard::Clipboard::new().map_err(|e| PasteError::ImageDecode(e.to_string()))?;
        clipboard
            .set_image(image_data)
            .map_err(|e| PasteError::ImageDecode(e.to_string()))?;
        // Drop closes arboard's clipboard session so we can reopen below.
    }

    // 2. Additive PNG MIME for Chromium-based apps (browser shells / Discord / Slack / Notion).
    if let Err(e) = unsafe { add_png_format(&png_bytes) } {
        // Non-fatal: bitmap formats from arboard are still on the clipboard,
        // so Paint/Word still work. Browsers will fail "Failed to process
        // image" but we surface this in logs for diagnosis.
        tracing::warn!(
            target: "klipo::paste",
            error = %e,
            "could not add CF_PNG; browser-based paste targets may fail"
        );
    }

    tracing::info!(
        target: "klipo::paste",
        width,
        height,
        png_bytes = png_bytes.len(),
        "image written to clipboard (arboard + CF_PNG)"
    );
    Ok(())
}

/// Append a "PNG" clipboard format record carrying the raw PNG bytes.
///
/// SAFETY: caller has just finished a clipboard write via arboard and
/// arboard has closed its session. We re-open without `EmptyClipboard` so
/// previously-written formats are preserved.
unsafe fn add_png_format(png_bytes: &[u8]) -> Result<(), PasteError> {
    // SAFETY: opening with no owner; we close on every path.
    unsafe { OpenClipboard(None) }.map_err(PasteError::OpenClipboard)?;

    let result: Result<(), PasteError> = (|| {
        // SAFETY: registering a custom format is reentrant + idempotent.
        let png_format = unsafe { register_format("PNG") };
        if png_format == 0 {
            return Err(PasteError::SetClipboardData(
                windows::core::Error::from_win32(),
            ));
        }
        // SAFETY: GMEM_MOVEABLE + GlobalAlloc + GlobalLock pattern below.
        unsafe { write_bytes(png_format, png_bytes) }?;
        Ok(())
    })();

    // SAFETY: pair with the OpenClipboard above.
    let _ = unsafe { CloseClipboard() };
    result
}

unsafe fn write_for_kind(clip: &Clip, _blob_root: Option<&Path>) -> Result<(), PasteError> {
    // SAFETY: Open/Close paired below; every error path closes too.
    unsafe { OpenClipboard(None) }.map_err(PasteError::OpenClipboard)?;

    let result: Result<(), PasteError> = (|| {
        // SAFETY: clipboard is open per our pairing above.
        unsafe { EmptyClipboard() }.map_err(PasteError::EmptyClipboard)?;

        match clip.kind {
            ClipKind::Text => {
                let text = clip
                    .text_content
                    .as_deref()
                    .ok_or(PasteError::MissingPayload("text"))?;
                unsafe { write_text(CF_UNICODETEXT, text) }?;
            }
            ClipKind::Html => {
                let html = clip
                    .text_content
                    .as_deref()
                    .ok_or(PasteError::MissingPayload("html"))?;
                let html_format = unsafe { register_format("HTML Format") };
                if html_format != 0 {
                    unsafe { write_bytes(html_format, html.as_bytes()) }?;
                }
                // Also write a plain-text fallback so non-HTML targets paste
                // a sensible string. We naively strip HTML tags here — a
                // full extractor lands in M3.2.x if real-world need shows up.
                let plain = strip_html_tags(html);
                unsafe { write_text(CF_UNICODETEXT, &plain) }?;
            }
            ClipKind::Rtf => {
                let rtf = clip
                    .text_content
                    .as_deref()
                    .ok_or(PasteError::MissingPayload("rtf"))?;
                let rtf_format = unsafe { register_format("Rich Text Format") };
                if rtf_format != 0 {
                    // RTF on the wire is 8-bit; convert lossily back from
                    // String (we kept original byte values via char roundtrip
                    // in capture).
                    let bytes: Vec<u8> = rtf.chars().map(|c| c as u8).collect();
                    unsafe { write_bytes(rtf_format, &bytes) }?;
                }
                let plain = strip_rtf(rtf);
                unsafe { write_text(CF_UNICODETEXT, &plain) }?;
            }
            ClipKind::File => {
                let json = clip
                    .text_content
                    .as_deref()
                    .ok_or(PasteError::MissingPayload("file"))?;
                let paths: Vec<String> = serde_json::from_str(json)
                    .map_err(|e| PasteError::FilePathParse(e.to_string()))?;

                // 1. CF_HDROP — Explorer, Outlook, Office accept this and
                //    actually copy the files when the user pastes.
                unsafe { write_file_paths(&paths) }?;

                // 2. Plain-text fallback. Chromium-based apps (browser
                //    shells, Discord, Slack, Notion) reject CF_HDROP from
                //    Ctrl+V for security reasons (only drag-and-drop loads
                //    files). Writing the
                //    path list as text means the user at least sees the file
                //    path in the target app — they can then paste that path
                //    into the app's upload dialog manually. Future M5.x.1
                //    adds drag-from-Klipo for the proper UX.
                let plain = paths.join("\n");
                unsafe { write_text(CF_UNICODETEXT, &plain) }?;
            }
            ClipKind::Image => {
                // Handled out-of-band via arboard before this fn is called —
                // see `paste_clip` for the dispatch. We should never reach
                // here, but if we do (future bug), be loud.
                return Err(PasteError::MissingPayload(
                    "image kind reached native write path (should be handled by arboard)",
                ));
            }
        }
        Ok(())
    })();

    // SAFETY: pair with OpenClipboard above.
    let _ = unsafe { CloseClipboard() };
    result
}

// ---------- Low-level clipboard write helpers ----------

unsafe fn register_format(name: &str) -> u32 {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: wide is null-terminated and lives for the call.
    unsafe { RegisterClipboardFormatW(windows::core::PCWSTR(wide.as_ptr())) }
}

unsafe fn write_text(format: u32, text: &str) -> Result<(), PasteError> {
    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = utf16.len() * std::mem::size_of::<u16>();
    let payload = unsafe { utf16_to_global(&utf16, bytes) }?;
    // SAFETY: payload owned by us until SetClipboardData transfers it.
    unsafe { SetClipboardData(format, Some(payload)) }.map_err(PasteError::SetClipboardData)?;
    Ok(())
}

unsafe fn write_bytes(format: u32, bytes: &[u8]) -> Result<(), PasteError> {
    let payload = unsafe { bytes_to_global(bytes) }?;
    // SAFETY: payload ownership transferred to clipboard on success.
    unsafe { SetClipboardData(format, Some(payload)) }.map_err(PasteError::SetClipboardData)?;
    Ok(())
}

/// Build CF_HDROP DROPFILES struct + concatenated wide paths and SetClipboardData.
unsafe fn write_file_paths(paths: &[String]) -> Result<(), PasteError> {
    // DROPFILES = 20 bytes (offset 20 to file list, fWide=1).
    // File list = each path UTF-16 + NUL, then a final extra NUL terminator.
    let mut wide: Vec<u16> = Vec::new();
    for p in paths {
        wide.extend(p.encode_utf16());
        wide.push(0);
    }
    wide.push(0); // double NUL at the end

    let dropfiles_size = 20usize;
    let total_bytes = dropfiles_size + wide.len() * 2;

    // SAFETY: GMEM_MOVEABLE is required for clipboard ownership transfer.
    let h = unsafe { GlobalAlloc(GMEM_MOVEABLE, total_bytes) }.map_err(PasteError::GlobalAlloc)?;
    let hglobal: HGLOBAL = h;
    // SAFETY: just allocated; no other lock holder.
    let dst = unsafe { GlobalLock(hglobal) } as *mut u8;
    if dst.is_null() {
        return Err(PasteError::GlobalLockNull);
    }

    // DROPFILES layout (little-endian on x86/x64):
    //   u32 pFiles = 20 (offset to file list)
    //   POINT pt   = (0, 0)            (8 bytes)
    //   BOOL fNC   = 0                 (4 bytes)
    //   BOOL fWide = 1                 (4 bytes)
    // SAFETY: dst points to `total_bytes`; we write exactly 20 bytes here.
    unsafe {
        std::ptr::write_unaligned(dst as *mut u32, 20u32); // pFiles
        std::ptr::write_unaligned(dst.add(4) as *mut u32, 0u32); // POINT.x
        std::ptr::write_unaligned(dst.add(8) as *mut u32, 0u32); // POINT.y
        std::ptr::write_unaligned(dst.add(12) as *mut i32, 0i32); // fNC
        std::ptr::write_unaligned(dst.add(16) as *mut i32, 1i32); // fWide
        std::ptr::copy_nonoverlapping(
            wide.as_ptr() as *const u8,
            dst.add(dropfiles_size),
            wide.len() * 2,
        );
    }

    // SAFETY: pair with the GlobalLock above.
    let _ = unsafe { GlobalUnlock(hglobal) };

    let handle = HANDLE(hglobal.0);
    // SAFETY: handle ownership transferred to clipboard on success.
    unsafe { SetClipboardData(CF_HDROP, Some(handle)) }.map_err(PasteError::SetClipboardData)?;
    Ok(())
}

unsafe fn utf16_to_global(utf16: &[u16], byte_size: usize) -> Result<HANDLE, PasteError> {
    // SAFETY: GMEM_MOVEABLE required by SetClipboardData.
    let h = unsafe { GlobalAlloc(GMEM_MOVEABLE, byte_size) }.map_err(PasteError::GlobalAlloc)?;
    let hglobal: HGLOBAL = h;
    // SAFETY: freshly allocated, no contention.
    let dst = unsafe { GlobalLock(hglobal) } as *mut u16;
    if dst.is_null() {
        return Err(PasteError::GlobalLockNull);
    }
    // SAFETY: dst valid for utf16.len() u16 writes (byte_size matches).
    unsafe { std::ptr::copy_nonoverlapping(utf16.as_ptr(), dst, utf16.len()) };
    let _ = unsafe { GlobalUnlock(hglobal) };
    Ok(HANDLE(hglobal.0))
}

unsafe fn bytes_to_global(bytes: &[u8]) -> Result<HANDLE, PasteError> {
    // SAFETY: GMEM_MOVEABLE required.
    let h = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }.map_err(PasteError::GlobalAlloc)?;
    let hglobal: HGLOBAL = h;
    // SAFETY: freshly allocated.
    let dst = unsafe { GlobalLock(hglobal) } as *mut u8;
    if dst.is_null() {
        return Err(PasteError::GlobalLockNull);
    }
    // SAFETY: dst valid for bytes.len() writes.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len()) };
    let _ = unsafe { GlobalUnlock(hglobal) };
    Ok(HANDLE(hglobal.0))
}

unsafe fn send_ctrl_v() -> Result<(), PasteError> {
    let down = KEYBD_EVENT_FLAGS(0);
    let up = KEYEVENTF_KEYUP;

    let inputs: [INPUT; 4] = [
        keyboard_input(VK_CONTROL.0, down),
        keyboard_input(VK_V.0, down),
        keyboard_input(VK_V.0, up),
        keyboard_input(VK_CONTROL.0, up),
    ];

    let size = std::mem::size_of::<INPUT>() as i32;
    // SAFETY: inputs is a stack array of valid INPUT structs.
    let sent = unsafe { SendInput(&inputs, size) };
    tracing::info!(target: "klipo::paste", sent, "SendInput Ctrl+V");
    if sent == 0 {
        return Err(PasteError::SendInputDropped);
    }
    Ok(())
}

fn keyboard_input(vk: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

// (PNG → DIB conversion was here before M3.2-fix; replaced by arboard which
// handles the format negotiation cross-platform. See `write_image_via_arboard`.)

// ---------- Plain-text fallbacks for HTML / RTF ----------

fn strip_html_tags(html: &str) -> String {
    // Skip the Microsoft CF_HTML header (everything up to the first '<') so
    // paste targets that don't speak HTML get the body only.
    let body_start = html.find('<').unwrap_or(0);
    let mut out = String::with_capacity(html.len() - body_start);
    let mut in_tag = false;
    for ch in html[body_start..].chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_rtf(rtf: &str) -> String {
    // Crude: drop control words + braces; keep readable text. Adequate as a
    // fallback for non-RTF-aware paste targets.
    let mut out = String::with_capacity(rtf.len());
    let mut chars = rtf.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' | '}' => {}
            '\\' => {
                // Skip control word: backslash + letters until non-letter.
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_alphabetic() {
                        chars.next();
                    } else {
                        break;
                    }
                }
                // Skip optional numeric parameter.
                while let Some(&next) = chars.peek() {
                    if next.is_ascii_digit() || next == '-' {
                        chars.next();
                    } else {
                        break;
                    }
                }
                if let Some(&' ') = chars.peek() {
                    chars.next();
                }
            }
            _ => out.push(ch),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_drops_tags() {
        let html = "<html><body><b>Hello</b> world</body></html>";
        assert_eq!(strip_html_tags(html), "Hello world");
    }

    #[test]
    fn strip_rtf_drops_control_words() {
        let rtf = r"{\rtf1\ansi\deff0 {\fonttbl{\f0 Helvetica;}}\f0\fs24 Hello world}";
        let stripped = strip_rtf(rtf);
        assert!(stripped.contains("Hello world"));
        assert!(!stripped.contains("\\rtf1"));
    }
}
