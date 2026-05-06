//! Klipo desktop app — library entry point.

mod autostart;
pub mod clipboard;
mod commands;
mod perf;
pub mod storage;

use std::sync::{Mutex, OnceLock};

use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::EnvFilter;

/// Custom log timestamp formatter — emits the user's *local* wall-clock
/// time with a UTC offset suffix, so log lines match what the user sees
/// on the system tray clock instead of UTC.
struct LocalTimer;

impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        // %.3f = milliseconds; %z = numeric offset (e.g. "+0300").
        write!(
            w,
            "{}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f %z")
        )
    }
}

const POPUP_LABEL: &str = "main";

/// Raw handle of the foreground window captured at the moment the user
/// pressed the global hotkey to summon the popup. Used by the paste path
/// to explicitly restore focus to that window before SendInput, so the
/// pasted text always lands in the right place.
pub(crate) static PREV_FOREGROUND_HWND: OnceLock<Mutex<Option<isize>>> = OnceLock::new();

/// Identifier of the app that was foreground when the user invoked the
/// hotkey (exe name on Windows, bundle id on macOS). Surfaced to the
/// popup as a chip so users know where their paste will land.
pub(crate) static PREV_FOREGROUND_APP: OnceLock<Mutex<Option<String>>> = OnceLock::new();

/// The currently-registered global hotkey. Tracked so the Settings UI's
/// "Rebind" path can unregister it before swapping in a new one. `None`
/// means no hotkey is registered (e.g. both the saved chord and the
/// defaults failed at startup; tray icon is the only way to summon).
pub(crate) static CURRENT_HOTKEY: OnceLock<Mutex<Option<Shortcut>>> = OnceLock::new();

pub(crate) fn current_hotkey() -> Option<Shortcut> {
    CURRENT_HOTKEY
        .get()
        .and_then(|m| m.lock().ok().and_then(|g| *g))
}

pub(crate) fn set_current_hotkey(s: Option<Shortcut>) {
    let lock = CURRENT_HOTKEY.get_or_init(|| Mutex::new(None));
    if let Ok(mut g) = lock.lock() {
        *g = s;
    }
}

fn last_foreground_hwnd() -> Option<isize> {
    PREV_FOREGROUND_HWND
        .get()
        .and_then(|m| m.lock().ok().and_then(|g| *g))
}

pub(crate) fn last_foreground_app_name() -> Option<String> {
    PREV_FOREGROUND_APP
        .get()
        .and_then(|m| m.lock().ok().and_then(|g| g.clone()))
}

#[cfg(windows)]
fn capture_foreground_hwnd() {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    // SAFETY: GetForegroundWindow is a stateless Win32 query; safe to call
    // from any thread, returns NULL if no foreground window.
    let hwnd = unsafe { GetForegroundWindow() };
    let raw = hwnd.0 as isize;
    let lock = PREV_FOREGROUND_HWND.get_or_init(|| Mutex::new(None));
    if let Ok(mut g) = lock.lock() {
        *g = Some(raw);
    }

    // Also capture the friendly app identifier — purely for the UI chip.
    // We use the same Win32 path the watcher uses for `source_app`, so the
    // string format is consistent with what's stored on captured clips.
    let app_name = clipboard::source_app::current().map(|s| s.identifier);
    let lock = PREV_FOREGROUND_APP.get_or_init(|| Mutex::new(None));
    if let Ok(mut g) = lock.lock() {
        *g = app_name.clone();
    }

    tracing::info!(
        target: "klipo::hotkey",
        hwnd = raw,
        app = app_name.as_deref().unwrap_or("(unknown)"),
        "captured foreground hwnd"
    );
}

#[cfg(not(windows))]
fn capture_foreground_hwnd() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();
    let started_at = std::time::Instant::now();

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // M5.x.1: native drag-and-drop from popup so file/image clips can be
        // dropped into browser-based apps that reject paste-of-files.
        .plugin(tauri_plugin_drag::init())
        // M7: auto-update. Wired with a placeholder pubkey by default; once
        // a real signing keypair is dropped in via tauri.conf.json the
        // frontend's "Check for updates" button starts working live.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            app.manage(perf::StartTime(started_at));

            // ---- Storage init ----
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("OS did not expose an app data dir; cannot continue");
            let db_path = app_data_dir.join("klipo.db");

            tracing::info!(
                target: "klipo::startup",
                db = %db_path.display(),
                "opening storage"
            );

            // Migration / open failures are unrecoverable for the runtime,
            // but we want the user (or the report we get from them) to see
            // *what* failed and *what to do*, not just `expect()` panic
            // text. Most realistic causes are listed in the message.
            let storage = match tauri::async_runtime::block_on(storage::Storage::open(&db_path)) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!(
                        target: "klipo::startup",
                        error = %e,
                        db = %db_path.display(),
                        "failed to open Klipo storage"
                    );
                    let hint = if format!("{e:?}").contains("VersionMismatch") {
                        "the on-disk database was created by a different build of Klipo \
                         (migration checksum mismatch). Delete the DB to start fresh: \
                         the file lives at the path above; close Klipo first, remove \
                         `klipo.db`, `klipo.db-shm`, `klipo.db-wal`, then relaunch."
                    } else {
                        "the on-disk database could not be opened or migrated. \
                         Check filesystem permissions and free disk space, or delete \
                         the file at the path above to start fresh."
                    };
                    panic!(
                        "Klipo storage failed to open at {}: {e}\n\nHint: {hint}",
                        db_path.display()
                    );
                }
            };
            app.manage(storage.clone());

            // ---- Clipboard watcher + pipeline ----
            if let Err(e) = clipboard::start(storage, app.handle().clone()) {
                tracing::error!(
                    target: "klipo::startup",
                    error = %e,
                    "clipboard watcher failed to start"
                );
            }

            // ---- Popup window styling (Mica / Acrylic) ----
            if let Some(popup) = app.get_webview_window(POPUP_LABEL) {
                apply_window_blur(&popup);

                let popup_for_event = popup.clone();
                let start_time_for_event = started_at;
                popup.on_window_event(move |event| match event {
                    WindowEvent::Focused(false) => {
                        let _ = popup_for_event.hide();
                    }
                    WindowEvent::Focused(true) => {
                        // First-ever focus → log cold-start latency. After
                        // that, log hotkey-to-focus latency on every
                        // hotkey-triggered show. Read `docs/perf-runbook.md`
                        // §1 + §2 to see how these get harvested.
                        if perf::mark_popup_first_visible() {
                            let elapsed_ms = start_time_for_event.elapsed().as_millis();
                            tracing::info!(
                                target: "klipo::perf",
                                popup_visible_ms = elapsed_ms as u64,
                                "popup_first_visible"
                            );
                        }
                        if let Some(press) = perf::take_hotkey_press() {
                            let elapsed_ms = press.elapsed().as_millis();
                            tracing::info!(
                                target: "klipo::perf",
                                hotkey_to_focus_ms = elapsed_ms as u64,
                                "hotkey_to_focus"
                            );
                        }
                    }
                    _ => {}
                });
            } else {
                tracing::error!(
                    target: "klipo::startup",
                    label = POPUP_LABEL,
                    "popup window not found in tauri.conf.json"
                );
            }

            // ---- Settings window: hide-instead-of-destroy on close ----
            // Otherwise the window would be torn down the first time the user
            // clicks X, and the next "Settings…" tray click would silently
            // fail because `get_webview_window` returns None for destroyed
            // windows. Persisting it costs ~5 MB of WebView idle RAM but keeps
            // re-open snappy.
            if let Some(settings_win) = app.get_webview_window("settings") {
                let settings_for_close = settings_win.clone();
                settings_win.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = settings_for_close.hide();
                    }
                });
            }

            // ---- Global hotkey ----
            // Order of preference at startup:
            //   1. The saved chord (if the user has rebound it via Settings).
            //   2. The default `Ctrl+Alt+V`.
            //   3. The Turkish-Q-friendly fallback `Ctrl+Alt+Shift+V`.
            // First one that registers successfully wins. The handler closure
            // is factored into a free function so the `register_hotkey` IPC
            // command can re-register against it after the user rebinds.
            let saved_chord: Option<String> = tauri::async_runtime::block_on(
                app.state::<storage::Storage>().get_setting("hotkey"),
            )
            .ok()
            .flatten();

            let default_primary =
                Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyV);
            let default_fallback = Shortcut::new(
                Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT),
                Code::KeyV,
            );

            let mut candidates: Vec<(Shortcut, String)> = Vec::with_capacity(3);
            if let Some(chord) = saved_chord.as_deref() {
                if let Ok(parsed) = parse_chord(chord) {
                    candidates.push((parsed, chord.to_string()));
                } else {
                    tracing::warn!(
                        target: "klipo::startup",
                        chord = chord,
                        "saved hotkey chord did not parse; falling back to defaults"
                    );
                }
            }
            // Always include the defaults — if the saved chord registered, we
            // never reach these, but if it failed (collision with another app)
            // they keep Klipo summonable.
            if !candidates.iter().any(|(s, _)| *s == default_primary) {
                candidates.push((default_primary, "Ctrl+Alt+V".to_string()));
            }
            if !candidates.iter().any(|(s, _)| *s == default_fallback) {
                candidates.push((default_fallback, "Ctrl+Alt+Shift+V".to_string()));
            }

            let mut chosen_hotkey: String = "(none — use tray icon)".to_string();
            for (shortcut, label) in candidates {
                match app.global_shortcut().on_shortcut(shortcut, handle_hotkey) {
                    Ok(()) => {
                        set_current_hotkey(Some(shortcut));
                        chosen_hotkey = label;
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(
                            target: "klipo::startup",
                            chord = label,
                            error = %e,
                            "hotkey registration failed; trying next candidate"
                        );
                    }
                }
            }
            let chosen_hotkey: &str = &chosen_hotkey;

            // Türkçe-Q + Türkçe-F layouts use AltGr (= Ctrl+Alt) for some
            // characters; if the user is on such a layout we surface a hint
            // in the log so they know about the fallback chord.
            #[cfg(windows)]
            warn_if_altgr_conflict_layout();

            // ---- Tray icon ----
            if let Err(e) = setup_tray(app.handle()) {
                tracing::warn!(
                    target: "klipo::startup",
                    error = %e,
                    "tray icon setup failed (non-fatal; hotkey still works)"
                );
            }

            tracing::info!(
                target: "klipo::startup",
                hotkey = chosen_hotkey,
                "tauri runtime ready"
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::list_clips,
            commands::search_clips,
            commands::get_clip,
            commands::pin_clip,
            commands::delete_clip,
            commands::count_live_clips,
            commands::hide_popup,
            commands::quit_app,
            commands::paste_clip,
            commands::resolve_blob_path,
            commands::resolve_thumb_path,
            commands::get_thumb_data_url,
            commands::get_last_app_name,
            commands::open_settings,
            commands::get_setting,
            commands::set_setting,
            commands::list_excluded_apps,
            commands::add_excluded_app,
            commands::remove_excluded_app,
            commands::capture_foreground_app,
            commands::app_data_dir_path,
            commands::open_data_folder,
            commands::wipe_all_data,
            commands::register_hotkey,
            commands::get_autostart,
            commands::set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running klipo desktop app");
}

/// Public so commands can read it without exposing the static directly.
pub(crate) fn last_prev_hwnd() -> Option<isize> {
    last_foreground_hwnd()
}

fn apply_window_blur(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        use window_vibrancy::{apply_acrylic, apply_mica};
        if apply_mica(window, Some(true)).is_err() {
            let _ = apply_acrylic(window, Some((18, 18, 18, 220)));
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
    }
}

/// Surface a one-time log warning if the user is on a Turkish (or other
/// AltGr-heavy) layout where `Ctrl+Alt` doubles as `AltGr`. We don't
/// auto-switch the hotkey — `Ctrl+Alt+V` still works because `V` doesn't
/// produce a character with `AltGr` on Turkish-Q / Turkish-F. But if the
/// user remaps to a key that DOES produce `AltGr+key`, they'll know why
/// the chord misfires, and the log gives a paper trail before M6
/// (Settings UI) ships rebind support.
#[cfg(windows)]
fn warn_if_altgr_conflict_layout() {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
    // SAFETY: GetKeyboardLayout(0) reads the active layout for the current
    // thread; it's a stateless query.
    let hkl = unsafe { GetKeyboardLayout(0) };
    let raw = hkl.0 as usize;
    // The lower 16 bits are the language identifier (LCID).
    let lcid = (raw & 0xFFFF) as u16;

    // Turkish: 0x041F (Q layout), 0xF01F (F layout sub-LCID).
    // German, Polish, Czech, Hungarian etc. also use AltGr extensively.
    let altgr_sensitive = matches!(
        lcid,
        0x041F  // Turkish
            | 0x0407  // German
            | 0x0415  // Polish
            | 0x0405  // Czech
            | 0x040E  // Hungarian
            | 0x040A  // Spanish
            | 0x0816  // Portuguese (PT)
            | 0x0C0C // French (Canadian)
    );

    if altgr_sensitive {
        tracing::warn!(
            target: "klipo::startup",
            lcid = format!("0x{:04X}", lcid),
            "AltGr-sensitive keyboard layout detected; \
             Ctrl+Alt+V works on Turkish/German/etc. because V has no AltGr binding, \
             but if you change the hotkey in M6 settings to a letter that produces \
             a character with AltGr (e.g. Polish AltGr+A, German AltGr+E), \
             the chord will misfire."
        );
    }
}

fn setup_tray(app_handle: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let show = MenuItem::with_id(app_handle, "show", "Show Klipo", true, Some("Ctrl+Alt+V"))?;
    let settings = MenuItem::with_id(app_handle, "settings", "Settings…", true, None::<&str>)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app_handle)?;
    let quit = MenuItem::with_id(app_handle, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app_handle, &[&show, &settings, &separator, &quit])?;

    let _ = TrayIconBuilder::with_id("klipo-tray")
        .icon(
            app_handle
                .default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::FailedToReceiveMessage)?,
        )
        .tooltip("Klipo — Ctrl+Alt+V to open")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window(POPUP_LABEL) {
                    capture_foreground_hwnd();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "settings" => {
                if let Some(window) = app.get_webview_window("settings") {
                    let _ = window.show();
                    let _ = window.set_focus();
                } else {
                    tracing::error!(
                        target: "klipo::tray",
                        "settings window missing from tauri.conf.json"
                    );
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Left click → toggle popup. Right click → menu (default).
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window(POPUP_LABEL) {
                    let visible = window.is_visible().unwrap_or(false);
                    if visible {
                        let _ = window.hide();
                    } else {
                        capture_foreground_hwnd();
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app_handle)?;
    Ok(())
}

/// Free function so the same handler can be registered at startup AND from
/// the `register_hotkey` IPC command after a rebind. Accepts `Fn(...)` per
/// `tauri-plugin-global-shortcut`'s signature; functions auto-impl `Fn`.
fn handle_hotkey(
    app_handle: &tauri::AppHandle,
    _shortcut: &Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    if let Some(window) = app_handle.get_webview_window(POPUP_LABEL) {
        let visible = window.is_visible().unwrap_or(false);
        if visible {
            let _ = window.hide();
        } else {
            // Stamp the press time BEFORE we capture foreground / show the
            // window so the `Focused(true)` handler sees a tight elapsed
            // value covering only Klipo's show path. The matching read
            // happens in the popup's window-event listener below.
            perf::record_hotkey_press();
            capture_foreground_hwnd();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

/// Parse a human-readable chord like `Ctrl+Alt+V` or `CmdOrCtrl+Shift+P`
/// into a `Shortcut`.
///
/// Accepts:
///   - Modifier names (case-insensitive): `Ctrl` / `Control`, `Alt` /
///     `Option`, `Shift`, `Meta` / `Cmd` / `Super` / `Win` /
///     `CmdOrCtrl` / `CommandOrControl`.
///   - Main key: a single ASCII letter (A–Z) or digit (0–9), or a function
///     key `F1`–`F24`.
///   - Order: any. The chord must include at least one modifier and exactly
///     one main key.
///
/// We intentionally don't accept punctuation / arrow keys / etc. for v0.1
/// — the Settings UI's chord-capture input restricts the user to the same
/// alphabet, so anything that arrives here outside the allowlist is
/// either a manual SQL edit or a sign of UI / parser drift.
pub(crate) fn parse_chord(s: &str) -> Result<Shortcut, String> {
    let parts: Vec<&str> = s
        .split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 2 {
        return Err("chord must contain at least one modifier and one key".to_string());
    }

    let mut modifiers = Modifiers::empty();
    let mut key_code: Option<Code> = None;

    for part in &parts {
        let upper = part.to_ascii_uppercase();
        match upper.as_str() {
            "CTRL" | "CONTROL" | "CMDORCTRL" | "COMMANDORCONTROL" => {
                modifiers |= Modifiers::CONTROL;
            }
            "ALT" | "OPTION" => modifiers |= Modifiers::ALT,
            "SHIFT" => modifiers |= Modifiers::SHIFT,
            "META" | "WIN" | "WINDOWS" | "SUPER" | "CMD" | "COMMAND" => {
                modifiers |= Modifiers::SUPER;
            }
            other => {
                if key_code.is_some() {
                    return Err(format!(
                        "chord '{s}' has more than one main key; got '{part}' after a previous key"
                    ));
                }
                key_code = Some(parse_key_code(other).ok_or_else(|| {
                    format!("unsupported key '{part}' in chord '{s}' (only A-Z, 0-9, F1-F24)")
                })?);
            }
        }
    }

    if modifiers.is_empty() {
        return Err(format!(
            "chord '{s}' must include at least one modifier (Ctrl, Alt, Shift, Meta)"
        ));
    }
    let code = key_code.ok_or_else(|| format!("chord '{s}' is missing a main key"))?;
    Ok(Shortcut::new(Some(modifiers), code))
}

fn parse_key_code(upper: &str) -> Option<Code> {
    if upper.len() == 1 {
        let c = upper.chars().next()?;
        return match c {
            'A' => Some(Code::KeyA),
            'B' => Some(Code::KeyB),
            'C' => Some(Code::KeyC),
            'D' => Some(Code::KeyD),
            'E' => Some(Code::KeyE),
            'F' => Some(Code::KeyF),
            'G' => Some(Code::KeyG),
            'H' => Some(Code::KeyH),
            'I' => Some(Code::KeyI),
            'J' => Some(Code::KeyJ),
            'K' => Some(Code::KeyK),
            'L' => Some(Code::KeyL),
            'M' => Some(Code::KeyM),
            'N' => Some(Code::KeyN),
            'O' => Some(Code::KeyO),
            'P' => Some(Code::KeyP),
            'Q' => Some(Code::KeyQ),
            'R' => Some(Code::KeyR),
            'S' => Some(Code::KeyS),
            'T' => Some(Code::KeyT),
            'U' => Some(Code::KeyU),
            'V' => Some(Code::KeyV),
            'W' => Some(Code::KeyW),
            'X' => Some(Code::KeyX),
            'Y' => Some(Code::KeyY),
            'Z' => Some(Code::KeyZ),
            '0' => Some(Code::Digit0),
            '1' => Some(Code::Digit1),
            '2' => Some(Code::Digit2),
            '3' => Some(Code::Digit3),
            '4' => Some(Code::Digit4),
            '5' => Some(Code::Digit5),
            '6' => Some(Code::Digit6),
            '7' => Some(Code::Digit7),
            '8' => Some(Code::Digit8),
            '9' => Some(Code::Digit9),
            _ => None,
        };
    }

    if let Some(rest) = upper.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u8>() {
            return match n {
                1 => Some(Code::F1),
                2 => Some(Code::F2),
                3 => Some(Code::F3),
                4 => Some(Code::F4),
                5 => Some(Code::F5),
                6 => Some(Code::F6),
                7 => Some(Code::F7),
                8 => Some(Code::F8),
                9 => Some(Code::F9),
                10 => Some(Code::F10),
                11 => Some(Code::F11),
                12 => Some(Code::F12),
                13 => Some(Code::F13),
                14 => Some(Code::F14),
                15 => Some(Code::F15),
                16 => Some(Code::F16),
                17 => Some(Code::F17),
                18 => Some(Code::F18),
                19 => Some(Code::F19),
                20 => Some(Code::F20),
                21 => Some(Code::F21),
                22 => Some(Code::F22),
                23 => Some(Code::F23),
                24 => Some(Code::F24),
                _ => None,
            };
        }
    }

    None
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_env("KLIPO_LOG").unwrap_or_else(|_| EnvFilter::new("info,klipo=debug"));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_timer(LocalTimer)
        .compact()
        .try_init();
}

#[cfg(test)]
mod chord_tests {
    use super::*;

    #[test]
    fn parses_basic_chord() {
        let s = parse_chord("Ctrl+Alt+V").unwrap();
        assert_eq!(
            s,
            Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyV)
        );
    }

    #[test]
    fn parses_case_insensitive_and_aliases() {
        let s1 = parse_chord("ctrl+alt+v").unwrap();
        let s2 = parse_chord("CMDORCTRL+ALT+V").unwrap();
        let s3 = parse_chord("Control+Option+v").unwrap();
        assert_eq!(s1, s2);
        assert_eq!(s2, s3);
    }

    #[test]
    fn parses_function_keys() {
        let s = parse_chord("Ctrl+F12").unwrap();
        assert_eq!(s, Shortcut::new(Some(Modifiers::CONTROL), Code::F12));
    }

    #[test]
    fn parses_digits() {
        let s = parse_chord("Alt+Shift+5").unwrap();
        assert_eq!(
            s,
            Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::Digit5)
        );
    }

    #[test]
    fn rejects_no_modifier() {
        assert!(parse_chord("V").is_err());
        assert!(parse_chord("F12").is_err());
    }

    #[test]
    fn rejects_no_main_key() {
        assert!(parse_chord("Ctrl+Alt").is_err());
    }

    #[test]
    fn rejects_unknown_key() {
        assert!(parse_chord("Ctrl+Plus").is_err());
        assert!(parse_chord("Ctrl+ArrowDown").is_err());
    }

    #[test]
    fn rejects_two_main_keys() {
        assert!(parse_chord("Ctrl+V+W").is_err());
    }
}
