// The bridge to tazuna's core (src-tauri/src/main.rs runs one `tazuna app-server` per open conversation, a "thread").
// Commands go to the thread on screen (`use`); views come from every thread, tagged. Outside the Tauri window (a plain
// browser preview) there is no core: the app shows its demo data instead.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** The thread commands go to: the one on screen. */
let current = "";
const to = <T = unknown>(cmd: string, args: Record<string, unknown> = {}) => invoke<T>(cmd, { tid: current, ...args });
/** A message from a thread's core, unwrapped: `type` says what it is (view, models, diff, settings, sessions). */
type Tagged<T> = { tid: string; msg: T & { type?: string } };
const on = <T,>(type: string, cb: (m: T, tid: string) => void) =>
  listen<Tagged<T>>("tazuna-view", (e) => { if (e.payload.msg.type === type) cb(e.payload.msg, e.payload.tid); });

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
  cost: number; usage: Usage; turns: number; context: number; context_window?: number; limits?: [number, number]; limits_reset?: number | null; balance?: number | null;
  /** what an untrusted folder's config would run (it runs lean until trusted) */
  untrusted?: string | null;
  /** the claude session of this conversation (null before the first reply, and on the own loop) */
  session_id?: string | null;
  /** the oldest open permission question (text made safe by the core) */
  asking?: { tool: string; what: string; note: string; once: boolean; queued: number } | null;
  /** the turns that changed files: where their card goes (`at_block`, a done block) and what they changed */
  changes?: { turn: number; at_block: number; undone: boolean; can_undo: boolean; files: { path: string; added: number | null; removed: number | null; status: string }[] }[];
  /** the messages "edit from here" can go back to: checkpoint and the message's block */
  edit_points?: { cp: number; block: number }[];
};
export type Provider = { name: string; host: string; page: string; place: string; example: string; key: string; trusted: boolean };
export type Settings = { providers: Provider[]; data_any: boolean; route: boolean; version: string };
export type Review = { scope: string; turn?: number | null; base?: string | null; text: string };
export type Diff = { turn: number; path: string; text: string };
export type SessionRow = { id: string; first: string; last_at: number; messages: number };
export type Sessions = { dirs: { dir: string; list: SessionRow[] }[] };

export type ModelChoice = {
  model: string; label: string; place: string; provider: string; url: string | null; key_env: string | null;
  efforts: string[]; effort: string | null; price: string; bench: string; why_not: string | null; current: boolean;
};
export type Models = { choices: ModelChoice[]; rebuild: string | null };

export const core = {
  /** Commands from now on go to thread `tid`. */
  use: (tid: string) => { current = tid; },
  /** Starts thread `tid`'s core; resolves to the folder it runs in. */
  start: (tid: string, cwd?: string, profile?: string) => invoke<string>("start", { tid, cwd, profile }),
  /** Ends thread `tid`'s core. */
  stop: (tid: string) => invoke("stop", { tid }),
  /** Resumes a conversation in thread `tid` (right after its start, before it is the current one). */
  resumeIn: (tid: string, id: string) => invoke("op", { tid, msg: { op: "resume", id } }),
  /** A message to thread `tid` whether or not it is on screen (creator mode's first message). */
  sendIn: (tid: string, text: string) => invoke("send", { tid, text, agent: true }),
  modeIn: (tid: string, mode: string) => invoke("op", { tid, msg: { op: "mode", mode } }),
  /** creator mode: writes inside this folder (a panel's or plugin's own) need no question; the core checks the real path */
  allowRootIn: (tid: string, path: string) => invoke("op", { tid, msg: { op: "allow_root", path } }),
  setMode: (mode: string) => to("op", { msg: { op: "mode", mode } }),
  answer: (allow: boolean, always = false) => to("op", { msg: { op: "answer", allow, always } }),
  diff: (turn: number, path: string) => to("op", { msg: { op: "diff", turn, path } }),
  undo: (turn: number) => to("op", { msg: { op: "undo", turn } }),
  /** the conversation back to before checkpoint `cp` (and with `files`, what tazuna changed since) */
  rewind: (cp: number, files: boolean) => to("op", { msg: { op: "rewind", cp, files } }),
  /** a diff to review: "uncommitted", "branch", or one "turn" */
  review: (scope: string, turn?: number) => to("op", { msg: { op: "review", scope, turn } }),
  /** one piece (a file's header and one hunk) reverted in the work tree, or staged */
  piece: (patch: string, stage: boolean) => to("op", { msg: { op: "piece", patch, stage } }),
  onReview: (cb: (r: Review) => void) => on<Review>("review", cb),
  onDiff: (cb: (d: Diff) => void) => on<Diff>("diff", cb),
  settings: () => to("op", { msg: { op: "settings" } }),
  login: (provider: string, key: string) => to("op", { msg: { op: "login", provider, key } }),
  setData: (policy: "trusted" | "any", confirm = false) => to("op", { msg: { op: "data", policy, confirm } }),
  setRoute: (on: boolean) => to("op", { msg: { op: "route", on } }),
  openKeyPage: (url: string) => invoke("open_key_page", { url }),
  /** a system notification */
  notify: async (title: string, body: string) => {
    // the system asks once whether tazuna may notify; until then nothing would show
    const granted = await invoke<boolean | null>("plugin:notification|is_permission_granted").catch(() => null);
    if (granted === false) await invoke("plugin:notification|request_permission").catch(() => {});
    return invoke("notify", { title, body });
  },
  /** a link in an answer, in the browser (http and https only) */
  openLink: (url: string) => invoke("open_link", { url }),
  onSettings: (cb: (s: Settings) => void) => on<Settings>("settings", cb),
  rename: (id: string, title: string) => to("op", { msg: { op: "rename", id, title } }),
  hide: (id: string) => to("op", { msg: { op: "hide", id } }),
  sessions: (dirs: string[]) => to("op", { msg: { op: "sessions", dirs } }),
  resume: (id: string) => to("op", { msg: { op: "resume", id } }),
  onSessions: (cb: (s: Sessions) => void) => on<Sessions>("sessions", cb),
  send: (text: string, agent = false) => to("send", { text, agent }),
  trust: () => to("trust"),
  pickFolder: () => invoke<string | null>("pick_folder"),
  pickFiles: () => invoke<string[]>("pick_files"),
  /** files dropped on the window, as paths */
  onDrop: (cb: (paths: string[]) => void) => import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
    getCurrentWebview().onDragDropEvent((e) => { if (e.payload.type === "drop") cb(e.payload.paths); })),
  interrupt: () => to("interrupt"),
  newChat: () => to("new_chat"),
  setProfile: (name: string) => to("set_profile", { name }),
  models: () => to("op", { msg: { op: "models" } }),
  setModel: (c: ModelChoice, effort?: string) => to("op", { msg: { op: "model", model: c.model, url: c.url, key_env: c.key_env, effort } }),
  setEffort: (effort: string) => to("op", { msg: { op: "effort", effort } }),
  onModels: (cb: (m: Models, tid: string) => void) => on<Models>("models", cb),
  /** a menu bar item (macOS): new-chat, open-folder, palette, shortcuts, theme */
  onMenu: (cb: (id: string) => void) => listen<string>("menu", (e) => cb(e.payload)),
  onExit: (cb: (tid: string) => void) => listen<string>("tazuna-exit", (e) => cb(e.payload)),
  /** Each thread's session as its core sends it: a view carries the blocks from `from` on (the ones that changed), merged
   * here with that thread's blocks from before. */
  onView: (cb: (v: View, tid: string) => void) => {
    const blocks = new Map<string, ViewBlock[]>();
    return on<View & { from?: number }>("view", (p, tid) => {
      const old = blocks.get(tid) ?? [], from = p.from ?? 0;
      if (from > old.length) { invoke("op", { tid, msg: { op: "full" } }); return; }
      const merged = old.slice(0, from).concat(p.blocks);
      blocks.set(tid, merged);
      cb({ ...p, blocks: merged }, tid);
    });
  },
};

/** A path as the core reads it in a message: quoted when it has a space (src/live.rs words). */
export const quotePath = (p: string) => (/\s/.test(p) ? `"${p}"` : p);

/** On macOS in the app the menu bar owns the shortcuts (its items send `menu`); elsewhere the window's key handlers do. */
export const menuKeys = inTauri && navigator.userAgent.includes("Mac");
