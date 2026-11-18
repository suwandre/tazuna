// Updates of the app itself (src-tauri update_check, update_install): looked for at start and hourly; a snackbar says
// when there is one and installs it on the user's word (Settings has the button too).
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { useSyncExternalStore } from "react";
import { inTauri } from "./tazuna";
import { toast } from "./toast";

export type UpdateState = { version?: string; notes?: string; installing?: boolean;
  /** while installing: the bytes downloaded and the whole (when the server says it), then "installing" */
  got?: number; total?: number | null; stage?: "downloading" | "installing" };
let state: UpdateState = {};
// the version a snackbar has said is out (said once, not at every hourly check)
let announced = "";
const listeners = new Set<() => void>();
const set = (s: UpdateState) => { state = s; listeners.forEach((l) => l()); };
export const useUpdate = () => useSyncExternalStore((l) => { listeners.add(l); return () => { listeners.delete(l); }; }, () => state);

/** Looks for a newer version; `told` says "up to date" for a while when there is none (a check the user asked for). */
export async function checkUpdate(told = false) {
  if (!inTauri || state.installing) return;
  try {
    const u = await invoke<{ version: string; notes?: string } | null>("update_check");
    if (u) set({ version: u.version, notes: u.notes });
    // the snackbar says it, and only it (the sidebar's row and Settings' line went in 0.1.60): a new version once, staying
    // until installed or closed; a check the user asked for also when there is none
    // asked for (Check for updates): the answer in the middle, over a dimmed window, until closed; found by the hourly
    // check: a snackbar at the bottom, which does not stop the work
    const now = told ? await getVersion().catch(() => "") : "";
    if (u && (told || announced !== u.version)) {
      announced = u.version;
      const notes = u.notes && u.notes !== `tazuna desktop ${u.version}` ? `${u.notes}\n\n` : "";
      toast(`tazuna desktop ${u.version} is out`, { kind: "info", sticky: true, modal: told, action: { label: "Install and restart", run: () => installUpdate() },
        detail: `${notes}You have ${now || "an older version"}. Installing restarts tazuna: replies running now stop, conversations stay resumable.` });
    } else if (!u && told) toast("You're up to date", { kind: "ok", modal: true, detail: `tazuna desktop ${now} is the newest version.` });
  } catch (e) {
    if (told) toast("Could not check for updates", { kind: "error", modal: true, detail: String(e) });
    else console.error(`update check: ${e}`);
  }
}

/** Downloads, checks the signature, installs and restarts (the conversations' cores stop; they stay resumable). The
 * download's progress comes as "update-progress" events and shows at the bottom until the restart (Toaster): GitHub's
 * release storage served the 6 MB at 80-200 KB/s on 2026-11-18, and "Installing…" alone read as stuck for a minute. */
export async function installUpdate() {
  set({ ...state, installing: true, stage: "downloading", got: 0, total: null });
  const { listen } = await import("@tauri-apps/api/event");
  const off = await listen<{ got?: number; total?: number | null; installing?: boolean }>("update-progress", (e) =>
    set({ ...state, ...(e.payload.installing ? { stage: "installing" } : { got: e.payload.got, total: e.payload.total }) }));
  try { await invoke("update_install"); }
  catch (e) { set({ ...state, installing: false, stage: undefined }); toast(`The update failed: ${e}`, { kind: "error" }); }
  finally { off(); }
}

/** "2.1 of 6.0 MB" */
export const progressText = (s: UpdateState) => {
  const mb = (n: number) => (n / 1e6).toFixed(1);
  return s.stage === "installing" ? "Installing… tazuna restarts by itself"
    : `Downloading${s.version ? ` ${s.version}` : ""} · ${mb(s.got ?? 0)}${s.total ? ` of ${mb(s.total)}` : ""} MB`;
};

export function watchUpdates() {
  if (!inTauri) return () => {};
  const first = setTimeout(() => checkUpdate(), 5000);
  // hourly (12-hourly until 0.1.55: a fix waited half a day to arrive); one small request to GitHub each time
  const every = setInterval(() => checkUpdate(), 3600 * 1000);
  return () => { clearTimeout(first); clearInterval(every); };
}
