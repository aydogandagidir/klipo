/**
 * Label chip colors, keyed by a label's `autoKey` (the stable classifier key).
 *
 * Auto-detected labels (URL, e-mail, code, …) get a distinct color; the keys
 * mirror `classify::CATEGORIES` in Rust. User-created labels have `autoKey =
 * null` and fall back to a neutral chip. The visible TEXT is always the label's
 * `name` (which the user can rename) — this map only decides the color.
 */

const AUTO_COLORS: Record<string, string> = {
  url: "bg-blue-500/15 text-blue-500",
  email: "bg-violet-500/15 text-violet-500",
  phone: "bg-emerald-500/15 text-emerald-500",
  iban: "bg-amber-500/15 text-amber-600",
  color: "bg-pink-500/15 text-pink-500",
  code: "bg-slate-500/20 text-slate-400",
  json: "bg-orange-500/15 text-orange-500",
  number: "bg-teal-500/15 text-teal-500",
  path: "bg-cyan-500/15 text-cyan-500",
};

const NEUTRAL = "bg-muted/50 text-muted-foreground";

/** Tailwind chip classes for a label, by its `autoKey`. Custom labels
 * (`null` / unknown key) get a neutral chip. */
export function labelColor(autoKey: string | null): string {
  return (autoKey && AUTO_COLORS[autoKey]) || NEUTRAL;
}
