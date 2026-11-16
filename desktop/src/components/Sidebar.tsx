import { ChevronDown, ChevronRight, FolderOpen, Plus, Search } from "lucide-react";
import type { Project, Thread } from "../lib/types";

const icon = { size: 16, strokeWidth: 1.5 };

function Kbd({ children }: { children: string }) {
  return <kbd className="ml-auto rounded-[5px] border border-line px-[5px] py-px font-mono text-[11px] text-faint">{children}</kbd>;
}

function ThreadRow({ t, active, sub, onClick }: { t: Thread; active: boolean; sub?: boolean; onClick?: () => void }) {
  return (
    <a onClick={onClick} title={t.title} className={`flex items-center gap-2 truncate rounded-md px-[9px] py-1.5 text-ink-2 transition-colors duration-150 hover:bg-hover ${sub ? "pl-7" : ""} ${active ? "bg-selected text-ink" : ""}`}>
      {sub && <span className={`size-1.5 flex-none rounded-full ${t.running ? "bg-accent shadow-[0_0_0_3px_var(--accent-soft)]" : "bg-faint"}`} />}
      <span className="truncate">{t.title}</span>
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
};

export function Sidebar({ chats, projects, active, planLeft, onNewChat, onOpenFolder, onOpenProject, onOpenThread, onSearch }: Props) {
  return (
    <aside className="flex flex-col gap-0.5 overflow-hidden border-r border-line bg-sidebar px-2.5 py-3.5">
      {macOverlay && <div data-tauri-drag-region className="-mx-2.5 -mt-3.5 h-[38px] flex-none" />}
      <div data-tauri-drag-region className="flex items-center gap-2 px-2 pb-3 pt-0.5 font-semibold tracking-[-0.01em]">
        <span className="grid size-[22px] place-items-center rounded-sm bg-accent text-[12px] font-bold text-on-accent">手</span>
        <span>tazuna</span>
      </div>
      <button onClick={onSearch} className="mb-1 flex items-center gap-2 rounded-md border border-line bg-field px-[9px] py-[7px] text-left text-muted hover:bg-hover">
        <Search {...icon} /><span>Search</span><Kbd>⌘K</Kbd>
      </button>
      <button onClick={onNewChat} className="flex items-center gap-2 rounded-md px-[9px] py-[7px] text-left text-ink transition-colors hover:bg-hover">
        <Plus {...icon} /><span>New chat</span><Kbd>⌘N</Kbd>
      </button>
      <button onClick={onOpenFolder} className="flex items-center gap-2 rounded-md px-[9px] py-[7px] text-left text-ink transition-colors hover:bg-hover">
        <FolderOpen {...icon} /><span>Open folder</span><Kbd>⌘O</Kbd>
      </button>
      {chats.length > 0 && <div className="px-[9px] pb-1 pt-3.5 text-[11.5px] font-medium text-faint">Chats</div>}
      <div className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto">
      {chats.slice(0, 15).map((t) => <ThreadRow key={t.id} t={t} active={t.id === active} onClick={() => onOpenThread?.(t)} />)}
      {projects.length > 0 && <div className="px-[9px] pb-1 pt-3.5 text-[11.5px] font-medium text-faint">Projects</div>}
      {projects.map((p) => (
        <div key={p.path ?? p.name}>
          <a onClick={() => onOpenProject?.(p)} title={p.path}
            className={`flex items-center gap-2 rounded-md px-[9px] py-1.5 text-ink-2 hover:bg-hover ${p.path && p.path === active ? "bg-selected text-ink" : ""}`}>
            {p.threads.length > 0 && (p.open ? <ChevronDown size={14} strokeWidth={1.5} /> : <ChevronRight size={14} strokeWidth={1.5} />)}
            <span className="flex-1 truncate">{p.name}</span>
            {p.pending ? <span className="rounded-full bg-accent px-1.5 text-[10.5px] font-semibold leading-[17px] text-on-accent">{p.pending}</span> : null}
          </a>
          {p.open && p.threads.slice(0, 15).map((t) => <ThreadRow key={t.id} t={t} active={t.id === active} sub onClick={() => onOpenThread?.(t, p)} />)}
        </div>
      ))}
      </div>
      <div className="mt-auto flex items-center gap-2 border-t border-line px-2 pt-2.5 text-muted">
        <span className="grid size-[22px] place-items-center rounded-full border border-line bg-field text-[11px] text-ink-2">S</span>
        <span>Settings</span>
        <span className="ml-auto text-[11.5px] text-faint">{planLeft}</span>
      </div>
    </aside>
  );
}
