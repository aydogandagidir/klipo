/**
 * Theme application helpers.
 *
 * Klipo persists the user's choice (light / dark / system) under the
 * `theme` key in the SQLite settings table. We keep the rendering details
 * here so both the popup and Settings windows can stay in sync.
 *
 * Strategy:
 *   - "light" / "dark"  → toggle the `.dark` class on `<html>`. shadcn's
 *     CSS variables in `globals.css` switch palettes off that class.
 *   - "system"          → follow `prefers-color-scheme` and re-evaluate
 *     when it changes. We register a single MediaQueryList listener per
 *     window; subsequent calls swap it out cleanly.
 *
 * The IPC may not be available in the very early bootstrap (e.g. before
 * Tauri sets up the IPC bridge). `getSetting` will throw in that case;
 * we swallow it and fall back to system preference so the user never
 * sees a wrong-theme flash.
 */

import { getSetting, setSetting } from "@/lib/ipc";

export type ThemeMode = "light" | "dark" | "system";

let systemListener: ((e: MediaQueryListEvent) => void) | null = null;

function isThemeMode(value: unknown): value is ThemeMode {
  return value === "light" || value === "dark" || value === "system";
}

function clearSystemListener() {
  if (systemListener) {
    window.matchMedia("(prefers-color-scheme: dark)").removeEventListener("change", systemListener);
    systemListener = null;
  }
}

/** Apply the resolved (dark vs light) class without touching IPC. */
function applyResolved(mode: "light" | "dark") {
  const root = document.documentElement;
  if (mode === "dark") {
    root.classList.add("dark");
  } else {
    root.classList.remove("dark");
  }
}

/**
 * Apply a theme mode immediately. For "system", also wires up a media
 * query listener so the UI tracks OS changes live (no app restart).
 */
export function applyTheme(mode: ThemeMode): void {
  clearSystemListener();
  if (mode === "system") {
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    applyResolved(mql.matches ? "dark" : "light");
    systemListener = (e) => applyResolved(e.matches ? "dark" : "light");
    mql.addEventListener("change", systemListener);
  } else {
    applyResolved(mode);
  }
}

/**
 * Read the persisted theme from storage and apply it. Falls back to
 * "system" on any error (IPC not ready, key never set, etc.).
 */
export async function applyThemeFromSetting(): Promise<ThemeMode> {
  let mode: ThemeMode = "system";
  try {
    const raw = await getSetting("theme");
    if (isThemeMode(raw)) mode = raw;
  } catch {
    // Tauri IPC unavailable (dev preview, hot-reload). Stay on system.
  }
  applyTheme(mode);
  return mode;
}

/** Persist + apply in one shot. Used by the Settings UI's theme switcher. */
export async function setTheme(mode: ThemeMode): Promise<void> {
  applyTheme(mode);
  try {
    await setSetting("theme", mode);
  } catch (e) {
    // Surface to the caller; the UI can show a toast.
    throw e instanceof Error ? e : new Error(String(e));
  }
}
