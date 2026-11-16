// The user's panels (docs/WIDGETS.md): the list from their folder, the data they receive (API 1), the tokens that make
// them look like tazuna, and creator mode's message to the agent that builds or changes one.
import { invoke } from "@tauri-apps/api/core";
import spec from "../../../docs/WIDGETS.md?raw";
import type { View } from "./tazuna";

export type UserWidget = { id: string; title: string; slot: string };

export const widgets = {
  list: () => invoke<UserWidget[]>("widgets_list"),
  /** a new panel's folder (empty); its path */
  create: (id: string) => invoke<string>("widgets_new", { id }),
  path: (id: string) => invoke<string>("widgets_path", { id }),
  remove: (id: string) => invoke("widgets_remove", { id }),
};

/** Where a panel's page is served (the tzwidget: scheme; Windows' webview reaches custom schemes over http). */
export const widgetUrl = (id: string) =>
  navigator.userAgent.includes("Windows") ? `http://tzwidget.localhost/${id}` : `tzwidget://localhost/${id}`;

/** A panel id from a request: its first words, lowercase, with a short random end. */
export const widgetId = (ask: string) =>
  `${ask.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").split("-").slice(0, 4).join("-") || "panel"}-${Math.random().toString(36).slice(2, 6)}`;

const TOKENS = ["--ink", "--ink-2", "--muted", "--faint", "--line", "--card", "--field", "--accent", "--accent-soft", "--ok", "--bad", "--warn", "--font-ui", "--font-mono"];

/** The design tokens as the window has them now (light or dark). */
export function tokens(): Record<string, string> {
  const cs = getComputedStyle(document.documentElement);
  return Object.fromEntries(TOKENS.map((t) => [t, cs.getPropertyValue(t).trim()]));
}

/** What a panel receives (API 1): the conversation on screen, read-only. */
export function widgetData(v: View | null) {
  const turns = (v?.blocks ?? []).flatMap((b) => b.kind === "done" ? [{
    cost: b.cost, secs: Math.round(b.secs), tokens: b.usage.input + b.usage.cache_write + b.usage.cache_read + b.usage.output,
    cached: (b.usage.input + b.usage.cache_write + b.usage.cache_read) ? b.usage.cache_read / (b.usage.input + b.usage.cache_write + b.usage.cache_read) : 0,
    model: v!.model,
  }] : []);
  const tools: Record<string, number> = {};
  for (const b of v?.blocks ?? []) if (b.kind === "tool") { const n = b.name.replace(/^mcp__\w+?__/, ""); tools[n] = (tools[n] ?? 0) + 1; }
  return {
    api: 1,
    theme: document.documentElement.classList.contains("dark") ? "dark" : "light",
    session: v && {
      model: v.model, effort: v.effort, mode: v.mode, activity: v.activity, cwd: v.cwd, profile: v.profile, cost: v.cost, turns: v.turns,
      context: v.context, context_window: v.context_window ?? null, usage: v.usage, limits: v.limits ?? null, limits_reset: v.limits_reset ?? null,
      balance: v.balance ?? null,
    },
    turns, tools,
    changes: (v?.changes ?? []).map((c) => ({ turn: c.turn, files: c.files.map((f) => ({ path: f.path, added: f.added, removed: f.removed })) })),
  };
}

/** Creator mode's message: build a new panel, or change one, in the folder the conversation runs in. */
export const creatorMessage = (ask: string, change: boolean) =>
  `${change ? "Change the tazuna desktop panel in this folder (manifest.json, index.html)" : "Build a tazuna desktop panel in this folder: write manifest.json and index.html here"}, following this specification exactly. Write only these two files, in this folder; nothing else on the machine. Keep it small and self-contained (no external URLs, libraries or fonts). When done, say in one line what the panel shows.

<specification>
${spec}
</specification>

What the user wants: ${ask}`;
