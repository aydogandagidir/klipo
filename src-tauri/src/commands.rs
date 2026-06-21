//! Tauri command handlers (IPC surface).
//!
//! Keep handlers thin: argument validation + delegate to a domain module.
//! The full command list lives in `tauri::generate_handler![...]` in `lib.rs`.
//!
//! Error contract: every command returns `Result<T, String>`. We map the
//! storage layer's typed errors into human-readable strings. We DO NOT
//! include any clipboard content in error strings — that would violate the
//! "no clipboard content in logs" non-negotiable.

use crate::license::manager::{
    self as license_manager, LicenseStatus, ReverifyOutcome, TrialStatus,
};
use crate::perf::StartTime;
use crate::storage::clips::{Clip, ExcludedApp, LabelInfo, ReclassifyReport, ResensitizeReport};
use crate::storage::search::SearchHit;
use crate::storage::Storage;
use tauri::State;

fn map_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Smoke-test command. Returns "pong:<uptime_ms>".
#[tauri::command]
pub async fn ping(start: State<'_, StartTime>) -> Result<String, String> {
    let uptime_ms = start.0.elapsed().as_millis();
    Ok(format!("pong:{}", uptime_ms))
}

/// List recent live (non-deleted) clips, pinned-first.
///
/// `limit` is clamped to a generous ceiling so callers that want to render
/// the user's full visible history (matching `history_limit`) can do so in
/// one round-trip. The ceiling exists only to bound JSON payload size and
/// DOM render cost — the frontend should typically pass `history_limit`
/// (capped at ~1000 for snappy popup-open) here.
#[tauri::command]
pub async fn list_clips(
    storage: State<'_, Storage>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Clip>, String> {
    let limit = limit.unwrap_or(500).clamp(1, 10_000);
    let offset = offset.unwrap_or(0).max(0);
    storage.list_clips(limit, offset).await.map_err(map_err)
}

/// Full-text + recency search. Empty `query` returns `list_clips`-equivalent.
///
/// Same clamp policy as `list_clips` so search results aren't artificially
/// truncated when the user has a large history.
#[tauri::command]
pub async fn search_clips(
    storage: State<'_, Storage>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<SearchHit>, String> {
    let limit = limit.unwrap_or(500).clamp(1, 10_000);
    storage.search_clips(&query, limit).await.map_err(map_err)
}

/// Fetch a single clip by id. NotFound becomes a string error.
#[tauri::command]
pub async fn get_clip(storage: State<'_, Storage>, id: String) -> Result<Clip, String> {
    storage.get_clip(&id).await.map_err(map_err)
}

/// Set or unset the pinned flag.
#[tauri::command]
pub async fn pin_clip(storage: State<'_, Storage>, id: String, pinned: bool) -> Result<(), String> {
    storage.pin_clip(&id, pinned).await.map_err(map_err)
}

/// Soft-delete a clip. Tombstone retention is configurable (default 30d).
#[tauri::command]
pub async fn delete_clip(storage: State<'_, Storage>, id: String) -> Result<(), String> {
    storage.soft_delete(&id).await.map_err(map_err)
}

/// Count of live clips. Cheap; useful for badges and the M2 smoke test.
#[tauri::command]
pub async fn count_live_clips(storage: State<'_, Storage>) -> Result<i64, String> {
    storage.count_live().await.map_err(map_err)
}

/// Friendly identifier (exe name / bundle id) of the app that was foreground
/// when the user invoked the hotkey. Surfaced as a chip in the popup so the
/// user sees where their paste will land.
#[tauri::command]
pub async fn get_last_app_name() -> Result<Option<String>, String> {
    Ok(crate::last_foreground_app_name())
}

/// Hide the popup window. Called by the frontend when the user presses Esc.
/// (Returning focus to the previously-active app is the OS's job once the
/// popup loses focus; we don't manually restore foreground.)
#[tauri::command]
pub async fn hide_popup(window: tauri::Window) -> Result<(), String> {
    window.hide().map_err(map_err)
}

/// Quit Klipo entirely — tear down the watcher, close the popup, exit the
/// process. Reachable from three UI surfaces:
///   1. Tray icon → right-click → "Quit"
///   2. Popup → `Ctrl+Q`
///   3. Settings → About → "Quit Klipo" button
///
/// We use `app.exit(0)` rather than `std::process::exit` so Tauri's
/// graceful-shutdown hooks (RAII drop on `Storage`, plugin cleanup, etc.)
/// run before the process actually terminates.
#[tauri::command]
pub async fn quit_app(app: tauri::AppHandle) -> Result<(), String> {
    tracing::info!(target: "klipo::lifecycle", "user-initiated quit");
    app.exit(0);
    Ok(())
}

/// Resolve a relative `blob_path` (as stored in `clips.blob_path`) to an
/// absolute filesystem path. The frontend hands this to Tauri's
/// `convertFileSrc` so the image / thumbnail renders via the asset protocol.
#[tauri::command]
pub async fn resolve_blob_path(
    storage: State<'_, Storage>,
    relative: String,
) -> Result<String, String> {
    let abs = storage
        .resolve_blob(&relative)
        .ok_or_else(|| "blob_root unavailable (in-memory storage?)".to_string())?;
    Ok(abs.to_string_lossy().to_string())
}

/// Resolve a clip's thumbnail path (192-px WebP). Returns absolute path or
/// an error if missing — frontend should fall back to the full blob in that
/// case (see `useImageThumb` in `ClipCard.tsx`).
#[tauri::command]
pub async fn resolve_thumb_path(
    storage: State<'_, Storage>,
    hash: String,
) -> Result<String, String> {
    let abs = storage
        .resolve_thumb(&hash)
        .ok_or_else(|| "thumb_root unavailable".to_string())?;
    if !abs.exists() {
        return Err(format!(
            "thumbnail not yet generated for {hash} (background task pending)"
        ));
    }
    Ok(abs.to_string_lossy().to_string())
}

/// Return a `data:image/<mime>;base64,...` URL for a clip's thumbnail.
///
/// This bypasses Tauri's asset protocol entirely — useful when path-scope
/// resolution is finicky. Tries the WebP thumbnail first; falls back to
/// the full blob (PNG) so the user sees the image even before the
/// background thumbnail task finishes.
///
/// Returns `Ok(None)` if the clip is not an image kind or has no usable
/// payload on disk. Frontend should fall back to the kind icon in that
/// case.
#[tauri::command]
pub async fn get_thumb_data_url(
    storage: State<'_, Storage>,
    clip_id: String,
) -> Result<Option<String>, String> {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;

    let clip = match storage.get_clip(&clip_id).await {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    if clip.kind != crate::storage::clips::ClipKind::Image {
        return Ok(None);
    }

    // 1. Try the small WebP thumbnail.
    if let Some(thumb_path) = storage.resolve_thumb(&clip.content_hash) {
        if let Ok(bytes) = tokio::fs::read(&thumb_path).await {
            if !bytes.is_empty() {
                let encoded = STANDARD.encode(&bytes);
                return Ok(Some(format!("data:image/webp;base64,{encoded}")));
            }
        }
    }

    // 2. Fallback to the full blob (PNG / JPEG / whatever). Browsers happily
    //    scale `<img>` to a 32-px container, so showing the full-resolution
    //    blob is fine until the thumbnail task catches up.
    if let Some(rel) = clip.blob_path.as_deref() {
        if let Some(abs) = storage.resolve_blob(rel) {
            if let Ok(bytes) = tokio::fs::read(&abs).await {
                if !bytes.is_empty() {
                    let mime = match rel.rsplit('.').next().unwrap_or("png") {
                        "jpg" | "jpeg" => "image/jpeg",
                        "bmp" => "image/bmp",
                        "webp" => "image/webp",
                        _ => "image/png",
                    };
                    let encoded = STANDARD.encode(&bytes);
                    return Ok(Some(format!("data:{mime};base64,{encoded}")));
                }
            }
        }
    }

    Ok(None)
}

/// Paste a clip into the previously-focused app.
///
/// Flow (all clip kinds — text, html, rtf, file, image):
///   1. Resolve the clip from storage.
///   2. Read the previously-foreground HWND captured at hotkey-press time.
///   3. Hide the popup.
///   4. Hand off to the platform paste implementation: restore foreground,
///      wait ~150 ms, write the appropriate clipboard format, synthesize
///      `Ctrl+V`. The platform layer dispatches by `clip.kind`.
#[tauri::command]
pub async fn paste_clip(
    window: tauri::Window,
    storage: State<'_, Storage>,
    id: String,
) -> Result<(), String> {
    tracing::info!(target: "klipo::paste", id = %id, "paste_clip called");

    let clip = storage.get_clip(&id).await.map_err(map_err)?;
    let blob_root = storage.blob_root();

    let prev_hwnd = crate::last_prev_hwnd();
    tracing::info!(
        target: "klipo::paste",
        prev_hwnd = ?prev_hwnd,
        kind = clip.kind.as_str(),
        "captured prev foreground hwnd from hotkey trigger"
    );

    window.hide().map_err(map_err)?;
    tracing::info!(target: "klipo::paste", "popup hidden");

    #[cfg(windows)]
    {
        crate::clipboard::paste::paste_clip(clip, blob_root, prev_hwnd)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (clip, blob_root, prev_hwnd);
        Err("paste not implemented on this platform yet (macOS arrives in v0.2)".to_string())
    }
}

/// Open (or focus) the Settings window. Defined as a separate Tauri window
/// so the popup can stay frameless/transparent while Settings looks like a
/// normal app window with chrome and resize handles.
#[tauri::command]
pub async fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    if let Some(win) = app.get_webview_window("settings") {
        // Window exists from a prior open — show + focus + (re-)center if it
        // wandered off-screen on a multi-monitor setup.
        win.show().map_err(map_err)?;
        win.set_focus().map_err(map_err)?;
        return Ok(());
    }

    // Fallback: window definition lives in tauri.conf.json with `visible:false`,
    // so under normal startup `get_webview_window` will always succeed. If it
    // doesn't (config drift), surface a clear error rather than silently
    // building one with default geometry.
    Err("settings window is not registered in tauri.conf.json".to_string())
}

/// Read a value from the `settings` k/v table. Returns `null` for missing
/// keys (frontend treats that as "use default"). Whitelist of permitted keys
/// is enforced here so a compromised renderer cannot probe arbitrary state.
///
/// `license_*` and `trial_*` keys are accepted by the whitelist (so the
/// backend `license::manager` can read/write them via this same plumbing)
/// but rejected here — the renderer must go through the dedicated license
/// commands so we don't hand the raw key to the WebView when the UI only
/// needs the masked form.
#[tauri::command]
pub async fn get_setting(
    storage: State<'_, Storage>,
    key: String,
) -> Result<Option<String>, String> {
    if !is_known_setting(&key) {
        return Err(format!("unknown setting key: {key}"));
    }
    if is_license_setting(&key) {
        return Err(format!("{key} is backend-only; use the license commands"));
    }
    storage.get_setting(&key).await.map_err(map_err)
}

/// Write a value to the `settings` k/v table. Same whitelist as `get_setting`
/// applies — plus per-key validation in `validate_setting`.
#[tauri::command]
pub async fn set_setting(
    storage: State<'_, Storage>,
    key: String,
    value: String,
) -> Result<(), String> {
    if !is_known_setting(&key) {
        return Err(format!("unknown setting key: {key}"));
    }
    if is_license_setting(&key) {
        return Err(format!("{key} is backend-only; use the license commands"));
    }
    validate_setting(&key, &value)?;
    storage.set_setting(&key, &value).await.map_err(map_err)
}

/// Mark license-related keys that the *renderer* must not read or write
/// directly. The `manager` module is allowed to touch them via the
/// `Storage::get_setting`/`set_setting` API (which doesn't pass through
/// this guard) — the IPC commands `activate_license`, `get_license_status`,
/// etc. mediate everything the UI is allowed to know.
fn is_license_setting(key: &str) -> bool {
    matches!(
        key,
        "license_key"
            | "license_email"
            | "license_product_name"
            | "license_purchase_id"
            | "license_activated_at"
            | "license_last_verified_at"
            | "license_grace_until"
            | "trial_started_at"
            | "license_product_id_override"
    )
}

/// Whitelist of setting keys the renderer may read or write. Anything else
/// (e.g. `schema_version`) is read-only and managed by migrations.
///
/// **License keys note:** `license_key`, `license_email`, etc. are listed
/// here so the backend's `manager.rs` can read/write them through the same
/// `Storage::get_setting` / `set_setting` plumbing. The `SettingKey` union
/// in `src/lib/ipc.ts` does NOT include them — the renderer goes through
/// the dedicated license commands instead, so a malicious renderer can't
/// exfiltrate the raw key via `getSetting("license_key")`.
fn is_known_setting(key: &str) -> bool {
    matches!(
        key,
        "theme"
            | "hotkey"
            | "history_limit"
            | "retention_days_unpinned"
            | "retention_days_sensitive"
            | "retention_days_deleted"
            | "clipboard_poll_interval_ms"
            | "telemetry"
            | "sync"
            | "max_blob_mb"
            | "thumbnail_size_px"
            | "onboarding_done"
            | "autostart"
            | "license_key"
            | "license_email"
            | "license_product_name"
            | "license_purchase_id"
            | "license_activated_at"
            | "license_last_verified_at"
            | "license_grace_until"
            | "trial_started_at"
            | "license_product_id_override"
    )
}

/// Per-key validation — keep it conservative. Migrations seed sane defaults,
/// so we only need to make sure the renderer doesn't write garbage that would
/// break later code paths (e.g. parsing `history_limit` as `i64`).
fn validate_setting(key: &str, value: &str) -> Result<(), String> {
    let parse_int_in_range = |min: i64, max: i64| -> Result<i64, String> {
        let n: i64 = value
            .parse()
            .map_err(|_| format!("{key} must be an integer"))?;
        if !(min..=max).contains(&n) {
            return Err(format!("{key} must be between {min} and {max}"));
        }
        Ok(n)
    };

    match key {
        "theme" if !matches!(value, "light" | "dark" | "system") => {
            Err("theme must be one of: light, dark, system".to_string())
        }
        "telemetry" | "sync" | "onboarding_done" | "autostart"
            if !matches!(value, "on" | "off") =>
        {
            Err(format!("{key} must be 'on' or 'off'"))
        }
        "history_limit" => {
            parse_int_in_range(100, 1_000_000)?;
            Ok(())
        }
        "retention_days_unpinned" | "retention_days_sensitive" | "retention_days_deleted" => {
            parse_int_in_range(0, 3650)?;
            Ok(())
        }
        "clipboard_poll_interval_ms" => {
            parse_int_in_range(50, 10_000)?;
            Ok(())
        }
        "max_blob_mb" => {
            parse_int_in_range(1, 2048)?;
            Ok(())
        }
        "thumbnail_size_px" => {
            parse_int_in_range(32, 512)?;
            Ok(())
        }
        // hotkey: any non-empty chord string. Real validation happens when we
        // try to register it with `tauri-plugin-global-shortcut`; the UI is
        // expected to show the resulting error to the user.
        "hotkey" if value.trim().is_empty() => Err("hotkey must not be empty".to_string()),
        _ => Ok(()),
    }
}

// ---------------- Excluded apps (M6.1) ----------------

/// List every entry in the `excluded_apps` table for the Settings UI.
/// Order: most-recently-added first (matches the storage layer's
/// `ORDER BY added_at DESC`).
#[tauri::command]
pub async fn list_excluded_apps(storage: State<'_, Storage>) -> Result<Vec<ExcludedApp>, String> {
    storage.list_excluded_apps().await.map_err(map_err)
}

/// Add (or upsert) an excluded-app entry. The watcher pipeline will start
/// dropping clipboard captures from a process whose `source_app` matches
/// `bundle_id` on the next event after this returns.
#[tauri::command]
pub async fn add_excluded_app(
    storage: State<'_, Storage>,
    bundle_id: String,
    label: Option<String>,
) -> Result<bool, String> {
    storage
        .add_excluded_app(&bundle_id, label.as_deref())
        .await
        .map_err(map_err)
}

/// Remove an excluded-app entry. The watcher will resume capturing from
/// that process on the next event.
#[tauri::command]
pub async fn remove_excluded_app(
    storage: State<'_, Storage>,
    bundle_id: String,
) -> Result<bool, String> {
    storage
        .remove_excluded_app(&bundle_id)
        .await
        .map_err(map_err)
}

/// Hide the Settings window for `delay_ms` so the user can focus the app
/// they want to add to the excluded list, then snap the foreground app
/// identifier and reopen Settings. Returns the captured identifier (e.g.
/// `MyVault.exe`) or `None` if nothing was foreground / the OS denied the
/// query.
///
/// Bounded between 1s and 10s to keep the user-facing wait predictable —
/// the typical sweet spot is 3 seconds (long enough to alt-tab, short
/// enough that nobody forgets they're mid-flow).
#[tauri::command]
pub async fn capture_foreground_app(
    app: tauri::AppHandle,
    delay_ms: Option<u64>,
) -> Result<Option<String>, String> {
    use tauri::Manager;

    let delay = delay_ms.unwrap_or(3_000).clamp(1_000, 10_000);

    let settings_win = app.get_webview_window("settings");
    if let Some(w) = &settings_win {
        w.hide().map_err(map_err)?;
    } else {
        return Err("settings window not registered".to_string());
    }

    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;

    let captured = crate::clipboard::source_app::current().map(|s| s.identifier);

    if let Some(w) = &settings_win {
        let _ = w.show();
        let _ = w.set_focus();
    }

    tracing::info!(
        target: "klipo::settings",
        delay_ms = delay,
        captured = captured.as_deref().unwrap_or("(none)"),
        "captured foreground app for excluded-apps add flow"
    );

    Ok(captured)
}

// ---------------- Privacy (M6.2) ----------------

/// Return the absolute path of Klipo's app-data directory, the parent of the
/// SQLite DB and `blobs/` / `thumbs/` folders. The Settings UI shows this
/// in a tooltip and uses `open_data_folder` to spawn the OS file manager.
#[tauri::command]
pub async fn app_data_dir_path(app: tauri::AppHandle) -> Result<String, String> {
    use tauri::Manager;
    let dir = app.path().app_data_dir().map_err(map_err)?;
    Ok(dir.to_string_lossy().to_string())
}

/// Open the app-data directory in the OS file manager (Explorer on Windows,
/// Finder on macOS). Useful for advanced users who want to inspect or
/// back up the SQLite DB / blob store directly.
#[tauri::command]
pub async fn open_data_folder(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    let dir = app.path().app_data_dir().map_err(map_err)?;

    // Ensure the directory exists — first-launch users may invoke this
    // before any clip has been written.
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return Err(format!("failed to create app data dir: {e}"));
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        Command::new("explorer")
            .arg(dir.as_os_str())
            .spawn()
            .map_err(|e| format!("failed to launch explorer: {e}"))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("open")
            .arg(dir.as_os_str())
            .spawn()
            .map_err(|e| format!("failed to launch open: {e}"))?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = dir;
        Err("open_data_folder is not implemented on this platform yet".to_string())
    }
}

// ---------------- Autostart (M6.4) ----------------

/// Whether Klipo is configured to start automatically on user login.
#[tauri::command]
pub async fn get_autostart() -> Result<bool, String> {
    crate::autostart::is_enabled()
}

/// Enable or disable autostart. On Windows this writes / deletes a value
/// under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. Returns the
/// new state on success — Settings UI should re-read via `get_autostart`
/// to be safe (most callers do).
#[tauri::command]
pub async fn set_autostart(enabled: bool) -> Result<bool, String> {
    crate::autostart::set_enabled(enabled)?;
    // Persist a hint in settings so other surfaces (e.g. M7 onboarding) can
    // ask "is autostart on" without paying the registry round-trip.
    Ok(enabled)
}

// ---------------- Hotkey rebind (M6.3) ----------------

/// Register a new global hotkey, replacing the currently-registered one.
///
/// Flow:
///   1. Parse the chord string (`Ctrl+Alt+V`).
///   2. Unregister the current hotkey (if any).
///   3. Try to register the new one.
///   4. On failure, attempt to re-register the old hotkey so the user is
///      not stranded with no way to summon Klipo.
///   5. On success, persist the chord under the `hotkey` setting.
///
/// Returns the chord string the caller passed in, so the Settings UI can
/// confirm-and-display the canonical form.
#[tauri::command]
pub async fn register_hotkey(
    app: tauri::AppHandle,
    storage: State<'_, Storage>,
    chord: String,
) -> Result<String, String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let new_shortcut = crate::parse_chord(&chord)?;
    let previous = crate::current_hotkey();

    // Unregister the current hotkey so the new one has a clear slot.
    if let Some(prev) = previous {
        if let Err(e) = app.global_shortcut().unregister(prev) {
            tracing::warn!(
                target: "klipo::hotkey",
                error = %e,
                "failed to unregister previous hotkey; new registration will likely fail"
            );
        }
    }

    // Attempt the new registration. On failure, restore the previous one so
    // the user still has a working hotkey — and propagate the error to the
    // UI so they can try a different chord.
    match app
        .global_shortcut()
        .on_shortcut(new_shortcut, crate::handle_hotkey)
    {
        Ok(()) => {
            crate::set_current_hotkey(Some(new_shortcut));
        }
        Err(register_err) => {
            // Best-effort restore.
            if let Some(prev) = previous {
                if let Err(e) = app
                    .global_shortcut()
                    .on_shortcut(prev, crate::handle_hotkey)
                {
                    tracing::error!(
                        target: "klipo::hotkey",
                        error = %e,
                        "could not restore previous hotkey after failed rebind; \
                         user must restart Klipo or pick another chord"
                    );
                    crate::set_current_hotkey(None);
                }
            }
            return Err(format!(
                "could not register '{chord}': {register_err}. \
                 Try a different chord (the previous hotkey was kept where possible)."
            ));
        }
    }

    // Persist on success only — we don't want to remember a chord that
    // failed at registration time.
    storage
        .set_setting("hotkey", &chord)
        .await
        .map_err(map_err)?;
    Ok(chord)
}

/// Re-run the current sensitive-content regex set against every live,
/// text-bearing clip and update each row's `sensitive` flag in place.
///
/// **Data-preserving:** only the `sensitive` column (+ `sync_version`)
/// changes. No INSERT, no DELETE, no text rewrites. Triggered from
/// Settings → Privacy → "Re-scan history".
///
/// Returns a `ResensitizeReport` with scanned / flagged / unflagged /
/// unchanged counters so the UI can render a toast like
/// "Scanned 247 clips: 5 newly flagged".
#[tauri::command]
pub async fn resensitize_history(storage: State<'_, Storage>) -> Result<ResensitizeReport, String> {
    let report = storage
        .resensitize_all(|text| crate::clipboard::sensitive::scan(text).is_sensitive())
        .await
        .map_err(map_err)?;
    tracing::info!(
        target: "klipo::resensitize",
        scanned = report.scanned,
        flagged = report.flagged,
        unflagged = report.unflagged,
        unchanged = report.unchanged,
        "user-initiated resensitize finished"
    );
    Ok(report)
}

// ---------------- Organize: title, labels (M9) ----------------

/// Set or clear a clip's user title. Pass `null`/empty to clear it. The title
/// is folded into the FTS index, so it becomes searchable immediately.
#[tauri::command]
pub async fn set_clip_title(
    storage: State<'_, Storage>,
    id: String,
    title: Option<String>,
) -> Result<(), String> {
    storage
        .set_clip_title(&id, title.as_deref())
        .await
        .map_err(map_err)
}

/// Add a label (by name) to a clip, creating it if new. Custom names are
/// allowed; if the name matches a known auto label its color is inherited.
/// Returns the trimmed name stored.
#[tauri::command]
pub async fn add_clip_label(
    storage: State<'_, Storage>,
    id: String,
    name: String,
) -> Result<String, String> {
    storage.add_label(&id, &name).await.map_err(map_err)
}

/// Remove a label (by name) from a clip. No-op if absent.
#[tauri::command]
pub async fn remove_clip_label(
    storage: State<'_, Storage>,
    id: String,
    name: String,
) -> Result<(), String> {
    storage.remove_label(&id, &name).await.map_err(map_err)
}

/// Rename a label everywhere it occurs (global). Used by the editor's
/// click-to-edit on a label chip.
#[tauri::command]
pub async fn rename_label(
    storage: State<'_, Storage>,
    old: String,
    new: String,
) -> Result<(), String> {
    storage.rename_label(&old, &new).await.map_err(map_err)
}

/// List the label vocabulary (names in use on live clips) with usage counts
/// and a representative auto-key (for chip color). Powers the popup filter
/// chips + the add-label autocomplete.
#[tauri::command]
pub async fn list_all_labels(storage: State<'_, Storage>) -> Result<Vec<LabelInfo>, String> {
    storage.list_labels().await.map_err(map_err)
}

/// Re-run the content classifier across all live text clips and re-apply each
/// clip's auto label (user-created labels are preserved). Sibling of
/// `resensitize_history`. Triggered from Settings → Privacy.
#[tauri::command]
pub async fn reclassify_history(storage: State<'_, Storage>) -> Result<ReclassifyReport, String> {
    let report = storage
        .reclassify_all(|text| crate::clipboard::classify::classify(text).map(str::to_string))
        .await
        .map_err(map_err)?;
    tracing::info!(
        target: "klipo::reclassify",
        scanned = report.scanned,
        changed = report.changed,
        unchanged = report.unchanged,
        "user-initiated reclassify finished"
    );
    Ok(report)
}

/// Hard-delete every clip and remove the on-disk blob + thumbnail trees.
/// Settings + excluded-apps stay intact so the user doesn't lose their
/// hotkey / theme preferences.
///
/// The caller (Settings UI) MUST gate this behind an explicit AlertDialog
/// — there's no undo. Returns the number of clips wiped.
#[tauri::command]
pub async fn wipe_all_data(
    app: tauri::AppHandle,
    storage: State<'_, Storage>,
) -> Result<u64, String> {
    use tauri::Manager;

    // 1. Wipe DB rows (FTS triggers cascade).
    let wiped = storage.wipe_all_clips().await.map_err(map_err)?;

    // 2. Wipe blob + thumb directories. Best-effort: missing dirs are fine,
    //    permission errors surface as a string error so the UI can show a
    //    "DB cleared but disk wipe failed — see logs" banner.
    let app_data_dir = app.path().app_data_dir().map_err(map_err)?;
    for sub in ["blobs", "thumbs"] {
        let dir = app_data_dir.join(sub);
        if dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                tracing::warn!(
                    target: "klipo::wipe",
                    error = %e,
                    dir = %dir.display(),
                    "failed to remove subtree; DB rows already cleared"
                );
                return Err(format!(
                    "DB cleared but failed to remove {}: {e}",
                    dir.display()
                ));
            }
        }
    }

    tracing::info!(
        target: "klipo::wipe",
        rows = wiped,
        "user wiped all clips + blobs"
    );
    Ok(wiped)
}

// ---------------- License + trial (M8) ----------------
//
// All five commands round-trip through `crate::license::manager`. The
// manager owns the storage shape + the Gumroad client; this layer is
// pure error mapping + logging.

/// Activate a Gumroad license key. Increments the per-key uses counter
/// (each device = +1) so Gumroad enforces the 3-device limit on the
/// server side. Returns the resulting `LicenseStatus` so the UI can flip
/// straight from "Activate" to "Pro — Activated" without a follow-up
/// `get_license_status` call.
#[tauri::command]
pub async fn activate_license(
    storage: State<'_, Storage>,
    key: String,
    email: Option<String>,
) -> Result<LicenseStatus, String> {
    let status = license_manager::activate(&storage, &key, email.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!(
        target: "klipo::license",
        tier = ?status.tier,
        reason = %status.reason,
        "license activated"
    );
    Ok(status)
}

/// Wipe every `license_*` key from settings. Falls back to trial-or-free
/// posture immediately. Idempotent — calling on a free user is a no-op.
#[tauri::command]
pub async fn deactivate_license(storage: State<'_, Storage>) -> Result<(), String> {
    license_manager::deactivate(&storage)
        .await
        .map_err(|e| e.to_string())?;
    tracing::info!(target: "klipo::license", "license deactivated by user");
    Ok(())
}

/// Current status snapshot. Cheap (a handful of SELECTs) — UI may call
/// this on every render that wants the latest state without going through
/// a network round-trip.
#[tauri::command]
pub async fn get_license_status(storage: State<'_, Storage>) -> Result<LicenseStatus, String> {
    Ok(license_manager::get_status(&storage).await)
}

/// Manual "Re-check now" button. Hits Gumroad without incrementing the
/// uses counter (so power users can re-check freely without burning their
/// 3-device allowance). On `Invalid` / `Refunded` the license is cleared
/// — the renderer will see `tier: free, reason: trial-active|trial-expired`
/// on the next status fetch.
#[tauri::command]
pub async fn reverify_license(storage: State<'_, Storage>) -> Result<ReverifyOutcome, String> {
    Ok(license_manager::reverify(&storage).await)
}

/// Trial countdown for the popup footer + Settings → License banner.
/// Initializes `trial_started_at` if absent, so this can safely be the
/// very first thing the renderer asks the backend.
#[tauri::command]
pub async fn get_trial_status(storage: State<'_, Storage>) -> Result<TrialStatus, String> {
    Ok(license_manager::get_trial_status(&storage).await)
}
