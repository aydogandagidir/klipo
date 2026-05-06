import {
  File as FileIcon,
  FileText,
  GripVertical,
  Image as ImageIcon,
  Pin,
  ScrollText,
  Type,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { startNativeDrag } from "@/lib/drag";
import { thumbDataUrl } from "@/lib/ipc";
import type { Clip, ClipKind } from "@/lib/ipc";
import { cn } from "@/lib/utils";

/** How far the mouse must move while held down on the row before we treat
 * it as a drag rather than a click. We additionally listen on the *window*
 * (not just the row) so the user can fling the cursor straight off the
 * popup — the previous in-row mousemove version dropped events as soon as
 * the cursor left the row, which made file drag feel broken. */
const DRAG_THRESHOLD_PX = 3;

interface ClipCardProps {
  clip: Clip;
  selected: boolean;
  onClick: () => void;
  /** Toggle pin without selecting/pasting. Click bubbles stopped. */
  onTogglePin?: () => void;
}

const KIND_ICON: Record<ClipKind, LucideIcon> = {
  text: Type,
  image: ImageIcon,
  file: FileIcon,
  rtf: FileText,
  html: ScrollText,
};

function timeAgo(unixMs: number): string {
  const now = Date.now();
  const diff = Math.max(0, now - unixMs);
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return `${seconds}s`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d`;
  const weeks = Math.floor(days / 7);
  if (weeks < 5) return `${weeks}w`;
  const months = Math.floor(days / 30);
  return `${months}mo`;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
}

/** Strip Microsoft CF_HTML header + tags so the popup shows readable text
 * instead of "Version:1.0". The header always ends with the StartFragment
 * comment, so we look for that first; fall back to the whole payload. */
function htmlPreview(payload: string): string {
  const startMarker = "<!--StartFragment-->";
  const endMarker = "<!--EndFragment-->";
  const start = payload.indexOf(startMarker);
  const end = payload.indexOf(endMarker, start === -1 ? 0 : start);
  const body =
    start !== -1 && end !== -1 && end > start
      ? payload.slice(start + startMarker.length, end)
      : // No fragment markers — try to skip the leading "Version:..." header
        // by jumping to the first '<' character.
        payload.slice(Math.max(0, payload.indexOf("<")));

  // Replace tags with spaces (so adjacent text doesn't collapse), decode a
  // few common HTML entities, then collapse whitespace.
  return body
    .replace(/<[^>]+>/g, " ")
    .replace(/&nbsp;/gi, " ")
    .replace(/&amp;/gi, "&")
    .replace(/&lt;/gi, "<")
    .replace(/&gt;/gi, ">")
    .replace(/&quot;/gi, '"')
    .replace(/&#39;/gi, "'")
    .replace(/\s+/g, " ")
    .trim();
}

/** Crude RTF stripper for the popup preview — same idea as paste.rs's
 * `strip_rtf` but TS-side. */
function rtfPreview(payload: string): string {
  let out = "";
  let i = 0;
  while (i < payload.length) {
    const ch = payload[i];
    if (ch === "{" || ch === "}") {
      i += 1;
      continue;
    }
    if (ch === "\\") {
      // Skip control word (backslash + letters), then optional digits + a space.
      i += 1;
      while (i < payload.length && /[a-zA-Z]/.test(payload[i])) i += 1;
      while (i < payload.length && /[0-9-]/.test(payload[i])) i += 1;
      if (payload[i] === " ") i += 1;
      continue;
    }
    out += ch;
    i += 1;
  }
  return out.replace(/\s+/g, " ").trim();
}

function previewText(clip: Clip): string {
  if (clip.kind === "image") {
    return `Image — ${formatSize(clip.size_bytes)}`;
  }
  if (clip.kind === "file") {
    try {
      const paths: unknown = JSON.parse(clip.text_content ?? "[]");
      if (Array.isArray(paths) && paths.length > 0) {
        const first = String(paths[0]);
        const tail = paths.length > 1 ? ` (+${paths.length - 1} more)` : "";
        const name = first.split(/[\\/]/).pop() ?? first;
        return `${name}${tail}`;
      }
    } catch {
      /* fall through */
    }
    return "File";
  }
  if (clip.kind === "html" && clip.text_content) {
    const text = htmlPreview(clip.text_content);
    if (text.length > 0) {
      return text.length > 80 ? `${text.slice(0, 80)}…` : text;
    }
  }
  if (clip.kind === "rtf" && clip.text_content) {
    const text = rtfPreview(clip.text_content);
    if (text.length > 0) {
      return text.length > 80 ? `${text.slice(0, 80)}…` : text;
    }
  }
  if (clip.text_content) {
    const first = clip.text_content.split("\n")[0] ?? "";
    return first.length > 80 ? `${first.slice(0, 80)}…` : first;
  }
  return clip.kind;
}

/** Loads a `data:image/...` URL for image clips. The Rust side serves the
 * 192-px WebP thumbnail when ready, or falls back to the full PNG blob —
 * either way the popup gets pixels without depending on Tauri's asset
 * protocol scope resolution. */
function useImageThumb(clip: Clip): string | null {
  const [url, setUrl] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setUrl(null);

    if (clip.kind !== "image") return;

    void thumbDataUrl(clip.id)
      .then((u) => {
        if (!cancelled) setUrl(u);
      })
      .catch(() => {
        /* surface nothing in the UI; the kind icon will render */
      });

    return () => {
      cancelled = true;
    };
  }, [clip.kind, clip.id]);

  return url;
}

export function ClipCard({ clip, selected, onClick, onTogglePin }: ClipCardProps) {
  const Icon = KIND_ICON[clip.kind];
  const thumb = useImageThumb(clip);
  const draggable = clip.kind === "file" || clip.kind === "image";

  // Drag detection on the row body. We register the mousemove + mouseup
  // listeners on `window` (not the row) so the cursor leaving the popup
  // doesn't kill detection — that was why image drag worked but file drag
  // didn't in the previous build (the user fled the row faster).
  const dragOriginRef = useRef<{ x: number; y: number } | null>(null);
  const dragFiredRef = useRef(false);

  const handleMouseDown = (e: React.MouseEvent) => {
    if (!draggable || e.button !== 0) return;
    dragOriginRef.current = { x: e.clientX, y: e.clientY };
    dragFiredRef.current = false;

    const onMove = (ev: MouseEvent) => {
      if (!dragOriginRef.current || dragFiredRef.current) return;
      const dx = Math.abs(ev.clientX - dragOriginRef.current.x);
      const dy = Math.abs(ev.clientY - dragOriginRef.current.y);
      if (Math.hypot(dx, dy) >= DRAG_THRESHOLD_PX) {
        dragFiredRef.current = true;
        void startNativeDrag(clip).catch(() => {
          /* drag failed silently; row click handler still works */
        });
      }
    };
    const onUp = () => {
      dragOriginRef.current = null;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      window.setTimeout(() => {
        dragFiredRef.current = false;
      }, 0);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  const handleClick = () => {
    if (dragFiredRef.current) return; // user dragged, not clicked
    onClick();
  };

  return (
    <div
      role="button"
      tabIndex={-1}
      onClick={handleClick}
      onMouseDown={handleMouseDown}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      className={cn(
        "group flex w-full items-start gap-3 rounded-md border px-3 py-2 text-left transition-colors",
        draggable ? "cursor-grab active:cursor-grabbing" : "cursor-pointer",
        selected
          ? "border-primary/40 bg-accent/60"
          : "border-transparent hover:border-border hover:bg-accent/30",
        clip.sensitive && "border-l-2 border-l-sensitive",
      )}
      data-clip-id={clip.id}
      data-selected={selected}
      title={draggable ? "Click to paste · drag to drop into a browser window" : undefined}
    >
      {clip.kind === "image" && thumb ? (
        <img
          src={thumb}
          alt=""
          // Three guards in concert make the row's mouseDown drag handler
          // win over the browser's HTML5 drag:
          //   1. `draggable={false}` — disables HTML5 drag attribute.
          //   2. `onDragStart preventDefault` — belt-and-suspenders; some
          //      Chromium versions still try to start a drag from img.
          //   3. NO `pointer-events-none` — we WANT mousedown to fire on
          //      the img and then bubble up to the row's handler.
          draggable={false}
          onDragStart={(e) => e.preventDefault()}
          className="mt-0.5 h-8 w-8 shrink-0 select-none rounded object-cover ring-1 ring-border/40"
          loading="lazy"
          onError={(e) => {
            // Hide broken-image glyph; ClipCard falls back to the kind icon
            // on the next render via `thumb` becoming effectively null.
            (e.currentTarget as HTMLImageElement).style.display = "none";
          }}
        />
      ) : (
        <Icon
          className={cn(
            "mt-0.5 h-4 w-4 shrink-0",
            clip.sensitive ? "text-sensitive" : "text-muted-foreground",
          )}
          aria-hidden="true"
        />
      )}
      <div className="min-w-0 flex-1">
        <div
          className={cn(
            "truncate text-sm",
            clip.sensitive && !selected && "blur-[3px] group-hover:blur-0",
          )}
        >
          {previewText(clip)}
        </div>
        <div className="mt-0.5 flex items-center gap-2 text-[10px] text-muted-foreground">
          <span>{timeAgo(clip.created_at)}</span>
          <span>·</span>
          <span>{formatSize(clip.size_bytes)}</span>
          {clip.source_app ? (
            <>
              <span>·</span>
              <span className="truncate">{clip.source_app}</span>
            </>
          ) : null}
          {clip.sensitive ? (
            <>
              <span>·</span>
              <span className="font-medium uppercase tracking-wide text-sensitive">sensitive</span>
            </>
          ) : null}
        </div>
      </div>
      {draggable ? (
        <span
          role="button"
          tabIndex={-1}
          aria-label="Drag to drop into another app"
          className="mt-0.5 flex h-5 w-4 shrink-0 cursor-grab select-none items-center justify-center text-muted-foreground opacity-50 transition-opacity hover:opacity-100 active:cursor-grabbing group-hover:opacity-90"
          onMouseDown={(e) => {
            // Bypass the row's drag-threshold logic: starting a drag on
            // the handle is unambiguous, so kick it off immediately.
            e.stopPropagation();
            e.preventDefault();
            void startNativeDrag(clip).catch(() => {
              /* drag failed silently */
            });
          }}
        >
          <GripVertical className="h-3 w-3" />
        </span>
      ) : null}
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          onTogglePin?.();
        }}
        className={cn(
          "mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded transition-colors",
          clip.pinned
            ? "text-primary opacity-100"
            : "text-muted-foreground opacity-0 hover:opacity-100 group-hover:opacity-60",
        )}
        aria-label={clip.pinned ? "Unpin clip" : "Pin clip"}
        title={clip.pinned ? "Unpin (Ctrl+P)" : "Pin (Ctrl+P)"}
      >
        <Pin className={cn("h-3 w-3", clip.pinned && "fill-primary")} aria-hidden="true" />
      </button>
    </div>
  );
}
