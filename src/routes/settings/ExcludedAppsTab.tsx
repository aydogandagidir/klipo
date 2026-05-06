import { Crosshair, Plus, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import {
  addExcludedApp,
  captureForegroundApp,
  type ExcludedApp,
  listExcludedApps,
  removeExcludedApp,
} from "@/lib/ipc";
import { cn } from "@/lib/utils";

/**
 * Excluded apps tab.
 *
 * What it does:
 *   - Lists every entry in `excluded_apps` (seeded by the migration plus
 *     anything the user has added).
 *   - Lets the user add a new entry by pasting an exe name (Windows) or
 *     bundle id (macOS) plus an optional friendly label.
 *   - Lets the user remove any entry — including seeded ones, if they want.
 *   - **Capture foreground app** — hides the Settings window for 3 s, snaps
 *     whichever app is foreground, and prefills the add form with its
 *     identifier. Saves the user from looking up `MyVault.exe` manually.
 *
 * What it doesn't do (yet):
 *   - Bulk import / export.
 *   - Per-row toggle to disable an entry without removing it.
 */
export function ExcludedAppsTab() {
  const [entries, setEntries] = useState<ExcludedApp[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const list = await listExcludedApps();
      setEntries(list);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return (
    <div className="space-y-6">
      <header>
        <h2 className="text-lg font-semibold">Excluded apps</h2>
        <p className="mt-1 max-w-prose text-sm text-muted-foreground">
          Klipo silently drops clipboard captures while one of these apps is in the foreground. The
          seed list covers common password managers; you can add others (e.g. corporate vaults,
          banking apps) by pasting their process name here.
        </p>
      </header>

      <AddForm onSaved={() => void refresh()} onError={setError} />

      {error ? (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      {entries === null ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : entries.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          No excluded apps. Add one above to start protecting clipboard data from a process.
        </p>
      ) : (
        <ul className="divide-y divide-border rounded-md border border-border">
          {entries.map((entry) => (
            <li key={entry.bundle_id} className="flex items-center gap-3 px-3 py-2 text-sm">
              <div className="min-w-0 flex-1">
                <div className="truncate font-mono text-xs">{entry.bundle_id}</div>
                {entry.label ? (
                  <div className="truncate text-[11px] text-muted-foreground">{entry.label}</div>
                ) : null}
              </div>
              <span className="shrink-0 text-[10px] text-muted-foreground">
                added {timeAgo(entry.added_at)}
              </span>
              <button
                type="button"
                onClick={() => {
                  void removeExcludedApp(entry.bundle_id)
                    .then(() => refresh())
                    .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
                }}
                className="flex h-7 w-7 shrink-0 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                title={`Remove ${entry.bundle_id}`}
                aria-label={`Remove ${entry.bundle_id}`}
              >
                <Trash2 className="h-3.5 w-3.5" aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

const CAPTURE_DELAY_MS = 3_000;

function AddForm({
  onSaved,
  onError,
}: {
  onSaved: () => void;
  onError: (message: string) => void;
}) {
  const [bundleId, setBundleId] = useState("");
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState(false);
  /** Visible state for the capture countdown — `null` = idle, number = remaining seconds. */
  const [captureCountdown, setCaptureCountdown] = useState<number | null>(null);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = bundleId.trim();
    if (trimmed.length === 0 || busy) return;
    setBusy(true);
    try {
      await addExcludedApp(trimmed, label.trim() || null);
      setBundleId("");
      setLabel("");
      onSaved();
    } catch (err: unknown) {
      onError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const captureForeground = async () => {
    if (busy || captureCountdown !== null) return;
    onError("");
    // Local countdown ticker just for UI — backend has its own timer
    // and is the source of truth for what gets captured.
    setCaptureCountdown(Math.ceil(CAPTURE_DELAY_MS / 1_000));
    const interval = window.setInterval(() => {
      setCaptureCountdown((c) => (c === null || c <= 1 ? null : c - 1));
    }, 1_000);

    try {
      const captured = await captureForegroundApp(CAPTURE_DELAY_MS);
      window.clearInterval(interval);
      setCaptureCountdown(null);
      if (captured) {
        setBundleId(captured);
        // Pre-populate the label too, stripping `.exe` and capitalising the
        // bare name — pure UI nicety; the user can edit before submit.
        if (label.length === 0) {
          const stem =
            captured
              .replace(/\.exe$/i, "")
              .split(".")
              .pop() ?? captured;
          setLabel(stem.charAt(0).toUpperCase() + stem.slice(1));
        }
      } else {
        onError("Could not detect a foreground app. Try again with the target window focused.");
      }
    } catch (e: unknown) {
      window.clearInterval(interval);
      setCaptureCountdown(null);
      onError(e instanceof Error ? e.message : String(e));
    }
  };

  const dirty = bundleId.trim().length > 0;
  const capturing = captureCountdown !== null;

  return (
    <form
      onSubmit={(e) => void submit(e)}
      className="space-y-2 rounded-md border border-border bg-card p-3"
    >
      <div className="flex flex-col gap-2 md:flex-row">
        <input
          type="text"
          value={bundleId}
          onChange={(e) => setBundleId(e.target.value)}
          placeholder="Process name (e.g. MyVault.exe) or bundle id (com.example.app)"
          className="flex-1 rounded-md border border-border bg-background px-3 py-2 font-mono text-xs outline-none focus:border-primary"
          aria-label="Process name or bundle id"
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
        />
        <input
          type="text"
          value={label}
          onChange={(e) => setLabel(e.target.value)}
          placeholder="Label (optional)"
          className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm outline-none focus:border-primary md:w-48"
          aria-label="Friendly label"
        />
        <button
          type="submit"
          disabled={!dirty || busy}
          className={cn(
            "flex shrink-0 items-center justify-center gap-1.5 rounded-md border px-3 py-2 text-sm transition-colors",
            dirty && !busy
              ? "border-primary bg-primary text-primary-foreground hover:opacity-90"
              : "border-border bg-muted text-muted-foreground",
          )}
        >
          <Plus className="h-3.5 w-3.5" aria-hidden="true" />
          Add
        </button>
      </div>
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={() => void captureForeground()}
          disabled={busy || capturing}
          className={cn(
            "inline-flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-xs transition-colors",
            capturing
              ? "border-primary bg-primary/10 text-primary"
              : "border-border bg-card hover:bg-accent/40",
          )}
        >
          <Crosshair className="h-3.5 w-3.5" aria-hidden="true" />
          {capturing ? `Switch to that app… ${captureCountdown}s` : "Capture foreground app"}
        </button>
        <p className="text-[11px] text-muted-foreground">
          Hides Klipo Settings for {CAPTURE_DELAY_MS / 1_000}&nbsp;s, then snaps whichever app you
          bring to the front and pastes it above.
        </p>
      </div>
      <p className="text-[11px] text-muted-foreground">
        Tip: on Windows the matcher uses the literal `.exe` filename Windows reports for the
        process. On macOS it uses the reverse-domain bundle id (e.g. `com.example.app`). Match is
        case-sensitive.
      </p>
    </form>
  );
}

function timeAgo(unixMs: number): string {
  if (unixMs <= 0) return "—";
  const diff = Math.max(0, Date.now() - unixMs);
  const days = Math.floor(diff / 86_400_000);
  if (days >= 365) return `${Math.floor(days / 365)}y ago`;
  if (days >= 30) return `${Math.floor(days / 30)}mo ago`;
  if (days >= 1) return `${days}d ago`;
  const hours = Math.floor(diff / 3_600_000);
  if (hours >= 1) return `${hours}h ago`;
  const mins = Math.floor(diff / 60_000);
  if (mins >= 1) return `${mins}m ago`;
  return "just now";
}
