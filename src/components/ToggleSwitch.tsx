import { cn } from "@/lib/utils";

interface ToggleSwitchProps {
  checked: boolean;
  onChange?: () => void;
  disabled?: boolean;
  /** Accessible label — required for screen readers since the switch has no
   * visible text. Form callers usually pair this with a sibling `<label>`
   * positioned to the side; this prop covers the a11y tree either way. */
  label: string;
}

/**
 * iOS-style toggle. Used in Settings → General (Run at login) and
 * Settings → Privacy (Telemetry, Sync placeholder).
 *
 * Visual contract:
 *   - **Off** state has a clearly visible grey track + crisp border so the
 *     control stays legible against a white background. The earlier inline
 *     version used `bg-muted`, which collapsed against `bg-background` in
 *     light theme — making the toggle look invisible until interacted.
 *   - **On** state inverts to the primary colour with no border (the bg
 *     itself is high-contrast).
 *   - Thumb has its own shadow + ring so it pops above the track on either
 *     side regardless of theme.
 */
export function ToggleSwitch({ checked, onChange, disabled, label }: ToggleSwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => {
        if (!disabled) onChange?.();
      }}
      className={cn(
        "relative inline-flex h-6 w-11 shrink-0 items-center rounded-full transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2",
        checked ? "border border-primary bg-primary" : "border border-border bg-input",
        disabled && "cursor-not-allowed opacity-60",
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          "inline-block h-5 w-5 transform rounded-full bg-background shadow-md ring-1 ring-black/10 transition-transform",
          checked ? "translate-x-5" : "translate-x-0.5",
        )}
      />
    </button>
  );
}
