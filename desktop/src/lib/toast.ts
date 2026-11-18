// Snackbars: a short message at the bottom of the window that goes away by itself (an update check's outcome, for one),
// with an action where there is something to do.
import { useSyncExternalStore } from "react";

export type Toast = { id: number; text: string; kind?: "ok" | "info" | "error"; action?: { label: string; run: () => void }; sticky?: boolean };
let toasts: Toast[] = [];
let next = 1;
const listeners = new Set<() => void>();
const set = (t: Toast[]) => { toasts = t; listeners.forEach((l) => l()); };

export const useToasts = () => useSyncExternalStore((l) => { listeners.add(l); return () => { listeners.delete(l); }; }, () => toasts);

export function dismiss(id: number) {
  set(toasts.filter((t) => t.id !== id));
}

/** Shows `text` for a few seconds (longer with an action, longer still for an error; a sticky one until closed); the
 * newest three stay. */
export function toast(text: string, opts: Omit<Toast, "id" | "text"> = {}) {
  const id = next++;
  set([...toasts.slice(-2), { id, text, ...opts }]);
  if (!opts.sticky) setTimeout(() => dismiss(id), opts.kind === "error" ? 9000 : opts.action ? 8000 : 4000);
  return id;
}
