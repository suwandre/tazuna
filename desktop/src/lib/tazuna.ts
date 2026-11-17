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
  /** the conversation's session: claude's id, or the own loop's ("own-...") (null before the first reply) */
  session_id?: string | null;
  /** the oldest open permission question (text made safe by the core) */
  asking?: { tool: string; what: string; note: string; once: boolean; queued: number } | null;
  /** the turns that changed files: where their card goes (`at_block`, a done block) and what they changed */
  changes?: { turn: number; at_block: number; undone: boolean; can_undo: boolean; files: { path: string; added: number | null; removed: number | null; status: string }[] }[];
  /** the messages "edit from here" can go back to: checkpoint and the message's block */
  edit_points?: { cp: number; block: number }[];
  /** each model call: the block it began at, its tokens (fresh: sent uncached or written into the cache), its cost at the
   * model's prices, its timing (null: in flight or not measured) */
  calls?: { block: number; fresh: number; cached: number; out: number; cost: number; first_ms: number | null; tps: number | null; total_ms: number | null }[];
};
export type Provider = { name: string; host: string; page: string; place: string; example: string; key: string; trusted: boolean };
export type Settings = { providers: Provider[]; data_any: boolean; route: boolean; version: string;
  /** dollars of paid API spend allowed per 24 hours (null: no cap), and what was spent */
  spend_cap?: number | null; spent_24h?: number };
/** MCP servers for every model (the core's mcp.json): on for this conversation or not, the list, a refusal to show */
export type Mcp = { on: boolean; claude: boolean; servers: { name: string; target: string }[]; note: string };
/** a server's check: progress (a login link) without `ok`, then the outcome */
export type McpStatus = { name: string; ok?: boolean; text: string };
export type Hook = { cwd: string; after_turn: string | null; note: string };
export type WorktreeMade = { ask_setup?: boolean; script?: string; id?: string; setup_skipped?: string; error?: string; path?: string; branch?: string; port?: number; copied?: string[]; setup?: { ok: boolean; tail: string } | null };
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
  /** any of the window's ops to thread `tid` (the side chat's) */
  opIn: (tid: string, msg: Record<string, unknown>) => invoke("op", { tid, msg }),
  interruptIn: (tid: string) => invoke("interrupt", { tid }),
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
  /** a new worktree of this project (src/worktree.rs); runSetup: the user's answer about .tazuna/setup.sh (null: ask) */
  worktreeNew: (runSetup: boolean | null, id?: string) => to("op", { msg: { op: "worktree_new", run_setup: runSetup, id } }),
  /** one of tazuna's worktrees removed (its branch stays); force: with uncommitted changes */
  worktreeRemove: (path: string, force = false) => to("op", { msg: { op: "worktree_remove", path, force } }),
  onWorktree: (cb: (w: WorktreeMade, tid: string) => void) => on<WorktreeMade>("worktree", cb),
  onWorktreeRemoved: (cb: (w: { path: string; error?: string }, tid: string) => void) => on<{ path: string; error?: string }>("worktree_removed", cb),
  /** /create for behaviour (src/create.rs): a command, skill or tool made in thread `tid`'s fresh conversation; enable or
   * discard what it made */
  createIn: (tid: string, ask: string) => invoke("op", { tid, msg: { op: "create", ask } }),
  createDoneIn: (tid: string, enable: boolean) => invoke("op", { tid, msg: { op: enable ? "create_enable" : "create_discard" } }),
  onCreation: (cb: (c: { state: string; dir?: string }, tid: string) => void) => on<{ state: string; dir?: string }>("creation", cb),
  /** the pull request of thread `tid`'s branch (gh) */
  prIn: (tid: string) => invoke("op", { tid, msg: { op: "pr" } }),
  onPr: (cb: (p: import("./pr").Pr, tid: string) => void) => on<import("./pr").Pr>("pr", cb),
  /** the cap on paid API spend per 24 hours (null: none) */
  setSpendCap: (value: number | null) => to("op", { msg: { op: "spend_cap", value } }),
  /** MCP servers (src/mcpc.rs), as the terminal's /mcp: list, on or off, add (a URL, or a command and its arguments), remove,
   * check (login: log in again first) */
  mcp: () => to("op", { msg: { op: "mcp" } }),
  mcpOn: (on: boolean) => to("op", { msg: { op: "mcp_on", on } }),
  mcpAdd: (name: string, target: string) => to("op", { msg: { op: "mcp_add", name, target } }),
  mcpRemove: (name: string) => to("op", { msg: { op: "mcp_remove", name } }),
  mcpCheck: (name: string, login = false) => to("op", { msg: { op: "mcp_check", name, login } }),
  onMcp: (cb: (m: Mcp) => void) => on<Mcp>("mcp", cb),
  onMcpStatus: (cb: (m: McpStatus) => void) => on<McpStatus>("mcp_status", cb),
  /** this folder's command after each turn that changed files (src/hooks.rs): set with a string (empty: none), asked without */
  hook: (afterTurn?: string) => to("op", { msg: { op: "hook", after_turn: afterTurn } }),
  onHook: (cb: (h: Hook) => void) => on<Hook>("hook", cb),
  /** the first run's checks: the claude CLI's version, or null when it is not installed */
  setup: () => to("op", { msg: { op: "setup" } }),
  onSetup: (cb: (s: { claude: string | null }) => void) => on<{ claude: string | null }>("setup", cb),
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
  /** the run log's last 90 days for the usage page (src/lib/usage.ts reads the rows) */
  usage: (signals = false) => to("op", { msg: { op: "usage", signals } }),
  onUsage: (cb: (u: { rows: import("./usage").Row[] }) => void) => on<{ rows: import("./usage").Row[] }>("usage", cb),
  send: (text: string, agent = false) => to("send", { text, agent }),
  trust: () => to("trust"),
  pickFolder: () => invoke<string | null>("pick_folder"),
  /** the quick-chat shortcut from anywhere ("Alt+Space"; empty: none); fails when another app holds it */
  hotkey: (keys: string) => invoke("hotkey", { keys }),
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
    const blocks = new Map<string, ViewBlock[]>(), calls = new Map<string, NonNullable<View["calls"]>>();
    return on<View & { from?: number; calls_from?: number }>("view", (p, tid) => {
      const old = blocks.get(tid) ?? [], from = p.from ?? 0, oldCalls = calls.get(tid) ?? [], callsFrom = p.calls_from ?? 0;
      if (from > old.length || callsFrom > oldCalls.length) { invoke("op", { tid, msg: { op: "full" } }); return; }
      const merged = old.slice(0, from).concat(p.blocks), mergedCalls = oldCalls.slice(0, callsFrom).concat(p.calls ?? []);
      blocks.set(tid, merged);
      calls.set(tid, mergedCalls);
      cb({ ...p, blocks: merged, calls: mergedCalls }, tid);
    });
  },
};

/** A path as the core reads it in a message: quoted when it has a space (src/live.rs words). */
export const quotePath = (p: string) => (/\s/.test(p) ? `"${p}"` : p);

/** The quick-chat shortcut from anywhere, as Settings saved it (empty: none). */
export const HOTKEY = "tazuna.hotkey";
export const defaultHotkey = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac") ? "Alt+Space" : "Ctrl+Alt+Space";

/** On macOS in the app the menu bar owns the shortcuts (its items send `menu`); elsewhere the window's key handlers do. */
export const menuKeys = inTauri && navigator.userAgent.includes("Mac");
