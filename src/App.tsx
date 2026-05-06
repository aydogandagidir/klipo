import { listen } from "@tauri-apps/api/event";
import { ArrowRight, Search, Settings as SettingsIcon } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { AlertDialog } from "@/components/AlertDialog";
import { ClipCard } from "@/components/ClipCard";
import { OnboardingOverlay } from "@/components/OnboardingOverlay";
import type { Clip } from "@/lib/ipc";
import {
  deleteClip,
  getLastAppName,
  getSetting,
  hidePopup,
  listClips,
  openSettings,
  pasteClip,
  pinClip,
  quitApp,
  searchClips,
  setSetting,
} from "@/lib/ipc";

/**
 * M5.x popup root.
 *
 * Adds on top of M5:
 *   - Pin / unpin via the per-row pin icon (or Ctrl+P).
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

  const searchRef = useRef<HTMLInputElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);

  const refresh = useCallback(async (currentQuery: string) => {
    // Stamp before the IPC round-trip; difference is logged after the
    // results land. `klipo:search_ms` shows up in DevTools console (and
    // eventually in `bench/results-<yyyy-mm>.md` once the runbook flow
    // gets exercised). No-op on production builds since console.debug
    // is filtered by default; surfaces under DevTools "Verbose" only.
    const t0 = typeof performance !== "undefined" ? performance.now() : 0;
    try {
      let count: number;
      if (currentQuery.trim().length === 0) {
        const items = await listClips(50, 0);
        setClips(items);
        count = items.length;
      } else {
        const hits = await searchClips(currentQuery, 50);
        setClips(hits.map((h) => h.clip));
        count = hits.length;
      }
      setSelectedIndex((prev) => Math.max(0, prev));
      setError(null);
      if (typeof performance !== "undefined") {
        const ms = (performance.now() - t0).toFixed(1);
        // eslint-disable-next-line no-console
        console.debug("klipo:search_ms", ms, "query.len", currentQuery.length, "hits", count);
      }
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlistenNew: (() => void) | null = null;
    let unlistenBumped: (() => void) | null = null;

    void (async () => {
      await refresh("");
      if (cancelled) return;
      unlistenNew = await listen("clip:new", () => void refresh(query));
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
    };
    window.addEventListener("focus", onFocus);
    onFocus();
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  useEffect(() => {
    const node = listRef.current?.querySelector<HTMLElement>(`[data-selected="true"]`);
    node?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  const selectedClip = useMemo<Clip | null>(
    () => clips[selectedIndex] ?? null,
    [clips, selectedIndex],
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

  const handleTogglePin = useCallback(
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
    const clip = clips[selectedIndex];
    if (!clip) return;
    try {
      await deleteClip(clip.id);
      // Select the row that takes its place, or the last row if we deleted
      // the bottom one.
      setSelectedIndex((prev) => Math.min(prev, Math.max(0, clips.length - 2)));
      await refresh(query);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [clips, selectedIndex, query, refresh]);

  const handleKey = useCallback(
    async (e: React.KeyboardEvent) => {
      // While the sensitive-paste dialog is open, let it handle keys.
      if (pendingSensitive) return;

      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) => Math.min(prev + 1, Math.max(0, clips.length - 1)));
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
        if (selectedClip) await handleTogglePin(selectedClip);
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
      clips.length,
      selectedClip,
      handlePaste,
      handleDeleteSelected,
      handleTogglePin,
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
          {clips.length}
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
        Enter or click → paste into the last app you used. Ctrl+P pin · Del delete.
      </div>

      {hint ? (
        <div
          role="status"
          className="rounded-md border border-yellow-500/30 bg-yellow-500/10 px-3 py-1.5 text-[11px] text-yellow-200"
        >
          {hint}
        </div>
      ) : null}

      <div ref={listRef} className="flex-1 space-y-1 overflow-y-auto pr-1">
        {error ? (
          <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">
            {error}
          </div>
        ) : clips.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-1 text-center text-muted-foreground">
            <p className="text-sm">{query ? "No matches" : "No clips yet"}</p>
            <p className="text-xs">
              {query
                ? "Try a different search."
                : "Copy something with Ctrl+C and it will appear here."}
            </p>
          </div>
        ) : (
          clips.map((clip, index) => (
            <ClipCard
              key={clip.id}
              clip={clip}
              selected={index === selectedIndex}
              onClick={() => {
                setSelectedIndex(index);
                void handlePaste(clip);
              }}
              onTogglePin={() => void handleTogglePin(clip)}
            />
          ))
        )}
      </div>

      <div className="flex items-center justify-between gap-2 border-t border-border/30 px-1 pt-1 text-[10px] text-muted-foreground">
        <span className="truncate">
          <kbd className="rounded bg-muted/50 px-1">↑↓</kbd> nav ·{" "}
          <kbd className="rounded bg-muted/50 px-1">Enter</kbd> paste ·{" "}
          <kbd className="rounded bg-muted/50 px-1">Ctrl+P</kbd> pin ·{" "}
          <kbd className="rounded bg-muted/50 px-1">Del</kbd> delete ·{" "}
          <kbd className="rounded bg-muted/50 px-1">Esc</kbd> close ·{" "}
          <kbd className="rounded bg-muted/50 px-1">Ctrl+Q</kbd> quit
        </span>
        <span className="shrink-0 font-mono">{isPasting ? "pasting…" : "Klipo v0.1.1"}</span>
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
    </div>
  );
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
