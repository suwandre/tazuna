// The bridge to tazuna's core (src-tauri/src/main.rs runs `tazuna app-server`). Outside the Tauri window (a plain
// browser preview) there is no core: the app shows its demo data instead.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const inTauri = "__TAURI_INTERNALS__" in window;

export type ViewBlock =
  | { kind: "prompt"; text: string }
  | { kind: "text"; text: string; live: boolean }
  | { kind: "thinking"; tokens: number; live: boolean }
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
};
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
  sessions: (dirs: string[]) => invoke("op", { msg: { op: "sessions", dirs } }),
  resume: (id: string) => invoke("op", { msg: { op: "resume", id } }),
  onSessions: (cb: (s: Sessions) => void) => listen<Sessions>("tazuna-view", (e) => { if ((e.payload as { type?: string }).type === "sessions") cb(e.payload); }),
  send: (text: string, agent = false) => invoke("send", { text, agent }),
  trust: () => invoke("trust"),
  pickFolder: () => invoke<string | null>("pick_folder"),
  interrupt: () => invoke("interrupt"),
  newChat: () => invoke("new_chat"),
  setProfile: (name: string) => invoke("set_profile", { name }),
  models: () => invoke("op", { msg: { op: "models" } }),
  setModel: (c: ModelChoice, effort?: string) => invoke("op", { msg: { op: "model", model: c.model, url: c.url, key_env: c.key_env, effort } }),
  setEffort: (effort: string) => invoke("op", { msg: { op: "effort", effort } }),
  onModels: (cb: (m: Models) => void) => listen<Models>("tazuna-view", (e) => { if ((e.payload as { type?: string }).type === "models") cb(e.payload); }),
  onExit: (cb: () => void) => listen("tazuna-exit", () => cb()),
  onView: (cb: (v: View) => void) => listen<View>("tazuna-view", (e) => { if ((e.payload as { type?: string }).type === "view") cb(e.payload); }),
};
