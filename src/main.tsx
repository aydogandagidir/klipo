import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "@/App";
import { applyThemeFromSetting } from "@/lib/theme";
import { Settings } from "@/routes/Settings";
import "@/styles/globals.css";

const rootEl = document.getElementById("root");
if (!rootEl) {
  throw new Error("#root element not found in index.html");
}

/**
 * Klipo serves both the frameless clipboard popup AND the chrome'd Settings
 * window from the same `index.html`. Tauri's `tauri.conf.json` distinguishes
 * them with a query param: the popup window is configured without one, and
 * the Settings window opens `/?window=settings`. This switch lets us share
 * a single Vite bundle while still rendering completely different React
 * trees per window — the popup needs no router, and Settings doesn't want
 * the popup's transparent background.
 *
 * Side effect: stamps `data-window=...` on `<html>` so `globals.css` can
 * scope per-window background rules.
 */
function pickRoot(): React.ReactNode {
  const params = new URLSearchParams(window.location.search);
  const which = params.get("window") === "settings" ? "settings" : "popup";
  document.documentElement.dataset.window = which;
  if (which === "settings") {
    return <Settings />;
  }
  return <App />;
}

// Apply saved theme as early as possible so we don't paint a wrong-theme
// flash. The function gracefully handles the case where the IPC isn't
// available yet (e.g. dev hot-reload) and falls back to system preference.
void applyThemeFromSetting();

ReactDOM.createRoot(rootEl).render(<React.StrictMode>{pickRoot()}</React.StrictMode>);
