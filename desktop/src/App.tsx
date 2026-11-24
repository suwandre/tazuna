import { createContext, useContext, useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { GitBranch, LoaderCircle, MessagesSquare, PanelLeft, PanelRight } from "lucide-react";
import { AgentPanel } from "./components/AgentPanel";
import { Approval } from "./components/Approval";
import { Composer, DRAFT, MODES, type Completions } from "./components/Composer";
import { DiffView } from "./components/DiffView";
import { ReviewView } from "./components/ReviewView";
import { Palette, Shortcuts, type Action } from "./components/Palette";
import { creatorMessage, dryRun, summary, widgetData, widgetId, widgets, type UserWidget } from "./lib/widgets";
import { footprint, load, onPaletteChange, pluginActions, safeMode, setActions, setData, unload } from "./lib/plugins";
import { SettingsView, TAB } from "./components/SettingsView";
import { SideChat } from "./components/SideChat";
import { UsageView } from "./components/UsageView";
import { PrView } from "./components/PrView";
import { BestOfStart, LanesView, type Lane, type Lanes } from "./components/BestOf";
import { Toaster } from "./components/Toaster";
import { SETUP, Setup } from "./components/Setup";
import { prChip, type Pr } from "./lib/pr";
import { checkUpdate, watchUpdates } from "./lib/update";
import { routeLine } from "./lib/route";
import { today as todayOf, type Row } from "./lib/usage";
import { usd, type ChangedFile, type Message, escFor, keysOf, panelChoice, panelState } from "./lib/types";
import { Sep } from "./components/Sep";
import type { Turn } from "./components/Receipt";
import { Rail, UserPanel, WorkCtx, openUsage, type Work } from "./components/Rail";
import { Slot, slots } from "./lib/slots";
import { FOCUSABLE } from "./lib/dialog";
import { Sidebar, macOverlay } from "./components/Sidebar";
import { Transcript } from "./components/Transcript";
import { CoreFailed, Start, type StartItem } from "./components/Start";
import { StatusBar } from "./components/StatusBar";
import { getVersion } from "@tauri-apps/api/app";
import * as demo from "./lib/demo";
import { askedInput, changedCount, heldTurn, messages as viewMessages, queued, readableError, resetIn, sessionIds, unsaved, usage as viewUsage } from "./lib/fromView";
import { modelLabel, shortTitle, titleFrom, type Usage } from "./lib/types";
import { items as accountItems, mine as accountOf, withLive } from "./lib/health";
import { Parts } from "./components/Sep";

/** What is open over the window or the box that Esc should shut before the work panel over the conversation: menus and
 * popovers (`tz-pop`; a closed one's fading copy is `tz-out`), the completion menu, dialogs. */
const MENUS = ".tz-pop:not([data-panel]):not(.tz-out), [role=menu], [role=listbox], [role=dialog]:not(.tz-out), dialog[open]";

const NO_USAGE: Usage = { contextPct: 0, contextUsed: "0", contextMax: "", plan5h: null, plan5hReset: "", planWeek: null, threadCost: 0, threadTurns: 0, cachedPct: 0, saved: 0, today: 0, todayTurns: 0, todaySeries: [], balance: null };
import { THEME, applyLooks, darkNow, look } from "./lib/chrome";
import { HOTKEY, core, defaultHotkey, inTauri, menuKeys, type Accounts, type ClaudeSetup, type Diff, type ModelChoice, type Models, type SessionRow, type View } from "./lib/tazuna";
import { listen } from "@tauri-apps/api/event";
import { accentButton, quietButton } from "./components/Controls";

const RECENT = "tazuna.projects";
const recent = (): string[] => { try { return JSON.parse(localStorage.getItem(RECENT) ?? "[]"); } catch { return []; } };
// messages queued behind a reply, per conversation, until the core takes them: after a restart they come back in the box
const QUEUE = "tazuna.queued.";
// the model new conversations of a kind start on ("chat", "project"): a non-Claude one the user picked by hand, until they
// pick a Claude model again (Claude's stay the router's choice per task). A new chat started on Opus 5.5 right after the user
// had picked another model, and failed while Claude was signed out (design review 12).
const START_MODEL = "tazuna.start-model.";
// With Claude signed out, a kind with no pick of its own takes the other kind's: the first project after a chat on another
// model started on Sonnet and failed (review 12's own check).
let claudeSignedOut = false;  // the last setup reading said claude is signed out (set in App)
const startModel = (folder: string | null): Pick<ModelChoice, "model" | "url" | "key_env"> | null => {
  const read = (kind: string) => { try { return JSON.parse(localStorage.getItem(START_MODEL + kind) ?? "null"); } catch { return null; } };
  return read(folder ? "project" : "chat") ?? (claudeSignedOut ? read(folder ? "chat" : "project") : null);
};

/** An open conversation: its own core, folder (null: a chat) and latest view. */
export type Thread = { tid: string; folder: string | null; view: View | null; error: string | null; used: number;
  /** the saved conversation it continues */
  resume?: string;
  /** a side chat: the thread it sits beside (not listed, closed with it) */
  side?: string;
  /** creator mode: this conversation builds ("new") or changes ("edit") a panel in its folder */
  creator?: "new" | "edit" };
const busyView = (v: View | null) => !!v && !["Ready", "Done"].includes(v.activity);
// idle open conversations kept running (each holds a claude process): more close, oldest first; they stay resumable
const KEEP_OPEN = 4;

/** The open conversations (each its own core, so one can keep working while another is on screen) and the one on
 * screen. Outside the Tauri window there is no core (the demo data shows instead). */
function useCore(onDraftRejected: (text: string) => void) {
  const [threads, setThreads] = useState<Record<string, Thread>>({});
  const [active, setActiveTid] = useState("");
  const [projects, setProjects] = useState<string[]>(recent);
  const [models, setModels] = useState<Models | null>(null);
  const [diff, setDiff] = useState<Diff | null>(null);
  // saved conversations per folder (the home folder's are the chats), as the terminal's /resume lists them
  const [sessions, setSessions] = useState<Record<string, SessionRow[]>>({});
  const [home, setHome] = useState<string | null>(null);
  const live = useRef<Record<string, Thread>>({});
  live.current = threads;
  // each conversation's earlier sessions, newest first: a change of model and a Retry go on in a new session (core C5), and
  // the saved lists know it by an earlier id until a turn has run in the new one. It stays one conversation: one row, the
  // one selected, the one a click on that row shows (not a second copy)
  const earlier = useRef<Record<string, string[]>>({});
  const idsOf = (t: Thread) => sessionIds(t.view, earlier.current[t.tid]);
  const draftKeyOf = (t: Thread) => idsOf(t)[0] ?? t.resume ?? `new:${t.folder ?? "chat"}`;
  // the window's queue: when its last message went (and how many of yours the view had then), and the wait set for the next
  const sent = useRef<Record<string, { prompts: number; at: number }>>({});
  const waits = useRef<Record<string, number>>({});
  const activeRef = useRef("");
  activeRef.current = active;
  const restoreDraft = (tid: string, text: string) => {
    const t = live.current[tid];
    if (!t) return;
    if (tid === activeRef.current) onDraftRejected(text);
    else {
      const key = DRAFT + draftKeyOf(t);
      localStorage.setItem(key, [localStorage.getItem(key), text].filter(Boolean).join("\n\n"));
    }
  };
  const patch = (tid: string, p: Partial<Thread>) => setThreads((t) => (t[tid] ? { ...t, [tid]: { ...t[tid], ...p } } : t));
  const show = (tid: string) => { core.use(tid); setActiveTid(tid); setModels(null); patch(tid, { used: Date.now() }); };
  // a conversation out of sight that finishes or asks for an answer says so with a system notification
  const was = useRef<Record<string, string>>({});
  const tell = (tid: string, v: View) => {
    if (live.current[tid]?.side) return;
    const now = v.asking ? "asking" : busyView(v) ? "working" : "idle", before = was.current[tid];
    was.current[tid] = now;
    if (now === before || (tid === activeRef.current && document.hasFocus())) return;
    const title = shortTitle((v.blocks.find((b) => b.kind === "prompt") as { text?: string } | undefined)?.text) || "A conversation";
    if (now === "asking") core.notify("tazuna asks", `${title}: ${v.asking!.tool.replace(/^mcp__lean__/, "")} ${v.asking!.what}`).catch(() => {});
    else if (now === "idle" && before === "working") core.notify("tazuna is done", title).catch(() => {});
  };
  // the queue as it stands, saved while it holds anything; dropped once it empties in this window (not on a first view)
  const queuedN = useRef<Record<string, number>>({});
  // messages sent while a reply runs wait here, in the window, and go on one at a time, each when the reply before it has
  // ended: Stop puts all that wait back in the box. Sent to the core at once, they went to the agent the moment the reply
  // was stopped (design review 12). Every message of yours waits the same way, from the box or not (review comments, the
  // work panel's and the pull request's asks: `agent`, past the router).
  const [held, setHeld] = useState<Record<string, { text: string; agent: boolean }[]>>({});
  const heldRef = useRef(held);
  heldRef.current = held;
  const hold = (tid: string, text: string, agent = false) => setHeld((m) => ({ ...m, [tid]: [...(m[tid] ?? []), { text, agent }] }));
  /** A thread's held messages, taken out (Stop: back into its box). */
  const unhold = (tid: string) => { const q = heldRef.current[tid] ?? []; setHeld(({ [tid]: _, ...m }) => m); return q.map((w) => w.text); };
  /** The queue as it stands (the core's and the window's), kept under the conversation's id while it holds anything: after
   * a restart it comes back in the box. From each view and whenever the window's queue changes (a long quiet command sends
   * no view, and a message queued then the app quit was lost). */
  const keepQueue = (tid: string, v: View | null) => {
    const sid = sessionIds(v, earlier.current[tid])[0];
    if (!sid || !v) return;
    const q = [...(busyView(v) ? queued(v) : []), ...(heldRef.current[tid] ?? []).map((w) => w.text)];
    if (q.length) localStorage.setItem(QUEUE + sid, JSON.stringify(q));
    else if (queuedN.current[tid]) localStorage.removeItem(QUEUE + sid);
    queuedN.current[tid] = q.length;
  };
  useEffect(() => { for (const t of Object.values(live.current)) keepQueue(t.tid, t.view); }, [held]);
  /** Too many idle conversations open: the oldest idle ones close (not the one on screen, nothing working or asking). */
  const trim = (keep: string) => {
    const idle = Object.values(live.current).filter((t) => t.tid !== keep && !t.side && !busyView(t.view) && !t.view?.asking).sort((a, b) => b.used - a.used);
    for (const t of idle.slice(KEEP_OPEN - 1)) close(t.tid);
  };
  const close = (tid: string) => {
    const side = Object.values(live.current).find((t) => t.side === tid);
    if (side) close(side.tid);
    core.stop(tid).catch(() => {});
    setThreads((t) => { const { [tid]: _, ...rest } = t; return rest; });
    // what was kept for it goes with it (its queue stays saved under its id, for when it is opened again)
    setHeld(({ [tid]: _, ...m }) => m);
    delete earlier.current[tid]; delete sent.current[tid]; clearTimeout(waits.current[tid]); delete waits.current[tid];
  };
  /** A side chat beside thread `of`: its own core in the same folder, the conversation forked on the same model and
   * provider (its cache is the conversation's; the own loop's saved history is that provider's), in plan mode. */
  const openSide = (of: string) => {
    const route = of === activeRef.current ? models?.choices.find((c) => c.current) : undefined;
    const p = live.current[of], id = p?.view?.session_id;
    if (!p || !id || Object.values(live.current).some((t) => t.side === of)) return;
    const tid = crypto.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
    setThreads((t) => ({ ...t, [tid]: { tid, folder: p.folder, view: null, error: null, used: Date.now(), side: of } }));
    core.start(tid, p.folder ?? undefined, p.folder ? undefined : "chat")
      // the core puts the fork in plan mode itself; the side box opens only once its view says plan
      .then(() => core.opIn(tid, { op: "fork", id, model: p.view!.model, effort: p.view!.effort, url: route?.url ?? null, key_env: route?.key_env ?? null }))
      .catch((e) => patch(tid, { error: String(e) }));
  };
  /** A folder leaves the recent projects (a removed worktree). */
  const forget = (path: string) => setProjects((p) => { const next = p.filter((x) => x !== path); localStorage.setItem(RECENT, JSON.stringify(next)); return next; });
  /** Closes thread `of`'s side chat; its fork leaves the saved lists (it was hidden when it got its id). */
  const closeSide = (of: string) => { const s = Object.values(live.current).find((t) => t.side === of); if (s) close(s.tid); };
  // a side chat's fork stays out of the saved lists: hidden as soon as it has its own id
  const hiddenForks = useRef(new Set<string>());
  const hideFork = (tid: string, v: View) => {
    const t = live.current[tid], parent = t?.side ? live.current[t.side] : undefined;
    const id = v.session_id;
    if (!parent || !id || id === parent.view?.session_id || hiddenForks.current.has(id)) return;
    hiddenForks.current.add(id);
    core.opIn(parent.tid, { op: "hide", id }).catch(() => {});
  };
  /** A new conversation in a folder (null: a chat), or `resume` an earlier one: on screen at once, in a thread of its
   * own; an earlier conversation already open is shown, not opened twice. */
  const open = (path: string | null, resume?: string, opts: { creator?: "new" | "edit"; then?: (tid: string) => void } = {}) => {
    if (path && !opts.creator) setProjects((p) => { const next = [path, ...p.filter((x) => x !== path)].slice(0, 12); localStorage.setItem(RECENT, JSON.stringify(next)); return next; });
    const already = !opts.creator && Object.values(live.current).find((t) => resume ? idsOf(t).includes(resume)
      : t.folder === path && !t.creator && !t.view?.blocks.some((b) => b.kind === "prompt") && !t.error);
    // an open one is reused; what was to follow its start (`then`) follows now
    if (already) { show(already.tid); opts.then?.(already.tid); return; }
    // messages it had queued when the window last closed were never sent: they go back in its box
    const lost = resume && localStorage.getItem(QUEUE + resume);
    if (lost) {
      try { localStorage.setItem(DRAFT + resume, [localStorage.getItem(DRAFT + resume), ...JSON.parse(lost)].filter(Boolean).join("\n\n")); } catch { /* a broken entry is dropped */ }
      localStorage.removeItem(QUEUE + resume);
    }
    // randomUUID needs a secure page, which some WebKit builds do not count the app's own pages as
    const tid = crypto.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
    setThreads((t) => ({ ...t, [tid]: { tid, folder: path, view: null, error: null, used: Date.now(), creator: opts.creator, resume } }));
    // the one on screen goes when it is idle: it is in the lists, a click away (until desktop 0.1.77 each conversation opened
    // stayed open with its own core, and four old ones looked through meant four "Open" rows and four processes)
    const was = live.current[activeRef.current];
    if (was && !was.creator && !was.side && !was.error && !unsaved(was.view) && (!was.view || (["Ready", "Done"].includes(was.view.activity) && !was.view.asking))) close(was.tid);
    show(tid);
    const m = resume ? null : startModel(path);
    core.start(tid, path ?? undefined, path ? undefined : "chat")
      .then((dir) => { if (!path) setHome(dir); if (resume) return core.resumeIn(tid, resume); if (m) core.opIn(tid, { op: "model", ...m }).catch(() => {}); opts.then?.(tid); })
      .catch((e) => patch(tid, { error: String(e) }));
    trim(tid);
  };
  useEffect(() => {
    if (!inTauri) return;
    const offs = [
      core.onExit((tid) => patch(tid, { error: "tazuna stopped. The app needs tazuna 0.1.128 or later (it carries its own; TAZUNA_BIN overrides)." })),
      core.onSendRejected(({ text }, tid) => restoreDraft(tid, text)),
      core.onView((v, tid) => {
        const was = live.current[tid]?.view?.session_id;
        // (a switch's view has no session until its first turn: the id before it is kept too)
        if (was && was !== v.session_id) earlier.current[tid] = [was, ...(earlier.current[tid] ?? []).filter((x) => x !== was)];
        tell(tid, v); keepQueue(tid, v); hideFork(tid, v); patch(tid, { view: v, error: null });
      }),
      // the quick-chat shortcut brought the window up: a new chat, its box focused
      listen("quick-chat", () => { open(null); requestAnimationFrame(() => document.querySelector<HTMLTextAreaElement>("footer textarea")?.focus()); }),
      core.onModels((m, tid) => { if (tid === activeRef.current) setModels(m); }),
      core.onDiff(setDiff),
      core.onSessions((s) => setSessions((old) => ({ ...old, ...Object.fromEntries(s.dirs.map((d) => [d.dir, d.list])) }))),
    ];
    Promise.all(offs).then(() => open(null)).catch(() => {});
    core.hotkey(localStorage.getItem(HOTKEY) ?? defaultHotkey).catch((e) => console.error(`quick-chat shortcut: ${e}`));
    return () => offs.forEach((p) => p.then((off) => off()));
  }, []);
  const pick = () => core.pickFolder().then((p) => p && open(p)).catch(() => {});
  // Cmd-N a new chat, Cmd-O a folder
  useEffect(() => {
    if (!inTauri) return;
    const key = (e: KeyboardEvent) => {
      if (menuKeys || !(e.metaKey || e.ctrlKey) || e.shiftKey) return;
      if (e.key === "n") { e.preventDefault(); open(null); }
      if (e.key === "o") { e.preventDefault(); pick(); }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);
  const cur = threads[active];
  const view = cur?.view ?? null, folder = cur?.folder ?? null, error = cur?.error ?? null;
  // the catalog for the picker and the model's name: again after each switch, so the current row is right
  const model = view ? `${active}|${view.model}|${view.effort}` : "";
  useEffect(() => { if (inTauri && model) core.models().catch(() => {}); }, [model]);
  // the lists again whenever a conversation's reply ends (the run log has it then, after tazuna's checks) and on another
  // folder. Until design review 12 by the view's `turns`, its model calls: the last refresh came during the reply's last
  // call, before the run log had the turn, and a new conversation left the sidebar when its first reply ended
  const turns = endedKey(Object.values(threads));
  const refreshSessions = () => {
    const dirs = [home, ...projects].filter((d): d is string => !!d);
    if (inTauri && dirs.length) core.sessions(dirs).catch(() => {});
  };
  // also once the conversation on screen has its view: one opened before its core answered (a project picked right after
  // launch) skipped the fetch, and the sidebar and start page stayed empty until another switch (desktop 0.1.76)
  useEffect(() => { if (view) refreshSessions(); }, [turns, folder, home, projects.length, active, !!view]);
  const list = Object.values(threads).filter((t) => !t.side).sort((a, b) => b.used - a.used);
  const side = Object.values(threads).find((t) => t.side === active);
  /** The thread on screen started again in its folder (after trusting the folder: its settings load at start). */
  const restart = () => {
    const t = live.current[activeRef.current];
    if (!t) return;
    patch(t.tid, { view: null, error: null });
    core.start(t.tid, t.folder ?? undefined, t.folder ? undefined : "chat").catch((e) => patch(t.tid, { error: String(e) }));
  };
  // where the box's draft is kept: the conversation's id once known (a resumed one's from the start), else its folder's
  // (after a change of model, before its first turn, the session before it: not a new chat's key)
  const draftKey = cur ? draftKeyOf(cur) : undefined;
  // a saved conversation being opened: its core is up and its messages are still on their way (the core's first view is
  // empty, the resumed one follows)
  const resuming = !!cur?.resume && !error && !!view && view.session_id !== cur.resume && !view.blocks.some((b) => b.kind === "prompt");
  // the first held message goes on once its conversation's reply has ended, the next once that one's reply has: a message
  // sent is taken when the view has one more of yours (a command that adds none: after 2 s idle)
  const [tick, setTick] = useState(0);
  useEffect(() => {
    for (const t of Object.values(threads)) {
      const q = held[t.tid], now = Date.now(), turn = heldTurn(t.view, sent.current[t.tid], now);
      if (!q?.length || t.error || turn < 0) continue;
      if (turn > 0) { clearTimeout(waits.current[t.tid]); waits.current[t.tid] = window.setTimeout(() => setTick((n) => n + 1), turn); continue; }
      sent.current[t.tid] = { prompts: t.view!.blocks.filter((b) => b.kind === "prompt").length, at: now };
      const [first] = q;
      setHeld((m) => ({ ...m, [t.tid]: (m[t.tid] ?? []).slice(1) }));
      // not taken (the core gone for a moment): back at the head of the queue, tried again after the wait
      core.sendIn(t.tid, first.text, first.agent).catch(() => setHeld((m) => (live.current[t.tid] ? { ...m, [t.tid]: [first, ...(m[t.tid] ?? [])] } : m)));
    }
  }, [threads, held, tick]);
  /** The id the saved lists know a conversation by: its session's, else an earlier one's they still hold. */
  const listed = new Set(Object.values(sessions).flatMap((rows) => rows.map((r) => r.id)));
  const listedId = (t?: Thread) => (t ? idsOf(t).find((id) => listed.has(id)) ?? t.view?.session_id ?? undefined : undefined);
  return { view, error, folder, projects, open, pick, models, sessions, home, diff, setDiff, refreshSessions, threads: list, active, show, close, restart, creator: cur?.creator, draftKey, restoreDraft, side, openSide, closeSide, forget, resuming, held: (held[active] ?? []).map((w) => w.text), hold, unhold, listedId };
}

/** Light or dark: the choice in Settings (lib/chrome.ts keeps it; until desktop 0.1.87 a flip lasted until the window
 * closed), else the system's, followed as it changes. Returns the flip. */
function useTheme() {
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", applyLooks);
    // Cmd/Ctrl-Shift-L flips light and dark
    const key = (e: KeyboardEvent) => { if (!menuKeys && (e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "l") flip(); };
    window.addEventListener("keydown", key);
    return () => { media.removeEventListener("change", applyLooks); window.removeEventListener("keydown", key); };
  }, []);
  return flip;
}
const flip = () => look(THEME, darkNow() ? "light" : "dark");

/** Cmd-K opens the palette, Cmd-/ the shortcuts. */
function useOverlay() {
  const [overlay, setOverlay] = useState<"palette" | "shortcuts" | "settings" | "usage" | "setup" | null>(() => (inTauri && !localStorage.getItem(SETUP) ? "setup" : null));
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (!menuKeys && (e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "u") { e.preventDefault(); if (inTauri) setOverlay((o) => (o === "usage" ? null : "usage")); }
      if (menuKeys || !(e.metaKey || e.ctrlKey) || e.shiftKey) return;
      if (e.key === "k") { e.preventDefault(); setOverlay((o) => (o === "palette" ? null : "palette")); }
      if (e.key === "/") { e.preventDefault(); setOverlay((o) => (o === "shortcuts" ? null : "shortcuts")); }
      if (e.key === ",") { e.preventDefault(); if (inTauri) setOverlay((o) => (o === "settings" ? null : "settings")); }
    };
    const usage = () => { if (inTauri) setOverlay("usage"); };
    window.addEventListener("keydown", key);
    window.addEventListener("tazuna-usage", usage);
    return () => { window.removeEventListener("keydown", key); window.removeEventListener("tazuna-usage", usage); };
  }, []);
  return [overlay, setOverlay] as const;
}

export default function App() {
  const flipTheme = useTheme();
  const [overlay, setOverlay] = useOverlay();
  // what the palette opens with (`/resume` opens it on the conversations); empty again once it closes
  const [paletteQ, setPaletteQ] = useState("");
  useEffect(() => { if (overlay !== "palette") setPaletteQ(""); }, [overlay]);
  // the provider Settings opens on (the model picker's "No key yet"), its key field open; that opening only
  const [keyFocus, setKeyFocus] = useState<string | null>(null);
  useEffect(() => { if (overlay !== "settings") setKeyFocus(null); }, [overlay]);
  // "edit from here": the message being edited (its checkpoint, its block, the files tazuna changed since, whether they
  // go back too) and the text put in the box
  const [editing, setEditing] = useState<{ cp: number; block: number; files: number; keep: boolean } | null>(null);
  const [draft, setDraft] = useState<{ text: string; n: number; append?: boolean }>();
  const [reviewing, setReviewing] = useState<{ scope: string; turn?: number; path?: string } | null>(null);
  // the menu bar's items (macOS)
  useEffect(() => {
    if (!menuKeys) return;
    const off = core.onMenu((id) => {
      if (id === "new-chat") open(null);
      else if (id === "open-folder") pick();
      else if (id === "palette") setOverlay((o) => (o === "palette" ? null : "palette"));
      else if (id === "shortcuts") setOverlay((o) => (o === "shortcuts" ? null : "shortcuts"));
      else if (id === "theme") flipTheme();
      else if (id === "settings") setOverlay("settings");
      else if (id === "usage") setOverlay("usage");
      else if (id === "update") checkUpdate(true);
    });
    return () => { off.then((f) => f()); };
  }, []);
  const { view, error, folder, projects, open, pick, models, sessions, home, diff, setDiff, refreshSessions, threads, active, show, close, restart, creator, draftKey, restoreDraft, resuming, side, openSide, closeSide, forget, held, hold, unhold, listedId } = useCore((text) => setDraft({ text, n: Date.now(), append: true }));
  useEffect(() => setEditing(null), [active]);
  useEffect(watchUpdates, []);
  // creator mode: the user's panels, read again whenever a conversation finishes a turn (a creator one may have made one)
  const [userWidgets, setUserWidgets] = useState<UserWidget[]>([]);
  const [, setPluginTick] = useState(0);
  const loadedIds = useRef<string[]>([]);
  const [paletteTick, setPaletteTick] = useState(0);
  useEffect(() => onPaletteChange(() => setPaletteTick((n) => n + 1)), []);
  const allTurns = endedKey(threads);
  const reloadWidgets = () => widgets.list().then((w) => setUserWidgets(w ?? [])).catch(() => {});
  useEffect(() => { if (inTauri) reloadWidgets(); }, [allTurns]);
  // enabled plugins run (each reloaded when its code changes, taken back when turned off or removed); none in safe mode
  useEffect(() => {
    if (!inTauri || safeMode()) return;
    const on = userWidgets.filter((w) => w.kind === "plugin" && w.state === "enabled");
    on.forEach((w) => { load(w.id, w.print!).then(() => setPluginTick((n) => n + 1)); });
    loadedIds.current.forEach((id) => { if (!on.some((w) => w.id === id)) unload(id); });
    loadedIds.current = on.map((w) => w.id);
  }, [userWidgets]);
  // plugins read the conversation's data and act through these
  useEffect(() => { setData(data); });
  useEffect(() => setActions({
    send: (text) => core.send(text).catch(() => {}), newChat: () => open(null), setMode: (m) => core.setMode(m).catch(() => {}),
    notify: (title, body) => core.notify(title, body).catch(() => {}), openLink: (url) => core.openLink(url).catch(() => {}),
    review: (scope) => setReviewing({ scope }),
  }), []);
  // each user panel is an entry of the rail spot, after tazuna's own cards
  useEffect(() => {
    const offs = userWidgets.filter((w) => w.kind === "panel").map((w, i) => slots.register({ id: `panel:${w.id}`, slot: w.slot ?? "rail", order: 1000 + i, source: `panel:${w.id}`,
      render: ({ data }: { data: unknown }) => <UserPanel w={w} data={data} onEdit={() => editPanel(w.id)}
        onRemove={() => widgets.remove(w.id).then(() => widgets.list().then((w) => setUserWidgets(w ?? []))).catch(() => {})} /> }));
    return () => offs.forEach((off) => off());
  }, [userWidgets]);
  // creator mode checks its work: when a creator conversation finishes a turn, tazuna checks what it wrote without running
  // it in the window (a plugin runs once, sandboxed, against a stand-in; security review) and sends a failure or "shows
  // nowhere" back to the agent, up to two rounds (DeepSeek Harness's creator reports done for plugins that never loaded).
  // A plugin that passes waits for the user: what it would do, its code, and Enable.
  const creatorRuns = useRef<Record<string, { state: string; rounds: number }>>({});
  const [ready, setReady] = useState<Record<string, { id: string; title: string; print: string; code: string; summary: string } | null>>({});
  const [codeView, setCodeView] = useState<{ title: string; text: string } | null>(null);
  const verifyCreation = async (t: Thread, run: { rounds: number }) => {
    const id = t.folder?.split(/[\\/]/).pop();
    if (!id) return;
    const list = (await widgets.list().catch(() => [])) ?? [];
    setUserWidgets(list);
    const w = list.find((x) => x.id === id);
    let problem: string | null = null;
    if (!w) problem = "Nothing is in the folder that tazuna can load: no index.html (a panel) and no plugin.js (a plugin), or the files are empty.";
    else if (w.kind === "plugin") {
      // the code and its fingerprint from one read: what the user reads is what Enable turns on
      const read = await widgets.code(id).catch(() => null);
      const r = await dryRun(id);
      const said = summary(r.rec);
      if (r.error) problem = `It failed when run: ${r.error}`;
      else if (!said) problem = "It ran but registers nothing: no spot entry, no palette action, no style and no hidden part. Check the spot names against the specification.";
      else if (read && (w.state !== "enabled" || w.print !== read.print)) setReady((m) => ({ ...m, [t.tid]: { id, title: w.title, print: read.print, code: read.code, summary: said } }));
    }
    if (problem && run.rounds < 2) {
      run.rounds++;
      core.sendIn(t.tid, `tazuna checked what you wrote. ${problem} Fix it in this folder.`).catch(() => {});
    }
  };
  /** The user enables a plugin creator mode made: it loads; a part that throws when drawn goes back to the agent. */
  const enableReady = async (tid: string) => {
    const r = ready[tid];
    if (!r) return;
    setReady((m) => ({ ...m, [tid]: null }));
    await widgets.enable(r.id, r.print, true).catch(() => {});
    const since = Date.now();
    await load(r.id, r.print);
    reloadWidgets();
    await new Promise((res) => setTimeout(res, 900));
    const f = footprint(r.id, since);
    const problem = f.loadError ? `It failed to load in the window: ${f.loadError}` : f.errors.length ? `A part it registered threw when drawn: ${f.errors.slice(-3).join("; ")}` : null;
    if (problem) core.sendIn(tid, `The user enabled the plugin. ${problem} Fix it in this folder.`).catch(() => {});
  };
  useEffect(() => {
    for (const t of threads) {
      if (!t.creator || !t.view) continue;
      const st = busyView(t.view) || t.view.asking ? "working" : "idle";
      const run = (creatorRuns.current[t.tid] ??= { state: "idle", rounds: 0 });
      if (run.state === "working" && st === "idle") verifyCreation(t, run);
      run.state = st;
    }
  }, [threads]);
  /** A new panel or plugin: its folder, a conversation in it (writes inside allowed), the request with the specification. */
  const createPanel = (ask: string) => {
    widgets.create(widgetId(ask)).then((path) => open(path, undefined, { creator: "new",
      then: (tid) => { core.allowRootIn(tid, path).then(() => core.sendIn(tid, creatorMessage(ask, false))).catch(() => {}); } })).catch(() => {});
  };
  /** A panel changed with tazuna: a conversation in its folder; the first message carries the specification. */
  const editPanel = (id: string) => {
    widgets.path(id).then((path) => {
      // the new conversation's box starts with this (its draft, before it has an id)
      localStorage.setItem(`${DRAFT}new:${path}`, "Change this panel: ");
      open(path, undefined, { creator: "edit", then: (tid) => { core.allowRootIn(tid, path).catch(() => {}); } });
    }).catch(() => {});
  };
  // where a message goes and what it costs (lib/route.ts), from the catalog and the run log's last 90 days
  const [rows, setRows] = useState<Row[] | null>(null);
  useEffect(() => { if (!inTauri) return; const off = core.onUsage((u) => setRows(u.rows)); return () => { off.then((f) => f()); }; }, []);
  const choice = models?.choices.find((c) => c.current);
  /** A model picked by hand: a non-Claude one is where new conversations of this kind start (START_MODEL). */
  const pickModel = (c: ModelChoice) => {
    const key = START_MODEL + (folder ? "project" : "chat");
    if (c.url) localStorage.setItem(key, JSON.stringify({ model: c.model, url: c.url, key_env: c.key_env })); else localStorage.removeItem(key);
    return core.setModel(c);
  };
  useEffect(() => { if (inTauri && view) core.usage().catch(() => {}); }, [choice?.model, choice?.url, allTurns]);
  // whether Claude Code is there and signed in (core C1): read at the start, from Settings' "Check again", and when the window
  // comes back while it was signed out (a login in a terminal)
  const [claude, setClaude] = useState<ClaudeSetup | null>(null);
  const claudeRef = useRef(claude);
  claudeRef.current = claude;
  useEffect(() => {
    if (!inTauri) return;
    const off = core.onSetup((s) => {
      // signed in or out since: the catalog's Claude rows change with it
      if (claudeRef.current && claudeRef.current.claude_login !== s.claude_login) core.models().catch(() => {});
      setClaude(s);
      claudeSignedOut = s.claude_login === false;
    });
    const back = () => { if (claudeRef.current?.claude_login === false) core.setup().catch(() => {}); };
    window.addEventListener("focus", back);
    return () => { off.then((f) => f()); window.removeEventListener("focus", back); };
  }, []);
  useEffect(() => { if (inTauri && view) core.setup().catch(() => {}); }, [!!view]);
  const line = inTauri ? routeLine(choice, view, rows, models?.choices ?? []) : null;
  // the conversation runs on Claude (its CLI: no url) and setup says it is signed out
  const claudeOut = claude?.claude_login === false && !!view && (choice ? !choice.url : /^(claude-)?(opus|sonnet|haiku|fable)/.test(view.model));
  // the first message to a paid model asks once (per model; the cap in Settings holds after that)
  const [paidAsk, setPaidAsk] = useState<{ text: string; draft: string } | null>(null);
  const paidKey = choice ? `tazuna.paid-ok.${choice.model}|${choice.url}` : "";
  // the pull request of the conversation's branch (gh, through the core): asked when a folder's conversation shows and
  // after each finished reply; the header chip and the PR panel read it
  const [prs, setPrs] = useState<Record<string, Pr>>({});
  const [showPr, setShowPr] = useState(false);
  useEffect(() => { if (!inTauri) return; const off = core.onPr((p, tid) => setPrs((m) => ({ ...m, [tid]: p }))); return () => { off.then((f) => f()); }; }, []);
  const replies = view ? view.blocks.reduce((n, b) => n + (b.kind === "done" ? 1 : 0), 0) : 0;
  useEffect(() => { if (inTauri && folder && view) core.prIn(active).catch((e) => setPrs((m) => (m[active] ? m : { ...m, [active]: { state: "", error: String(e) } }))); }, [active, folder, !!view, replies]);
  const pr = prs[active];
  const [appVersion, setAppVersion] = useState("");
  useEffect(() => { if (inTauri) getVersion().then(setAppVersion).catch(() => {}); }, []);
  // the work panel (the right column): by rule (lib/types.ts `panelState`), and by hand per conversation (⌘\, the header's
  // button, the tab): one choice for the whole window put the panel into plain chats and stopped the fold everywhere
  // (design review 11)
  const [panelBy, setPanelBy] = useState<Record<string, boolean>>({});
  // the window's width: the work panel closes by itself below 1280 px, and below 1000 px the sidebar opens over the
  // conversation from the header's button (at 980 px the panel left the conversation 430 px and the composer's pills
  // wrapped; design review, 2026-11-18)
  const width = useWidth();
  const narrow = width < 1000;
  const [navOpen, setNavOpen] = useState(false);
  const navDialog = useRef<HTMLDialogElement>(null);
  // and when a dialog opens over the window: the narrow sidebar over it took the first click (review of design review 12)
  useEffect(() => setNavOpen(false), [active, narrow, overlay]);
  useLayoutEffect(() => {
    const dialog = navDialog.current;
    if (!narrow || !navOpen || !dialog) return;
    dialog.showModal();
    return () => dialog.close();
  }, [narrow, navOpen]);
  // in a project it stays folded, a small tab on the sheet's edge that counts the files changed so far, and it never opens
  // or closes by itself: opening at a turn's end moved the reading column under the reader, and Undo moved it back (design
  // review 12; review 11 had it open by itself at the first change)
  const col = panelState({ project: !!folder, width, choice: panelBy[active] });
  const panelOpen = col.open;
  // the control pressed (the tab, the header's button), where focus goes back when the panel over the conversation shuts:
  // WebKit does not focus a button on a click, so what had focus then was the page
  const opener = useRef<HTMLElement | null>(null);
  const togglePanel = (e?: { currentTarget: EventTarget | null }) => {
    opener.current = (e?.currentTarget as HTMLElement | undefined) ?? (document.activeElement as HTMLElement | null);
    setPanelBy(({ [active]: _, ...m }) => { const c = panelChoice(panelOpen); return c === undefined ? m : { ...m, [active]: c }; });
  };
  const toggleNow = useRef(togglePanel);
  toggleNow.current = togglePanel;
  // a side chat or a subagent's sheet has the column: the panel's key does nothing then (it flipped the hidden panel)
  const paneOwns = useRef(false);
  useEffect(() => {
    const key = (e: KeyboardEvent) => { if ((e.metaKey || e.ctrlKey) && e.key === "\\") { e.preventDefault(); if (!paneOwns.current) toggleNow.current(); } };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);
  /** A message of yours from anywhere but the box (review comments, the work panel, the pull request, "send to agent"): to
   * the agent, and during a reply it waits in the window's queue as the box's do (it jumped ahead of them, and Stop did not
   * bring it back) */
  const say = (text: string, agent = true) => { if (busy) hold(active, text, agent); else core.send(text, agent).catch(() => {}); };
  const work: Work = {
    files: changedFiles(view),
    branch: pr?.headRefName ?? pr?.branch, base: pr?.baseRefName,
    checks: (view?.blocks ?? []).flatMap((b) => (b.kind === "verify" ? [{ cmd: b.cmd, ok: b.ok, secs: b.secs, none: b.cannot_run ? "Could not run" : b.timed_out ? "Timed out" : b.no_tests ? "No tests found" : undefined }] : [])).reverse(),
    pr: pr ?? null, folder,
    review: inTauri && folder ? () => setReviewing({ scope: "uncommitted" }) : undefined,
    openPr: inTauri && folder ? () => setShowPr(true) : undefined,
    ask: inTauri ? (text) => say(text) : undefined,
  };
  // the tab's count, live: the files the finished turns changed and those the running reply has edited so far (it said
  // "Changes 0" while the fold said "Edited main.rs": a turn's changes come from git when it ends)
  // (a folder whose pull request reply has an empty branch is not a git repository; gh missing says nothing either way)
  const changes = changedCount(view, folder, work.files.map((f) => f.path), !(pr && pr.branch === "" && !pr.headRefName));
  // worktrees (src/worktree.rs): a new conversation in a worktree of this project; its setup script runs on the user's word
  const [wt, setWt] = useState<{ ask?: boolean; script?: string; id?: string; error?: string; remove?: { path: string; dirty?: string } } | null>(null);
  const [wtNotes, setWtNotes] = useState<Record<string, string>>({});
  const inWorktree = !!folder && isWorktree(folder);
  // best-of-N (components/BestOf.tsx): the lanes of one task, and the models still waiting for their worktree (made one
  // after another by the conversation they were started from)
  const [bestOf, setBestOf] = useState(false);
  const [lanes, setLanes] = useState<Lanes | null>(null);
  const [showLanes, setShowLanes] = useState(false);
  const lanesRef = useRef(lanes);
  lanesRef.current = lanes;
  const laneQueue = useRef<ModelChoice[]>([]);
  // each worktree's event handled once (React's development mode listens twice)
  const seenWorktrees = useRef(new Set<string>());
  const startLanes = (task: string, models: ModelChoice[]) => {
    setLanes({ task, origin: active, lanes: models.map((model) => ({ model })) });
    laneQueue.current = [...models];
    core.opIn(active, { op: "worktree_new", run_setup: false }).catch(() => {});
    setShowLanes(true);
  };
  const keepLane = (keep: Lane) => {
    const ls = lanesRef.current;
    if (!ls) return;
    const via = threadsRef.current.some((t) => t.tid === ls.origin) ? ls.origin : keep.tid!;
    for (const l of ls.lanes) if (l !== keep && l.path) core.opIn(via, { op: "worktree_remove", path: l.path, force: true }).catch(() => {});
    setLanes(null); setShowLanes(false);
    if (keep.tid) show(keep.tid);
  };
  useEffect(() => {
    if (!inTauri) return;
    const offs = [
      core.onWorktree((w) => {
        if (w.ask_setup) { setWt({ ask: true, script: w.script, id: w.id }); return; }
        if (w.error || !w.path) {
          // a lane's worktree failed: the lanes not started yet are dropped
          if (laneQueue.current.length) { laneQueue.current = []; setLanes((ls) => ls && { ...ls, lanes: ls.lanes.filter((l) => l.tid) }); }
          setWt({ error: w.error ?? "no worktree made" }); return;
        }
        setWt(null);
        const parts = [`Branch ${w.branch}`, `ports ${w.port}-${(w.port ?? 0) + 9} (TAZUNA_PORT)`, w.copied?.length ? `copied ${w.copied.join(", ")}` : "no local files to copy"];
        if (w.setup) parts.push(w.setup.ok ? "setup ran" : `setup failed: ${w.setup.tail.split("\n").slice(-2).join(" ")}`);
        if (w.setup_skipped) parts.push(w.setup_skipped);
        setWtNotes((m) => ({ ...m, [w.path!]: parts.join("; ") }));
        if (seenWorktrees.current.has(w.path)) return;
        seenWorktrees.current.add(w.path);
        const model = laneQueue.current.shift(), ls = lanesRef.current;
        if (model && ls) {
          // a lane: its model, auto mode (it works inside its own worktree), the task; then the next lane's worktree
          open(w.path, undefined, { then: (tid) => {
            core.opIn(tid, { op: "model", model: model.model, url: model.url, key_env: model.key_env }).catch(() => {});
            core.modeIn(tid, "auto").catch(() => {});
            core.sendIn(tid, ls.task).catch(() => {});
            setLanes((cur) => cur && { ...cur, lanes: cur.lanes.map((l) => (l.model === model ? { ...l, path: w.path, tid } : l)) });
          } });
          if (laneQueue.current.length) core.opIn(ls.origin, { op: "worktree_new", run_setup: false }).catch(() => {});
          return;
        }
        open(w.path);
      }),
      core.onWorktreeRemoved((r) => {
        if (r.error) { setWt({ remove: { path: r.path, dirty: r.error } }); return; }
        setWt(null);
        for (const t of threadsRef.current) if (t.folder === r.path) close(t.tid);
        forget(r.path);
      }),
    ];
    return () => offs.forEach((p) => p.then((off) => off()));
  }, []);
  const threadsRef = useRef(threads);
  threadsRef.current = threads;
  // /create's question, and what creating behaviour made in each conversation (the core's "creation" lines)
  const [createAsk, setCreateAsk] = useState<string | null>(null);
  const [creations, setCreations] = useState<Record<string, { state: string; dir?: string }>>({});
  useEffect(() => { if (!inTauri) return; const off = core.onCreation((c, tid) => setCreations((m) => ({ ...m, [tid]: c }))); return () => { off.then((f) => f()); }; }, []);
  // A subagent's own conversation beside this one (the terminal's ctrl-b): opened from its step or from the row that says
  // it is working in the background, by the call that started it. Asked for when opened and, while it runs, every 1.5 s.
  // ponytail: the whole of it each time; only what is new once a long agent's makes the window slow
  const [agent, setAgent] = useState<{ tid: string; call: string } | null>(null);
  const [agentViews, setAgentViews] = useState<Record<string, View>>({});
  useEffect(() => { if (!inTauri) return; const off = core.onAgent((a) => setAgentViews((m) => ({ ...m, [a.call]: a.view }))); return () => { off.then((f) => f()); }; }, []);
  useEffect(() => {
    const open = (e: Event) => setAgent({ tid: active, call: String((e as CustomEvent).detail) });
    window.addEventListener("tazuna-agent", open);
    return () => window.removeEventListener("tazuna-agent", open);
  }, [active]);
  const tasks = view?.tasks ?? [];
  const agentHere = agent?.tid === active ? agent : null, agentTask = agentHere ? tasks.find((t) => t.calls.includes(agentHere.call)) : undefined;
  // where the column beside the sheet would leave too little text, the panel floats over the conversation (`col.over`):
  // Esc (after any menu open over it: `escFor`), a click outside it, the tab, its button or ⌘\ shut it. Focus goes into it
  // and back to what had it. Both listeners in the capture phase: Tauri's drag regions stop a press at the document.
  const floating = panelOpen && col.over && !side && !agentHere && !reviewing;
  const popRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!floating) return;
    const before = opener.current ?? (document.activeElement as HTMLElement | null);
    const pop = popRef.current;
    (pop?.querySelector<HTMLElement>(FOCUSABLE) ?? pop)?.focus({ preventScroll: true });
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || escFor({ menu: !!document.querySelector(MENUS), over: true }) !== "panel") return;
      e.stopPropagation(); toggleNow.current();
    };
    const away = (e: MouseEvent) => { if (!(e.target as Element).closest?.("[data-panel]")) toggleNow.current(); };
    // A timeline jump reveals the conversation, not a target left under this floating panel. The docked rail stays put.
    const jump = (e: Event) => { const i = Number((e as CustomEvent).detail); if (Number.isInteger(i) && i >= 0) toggleNow.current(); };
    window.addEventListener("keydown", key, true); window.addEventListener("mousedown", away, true);
    window.addEventListener("tazuna-jump", jump);
    return () => {
      window.removeEventListener("keydown", key, true); window.removeEventListener("mousedown", away, true);
      window.removeEventListener("tazuna-jump", jump);
      const now = document.activeElement;
      if (before?.isConnected && (!now || now === document.body || pop?.contains(now))) before.focus({ preventScroll: true });
    };
  }, [floating]);
  paneOwns.current = !!side || !!agentHere;
  // a subagent with no task of its own (one that ran inside the reply) runs as long as its step does
  const agentStep = agentHere && (view?.blocks.find((b) => b.kind === "tool" && b.id === agentHere.call) as { running?: boolean; input?: Record<string, unknown> | null } | undefined);
  const agentRunning = agentTask ? agentTask.running : !!agentStep?.running;
  useEffect(() => {
    if (!agentHere) return;
    const ask = () => { core.agentIn(agentHere.tid, agentHere.call).catch(() => {}); };
    ask();
    if (!agentRunning) return;
    const t = setInterval(ask, 1500);
    return () => clearInterval(t);
  }, [agentHere?.call, agentHere?.tid, agentRunning]);
  // what `/` and `@` complete to in each conversation, asked when one is first typed there
  const [complete, setComplete] = useState<Record<string, Completions>>({});
  useEffect(() => { if (!inTauri) return; const off = core.onComplete((c, tid) => setComplete((m) => ({ ...m, [tid]: c }))); return () => { off.then((f) => f()); }; }, []);
  /** A command, skill or tool: a new conversation in this folder makes it (src/create.rs), checked by the core. */
  const createBehaviour = (ask: string) => open(folder, undefined, { then: (tid) => { core.createIn(tid, ask).catch(() => {}); } });
  const creation = creations[active];
  /** A message on its way: /create, a creator conversation's first, an edit from an earlier message, or a plain one. */
  const sendNow = (text: string, draft: string) => {
    // /create: part of the window (a panel or plugin) or something tazuna does (a command, skill or tool): asked first
    if (text.startsWith("/create ")) { setCreateAsk(text.slice(8).trim()); return; }
    // during a reply it waits in the window, shown queued, and goes when the reply ends
    if (busy) { hold(active, text); return; }
    const send = (message: string, agent = false) => core.sendDraft(active, message, draft, agent);
    const refused = (e: unknown) => { restoreDraft(active, draft); console.error(e); };
    if (creator === "edit" && !view?.blocks.some((b) => b.kind === "prompt")) { send(creatorMessage(text, true), true).catch(refused); return; }
    if (!editing) { send(text).catch(refused); return; }
    const { cp, keep } = editing;
    setEditing(null);
    core.rewind(cp, keep).then(() => send(text)).catch(refused);
  };
  const messages: Message[] = [...(view ? viewMessages(view) : inTauri ? [] : demo.messages), ...held.map((text) => ({ role: "user" as const, text, queued: true }))];
  /** Stop: the reply ends, and what waited behind it goes back into the box (it went to the agent at once). */
  const stop = () => { core.interrupt().catch(() => {}); const q = unhold(active); if (q.length) setDraft({ text: q.join("\n\n"), n: Date.now(), append: true }); };
  // the core did not start: its reason and a way to try again, no conversation (until desktop 0.1.77 the demo's title,
  // plan and cost filled the window around a small red line)
  const failed = inTauri && !view && !!error;
  // the plan's windows are the account's: the newest reading of any open conversation, not the one on screen (a resumed
  // conversation showed its own days-old reading, design review 5)
  const planView = threads.map((t) => t.view).filter((v) => v?.limits).sort((a, b) => (b!.limits_at ?? 0) - (a!.limits_at ?? 0))[0];
  // every account's reading for the status bar (claude's own /usage: the CLI for a few seconds, no model call). Asked
  // for when something may have moved it, not by the clock (until desktop 0.1.89 at the start and every 15 minutes: the
  // bar stood still through a long reply, and read on while nothing ran): at the start, when a reply ends, when the
  // window comes back after a while (other programs spend the same plan), every 15 minutes while a reply runs, from the
  // usage page and from the bar's button. Idle, nothing. Between readings the plan's 5 hours and week come from the
  // conversations themselves (withLive).
  const [accounts, setAccounts] = useState<Accounts | null>(null);
  // when the reading came, and whether one is on its way (it gives up waiting after 45 s: the core may not answer)
  const [acct, setAcct] = useState<{ at?: number; busy: boolean }>({ busy: false });
  useEffect(() => {
    if (!inTauri) return;
    const off = core.onAccounts((a) => { setAccounts(a); setAcct({ at: Date.now(), busy: false }); });
    // the plan comes first, about a second in: the bar has it then, the other accounts as they were until theirs come
    const offPlan = core.onAccountsPlan((p) => { setAccounts((a) => ({ credits: [], omniroute: [], ...a, ...p })); setAcct({ at: Date.now(), busy: false }); });
    return () => { off.then((f) => f()); offPlan.then((f) => f()); };
  }, []);
  const askedAt = useRef(0);
  /** `auto`: one the window asks for by itself, at most once a minute. */
  const readAccounts = (auto = false) => {
    if (!inTauri || (auto && Date.now() - askedAt.current < 60_000)) return;
    askedAt.current = Date.now();
    setAcct((a) => ({ ...a, busy: true }));
    core.accounts().catch(() => setAcct((a) => ({ ...a, busy: false })));
    setTimeout(() => setAcct((a) => (a.busy ? { ...a, busy: false } : a)), 45_000);
  };
  const haveView = !!view;
  useEffect(() => { if (haveView) readAccounts(); }, [haveView]);
  // the replies running now, in any open conversation: one fewer is a reply that ended
  const working = threads.filter((t) => busyView(t.view)).length, workedBefore = useRef(0);
  useEffect(() => { if (working < workedBefore.current) readAccounts(true); workedBefore.current = working; }, [working]);
  useEffect(() => {
    if (!working) return;
    const t = setInterval(() => readAccounts(true), 15 * 60_000);
    return () => clearInterval(t);
  }, [working > 0]);
  useEffect(() => {
    const back = () => { if (Date.now() - askedAt.current > 3 * 60_000) readAccounts(true); };
    window.addEventListener("focus", back);
    return () => window.removeEventListener("focus", back);
  }, []);
  const accountsNow = withLive(accounts, acct.at, planView);
  // when the numbers on screen were read: the full reading, or a conversation's newer one
  const readAt = accountsNow === accounts ? acct.at : (planView!.limits_at ?? 0) * 1000;
  const usage = { ...(view ? viewUsage(view) : inTauri ? NO_USAGE : demo.usage),
    ...(planView?.limits ? { plan5h: Math.round(planView.limits[0]), planWeek: Math.round(planView.limits[1]), plan5hReset: resetIn(planView.limits_reset) } : {}) };
  const accts = accountItems(accountsNow, { plan5h: usage.plan5h, planWeek: usage.planWeek, resetAt: planView?.limits_reset, balance: view?.balance });
  const hereAccts = accountOf(accts, models?.choices.find((c) => c.current), view?.model ?? "");
  const busy = !!view && !["Ready", "Done"].includes(view.activity);
  // when the reply now running began, as this window first saw it: the fold's "Working 12 s" keeps counting across a switch
  // to another conversation and back (it began at 0 s again on every switch)
  const replyStart = useRef<Record<string, number>>({});
  const startKey = `${active}:${view ? view.blocks.filter((b) => b.kind === "prompt").length : 0}`;
  // (and from zero again when a reply of the same message starts anew: a Retry counted on from the failed try)
  if (busy && !replyStart.current[startKey]) replyStart.current[startKey] = Date.now();
  else if (!busy && view) delete replyStart.current[startKey];
  const session = view
    ? { model: view.model || "default", effort: view.effort || "", mode: view.mode, branch: "", changed: 0, lastCost: lastCost(view) }
    : inTauri ? { model: "", effort: "", mode: "auto", branch: "", changed: 0, lastCost: 0 } : demo.session;
  const place = inTauri ? (folder ? base(folder) : "Chat") : "api-gateway";
  const actions = (): Action[] => {
    const a: Action[] = [
      { id: "new", group: "Start", label: "New chat", keys: "⌘N", run: () => open(null) },
      { id: "folder", group: "Start", label: "Open folder…", keys: "⌘O", run: pick },
      ...projects.map((p) => ({ id: `p:${p}`, group: "Projects", label: base(p), hint: p, keywords: "open project folder", run: () => open(p) })),
      ...[...(home ? (sessions[home] ?? []).map((r) => [r, null] as const) : []), ...projects.flatMap((p) => (sessions[p] ?? []).slice(0, 10).map((r) => [r, p] as const))]
        .map(([r, dir]) => ({ id: `s:${r.id}`, group: "Conversations", label: shortTitle(r.first, r.named) || "(no message)", hint: dir ? base(dir) : "chat", keywords: `resume continue ${r.first}`, run: () => open(dir, r.id) })),
    ];
    if (busy) a.unshift({ id: "stop", group: "This conversation", label: "Stop the reply", keys: "Esc", run: stop });
    const undo = view?.changes?.find((c) => c.can_undo);
    if (undo && !busy) a.push({ id: "undo", group: "This conversation", label: `Undo the last turn's changes`, hint: `${undo.files.length} file${undo.files.length === 1 ? "" : "s"}`, run: () => core.undo(undo.turn) });
    if (view && folder) a.push({ id: "worktree", group: "This project", label: "New conversation in a worktree", hint: "a branch and folder of its own", keywords: "worktree branch parallel", run: () => core.worktreeNew(null).catch(() => {}) },
      { id: "pr", group: "This project", label: "Pull request: checks and reviews", keywords: "pr github gh checks ci", run: () => setShowPr(true) },
      { id: "best-of", group: "This project", label: "Try on several models…", hint: "one task, a worktree each, compared", keywords: "best of n compare lanes models parallel",
        run: () => { core.models().catch(() => {}); setBestOf(true); } });
    if (lanes) a.push({ id: "lanes", group: "This project", label: "Compare the lanes", hint: `${lanes.lanes.length} models`, keywords: "best of n lanes", run: () => setShowLanes(true) });
    if (inWorktree) a.push({ id: "worktree-remove", group: "This project", label: "Remove this worktree", hint: "its branch stays", run: () => setWt({ remove: { path: folder! } }) });
    if (view?.session_id && !side) a.push({ id: "side", group: "This conversation", label: "Side chat", hint: "ask about it; nothing goes back", keywords: "btw aside question fork", run: () => openSide(active) });
    if (side) a.push({ id: "side-close", group: "This conversation", label: "Close the side chat", run: () => closeSide(active) });
    if (view && folder) a.push(
      { id: "review-uncommitted", group: "Review", label: "Review uncommitted changes", keywords: "diff stage revert hunk", run: () => setReviewing({ scope: "uncommitted" }) },
      { id: "review-branch", group: "Review", label: "Review the branch against its base", keywords: "diff pr", run: () => setReviewing({ scope: "branch" }) });
    if (view) {
      // each mode with what it does (the palette listed bare "auto, ask, edits", review 3); a plain chat has none
      if (folder) a.push(...MODES.map(([m, about]) => ({ id: `m:${m}`, group: "Permission mode", label: `${m.charAt(0).toUpperCase()}${m.slice(1)}`, hint: m === view.mode ? `${about} (now)` : about, keywords: `mode permission ${m}`, run: () => core.setMode(m) })));
      const cur = models?.choices.find((c) => c.current);
      a.push(...(cur?.efforts ?? []).map((e) => ({ id: `e:${e}`, group: "Effort", label: e, hint: e === view.effort ? "current" : undefined, keywords: "effort thinking", run: () => core.setEffort(e) })));
      a.push(...(models?.choices ?? []).filter((c) => !c.why_not).map((c) => ({ id: `mo:${c.model}|${c.url}|${c.key_env}|${c.label}`, group: "Model", label: modelLabel(c.model), hint: c.current ? "current" : c.place, keywords: `model ${c.provider}`, run: () => pickModel(c) })));
    }
    a.push({ id: "create-behaviour", group: "Creator mode", label: "Make a command, skill or tool…", hint: "/create what it should do", keywords: "skill command mcp tool creator",
        run: () => setDraft({ text: "/create ", n: Date.now() }) });
    a.push({ id: "create-panel", group: "Creator mode", label: "Create a panel…", hint: "/create what it should show", keywords: "widget creator customize",
        run: () => setDraft({ text: "/create ", n: Date.now() }) },
      ...userWidgets.map((w) => ({ id: `edit-panel:${w.id}`, group: "Creator mode", label: `Change the panel "${w.title}"`, keywords: "widget edit", run: () => editPanel(w.id) })));
    void paletteTick;
    a.push(...pluginActions().map((p) => ({ id: p.id, group: "Plugins", label: p.label, hint: p.hint, run: p.run })));
    a.push({ id: "update", group: "Window", label: "Check for updates", keywords: "upgrade version new", run: () => checkUpdate(true) });
    a.push({ id: "usage", group: "Window", label: "Usage: cost by day and model, cache, plan", keys: "⌘⇧U", keywords: "spend money tokens stats", run: () => setTimeout(() => setOverlay("usage")) });
    a.push({ id: "theme", group: "Window", label: "Light or dark", keys: "⌘⇧L", run: flipTheme },
      { id: "keys", group: "Window", label: "Keyboard shortcuts", keys: "⌘/", run: () => setTimeout(() => setOverlay("shortcuts")) },
      { id: "settings", group: "Window", label: "Settings: provider keys, data policy, router", keys: "⌘,", run: () => setTimeout(() => setOverlay("settings")) },
      { id: "setup", group: "Window", label: "Set up tazuna", hint: "what is set, what is left", keywords: "welcome first run onboarding", run: () => setTimeout(() => setOverlay("setup")) });
    return inTauri ? a : a.map((action) => ["theme", "keys"].includes(action.id) ? action : { ...action, unavailable: "Available in the desktop app" });
  };
  // a new conversation before its first message: the start screen, with the composer in its middle (design review 2,
  // prototype mockups/v3.html)
  // Not while a saved conversation's messages are on their way: the start screen showed for a moment before them, each
  // time one was opened (the user, 2026-11-20: "quickly flashes the main new chat page")
  const fresh = inTauri && !!view && !busy && !view.asking && !editing && !resuming && !view.blocks.some((b) => b.kind === "prompt");
  const freshError = fresh ? [...view.blocks].reverse().find((b) => b.kind === "notice" && b.error) : undefined;
  // the start page's ledger reads the run log again when the page shows and when the day turns: yesterday's numbers stood
  // there overnight as "Today", and runs made in the terminal never came (design review 11)
  const [day, setDay] = useState(0);
  useEffect(() => { if (fresh) core.usage().catch(() => {}); }, [fresh, active, day]);
  useEffect(() => { const t = setTimeout(() => setDay((n) => n + 1), new Date().setHours(24, 0, 1, 0) - Date.now()); return () => clearTimeout(t); }, [day]);
  // in a project, its own conversations; in a chat, everything by when it last ran
  const startRecent: StartItem[] = Object.entries(sessions).filter(([dir]) => !folder || dir === folder).flatMap(([dir, rows]) => rows.map((r) => ({ id: r.id, title: shortTitle(r.first, r.named) || "(no message)", full: titleFrom(r.first),
    place: dir === home ? "chat" : base(dir), dir: dir === home ? null : dir, at: r.last_at }))).sort((a, b) => b.at - a.at).slice(0, 3);
  const where = { label: folder ? base(folder) : "Chat", folder: !!folder, pickFolder: pick,
    choices: [{ label: "Chat", hint: "no folder: a general question", path: null as string | null }, ...projects.slice(0, 6).map((p) => ({ label: base(p), hint: p, path: p as string | null }))],
    pick: (path: string | null) => { if (path !== folder) open(path); } };
  // the terminal's panels and switches, typed in the box (or taken from its `/` menu), open the window's own. Before, the
  // menu held only /create and /agent here, and a typed `/mcp` went on to the agent (the user, 2026-11-20).
  const settingsAt = (tab: string) => { localStorage.setItem(TAB, tab); setOverlay("settings"); };
  const find = (q: string) => { setPaletteQ(q); setOverlay("palette"); };
  const windowCommands: Record<string, [string, () => void]> = inTauri ? {
    new: ["A fresh conversation", () => open(folder)],
    resume: ["Continue an earlier conversation", () => find("resume")],
    mcp: ["Your MCP servers for every model", () => settingsAt("Tools from MCP servers")],
    login: ["Save a provider's API key", () => settingsAt("Providers")],
    data: ["Which providers may see your code", () => settingsAt("Privacy and routing")],
    route: ["Whether general questions may go to a chat", () => settingsAt("Privacy and routing")],
    usage: ["Cost by day and model, cache, plan", () => setOverlay("usage")],
    keys: ["All keyboard shortcuts", () => setOverlay("shortcuts")],
    ...(folder ? {
      diff: ["Review the uncommitted changes", () => setReviewing({ scope: "uncommitted" })],
      hook: ["A command of yours after each turn that changed files", () => settingsAt("After each turn")],
    } satisfies Record<string, [string, () => void]> : {}),
  } : {};
  const composer = (
        <Composer key={active} readOnly={!inTauri} commands={windowCommands} where={fresh ? where : undefined} chat={inTauri && !folder} draftKey={draftKey} session={session} threadCost={usage.threadCost} busy={busy}
            onSend={inTauri ? (text, draft) => {
              if (line?.paid && paidKey && !localStorage.getItem(paidKey)) { setPaidAsk({ text, draft }); return; }
              sendNow(text, draft);
            } : undefined}
            below={claudeOut ? (
              // a conversation on Claude while it is signed out: every message would fail (design review 12)
              <div className="flex items-center text-[11.5px] text-faint">Claude isn't signed in<Sep />
                <button onClick={() => { setKeyFocus("Claude"); settingsAt("Providers"); }} className="font-medium text-accent-ink">Settings → Providers</button></div>
            ) : line && (line.paid || line.plan || line.low) && (
              <div className={`flex items-center gap-2 text-[11.5px] ${line.low ? "text-warn" : "text-faint"}`}>
                <span className="truncate" title="Where your next message goes and what it costs">→ {line.text}</span>
                {line.offer && <button onClick={() => pickModel(line.offer!)} className="font-medium text-accent-ink">Switch to {line.offer.label}</button>}
              </div>
            )}
            draft={draft}
            above={paidAsk != null ? (
              <div className="mx-auto mb-2 flex max-w-[700px] items-center gap-3 rounded-lg border border-line-strong bg-card px-3.5 py-2 text-[12.5px] text-ink-2">
                <span className="flex-1">This goes to <span className="text-ink">{choice?.label}</span> on {choice?.provider}, a paid API{line?.est != null ? `: about ${usd(line.est)} a message` : ""}. Paid spend stops at your cap per 24 hours (Settings).</span>
                <button onClick={() => { localStorage.setItem(paidKey, "1"); setPaidAsk(null); sendNow(paidAsk.text, paidAsk.draft); }} className={accentButton}>Send</button>
                <button onClick={() => { setDraft({ text: paidAsk.draft, n: Date.now() }); setPaidAsk(null); }} className={quietButton}>Cancel</button>
              </div>
            ) : editing ? (
              <div className="mx-auto mb-2 flex max-w-[700px] items-center gap-3 rounded-lg border border-line-strong bg-card px-3.5 py-2 text-[12.5px] text-ink-2">
                <span className="flex-1">Editing an earlier message: sending it goes back to before it, and the messages after it leave the conversation.
                  {editing.files > 0 && <label className="mt-1 flex items-center gap-2 text-ink"><input type="checkbox" checked={editing.keep} onChange={(e) => setEditing({ ...editing, keep: e.target.checked })} />
                    Also put back the {editing.files} file{editing.files === 1 ? "" : "s"} tazuna changed since</label>}</span>
                <button onClick={() => { setEditing(null); setDraft({ text: "", n: Date.now() }); }} className={quietButton}>Cancel</button>
              </div>
            ) : freshError?.kind === "notice" && (
              <div role="alert" className="mx-auto mb-2 max-w-[700px] text-[12.5px] text-bad">{readableError(freshError.text)}</div>
            )} onStop={stop}
            models={models} onModels={inTauri ? () => core.models() : undefined}
            onRefreshModelList={inTauri ? (p) => { core.refreshModelList(p).catch(() => {}); } : undefined}
            onPickModel={inTauri ? pickModel : undefined} onPickEffort={inTauri ? (e) => core.setEffort(e) : undefined}
            onAddKey={inTauri ? (p) => { setKeyFocus(p); settingsAt("Providers"); } : undefined}
            onMode={inTauri ? (m) => core.setMode(m) : undefined}
            onPickFiles={inTauri ? core.pickFiles : undefined} onPasteImage={inTauri ? core.pasteImage : undefined} onDrop={inTauri ? core.onDrop : undefined}
            complete={complete[active] ?? null} onComplete={inTauri ? () => { core.complete().catch(() => {}); } : undefined}
            onImage={inTauri ? (p) => core.imageData(p, folder ?? undefined) : undefined} />
  );
  const data = widgetData(view);
  // the header: the short title, then the first message in full where it is longer (variant A of mockups/v6.html)
  const firstAsked = titleFrom((view?.blocks.find((b) => b.kind === "prompt") as { text?: string } | undefined)?.text);
  const title = view ? shortTitle(firstAsked) || "New chat" : inTauri ? (failed ? "Could not start" : "New chat") : "Retry test fails on CI";
  // after the title, only what it left out: "— Can you find out why and fix it?", not the title's words again; nothing where
  // that cannot be said (a title without the polite opening repeated the whole message, design review 12)
  const said = title.replace(/…$/, "").toLowerCase(), at = firstAsked.toLowerCase().indexOf(said);
  const more = at < 0 ? "" : firstAsked.slice(at + said.length).replace(/^[\s.?!,;:]+/, "");
  const sidebar = <Sidebar active={inTauri ? listedId(threads.find((t) => t.tid === active)) ?? folder ?? "" : "t1"}
    chats={inTauri ? rowsOf(home ? sessions[home] : undefined) : demo.chats}
    projects={inTauri ? projects.map((p) => ({ name: isWorktree(p) ? `${base(p.replace(/[\\/][^\\/]+$/, ""))} / ${base(p)}` : base(p), path: p, open: p === folder, threads: rowsOf(sessions[p], isWorktree(p) ? `${base(p.replace(/[\\/][^\\/]+$/, ""))} · ${base(p)}` : base(p), p) })) : demo.projects}
    onNewChat={inTauri ? () => open(null) : undefined} onOpenFolder={inTauri ? pick : undefined} onOpenProject={inTauri ? (p) => p.path && open(p.path) : undefined}
    onOpenThread={inTauri ? (t, p) => open(p?.path ?? null, t.id) : undefined} onSearch={() => setOverlay("palette")} onSettings={inTauri ? () => setOverlay("settings") : undefined}
    open={inTauri ? threads.filter((t) => t.tid === active || t.view?.blocks.some((b) => b.kind === "prompt")).map((t) => ({ tid: t.tid, id: listedId(t), model: t.view?.model, title: titleOf(t), place: t.folder ? base(t.folder) : "chat",
      state: t.error ? "stopped" : t.view?.asking ? "asking" : busyView(t.view) ? "working" : "idle" })) : []}
    activeTid={active} onSwitch={show} onClose={close}
    onRename={inTauri ? (t, title) => core.rename(t.id, title).then(refreshSessions) : undefined}
    onHide={inTauri ? (t) => core.hide(t.id).then(refreshSessions) : undefined}
    onDismiss={narrow ? () => setNavOpen(false) : undefined}
    planLeft={view?.limits ? `${Math.round(100 - view.limits[0])}% left` : inTauri ? "" : "Max · 62% left"} />;
  return (
    // paper on the desk (design review 10): the window is the desk; the conversation (and a side chat or subagent beside it)
    // is a sheet on it; the sidebar, the work panel and the status bar lie on the desk
    <div className="flex h-screen flex-col overflow-hidden bg-desk">
    <div data-tauri-drag-region className="grid min-h-0 flex-1 overflow-hidden"
      // the right column is always a track (0 px when shut), so it slides: a track that came and went was a jump cut
      style={{ gridTemplateColumns: [narrow ? null : "248px", "minmax(0, 1fr)", reviewing ? "0px" : agentHere ? "440px" : side ? "380px" : panelOpen && !col.over ? "300px" : "0px"].filter(Boolean).join(" "), transition: "grid-template-columns .18s cubic-bezier(.2, 0, 0, 1)" }}>
      {narrow ? <dialog ref={navDialog} id="conversation-drawer" aria-label="Conversations"
        onCancel={(e) => { e.preventDefault(); setNavOpen(false); }}
        onClick={(e) => { if (e.target === e.currentTarget) setNavOpen(false); }}
        className="tz-enter fixed inset-y-0 left-0 m-0 h-dvh max-h-none w-[248px] max-w-none shadow-[var(--shadow-pop)] open:flex backdrop:bg-scrim">
        {sidebar}
      </dialog> : <div className="flex min-h-0">{sidebar}</div>}
      {/* relative: the review is a pane over the conversation (header and box too), not a dialog over the window. tz-keep: the
          reading column keeps the left edge centring gives it in the sheet with the panel shut (`col.left`), so an open
          panel beside it narrows it on the right and nothing jumps (design review 12, B3; styles.css .tz-col); a side chat or
          a subagent's sheet beside it centres it in what is left, as before */}
      <main className={`relative mr-2 mt-[calc(8*var(--tz-pt))] flex min-h-0 min-w-0 flex-col overflow-hidden rounded-xl bg-main shadow-sheet ${narrow ? "ml-2" : ""} ${side || agentHere ? "" : "tz-keep"}`}
        style={{ "--tz-l": `${col.left}px` } as CSSProperties}>
        {/* the window's top row (design review 12, A2): 40 points, centred on the traffic lights' line whatever the text size
            (`--tz-pt`, styles.css); in a narrow window on a Mac the lights sit on it, so it starts after them. A drag region,
            its title too */}
        <header data-tauri-drag-region="deep" className={`flex h-[calc(40*var(--tz-pt)+1px)] flex-none items-center justify-between gap-3 border-b border-line-soft pr-4 ${narrow && macOverlay ? "pl-[calc(92*var(--tz-pt)-8px)]" : "pl-7"}`}>
          <div className="flex min-w-0 flex-1 items-center gap-2 font-medium">
            {narrow && <button onClick={(e) => { e.currentTarget.focus(); setNavOpen(true); }} aria-label="Conversations" aria-haspopup="dialog" aria-controls="conversation-drawer" title="Conversations" className={`${macOverlay ? "" : "-ml-3"} grid size-7 flex-none place-items-center rounded-md text-muted hover:text-ink`}><PanelLeft size={16} strokeWidth={1.5} /></button>}
            <span className="min-w-0 max-w-[30%] truncate text-muted">{place}</span><span className="text-faint" aria-hidden="true">/</span><h1 key={`title:${active}`} title={firstAsked || title} className={`tz-enter truncate text-[inherit] font-[inherit] ${view && more ? "max-w-[60%] shrink-[0.3]" : ""} min-w-0`}>{title}</h1>
            {view && more && <span className="min-w-0 truncate text-[12px] font-normal text-faint" aria-hidden="true">— {more}</span>}</div>
          <div className="flex flex-none items-center gap-1.5">
            <HeaderCtx.Provider value={{ modelLabel: modelLabel(models?.choices.find((c) => c.current)?.model ?? session.model), effort: session.effort,
              branch: session.branch, changed: session.changed, review: inTauri && folder ? () => setReviewing({ scope: "uncommitted" }) : undefined,
              side: inTauri && view?.session_id ? () => (side ? closeSide(active) : openSide(active)) : undefined, sideOpen: !!side,
              pr: pr ? prChip(pr) : null, openPr: () => setShowPr(true), cost: usage.threadCost, panelOpen: panelOpen && !side && !agentHere, togglePanel: side || agentHere ? undefined : togglePanel }}>
              <Slot name="header.actions" props={{ data, folder }} />
            </HeaderCtx.Provider>
          </div>
        </header>
        {!inTauri && <div role="note" aria-label="Read-only demo" className="flex flex-none items-center gap-4 border-b border-line-soft bg-field/40 px-7 py-2 text-[12px] text-muted">
          <div className="min-w-0 flex-1"><span className="font-medium text-ink-2">Read-only demo</span><span className="block">Example conversation · no agent connected. Nothing is sent.</span></div>
          <button onClick={flipTheme} title="Switch light and dark theme" className="flex-none rounded-md border border-line px-2 py-1 text-ink-2 hover:bg-hover">Light / dark</button>
        </div>}
        <div key={`view:${active}`} className="tz-enter flex min-h-0 flex-1 flex-col">
        {failed ? <CoreFailed error={error!} folder={folder} onRetry={restart} onChat={() => open(null)} />
        : fresh ? <Start place={folder ? base(folder) : null} composer={composer} recent={startRecent} onResume={(r) => open(r.dir, r.id)} onFolder={pick} today={todayOf(rows)} onUsage={() => setOverlay("usage")}
            projects={projects.filter((p) => p !== folder).slice(0, 3).map((p) => ({ path: p, name: base(p), count: sessions[p]?.length ?? 0 }))} onProject={(p) => open(p)} />
        // keyed by the conversation, not its session: a change of model goes on in a new session with the same messages (core
        // C5), and the transcript stays where it was
        : <Transcript key={active} messages={messages} editFrom={editing?.cp}
          onEdit={inTauri ? (e, text) => {
            const files = new Set((view?.changes ?? []).filter((c) => c.at_block > e.block && !c.undone).flatMap((c) => c.files.map((f) => f.path))).size;
            setEditing({ ...e, files, keep: files > 0 }); setDraft({ text, n: Date.now() });
          } : undefined} busy={busy} activity={view?.asking ? undefined : view?.activity} asking={!!view?.asking} since={busy ? replyStart.current[startKey] : undefined} onSendToAgent={inTauri ? (text) => say(text) : undefined} onRetry={inTauri ? () => { core.retry().catch(() => {}); } : undefined}
          onImage={inTauri ? (p) => core.imageData(p, folder ?? undefined) : undefined}
          onDiff={inTauri ? (turn, path) => setReviewing({ scope: "turn", turn, path }) : undefined} onReview={inTauri ? (turn) => setReviewing({ scope: "turn", turn }) : undefined} onUndo={inTauri ? (turn) => core.undo(turn) : undefined} />}
        </div>
        {inTauri && col.tab && !side && !agentHere && !reviewing && !fresh && !failed && (
          <button data-panel onClick={togglePanel} aria-expanded={panelOpen} aria-label={`The work panel: ${changes} file${changes === 1 ? "" : "s"} changed`} title={`The work panel: changes, checks, the pull request (${keysOf("⌘\\")})`}
            className={`tz-enter absolute right-[var(--tz-sbw,0px)] top-[calc(40*var(--tz-pt)+13px)] z-[2] flex items-center gap-1.5 rounded-l-[7px] border border-r-0 border-line py-1 pl-2 pr-[9px] text-[12px] hover:text-ink ${panelOpen ? "bg-hover text-ink" : "bg-desk text-muted"}`}>
            <GitBranch size={13} strokeWidth={1.5} aria-hidden="true" />Changes<span className={`font-medium tabular-nums ${changes ? "text-ink-2" : "text-faint"}`}>{changes}</span></button>
        )}
        {diff && <DiffView diff={diff} onClose={() => setDiff(null)} />}
        {reviewing && <ReviewView start={reviewing} turns={(view?.changes ?? []).filter((c) => !c.undone).map((c) => c.turn)}
          onClose={() => setReviewing(null)} onSend={(text) => say(text)} />}
        {/* what stands over the message box, and the box: in the reading column (tz-col), the 700 px ones on the box's edges */}
        <div className="tz-col flex flex-none flex-col">
          {view?.untrusted && (
            <div className="mx-auto mb-2 flex w-full max-w-[756px] items-center gap-3 px-7 text-[12.5px] text-ink-2">
              <span className="flex-1">This folder's settings run code ({view.untrusted}). tazuna works here without them until you trust it.</span>
              <button onClick={() => core.trust().then(restart)} className={`${quietButton} font-medium`}>Trust folder</button>
            </div>
          )}
          {error && !failed && <div className="mx-auto mb-2 max-w-[756px] px-7 text-[12.5px] text-bad">{error}</div>}
          {folder && wtNotes[folder] && (
            <div className="mx-auto mb-2 flex w-full max-w-[756px] items-center gap-3 px-7 text-[12.5px] text-ink-2">
              <span className="flex-1">Worktree ready. {wtNotes[folder]}.</span>
              <button onClick={() => setWtNotes((m) => { const { [folder]: _, ...rest } = m; return rest; })} className={quietButton}>Dismiss</button>
            </div>
          )}
          {createAsk != null && (
            <div className="mx-auto mb-2 flex w-[calc(100%-56px)] max-w-[700px] flex-wrap items-center gap-2 rounded-lg border border-line-strong bg-card px-3.5 py-2 text-[12.5px] text-ink-2">
              <span className="flex-1">What should <span className="text-ink">“{createAsk.slice(0, 60)}”</span> be?</span>
              <button onClick={() => { createPanel(createAsk); setCreateAsk(null); }} className={quietButton} title="A panel in the right column, or a plugin that changes the window">Part of the window</button>
              <button onClick={() => { createBehaviour(createAsk); setCreateAsk(null); }} className={accentButton} title="A /command, a skill the agent reads, or a tool (MCP server): on every model">Something tazuna does</button>
              <button onClick={() => setCreateAsk(null)} className={quietButton}>Cancel</button>
            </div>
          )}
          {creation && creation.state !== "done" && (
            <div className="mx-auto mb-2 flex w-[calc(100%-56px)] max-w-[700px] items-center gap-3 rounded-lg border border-line-strong bg-card px-3.5 py-2 text-[12.5px] text-ink-2">
              <span className="flex-1">{creation.state === "made" ? "It is made and checked: what it is and where it is are in the conversation above. Read it, then turn it on." : "It did not pass the checks: the conversation says why."}</span>
              {creation.state === "made" && <button onClick={() => core.createDoneIn(active, true).catch(() => {})} className={accentButton}>Enable</button>}
              <button onClick={() => core.createDoneIn(active, false).catch(() => {})} className={quietButton}>Discard</button>
            </div>
          )}
          {wt && (
            <div className="mx-auto mb-2 flex w-[calc(100%-56px)] max-w-[700px] items-center gap-3 rounded-lg border border-line-strong bg-card px-3.5 py-2 text-[12.5px] text-ink-2">
              {wt.ask && <><span className="flex-1">This project has <span className="font-mono">.tazuna/setup.sh</span>. Run it in the new worktree (installs and the like)?
                {wt.script != null && <pre className="mt-1.5 max-h-60 overflow-auto whitespace-pre-wrap rounded bg-field px-2 py-1.5 font-mono text-[11.5px] text-ink">{wt.script}</pre>}
                <span className="mt-1 block text-[11.5px] text-muted">This is the whole script; exactly this runs.</span></span>
                <button onClick={() => core.worktreeNew(true, wt.id).catch(() => {})} className={accentButton}>Run it</button>
                <button onClick={() => core.worktreeNew(false).catch(() => {})} className={quietButton}>Skip</button></>}
              {wt.error && <span className="flex-1 text-bad">The worktree was not made: {wt.error}</span>}
              {wt.remove && <><span className="flex-1">{wt.remove.dirty ? `It has changes that are not committed: ${wt.remove.dirty.slice(0, 160)}` : "Remove this worktree? Its folder goes; its branch and commits stay."}</span>
                <button onClick={() => core.worktreeRemove(wt.remove!.path, !!wt.remove!.dirty).catch(() => {})} className={`${quietButton} font-medium text-bad!`}>{wt.remove.dirty ? "Remove anyway" : "Remove"}</button></>}
              <button onClick={() => setWt(null)} className={quietButton}>{wt.ask ? "Cancel" : "Close"}</button>
            </div>
          )}
          {inTauri && safeMode() && userWidgets.some((w) => w.kind === "plugin") && (
            <div className="mx-auto mb-2 max-w-[756px] px-7 text-[12.5px] text-muted">Safe mode: plugins are off (Settings turns them back on).</div>
          )}
          {ready[active] && (
            <div className="mx-auto mb-2 flex w-[calc(100%-56px)] max-w-[700px] items-center gap-3 rounded-lg border border-line-strong bg-card px-3.5 py-2.5 text-[12.5px] text-ink-2">
              <span className="flex-1"><span className="font-medium text-ink">{ready[active]!.title}</span> is ready. Its own code says it {ready[active]!.summary}.
                <span className="block text-[12px] text-muted">That is self-reported: read the code to be sure. A plugin runs inside tazuna's window and can send messages to the agent.</span></span>
              <button onClick={() => setCodeView({ title: ready[active]!.title, text: ready[active]!.code })}
                className={quietButton}>Code</button>
              <button onClick={() => enableReady(active)} className={accentButton}>Enable</button>
            </div>
          )}
          {codeView && <DiffView diff={{ turn: 0, path: `${codeView.title} / plugin.js`, text: codeView.text }} plain onClose={() => setCodeView(null)} />}
          {/* claude's work that goes on after the reply: a subagent or a command in the background. The conversation looked
              idle while one ran and spent the plan (the user, 2026-11-20); the terminal says "1 working in background" */}
          {tasks.some((t) => t.running) && (
            <div className="mx-auto mb-2 flex w-full max-w-[756px] flex-col gap-1 px-7">
              {tasks.filter((t) => t.running).map((t) => {
                const isAgent = t.kind === "local_agent" && t.calls.length > 0;
                return (
                  <button key={t.id} disabled={!isAgent} onClick={() => setAgent({ tid: active, call: t.calls[0] })} title={isAgent ? "See what it is doing: its reasoning, steps and answers" : undefined}
                    className="flex w-full items-center gap-2 rounded-lg border border-line bg-card px-3 py-1.5 text-left text-[12.5px] text-ink-2 disabled:cursor-default">
                    <LoaderCircle size={13} strokeWidth={1.75} className="flex-none animate-spin text-accent" aria-hidden="true" />
                    <span className="flex-none text-ink">{isAgent ? "Subagent" : "Command"} in the background</span>
                    <span className="min-w-0 flex-1 truncate text-muted">{t.doing || t.description}</span>
                    {t.tokens > 0 && <span className="flex-none tabular-nums text-faint">{(t.tokens / 1000).toFixed(1)}k tokens</span>}
                    {isAgent && <span className="flex-none text-accent-ink">Watch</span>}
                  </button>
                );
              })}
            </div>
          )}
          {view?.asking && <Approval asking={view.asking} input={askedInput(view)} onAnswer={(allow, always) => core.answer(allow, always)} />}
          {!fresh && !failed && composer}
        </div>
      </main>
      {agentHere ? <div className="flex min-h-0 justify-end overflow-hidden"><AgentPanel key={agentHere.call} view={agentViews[agentHere.call] ?? null} task={agentTask} running={agentRunning}
          title={agentTask?.description || String(agentStep?.input?.description ?? "")} onClose={() => setAgent(null)}
          onStop={agentTask ? () => { core.taskStopIn(agentHere.tid, agentTask.id).catch(() => {}); } : undefined} /></div>
        : side ? <SideChat key={side.tid} view={side.view} onClose={() => closeSide(active)} onStop={() => core.interruptIn(side.tid)}
          onSend={(text) => core.sendIn(side.tid, text).catch(() => {})} onAnswer={(allow, always) => core.opIn(side.tid, { op: "answer", allow, always })} />
        : panelOpen && !col.over && <div className="flex min-h-0 justify-end overflow-hidden"><WorkCtx.Provider value={work}><Rail usage={usage} data={data} messages={messages} /></WorkCtx.Provider></div>}
      {/* the panel over the conversation, under its tab (design review 12, B2 of mockups/v9.html): where beside it the text
          would be under 580 px; nothing under it moves */}
      {floating && <div ref={popRef} data-panel tabIndex={-1} aria-label="The work panel" className="tz-pop fixed right-5 top-[calc(48*var(--tz-pt)+47px)] z-30 flex max-h-[calc(100vh-48*var(--tz-pt)-88px)] w-[300px] flex-col overflow-hidden rounded-xl border border-line-strong outline-none">
        <WorkCtx.Provider value={work}><Rail usage={usage} data={data} messages={messages} over /></WorkCtx.Provider></div>}
      <Slot name="overlay" props={{ data }} />
      {overlay === "palette" && <Palette actions={actions()} query={paletteQ} onClose={() => setOverlay(null)} />}
      {overlay === "shortcuts" && <Shortcuts readOnly={!inTauri} onClose={() => setOverlay(null)} />}
      {bestOf && <BestOfStart choices={models?.choices ?? []} onStart={(task, ms) => { setBestOf(false); startLanes(task, ms); }} onClose={() => setBestOf(false)} />}
      {showLanes && lanes && <LanesView lanes={lanes} views={Object.fromEntries(threads.map((t) => [t.tid, t.view]))}
        onOpen={(tid) => { setShowLanes(false); show(tid); }} onKeep={keepLane} onClose={() => setShowLanes(false)} />}
      <Toaster />
      {showPr && inTauri && <PrView pr={pr ?? null} worktree={inWorktree} onClose={() => setShowPr(false)} onRefresh={() => core.prIn(active).catch(() => {})}
        onSend={(text) => { setShowPr(false); say(text); }} onRemoveWorktree={() => { setShowPr(false); setWt({ remove: { path: folder! } }); }} />}
      {overlay === "usage" && inTauri && <UsageView onClose={() => setOverlay(null)} limits={view?.limits} balance={view?.balance} choices={models?.choices} />}
      {overlay === "setup" && inTauri && <Setup onSettings={() => { localStorage.setItem(SETUP, "1"); localStorage.setItem("tazuna.settings.tab", "Providers"); setOverlay("settings"); }}
        onClose={() => { localStorage.setItem(SETUP, "1"); setOverlay(null); }} />}
      {overlay === "settings" && inTauri && <SettingsView onClose={() => setOverlay(null)} focus={keyFocus} items={userWidgets} onEdit={editPanel}
        onEnable={(w, on) => widgets.enable(w.id, w.print!, on).then(reloadWidgets).catch((e) => console.error(e))}
        onRemove={(id) => widgets.remove(id).then(reloadWidgets).catch(() => {})} onChanged={reloadWidgets} />}
    </div>
    {inTauri ? <StatusBar usage={usage} version={appVersion} idle={fresh || failed} accounts={accts} here={hereAccts} turns={turnsOf(messages)} busy={busy}
      onRefresh={haveView ? () => readAccounts() : undefined} refreshing={acct.busy} readAt={readAt} />
      : <footer aria-label="Demo status" className="flex h-7 flex-none items-center bg-desk px-3.5 text-[12px] text-muted">Example data · not live usage</footer>}
    </div>
  );
}

/** The conversation's finished turns for its receipt: each message of yours (made short, as the sidebar's titles are) with
 * what its reply cost and took. */
function turnsOf(messages: Message[]): Turn[] {
  const out: Turn[] = [];
  let asked = "";
  for (const m of messages) {
    if (m.role === "user") asked = m.text;
    else if (m.role === "agent" && m.meta) out.push({ ...m.meta, title: shortTitle(asked) || "(no message)", calls: m.meta.calls?.length, in: m.meta.in ?? 0, out: m.meta.out ?? 0 });
  }
  return out;
}

/** The cost of the last finished turn. */
function lastCost(v: View): number {
  for (let i = v.blocks.length - 1; i >= 0; i--) { const b = v.blocks[i]; if (b.kind === "done") return b.cost; }
  return 0;
}

/** Changes whenever a conversation's reply starts or ends (or one opens or closes): what is read again after a reply. */
const endedKey = (ts: Thread[]) => ts.map((t) => `${t.tid}:${busyView(t.view) ? 1 : 0}`).join(",");

/** The window's width, kept as it is resized. */
function useWidth() {
  const [w, setW] = useState(() => window.innerWidth);
  useEffect(() => { const on = () => setW(window.innerWidth); window.addEventListener("resize", on); return () => window.removeEventListener("resize", on); }, []);
  return w;
}

/** The files a conversation changed and has not undone, each once with its lines summed over the turns. */
function changedFiles(v: View | null): ChangedFile[] {
  const all = new Map<string, ChangedFile>();
  for (const c of v?.changes ?? []) {
    if (c.undone) continue;
    for (const f of c.files) {
      const o = all.get(f.path);
      all.set(f.path, o ? { ...f, added: o.added == null || f.added == null ? null : o.added + f.added, removed: o.removed == null || f.removed == null ? null : o.removed + f.removed } : { ...f });
    }
  }
  return [...all.values()];
}

const base = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

/** One of tazuna's worktrees (src/worktree.rs: <data>/worktrees/<repository>/wt-<name>). */
const isWorktree = (path: string) => /[\\/]worktrees[\\/][^\\/]+[\\/]wt-[a-z0-9]+$/.test(path);

const rowsOf = (rows?: SessionRow[], place = "chat", project?: string) => (rows ?? []).map((r) => ({ id: r.id, title: shortTitle(r.first, r.named) || "(no message)", full: titleFrom(r.first), place, project, at: r.last_at, model: r.model }));

/** An open conversation's name: its first message, else a new one's. */
const titleOf = (t: Thread) => shortTitle((t.view?.blocks.find((b) => b.kind === "prompt") as { text?: string } | undefined)?.text) || "New conversation";

/** The header's built-in chips read this (and a plugin replacing one can too). */
export const HeaderCtx = createContext<{ modelLabel: string; effort: string; branch: string; changed: number; review?: () => void; side?: () => void; sideOpen?: boolean;
  pr?: string | null; openPr?: () => void; cost?: number; panelOpen?: boolean; togglePanel?: (e?: { currentTarget: EventTarget | null }) => void }>({ modelLabel: "", effort: "", branch: "", changed: 0 });

function ModelChip() {
  const h = useContext(HeaderCtx);
  return <span className="rounded-full border border-line bg-field px-2.5 py-1 text-[12px] text-ink-2"><Parts items={[h.modelLabel, h.effort]} /></span>;
}
function BranchChip() {
  const h = useContext(HeaderCtx);
  return h.branch ? <span className="rounded-full border border-line px-2.5 py-1 text-[12px] text-ink-2"><Parts items={[h.branch, `${h.changed} changed`]} /></span> : null;
}
function ReviewChip() {
  const h = useContext(HeaderCtx);
  return h.review ? <button onClick={h.review} className="rounded-full border border-line px-2.5 py-1 text-[12px] text-ink-2 hover:bg-hover">Review</button> : null;
}
function SideChip() {
  const h = useContext(HeaderCtx);
  // an icon like the panel's button beside it (it was the header's one text pill)
  return h.side ? <button onClick={h.side} aria-pressed={h.sideOpen} aria-label="Side chat" title="Side chat: ask about this conversation; nothing in it goes back"
    className={`grid size-7 place-items-center rounded-md ${h.sideOpen ? "bg-field text-ink" : "text-muted hover:text-ink"}`}><MessagesSquare size={16} strokeWidth={1.5} /></button> : null;
}
slots.register({ id: "header.side", slot: "header.actions", order: 40, source: "tazuna", render: SideChip });
function PrChip() {
  const h = useContext(HeaderCtx);
  return h.pr ? <button onClick={h.openPr} title="The pull request: checks and reviews" className={`rounded-full border px-2.5 py-1 text-[12px] hover:bg-hover ${h.pr.includes("✗") ? "border-bad/40 text-bad" : "border-line text-ink-2"}`}>{h.pr}</button> : null;
}
// the header holds the thread's cost (the usage page on a click), the side chat and the work panel's switch; the model,
// branch, review and PR chips of desktop 0.1.63 and before went to the composer and the work panel (each shown once).
// ModelChip, BranchChip, ReviewChip and PrChip stay for a plugin that wants one back: slots.register({ ..., render: ModelChip }).
function CostChip() {
  const h = useContext(HeaderCtx);
  // nothing until there is a cost (a new chat showed $0.000)
  return h.cost ? <button onClick={openUsage} title={`This conversation's cost · Usage (${keysOf("⌘⇧U")})`} className="rounded-md px-2 py-1 text-[12.5px] tabular-nums text-muted hover:text-ink">{usd(h.cost)}</button> : null;
}
function PanelChip() {
  const h = useContext(HeaderCtx);
  return h.togglePanel ? <button data-panel onClick={h.togglePanel} aria-pressed={h.panelOpen} title={`${h.panelOpen ? "Close" : "Open"} the work panel (${keysOf("⌘\\")})`}
    className={`grid size-7 place-items-center rounded-md ${h.panelOpen ? "bg-field text-ink" : "text-muted hover:text-ink"}`}><PanelRight size={16} strokeWidth={1.5} /></button> : null;
}
// the cost moved to the status bar (desktop 0.1.71): CostChip stays exported for a plugin that wants it in the header
slots.register({ id: "header.panel", slot: "header.actions", order: 50, source: "tazuna", render: PanelChip });
export { BranchChip, CostChip, ModelChip, PrChip, ReviewChip };
