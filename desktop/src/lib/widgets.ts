// The user's panels and plugins (docs/PLUGINS.md): the list from their folder, the data they receive (API 1), the tokens that make
// them look like tazuna, and creator mode's message to the agent that builds or changes one.
import { invoke } from "@tauri-apps/api/core";
import spec from "../../../docs/PLUGINS.md?raw";
import { slots } from "./slots";
import type { View } from "./tazuna";

export type UserWidget = {
  id: string; title: string; kind: "panel" | "plugin"; slot?: string;
  /** a plugin's: enabled (this exact code), changed (enabled, code changed since), new; its code's fingerprint; what it says it does */
  state?: "enabled" | "changed" | "new"; print?: string; about?: string;
};

export const widgets = {
  list: () => invoke<UserWidget[]>("widgets_list"),
  /** a new panel's folder (empty); its path */
  create: (id: string) => invoke<string>("widgets_new", { id }),
  path: (id: string) => invoke<string>("widgets_path", { id }),
  remove: (id: string) => invoke("widgets_remove", { id }),
  /** a plugin's code as text (to read before enabling it) */
  code: (id: string) => invoke<string>("plugins_code", { id }),
  /** a plugin on (for the code it had when listed) or off */
  enable: (id: string, print: string, on: boolean) => invoke("plugins_enable", { id, print, on }),
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

/** Creator mode's message: build a panel or plugin, or change one, in the folder the conversation runs in. It carries the
 * specification and a map of the window as it is now (every spot, the ids in it, and what provides each), so the agent
 * can name tazuna's own parts to replace or hide. */
export const creatorMessage = (ask: string, change: boolean) =>
  `${change ? "Change the tazuna desktop panel or plugin in this folder" : "Build a tazuna desktop panel or plugin in this folder"}, following this specification exactly: a panel (manifest.json, index.html) when showing information in the right column is enough; a plugin (manifest.json, plugin.js) when the change touches the window itself (another spot, replacing or hiding tazuna's own parts, styling, Cmd-K actions) or must act. Write only in this folder; nothing else on the machine. One self-contained file of code (no imports, external URLs, libraries or fonts). When done, say in one line what it does and where it shows. tazuna then loads it and tells you if it failed or showed nowhere.

<specification>
${spec}
</specification>

<window_now>
${JSON.stringify(slots.map(), null, 1)}
</window_now>

What the user wants: ${ask}`;

export type DryRun = { error: string | null; rec: { slots: [string, string][]; hidden: string[]; palette: string[]; theme: number; css: number } };

/** A plugin's code run once in a sandboxed frame against a stand-in ctx (tzwidget://localhost/__dryrun/<id>): what it
 * would register, or its error, without it running in the window. */
export function dryRun(id: string): Promise<DryRun> {
  return new Promise((resolve) => {
    const frame = document.createElement("iframe");
    frame.sandbox.add("allow-scripts");
    frame.style.display = "none";
    const finish = (r: DryRun) => { window.removeEventListener("message", on); clearTimeout(timer); frame.remove(); resolve(r); };
    const on = (e: MessageEvent) => {
      if (e.source !== frame.contentWindow || e.data?.type !== "tazuna-dryrun" || e.data.id !== id) return;
      finish({ error: e.data.error ?? null, rec: e.data.rec });
    };
    const timer = setTimeout(() => finish({ error: "it did not finish within 5 s", rec: { slots: [], hidden: [], palette: [], theme: 0, css: 0 } }), 5000);
    window.addEventListener("message", on);
    frame.src = widgetUrl(`__dryrun/${id}`);
    document.body.appendChild(frame);
  });
}

/** What a dry run says the plugin would do, in a line. */
export function summary(r: DryRun["rec"]): string {
  const parts = [
    ...r.slots.map(([slot, id]) => `adds ${id} in ${slot}`), ...r.hidden.map((id) => `hides ${id}`),
    ...(r.palette.length ? [`adds ${r.palette.length} Cmd-K action${r.palette.length === 1 ? "" : "s"}`] : []),
    ...(r.theme ? ["changes design tokens"] : []), ...(r.css ? ["adds styles"] : []),
  ];
  return parts.join("; ");
}
