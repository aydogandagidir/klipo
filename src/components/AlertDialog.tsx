import { useEffect, useRef } from "react";
import { cn } from "@/lib/utils";

interface AlertDialogProps {
  /** Is the dialog visible? */
  open: boolean;
  /** Bold one-line title shown at the top of the dialog. */
  title: string;
  /** Body text — usually one or two short sentences. */
  description: string;
  /** Label for the destructive / confirming button. Defaults to "OK". */
  confirmLabel?: string;
  /** Label for the cancel button. Defaults to "Cancel". */
  cancelLabel?: string;
  /** Style hint — `destructive` paints the confirm button red. */
  variant?: "default" | "destructive";
  /** Called when the user confirms (Enter or click). */
  onConfirm: () => void;
  /** Called when the user cancels (Esc or click). */
  onCancel: () => void;
}

/**
 * Lightweight alert dialog used for paste-confirm flows. Modeled after
 * shadcn/ui's `AlertDialog` but inlined to avoid pulling in
 * `@radix-ui/react-alert-dialog` for one screen. Keyboard handling:
 *
 *   - `Esc`   → cancel
 *   - `Enter` → confirm (default-focused on the confirm button)
 */
export function AlertDialog({
  open,
  title,
  description,
  confirmLabel = "OK",
  cancelLabel = "Cancel",
  variant = "default",
  onConfirm,
  onCancel,
}: AlertDialogProps) {
  const confirmRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    if (open) {
      // Focus the confirm button on open so Enter triggers the destructive
      // action — matches what most OS-level alert dialogs do.
      const t = window.setTimeout(() => confirmRef.current?.focus(), 0);
      return () => window.clearTimeout(t);
    }
    return undefined;
  }, [open]);

  if (!open) return null;

  return (
    <div
      role="alertdialog"
      aria-modal="true"
      className="absolute inset-0 z-50 flex items-center justify-center"
      onKeyDown={(e) => {
        // Stop these keys from leaking to the parent popup's handler so
        // ↑/↓ and Enter don't navigate the clip list while the dialog is up.
        e.stopPropagation();
        if (e.key === "Escape") {
          e.preventDefault();
          onCancel();
        } else if (e.key === "Enter") {
          e.preventDefault();
          onConfirm();
        }
      }}
    >
      <div
        aria-hidden="true"
        className="absolute inset-0 bg-background/70 backdrop-blur-sm"
        onClick={onCancel}
      />
      <div className="relative w-[90%] max-w-sm rounded-lg border border-border bg-card p-4 shadow-lg">
        <h2 className="text-sm font-semibold text-foreground">{title}</h2>
        <p className="mt-1 text-xs text-muted-foreground">{description}</p>
        <div className="mt-4 flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            className="rounded-md border border-border bg-transparent px-3 py-1 text-xs hover:bg-accent"
          >
            {cancelLabel}
          </button>
          <button
            ref={confirmRef}
            type="button"
            onClick={onConfirm}
            className={cn(
              "rounded-md px-3 py-1 text-xs font-medium",
              variant === "destructive"
                ? "bg-destructive text-destructive-foreground hover:bg-destructive/90"
                : "bg-primary text-primary-foreground hover:bg-primary/90",
            )}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
