import { listen } from "@tauri-apps/api/event";
import {
  ArrowRight,
  ExternalLink,
  KeyRound,
  Search,
  Settings as SettingsIcon,
  Star,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { AlertDialog } from "@/components/AlertDialog";
import { ClipCard } from "@/components/ClipCard";
import { OnboardingOverlay } from "@/components/OnboardingOverlay";
import { labelColor } from "@/lib/categories";
import type { Clip, LabelInfo, LicenseStatus, TrialStatus } from "@/lib/ipc";
import {
  addClipLabel,
  deleteClip,
  getLastAppName,
  getLicenseStatus,
  getSetting,
  getTrialStatus,
  hidePopup,
  listAllLabels,
  listClips,
  openSettings,
  pasteClip,
  pinClip,
  quitApp,
  removeClipLabel,
  renameLabel,
  searchClips,
  setClipTitle,
  setSetting,
} from "@/lib/ipc";
import { cn } from "@/lib/utils";

/** Where the popup's "Buy Klipo" link sends the user. Mirrors
 * `PURCHASE_URL` in `src-tauri/src/license/mod.rs` and the LicenseTab. */
const PURCHASE_URL = "https://bluedev.dev/products/klipo";

/**
 * M5.x popup root.
 *
 * Adds on top of M5:
 *   - Favorite / unfavorite via the per-row star icon (or Ctrl+P); favorited
 *     clips float to the top and are filterable via the "Favoriler" chip.
 *   - Delete the selected clip with `Backspace` / `Delete`.
 *   - Confirm paste of sensitive clips before they leak into the wrong app.
 *   - Show the destination app as a chip ("→ Notepad") so the user knows
 *     where Enter will land.
 */
export function App() {
  const [clips, setClips] = useState<Clip[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [isPasting, setIsPasting] = useState(false);
  const [lastApp, setLastApp] = useState<string | null>(null);
  const [pendingSensitive, setPendingSensitive] = useState<Clip | null>(null);
  /** Brief inline notice shown after certain pastes (e.g. file → browser). */
  const [hint, setHint] = useState<string | null>(null);
  /** True when the first-run wizard should be visible. Re-checked on focus
   * so users who replay onboarding from Settings → About see it again. */
  const [showOnboarding, setShowOnboarding] = useState(false);
  const [hotkeyLabel, setHotkeyLabel] = useState<string>("Ctrl+Alt+V");
  /**
   * License + trial state, used to render either:
   *   - the normal "Klipo v0.1.7" footer when Pro,
   *   - a brand-accent "Trial: N days left" footer + "Buy Klipo" link during trial,
   *   - or a full-popup "Trial expired" overlay when neither holds.
   *
   * Fetched once on mount and re-fetched on focus, so a user who activates
   * via Settings sees the popup unblock the next time they hit the hotkey
   * without restarting Klipo.
   */
  const [licenseStatus, setLicenseStatus] = useState<LicenseStatus | null>(null);
  const [trialStatus, setTrialStatus] = useState<TrialStatus | null>(null);
  /**
   * How many clips the popup pulls per refresh. Bound to the user's
   * `history_limit` setting at mount, capped at 1000 so opening the popup
   * stays snappy even if the user has 10K rows in storage. Beyond the cap,
   * search (Ctrl+F) is the right UX — FTS5 makes it instant. Default 500
   * before the setting load resolves.
   */
  const [historyLimit, setHistoryLimit] = useState<number>(500);
  /** Which clip's inline title/category editor is open (by id), or null. Keyed
   * by id (not index) so it survives list refreshes after an edit. */
  const [editingId, setEditingId] = useState<string | null>(null);
  /** Active filter chip: "all", "favorites", or a label name. Applied
   * client-side over the already-fetched `clips` — no extra IPC round-trip. */
  const [filter, setFilter] = useState<string>("all");
  /** Label vocabulary (names + counts + color key) for the filter bar and the
   * editor's add-label autocomplete. */
  const [labels, setLabels] = useState<LabelInfo[]>([]);

  const searchRef = useRef<HTMLInputElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);

  const refresh = useCallback(
    async (currentQuery: string) => {
      // Stamp before the IPC round-trip; difference is logged after the
      // results land. `klipo:search_ms` shows up in DevTools console (and
      // eventually in `bench/results-<yyyy-mm>.md` once the runbook flow
      // gets exercised). No-op on production builds since console.debug
      // is filtered by default; surfaces under DevTools "Verbose" only.
      const t0 = typeof performance !== "undefined" ? performance.now() : 0;
      try {
        let count: number;
        if (currentQuery.trim().length === 0) {
          const items = await listClips(historyLimit, 0);
          setClips(items);
          count = items.length;
        } else {
          const hits = await searchClips(currentQuery, historyLimit);
          setClips(hits.map((h) => h.clip));
          count = hits.length;
        }
        setSelectedIndex((prev) => Math.max(0, prev));
        setError(null);
        if (typeof performance !== "undefined") {
          const ms = (performance.now() - t0).toFixed(1);
          // eslint-disable-next-line no-console
          console.debug(
            "klipo:search_ms",
            ms,
            "query.len",
            currentQuery.length,
            "hits",
            count,
            "limit",
            historyLimit,
          );
        }
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [historyLimit],
  );

  /** Refresh the label vocabulary (filter bar + add-label autocomplete). */
  const refreshLabels = useCallback(async () => {
    try {
      setLabels(await listAllLabels());
    } catch {
      /* non-fatal — filter bar / autocomplete just stay as-is */
    }
  }, []);

  // Read the user's `history_limit` setting once and cap the popup display
  // at 1000 so the popup-open path stays snappy even when the user picks
  // a large pruning ceiling (e.g. 10K). Falls back to the default of 500
  // if the IPC call errors.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const raw = await getSetting("history_limit");
        if (cancelled) return;
        const parsed = raw ? Number.parseInt(raw, 10) : 10_000;
        if (Number.isFinite(parsed) && parsed > 0) {
          setHistoryLimit(Math.min(parsed, 1000));
        }
      } catch {
        // Keep the default; surfacing the IPC error here would block the
        // popup on a setting we have a sane fallback for.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlistenNew: (() => void) | null = null;
    let unlistenBumped: (() => void) | null = null;

    void (async () => {
      await refresh("");
      void refreshLabels();
      if (cancelled) return;
      unlistenNew = await listen("clip:new", () => {
        void refresh(query);
        void refreshLabels();
      });
      unlistenBumped = await listen("clip:bumped", () => void refresh(query));
    })();

    return () => {
      cancelled = true;
      unlistenNew?.();
      unlistenBumped?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refresh]);

  useEffect(() => {
    const timer = setTimeout(() => {
      void refresh(query);
    }, 50);
    return () => clearTimeout(timer);
  }, [query, refresh]);

  // On window focus (e.g. user pressed the hotkey), reset selection +
  // re-fetch the previously-active app name + recheck onboarding flag (so
  // "Replay onboarding" from Settings → About takes effect on next focus).
  useEffect(() => {
    const onFocus = () => {
      searchRef.current?.focus();
      searchRef.current?.select();
      setSelectedIndex(0);
      void getLastAppName().then((name) => setLastApp(name));
      void (async () => {
        try {
          const [done, hk] = await Promise.all([
            getSetting("onboarding_done"),
            getSetting("hotkey"),
          ]);
          setShowOnboarding(done !== "on");
          if (hk) setHotkeyLabel(hk);
        } catch {
          // IPC unavailable — never show wizard in dev hot-reload state.
          setShowOnboarding(false);
        }
      })();
      // License + trial are also re-fetched on focus so activating via
      // Settings while the popup is hidden flips the badge and removes the
      // overlay on the next hotkey press without a restart.
      void (async () => {
        try {
          const [lic, tr] = await Promise.all([getLicenseStatus(), getTrialStatus()]);
          setLicenseStatus(lic);
          setTrialStatus(tr);
        } catch {
          /* IPC unavailable — keep last value */
        }
      })();
    };
    window.addEventListener("focus", onFocus);
    onFocus();
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  useEffect(() => {
    const node = listRef.current?.querySelector<HTMLElement>(`[data-selected="true"]`);
    node?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  // How many clips are favorited (pinned) — drives the "Favoriler" chip.
  const favoritesCount = useMemo(() => clips.filter((c) => c.pinned).length, [clips]);

  // The list the user actually sees, filtered by the active chip.
  const visibleClips = useMemo(() => {
    if (filter === "all") return clips;
    if (filter === "favorites") return clips.filter((c) => c.pinned);
    return clips.filter((c) => c.labels.some((l) => l.name === filter));
  }, [clips, filter]);

  // Keep the selection in range whenever the visible set shrinks (filter
  // toggled, clip deleted, search narrowed).
  useEffect(() => {
    setSelectedIndex((prev) => Math.min(prev, Math.max(0, visibleClips.length - 1)));
  }, [visibleClips.length]);

  const selectedClip = useMemo<Clip | null>(
    () => visibleClips[selectedIndex] ?? null,
    [visibleClips, selectedIndex],
  );

  /** Actually perform the paste IPC; called only after sensitive-confirm
   * if the clip was sensitive. */
  const performPaste = useCallback(
    async (clip: Clip) => {
      if (isPasting) return;
      setIsPasting(true);
      try {
        await pasteClip(clip.id);
        // File paste into a Chromium-based app (any browser, Discord,
        // Slack, Notion, etc.) gets dropped by Chromium's security policy.
        // Klipo writes the path list as plain text fallback, but the user
        // should know why their file didn't actually upload.
        if (clip.kind === "file" && lastApp && isBrowserLikeApp(lastApp)) {
          setHint(
            `${lastApp} doesn't accept file paste — path was inserted as text. Drag-and-drop arrives in M5.x.1.`,
          );
          window.setTimeout(() => setHint(null), 6000);
        }
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setIsPasting(false);
      }
    },
    [isPasting, lastApp],
  );

  /** Entry point for "user picked this clip to paste". Branches on
   * `clip.sensitive` — sensitive clips go through the AlertDialog gate. */
  const handlePaste = useCallback(
    async (clip: Clip) => {
      if (clip.sensitive) {
        setPendingSensitive(clip);
        return;
      }
      await performPaste(clip);
    },
    [performPaste],
  );

  /** Toggle favorite — backed by the `pinned` flag, so favorited clips also
   * float to the top of the list. */
  const handleToggleFavorite = useCallback(
    async (clip: Clip) => {
      try {
        await pinClip(clip.id, !clip.pinned);
        await refresh(query);
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [query, refresh],
  );

  const handleDeleteSelected = useCallback(async () => {
    const clip = visibleClips[selectedIndex];
    if (!clip) return;
    try {
      await deleteClip(clip.id);
      // Select the row that takes its place, or the last row if we deleted
      // the bottom one.
      setSelectedIndex((prev) => Math.min(prev, Math.max(0, visibleClips.length - 2)));
      await refresh(query);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [visibleClips, selectedIndex, query, refresh]);

  // ---- Organize: title + category edits ----

  const handleSetTitle = useCallback(
    async (clip: Clip, title: string | null) => {
      try {
        await setClipTitle(clip.id, title);
        await refresh(query);
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [query, refresh],
  );

  const handleAddLabel = useCallback(
    async (clip: Clip, name: string) => {
      try {
        await addClipLabel(clip.id, name);
        await Promise.all([refresh(query), refreshLabels()]);
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [query, refresh, refreshLabels],
  );

  const handleRemoveLabel = useCallback(
    async (clip: Clip, name: string) => {
      try {
        await removeClipLabel(clip.id, name);
        await Promise.all([refresh(query), refreshLabels()]);
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [query, refresh, refreshLabels],
  );

  const handleRenameLabel = useCallback(
    async (oldName: string, newName: string) => {
      try {
        await renameLabel(oldName, newName);
        await Promise.all([refresh(query), refreshLabels()]);
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [query, refresh, refreshLabels],
  );

  const handleKey = useCallback(
    async (e: React.KeyboardEvent) => {
      // While the sensitive-paste dialog is open, let it handle keys.
      if (pendingSensitive) return;

      // While an inline editor is open, the editor itself owns typing (it
      // stops propagation). If focus has left it, swallow list shortcuts and
      // let Escape close the editor rather than hiding the popup.
      if (editingId) {
        if (e.key === "Escape") {
          e.preventDefault();
          setEditingId(null);
        }
        return;
      }

      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) => Math.min(prev + 1, Math.max(0, visibleClips.length - 1)));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((prev) => Math.max(prev - 1, 0));
      } else if (e.key === "Escape") {
        e.preventDefault();
        try {
          await hidePopup();
        } catch {
          /* best effort */
        }
      } else if (e.key === "Enter" && selectedClip) {
        e.preventDefault();
        await handlePaste(selectedClip);
      } else if ((e.key === "Delete" || e.key === "Backspace") && !isInputFocused()) {
        // Don't hijack Delete/Backspace while user is typing in the search
        // box — only when focus is on the list.
        e.preventDefault();
        await handleDeleteSelected();
      } else if (e.key.toLowerCase() === "p" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        if (selectedClip) await handleToggleFavorite(selectedClip);
      } else if (e.key === "F2" || (e.key.toLowerCase() === "e" && (e.ctrlKey || e.metaKey))) {
        // F2 / Ctrl+E open the title + category editor on the selected clip.
        e.preventDefault();
        if (selectedClip) setEditingId(selectedClip.id);
      } else if (e.key.toLowerCase() === "q" && (e.ctrlKey || e.metaKey)) {
        // Ctrl+Q closes Klipo entirely (vs. Esc which only hides the popup).
        // Discoverable via the footer hint + onboarding step 3.
        e.preventDefault();
        try {
          await quitApp();
        } catch {
          /* if quit fails the app stays running — no-op */
        }
      }
    },
    [
      pendingSensitive,
      editingId,
      visibleClips.length,
      selectedClip,
      handlePaste,
      handleDeleteSelected,
      handleToggleFavorite,
    ],
  );

  return (
    <div
      className="popup-window relative flex h-full flex-col gap-2 overflow-hidden rounded-xl border border-border/40 p-2 text-foreground"
      onKeyDown={handleKey}
      role="dialog"
      aria-label="Klipo clipboard history"
    >
      <div className="flex items-center gap-2 rounded-md bg-card/60 px-3 py-2">
        <Search className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <input
          ref={searchRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search clips… (Ctrl+Alt+V to toggle)"
          className="flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground/60"
          aria-label="Search clipboard history"
        />
        {lastApp ? (
          <span
            className="flex shrink-0 items-center gap-1 rounded bg-muted/40 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground"
            title={`Paste lands in ${lastApp}`}
          >
            <ArrowRight className="h-3 w-3" aria-hidden="true" />
            <span className="max-w-[140px] truncate">{lastApp}</span>
          </span>
        ) : null}
        <span className="rounded bg-muted/40 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
          {visibleClips.length}
        </span>
        <button
          type="button"
          onClick={() => {
            void openSettings().catch((e: unknown) =>
              setError(e instanceof Error ? e.message : String(e)),
            );
          }}
          className="flex h-5 w-5 shrink-0 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent/40 hover:text-foreground"
          title="Settings"
          aria-label="Open Klipo settings"
        >
          <SettingsIcon className="h-3.5 w-3.5" aria-hidden="true" />
        </button>
      </div>

      <div className="px-1 text-[10px] leading-tight text-muted-foreground/80">
        Enter or click → paste into the last app you used. Ctrl+P favorite · F2 edit · Del delete.
      </div>

      {labels.length > 0 || favoritesCount > 0 ? (
        <div className="flex flex-wrap items-center gap-1 px-1">
          <button
            type="button"
            onClick={() => setFilter("all")}
            className={cn(
              "rounded-full px-2 py-0.5 text-[10px] transition-colors",
              filter === "all"
                ? "bg-primary/20 text-primary"
                : "bg-muted/40 text-muted-foreground hover:bg-muted/60",
            )}
          >
            Tümü
          </button>
          {favoritesCount > 0 ? (
            <button
              type="button"
              onClick={() => setFilter(filter === "favorites" ? "all" : "favorites")}
              className={cn(
                "flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] transition-colors",
                filter === "favorites"
                  ? "bg-amber-500/20 text-amber-500"
                  : "bg-muted/40 text-muted-foreground hover:bg-muted/60",
              )}
              title={`Favoriler (${favoritesCount})`}
            >
              <Star className="h-3 w-3" aria-hidden="true" /> Favoriler {favoritesCount}
            </button>
          ) : null}
          {labels.map((l) => {
            const active = filter === l.name;
            return (
              <button
                key={l.name}
                type="button"
                onClick={() => setFilter(active ? "all" : l.name)}
                className={cn(
                  "rounded-full px-2 py-0.5 text-[10px] transition-colors",
                  active
                    ? labelColor(l.autoKey)
                    : "bg-muted/40 text-muted-foreground hover:bg-muted/60",
                )}
                title={`${l.name} (${l.count})`}
              >
                {l.name} {l.count}
              </button>
            );
          })}
        </div>
      ) : null}

      {hint ? (
        <div
          role="status"
          className="rounded-md border border-primary/30 bg-primary/10 px-3 py-1.5 text-[11px] text-primary"
        >
          {hint}
        </div>
      ) : null}

      <div ref={listRef} className="flex-1 space-y-1 overflow-y-auto pr-1">
        {error ? (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        ) : visibleClips.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-1 text-center text-muted-foreground">
            <p className="text-sm">
              {query
                ? "No matches"
                : filter === "favorites"
                  ? "Favori yok"
                  : filter !== "all"
                    ? "Bu etikette öğe yok"
                    : "No clips yet"}
            </p>
            <p className="text-xs">
              {query
                ? "Try a different search."
                : filter !== "all"
                  ? "Başka bir filtre seçin veya Tümü'ne dönün."
                  : "Copy something with Ctrl+C and it will appear here."}
            </p>
          </div>
        ) : (
          visibleClips.map((clip, index) => (
            <ClipCard
              key={clip.id}
              clip={clip}
              selected={index === selectedIndex}
              onClick={() => {
                setSelectedIndex(index);
                void handlePaste(clip);
              }}
              onToggleFavorite={() => void handleToggleFavorite(clip)}
              editing={editingId === clip.id}
              onStartEdit={() => setEditingId(clip.id)}
              onStopEdit={() => setEditingId(null)}
              onSetTitle={(title) => void handleSetTitle(clip, title)}
              onAddLabel={(name) => void handleAddLabel(clip, name)}
              onRemoveLabel={(name) => void handleRemoveLabel(clip, name)}
              onRenameLabel={(oldName, newName) => void handleRenameLabel(oldName, newName)}
              allLabels={labels.map((l) => l.name)}
            />
          ))
        )}
      </div>

      <div className="flex items-center justify-between gap-2 border-t border-border/30 px-1 pt-1 text-[10px] text-muted-foreground">
        <span className="truncate">
          <kbd className="rounded bg-muted/50 px-1">↑↓</kbd> nav ·{" "}
          <kbd className="rounded bg-muted/50 px-1">Enter</kbd> paste ·{" "}
          <kbd className="rounded bg-muted/50 px-1">Ctrl+P</kbd> favorite ·{" "}
          <kbd className="rounded bg-muted/50 px-1">Del</kbd> delete ·{" "}
          <kbd className="rounded bg-muted/50 px-1">Esc</kbd> close ·{" "}
          <kbd className="rounded bg-muted/50 px-1">Ctrl+Q</kbd> quit
        </span>
        <FooterStatus
          isPasting={isPasting}
          licenseStatus={licenseStatus}
          trialStatus={trialStatus}
        />
      </div>

      <AlertDialog
        open={pendingSensitive !== null}
        title="Paste sensitive content?"
        description={`This clip looks like a credential, key, or other secret. It will be pasted into ${
          lastApp ?? "the previously focused app"
        }.`}
        confirmLabel="Paste anyway"
        cancelLabel="Cancel"
        variant="destructive"
        onConfirm={() => {
          const clip = pendingSensitive;
          setPendingSensitive(null);
          if (clip) void performPaste(clip);
        }}
        onCancel={() => setPendingSensitive(null)}
      />

      {showOnboarding ? (
        <OnboardingOverlay
          hotkeyLabel={hotkeyLabel}
          onComplete={() => {
            setShowOnboarding(false);
            void setSetting("onboarding_done", "on").catch(() => {
              /* setting persist failure is non-fatal — user can replay */
            });
          }}
        />
      ) : null}

      {licenseStatus?.tier === "expired" ? <TrialExpiredOverlay /> : null}
    </div>
  );
}

// ---------------- Trial-expired overlay ----------------
//
// Rendered on top of the popup when the 14-day trial has expired AND no
// license is on file. The clip list keeps rendering behind it (greyed-out,
// non-interactive — pointer events are caught by this overlay) so the user
// can see what they'll get back the moment they activate.

function TrialExpiredOverlay() {
  const buy = () => {
    // Tauri exposes `shell.open` for external URLs; we lazy-load it so the
    // popup bundle stays minimal for the common (Pro / trial) path.
    // Tauri's WebView2 hooks `window.open` to send the URL to the user's
    // default browser, so we don't need the shell plugin for a simple
    // outgoing link. Falls through to a normal browser tab in dev.
    window.open(PURCHASE_URL, "_blank", "noopener,noreferrer");
  };

  const goActivate = () => {
    void (async () => {
      try {
        const { openSettings: openSettingsCmd } = await import("@/lib/ipc");
        await openSettingsCmd();
      } catch {
        /* best effort — fall through */
      }
    })();
  };

  return (
    <div
      className="absolute inset-0 z-50 flex items-center justify-center bg-background/90 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      aria-label="Klipo trial expired"
    >
      <div className="mx-4 max-w-md space-y-4 rounded-xl border border-destructive/40 bg-card p-5 shadow-2xl">
        <div className="flex items-center gap-2 text-destructive">
          <KeyRound className="h-5 w-5" aria-hidden="true" />
          <h2 className="text-base font-semibold">Trial expired</h2>
        </div>
        <p className="text-sm text-foreground">
          Your 14-day Klipo trial has ended. Activate a license to keep using clipboard history,
          search, and pinning.
        </p>
        <p className="text-xs text-muted-foreground">
          New captures are paused until you activate. Your existing clips are still here —
          they&apos;ll stay with you whether you continue or not.
        </p>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={goActivate}
            className="inline-flex items-center gap-2 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground transition-opacity hover:opacity-90"
          >
            <KeyRound className="h-4 w-4" aria-hidden="true" />
            Activate license
          </button>
          <button
            type="button"
            onClick={buy}
            className="inline-flex items-center gap-2 rounded-md border border-border bg-card px-3 py-2 text-sm transition-colors hover:bg-accent/40"
          >
            <ExternalLink className="h-4 w-4" aria-hidden="true" />
            Buy Klipo
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------- Footer status (trial / Pro / paste-busy) ----------------

function FooterStatus({
  isPasting,
  licenseStatus,
  trialStatus,
}: {
  isPasting: boolean;
  licenseStatus: LicenseStatus | null;
  trialStatus: TrialStatus | null;
}) {
  if (isPasting) {
    return <span className="shrink-0 font-mono">pasting…</span>;
  }
  // Trial active → countdown badge + buy link in place of the version text.
  if (licenseStatus?.tier === "trial") {
    const days = licenseStatus.trial_days_remaining ?? trialStatus?.days_remaining ?? 0;
    const urgent = days <= 3;
    const buy = () => {
      window.open(PURCHASE_URL, "_blank", "noopener,noreferrer");
    };
    return (
      <span className="flex shrink-0 items-center gap-2">
        <span
          className={cnFooter(
            "rounded px-1.5 py-0.5 font-mono",
            urgent ? "bg-destructive/30 text-destructive" : "bg-muted/50 text-muted-foreground",
          )}
          title={`Free trial — ${days} of 14 days left`}
        >
          Trial: {days}d left
        </span>
        <button
          type="button"
          onClick={buy}
          className="text-primary underline-offset-2 hover:underline"
        >
          Buy Klipo
        </button>
      </span>
    );
  }
  // Pro / Free / Expired all keep the version text. The expired-overlay
  // owns the user-facing message in the expired branch.
  return <span className="shrink-0 font-mono">Klipo v0.1.7</span>;
}

/** Local class joiner — the popup file already imports `useCallback` etc.
 * but not `cn`, and we want to keep this leaf component dependency-free. */
function cnFooter(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

/** True if focus is currently in an editable text field, so we shouldn't
 * hijack Backspace/Delete/etc. that the user is typing into the search. */
function isInputFocused(): boolean {
  const el = document.activeElement;
  if (!el) return false;
  const tag = el.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || (el as HTMLElement).isContentEditable;
}

/** Heuristic: is the destination app likely a Chromium-based web app that
 * silently rejects file paste? Used to show a one-time hint after the user
 * pastes a `kind=file` clip into a browser-shell app — Klipo can't fix
 * Chromium's security policy, but the user should know what happened. */
function isBrowserLikeApp(name: string): boolean {
  const lc = name.toLowerCase();
  return [
    "chrome",
    "msedge",
    "edge",
    "firefox",
    "brave",
    "opera",
    "discord",
    "slack",
    "notion",
    "obsidian",
    "vivaldi",
    "arc",
  ].some((b) => lc.includes(b));
}
