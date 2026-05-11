import { CheckCircle2, ExternalLink, KeyRound, RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";

import {
  activateLicense,
  deactivateLicense,
  getLicenseStatus,
  getTrialStatus,
  reverifyLicense,
} from "@/lib/ipc";
import type { LicenseStatus, ReverifyOutcome, TrialStatus } from "@/lib/ipc";
import { cn } from "@/lib/utils";

/**
 * License tab (M8).
 *
 * Status pill + activation form + Pro-mode controls. Mirrors the WA-contact
 * `license-tab.js` UX:
 *
 *   - **Trial active** → yellow pill, banner "Free trial — N days left of 14".
 *   - **Trial expired (no license)** → red pill, "Activate license to continue".
 *   - **Pro / verified** → green pill, email + masked key + activated date.
 *   - **Pro / grace** → amber pill, "offline grace until …".
 *   - **expired-grace (license stale)** → red pill, suggests Re-check now.
 *
 * The activation form posts to `activateLicense()`, which incurs a Gumroad
 * round-trip (≈600 ms) and counts the device against the user's 3-device
 * limit. The "Re-check now" button does NOT count a device.
 *
 * Buy URL: `https://bluedev.dev/products/klipo` (placeholder — final URL
 * wires up once the Gumroad listing is live and the bluedev.dev product
 * page lands).
 */
const PURCHASE_URL = "https://bluedev.dev/products/klipo";

export function LicenseTab() {
  const [status, setStatus] = useState<LicenseStatus | null>(null);
  const [trial, setTrial] = useState<TrialStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const [s, t] = await Promise.all([getLicenseStatus(), getTrialStatus()]);
      setStatus(s);
      setTrial(t);
      setError(null);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  return (
    <div className="space-y-8">
      <header>
        <h2 className="text-lg font-semibold">License</h2>
        <p className="mt-1 max-w-prose text-sm text-muted-foreground">
          Klipo is free to try for 14 days. Activate a license key to keep using it past the trial.
          Each license unlocks 3 devices.
        </p>
      </header>

      {error ? (
        <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
          {error}
        </div>
      ) : null}

      <StatusPill status={status} trial={trial} />

      {status?.tier === "pro" ? (
        <ProDetailsRow status={status} onChange={() => void refresh()} />
      ) : (
        <ActivateRow status={status} trial={trial} onActivated={() => void refresh()} />
      )}

      <BuyLinkRow />
    </div>
  );
}

// ---------------- Status pill ----------------

function StatusPill({
  status,
  trial,
}: {
  status: LicenseStatus | null;
  trial: TrialStatus | null;
}) {
  if (!status || !trial) {
    return (
      <Row label="Status" description="Loading current entitlement state…">
        <span className="text-sm text-muted-foreground">…</span>
      </Row>
    );
  }

  let pill: { label: string; tone: "green" | "amber" | "red" | "neutral" };
  let summary = "";

  if (status.tier === "pro" && status.reason === "verified") {
    pill = { label: "Klipo Pro — Activated", tone: "green" };
    summary = status.email ? `Licensed to ${status.email}.` : "License is active and verified.";
  } else if (status.tier === "pro" && status.reason === "grace") {
    pill = { label: "Klipo Pro — Offline grace", tone: "amber" };
    const until = status.grace_until ? new Date(status.grace_until).toLocaleDateString() : "soon";
    summary = `License hasn't been re-verified recently. Grace window valid until ${until}. Use "Re-check now" once you're back online.`;
  } else if (status.tier === "free" && status.reason === "expired-grace") {
    pill = { label: "License re-check needed", tone: "red" };
    summary =
      "Your license is on file but couldn't be verified within the 30-day offline grace window. Click Re-check now to refresh, or activate a different key.";
  } else if (status.tier === "trial") {
    const days = status.trial_days_remaining ?? trial.days_remaining;
    pill = {
      label: `Free trial — ${days} day${days === 1 ? "" : "s"} left`,
      tone: days <= 3 ? "amber" : "neutral",
    };
    summary = `Free trial — ${days} day${days === 1 ? "" : "s"} left of 14. Activate to unlock the full lifetime license.`;
  } else if (status.tier === "expired") {
    pill = { label: "Trial expired", tone: "red" };
    summary = "Trial expired. Activate a license to continue using Klipo.";
  } else {
    pill = { label: "Free", tone: "neutral" };
    summary = "No license on file.";
  }

  const toneClass = {
    green: "border-emerald-500/40 bg-emerald-500/10 text-emerald-300",
    amber: "border-amber-500/40 bg-amber-500/10 text-amber-300",
    red: "border-destructive/40 bg-destructive/10 text-destructive",
    neutral: "border-border bg-muted/30 text-muted-foreground",
  }[pill.tone];

  return (
    <Row label="Status" description="Current entitlement, recomputed on every Settings open.">
      <div className="space-y-2">
        <span
          className={cn(
            "inline-flex items-center gap-1.5 rounded-full border px-3 py-1 text-xs font-medium",
            toneClass,
          )}
        >
          {pill.tone === "green" ? <CheckCircle2 className="h-3.5 w-3.5" aria-hidden="true" /> : null}
          {pill.label}
        </span>
        <p className="max-w-prose text-xs text-muted-foreground">{summary}</p>
      </div>
    </Row>
  );
}

// ---------------- Pro details row (replace the form when active) ----------------

function ProDetailsRow({
  status,
  onChange,
}: {
  status: LicenseStatus;
  onChange: () => void;
}) {
  const [busy, setBusy] = useState<"recheck" | "deactivate" | null>(null);
  const [recheckMsg, setRecheckMsg] = useState<string | null>(null);
  const [recheckOk, setRecheckOk] = useState<boolean | null>(null);

  const recheck = async () => {
    setBusy("recheck");
    setRecheckMsg(null);
    setRecheckOk(null);
    try {
      const outcome: ReverifyOutcome = await reverifyLicense();
      switch (outcome.kind) {
        case "verified":
          setRecheckOk(true);
          setRecheckMsg("Re-verified. License is good for another 30 days offline.");
          break;
        case "invalid":
          setRecheckOk(false);
          setRecheckMsg(`Gumroad rejected the key: ${outcome.message}`);
          break;
        case "refunded":
          setRecheckOk(false);
          setRecheckMsg(`License is no longer valid: ${outcome.message}`);
          break;
        case "network":
        case "server":
          setRecheckOk(false);
          setRecheckMsg(
            `Could not reach Gumroad: ${outcome.message}. Existing license stays active until grace expires.`,
          );
          break;
        case "no-license":
          setRecheckOk(false);
          setRecheckMsg("No license on file to re-check.");
          break;
      }
      onChange();
    } catch (e: unknown) {
      setRecheckOk(false);
      setRecheckMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  };

  const deactivate = async () => {
    setBusy("deactivate");
    try {
      await deactivateLicense();
      onChange();
    } catch (e: unknown) {
      setRecheckOk(false);
      setRecheckMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  };

  const fmt = (ms: number | null) => (ms ? new Date(ms).toLocaleString() : "—");

  return (
    <Row
      label="License"
      description="Details of your active Klipo Pro license. Re-check refreshes the offline grace window without using a device slot."
    >
      <div className="space-y-3">
        <dl className="grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-xs">
          <dt className="text-muted-foreground">Email</dt>
          <dd>{status.email ?? "—"}</dd>
          <dt className="text-muted-foreground">Key</dt>
          <dd className="font-mono">{status.key_masked ?? "—"}</dd>
          {status.product_name ? (
            <>
              <dt className="text-muted-foreground">Product</dt>
              <dd>{status.product_name}</dd>
            </>
          ) : null}
          <dt className="text-muted-foreground">Activated</dt>
          <dd>{fmt(status.activated_at)}</dd>
          <dt className="text-muted-foreground">Last verified</dt>
          <dd>{fmt(status.last_verified_at)}</dd>
        </dl>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => void recheck()}
            disabled={busy !== null}
            className={cn(
              "inline-flex items-center gap-2 rounded-md border border-border bg-card px-3 py-2 text-sm transition-colors hover:bg-accent/40",
              busy !== null && "opacity-60",
            )}
          >
            <RefreshCw
              className={cn("h-4 w-4", busy === "recheck" && "animate-spin")}
              aria-hidden="true"
            />
            {busy === "recheck" ? "Re-checking…" : "Re-check now"}
          </button>
          <button
            type="button"
            onClick={() => void deactivate()}
            disabled={busy !== null}
            className={cn(
              "inline-flex items-center gap-2 rounded-md border border-destructive/40 bg-card px-3 py-2 text-sm text-destructive transition-colors hover:bg-destructive/10",
              busy !== null && "opacity-60",
            )}
          >
            <Trash2 className="h-4 w-4" aria-hidden="true" />
            {busy === "deactivate" ? "Deactivating…" : "Deactivate"}
          </button>
        </div>
        {recheckMsg ? (
          <p
            className={cn(
              "text-xs",
              recheckOk ? "text-emerald-500" : "text-destructive",
            )}
          >
            {recheckMsg}
          </p>
        ) : null}
      </div>
    </Row>
  );
}

// ---------------- Activate form (free / trial / expired) ----------------

function ActivateRow({
  status,
  trial,
  onActivated,
}: {
  status: LicenseStatus | null;
  trial: TrialStatus | null;
  onActivated: () => void;
}) {
  const [email, setEmail] = useState("");
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const activate = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await activateLicense(key.trim(), email.trim() || undefined);
      setKey("");
      onActivated();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  // Trial-active or expired banner above the form.
  let banner: React.ReactNode = null;
  if (status?.tier === "trial" && trial) {
    banner = (
      <div
        className={cn(
          "rounded-md border px-3 py-2 text-xs",
          trial.days_remaining <= 3
            ? "border-amber-500/40 bg-amber-500/10 text-amber-200"
            : "border-border bg-muted/30 text-muted-foreground",
        )}
      >
        Free trial — {trial.days_remaining} day{trial.days_remaining === 1 ? "" : "s"} left of 14.
        Activate to unlock the full lifetime license.
      </div>
    );
  } else if (status?.tier === "expired") {
    banner = (
      <div className="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive">
        Trial expired. Activate a license below to continue using Klipo.
      </div>
    );
  }

  return (
    <Row
      label="Activate"
      description="Enter the license key you received from Gumroad. The first activation on each device counts against your 3-device allowance."
    >
      <form className="space-y-3" onSubmit={(e) => void activate(e)}>
        {banner}
        <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
          <div className="space-y-1">
            <label htmlFor="license-email" className="text-xs text-muted-foreground">
              Email (optional)
            </label>
            <input
              id="license-email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="you@example.com"
              className="w-full rounded-md border border-border bg-card px-2 py-1.5 text-sm outline-none placeholder:text-muted-foreground/60 focus:border-primary"
            />
          </div>
          <div className="space-y-1">
            <label htmlFor="license-key" className="text-xs text-muted-foreground">
              License key
            </label>
            <input
              id="license-key"
              type="text"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder="ABCD-1234-EFGH-5678"
              className="w-full rounded-md border border-border bg-card px-2 py-1.5 font-mono text-sm outline-none placeholder:text-muted-foreground/60 focus:border-primary"
              required
            />
          </div>
        </div>
        <button
          type="submit"
          disabled={busy || key.trim().length === 0}
          className={cn(
            "inline-flex items-center gap-2 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-opacity hover:opacity-90",
            (busy || key.trim().length === 0) && "opacity-60",
          )}
        >
          <KeyRound className="h-4 w-4" aria-hidden="true" />
          {busy ? "Activating…" : "Activate"}
        </button>
        {error ? <p className="text-xs text-destructive">{error}</p> : null}
        <p className="text-[11px] text-muted-foreground">
          Don't have a key?{" "}
          <a
            href={PURCHASE_URL}
            target="_blank"
            rel="noreferrer"
            className="text-primary underline-offset-2 hover:underline"
          >
            Get one at bluedev.dev/products/klipo
          </a>
          .
        </p>
      </form>
    </Row>
  );
}

// ---------------- Buy link row (always visible) ----------------

function BuyLinkRow() {
  return (
    <Row
      label="Buy Klipo"
      description="One purchase covers 3 devices. No subscriptions; updates within the same major release are free."
    >
      <a
        href={PURCHASE_URL}
        target="_blank"
        rel="noreferrer"
        className="inline-flex w-fit items-center gap-2 rounded-md border border-border bg-card px-3 py-2 text-sm transition-colors hover:bg-accent/40"
      >
        <ExternalLink className="h-4 w-4" aria-hidden="true" />
        bluedev.dev/products/klipo
      </a>
    </Row>
  );
}

// ---------------- Layout primitive (mirrors PrivacyTab) ----------------

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
