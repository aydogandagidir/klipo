import { Info, KeyRound, Lock, Shield, Sliders } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useEffect, useState } from "react";

import { Power } from "lucide-react";

import { quitApp, setSetting } from "@/lib/ipc";
import { ExcludedAppsTab } from "@/routes/settings/ExcludedAppsTab";
import { GeneralTab } from "@/routes/settings/GeneralTab";
import { LicenseTab } from "@/routes/settings/LicenseTab";
import { PrivacyTab } from "@/routes/settings/PrivacyTab";
import { cn } from "@/lib/utils";

/**
 * Klipo Settings window root.
 *
 * Layout (M6):
 *   ┌──────────────────┬───────────────────────────────────────┐
 *   │  Sidebar         │  Active tab pane                       │
 *   │  · General       │                                        │
 *   │  · Excluded apps │  (form fields, descriptions, etc.)     │
 *   │  · Privacy       │                                        │
 *   │  · About         │                                        │
 *   └──────────────────┴───────────────────────────────────────┘
 *
 * For v0.1 we ship the General tab fully wired (theme + hotkey display +
 * history limit), and stub the others with "Coming soon" placeholders.
 * The remaining tabs land alongside their respective backend features
 * (excluded-apps editor in M6.x, privacy/wipe-all in M6.y).
 */
type TabId = "general" | "excluded" | "privacy" | "license" | "about";

interface TabDef {
  id: TabId;
  label: string;
  icon: LucideIcon;
}

const TABS: TabDef[] = [
  { id: "general", label: "General", icon: Sliders },
  { id: "excluded", label: "Excluded apps", icon: Shield },
  { id: "privacy", label: "Privacy", icon: Lock },
  { id: "license", label: "License", icon: KeyRound },
  { id: "about", label: "About", icon: Info },
];

export function Settings() {
  const [active, setActive] = useState<TabId>("general");

  // Esc closes the Settings window. The window itself is configured to
  // hide-instead-of-destroy in lib.rs, so reopening is instant.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        // Importing lazily avoids pulling the Tauri runtime into the popup
        // bundle when this module isn't actually rendered.
        void import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
          void getCurrentWindow().hide();
        });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="settings-window flex h-full bg-background text-foreground">
      <aside className="flex w-52 shrink-0 flex-col border-r border-border bg-muted/30 px-2 py-4">
        <h1 className="mb-4 px-3 text-sm font-semibold tracking-tight text-foreground">
          Klipo Settings
        </h1>
        <nav className="flex flex-col gap-1">
          {TABS.map((t) => {
            const Icon = t.icon;
            const isActive = t.id === active;
            return (
              <button
                key={t.id}
                type="button"
                onClick={() => setActive(t.id)}
                className={cn(
                  "flex items-center gap-2 rounded-md px-3 py-2 text-left text-sm transition-colors",
                  isActive
                    ? "bg-accent text-accent-foreground"
                    : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
                )}
              >
                <Icon className="h-4 w-4" aria-hidden="true" />
                <span>{t.label}</span>
              </button>
            );
          })}
        </nav>
        <div className="mt-auto px-3 pt-4 text-[10px] text-muted-foreground">
          Klipo v0.1.7 · by bluedev · Esc to close
        </div>
      </aside>

      <main className="flex-1 overflow-y-auto p-8">
        {active === "general" && <GeneralTab />}
        {active === "excluded" && <ExcludedAppsTab />}
        {active === "privacy" && <PrivacyTab />}
        {active === "license" && <LicenseTab />}
        {active === "about" && <AboutTab />}
      </main>
    </div>
  );
}

// ---------------- About tab ----------------

function AboutTab() {
  const [replayState, setReplayState] = useState<"idle" | "armed" | "error">("idle");
  /** Klipo version as reported by the running binary. Read from
   * `getVersion()` (Tauri) so it always reflects `CARGO_PKG_VERSION` and
   * never drifts from `Cargo.toml` — a hardcoded string here was the root
   * cause of the v0.1.3 / v0.1.4 mismatch confusion (see CHANGELOG).
   * `null` while loading, otherwise e.g. `"0.1.5"`. */
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const { getVersion } = await import("@tauri-apps/api/app");
        const v = await getVersion();
        if (!cancelled) setVersion(v);
      } catch {
        // If Tauri's app module isn't reachable (e.g. dev-time browser preview),
        // leave version null rather than crash the About tab.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const replay = async () => {
    try {
      await setSetting("onboarding_done", "off");
      setReplayState("armed");
    } catch {
      setReplayState("error");
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-semibold">About Klipo</h2>
        <p className="mt-1 text-sm text-muted-foreground">
          A keyboard-first clipboard manager by{" "}
          <a
            href="https://bluedev.dev"
            target="_blank"
            rel="noreferrer"
            className="font-medium text-primary underline-offset-2 hover:underline"
          >
            bluedev
          </a>
          {" "}— fast, private, local-first. Every clip stays on this machine.
        </p>
      </div>
      <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-2 text-sm">
        <dt className="text-muted-foreground">Version</dt>
        <dd className="font-mono">{version ?? "—"}</dd>
        <dt className="text-muted-foreground">Publisher</dt>
        <dd>
          bluedev (
          <a
            href="https://bluedev.dev"
            target="_blank"
            rel="noreferrer"
            className="text-primary underline-offset-2 hover:underline"
          >
            bluedev.dev
          </a>
          )
        </dd>
        <dt className="text-muted-foreground">License</dt>
        <dd>
          Proprietary —{" "}
          <a
            href="https://github.com/aydogandagidir/klipo/blob/main/LEGAL/EULA.md"
            target="_blank"
            rel="noreferrer"
            className="text-primary underline-offset-2 hover:underline"
          >
            EULA
          </a>{" "}
          ·{" "}
          <a
            href="https://github.com/aydogandagidir/klipo/blob/main/LEGAL/PRIVACY.md"
            target="_blank"
            rel="noreferrer"
            className="text-primary underline-offset-2 hover:underline"
          >
            Privacy
          </a>
        </dd>
        <dt className="text-muted-foreground">Support</dt>
        <dd>
          <a
            href="mailto:support@bluedev.dev"
            className="text-primary underline-offset-2 hover:underline"
          >
            support@bluedev.dev
          </a>
        </dd>
        <dt className="text-muted-foreground">Platform</dt>
        <dd>Windows 10 (1809+) / Windows 11</dd>
      </dl>
      <div className="space-y-2 border-t border-border/40 pt-4">
        <h3 className="text-sm font-medium">Replay welcome tour</h3>
        <p className="text-xs text-muted-foreground">
          Resets the first-run wizard. Press your hotkey from any app afterwards to see the tour
          again.
        </p>
        <button
          type="button"
          onClick={() => void replay()}
          className="rounded-md border border-border bg-card px-3 py-1.5 text-sm transition-colors hover:bg-accent/40"
        >
          Replay onboarding
        </button>
        {replayState === "armed" ? (
          <p className="text-xs text-emerald-500">
            Done — press your hotkey from any app to see the tour again.
          </p>
        ) : null}
        {replayState === "error" ? (
          <p className="text-xs text-destructive">Could not reset onboarding flag.</p>
        ) : null}
      </div>

      <div className="space-y-2 border-t border-border/40 pt-4">
        <h3 className="text-sm font-medium">Quit Klipo</h3>
        <p className="max-w-prose text-xs text-muted-foreground">
          Klipo lives in the Windows system tray (the chevron <span aria-hidden="true">▲</span> area
          next to the clock). Right-click the tray icon and choose <em>Quit</em>, or press{" "}
          <kbd className="rounded bg-muted/50 px-1 font-mono">Ctrl+Q</kbd> while the popup is open —
          both close the app cleanly.
        </p>
        <p className="max-w-prose text-xs text-muted-foreground">
          <strong>To bring Klipo back after quitting:</strong> press{" "}
          <kbd className="rounded bg-muted/50 px-1 font-mono">Win</kbd>, type <em>Klipo</em>, hit
          Enter. The hotkey works again within a second. Enable <em>Run at login</em> above to skip
          this step every reboot.
        </p>
        <button
          type="button"
          onClick={() =>
            void quitApp().catch(() => {
              /* if quit fails the app stays running — visible to user */
            })
          }
          className="inline-flex items-center gap-2 rounded-md border border-destructive/40 bg-card px-3 py-1.5 text-sm text-destructive transition-colors hover:bg-destructive/10"
        >
          <Power className="h-4 w-4" aria-hidden="true" />
          Quit Klipo
        </button>
      </div>
    </div>
  );
}
