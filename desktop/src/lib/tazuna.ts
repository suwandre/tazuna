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
};

export const core = {
  start: (cwd?: string, profile?: string) => invoke("start", { cwd, profile }),
  send: (text: string) => invoke("send", { text }),
  interrupt: () => invoke("interrupt"),
  newChat: () => invoke("new_chat"),
  setProfile: (name: string) => invoke("set_profile", { name }),
  onExit: (cb: () => void) => listen("tazuna-exit", () => cb()),
  onView: (cb: (v: View) => void) => listen<View>("tazuna-view", (e) => { if ((e.payload as { type?: string }).type === "view") cb(e.payload); }),
};
