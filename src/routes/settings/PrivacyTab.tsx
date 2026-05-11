import { Folder, ShieldAlert, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";

import { AlertDialog } from "@/components/AlertDialog";
import { ToggleSwitch } from "@/components/ToggleSwitch";
import {
  appDataDirPath,
  getSetting,
  openDataFolder,
  resensitizeHistory,
  setSetting,
  wipeAllData,
} from "@/lib/ipc";
import type { ResensitizeReport } from "@/lib/ipc";
import { cn } from "@/lib/utils";

/**
 * Privacy tab.
 *
 * Wired in M6.2:
 *   - Telemetry toggle (default off; persists `telemetry` setting)
 *   - Sync placeholder (always-off badge until Faz D ships D1/D2)
 *   - Data folder reveal — opens Explorer / Finder at `%APPDATA%/Klipo`
 *   - Wipe all data — hard-deletes clips + blobs after a confirm dialog
 *
 * Stays for v0.3+ once we have the corresponding backend bits:
 *   - Crash reporting opt-in (Sentry)
 *   - Encrypted backup export (.kpb file)
 */
export function PrivacyTab() {
  return (
    <div className="space-y-8">
      <header>
        <h2 className="text-lg font-semibold">Privacy</h2>
        <p className="mt-1 max-w-prose text-sm text-muted-foreground">
          Klipo stores everything locally by default and never sends content off your machine unless
          you opt in. Use the controls below to confirm or change that posture.
        </p>
      </header>

      <TelemetryRow />
      <SyncRow />
      <DataFolderRow />
      <ResensitizeRow />
      <WipeRow />
    </div>
  );
}

// ---------------- Telemetry toggle ----------------

function TelemetryRow() {
  const [value, setValue] = useState<"on" | "off" | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const raw = await getSetting("telemetry");
        if (!cancelled) {
          setValue(raw === "on" ? "on" : "off");
        }
      } catch (e: unknown) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const toggle = async () => {
    if (value === null) return;
    const next = value === "on" ? "off" : "on";
    setValue(next);
    try {
      await setSetting("telemetry", next);
      setError(null);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
      // revert on error
      setValue(value);
    }
  };

  return (
    <Row
      label="Anonymous telemetry"
      description="If enabled, Klipo sends an opaque heartbeat (version + OS) once a day. No clipboard contents, no identifiers. Default: off."
    >
      <ToggleSwitch
        checked={value === "on"}
        disabled={value === null}
        onChange={() => void toggle()}
        label="Telemetry"
      />
      {error ? <p className="mt-2 text-xs text-destructive">{error}</p> : null}
    </Row>
  );
}

// ---------------- Sync placeholder ----------------

function SyncRow() {
  return (
    <Row
      label="Cloud sync"
      description="End-to-end encrypted sync across devices is on the roadmap but not committed to a release. Until it ships, every Klipo install keeps its own local history."
    >
      <div className="flex items-center gap-2">
        <ToggleSwitch checked={false} disabled label="Sync (not yet available)" />
      </div>
    </Row>
  );
}

// ---------------- Data folder reveal ----------------

function DataFolderRow() {
  const [path, setPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const p = await appDataDirPath();
        if (!cancelled) setPath(p);
      } catch (e: unknown) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const open = async () => {
    setError(null);
    try {
      await openDataFolder();
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <Row
      label="Data folder"
      description="The on-disk location of the SQLite database, blob store, and thumbnails. Open it to inspect or back up the files manually."
    >
      <div className="flex flex-col gap-2">
        <button
          type="button"
          onClick={() => void open()}
          className="inline-flex w-fit items-center gap-2 rounded-md border border-border bg-card px-3 py-2 text-sm transition-colors hover:bg-accent/40"
        >
          <Folder className="h-4 w-4" aria-hidden="true" />
          Open data folder
        </button>
        {path ? (
          <code className="block break-all rounded bg-muted/30 px-2 py-1 font-mono text-[11px] text-muted-foreground">
            {path}
          </code>
        ) : null}
        {error ? <p className="text-xs text-destructive">{error}</p> : null}
      </div>
    </Row>
  );
}

// ---------------- Re-scan history (resensitize) ----------------

/**
 * "Re-scan history" action.
 *
 * Triggers a backend pass that re-runs the current sensitive-content regex
 * set against every live, text-bearing clip and flips the `sensitive`
 * column where the verdict has changed. **No data is deleted or
 * rewritten** — only the boolean flag (and `sync_version`) changes.
 *
 * Use case: after a Klipo update bumps the regex set (e.g. v0.1.3 added
 * the `sk-proj-` / `sk-svcacct-` / `sk-admin-` OpenAI formats),
 * historical clips that were captured under the old regex still carry
 * the old verdict. One click here brings them in line with the new rules.
 */
function ResensitizeRow() {
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<
    { kind: "ok"; report: ResensitizeReport } | { kind: "err"; msg: string } | null
  >(null);

  const run = async () => {
    setBusy(true);
    setOutcome(null);
    try {
      const report = await resensitizeHistory();
      setOutcome({ kind: "ok", report });
    } catch (e: unknown) {
      setOutcome({
        kind: "err",
        msg: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Row
      label="Re-scan history"
      description="Re-runs the current sensitive-content rules over every clip already in your history. Updates the red border / blur for clips that match the latest regex set. No data is deleted — only the sensitive flag is updated in place."
    >
      <div className="space-y-2">
        <button
          type="button"
          onClick={() => void run()}
          disabled={busy}
          className={cn(
            "inline-flex w-fit items-center gap-2 rounded-md border border-border bg-card px-3 py-2 text-sm transition-colors hover:bg-accent/40",
            busy && "opacity-60",
          )}
        >
          <ShieldAlert className="h-4 w-4" aria-hidden="true" />
          {busy ? "Scanning…" : "Re-scan history"}
        </button>
        {outcome?.kind === "ok" ? (
          <p className="text-xs text-emerald-500">
            Scanned {outcome.report.scanned} clip
            {outcome.report.scanned === 1 ? "" : "s"}: {outcome.report.flagged} newly flagged
            {outcome.report.unflagged > 0 ? `, ${outcome.report.unflagged} unflagged` : ""},{" "}
            {outcome.report.unchanged} unchanged.
          </p>
        ) : null}
        {outcome?.kind === "err" ? <p className="text-xs text-destructive">{outcome.msg}</p> : null}
      </div>
    </Row>
  );
}

// ---------------- Wipe all data ----------------

function WipeRow() {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<
    { kind: "ok"; wiped: number } | { kind: "err"; msg: string } | null
  >(null);

  const confirm = async () => {
    setConfirming(false);
    setBusy(true);
    setOutcome(null);
    try {
      const wiped = await wipeAllData();
      setOutcome({ kind: "ok", wiped });
    } catch (e: unknown) {
      setOutcome({
        kind: "err",
        msg: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Row
      label="Wipe all data"
      description="Hard-deletes every clip and removes all on-disk blobs / thumbnails. Settings and excluded apps are preserved. There is no undo."
    >
      <div className="space-y-2">
        <button
          type="button"
          onClick={() => setConfirming(true)}
          disabled={busy}
          className={cn(
            "inline-flex w-fit items-center gap-2 rounded-md border px-3 py-2 text-sm transition-colors",
            "border-destructive/40 text-destructive hover:bg-destructive/10",
            busy && "opacity-60",
          )}
        >
          <Trash2 className="h-4 w-4" aria-hidden="true" />
          {busy ? "Wiping…" : "Wipe all clips & blobs"}
        </button>
        {outcome?.kind === "ok" ? (
          <p className="text-xs text-emerald-500">Wiped {outcome.wiped} clip(s).</p>
        ) : null}
        {outcome?.kind === "err" ? <p className="text-xs text-destructive">{outcome.msg}</p> : null}
      </div>

      <AlertDialog
        open={confirming}
        title="Wipe everything?"
        description="All clips, images, and files in the local store will be permanently deleted. This cannot be undone. Settings and excluded apps will be kept."
        confirmLabel="Wipe everything"
        cancelLabel="Cancel"
        variant="destructive"
        onConfirm={() => void confirm()}
        onCancel={() => setConfirming(false)}
      />
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
