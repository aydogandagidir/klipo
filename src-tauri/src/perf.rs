//! Performance instrumentation primitives shared across modules.
//!
//! Targets logged through this module:
//!   - `klipo::perf::popup_visible_ms` — process start → first popup paint.
//!   - `klipo::perf::hotkey_to_focus_ms` — hotkey press → popup re-focused.
//!   - `klipo::perf::search_ms` (frontend) — query keystroke → first hit.
//!
//! The `KLIPO_LOG=info,klipo::perf=debug` env var enables these emissions.
//! `docs/perf-runbook.md` describes how to harvest them into the per-month
//! results sheet (`bench/results-<yyyy-mm>.md`).

use std::sync::OnceLock;
use std::time::Instant;

/// Process start timestamp captured during `tauri::Builder::setup`.
/// Stored in Tauri's managed state so commands can read it without a lock.
/// Direct callers (`lib.rs::run` for the popup focus event) read the
/// inner `Instant` and call `.elapsed()` themselves.
pub struct StartTime(pub Instant);

/// Latch that flips to `true` after the popup has fired its first
/// `Focused(true)` event. Subsequent focus events are not interesting for
/// `popup_visible_ms` — we only want the cold-start sample.
pub(crate) static POPUP_FIRST_VISIBLE_LOGGED: OnceLock<()> = OnceLock::new();

/// Mark the popup as having become visible for the first time. Safe to call
/// from any thread; subsequent calls are no-ops thanks to `OnceLock`.
pub(crate) fn mark_popup_first_visible() -> bool {
    POPUP_FIRST_VISIBLE_LOGGED.set(()).is_ok()
}

/// Hotkey-press timestamp, captured in the `handle_hotkey` callback before
/// the popup is shown. The popup's first `Focused(true)` after this stamp
/// reads it back to log `hotkey_to_focus_ms`. `None` outside hotkey-driven
/// shows (e.g. tray icon click) — those don't get the metric.
pub(crate) static HOTKEY_PRESS_INSTANT: OnceLock<std::sync::Mutex<Option<Instant>>> =
    OnceLock::new();

pub(crate) fn record_hotkey_press() {
    let lock = HOTKEY_PRESS_INSTANT.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(mut g) = lock.lock() {
        *g = Some(Instant::now());
    }
}

pub(crate) fn take_hotkey_press() -> Option<Instant> {
    HOTKEY_PRESS_INSTANT
        .get()
        .and_then(|m| m.lock().ok().and_then(|mut g| g.take()))
}
