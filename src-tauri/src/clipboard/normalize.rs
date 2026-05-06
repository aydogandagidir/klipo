//! Format-specific read helpers for the Windows clipboard.
//!
//! All `read_*` functions assume `OpenClipboard` is already open by the
//! caller and that the caller will pair it with `CloseClipboard`. This lets
//! the watcher decide format priority once per `WM_CLIPBOARDUPDATE` and
//! avoid re-opening the clipboard.
//!
//! Format priority (richest → simplest), matched in `watcher_windows.rs`:
//!   1. `CF_HDROP`         → file paths           (kind = `file`)
//!   2. `CF_HBITMAP / CF_DIB` → image (BMP)        (kind = `image`)
//!   3. `CF_HTML`          → HTML fragment         (kind = `html`)
//!   4. `CF_RTF`           → RTF                   (kind = `rtf`)
//!   5. `CF_UNICODETEXT`   → plain text            (kind = `text`)

#![cfg(windows)]

use windows::core::w;
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    GetClipboardData, IsClipboardFormatAvailable, RegisterClipboardFormatW,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

/// Standard CF_* values we use directly. (windows-rs has them in submodules
/// that drift across versions; the constants are stable.)
pub const CF_UNICODETEXT: u32 = 13;
pub const CF_HDROP: u32 = 15;
/// Device-Independent Bitmap — preferred image format from screenshots.
pub const CF_DIB: u32 = 8;
pub const CF_DIBV5: u32 = 17;

/// Hard cap matches `docs/storage.md` §6 (50 MB) and `storage::blob::MAX_BLOB_BYTES`.
pub const MAX_PAYLOAD_BYTES: usize = 50 * 1024 * 1024;

/// Look up custom clipboard format ids that Windows registers globally per
/// session. Returns 0 on failure (caller should treat that as "not available").
pub fn html_format_id() -> u32 {
    // SAFETY: PCWSTR static literal; RegisterClipboardFormatW is reentrant
    // and idempotent — same id returned across calls.
    unsafe { RegisterClipboardFormatW(w!("HTML Format")) }
}

pub fn rtf_format_id() -> u32 {
    // Microsoft's documented name; same string Word, WordPad, browsers use.
    unsafe { RegisterClipboardFormatW(w!("Rich Text Format")) }
}

/// Returns true if the clipboard currently advertises this format. Caller
/// must already hold the clipboard via `OpenClipboard`.
pub fn has_format(format: u32) -> bool {
    if format == 0 {
        return false;
    }
    // SAFETY: Windows API reads its own static state; safe while clipboard
    // is open.
    unsafe { IsClipboardFormatAvailable(format).is_ok() }
}

// ---------- Plain text (CF_UNICODETEXT) ----------

/// Read the active CF_UNICODETEXT payload as a String. Empty/missing → None.
///
/// # Safety
/// Caller MUST hold the clipboard via `OpenClipboard` and pair with
/// `CloseClipboard` after this call returns.
pub unsafe fn read_unicode_text() -> Option<String> {
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) }.ok()?;
    if handle.0.is_null() {
        return None;
    }
    let hglobal = HGLOBAL(handle.0);
    let raw = unsafe { GlobalLock(hglobal) } as *const u16;
    if raw.is_null() {
        return None;
    }
    let max_chars = MAX_PAYLOAD_BYTES / 2;
    let mut len = 0usize;
    // SAFETY: CF_UNICODETEXT is documented null-terminated; bounded to avoid
    // runaway reads on a corrupted clipboard.
    while len < max_chars && unsafe { *raw.add(len) } != 0 {
        len += 1;
    }
    let slice = unsafe { std::slice::from_raw_parts(raw, len) };
    let text = String::from_utf16_lossy(slice);
    let _ = unsafe { GlobalUnlock(hglobal) };
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

// ---------- HTML / RTF custom formats ----------

/// Read a registered-format payload as raw bytes. Used for CF_HTML and CF_RTF
/// where the payload has UTF-8 (HTML) or 8-bit (RTF) encoding rather than
/// UTF-16. Caller decodes appropriately.
unsafe fn read_global_bytes(format: u32) -> Option<Vec<u8>> {
    let handle: HANDLE = unsafe { GetClipboardData(format) }.ok()?;
    if handle.0.is_null() {
        return None;
    }
    let hglobal = HGLOBAL(handle.0);
    let size = unsafe { GlobalSize(hglobal) };
    if size == 0 || size > MAX_PAYLOAD_BYTES {
        return None;
    }
    let raw = unsafe { GlobalLock(hglobal) } as *const u8;
    if raw.is_null() {
        return None;
    }
    let slice = unsafe { std::slice::from_raw_parts(raw, size) };
    let bytes = slice.to_vec();
    let _ = unsafe { GlobalUnlock(hglobal) };
    Some(bytes)
}

/// Read CF_HTML (Microsoft HTML Clipboard Format). The payload is UTF-8
/// with a header like:
/// ```text
/// Version:0.9
/// StartHTML:00000097
/// EndHTML:00000337
/// StartFragment:00000131
/// EndFragment:00000301
/// <html>…
/// ```
/// We return the *full payload* (header + body); paste reconstructs it back
/// into the same wire format. Storage stores the header too so paste targets
/// (browsers, Word) interpret styling correctly.
///
/// # Safety
/// Caller MUST hold the clipboard via `OpenClipboard` and pair with
/// `CloseClipboard` after this call returns.
pub unsafe fn read_html() -> Option<String> {
    let format = html_format_id();
    if format == 0 {
        return None;
    }
    let bytes = unsafe { read_global_bytes(format) }?;
    String::from_utf8(bytes).ok()
}

/// Read CF_RTF — payload is 8-bit (Windows-1252) typically. RTF spec is
/// ASCII-tolerant; we pass through as a string lossy-decoded. Paste writes
/// the same bytes back.
///
/// # Safety
/// Caller MUST hold the clipboard via `OpenClipboard` and pair with
/// `CloseClipboard` after this call returns.
pub unsafe fn read_rtf() -> Option<String> {
    let format = rtf_format_id();
    if format == 0 {
        return None;
    }
    let bytes = unsafe { read_global_bytes(format) }?;
    // RTF bodies use \uNNNN escapes for non-ASCII, so plain UTF-8 fails;
    // ISO-8859-1 / cp1252 lossy decode preserves bytes 0..255.
    let text: String = bytes.iter().map(|&b| b as char).collect();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

// ---------- File list (CF_HDROP) ----------

/// Read CF_HDROP file path list as a Vec<PathBuf>-equivalent of strings.
///
/// # Safety
/// Caller MUST hold the clipboard via `OpenClipboard` and pair with
/// `CloseClipboard` after this call returns. The HDROP handle is owned by
/// the clipboard subsystem; we only borrow it for the duration of this fn.
pub unsafe fn read_file_paths() -> Option<Vec<String>> {
    let handle = unsafe { GetClipboardData(CF_HDROP) }.ok()?;
    if handle.0.is_null() {
        return None;
    }
    let hdrop = HDROP(handle.0);

    // Query count by passing 0xFFFFFFFF.
    let count = unsafe { DragQueryFileW(hdrop, 0xFFFFFFFF, None) };
    if count == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        // First pass: required buffer size.
        let needed = unsafe { DragQueryFileW(hdrop, i, None) };
        if needed == 0 {
            continue;
        }
        let mut buf = vec![0u16; (needed + 1) as usize];
        let written = unsafe { DragQueryFileW(hdrop, i, Some(&mut buf)) };
        if written == 0 {
            continue;
        }
        let path = String::from_utf16_lossy(&buf[..written as usize]);
        out.push(path);
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

// ---------- Image (CF_DIB / CF_DIBV5) ----------

/// Read an image payload as a re-encoded PNG byte stream + SHA-256 hash.
/// Tries CF_DIBV5 first (preferred for HiDPI screenshots) then CF_DIB.
///
/// Returns `(png_bytes, sha256_hex)` on success.
///
/// # Safety
/// Caller MUST hold the clipboard via `OpenClipboard` and pair with
/// `CloseClipboard` after this call returns. The DIB handle is owned by the
/// clipboard subsystem; the bytes we copy out are independent of it.
pub unsafe fn read_image_as_png() -> Option<(Vec<u8>, String)> {
    let format = if has_format(CF_DIBV5) {
        CF_DIBV5
    } else if has_format(CF_DIB) {
        CF_DIB
    } else {
        return None;
    };

    let dib_bytes = unsafe { read_global_bytes(format) }?;
    // CF_DIB / CF_DIBV5 payload is a BITMAPINFOHEADER (or V5HEADER) followed
    // by pixel data. Most consumers (including the `image` crate's BMP
    // decoder) expect a 14-byte BITMAPFILEHEADER prefix. Synthesize one.
    let bmp = wrap_dib_as_bmp(&dib_bytes)?;

    use crate::storage::blob::reencode_to_png;
    reencode_to_png(&bmp).ok()
}

/// Build a `BITMAPFILEHEADER` (14 bytes) prepended to the supplied DIB bytes.
/// Returns `None` if the DIB header is malformed.
fn wrap_dib_as_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.len() < 4 {
        return None;
    }
    // First 4 bytes of any BITMAPINFOHEADER variant = biSize (LE u32).
    let header_size = u32::from_le_bytes([dib[0], dib[1], dib[2], dib[3]]) as usize;
    if header_size > dib.len() {
        return None;
    }

    // Color table: BITMAPV5HEADER spec says 0 entries when biClrUsed=0 and
    // bit depth is >8. We approximate by skipping the color table for now;
    // the `image` crate handles the common cases.
    let pixel_offset = 14 + header_size;
    let total_size = 14 + dib.len();

    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(b"BM"); // 2  signature
    out.extend_from_slice(&(total_size as u32).to_le_bytes()); // 6  bfSize
    out.extend_from_slice(&[0u8; 4]); // 10 reserved
    out.extend_from_slice(&(pixel_offset as u32).to_le_bytes()); // 14 bfOffBits
    out.extend_from_slice(dib);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_format_id_is_nonzero_on_windows() {
        // RegisterClipboardFormatW always returns a session-stable id ≠0.
        let id = html_format_id();
        assert!(id != 0);
    }

    #[test]
    fn wrap_dib_as_bmp_minimum_size() {
        // BITMAPINFOHEADER size is the first u32; supply 40 (BMP_INFO_HEADER).
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&[0u8; 36]); // remaining BITMAPINFOHEADER bytes
        let bmp = wrap_dib_as_bmp(&dib).expect("valid header wraps");
        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(bmp.len(), 14 + 40);
    }
}
