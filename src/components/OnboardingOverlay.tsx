import { ArrowRight, Check, Copy, Keyboard, Pin, Power } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { useState } from "react";

import { cn } from "@/lib/utils";

interface OnboardingOverlayProps {
  /** Currently-bound hotkey label, e.g. "Ctrl+Alt+V". */
  hotkeyLabel: string;
  /** Called once the user finishes or skips. Caller persists `onboarding_done`. */
  onComplete: () => void;
}

interface Step {
  icon: LucideIcon;
  title: string;
  body: React.ReactNode;
}

/**
 * 4-step welcome tutorial for first-run users.
 *
 * Lives inside the popup (not the Settings window) so it's the first thing
 * a user sees the very first time they press the hotkey. Skip / Done both
 * route through `onComplete`, which is responsible for persisting the
 * `onboarding_done` setting so the overlay never returns unsolicited.
 *
 * Replaying the tour from Settings → About sets `onboarding_done = off`
 * and re-summons the popup; we handle that flow in `App.tsx` by always
 * checking the persisted flag on focus.
 */
export function OnboardingOverlay({ hotkeyLabel, onComplete }: OnboardingOverlayProps) {
  const [stepIndex, setStepIndex] = useState(0);

  const steps: Step[] = [
    {
      icon: Copy,
      title: "Welcome to Klipo",
      body: (
        <p>
          Every{" "}
          <kbd className="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[11px]">Ctrl+C</kbd> you
          make from now on lands here. Klipo runs quietly in the background — you only see it when
          you summon it.
        </p>
      ),
    },
    {
      icon: Keyboard,
      title: "Press your hotkey to summon Klipo",
      body: (
        <>
          <p>
            From any app, press{" "}
            <kbd className="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[11px]">
              {hotkeyLabel}
            </kbd>{" "}
            to bring this popup up. Type to search, use{" "}
            <kbd className="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[11px]">↑/↓</kbd> to
            navigate, hit{" "}
            <kbd className="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[11px]">Enter</kbd> to
            paste back into the app you came from.
          </p>
          <p className="mt-2 text-muted-foreground">
            You can rebind the hotkey from <em>Settings → General → Hotkey</em>.
          </p>
        </>
      ),
    },
    {
      icon: Pin,
      title: "Pin, delete, and search",
      body: (
        <>
          <p>
            <kbd className="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[11px]">Ctrl+P</kbd>{" "}
            pins a clip so it sticks at the top.{" "}
            <kbd className="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[11px]">Del</kbd>{" "}
            removes the selected clip. Sensitive clips (API keys, credit cards) get a red border and
            you&rsquo;ll be asked to confirm before pasting.
          </p>
          <p className="mt-2 text-muted-foreground">
            Open the gear icon for full settings — themes, excluded apps, privacy controls.
          </p>
        </>
      ),
    },
    {
      icon: Power,
      title: "Klipo lives in your tray",
      body: (
        <>
          <p>
            Klipo runs quietly in the background even when the popup is hidden. To close it, find
            the Klipo icon in the Windows tray (the <span aria-hidden="true">▲</span> chevron next
            to the system clock), <strong>right-click</strong> it, and choose <em>Quit</em>. Or
            press{" "}
            <kbd className="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[11px]">Ctrl+Q</kbd>{" "}
            from the popup —{" "}
            <kbd className="rounded bg-muted/60 px-1.5 py-0.5 font-mono text-[11px]">Esc</kbd> only
            hides the popup; the watcher keeps running.
          </p>
          <p className="mt-2 text-muted-foreground">
            <strong>To use Klipo again after quitting:</strong> open the Start menu, type{" "}
            <em>Klipo</em>, hit Enter — the tray icon and your hotkey come right back. To skip this
            on every login, enable <em>Run at login</em> in Settings → General.
          </p>
        </>
      ),
    },
  ];

  const step = steps[stepIndex]!;
  const Icon = step.icon;
  const isLast = stepIndex === steps.length - 1;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="klipo-onboarding-title"
      className="absolute inset-0 z-40 flex items-center justify-center"
    >
      <div aria-hidden="true" className="absolute inset-0 bg-background/80 backdrop-blur-sm" />
      <div className="relative w-[92%] max-w-sm rounded-xl border border-border bg-card p-5 shadow-xl">
        <div className="mb-3 flex items-center gap-2 text-primary">
          <Icon className="h-5 w-5" aria-hidden="true" />
          <h2 id="klipo-onboarding-title" className="text-base font-semibold">
            {step.title}
          </h2>
        </div>
        <div className="space-y-1 text-sm text-foreground">{step.body}</div>

        <div className="mt-5 flex items-center justify-between gap-2">
          <button
            type="button"
            onClick={onComplete}
            className="text-xs text-muted-foreground underline-offset-2 hover:underline"
          >
            Skip tour
          </button>
          <div className="flex items-center gap-1.5">
            {steps.map((_, i) => (
              <span
                key={i}
                aria-hidden="true"
                className={cn(
                  "h-1.5 w-5 rounded-full transition-colors",
                  i === stepIndex ? "bg-primary" : "bg-muted",
                )}
              />
            ))}
          </div>
          <button
            type="button"
            onClick={() => {
              if (isLast) onComplete();
              else setStepIndex((i) => i + 1);
            }}
            className="inline-flex items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:opacity-90"
          >
            {isLast ? (
              <>
                <Check className="h-3.5 w-3.5" aria-hidden="true" />
                Done
              </>
            ) : (
              <>
                Next
                <ArrowRight className="h-3.5 w-3.5" aria-hidden="true" />
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
}
