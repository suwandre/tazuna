import { createContext, useContext, useEffect, useRef, useState } from "react";
import { Approval } from "./components/Approval";
import { Composer } from "./components/Composer";
import { DiffView } from "./components/DiffView";
import { ReviewView } from "./components/ReviewView";
import { Palette, Shortcuts, type Action } from "./components/Palette";
import { creatorMessage, dryRun, summary, widgetData, widgetId, widgets, type UserWidget } from "./lib/widgets";
import { footprint, load, onPaletteChange, pluginActions, safeMode, setActions, setData, unload } from "./lib/plugins";
import { SettingsView } from "./components/SettingsView";
import { Rail, UserPanel } from "./components/Rail";
import { Slot, slots } from "./lib/slots";
import { Sidebar } from "./components/Sidebar";
import { Transcript } from "./components/Transcript";
import * as demo from "./lib/demo";
import { messages as viewMessages, usage as viewUsage } from "./lib/fromView";
import { core, inTauri, menuKeys, type Diff, type Models, type SessionRow, type View } from "./lib/tazuna";

const RECENT = "tazuna.projects";
const recent = (): string[] => { try { return JSON.parse(localStorage.getItem(RECENT) ?? "[]"); } catch { return []; } };

/** An open conversation: its own core, folder (null: a chat) and latest view. */
export type Thread = { tid: string; folder: string | null; view: View | null; error: string | null; used: number;
  /** creator mode: this conversation builds ("new") or changes ("edit") a panel in its folder */
  creator?: "new" | "edit" };
const busyView = (v: View | null) => !!v && !["Ready", "Done"].includes(v.activity);
// idle open conversations kept running (each holds a claude process): more close, oldest first; they stay resumable
const KEEP_OPEN = 4;

/** The open conversations (each its own core, so one can keep working while another is on screen) and the one on
 * screen. Outside the Tauri window there is no core (the demo data shows instead). */
function useCore() {
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
  const activeRef = useRef("");
  activeRef.current = active;
  const patch = (tid: string, p: Partial<Thread>) => setThreads((t) => (t[tid] ? { ...t, [tid]: { ...t[tid], ...p } } : t));
  const show = (tid: string) => { core.use(tid); setActiveTid(tid); setModels(null); patch(tid, { used: Date.now() }); };
  // a conversation out of sight that finishes or asks for an answer says so with a system notification
  const was = useRef<Record<string, string>>({});
  const tell = (tid: string, v: View) => {
    const now = v.asking ? "asking" : busyView(v) ? "working" : "idle", before = was.current[tid];
    was.current[tid] = now;
    if (now === before || (tid === activeRef.current && document.hasFocus())) return;
    const title = (v.blocks.find((b) => b.kind === "prompt") as { text?: string } | undefined)?.text?.split("\n")[0] ?? "A conversation";
    if (now === "asking") core.notify("tazuna asks", `${title}: ${v.asking!.tool.replace(/^mcp__lean__/, "")} ${v.asking!.what}`).catch(() => {});
    else if (now === "idle" && before === "working") core.notify("tazuna is done", title).catch(() => {});
  };
  /** Too many idle conversations open: the oldest idle ones close (not the one on screen, nothing working or asking). */
  const trim = (keep: string) => {
    const idle = Object.values(live.current).filter((t) => t.tid !== keep && !busyView(t.view) && !t.view?.asking).sort((a, b) => b.used - a.used);
    for (const t of idle.slice(KEEP_OPEN - 1)) close(t.tid);
  };
  const close = (tid: string) => {
    core.stop(tid).catch(() => {});
    setThreads((t) => { const { [tid]: _, ...rest } = t; return rest; });
  };
  /** A new conversation in a folder (null: a chat), or `resume` an earlier one: on screen at once, in a thread of its
   * own; an earlier conversation already open is shown, not opened twice. */
  const open = (path: string | null, resume?: string, opts: { creator?: "new" | "edit"; then?: (tid: string) => void } = {}) => {
    if (path && !opts.creator) setProjects((p) => { const next = [path, ...p.filter((x) => x !== path)].slice(0, 12); localStorage.setItem(RECENT, JSON.stringify(next)); return next; });
    const already = !opts.creator && Object.values(live.current).find((t) => resume ? t.view?.session_id === resume
      : t.folder === path && !t.creator && !t.view?.blocks.some((b) => b.kind === "prompt") && !t.error);
    if (already) { show(already.tid); return; }
    // randomUUID needs a secure page, which some WebKit builds do not count the app's own pages as
    const tid = crypto.randomUUID?.() ?? `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
    setThreads((t) => ({ ...t, [tid]: { tid, folder: path, view: null, error: null, used: Date.now(), creator: opts.creator } }));
    show(tid);
    core.start(tid, path ?? undefined, path ? undefined : "chat")
      .then((dir) => { if (!path) setHome(dir); if (resume) return core.resumeIn(tid, resume); opts.then?.(tid); })
      .catch((e) => patch(tid, { error: String(e) }));
    trim(tid);
  };
  useEffect(() => {
    if (!inTauri) return;
    const offs = [
      core.onExit((tid) => patch(tid, { error: "tazuna stopped. The app needs tazuna 0.1.128 or later (it carries its own; TAZUNA_BIN overrides)." })),
      core.onView((v, tid) => { tell(tid, v); patch(tid, { view: v, error: null }); }),
      core.onModels((m, tid) => { if (tid === activeRef.current) setModels(m); }),
      core.onDiff(setDiff),
      core.onSessions((s) => setSessions((old) => ({ ...old, ...Object.fromEntries(s.dirs.map((d) => [d.dir, d.list])) }))),
    ];
    Promise.all(offs).then(() => open(null)).catch(() => {});
    return () => offs.forEach((p) => p.then((off) => off()));
  }, []);
  const pick = () => core.pickFolder().then((p) => p && open(p)).catch(() => {});
  // Cmd-N a new chat, Cmd-O a folder
  useEffect(() => {
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
  // the lists again after any conversation finishes a turn (the run log has it then) and on another folder
  const turns = Object.values(threads).map((t) => t.view?.turns ?? 0).join(",");
  const refreshSessions = () => {
    const dirs = [home, ...projects].filter((d): d is string => !!d);
    if (inTauri && dirs.length) core.sessions(dirs).catch(() => {});
  };
  useEffect(() => { if (view) refreshSessions(); }, [turns, folder, home, projects.length]);
  const list = Object.values(threads).sort((a, b) => b.used - a.used);
  /** The thread on screen started again in its folder (after trusting the folder: its settings load at start). */
  const restart = () => {
    const t = live.current[activeRef.current];
    if (!t) return;
    patch(t.tid, { view: null, error: null });
    core.start(t.tid, t.folder ?? undefined, t.folder ? undefined : "chat").catch((e) => patch(t.tid, { error: String(e) }));
  };
  return { view, error, folder, projects, open, pick, models, sessions, home, diff, setDiff, refreshSessions, threads: list, active, show, close, restart, creator: cur?.creator };
}

function useTheme() {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const [dark, setDark] = useState(media.matches);
  useEffect(() => {
    const on = (e: MediaQueryListEvent) => setDark(e.matches);
    media.addEventListener("change", on);
    return () => media.removeEventListener("change", on);
  }, [media]);
  useEffect(() => { document.documentElement.classList.toggle("dark", dark); }, [dark]);
  // Cmd/Ctrl-Shift-L flips light and dark for this window
  useEffect(() => {
    const key = (e: KeyboardEvent) => { if (!menuKeys && (e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "l") setDark((d) => !d); };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);
  return () => setDark((d) => !d);
}

/** Cmd-K opens the palette, Cmd-/ the shortcuts. */
function useOverlay() {
  const [overlay, setOverlay] = useState<"palette" | "shortcuts" | "settings" | null>(null);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (menuKeys || !(e.metaKey || e.ctrlKey) || e.shiftKey) return;
      if (e.key === "k") { e.preventDefault(); setOverlay((o) => (o === "palette" ? null : "palette")); }
      if (e.key === "/") { e.preventDefault(); setOverlay((o) => (o === "shortcuts" ? null : "shortcuts")); }
      if (e.key === ",") { e.preventDefault(); setOverlay((o) => (o === "settings" ? null : "settings")); }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, []);
  return [overlay, setOverlay] as const;
}

export default function App() {
  const flipTheme = useTheme();
  const [overlay, setOverlay] = useOverlay();
  // "edit from here": the message being edited (its checkpoint, its block, the files tazuna changed since, whether they
  // go back too) and the text put in the box
  const [editing, setEditing] = useState<{ cp: number; block: number; files: number; keep: boolean } | null>(null);
  const [draft, setDraft] = useState<{ text: string; n: number }>();
  const [reviewing, setReviewing] = useState<{ scope: string; turn?: number } | null>(null);
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
    });
    return () => { off.then((f) => f()); };
  }, []);
  const { view, error, folder, projects, open, pick, models, sessions, home, diff, setDiff, refreshSessions, threads, active, show, close, restart, creator } = useCore();
  useEffect(() => setEditing(null), [active]);
  // creator mode: the user's panels, read again whenever a conversation finishes a turn (a creator one may have made one)
  const [userWidgets, setUserWidgets] = useState<UserWidget[]>([]);
  const [, setPluginTick] = useState(0);
  const loadedIds = useRef<string[]>([]);
  const [paletteTick, setPaletteTick] = useState(0);
  useEffect(() => onPaletteChange(() => setPaletteTick((n) => n + 1)), []);
  const allTurns = threads.map((t) => t.view?.turns ?? 0).join(",");
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
  const [ready, setReady] = useState<Record<string, { id: string; title: string; print: string; summary: string } | null>>({});
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
      const r = await dryRun(id);
      const said = summary(r.rec);
      if (r.error) problem = `It failed when run: ${r.error}`;
      else if (!said) problem = "It ran but registers nothing: no spot entry, no Cmd-K action, no style and no hidden part. Check the spot names against the specification.";
      else if (w.state !== "enabled") setReady((m) => ({ ...m, [t.tid]: { id, title: w.title, print: w.print!, summary: said } }));
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
      open(path, undefined, { creator: "edit", then: (tid) => { core.allowRootIn(tid, path).catch(() => {}); } });
      setDraft({ text: "Change this panel: ", n: Date.now() });
    }).catch(() => {});
  };
  const messages = view ? viewMessages(view) : inTauri ? [] : demo.messages;
  const usage = view ? viewUsage(view) : demo.usage;
  const busy = !!view && !["Ready", "Done"].includes(view.activity);
  const session = view
    ? { model: view.model || "default", effort: view.effort || "", mode: view.mode, branch: "", changed: 0, lastCost: lastCost(view) }
    : demo.session;
  const place = inTauri ? (folder ? base(folder) : "Chat") : "api-gateway";
  const actions = (): Action[] => {
    const a: Action[] = [
      { id: "new", group: "Start", label: "New chat", keys: "⌘N", run: () => open(null) },
      { id: "folder", group: "Start", label: "Open folder…", keys: "⌘O", run: pick },
      ...projects.map((p) => ({ id: `p:${p}`, group: "Projects", label: base(p), hint: p, keywords: "open project folder", run: () => open(p) })),
      ...[...(home ? (sessions[home] ?? []).map((r) => [r, null] as const) : []), ...projects.flatMap((p) => (sessions[p] ?? []).slice(0, 10).map((r) => [r, p] as const))]
        .map(([r, dir]) => ({ id: `s:${r.id}`, group: "Conversations", label: r.first.split("\n")[0] || "(no message)", hint: dir ? base(dir) : "chat", keywords: "resume continue", run: () => open(dir, r.id) })),
    ];
    if (busy) a.unshift({ id: "stop", group: "This conversation", label: "Stop the reply", keys: "Esc", run: () => core.interrupt() });
    const undo = view?.changes?.find((c) => c.can_undo);
    if (undo && !busy) a.push({ id: "undo", group: "This conversation", label: `Undo the last turn's changes`, hint: `${undo.files.length} file${undo.files.length === 1 ? "" : "s"}`, run: () => core.undo(undo.turn) });
    if (view && folder) a.push(
      { id: "review-uncommitted", group: "Review", label: "Review uncommitted changes", keywords: "diff stage revert hunk", run: () => setReviewing({ scope: "uncommitted" }) },
      { id: "review-branch", group: "Review", label: "Review the branch against its base", keywords: "diff pr", run: () => setReviewing({ scope: "branch" }) });
    if (view) {
      a.push(...["auto", "ask", "edits", "plan"].map((m) => ({ id: `m:${m}`, group: "Permission mode", label: m, hint: m === view.mode ? "current" : undefined, keywords: "mode permission", run: () => core.setMode(m) })));
      const cur = models?.choices.find((c) => c.current);
      a.push(...(cur?.efforts ?? []).map((e) => ({ id: `e:${e}`, group: "Effort", label: e, hint: e === view.effort ? "current" : undefined, keywords: "effort thinking", run: () => core.setEffort(e) })));
      a.push(...(models?.choices ?? []).filter((c) => !c.why_not).map((c) => ({ id: `mo:${c.model}|${c.url}|${c.key_env}|${c.label}`, group: "Model", label: c.label, hint: c.current ? "current" : c.place, keywords: `model ${c.provider}`, run: () => core.setModel(c) })));
    }
    a.push({ id: "create-panel", group: "Creator mode", label: "Create a panel…", hint: "/create what it should show", keywords: "widget creator customize",
        run: () => setDraft({ text: "/create ", n: Date.now() }) },
      ...userWidgets.map((w) => ({ id: `edit-panel:${w.id}`, group: "Creator mode", label: `Change the panel "${w.title}"`, keywords: "widget edit", run: () => editPanel(w.id) })));
    void paletteTick;
    a.push(...pluginActions().map((p) => ({ id: p.id, group: "Plugins", label: p.label, hint: p.hint, run: p.run })));
    a.push({ id: "theme", group: "Window", label: "Light or dark", keys: "⌘⇧L", run: flipTheme },
      { id: "keys", group: "Window", label: "Keyboard shortcuts", keys: "⌘/", run: () => setTimeout(() => setOverlay("shortcuts")) },
      { id: "settings", group: "Window", label: "Settings: provider keys, data policy, router", keys: "⌘,", run: () => setTimeout(() => setOverlay("settings")) });
    return a;
  };
  const data = widgetData(view);
  const title = view ? (view.blocks.find((b) => b.kind === "prompt") as { text?: string } | undefined)?.text?.slice(0, 60) ?? "New chat" : "Retry test fails on CI";
  return (
    <div className="grid h-screen grid-cols-[248px_1fr_232px] overflow-hidden">
      <Sidebar active={inTauri ? view?.session_id ?? folder ?? "" : "t1"}
        chats={inTauri ? rowsOf(home ? sessions[home] : undefined) : demo.chats}
        projects={inTauri ? projects.map((p) => ({ name: base(p), path: p, open: p === folder, threads: rowsOf(sessions[p]) })) : demo.projects}
        onNewChat={() => open(null)} onOpenFolder={pick} onOpenProject={(p) => p.path && open(p.path)}
        onOpenThread={(t, p) => open(p?.path ?? null, t.id)} onSearch={() => setOverlay("palette")} onSettings={() => setOverlay("settings")}
        open={inTauri ? threads.map((t) => ({ tid: t.tid, title: titleOf(t), place: t.folder ? base(t.folder) : "chat",
          state: t.error ? "stopped" : t.view?.asking ? "asking" : busyView(t.view) ? "working" : "idle" })) : []}
        activeTid={active} onSwitch={show} onClose={close}
        onRename={inTauri ? (t, title) => core.rename(t.id, title).then(refreshSessions) : undefined}
        onHide={inTauri ? (t) => core.hide(t.id).then(refreshSessions) : undefined}
        planLeft={view?.limits ? `${Math.round(100 - view.limits[0])}% left` : inTauri ? "" : "Max · 62% left"} />
      <main className="flex min-h-0 min-w-0 flex-col bg-main">
        <header data-tauri-drag-region className="flex items-center justify-between border-b border-line-soft px-7 py-3">
          <div className="flex items-center gap-2 font-medium"><span className="text-muted">{place}</span><span className="text-faint">/</span><span className="truncate">{title}</span></div>
          <div className="flex gap-1.5">
            <HeaderCtx.Provider value={{ modelLabel: models?.choices.find((c) => c.current)?.label ?? session.model, effort: session.effort,
              branch: session.branch, changed: session.changed, review: inTauri && folder ? () => setReviewing({ scope: "uncommitted" }) : undefined }}>
              <Slot name="header.actions" props={{ data, folder }} />
            </HeaderCtx.Provider>
          </div>
        </header>
        <Transcript key={`${active}:${view?.session_id ?? ""}`} messages={messages} editFrom={editing?.cp}
          onEdit={inTauri ? (e, text) => {
            const files = new Set((view?.changes ?? []).filter((c) => c.at_block > e.block && !c.undone).flatMap((c) => c.files.map((f) => f.path))).size;
            setEditing({ ...e, files, keep: files > 0 }); setDraft({ text, n: Date.now() });
          } : undefined} busy={busy} activity={view?.asking ? undefined : view?.activity} onSendToAgent={(text) => core.send(text, true)}
          onDiff={(turn, path) => core.diff(turn, path)} onReview={inTauri ? (turn) => setReviewing({ scope: "turn", turn }) : undefined} onUndo={(turn) => core.undo(turn)} />
        {diff && <DiffView diff={diff} onClose={() => setDiff(null)} />}
        {reviewing && <ReviewView start={reviewing} turns={(view?.changes ?? []).filter((c) => !c.undone).map((c) => c.turn)}
          onClose={() => setReviewing(null)} onSend={(text) => core.send(text, true)} />}
        {view?.untrusted && (
          <div className="mx-auto mb-2 flex w-full max-w-[700px] items-center gap-3 px-7 text-[12.5px] text-ink-2">
            <span className="flex-1">This folder's settings run code ({view.untrusted}). tazuna works here without them until you trust it.</span>
            <button onClick={() => core.trust().then(restart)} className="rounded-md border border-line-strong px-2.5 py-1 font-medium text-ink hover:bg-hover">Trust folder</button>
          </div>
        )}
        {error && <div className="mx-auto mb-2 max-w-[700px] px-7 text-[12.5px] text-bad">{error}</div>}
        {inTauri && safeMode() && userWidgets.some((w) => w.kind === "plugin") && (
          <div className="mx-auto mb-2 max-w-[700px] px-7 text-[12.5px] text-muted">Safe mode: plugins are off (Settings turns them back on).</div>
        )}
        {ready[active] && (
          <div className="mx-auto mb-2 flex w-full max-w-[700px] items-center gap-3 rounded-lg border border-line-strong bg-card px-3.5 py-2.5 text-[12.5px] text-ink-2">
            <span className="flex-1"><span className="font-medium text-ink">{ready[active]!.title}</span> is ready: it {ready[active]!.summary}.
              <span className="block text-[12px] text-muted">A plugin runs inside tazuna's window and can send messages to the agent: enable it if you want it.</span></span>
            <button onClick={() => widgets.code(ready[active]!.id).then((text) => setCodeView({ title: ready[active]!.title, text })).catch(() => {})}
              className="rounded-md border border-line px-2.5 py-1 text-ink-2 hover:bg-hover">Code</button>
            <button onClick={() => enableReady(active)} className="rounded-md bg-accent px-2.5 py-1 font-medium text-on-accent">Enable</button>
          </div>
        )}
        {codeView && <DiffView diff={{ turn: 0, path: `${codeView.title} · plugin.js`, text: codeView.text }} plain onClose={() => setCodeView(null)} />}
        {view?.asking && <Approval asking={view.asking} onAnswer={(allow, always) => core.answer(allow, always)} />}
        <Composer session={session} threadCost={usage.threadCost} busy={busy}
          onSend={(text) => {
            if (text.startsWith("/create ")) { createPanel(text.slice(8).trim()); return; }
            if (creator === "edit" && !view?.blocks.some((b) => b.kind === "prompt")) { core.send(creatorMessage(text, true), true).catch(() => {}); return; }
            if (!editing) { core.send(text).catch((e) => console.error(e)); return; }
            const { cp, keep } = editing;
            setEditing(null);
            core.rewind(cp, keep).then(() => core.send(text)).catch((e) => console.error(e));
          }}
          draft={draft}
          above={editing && (
            <div className="mx-auto mb-2 flex max-w-[700px] items-center gap-3 rounded-lg border border-line-strong bg-card px-3.5 py-2 text-[12.5px] text-ink-2">
              <span className="flex-1">Editing an earlier message: sending it goes back to before it, and the messages after it leave the conversation.
                {editing.files > 0 && <label className="mt-1 flex items-center gap-2 text-ink"><input type="checkbox" checked={editing.keep} onChange={(e) => setEditing({ ...editing, keep: e.target.checked })} />
                  Also put back the {editing.files} file{editing.files === 1 ? "" : "s"} tazuna changed since</label>}</span>
              <button onClick={() => { setEditing(null); setDraft({ text: "", n: Date.now() }); }} className="rounded-md border border-line px-2.5 py-1 text-muted hover:bg-hover">Cancel</button>
            </div>
          )} onStop={() => core.interrupt()}
          models={models} onModels={inTauri ? () => core.models() : undefined}
          onPickModel={inTauri ? (c) => core.setModel(c) : undefined} onPickEffort={inTauri ? (e) => core.setEffort(e) : undefined}
          onMode={inTauri ? (m) => core.setMode(m) : undefined}
          onPickFiles={inTauri ? core.pickFiles : undefined} onDrop={inTauri ? core.onDrop : undefined} />
      </main>
      <Rail usage={usage} data={data} />
      <Slot name="overlay" props={{ data }} />
      {overlay === "palette" && <Palette actions={actions()} onClose={() => setOverlay(null)} />}
      {overlay === "shortcuts" && <Shortcuts onClose={() => setOverlay(null)} />}
      {overlay === "settings" && inTauri && <SettingsView onClose={() => setOverlay(null)} items={userWidgets} onEdit={editPanel}
        onEnable={(w, on) => widgets.enable(w.id, w.print!, on).then(reloadWidgets).catch((e) => console.error(e))}
        onRemove={(id) => widgets.remove(id).then(reloadWidgets).catch(() => {})} />}
    </div>
  );
}

/** The cost of the last finished turn. */
function lastCost(v: View): number {
  for (let i = v.blocks.length - 1; i >= 0; i--) { const b = v.blocks[i]; if (b.kind === "done") return b.cost; }
  return 0;
}

const base = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

const rowsOf = (rows?: SessionRow[]) => (rows ?? []).map((r) => ({ id: r.id, title: r.first.split("\n")[0] || "(no message)" }));

/** An open conversation's name: its first message, else a new one's. */
const titleOf = (t: Thread) => (t.view?.blocks.find((b) => b.kind === "prompt") as { text?: string } | undefined)?.text?.split("\n")[0] || "New conversation";

/** The header's built-in chips read this (and a plugin replacing one can too). */
export const HeaderCtx = createContext<{ modelLabel: string; effort: string; branch: string; changed: number; review?: () => void }>({ modelLabel: "", effort: "", branch: "", changed: 0 });

function ModelChip() {
  const h = useContext(HeaderCtx);
  return <span className="rounded-full border border-line bg-field px-2.5 py-1 text-[12px] text-ink-2">{h.modelLabel}{h.effort && ` · ${h.effort}`}</span>;
}
function BranchChip() {
  const h = useContext(HeaderCtx);
  return h.branch ? <span className="rounded-full border border-line px-2.5 py-1 text-[12px] text-ink-2">{h.branch} · {h.changed} changed</span> : null;
}
function ReviewChip() {
  const h = useContext(HeaderCtx);
  return h.review ? <button onClick={h.review} className="rounded-full border border-line px-2.5 py-1 text-[12px] text-ink-2 hover:bg-hover">Review</button> : null;
}
slots.register({ id: "header.model", slot: "header.actions", order: 10, source: "tazuna", render: ModelChip });
slots.register({ id: "header.branch", slot: "header.actions", order: 20, source: "tazuna", render: BranchChip });
slots.register({ id: "header.review", slot: "header.actions", order: 30, source: "tazuna", render: ReviewChip });
