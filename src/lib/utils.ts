import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Conditional class joiner used by shadcn/ui components.
 * `cn(["a", false && "b", "c"])` → "a c".
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
