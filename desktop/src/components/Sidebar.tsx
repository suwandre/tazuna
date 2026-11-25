import { ChevronRight, FolderOpen, LoaderCircle, Pencil, Plus, Search, Settings, X } from "lucide-react";
import { createContext, useContext, useEffect, useLayoutEffect, useRef, useState } from "react";
import { Slot } from "../lib/slots";
import type { Project, Thread } from "../lib/types";
import { Mark } from "./Mark";
import { Parts } from "./Sep";
import { clock } from "../lib/health";
import { keysOf } from "../lib/types";

const icon = { size: 16, strokeWidth: 1.5 };

/** A row's key. Only Search shows its own all the time; the others come with the row's hover or focus: eight key chips on
 * the start screen read as a cheat sheet (design review 9). The shortcut sheet (⌘/) has them all. */
function Kbd({ children, always }: { children: string; always?: boolean }) {
  return <kbd className={`ml-auto rounded-[5px] border border-line px-[5px] py-px font-mono text-[11px] text-faint ${always ? "" : "opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"}`}>{keysOf(children)}</kbd>;
}

/** A row that holds buttons of its own (rename, close) cannot be a button itself: it acts as one, for the keyboard and the
 * pointer (a hand, Enter and Space), as the window's other buttons do. */
/** A row's own buttons (rename, close): there with the pointer on the row or the keyboard's focus on it or in them. Out of
 * the row's flow and unseen otherwise, not `display: none`: Tab has to find them (WebKit's and Chromium's Tab both lost a
 * button that only showed while its row had the focus, and a row clicked with the mouse kept them; design review 11). */
const reach = "absolute opacity-0 pointer-events-none group-hover:static group-hover:opacity-100 group-hover:pointer-events-auto group-focus-visible:static group-focus-visible:opacity-100 group-focus-visible:pointer-events-auto";
const press = (go?: () => void) => ({
  role: "button", tabIndex: go ? 0 : -1, "aria-disabled": !go || undefined,
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

/** The day a conversation last ran, in the user's own time: the saved list's groups (until desktop 0.1.94 one "Recent", an
 * age on every row). */
export function dayOf(at?: number, now = Date.now()): "Today" | "Yesterday" | "This week" | "Earlier" {
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  const t = (at ?? 0) * 1000, s = start.getTime();
  return t >= s ? "Today" : t >= s - 864e5 ? "Yesterday" : t >= s - 6 * 864e5 ? "This week" : "Earlier";
}

/** What a row says of when, beside its group's name: today its age ("3h"), yesterday its time, this week its day, before
 * that its date. */
export function when(at?: number, now = Date.now()): string {
  if (!at) return "";
  const d = new Date(at * 1000);
  switch (dayOf(at, now)) {
    case "Today": return age(at, now / 1000);
    case "Yesterday": return clock(d);
    case "This week": return d.toLocaleDateString([], { weekday: "short" });
    default: return d.toLocaleDateString([], { month: "short", day: "numeric" });
  }
}

const groupHead = "flex items-center px-[9px] pb-1 pt-3.5 text-[11.5px] font-medium text-faint";

function ThreadRow({ t, active, sub, onClick, onRename, onHide, opened, onClose, noPlace }: {
  t: Thread; active: boolean; sub?: boolean; onClick?: () => void; onRename?: (title: string) => void; onHide?: () => void;
  /** open now with nothing to do: it stays under its day, marked, and × closes it (design review 10: it jumped into "Open") */
  opened?: boolean; onClose?: () => void;
  /** every row of the list is in the same place: the place is not repeated under each title */
  noPlace?: boolean;
}) {
  const [mode, setMode] = useState<"view" | "rename" | "hide">("view");
  const [title, setTitle] = useState(t.title);
  // back from the remove question by Cancel or Esc, the keyboard's focus is on the row again (it fell to the window)
  const row = useRef<HTMLAnchorElement>(null), back = useRef(false);
  useEffect(() => { if (mode === "view" && back.current) { back.current = false; row.current?.focus(); } }, [mode]);
  // 24 px to hit (WCAG 2.2's minimum; they were 16, design review 5)
  const icon = "grid size-6 place-items-center rounded text-faint hover:bg-hover hover:text-ink";
  if (mode === "rename") {
    return (
      <input autoFocus value={title} onChange={(e) => setTitle(e.target.value)} onBlur={() => setMode("view")}
        onKeyDown={(e) => { if (e.key === "Enter" && title.trim()) { onRename?.(title.trim()); setMode("view"); } if (e.key === "Escape") { e.preventDefault(); back.current = true; setMode("view"); } }}
        className={`mx-[2px] rounded-md border border-line-strong bg-card px-[7px] ${sub ? "ml-6 py-[5px]" : "py-[11px]"} text-ink outline-none`} />
    );
  }
  if (mode === "hide") {
    const twoLinesAsk = !sub && !noPlace && !!t.place;
    return (
      // the row's own height and its name stay: it said only "Remove?" and the list jumped by 18 px (design review 11).
      // Esc, Cancel or the focus going elsewhere leaves the question.
      <div title="Remove from the list (its recording stays)" onKeyDown={(e) => { if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); back.current = true; setMode("view"); } }}
        onBlur={(e) => { if (!e.currentTarget.contains(e.relatedTarget)) setMode("view"); }}
        className={`rounded-md bg-field px-[9px] ${twoLinesAsk ? "py-[5px]" : "flex items-center gap-2 py-1.5"} ${sub ? "pl-7" : ""}`}>
        <span className="block min-w-0 flex-1 truncate text-ink">{t.title}</span>
        <span className="flex flex-none items-center gap-2 whitespace-nowrap text-[11.5px]">{twoLinesAsk && <span className="flex-1 text-faint">Remove?</span>}
          <button autoFocus tabIndex={0} onClick={() => { onHide?.(); setMode("view"); }} className="font-medium text-bad">Remove</button>
          <button tabIndex={0} onClick={() => { back.current = true; setMode("view"); }} className="text-muted">Cancel</button></span>
      </div>
    );
  }
  const place = noPlace ? "" : t.place;
  const twoLines = !sub && !!place;
  const dot = opened && <span title="Open" className="size-1.5 flex-none rounded-full bg-live" />;
  return (
    <a ref={row} data-flip={sub ? undefined : t.id} data-tip-clip={(t.full ?? t.title).toLowerCase() === t.title.toLowerCase() ? "" : undefined} onClick={onClick} {...press(onClick)} title={onClick ? t.full ?? t.title : "Example conversation · available in the desktop app"} className={`group flex items-center gap-2 truncate rounded-md px-[9px] ${twoLines ? "py-[5px]" : "py-1.5"} text-ink-2 transition-colors duration-150 ${onClick ? "hover:bg-hover" : "cursor-default"} ${sub ? "pl-7" : ""} ${active ? "bg-selected text-ink" : ""}`}>
      {sub && <span className={`size-1.5 flex-none rounded-full ${t.running ? "bg-live shadow-[0_0_0_3px_var(--live-soft)]" : "bg-faint"}`} />}
      {twoLines ? (
        <span className="min-w-0 flex-1">
          {/* where it ran and when, under its title: the model left the row (design review 9: "DeepSeek V4.1 Flas…" took
              the title's room on every row); it is in the conversation and the palette */}
          <span className="flex items-center gap-1.5 text-ink">{dot}<span data-tip-text className="truncate">{t.title}</span></span>
          <span className="flex gap-2 text-[11.5px] text-faint"><span className="truncate">{place}</span><span className="ml-auto flex-none tabular-nums group-hover:hidden group-focus-visible:hidden group-has-[:focus-visible]:hidden">{when(t.at)}</span></span>
        </span>
      ) : <span className="flex min-w-0 flex-1 items-center gap-1.5">{dot}<span data-tip-text className="truncate">{t.title}</span>
          {!sub && <span className="ml-auto flex-none text-[11.5px] tabular-nums text-faint group-hover:hidden group-focus-visible:hidden group-has-[:focus-visible]:hidden">{when(t.at)}</span>}</span>}
      {onRename && <span className={`${reach} flex flex-none gap-0.5 focus-within:static focus-within:opacity-100 focus-within:pointer-events-auto`}>
        <button tabIndex={0} title="Rename" onClick={(e) => { e.stopPropagation(); setTitle(t.title); setMode("rename"); }} className={icon}><Pencil size={12} strokeWidth={1.5} /></button>
        {opened && onClose
          ? <button tabIndex={0} title="Close (the conversation stays in the lists)" aria-label={`Close ${t.title}`} onClick={(e) => { e.stopPropagation(); onClose(); }} className={icon}><X size={12} strokeWidth={1.5} /></button>
          : <button tabIndex={0} title="Remove from the list (its recording stays)" onClick={(e) => { e.stopPropagation(); setMode("hide"); }} className={icon}><X size={12} strokeWidth={1.5} /></button>}
      </span>}
    </a>
  );
}

type Open = { tid: string; title: string; place: string; state: "working" | "asking" | "idle" | "stopped"; id?: string | null; model?: string };

/** A conversation open now: its name, then where it runs and what it needs; a spinner while it works. */
function OpenRow({ o, active, onSwitch, onClose }: { o: Open; active: boolean; onSwitch?: (tid: string) => void; onClose?: (tid: string) => void }) {
  return (
    <a data-flip={o.id ?? o.tid} onClick={() => onSwitch?.(o.tid)} {...press(() => onSwitch?.(o.tid))} title={`${o.title} (${STATE[o.state][1]})`}
      className={`group flex items-center gap-2 rounded-md px-[9px] py-[5px] text-ink-2 hover:bg-hover ${active ? "bg-selected text-ink" : ""}`}>
      <span className="min-w-0 flex-1">
        <span className="flex items-center gap-1.5 text-ink">
          {o.state === "working" ? <LoaderCircle size={12} strokeWidth={2} className="flex-none animate-spin text-live" aria-hidden="true" />
            // one waiting for you: a dot in the warning's colour (an "asks" badge said it a third time, under "Needs you" and
            // over "waiting for your answer", and took the title's room; design review 11)
            : <span className={`size-1.5 flex-none rounded-full ${STATE[o.state][0]}`} />}
          <span className="truncate">{o.title}</span>
        </span>
        <Parts className="block truncate text-[11.5px] text-faint" items={[o.place, o.state === "asking" && "waiting for your answer", o.state === "stopped" && "stopped"]} />
      </span>
      <button tabIndex={0} title="Close (the conversation stays in the lists)" aria-label={`Close ${o.title}`} onClick={(e) => { e.stopPropagation(); onClose?.(o.tid); }}
        className={`${reach} grid size-6 flex-none place-items-center rounded text-faint hover:bg-hover hover:text-ink focus-visible:static focus-visible:opacity-100 focus-visible:pointer-events-auto`}><X size={12} strokeWidth={1.5} /></button>
    </a>
  );
}

// in the Tauri window on macOS the title bar is an overlay: the traffic lights sit on the window's top row, beside the brand
// (or the sheet's header, in a narrow window), moved there by tauri.conf.json's trafficLightPosition
export const macOverlay = "__TAURI_INTERNALS__" in window && navigator.userAgent.includes("Mac");

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
  onDismiss?: () => void;
};

const STATE: Record<string, [string, string]> = {
  working: ["bg-live shadow-[0_0_0_3px_var(--live-soft)]", "working"],
  asking: ["bg-warn shadow-[0_0_0_3px_color-mix(in_srgb,var(--warn)_25%,transparent)]", "waiting for your answer"],
  idle: ["bg-faint", "idle"],
  stopped: ["bg-bad", "stopped"],
};

export function Sidebar({ chats, projects, active, planLeft, onNewChat, onOpenFolder, onOpenProject, onOpenThread, onSearch, onSettings, onRename, onHide, open = [], activeTid, onSwitch, onClose, onDismiss }: Props) {
  // the chevron folds and unfolds a project's conversations by itself (until 0.1.57 "open" was only "the active project":
  // a click opened it and nothing folded it again); the name opens the project
  const [fold, setFold] = useState<Record<string, boolean>>({});
  const needs = open.filter((o) => o.state === "asking"), working = open.filter((o) => o.state === "working");
  // only what needs you or works lifts a conversation out of its day; one open with nothing to do stays under its day, marked
  // by a dot (design review 10: the row you clicked jumped into an "Open" group). A conversation in a group above shows
  // there only.
  const lifted = new Set([...needs, ...working].map((o) => o.id).filter(Boolean));
  const all = [...chats.map((t) => ({ t, p: undefined as Project | undefined })), ...projects.flatMap((p) => p.threads.map((t) => ({ t, p })))]
    .filter(({ t }) => !lifted.has(t.id)).sort((a, b) => (b.t.at ?? 0) - (a.t.at ?? 0));
  const recent = all.slice(0, 20);
  const idleOpen = new Map(open.filter((o) => (o.state === "idle" || o.state === "stopped") && o.id).map((o) => [o.id!, o]));
  // an open one the lists do not hold (older than the twenty, or not saved yet) keeps a group of its own
  const shown = new Set(recent.map(({ t }) => t.id));
  const idle = open.length > 1 ? open.filter((o) => (o.state === "idle" || o.state === "stopped") && !(o.id && shown.has(o.id))) : [];
  // a list where every row says the same place ("tazuna" twenty times) does not repeat it under each title
  const samePlace = recent.length > 1 && recent.every(({ t }) => t.place === recent[0].t.place);
  // rows slide to where they go (a conversation that starts working lifts into Working), not a jump cut: each row's top is
  // kept by its conversation's id, and a row that moved animates from the old one (design review 10)
  const listRef = useRef<HTMLDivElement>(null), tops = useRef(new Map<string, number>());
  useLayoutEffect(() => {
    const box = listRef.current;
    if (!box) return;
    const still = window.matchMedia("(prefers-reduced-motion: reduce)").matches, next = new Map<string, number>();
    for (const el of box.querySelectorAll<HTMLElement>("[data-flip]")) {
      const key = el.dataset.flip!, top = el.offsetTop, was = tops.current.get(key);
      next.set(key, top);
      if (!still && was !== undefined && Math.abs(was - top) > 1) el.animate([{ transform: `translateY(${was - top}px)` }, { transform: "none" }], { duration: 200, easing: "cubic-bezier(0.2, 0, 0, 1)" });
    }
    tops.current = next;
  });
  return (
    // the column's whole width, whatever its rows: with a short list it shrank to them and left a strip of the window's
    // background between it and the conversation (desktop 0.1.75)
    <aside aria-label="Conversations" className="flex min-w-0 flex-1 flex-col gap-0.5 overflow-hidden bg-desk px-2.5 pb-3.5">
      {/* the window's one top row (design review 12, A2 of mockups/v9.html): the traffic lights, then the brand on their line,
          the sheet's header centred on it too (y 28); a drag region, its words too ("deep": Tauri's drag ignores children) */}
      <div data-tauri-drag-region="deep" className={`-mx-2.5 mb-1.5 flex h-[calc(48*var(--tz-pt))] flex-none items-center gap-2 pt-[calc(8*var(--tz-pt))] font-semibold tracking-[-0.01em] ${macOverlay ? "pl-[calc(92*var(--tz-pt))]" : "pl-[18px]"}`}>
        {/* at work while any open conversation's reply runs (design review 13, D1) */}
        <Mark motion={working.length > 0 ? "pulse" : "still"} />
        <span>tazuna</span>
        {onDismiss && <button type="button" onClick={onDismiss} aria-label="Close conversations" title="Close conversations" className="ml-auto mr-2.5 grid size-7 place-items-center rounded-md text-muted hover:bg-hover hover:text-ink"><X {...icon} /></button>}
      </div>
      <button onClick={onSearch} className="tz-search mb-1 flex items-center gap-2 rounded-md border border-line px-[9px] py-[7px] text-left text-muted hover:bg-hover">
        <Search {...icon} /><span>Search</span><Kbd always>⌘K</Kbd>
      </button>
      <SidebarCtx.Provider value={{ onNewChat, onOpenFolder, onSettings, planLeft }}>
      <Slot name="sidebar.top" props={{}} />
      {/* rows keep their height and the list scrolls (*:shrink-0): a row's truncate hides its overflow, so in a full list the
          flex column squeezed every row to fit and they overlapped (desktop 0.1.64, with many saved conversations) */}
      {/* relative: the rows' offsetTop for the sliding; tz-edges fades the list at its ends, so a row cut by the scroll reads
          as more to come, not as a sliced heading (design review 10) */}
      <div ref={listRef} className="tz-edges relative flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto pb-[18px] *:shrink-0">
      {/* the conversations open now by what they need: an answer from you, time to work, nothing (when more than one is open);
          then every saved one by when it last ran, its project and age under its name; then the projects (2026-11-18) */}
      {([["Needs you", needs], ["Working", working], ["Open", idle]] as const).map(([head, list]) => list.length > 0 && (
        <div key={head} className="flex flex-col gap-0.5">
          <div className={groupHead}><span className="flex-1">{head}</span><span className="tabular-nums">{list.length}</span></div>
          {list.map((o) => <OpenRow key={o.tid} o={o} active={o.tid === activeTid} onSwitch={onSwitch} onClose={onClose} />)}
        </div>
      ))}
      {(["Today", "Yesterday", "This week", "Earlier"] as const).map((day) => {
        const list = recent.filter(({ t }) => dayOf(t.at) === day);
        return list.length > 0 && <div key={day} className="flex flex-col gap-0.5">
          <div className={groupHead}>{day}</div>
          {list.map(({ t, p }) => {
            const o = idleOpen.get(t.id);
            return <ThreadRow key={t.id} t={t} active={o ? o.tid === activeTid : t.id === active} noPlace={samePlace} opened={!!o && open.length > 1}
              onClick={o ? () => onSwitch?.(o.tid) : onOpenThread && (() => onOpenThread(t, p))} onClose={o && (() => onClose?.(o.tid))}
              onRename={onRename && ((title) => onRename(t, title))} onHide={onHide && (() => onHide(t))} />;
          })}
        </div>;
      })}
      {projects.length > 0 && <div className={groupHead}>Projects</div>}
      {projects.map((p) => {
        // folded until unfolded by hand: its conversations are in Recent too (unfolded on open, desktop 0.1.76 listed each twice)
        const key = p.path ?? p.name, shown = fold[key] ?? false;
        return (
        <div key={key}>
          <div className={`flex items-center rounded-md text-ink-2 ${p.path && p.path === active ? "bg-selected text-ink" : ""}`}>
            {p.threads.length > 0 ? (
              <button onClick={() => setFold((f) => ({ ...f, [key]: !shown }))} aria-expanded={shown} aria-label={`${shown ? "Hide" : "Show"} ${p.name}'s conversations`}
                className="ml-1 rounded p-1 text-faint hover:bg-hover hover:text-ink">
                <ChevronRight size={14} strokeWidth={1.5} className={`transition-transform duration-150 ${shown ? "rotate-90" : ""}`} />
              </button>
            ) : <span className="ml-1 w-[22px]" />}
            <button disabled={!onOpenProject} onClick={() => onOpenProject?.(p)} title={onOpenProject ? p.path : "Available in the desktop app"}
              className="flex min-w-0 flex-1 items-center gap-2 rounded-md py-1.5 pl-1 pr-[9px] text-left enabled:hover:bg-hover disabled:cursor-default disabled:text-muted">
              <span className="flex-1 truncate">{p.name}</span>
              {p.pending ? <span className="rounded-full bg-live px-1.5 text-[10.5px] font-semibold leading-[17px] text-on-accent">{p.pending}</span> : null}
            </button>
          </div>
          {shown && p.threads.slice(0, 15).map((t) => <ThreadRow key={t.id} t={t} active={t.id === active} sub onClick={onOpenThread && (() => onOpenThread(t, p))}
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
  return <button disabled={!onNewChat} title={!onNewChat ? "Available in the desktop app" : undefined} onClick={onNewChat} className="group flex w-full items-center gap-2 rounded-md px-[9px] py-[7px] text-left text-ink transition-colors enabled:hover:bg-hover disabled:cursor-default disabled:text-muted">
    <Plus {...icon} /><span>New chat</span><Kbd>⌘N</Kbd></button>;
}

export function OpenFolderButton() {
  const { onOpenFolder } = useContext(SidebarCtx);
  return <button disabled={!onOpenFolder} title={!onOpenFolder ? "Available in the desktop app" : undefined} onClick={onOpenFolder} className="group flex w-full items-center gap-2 rounded-md px-[9px] py-[7px] text-left text-ink transition-colors enabled:hover:bg-hover disabled:cursor-default disabled:text-muted">
    <FolderOpen {...icon} /><span>Open folder</span><Kbd>⌘O</Kbd></button>;
}


export function SettingsRow() {
  const { onSettings } = useContext(SidebarCtx);
  return (
    <div className="border-t border-line pt-1.5">
      <button disabled={!onSettings} title={!onSettings ? "Available in the desktop app" : undefined} onClick={onSettings} className="group flex w-full items-center gap-2 rounded-md px-[9px] py-[7px] text-left text-muted transition-colors enabled:hover:bg-hover enabled:hover:text-ink disabled:cursor-default">
        {/* a gear: the "S" in a circle read as someone's avatar (design review 10) */}
        <Settings {...icon} />
        <span>Settings</span>
        {/* planLeft stays in the context for a plugin; the plan's figures are by the composer now */}
        <Kbd>⌘,</Kbd>
      </button>
    </div>
  );
}
