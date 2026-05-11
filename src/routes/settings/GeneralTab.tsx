import { Download, RefreshCw } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { ToggleSwitch } from "@/components/ToggleSwitch";
import {
  checkForUpdates,
  downloadAndInstallUpdate,
  getAutostart,
  getSetting,
  registerHotkey,
  setAutostart,
  setSetting,
  type UpdateCheckResult,
} from "@/lib/ipc";
import { applyTheme, type ThemeMode } from "@/lib/theme";
import { cn } from "@/lib/utils";

/**
 * General settings tab.
 *
 * Wired in M6.0:
 *   - Theme picker (light / dark / system) — applies live, persists.
 *   - History limit — read/write of the `history_limit` key.
 *
 * Read-only for now (UI lands in M6.1 once we have a hotkey-rebind capture):
 *   - Hotkey display.
 *
 * The tab loads each value lazily via IPC. We avoid Suspense / a loader
 * library and simply show "—" until the round-trip completes; the IPC is
 * fast (<5 ms p99 in dev) so users won't perceive the gap.
 */
export function GeneralTab() {
  return (
    <div className="space-y-8">
      <header>
        <h2 className="text-lg font-semibold">General</h2>
        <p className="mt-1 text-sm text-muted-foreground">
          Appearance, hotkey, and history retention. Changes apply immediately.
        </p>
      </header>

      <ThemeRow />
      <HistoryLimitRow />
      <HotkeyRebindRow />
      <AutostartRow />
      <UpdatesRow />
    </div>
  );
}

// ---------------- Updates row (M7) ----------------

type UpdatesUiState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "result"; result: UpdateCheckResult }
  | { kind: "installing" }
  | { kind: "install-error"; message: string };

function UpdatesRow() {
  const [state, setState] = useState<UpdatesUiState>({ kind: "idle" });

  const check = async () => {
    setState({ kind: "checking" });
    try {
      const result = await checkForUpdates();
      setState({ kind: "result", result });
    } catch (e: unknown) {
      setState({
        kind: "result",
        result: {
          available: false,
          currentVersion: "?",
          error: e instanceof Error ? e.message : String(e),
        },
      });
    }
  };

  const install = async () => {
    setState({ kind: "installing" });
    try {
      await downloadAndInstallUpdate();
      // On Windows the installer relaunches Klipo for us; if we get here
      // and the app is still running, surface a hint.
      setState({
        kind: "install-error",
        message:
          "Update installed. Klipo should restart automatically; if it doesn't, close and reopen the app manually.",
      });
    } catch (e: unknown) {
      setState({
        kind: "install-error",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  };

  return (
    <Row
      label="Updates"
      description="Klipo can check for new releases on demand. Auto-updates ship signed manifests; until a signing key is provisioned for this build, the check will return a friendly 'not configured' message."
    >
      <div className="space-y-2">
        <button
          type="button"
          onClick={() => void check()}
          disabled={state.kind === "checking" || state.kind === "installing"}
          className={cn(
            "inline-flex items-center gap-2 rounded-md border border-border bg-card px-3 py-2 text-sm transition-colors",
            "hover:bg-accent/40 disabled:cursor-not-allowed disabled:opacity-60",
          )}
        >
          <RefreshCw
            className={cn("h-3.5 w-3.5", state.kind === "checking" && "animate-spin")}
            aria-hidden="true"
          />
          {state.kind === "checking" ? "Checking…" : "Check for updates"}
        </button>

        {state.kind === "result" && state.result.error ? (
          <p className="rounded bg-muted/30 px-2 py-1 text-xs text-muted-foreground">
            {state.result.error}
          </p>
        ) : null}

        {state.kind === "result" && !state.result.error && !state.result.available ? (
          <p className="text-xs text-muted-foreground">
            You&rsquo;re on the latest version (
            <span className="font-mono">{state.result.currentVersion}</span>).
          </p>
        ) : null}

        {state.kind === "result" && state.result.available ? (
          <div className="space-y-2 rounded-md border border-primary/30 bg-primary/5 p-3">
            <div className="text-xs">
              Update available: <span className="font-mono">{state.result.latestVersion}</span>
              {state.result.date ? (
                <span className="text-muted-foreground">
                  {" "}
                  · published {state.result.date.slice(0, 10)}
                </span>
              ) : null}
            </div>
            {state.result.notes ? (
              <pre className="max-h-24 overflow-y-auto whitespace-pre-wrap rounded bg-muted/40 p-2 font-sans text-[11px] text-muted-foreground">
                {state.result.notes}
              </pre>
            ) : null}
            <button
              type="button"
              onClick={() => void install()}
              className="inline-flex items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:opacity-90"
            >
              <Download className="h-3.5 w-3.5" aria-hidden="true" />
              Download and install
            </button>
          </div>
        ) : null}

        {state.kind === "installing" ? (
          <p className="text-xs text-muted-foreground">Downloading and installing update…</p>
        ) : null}

        {state.kind === "install-error" ? (
          <p className="text-xs text-destructive">{state.message}</p>
        ) : null}
      </div>
    </Row>
  );
}

// ---------------- Autostart toggle ----------------

function AutostartRow() {
  const [value, setValue] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const enabled = await getAutostart();
        if (!cancelled) setValue(enabled);
      } catch (e: unknown) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e));
          setValue(false);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const toggle = async () => {
    if (value === null) return;
    const next = !value;
    // Optimistic — flip immediately, revert on error.
    setValue(next);
    setError(null);
    try {
      await setAutostart(next);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
      setValue(!next);
    }
  };

  return (
    <Row
      label="Run at login"
      description="Start Klipo automatically when you sign in. Stored under HKCU on Windows."
    >
      <ToggleSwitch
        checked={value === true}
        disabled={value === null}
        onChange={() => void toggle()}
        label="Run at login"
      />
      {error ? <ErrorLine text={error} /> : null}
    </Row>
  );
}

// ---------------- Theme picker ----------------

const THEME_OPTIONS: { id: ThemeMode; label: string; description: string }[] = [
  { id: "light", label: "Light", description: "Bright UI; matches macOS Aqua / Windows Light." },
  { id: "dark", label: "Dark", description: "Easier on the eyes in low light." },
  { id: "system", label: "System", description: "Follow the OS preference." },
];

function ThemeRow() {
  const [mode, setMode] = useState<ThemeMode>("system");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const raw = await getSetting("theme");
        if (cancelled) return;
        if (raw === "light" || raw === "dark" || raw === "system") {
          setMode(raw);
        }
      } catch (e: unknown) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const handlePick = async (next: ThemeMode) => {
    setError(null);
    setMode(next);
    applyTheme(next);
    try {
      await setSetting("theme", next);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <Row label="Theme" description="Choose your preferred color scheme.">
      <div className="flex flex-wrap gap-2">
        {THEME_OPTIONS.map((opt) => {
          const isActive = opt.id === mode;
          return (
            <button
              key={opt.id}
              type="button"
              onClick={() => void handlePick(opt.id)}
              className={cn(
                "min-w-[140px] rounded-md border px-3 py-2 text-left text-sm transition-colors",
                isActive
                  ? "border-primary bg-primary/10 text-foreground"
                  : "border-border bg-card hover:border-foreground/40 hover:bg-accent/40",
              )}
              aria-pressed={isActive}
            >
              <div className="font-medium">{opt.label}</div>
              <div className="mt-0.5 text-xs text-muted-foreground">{opt.description}</div>
            </button>
          );
        })}
      </div>
      {error ? <ErrorLine text={error} /> : null}
    </Row>
  );
}

// ---------------- History limit ----------------

const MIN_HISTORY = 100;
const MAX_HISTORY = 1_000_000;
const DEFAULT_HISTORY = 10_000;

function HistoryLimitRow() {
  const [value, setValue] = useState<number | null>(null);
  const [draft, setDraft] = useState<string>("");
  const [savingError, setSavingError] = useState<string | null>(null);
  const [savedFlash, setSavedFlash] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const raw = await getSetting("history_limit");
        if (cancelled) return;
        const n = raw ? Number.parseInt(raw, 10) : DEFAULT_HISTORY;
        const clamped = Number.isFinite(n) && n > 0 ? n : DEFAULT_HISTORY;
        setValue(clamped);
        setDraft(String(clamped));
      } catch (e: unknown) {
        if (!cancelled) setSavingError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const commit = async () => {
    setSavingError(null);
    setSavedFlash(false);
    const n = Number.parseInt(draft, 10);
    if (!Number.isFinite(n) || n < MIN_HISTORY || n > MAX_HISTORY) {
      setSavingError(
        `Enter a whole number between ${MIN_HISTORY.toLocaleString()} and ${MAX_HISTORY.toLocaleString()}.`,
      );
      return;
    }
    try {
      await setSetting("history_limit", String(n));
      setValue(n);
      setSavedFlash(true);
      window.setTimeout(() => setSavedFlash(false), 1200);
    } catch (e: unknown) {
      setSavingError(e instanceof Error ? e.message : String(e));
    }
  };

  const dirty = value !== null && draft !== String(value);

  return (
    <Row
      label="History limit"
      description="How many clips Klipo keeps in your local database before pruning the oldest unpinned ones. Default 10,000. The popup itself shows up to the most recent 1,000 at a time for snappy open — use search (Ctrl+F) to find anything older."
    >
      <div className="flex items-center gap-2">
        <input
          type="number"
          inputMode="numeric"
          min={MIN_HISTORY}
          max={MAX_HISTORY}
          step={100}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void commit();
          }}
          className="w-32 rounded-md border border-border bg-card px-2 py-1.5 text-sm outline-none focus:border-primary"
        />
        <button
          type="button"
          onClick={() => void commit()}
          disabled={!dirty}
          className={cn(
            "rounded-md border px-3 py-1.5 text-sm transition-colors",
            dirty
              ? "border-primary bg-primary text-primary-foreground hover:opacity-90"
              : "border-border bg-muted text-muted-foreground",
          )}
        >
          Save
        </button>
        {savedFlash ? <span className="text-xs text-emerald-500">Saved.</span> : null}
      </div>
      {savingError ? <ErrorLine text={savingError} /> : null}
    </Row>
  );
}

// ---------------- Hotkey rebind ----------------

/** Translate a `KeyboardEvent.code` like `KeyV` / `Digit5` / `F12` into the
 * canonical chord-piece our backend's `parse_chord` understands ("V", "5",
 * "F12"). Returns null for keys we don't accept (arrows, punctuation,
 * media keys etc). */
function codeToChordKey(code: string): string | null {
  if (code.startsWith("Key") && code.length === 4) {
    return code.slice(3); // KeyV -> V
  }
  if (code.startsWith("Digit") && code.length === 6) {
    return code.slice(5); // Digit5 -> 5
  }
  if (/^F([1-9]|1\d|2[0-4])$/.test(code)) {
    return code; // F1..F24
  }
  return null;
}

/** Build the modifier list in the canonical order the backend prints back. */
function modifiersFromEvent(e: KeyboardEvent): string[] {
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Meta");
  return mods;
}

function HotkeyRebindRow() {
  const [value, setValue] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedFlash, setSavedFlash] = useState(false);
  const captureRef = useRef<HTMLDivElement | null>(null);

  // Initial load: surface the persisted hotkey or the default.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const raw = await getSetting("hotkey");
        if (!cancelled) setValue(raw ?? "Ctrl+Alt+V");
      } catch {
        if (!cancelled) setValue("Ctrl+Alt+V");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // While `recording` is true, capture key events on the document. We use
  // `keydown` on the window so the user doesn't need to focus a specific
  // input. Esc cancels; a complete chord (≥1 modifier + accepted main key)
  // commits.
  useEffect(() => {
    if (!recording) return undefined;

    const onKey = (e: KeyboardEvent) => {
      // Always intercept while recording — otherwise Tab / Enter etc. would
      // jump focus or submit the parent form.
      e.preventDefault();
      e.stopPropagation();

      if (e.key === "Escape") {
        setRecording(false);
        setError(null);
        return;
      }

      // Bare modifier press (Ctrl alone, etc.) is a no-op until the user
      // picks a main key.
      if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) {
        return;
      }

      const mods = modifiersFromEvent(e);
      if (mods.length === 0) {
        setError("Hotkey needs at least one modifier (Ctrl / Alt / Shift / Meta).");
        return;
      }

      const mainKey = codeToChordKey(e.code);
      if (!mainKey) {
        setError(
          `Key '${e.key}' is not supported as a hotkey. Use a letter, digit, or function key.`,
        );
        return;
      }

      const chord = [...mods, mainKey].join("+");
      setRecording(false);
      setError(null);

      void (async () => {
        try {
          const canonical = await registerHotkey(chord);
          setValue(canonical);
          setSavedFlash(true);
          window.setTimeout(() => setSavedFlash(false), 1200);
        } catch (err: unknown) {
          setError(err instanceof Error ? err.message : String(err));
        }
      })();
    };

    window.addEventListener("keydown", onKey, { capture: true });
    return () => window.removeEventListener("keydown", onKey, { capture: true });
  }, [recording]);

  return (
    <Row
      label="Hotkey"
      description="Press this combination from any app to summon Klipo. Click ‘Rebind’ then press a new combination of modifiers + a letter / digit / function key."
    >
      <div className="flex flex-wrap items-center gap-2">
        <kbd className="inline-block min-w-[120px] rounded border border-border bg-card px-2 py-1 text-center font-mono text-sm">
          {recording ? <span className="text-primary">press a chord…</span> : (value ?? "—")}
        </kbd>
        <button
          type="button"
          onClick={() => {
            setError(null);
            setRecording((r) => !r);
            // Move focus into the row so Esc handler picks it up reliably.
            window.setTimeout(() => captureRef.current?.focus(), 0);
          }}
          className={cn(
            "rounded-md border px-3 py-1.5 text-sm transition-colors",
            recording
              ? "border-primary bg-primary/10 text-primary"
              : "border-border bg-card hover:bg-accent/40",
          )}
        >
          {recording ? "Cancel" : "Rebind"}
        </button>
        {savedFlash ? <span className="text-xs text-emerald-500">Saved.</span> : null}
      </div>
      <div ref={captureRef} tabIndex={-1} className="outline-none" />
      {recording ? (
        <p className="mt-2 text-xs text-muted-foreground">
          Press the new chord (e.g. <kbd className="font-mono">Ctrl+Shift+Space</kbd> won&rsquo;t
          work — only A–Z, 0–9, F1–F24 as the main key). Press <kbd className="font-mono">Esc</kbd>{" "}
          to cancel.
        </p>
      ) : null}
      {error ? <ErrorLine text={error} /> : null}
    </Row>
  );
}

// ---------------- Layout primitives ----------------

function Row({
  label,
  description,
  children,
}: {
  label: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <section className="grid grid-cols-1 gap-2 border-b border-border/40 pb-6 last:border-b-0 md:grid-cols-[200px_1fr] md:gap-6">
      <div>
        <h3 className="text-sm font-medium text-foreground">{label}</h3>
        <p className="mt-1 text-xs text-muted-foreground">{description}</p>
      </div>
      <div>{children}</div>
    </section>
  );
}

function ErrorLine({ text }: { text: string }) {
  return (
    <p className="mt-2 text-xs text-destructive" role="alert">
      {text}
    </p>
  );
}
