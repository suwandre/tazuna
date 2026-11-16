// The bridge to tazuna's core (src-tauri/src/main.rs runs `tazuna app-server`). Outside the Tauri window (a plain
// browser preview) there is no core: the app shows its demo data instead.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const inTauri = "__TAURI_INTERNALS__" in window;

export type ViewBlock =
  | { kind: "prompt"; text: string }
  | { kind: "text"; text: string; live: boolean }
  | { kind: "thinking"; tokens: number; live: boolean; text?: string }
  | { kind: "tool"; id: string; name: string; input: Record<string, unknown> | null; result?: string; error: boolean; secs?: number; running: boolean }
  | { kind: "done"; ok: boolean; secs: number; turns: number; cost: number; usage: Usage; stopped: boolean }
  | { kind: "notice"; text: string; error: boolean }
  | { kind: "verify"; cmd: string; ok?: boolean; secs: number };
type Usage = { input: number; cache_write: number; cache_read: number; output: number };
export type View = {
  blocks: ViewBlock[]; model: string; effort: string; cwd: string; profile: string; mode: string; activity: string;
  cost: number; usage: Usage; turns: number; context: number; context_window?: number; limits?: [number, number]; balance?: number;
  /** what an untrusted folder's config would run (it runs lean until trusted) */
  untrusted?: string | null;
  /** the claude session of this conversation (null before the first reply, and on the own loop) */
  session_id?: string | null;
  /** the oldest open permission question (text made safe by the core) */
  asking?: { tool: string; what: string; note: string; once: boolean; queued: number } | null;
  /** the turns that changed files: where their card goes (`at_block`, a done block) and what they changed */
  changes?: { turn: number; at_block: number; undone: boolean; can_undo: boolean; files: { path: string; added: number | null; removed: number | null; status: string }[] }[];
};
export type Provider = { name: string; host: string; page: string; place: string; example: string; key: string; trusted: boolean };
export type Settings = { providers: Provider[]; data_any: boolean; route: boolean; version: string };
export type Diff = { turn: number; path: string; text: string };
export type SessionRow = { id: string; first: string; last_at: number; messages: number };
export type Sessions = { dirs: { dir: string; list: SessionRow[] }[] };

export type ModelChoice = {
  model: string; label: string; place: string; provider: string; url: string | null; key_env: string | null;
  efforts: string[]; effort: string | null; price: string; bench: string; why_not: string | null; current: boolean;
};
export type Models = { choices: ModelChoice[]; rebuild: string | null };

export const core = {
  /** resolves to the folder the core runs in */
  start: (cwd?: string, profile?: string) => invoke<string>("start", { cwd, profile }),
  setMode: (mode: string) => invoke("op", { msg: { op: "mode", mode } }),
  answer: (allow: boolean, always = false) => invoke("op", { msg: { op: "answer", allow, always } }),
  diff: (turn: number, path: string) => invoke("op", { msg: { op: "diff", turn, path } }),
  undo: (turn: number) => invoke("op", { msg: { op: "undo", turn } }),
  onDiff: (cb: (d: Diff) => void) => listen<Diff>("tazuna-view", (e) => { if ((e.payload as { type?: string }).type === "diff") cb(e.payload); }),
  settings: () => invoke("op", { msg: { op: "settings" } }),
  login: (provider: string, key: string) => invoke("op", { msg: { op: "login", provider, key } }),
  setData: (policy: "trusted" | "any", confirm = false) => invoke("op", { msg: { op: "data", policy, confirm } }),
  setRoute: (on: boolean) => invoke("op", { msg: { op: "route", on } }),
  openKeyPage: (url: string) => invoke("open_key_page", { url }),
  onSettings: (cb: (s: Settings) => void) => listen<Settings>("tazuna-view", (e) => { if ((e.payload as { type?: string }).type === "settings") cb(e.payload); }),
  sessions: (dirs: string[]) => invoke("op", { msg: { op: "sessions", dirs } }),
  resume: (id: string) => invoke("op", { msg: { op: "resume", id } }),
  onSessions: (cb: (s: Sessions) => void) => listen<Sessions>("tazuna-view", (e) => { if ((e.payload as { type?: string }).type === "sessions") cb(e.payload); }),
  send: (text: string, agent = false) => invoke("send", { text, agent }),
  trust: () => invoke("trust"),
  pickFolder: () => invoke<string | null>("pick_folder"),
  pickFiles: () => invoke<string[]>("pick_files"),
  /** files dropped on the window, as paths */
  onDrop: (cb: (paths: string[]) => void) => import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
    getCurrentWebview().onDragDropEvent((e) => { if (e.payload.type === "drop") cb(e.payload.paths); })),
  interrupt: () => invoke("interrupt"),
  newChat: () => invoke("new_chat"),
  setProfile: (name: string) => invoke("set_profile", { name }),
  models: () => invoke("op", { msg: { op: "models" } }),
  setModel: (c: ModelChoice, effort?: string) => invoke("op", { msg: { op: "model", model: c.model, url: c.url, key_env: c.key_env, effort } }),
  setEffort: (effort: string) => invoke("op", { msg: { op: "effort", effort } }),
  onModels: (cb: (m: Models) => void) => listen<Models>("tazuna-view", (e) => { if ((e.payload as { type?: string }).type === "models") cb(e.payload); }),
  /** a menu bar item (macOS): new-chat, open-folder, palette, shortcuts, theme */
  onMenu: (cb: (id: string) => void) => listen<string>("menu", (e) => cb(e.payload)),
  onExit: (cb: () => void) => listen("tazuna-exit", () => cb()),
  onView: (cb: (v: View) => void) => listen<View>("tazuna-view", (e) => { if ((e.payload as { type?: string }).type === "view") cb(e.payload); }),
};

/** A path as the core reads it in a message: quoted when it has a space (src/live.rs words). */
export const quotePath = (p: string) => (/\s/.test(p) ? `"${p}"` : p);

/** On macOS in the app the menu bar owns the shortcuts (its items send `menu`); elsewhere the window's key handlers do. */
export const menuKeys = inTauri && navigator.userAgent.includes("Mac");
