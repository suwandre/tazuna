import { ChevronRight, FolderOpen, Pencil, Plus, Search, X } from "lucide-react";
import { createContext, useContext, useState } from "react";
import { Slot } from "../lib/slots";
import type { Project, Thread } from "../lib/types";

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
        className={`mx-[2px] rounded-md border border-line-strong bg-card px-[7px] py-[5px] text-ink outline-none ${sub ? "ml-6" : ""}`} />
    );
  }
  if (mode === "hide") {
    return (
      <div className={`flex items-center gap-2 rounded-md bg-field px-[9px] py-1.5 text-[12px] ${sub ? "pl-7" : ""}`}>
        <span className="flex-1 truncate text-ink-2">Remove from the list?</span>
        <button onClick={() => { onHide?.(); setMode("view"); }} className="font-medium text-bad">Remove</button>
        <button onClick={() => setMode("view")} className="text-muted">Cancel</button>
      </div>
    );
  }
  return (
    <a onClick={onClick} {...press(onClick)} title={t.title} className={`group flex items-center gap-2 truncate rounded-md px-[9px] py-1.5 text-ink-2 transition-colors duration-150 hover:bg-hover ${sub ? "pl-7" : ""} ${active ? "bg-selected text-ink" : ""}`}>
      {sub && <span className={`size-1.5 flex-none rounded-full ${t.running ? "bg-accent shadow-[0_0_0_3px_var(--accent-soft)]" : "bg-faint"}`} />}
      <span className="flex-1 truncate">{t.title}</span>
      {onRename && <span className="hidden flex-none gap-0.5 group-hover:flex">
        <button title="Rename" onClick={(e) => { e.stopPropagation(); setTitle(t.title); setMode("rename"); }} className={icon}><Pencil size={12} strokeWidth={1.5} /></button>
        <button title="Remove from the list (its recording stays)" onClick={(e) => { e.stopPropagation(); setMode("hide"); }} className={icon}><X size={12} strokeWidth={1.5} /></button>
      </span>}
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
  open?: { tid: string; title: string; place: string; state: "working" | "asking" | "idle" | "stopped" }[];
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
  return (
    <aside aria-label="Conversations" className="flex flex-col gap-0.5 overflow-hidden border-r border-line bg-sidebar px-2.5 py-3.5">
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
      {chats.length > 0 && <div className="px-[9px] pb-1 pt-3.5 text-[11.5px] font-medium text-faint">Chats</div>}
      <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto">
      {open.length > 1 && <div className="px-[9px] pb-1 pt-3.5 text-[11.5px] font-medium text-faint">Open now</div>}
      {open.length > 1 && open.map((o) => (
        <a key={o.tid} onClick={() => onSwitch?.(o.tid)} {...press(() => onSwitch?.(o.tid))} title={`${o.title} (${STATE[o.state][1]})`}
          className={`group flex items-center gap-2 rounded-md px-[9px] py-1.5 text-ink-2 hover:bg-hover ${o.tid === activeTid ? "bg-selected text-ink" : ""}`}>
          <span className={`size-1.5 flex-none rounded-full ${STATE[o.state][0]}`} />
          <span className="min-w-0 flex-1 truncate">{o.title}</span>
          <span className="flex-none text-[11px] text-faint group-hover:hidden">{o.state === "asking" ? "asks" : o.place}</span>
          <button title="Close (the conversation stays in the lists)" onClick={(e) => { e.stopPropagation(); onClose?.(o.tid); }}
            className="hidden flex-none rounded p-0.5 text-faint hover:bg-hover hover:text-ink group-hover:block"><X size={12} strokeWidth={1.5} /></button>
        </a>
      ))}
      {chats.slice(0, 15).map((t) => <ThreadRow key={t.id} t={t} active={t.id === active} onClick={() => onOpenThread?.(t)}
        onRename={onRename && ((title) => onRename(t, title))} onHide={onHide && (() => onHide(t))} />)}
      {projects.length > 0 && <div className="px-[9px] pb-1 pt-3.5 text-[11.5px] font-medium text-faint">Projects</div>}
      {projects.map((p) => {
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
  const { onSettings, planLeft } = useContext(SidebarCtx);
  return (
    <div className="border-t border-line pt-1.5">
      <button onClick={onSettings} className="flex w-full items-center gap-2 rounded-md px-[9px] py-[7px] text-left text-muted transition-colors hover:bg-hover hover:text-ink">
        <span className="grid size-[22px] place-items-center rounded-full border border-line bg-field text-[11px] text-ink-2">S</span>
        <span>Settings</span>
        <span className="ml-auto text-[11.5px] text-faint">{planLeft}</span>
      </button>
    </div>
  );
}
