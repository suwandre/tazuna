// The window's named spots (docs/PLUGINS.md "Slots"). Every region renders what is registered for its spot; tazuna's own
// parts register the same way a plugin does, so a plugin can add to a spot, replace a part by registering its id, or hide
// it. An entry that throws renders nothing and its error is kept (Settings, and creator mode's check, show it).
import { Component, useSyncExternalStore, type ReactNode } from "react";

export type Entry<P = any> = {
  /** unique: registering an id again replaces the entry (a plugin replacing a built-in part); unregistering restores it */
  id: string;
  slot: string;
  order: number;
  render: (props: P) => ReactNode;
  /** "tazuna" for built-in parts, "plugin:<id>" for a user's plugin */
  source: string;
};
export type SlotError = { id: string; source: string; message: string; at: number };

const stacks = new Map<string, Entry[]>();
const hidden = new Map<string, number>();
const errors: SlotError[] = [];
const listeners = new Set<() => void>();
let version = 0;
const changed = () => { version++; listeners.forEach((l) => l()); };

export const slots = {
  /** Adds (or, for an id already there, replaces) an entry; the returned function takes it back out. */
  register<P>(e: Entry<P>): () => void {
    const stack = stacks.get(e.id) ?? [];
    stack.push(e as Entry);
    stacks.set(e.id, stack);
    changed();
    return () => {
      const s = stacks.get(e.id) ?? [];
      const i = s.lastIndexOf(e as Entry);
      if (i >= 0) s.splice(i, 1);
      if (!s.length) stacks.delete(e.id);
      changed();
    };
  },
  /** Hides an entry by id (a plugin removing a built-in part); the returned function shows it again. */
  hide(id: string): () => void {
    hidden.set(id, (hidden.get(id) ?? 0) + 1);
    changed();
    return () => { const n = (hidden.get(id) ?? 1) - 1; if (n > 0) hidden.set(id, n); else hidden.delete(id); changed(); };
  },
  /** The entries of a spot, in order: the active (last registered) entry of each id, hidden ones left out. */
  entries(slot: string): Entry[] {
    const out: Entry[] = [];
    for (const [id, s] of stacks) { const e = s[s.length - 1]; if (e.slot === slot && !hidden.has(id)) out.push(e); }
    return out.sort((a, b) => a.order - b.order || a.id.localeCompare(b.id));
  },
  /** Every spot in use and the ids in it, with what provides each (creator mode's map of the window). */
  map(): Record<string, { id: string; order: number; source: string; hidden: boolean }[]> {
    const m: Record<string, { id: string; order: number; source: string; hidden: boolean }[]> = {};
    for (const [id, s] of stacks) { const e = s[s.length - 1]; (m[e.slot] ??= []).push({ id, order: e.order, source: e.source, hidden: hidden.has(id) }); }
    return m;
  },
  errors: () => errors.slice(),
  report(e: Omit<SlotError, "at">) { errors.push({ ...e, at: Date.now() }); if (errors.length > 50) errors.shift(); changed(); },
  subscribe(l: () => void) { listeners.add(l); return () => { listeners.delete(l); }; },
  version: () => version,
};

/** Re-renders when the registry changes. */
export const useSlots = () => useSyncExternalStore(slots.subscribe, slots.version);

class Boundary extends Component<{ entry: Entry; children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() { return { failed: true }; }
  componentDidCatch(err: unknown) { slots.report({ id: this.props.entry.id, source: this.props.entry.source, message: String((err as Error)?.message ?? err) }); }
  render() { return this.state.failed ? null : this.props.children; }
}

/** Renders a spot: its entries in order, each guarded. `only` renders just the first (a spot that holds one part, such
 * as the renderer of an answer's text). */
export function Slot<P>({ name, props, only }: { name: string; props: P; only?: boolean }) {
  useSlots();
  const list = slots.entries(name);
  return <>{(only ? list.slice(0, 1) : list).map((e) => <Boundary key={e.id} entry={e}>{e.render(props)}</Boundary>)}</>;
}

/** A check of replace, restore and hide: `npx tsx -e 'import("./src/lib/slots.tsx").then(m => m.demo())'`. */
export function demo() {
  const ids = (s: string) => slots.entries(s).map((e) => `${e.id}:${e.source}`).join(",");
  slots.register({ id: "t.a", slot: "t", order: 10, render: () => null, source: "tazuna" });
  slots.register({ id: "t.b", slot: "t", order: 20, render: () => null, source: "tazuna" });
  const off = slots.register({ id: "t.a", slot: "t", order: 10, render: () => null, source: "plugin:x" });
  const replaced = ids("t");
  off();
  const restored = ids("t");
  const show = slots.hide("t.b");
  const hiddenOne = ids("t");
  show();
  const ok = replaced === "t.a:plugin:x,t.b:tazuna" && restored === "t.a:tazuna,t.b:tazuna" && hiddenOne === "t.a:tazuna" && ids("t") === restored;
  if (!ok) throw new Error(`slots: ${replaced} | ${restored} | ${hiddenOne}`);
  console.log("slots ok");
}
