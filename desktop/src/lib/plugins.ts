// Full plugins (docs/PLUGINS.md): code the user enabled runs inside the window and works through `ctx`: it registers
// parts in the window's spots (adding, replacing tazuna's own by id, or hiding them), reads the conversation's data,
// acts (send a message, switch mode, notify), styles the window, adds Cmd-K actions, keeps small settings. Everything a
// plugin registers is taken back when it is turned off or reloaded. Safe mode loads none.
import * as React from "react";
import { useSyncExternalStore } from "react";
import { slots } from "./slots";

export type PaletteAction = { id: string; label: string; hint?: string; run: () => void; source: string };
export type Actions = {
  send: (text: string) => void; newChat: () => void; setMode: (mode: string) => void; notify: (title: string, body: string) => void;
  openLink: (url: string) => void; review: (scope: "uncommitted" | "branch") => void;
};

// the conversation's data as panels get it (API 1), kept here for plugins' useData
let data: unknown = null;
let dataVersion = 0;
const dataListeners = new Set<() => void>();
export function setData(d: unknown) { data = d; dataVersion++; dataListeners.forEach((l) => l()); }
const useData = () => {
  useSyncExternalStore((l) => { dataListeners.add(l); return () => { dataListeners.delete(l); }; }, () => dataVersion);
  return data;
};

let actions: Actions | null = null;
export function setActions(a: Actions) { actions = a; }

const palette = new Map<string, PaletteAction>();
const paletteListeners = new Set<() => void>();
export const pluginActions = () => [...palette.values()];
export const onPaletteChange = (l: () => void) => { paletteListeners.add(l); return () => { paletteListeners.delete(l); }; };

/** Safe mode: the window starts without plugins (Settings, the View menu, or ?safe in the address). */
export const safeMode = () => location.search.includes("safe") || localStorage.getItem("tazuna.safe") === "1";

const loaded = new Map<string, { print: string; undo: (() => void)[] }>();
export const loadErrors = new Map<string, string>();

/** Where a plugin's code is served (only while enabled; Windows' webview reaches custom schemes over http). */
const codeUrl = (id: string, print: string) =>
  `${navigator.userAgent.includes("Windows") ? `http://tzplugin.localhost/${id}` : `tzplugin://localhost/${id}`}?v=${print}`;

function contextFor(id: string, undo: (() => void)[]) {
  const source = `plugin:${id}`;
  return {
    api: 1, id, React, h: React.createElement,
    slots: {
      register: (slot: string, e: { id: string; order?: number; render: (props: any) => React.ReactNode }) =>
        undo.push(slots.register({ id: e.id, slot, order: e.order ?? 500, render: e.render, source })),
      hide: (entryId: string) => undo.push(slots.hide(entryId)),
      map: () => slots.map(),
    },
    useData,
    actions: {
      send: (text: string) => actions?.send(text),
      newChat: () => actions?.newChat(),
      setMode: (mode: string) => actions?.setMode(mode),
      notify: (title: string, body: string) => actions?.notify(title, body),
      openLink: (url: string) => actions?.openLink(url),
      review: (scope: "uncommitted" | "branch") => actions?.review(scope),
    },
    theme: {
      set: (vars: Record<string, string>) => {
        const root = document.documentElement.style, before = Object.keys(vars).map((k) => [k, root.getPropertyValue(k)] as const);
        Object.entries(vars).forEach(([k, v]) => root.setProperty(k, v));
        undo.push(() => before.forEach(([k, v]) => (v ? root.setProperty(k, v) : root.removeProperty(k))));
      },
      css: (text: string) => {
        const el = document.createElement("style");
        el.dataset.plugin = id; el.textContent = text; document.head.appendChild(el);
        undo.push(() => el.remove());
      },
    },
    palette: {
      add: (a: { id: string; label: string; hint?: string; run: () => void }) => {
        const key = `${id}:${a.id}`;
        palette.set(key, { ...a, id: key, source });
        paletteListeners.forEach((l) => l());
        undo.push(() => { palette.delete(key); paletteListeners.forEach((l) => l()); });
      },
    },
    storage: {
      get: (key: string) => { try { return JSON.parse(localStorage.getItem(`tazuna.plugin.${id}.${key}`) ?? "null"); } catch { return null; } },
      set: (key: string, value: unknown) => localStorage.setItem(`tazuna.plugin.${id}.${key}`, JSON.stringify(value)),
    },
  };
}

/** Loads plugin `id` at `print` (unloading an older load first); its errors are kept per plugin. */
export async function load(id: string, print: string) {
  if (loaded.get(id)?.print === print) return;
  unload(id);
  const undo: (() => void)[] = [];
  loaded.set(id, { print, undo });
  loadErrors.delete(id);
  try {
    const mod = await import(/* @vite-ignore */ codeUrl(id, print));
    if (typeof mod.default !== "function") throw new Error("plugin.js has no default export function(ctx)");
    const cleanup = await mod.default(contextFor(id, undo));
    if (typeof cleanup === "function") undo.push(cleanup);
  } catch (e) {
    loadErrors.set(id, String((e as Error)?.message ?? e));
    slots.report({ id, source: `plugin:${id}`, message: `load: ${loadErrors.get(id)}` });
  }
}

/** Takes back everything plugin `id` registered. */
export function unload(id: string) {
  const l = loaded.get(id);
  if (!l) return;
  for (const f of l.undo.reverse()) { try { f(); } catch { /* a broken cleanup must not stop the rest */ } }
  loaded.delete(id);
}

/** What plugin `id` put into the window now: its spot entries (creator mode's check that it took effect). */
export function footprint(id: string, since = 0) {
  const mine = Object.entries(slots.map()).flatMap(([slot, es]) => es.filter((e) => e.source === `plugin:${id}`).map((e) => `${slot}/${e.id}`));
  const errors = slots.errors().filter((e) => e.source === `plugin:${id}` && e.at >= since).map((e) => e.message);
  const actionsAdded = pluginActions().filter((a) => a.source === `plugin:${id}`).length;
  return { entries: mine, errors, actions: actionsAdded, effects: loaded.get(id)?.undo.length ?? 0, loaded: loaded.has(id), loadError: loadErrors.get(id) };
}
