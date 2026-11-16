import { useEffect, useRef, useState } from "react";
import { Approval } from "./components/Approval";
import { Composer } from "./components/Composer";
import { DiffView } from "./components/DiffView";
import { Palette, Shortcuts, type Action } from "./components/Palette";
import { SettingsView } from "./components/SettingsView";
import { Rail, builtInWidgets } from "./components/Rail";
import { Sidebar } from "./components/Sidebar";
import { Transcript } from "./components/Transcript";
import * as demo from "./lib/demo";
import { messages as viewMessages, usage as viewUsage } from "./lib/fromView";
import { core, inTauri, menuKeys, type Diff, type Models, type SessionRow, type View } from "./lib/tazuna";

const RECENT = "tazuna.projects";
const recent = (): string[] => { try { return JSON.parse(localStorage.getItem(RECENT) ?? "[]"); } catch { return []; } };

/** The core's latest view, in the Tauri window; null in a plain browser (the demo data shows instead). `folder` null is a
 * plain chat (home folder, chat profile); a path is a project, where the router sends general questions to a chat. */
function useCore() {
  const [view, setView] = useState<View | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [folder, setFolder] = useState<string | null>(null);
  const [projects, setProjects] = useState<string[]>(recent);
  const [models, setModels] = useState<Models | null>(null);
  const [diff, setDiff] = useState<Diff | null>(null);
  // saved conversations per folder (the home folder's are the chats), as the terminal's /resume lists them
  const [sessions, setSessions] = useState<Record<string, SessionRow[]>>({});
  const [home, setHome] = useState<string | null>(null);
  const running = useRef<string | null | undefined>(undefined); // the folder the core runs in (null: chat)
  /** A folder (null: a chat) with a new conversation, or `resume` an earlier one; the core restarts only for another folder. */
  const open = (path: string | null, resume?: string) => {
    setError(null);
    if (path) setProjects((p) => { const next = [path, ...p.filter((x) => x !== path)].slice(0, 12); localStorage.setItem(RECENT, JSON.stringify(next)); return next; });
    if (running.current === path && resume) { core.resume(resume).catch((e) => setError(String(e))); return; }
    setFolder(path); setView(null); running.current = path;
    core.start(path ?? undefined, path ? undefined : "chat")
      .then((dir) => { if (!path) setHome(dir); if (resume) return core.resume(resume); })
      .catch((e) => setError(String(e)));
  };
  useEffect(() => {
    if (!inTauri) return;
    const offs = [
      core.onExit(() => setError("tazuna stopped. The app needs tazuna 0.1.128 or later on the PATH (or TAZUNA_BIN).")),
      core.onView((v) => { setError(null); setView(v); }),
      core.onModels(setModels),
      core.onDiff(setDiff),
      core.onSessions((s) => setSessions((old) => ({ ...old, ...Object.fromEntries(s.dirs.map((d) => [d.dir, d.list])) }))),
    ];
    Promise.all(offs).then(() => open(null)).catch((e) => setError(String(e)));
    return () => offs.forEach((p) => p.then((off) => off()));
  }, []);
  const pick = () => core.pickFolder().then((p) => p && open(p)).catch((e) => setError(String(e)));
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
  // the catalog for the picker and the model's name: again after each switch, so the current row is right
  const model = view ? `${view.model}|${view.effort}` : "";
  useEffect(() => { if (inTauri && model) core.models().catch(() => {}); }, [model]);
  // the lists again after each finished turn (the run log has the session then) and on another folder
  const turns = view?.turns ?? -1;
  useEffect(() => {
    const dirs = [home, ...projects].filter((d): d is string => !!d);
    if (inTauri && view && dirs.length) core.sessions(dirs).catch(() => {});
  }, [turns, folder, home, projects.length]);
  return { view, error, folder, projects, open, pick, models, sessions, home, diff, setDiff };
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
  const { view, error, folder, projects, open, pick, models, sessions, home, diff, setDiff } = useCore();
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
    if (view) {
      a.push(...["auto", "ask", "edits", "plan"].map((m) => ({ id: `m:${m}`, group: "Permission mode", label: m, hint: m === view.mode ? "current" : undefined, keywords: "mode permission", run: () => core.setMode(m) })));
      const cur = models?.choices.find((c) => c.current);
      a.push(...(cur?.efforts ?? []).map((e) => ({ id: `e:${e}`, group: "Effort", label: e, hint: e === view.effort ? "current" : undefined, keywords: "effort thinking", run: () => core.setEffort(e) })));
      a.push(...(models?.choices ?? []).filter((c) => !c.why_not).map((c) => ({ id: `mo:${c.model}|${c.url}|${c.key_env}|${c.label}`, group: "Model", label: c.label, hint: c.current ? "current" : c.place, keywords: `model ${c.provider}`, run: () => core.setModel(c) })));
    }
    a.push({ id: "theme", group: "Window", label: "Light or dark", keys: "⌘⇧L", run: flipTheme },
      { id: "keys", group: "Window", label: "Keyboard shortcuts", keys: "⌘/", run: () => setTimeout(() => setOverlay("shortcuts")) },
      { id: "settings", group: "Window", label: "Settings: provider keys, data policy, router", keys: "⌘,", run: () => setTimeout(() => setOverlay("settings")) });
    return a;
  };
  const title = view ? (view.blocks.find((b) => b.kind === "prompt") as { text?: string } | undefined)?.text?.slice(0, 60) ?? "New chat" : "Retry test fails on CI";
  return (
    <div className="grid h-screen grid-cols-[248px_1fr_232px] overflow-hidden">
      <Sidebar active={inTauri ? view?.session_id ?? folder ?? "" : "t1"}
        chats={inTauri ? threads(home ? sessions[home] : undefined) : demo.chats}
        projects={inTauri ? projects.map((p) => ({ name: base(p), path: p, open: p === folder, threads: threads(sessions[p]) })) : demo.projects}
        onNewChat={() => open(null)} onOpenFolder={pick} onOpenProject={(p) => p.path && open(p.path)}
        onOpenThread={(t, p) => open(p?.path ?? null, t.id)} onSearch={() => setOverlay("palette")} onSettings={() => setOverlay("settings")}
        planLeft={view?.limits ? `${Math.round(100 - view.limits[0])}% left` : inTauri ? "" : "Max · 62% left"} />
      <main className="flex min-h-0 min-w-0 flex-col bg-main">
        <header data-tauri-drag-region className="flex items-center justify-between border-b border-line-soft px-7 py-3">
          <div className="flex items-center gap-2 font-medium"><span className="text-muted">{place}</span><span className="text-faint">/</span><span className="truncate">{title}</span></div>
          <div className="flex gap-1.5">
            <span className="rounded-full border border-line bg-field px-2.5 py-1 text-[12px] text-ink-2">{models?.choices.find((c) => c.current)?.label ?? session.model}{session.effort && ` · ${session.effort}`}</span>
            {session.branch && <span className="rounded-full border border-line px-2.5 py-1 text-[12px] text-ink-2">{session.branch} · {session.changed} changed</span>}
          </div>
        </header>
        <Transcript messages={messages} busy={busy} activity={view?.asking ? undefined : view?.activity} onSendToAgent={(text) => core.send(text, true)}
          onDiff={(turn, path) => core.diff(turn, path)} onUndo={(turn) => core.undo(turn)} />
        {diff && <DiffView diff={diff} onClose={() => setDiff(null)} />}
        {view?.untrusted && (
          <div className="mx-auto mb-2 flex w-full max-w-[700px] items-center gap-3 px-7 text-[12.5px] text-ink-2">
            <span className="flex-1">This folder's settings run code ({view.untrusted}). tazuna works here without them until you trust it.</span>
            <button onClick={() => core.trust().then(() => open(folder))} className="rounded-md border border-line-strong px-2.5 py-1 font-medium text-ink hover:bg-hover">Trust folder</button>
          </div>
        )}
        {error && <div className="mx-auto mb-2 max-w-[700px] px-7 text-[12.5px] text-bad">{error}</div>}
        {view?.asking && <Approval asking={view.asking} onAnswer={(allow, always) => core.answer(allow, always)} />}
        <Composer session={session} threadCost={usage.threadCost} busy={busy}
          onSend={(text) => core.send(text).catch((e) => console.error(e))} onStop={() => core.interrupt()}
          models={models} onModels={inTauri ? () => core.models() : undefined}
          onPickModel={inTauri ? (c) => core.setModel(c) : undefined} onPickEffort={inTauri ? (e) => core.setEffort(e) : undefined}
          onMode={inTauri ? (m) => core.setMode(m) : undefined}
          onPickFiles={inTauri ? core.pickFiles : undefined} onDrop={inTauri ? core.onDrop : undefined} />
      </main>
      <Rail usage={usage} widgets={builtInWidgets} />
      {overlay === "palette" && <Palette actions={actions()} onClose={() => setOverlay(null)} />}
      {overlay === "shortcuts" && <Shortcuts onClose={() => setOverlay(null)} />}
      {overlay === "settings" && inTauri && <SettingsView onClose={() => setOverlay(null)} />}
    </div>
  );
}

/** The cost of the last finished turn. */
function lastCost(v: View): number {
  for (let i = v.blocks.length - 1; i >= 0; i--) { const b = v.blocks[i]; if (b.kind === "done") return b.cost; }
  return 0;
}

const base = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

const threads = (rows?: SessionRow[]) => (rows ?? []).map((r) => ({ id: r.id, title: r.first.split("\n")[0] || "(no message)" }));
