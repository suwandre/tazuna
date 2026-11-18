// Updates of the app itself (src-tauri update_check, update_install): looked for at start and twice a day; the sidebar
// shows one when there is, and installs it on the user's word.
import { invoke } from "@tauri-apps/api/core";
import { useSyncExternalStore } from "react";
import { inTauri } from "./tazuna";

export type UpdateState = { version?: string; notes?: string; installing?: boolean; error?: string; upToDate?: boolean };
let state: UpdateState = {};
const listeners = new Set<() => void>();
const set = (s: UpdateState) => { state = s; listeners.forEach((l) => l()); };
export const useUpdate = () => useSyncExternalStore((l) => { listeners.add(l); return () => { listeners.delete(l); }; }, () => state);

/** Looks for a newer version; `told` says "up to date" for a while when there is none (a check the user asked for). */
export async function checkUpdate(told = false) {
  if (!inTauri || state.installing) return;
  try {
    const u = await invoke<{ version: string; notes?: string } | null>("update_check");
    if (u) set({ version: u.version, notes: u.notes });
    else if (told) { set({ upToDate: true }); setTimeout(() => state.upToDate && set({}), 4000); }
  } catch (e) {
    if (told) set({ error: `Could not check for updates: ${e}` });
    else console.error(`update check: ${e}`);
  }
}

/** Downloads, checks the signature, installs and restarts (the conversations' cores stop; they stay resumable). */
export async function installUpdate() {
  set({ ...state, installing: true, error: undefined });
  try { await invoke("update_install"); } catch (e) { set({ ...state, installing: false, error: `The update failed: ${e}` }); }
}

export function watchUpdates() {
  if (!inTauri) return () => {};
  const first = setTimeout(() => checkUpdate(), 5000);
  // hourly (12-hourly until 0.1.55: a fix waited half a day to arrive); one small request to GitHub each time
  const every = setInterval(() => checkUpdate(), 3600 * 1000);
  return () => { clearTimeout(first); clearInterval(every); };
}
