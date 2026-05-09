import { convertFileSrc, invoke } from "@tauri-apps/api/core";

/**
 * Type-safe wrappers around Rust `#[tauri::command]` handlers.
 *
 * The argument shape and return shape stay in lockstep with
 * `src-tauri/src/commands.rs`. When you add a Rust command, add a wrapper here
 * and re-export from the `ipc` namespace at the bottom.
 */

// ---------- Domain types (mirror src-tauri/src/storage/clips.rs) ----------

export type ClipKind = "text" | "image" | "file" | "rtf" | "html";

export interface Clip {
  id: string;
  kind: ClipKind;
  content_hash: string;
  text_content: string | null;
  blob_path: string | null;
  size_bytes: number;
  source_app: string | null;
  source_url: string | null;
  source_window_title: string | null;
  /** Unix milliseconds. */
  created_at: number;
  pinned: boolean;
  sensitive: boolean;
  category: string | null;
}

export interface SearchHit {
  clip: Clip;
  /** FTS5 BM25 rank — lower is better. `null` when query was empty (recency mode). */
  rank: number | null;
}

// ---------- Smoke test ----------

/** Returns the string "pong:<server_uptime_ms>". */
export async function ping(): Promise<string> {
  return invoke<string>("ping");
}

// ---------- Clip CRUD ----------

/**
 * List recent live clips, pinned-first.
 *
 * Default limit (500) covers the typical "I want to see everything I've
 * captured today" case without paying the cost of a 10K-row JSON payload
 * on every popup open. Callers that want to honour the user's full
 * `history_limit` setting (up to the backend ceiling of 10,000) should
 * pass it explicitly.
 */
export async function listClips(limit = 500, offset = 0): Promise<Clip[]> {
  return invoke<Clip[]>("list_clips", { limit, offset });
}

/** FTS5 + recency search. Same default-limit reasoning as `listClips`. */
export async function searchClips(query: string, limit = 500): Promise<SearchHit[]> {
  return invoke<SearchHit[]>("search_clips", { query, limit });
}

export async function getClip(id: string): Promise<Clip> {
  return invoke<Clip>("get_clip", { id });
}

export async function pinClip(id: string, pinned: boolean): Promise<void> {
  return invoke<void>("pin_clip", { id, pinned });
}

export async function deleteClip(id: string): Promise<void> {
  return invoke<void>("delete_clip", { id });
}

export async function countLiveClips(): Promise<number> {
  return invoke<number>("count_live_clips");
}

// ---------- Window control + paste ----------

export async function hidePopup(): Promise<void> {
  return invoke<void>("hide_popup");
}

/** Quit Klipo entirely. Tears down the watcher, closes windows, exits the
 * process. Wired into the popup's Ctrl+Q shortcut, the tray menu, and the
 * Settings → About tab. Resolves just before the process exits, so callers
 * shouldn't await additional work after this. */
export async function quitApp(): Promise<void> {
  return invoke<void>("quit_app");
}

/** Hide the popup, then write the clip's text into the OS clipboard and
 * synthesize Ctrl+V so the previously-active app receives a paste. */
export async function pasteClip(id: string): Promise<void> {
  return invoke<void>("paste_clip", { id });
}

/** Resolve a relative `blob_path` (as stored in `clips.blob_path`) to an
 * absolute filesystem path, then run it through Tauri's asset-protocol
 * converter so it's loadable by `<img src>`. */
export async function blobAssetUrl(relative: string): Promise<string> {
  const abs = await invoke<string>("resolve_blob_path", { relative });
  return convertFileSrc(abs);
}

/** Like `blobAssetUrl` but for the 192-px webp thumbnail. Falls back to the
 * full blob if the thumbnail does not exist (lazy thumbnail generation may
 * not have completed yet). */
export async function thumbAssetUrl(hash: string): Promise<string> {
  const abs = await invoke<string>("resolve_thumb_path", { hash });
  return convertFileSrc(abs);
}

/** Return a base64 `data:image/...` URL for a clip's thumbnail (or full
 * blob if thumbnail isn't generated yet). Bypasses Tauri's asset protocol —
 * preferred path because it works regardless of `assetProtocol.scope`. */
export async function thumbDataUrl(clipId: string): Promise<string | null> {
  return invoke<string | null>("get_thumb_data_url", { clipId });
}

/** Return a short identifier (exe name on Windows, bundle id on macOS) for
 * the app that was foreground when the user pressed `Ctrl+Alt+V`. The popup
 * shows it as a chip so users know where their paste will land. */
export async function getLastAppName(): Promise<string | null> {
  return invoke<string | null>("get_last_app_name");
}

/** Resolve a clip's blob_path (relative) to an absolute filesystem path,
 * for the drag-and-drop pipeline below. Files dropped into Chromium-based
 * apps must come from native OS drag with real paths. */
export async function resolveBlobAbsolute(relative: string): Promise<string> {
  return invoke<string>("resolve_blob_path", { relative });
}

// ---------- Settings (M6) ----------

/**
 * Setting keys the renderer is allowed to read or write. Mirrors the
 * `is_known_setting` whitelist in `src-tauri/src/commands.rs`. Keep this
 * union in lockstep with that match arm.
 */
export type SettingKey =
  | "theme"
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
  | "autostart";

/** Read a value from the typed-key whitelist. `null` = key not yet set. */
export async function getSetting(key: SettingKey): Promise<string | null> {
  return invoke<string | null>("get_setting", { key });
}

/** Write a value to the typed-key whitelist. Backend validates per-key. */
export async function setSetting(key: SettingKey, value: string): Promise<void> {
  return invoke<void>("set_setting", { key, value });
}

/** Open (or focus) the Settings window. Wired into the popup's gear icon
 * and the tray "Settings…" menu item. */
export async function openSettings(): Promise<void> {
  return invoke<void>("open_settings");
}

// ---------- Excluded apps (M6.1) ----------

/** One row from the `excluded_apps` table. Mirrors `ExcludedApp` in
 * `src-tauri/src/storage/clips.rs`. */
export interface ExcludedApp {
  bundle_id: string;
  label: string | null;
  /** Unix milliseconds. */
  added_at: number;
}

/** List every excluded-app entry, most-recently-added first. */
export async function listExcludedApps(): Promise<ExcludedApp[]> {
  return invoke<ExcludedApp[]>("list_excluded_apps");
}

/** Add (or upsert) an excluded-app entry. Returns `true` if a new row was
 * inserted, `false` if an existing row's label was updated. */
export async function addExcludedApp(
  bundleId: string,
  label: string | null = null,
): Promise<boolean> {
  return invoke<boolean>("add_excluded_app", { bundleId, label });
}

/** Remove an excluded-app entry. Returns `true` if a row was deleted. */
export async function removeExcludedApp(bundleId: string): Promise<boolean> {
  return invoke<boolean>("remove_excluded_app", { bundleId });
}

/**
 * Hide the Settings window for `delayMs` (1–10s, default 3s), then snap
 * the OS's current foreground process identifier and reopen Settings.
 * Returns the captured identifier or `null` if nothing was foreground.
 *
 * Used by the Excluded-apps tab "Capture foreground app" button so users
 * can add a process to the exclusion list without typing its `.exe` name.
 */
export async function captureForegroundApp(delayMs?: number): Promise<string | null> {
  return invoke<string | null>("capture_foreground_app", { delayMs });
}

// ---------- Privacy / data management (M6.2) ----------

/** Absolute path of Klipo's app-data directory (parent of the SQLite DB,
 * `blobs/`, `thumbs/`). Settings UI shows this in a tooltip and as the
 * subtitle of the "Open data folder" button. */
export async function appDataDirPath(): Promise<string> {
  return invoke<string>("app_data_dir_path");
}

/** Open the app-data directory in the OS file manager. */
export async function openDataFolder(): Promise<void> {
  return invoke<void>("open_data_folder");
}

/** Hard-delete every clip + on-disk blob. Settings + excluded-apps stay.
 * Returns the number of clip rows that were removed. */
export async function wipeAllData(): Promise<number> {
  return invoke<number>("wipe_all_data");
}

/** Counters returned by `resensitizeHistory`. UI surfaces these as a
 * "Scanned N clips: M newly flagged" toast. Mirrors `ResensitizeReport`
 * in `src-tauri/src/storage/clips.rs`. */
export interface ResensitizeReport {
  /** Total live, text-bearing clips processed. */
  scanned: number;
  /** Rows that flipped sensitive=false → true (newly detected). */
  flagged: number;
  /** Rows that flipped sensitive=true → false (regex loosened — rare). */
  unflagged: number;
  /** Rows whose verdict matched what was already on disk. */
  unchanged: number;
}

/**
 * Re-run the current sensitive-content regex set against every live,
 * text-bearing clip and update each row's `sensitive` flag.
 *
 * Data-preserving: only `sensitive` changes; text content, blobs, hashes
 * and pinned/deleted state are untouched. Use after a regex bump (e.g.
 * v0.1.3 added the `sk-proj-` OpenAI format) so historical clips inherit
 * the new verdict without losing data.
 */
export async function resensitizeHistory(): Promise<ResensitizeReport> {
  return invoke<ResensitizeReport>("resensitize_history");
}

// ---------- Hotkey rebind (M6.3) ----------

/**
 * Replace the currently-registered global hotkey with `chord`. The backend
 * parses, validates, and atomically swaps in the new chord — on failure the
 * previous chord stays active so the user is never stranded with no
 * shortcut. Returns the canonical chord string the backend persisted.
 */
export async function registerHotkey(chord: string): Promise<string> {
  return invoke<string>("register_hotkey", { chord });
}

// ---------- Autostart on login (M6.4) ----------

/** Whether Klipo is configured to start automatically on user login. On
 * non-Windows builds always returns `false` (macOS lands in v0.2). */
export async function getAutostart(): Promise<boolean> {
  return invoke<boolean>("get_autostart");
}

/** Enable / disable autostart. Returns the new state on success. */
export async function setAutostart(enabled: boolean): Promise<boolean> {
  return invoke<boolean>("set_autostart", { enabled });
}

// ---------- Updates (M7) ----------

export interface UpdateCheckResult {
  /** True iff a newer build is available at the configured endpoint. */
  available: boolean;
  /** Currently-running Klipo version (e.g. "0.1.0"). */
  currentVersion: string;
  /** Available version on the update channel, only set when `available`. */
  latestVersion?: string;
  /** Release notes for the latest version, surfaced as plaintext. */
  notes?: string;
  /** Date the latest release was published, ISO-8601 string. */
  date?: string;
  /** When non-null the check itself failed — most common cause is a
   * placeholder pubkey in `tauri.conf.json` (Updates not configured). */
  error?: string;
}

/**
 * Ask the updater plugin whether a newer release is available.
 *
 * The plugin's `check()` reads `plugins.updater.endpoints` from
 * `tauri.conf.json`, fetches the manifest, and verifies the signature
 * against `plugins.updater.pubkey`. If the pubkey is the placeholder
 * value, the plugin throws — we catch that and surface a friendly
 * "Updates not configured for this build" message instead of letting
 * it bubble up as an opaque error.
 */
export async function checkForUpdates(): Promise<UpdateCheckResult> {
  // Lazy import keeps the plugin out of the popup bundle when Settings is
  // not the active route.
  const { check } = await import("@tauri-apps/plugin-updater");
  const { getVersion } = await import("@tauri-apps/api/app");

  const currentVersion = await getVersion();
  try {
    const update = await check();
    if (update) {
      return {
        available: true,
        currentVersion,
        latestVersion: update.version,
        notes: update.body ?? undefined,
        date: update.date ?? undefined,
      };
    }
    return { available: false, currentVersion };
  } catch (e: unknown) {
    const raw = e instanceof Error ? e.message : String(e);
    const friendly = /pubkey|public key|signature|verify/i.test(raw)
      ? "Updates not configured for this build (signing key not yet provisioned)."
      : raw;
    return { available: false, currentVersion, error: friendly };
  }
}

/**
 * Download and install the latest available update. Caller should have
 * just received `available: true` from `checkForUpdates()`. On Windows,
 * the installer typically restarts the app on its own; on macOS we
 * relaunch via the process plugin (added when Faz C ships).
 */
export async function downloadAndInstallUpdate(): Promise<void> {
  const { check } = await import("@tauri-apps/plugin-updater");
  const update = await check();
  if (!update) {
    throw new Error("No update available");
  }
  await update.downloadAndInstall();
}

// ---------- Re-export bundle ----------

export const ipc = {
  ping,
  listClips,
  searchClips,
  getClip,
  pinClip,
  deleteClip,
  countLiveClips,
  hidePopup,
  pasteClip,
  quitApp,
  blobAssetUrl,
  thumbAssetUrl,
  thumbDataUrl,
  getLastAppName,
  resolveBlobAbsolute,
  getSetting,
  setSetting,
  openSettings,
  listExcludedApps,
  addExcludedApp,
  removeExcludedApp,
  captureForegroundApp,
  appDataDirPath,
  openDataFolder,
  wipeAllData,
  resensitizeHistory,
  registerHotkey,
  getAutostart,
  setAutostart,
  checkForUpdates,
  downloadAndInstallUpdate,
} as const;
