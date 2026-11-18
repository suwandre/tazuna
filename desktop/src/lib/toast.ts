// Snackbars: a short message at the bottom of the window that goes away by itself, with an action where there is something
// to do. A modal one (the outcome of something the user just asked for, such as Check for updates) shows in the middle over
// a dimmed window instead, until closed: a snackbar at the bottom sat on Settings' own text (2026-11-18).
import { useSyncExternalStore } from "react";

export type Toast = { id: number; text: string; kind?: "ok" | "info" | "error"; action?: { label: string; run: () => void }; sticky?: boolean;
  modal?: boolean; detail?: string };
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
  if (!opts.sticky && !opts.modal) setTimeout(() => dismiss(id), opts.kind === "error" ? 9000 : opts.action ? 8000 : 4000);
  return id;
}
