import { ChevronRight, FolderOpen, LoaderCircle, Pencil, Plus, Search, X } from "lucide-react";
import { createContext, useContext, useState } from "react";
import { Slot } from "../lib/slots";
import type { Project, Thread } from "../lib/types";
import { modelLabel } from "../lib/types";

const icon = { size: 16, strokeWidth: 1.5 };

function Kbd({ children }: { children: string }) {
  return <kbd className="ml-auto rounded-[5px] border border-line px-[5px] py-px font-mono text-[11px] text-faint">{children}</kbd>;
}

/** A row that holds buttons of its own (rename, close) cannot be a button itself: it acts as one, for the keyboard and the
 * pointer (a hand, Enter and Space), as the window's other buttons do. */
const press = (go?: () => void) => ({
  role: "button", tabIndex: 0,
  onKeyDown: (e: React.KeyboardEvent) => { if ((e.key === "Enter" || e.key === " ") && e.target === e.currentTarget) { e.preventDefault(); go?.(); } },
});

/** "now", "5m", "3h", "2d", "Nov 3": how long since a conversation last ran. */
export function age(at?: number, now = Date.now() / 1000): string {
  if (!at) return "";
  const s = now - at;
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  if (s < 7 * 86400) return `${Math.floor(s / 86400)}d`;
  return new Date(at * 1000).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

const groupHead = "flex items-center px-[9px] pb-1 pt-3.5 text-[11.5px] font-medium text-faint";

function ThreadRow({ t, active, sub, onClick, onRename, onHide }: {
  t: Thread; active: boolean; sub?: boolean; onClick?: () => void; onRename?: (title: string) => void; onHide?: () => void;
}) {
  const [mode, setMode] = useState<"view" | "rename" | "hide">("view");
  const [title, setTitle] = useState(t.title);
  const icon = "rounded p-0.5 text-faint hover:bg-hover hover:text-ink";
  if (mode === "rename") {
    return (
      <input autoFocus value={title} onChange={(e) => setTitle(e.target.value)} onBlur={() => setMode("view")}
        onKeyDown={(e) => { if (e.key === "Enter" && title.trim()) { onRename?.(title.trim()); setMode("view"); } if (e.key === "Escape") setMode("view"); }}
        className={`mx-[2px] rounded-md border border-line-strong bg-card px-[7px] ${sub ? "ml-6 py-[5px]" : "py-[11px]"} text-ink outline-none`} />
    );
  }
  if (mode === "hide") {
    return (
      <div className={`flex items-center gap-2 rounded-md bg-field px-[9px] py-1.5 text-[12px] ${sub ? "pl-7" : ""}`}>
        <span className="flex-1 truncate text-ink-2" title="Remove from the list (its recording stays)">Remove?</span>
        <button onClick={() => { onHide?.(); setMode("view"); }} className="font-medium text-bad">Remove</button>
        <button onClick={() => setMode("view")} className="text-muted">Cancel</button>
      </div>
    );
  }
  const twoLines = !sub && (t.place || t.at);
  return (
    <a onClick={onClick} {...press(onClick)} title={t.title} className={`group flex items-center gap-2 truncate rounded-md px-[9px] ${twoLines ? "py-[5px]" : "py-1.5"} text-ink-2 transition-colors duration-150 hover:bg-hover ${sub ? "pl-7" : ""} ${active ? "bg-selected text-ink" : ""}`}>
      {sub && <span className={`size-1.5 flex-none rounded-full ${t.running ? "bg-accent shadow-[0_0_0_3px_var(--accent-soft)]" : "bg-faint"}`} />}
      {twoLines ? (
        <span className="min-w-0 flex-1">
          {/* the model by name on the second line: a letter in a box ("C", "G", "O") said nothing (design review 4) */}
          <span className="block truncate text-ink">{t.title}</span>
          <span className="flex gap-2 text-[11.5px] text-faint"><span className="truncate">{[t.place, modelLabel(t.model)].filter(Boolean).join(" · ")}</span><span className="ml-auto flex-none tabular-nums group-hover:hidden">{age(t.at)}</span></span>
        </span>
      ) : <span className="flex-1 truncate">{t.title}</span>}
      {onRename && <span className="hidden flex-none gap-0.5 group-hover:flex">
        <button title="Rename" onClick={(e) => { e.stopPropagation(); setTitle(t.title); setMode("rename"); }} className={icon}><Pencil size={12} strokeWidth={1.5} /></button>
        <button title="Remove from the list (its recording stays)" onClick={(e) => { e.stopPropagation(); setMode("hide"); }} className={icon}><X size={12} strokeWidth={1.5} /></button>
      </span>}
    </a>
  );
}

type Open = { tid: string; title: string; place: string; state: "working" | "asking" | "idle" | "stopped"; id?: string | null; model?: string };

/** A conversation open now: its name, then where it runs and what it needs; a spinner while it works. */
function OpenRow({ o, active, onSwitch, onClose }: { o: Open; active: boolean; onSwitch?: (tid: string) => void; onClose?: (tid: string) => void }) {
  return (
    <a onClick={() => onSwitch?.(o.tid)} {...press(() => onSwitch?.(o.tid))} title={`${o.title} (${STATE[o.state][1]})`}
      className={`group flex items-center gap-2 rounded-md px-[9px] py-[5px] text-ink-2 hover:bg-hover ${active ? "bg-selected text-ink" : ""}`}>
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-1.5 text-ink">
          {o.state === "working" ? <LoaderCircle size={12} strokeWidth={2} className="flex-none animate-spin text-accent" aria-hidden="true" />
            : o.state !== "asking" && <span className={`size-1.5 flex-none rounded-full ${STATE[o.state][0]}`} />}
          <span className="truncate">{o.title}</span>
          {o.state === "asking" && <span className="ml-auto flex-none rounded-full border border-warn/45 px-1.5 text-[10.5px] font-medium leading-4 text-warn">asks</span>}
        </span>
        <span className="block truncate text-[11.5px] text-faint">{[o.place, modelLabel(o.model)].filter(Boolean).join(" · ")}{o.state === "asking" ? " · waiting for your answer" : o.state === "stopped" ? " · stopped" : ""}</span>
      </span>
      <button title="Close (the conversation stays in the lists)" aria-label={`Close ${o.title}`} onClick={(e) => { e.stopPropagation(); onClose?.(o.tid); }}
        className="hidden flex-none rounded p-0.5 text-faint hover:bg-hover hover:text-ink group-hover:block"><X size={12} strokeWidth={1.5} /></button>
    </a>
  );
}

// in the Tauri window on macOS the title bar is an overlay: the traffic lights sit over the sidebar's top
const macOverlay = "__TAURI_INTERNALS__" in window && navigator.userAgent.includes("Mac");

type Props = {
  chats: Thread[]; projects: Project[]; active: string; planLeft: string;
  onNewChat?: () => void; onOpenFolder?: () => void; onOpenProject?: (p: Project) => void;
  /** a saved conversation: a chat (no project) or one of a project's */
  onOpenThread?: (t: Thread, p?: Project) => void;
  onSearch?: () => void;
  onSettings?: () => void;
  onRename?: (t: Thread, title: string) => void;
  onHide?: (t: Thread) => void;
  /** the conversations open now, each with its own core: working ones keep going while another is on screen */
  open?: Open[];
  activeTid?: string;
  onSwitch?: (tid: string) => void;
  onClose?: (tid: string) => void;
};

const STATE: Record<string, [string, string]> = {
  working: ["bg-accent shadow-[0_0_0_3px_var(--accent-soft)]", "working"],
  asking: ["bg-warn shadow-[0_0_0_3px_color-mix(in_srgb,var(--warn)_25%,transparent)]", "waiting for your answer"],
  idle: ["bg-faint", "idle"],
  stopped: ["bg-bad", "stopped"],
};

export function Sidebar({ chats, projects, active, planLeft, onNewChat, onOpenFolder, onOpenProject, onOpenThread, onSearch, onSettings, onRename, onHide, open = [], activeTid, onSwitch, onClose }: Props) {
  // the chevron folds and unfolds a project's conversations by itself (until 0.1.57 "open" was only "the active project":
  // a click opened it and nothing folded it again); the name opens the project
  const [fold, setFold] = useState<Record<string, boolean>>({});
  const needs = open.filter((o) => o.state === "asking"), working = open.filter((o) => o.state === "working");
  const idle = open.length > 1 ? open.filter((o) => o.state === "idle" || o.state === "stopped") : [];
  // an open conversation shows once, in its group above
  const opened = new Set(open.map((o) => o.id).filter(Boolean));
  const recent = [...chats.map((t) => ({ t, p: undefined as Project | undefined })), ...projects.flatMap((p) => p.threads.map((t) => ({ t, p })))]
    .filter(({ t }) => !opened.has(t.id)).sort((a, b) => (b.t.at ?? 0) - (a.t.at ?? 0)).slice(0, 20);
  return (
    // the column's whole width, whatever its rows: with a short list it shrank to them and left a strip of the window's
    // background between it and the conversation (desktop 0.1.75)
    <aside aria-label="Conversations" className="flex min-w-0 flex-1 flex-col gap-0.5 overflow-hidden border-r border-line bg-sidebar px-2.5 py-3.5">
      {macOverlay && <div data-tauri-drag-region className="-mx-2.5 -mt-3.5 h-[38px] flex-none" />}
      <div data-tauri-drag-region className="flex items-center gap-2 px-2 pb-3 pt-0.5 font-semibold tracking-[-0.01em]">
        <span className="grid size-[22px] place-items-center rounded-sm bg-accent text-[12px] font-bold text-on-accent">手</span>
        <span>tazuna</span>
      </div>
      <button onClick={onSearch} className="mb-1 flex items-center gap-2 rounded-md border border-line bg-field px-[9px] py-[7px] text-left text-muted hover:bg-hover">
        <Search {...icon} /><span>Search</span><Kbd>⌘K</Kbd>
      </button>
      <SidebarCtx.Provider value={{ onNewChat, onOpenFolder, onSettings, planLeft }}>
      <Slot name="sidebar.top" props={{}} />
      {/* rows keep their height and the list scrolls (*:shrink-0): a row's truncate hides its overflow, so in a full list the
          flex column squeezed every row to fit and they overlapped (desktop 0.1.64, with many saved conversations) */}
      <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto *:shrink-0">
      {/* the conversations open now by what they need: an answer from you, time to work, nothing (when more than one is open);
          then every saved one by when it last ran, its project and age under its name; then the projects (2026-11-18) */}
      {([["Needs you", needs], ["Working", working], ["Open", idle]] as const).map(([head, list]) => list.length > 0 && (
        <div key={head} className="flex flex-col gap-0.5">
          <div className={groupHead}><span className="flex-1">{head}</span><span className="tabular-nums">{list.length}</span></div>
          {list.map((o) => <OpenRow key={o.tid} o={o} active={o.tid === activeTid} onSwitch={onSwitch} onClose={onClose} />)}
        </div>
      ))}
      {recent.length > 0 && <div className={groupHead}>Recent</div>}
      {recent.map(({ t, p }) => <ThreadRow key={t.id} t={t} active={t.id === active} onClick={() => onOpenThread?.(t, p)}
        onRename={onRename && ((title) => onRename(t, title))} onHide={onHide && (() => onHide(t))} />)}
      {projects.length > 0 && <div className={groupHead}>Projects</div>}
      {projects.map((p) => {
        // the open project shows its conversations unless folded by hand
        const key = p.path ?? p.name, shown = fold[key] ?? p.open;
        return (
        <div key={key}>
          <div className={`flex items-center rounded-md text-ink-2 ${p.path && p.path === active ? "bg-selected text-ink" : ""}`}>
            {p.threads.length > 0 ? (
              <button onClick={() => setFold((f) => ({ ...f, [key]: !shown }))} aria-expanded={shown} aria-label={`${shown ? "Hide" : "Show"} ${p.name}'s conversations`}
                className="ml-1 rounded p-1 text-faint hover:bg-hover hover:text-ink">
                <ChevronRight size={14} strokeWidth={1.5} className={`transition-transform duration-150 ${shown ? "rotate-90" : ""}`} />
              </button>
            ) : <span className="ml-1 w-[22px]" />}
            <button onClick={() => { setFold((f) => ({ ...f, [key]: true })); onOpenProject?.(p); }} title={p.path}
              className="flex min-w-0 flex-1 items-center gap-2 rounded-md py-1.5 pl-1 pr-[9px] text-left hover:bg-hover">
              <span className="flex-1 truncate">{p.name}</span>
              {p.pending ? <span className="rounded-full bg-accent px-1.5 text-[10.5px] font-semibold leading-[17px] text-on-accent">{p.pending}</span> : null}
            </button>
          </div>
          {shown && p.threads.slice(0, 15).map((t) => <ThreadRow key={t.id} t={t} active={t.id === active} sub onClick={() => onOpenThread?.(t, p)}
            onRename={onRename && ((title) => onRename(t, title))} onHide={onHide && (() => onHide(t))} />)}
        </div>
        );
      })}
      </div>
      <div className="mt-auto"><Slot name="sidebar.footer" props={{ planLeft }} /></div>
      </SidebarCtx.Provider>
    </aside>
  );
}

/** The sidebar's actions for its built-in parts (and a plugin that replaces one). */
export const SidebarCtx = createContext<{ onNewChat?: () => void; onOpenFolder?: () => void; onSettings?: () => void; planLeft?: string }>({});

export function NewChatButton() {
  const { onNewChat } = useContext(SidebarCtx);
  return <button onClick={onNewChat} className="flex w-full items-center gap-2 rounded-md px-[9px] py-[7px] text-left text-ink transition-colors hover:bg-hover">
    <Plus {...icon} /><span>New chat</span><Kbd>⌘N</Kbd></button>;
}

export function OpenFolderButton() {
  const { onOpenFolder } = useContext(SidebarCtx);
  return <button onClick={onOpenFolder} className="flex w-full items-center gap-2 rounded-md px-[9px] py-[7px] text-left text-ink transition-colors hover:bg-hover">
    <FolderOpen {...icon} /><span>Open folder</span><Kbd>⌘O</Kbd></button>;
}


export function SettingsRow() {
  const { onSettings } = useContext(SidebarCtx);
  return (
    <div className="border-t border-line pt-1.5">
      <button onClick={onSettings} className="flex w-full items-center gap-2 rounded-md px-[9px] py-[7px] text-left text-muted transition-colors hover:bg-hover hover:text-ink">
        <span className="grid size-[22px] place-items-center rounded-full border border-line bg-field text-[11px] text-ink-2">S</span>
        <span>Settings</span>
        {/* planLeft stays in the context for a plugin; the plan's figures are by the composer now */}
        <Kbd>⌘,</Kbd>
      </button>
    </div>
  );
}
