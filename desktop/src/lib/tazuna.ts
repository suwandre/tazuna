// The bridge to tazuna's core (src-tauri/src/main.rs runs one `tazuna app-server` per open conversation, a "thread").
// Commands go to the thread on screen (`use`); views come from every thread, tagged. Outside the Tauri window (a plain
// browser preview) there is no core: the app shows its demo data instead.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export type SendDraft = { text: string; files?: string[] };

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
  | { kind: "tool"; id: string; name: string; input: Record<string, unknown> | null; result?: string; error: boolean; declined?: boolean; secs?: number; running: boolean; stopped?: boolean }
  | { kind: "done"; ok: boolean; secs: number; turns: number; cost: number; usage: Usage; stopped: boolean;
      /** this reply's own share of the totals (core 0.1.183; before, only the totals); null: no amount can be told (a stopped
       * turn on a model without list prices in the core), and none is in the totals */
      run_cost?: number | null; run_usage?: Usage;
      /** `run_cost` is tazuna's estimate at list prices (nobody reported it: a stopped turn, a hard stop, a crash);
       * `uncounted`: tries the provider may have billed that are not in it (empty replies sent again, timed-out ones) */
      estimate?: boolean; uncounted?: number;
      /** how this turn was paid, as it ran (a receipt takes nothing from the conversation's model of now): the model's id,
       * whether it ran on the Claude plan (not billed), who was paid, and the dollars per million input, cached input and
       * output tokens (a cache write's surcharge is not in them: each call's `cost` holds it). Absent from a core before
       * these and on turns saved before them; absent or null is not known, never false. `providers`: the turn's calls went
       * to more than one (the own loop changed route inside it), in order; `provider` and `price_m` are then null.
       * `sub_calls`: its subagents' model calls, which are in its cost and not among `calls`. */
      model?: string; plan?: boolean; provider?: string | null; price_m?: [number, number, number] | null; providers?: string[]; sub_calls?: number }
  | { kind: "notice"; text: string; error: boolean }
  | { kind: "verify"; cmd: string; ok?: boolean; secs: number; timed_out?: boolean; cannot_run?: boolean; no_tests?: boolean };
type Usage = { input: number; cache_write: number; cache_read: number; output: number };
export type View = {
  blocks: ViewBlock[]; model: string; effort: string; cwd: string; profile: string; mode: string; activity: string;
  /** `context_window` null: the model's window is not known (a local model), and no share of it is shown */
  cost: number; usage: Usage; turns: number; context: number; context_window?: number | null; tasks?: Task[]; limits?: [number, number]; limits_at?: number | null; limits_reset?: number | null; balance?: number | null;
  /** what an untrusted folder's config would run (it runs lean until trusted) */
  untrusted?: string | null;
  /** the conversation's session: claude's id, or the own loop's ("own-...") (null before the first reply, and right after a
   * change of model or a first message's Retry until the new session has its id: never a new conversation then) */
  session_id?: string | null;
  /** every earlier session this conversation goes on from (a change of model, a Retry), oldest first */
  earlier_sessions?: string[];
  /** the oldest open permission question (text made safe by the core) */
  asking?: { tool: string; what: string; note: string; once: boolean; queued: number; reviewing?: boolean } | null;
  /** the turns that changed files: where their card goes (`at_block`, a done block) and what they changed */
  changes?: { turn: number; at_block: number; undone: boolean; can_undo: boolean; files: { path: string; added: number | null; removed: number | null; status: string }[] }[];
  /** the messages "edit from here" can go back to: checkpoint and the message's block */
  edit_points?: { cp: number; block: number }[];
  /** each model call: the block it began at, its tokens (fresh: sent uncached or written into the cache), its cost at the
   * model's prices, its timing (null: in flight or not measured) */
  /** `model` (with `role`, e.g. "explorer", and `provider`, who was paid for it): the call's own model, sent together with a
   * `cost` fixed when the call began; without it `cost` is a rough figure, which the window does not show. `estimate`: the
   * cost is at list prices (claude's calls: its exact total comes with the result alone); else it is what the own loop
   * charged, and a turn's calls add up to its `run_cost` */
  /** `cost` null: not known (the provider reported no usage, or the call was cut off by a stop) */
  calls?: { block: number; fresh: number; cached: number; out: number; cost: number | null; first_ms: number | null; tps: number | null; total_ms: number | null;
    model?: string; role?: string | null; provider?: string | null; estimate?: boolean }[];
};
/** `key`: where its key comes from ("logged in", "key in the environment", …), empty: no key */
export type Provider = { name: string; host: string; page: string; place: string; example: string; key: string; trusted: boolean;
  /** the provider's terms in one line (core 0.1.196 on), empty: none; `warn`: they say the account may be at risk or the
   * provider may refuse tazuna */
  note: string; warn: boolean;
  /** a model server running on this machine (Ollama, LM Studio, llama.cpp, vLLM): it needs no key */
  local?: boolean;
  /** a sign-in instead of a key (core oauth.rs, 2026-11-22): "chatgpt", "copilot", "xai", "gemini", "antigravity"; empty or
   * absent: a key. `key` is "signed in" while signed in; one whose `warn` is set needs that warning confirmed before its
   * first sign-in */
  sign_in?: string;
  /** its warning was confirmed once (it stays through a sign-out) */
  confirmed?: boolean;
  /** signed in to ChatGPT with plan use declined at its consent page: no model until a sign-in allows it */
  plan_off?: boolean;
  /** what tazuna does not know about Mistral's keys (billed per token or by the Vibe plan, a key does not say): "pair", the
   * same key for both, used by neither until the user says which it is on one of the two rows (`key` asks); "old_vibe",
   * that key kept under Mistral Vibe once a new API key replaced it, unused until confirmed or removed */
  unsure?: "pair" | "old_vibe" | null;
  /** a line on what the key may be (MISTRAL_API_KEY in the environment may be a Vibe key) */
  hint?: string | null;
  /** a Gemini CLI account needs its Google Cloud project (its last sign-in said so): the one given so far ("" none) */
  needs_project?: string | null };
/** A sign-in's progress (`ok` null: still going; its text says what to do: a code to type, the browser tab) and then its
 * outcome. `first`: the first sign-in on the ChatGPT plan, whose welcome shows once. */
export type SignIn = { provider: string; ok?: boolean | null; text: string; first?: boolean };
/** Claude Code as the core finds it: its version (null: not installed) and whether it is signed in (`claude_login`; null
 * or absent: not known, a core before 0.1.198). */
export type ClaudeSetup = { claude: string | null; claude_login?: boolean | null };
export type Settings = { providers: Provider[]; data_any: boolean; route: boolean; version: string;
  /** dollars of paid API spend allowed per 24 hours (null: no cap), and what was spent */
  spend_cap?: number | null; spent_24h?: number };
/** MCP servers for every model (the core's mcp.json): on for this conversation or not, the list, a refusal to show */
export type Mcp = { on: boolean; claude: boolean; servers: { name: string; target: string }[]; note: string };
/** a server's check: progress (a login link) without `ok`, then the outcome */
export type McpStatus = { name: string; ok?: boolean; text: string };
export type GalleryItem = { file: string; id: string; kind: "panel" | "plugin"; title: string; about: string; author: string };
export type Hook = { cwd: string; after_turn: string | null; note: string };
export type WorktreeMade = { ask_setup?: boolean; script?: string; id?: string; setup_skipped?: string; error?: string; path?: string; branch?: string; port?: number; copied?: string[]; setup?: { ok: boolean; tail: string } | null };
export type Review = { scope: string; turn?: number | null; base?: string | null; text: string };
export type Diff = { turn: number; path: string; text: string };
export type SessionRow = { id: string; first: string; last_at: number; messages: number;
  /** its latest run's model (core 0.1.178 on; empty for an older core) */
  model?: string;
  /** `first` is a name the user gave it (core 0.1.189 on): shown as it is */
  named?: boolean };
export type Sessions = { dirs: { dir: string; list: SessionRow[] }[] };

/** A row of the catalog. One that can be picked (`why_not` null) has every field; one that cannot is compact (core 0.1.196
 * on, ~3000 rows): its model, provider, price and why_not, its label and bench only when they say something, and `no_key`
 * when its provider has no key yet. */
export type ModelChoice = {
  model: string; provider: string; price: string; why_not: string | null;
  label?: string; place?: string; url?: string | null; key_env?: string | null; efforts?: string[]; effort?: string | null; bench?: string; current?: boolean;
  no_key?: boolean;
};
export type Accounts = {
  plan: { label: string; used: number; resets: string }[] | null; plan_notes: string[];
  credits: { provider: string; left: string | null; page: string }[];
  omniroute: { provider: string; name: string; plan: string | null; status: string; windows: { group: string; window: string; used: number | null; resets: string | null; unlimited: boolean | null }[] }[];
};
export type Models = { choices: ModelChoice[]; rebuild: string | null;
  /** each provider's terms in one line, once (`Provider.note` and `warn`), shown on its group in the picker */
  cautions?: Record<string, { note: string; warn: boolean }>;
  catalogs?: { provider: string; state: "loading" | "ready" | "unavailable" }[] };
/** A background task of claude's (a subagent, a command), as the core's view lists them: `calls` are the calls that
 * started and continued it (a step's id), `secs` how long it took once ended. */
export type Task = { id: string; kind: string; agent: string; description: string; status: string; running: boolean; doing: string; tokens: number; tools: number; calls: string[]; secs: number | null };

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
  sendIn: (tid: string, text: string, agent = true) => invoke("send", { tid, text, agent }),
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
  /** a sign-in (the core opens the browser and waits for it); `confirm`: the user confirmed the provider's warning */
  signIn: (provider: string, confirm = false, project?: string) => to("op", { msg: { op: "login", provider, confirm, ...(project != null ? { project } : {}) } }),
  /** Mistral Vibe's key kept from before (unsure "old_vibe"): taken out */
  removeKey: (provider: string) => to("op", { msg: { op: "login", provider, remove: true } }),
  signOut: (provider: string) => to("op", { msg: { op: "login", provider, sign_out: true } }),
  /** ChatGPT with another account: the session revoked, a new registration */
  signInOther: (provider: string) => to("op", { msg: { op: "login", provider, other_account: true } }),
  onSignIn: (cb: (s: SignIn) => void) => on<SignIn>("signin", cb),
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
  /** the plugin gallery (gallery/ in tazuna's repository): its list; one entry's shared file */
  gallery: () => to("op", { msg: { op: "gallery" } }),
  galleryGet: (file: string) => to("op", { msg: { op: "gallery_get", file } }),
  onGallery: (cb: (g: { items?: GalleryItem[]; error?: string }) => void) => on<{ items?: GalleryItem[]; error?: string }>("gallery", cb),
  onGalleryItem: (cb: (g: { file?: string; text?: string; error?: string }) => void) => on<{ file?: string; text?: string; error?: string }>("gallery_item", cb),
  /** the first run's checks: the claude CLI's version (null: not installed) and whether it is signed in */
  setup: () => to("op", { msg: { op: "setup" } }),
  onSetup: (cb: (s: ClaudeSetup) => void) => on<ClaudeSetup>("setup", cb),
  /** the last failed turn again, without a second message of yours: its error is replaced by the new reply (core C11) */
  retry: () => to("op", { msg: { op: "retry" } }),
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
  /** what each account has left (src/accounts.rs): Claude's plan from claude's own /usage (no model call), the providers'
   * credits, OmniRoute's connected accounts; takes a few seconds */
  accounts: () => to("op", { msg: { op: "accounts" } }),
  /** a subagent's own conversation, by the call that started it (the step's id): answered as an `agent` line */
  agentIn: (tid: string, call: string) => invoke("op", { tid, msg: { op: "agent", call } }),
  onAgent: (cb: (a: { call: string; view: View }, tid: string) => void) => on<{ call: string; view: View }>("agent", cb),
  /** stops one background task; the reply and the other tasks go on */
  taskStopIn: (tid: string, task: string) => invoke("op", { tid, msg: { op: "task_stop", task } }),
  onAccounts: (cb: (a: Accounts) => void) => on<Accounts>("accounts", cb),
  /** the plan alone, a second or so before the rest of an accounts reading (the credits and OmniRoute ask other servers) */
  onAccountsPlan: (cb: (a: Pick<Accounts, "plan" | "plan_notes">) => void) => on<Pick<Accounts, "plan" | "plan_notes">>("accounts_plan", cb),
  send: (text: string, agent = false) => to("send", { text, agent }),
  sendDraft: (tid: string, text: string, draft: SendDraft, agent = false) => invoke("send", { tid, text, draft, agent }),
  onSendRejected: (cb: (m: SendDraft, tid: string) => void) => on<SendDraft>("send_rejected", cb),
  trust: () => to("trust"),
  pickFolder: () => invoke<string | null>("pick_folder"),
  /** the quick-chat shortcut from anywhere ("Alt+Space"; empty: none); fails when another app holds it */
  hotkey: (keys: string) => invoke("hotkey", { keys }),
  pickFiles: () => invoke<string[]>("pick_files"),
  /** what `/` and `@` complete to here: the conversation's commands with what each does, and the project's files */
  complete: () => to("op", { msg: { op: "complete" } }),
  onComplete: (cb: (c: { commands: [string, string][]; files: string[] }, tid: string) => void) => on<{ commands: [string, string][]; files: string[] }>("complete", cb),
  /** an image file as a data URL (the page may not read files): `cwd` for a path an answer gives from its folder */
  imageData: (path: string, cwd?: string) => invoke<string>("image_data", { path, cwd }),
  /** a pasted image's bytes saved as a file (src-tauri paste_image): its path, for the message */
  pasteImage: async (f: File) => invoke<string>("paste_image", new Uint8Array(await f.arrayBuffer()),
    { headers: { "x-ext": (f.type.split("/")[1] ?? "png").replace("jpeg", "jpg") } }),
  pasteClipboardImage: () => invoke<string | null>("paste_clipboard_image"),
  /** files dropped on the window, as paths */
  onDrop: (cb: (paths: string[]) => void) => import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
    getCurrentWebview().onDragDropEvent((e) => { if (e.payload.type === "drop") cb(e.payload.paths); })),
  interrupt: () => to("interrupt"),
  newChat: () => to("new_chat"),
  setProfile: (name: string) => to("set_profile", { name }),
  models: () => to("op", { msg: { op: "models" } }),
  refreshModelList: (provider: string) => to("op", { msg: { op: "refresh_model_list", provider } }),
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
