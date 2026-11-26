import { Check, GitBranch, GitPullRequest, LoaderCircle, Minus, Pencil, SquareTerminal, X } from "lucide-react";
import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { Slot } from "../lib/slots";
import { tokens, widgetUrl, type UserWidget } from "../lib/widgets";
import { OPEN_PR, checksOf, type Pr } from "../lib/pr";
import type { View } from "../lib/tazuna";
import { PANEL, tenths, type ChangedFile, type Message, type Usage } from "../lib/types";
import { Counts, Sep } from "./Sep";
import { quietButton } from "./Controls";

// The right column is the work panel (the design review of 2026-11-18): its Stats (components/Stats.tsx), what the agent
// changed, the checks it ran, the branch's pull request, then your messages and the user's own panels; it closes (⌘\), and a plain chat opens without
// it. Until desktop 0.1.64 it held four equal cards of numbers (context, plan, thread cost, today) that repeated the
// composer's and the header's; those went into the plan ring by the composer and the usage page. It stays a slot
// (docs/PLUGINS.md): a plugin adds a section, replaces one by id, or hides it. A section with nothing real to show
// returns null and is left out.

/** What the built-in sections read, from the window (a plugin replacing one can read it too). */
export type Work = {
  /** the files this conversation changed (not undone), summed over its turns */
  files: ChangedFile[];
  /** the branch and its base, when the pull request says them */
  branch?: string; base?: string;
  /** tazuna's own checks after a turn (the last run first) */
  /** `none`: it gave no result (the command is not installed, or it ran out of time), in words: not a failure */
  checks: { cmd: string; ok?: boolean; secs: number; none?: string }[];
  pr?: Pr | null;
  folder?: string | null;
  review?: () => void; openPr?: () => void;
  /** a message to the agent (the PR section's "Ask tazuna to open one") */
  ask?: (text: string) => void;
  /** the conversation as the core sends it, and the window's id for it: the Stats read its calls (the user, 2026-11-25) */
  view?: View | null; active?: string;
  /** the composer's numbers asked for Stats over this scope (`n` counts the asks: each is taken once) */
  statsAsk?: { scope: "turn" | "all"; n: number };
};
export const WorkCtx = createContext<Work>({ files: [], checks: [] });

/** One section of the panel: a heading with an icon and a figure on the right, its body under it; sections lie on the desk
 * apart by a rule, not as bordered cards (design review 10: the conversation is the one sheet; cards beside it were boxes
 * on the same fill). */
export function Section({ icon, title, right, children }: { icon?: ReactNode; title: string; right?: ReactNode; children?: ReactNode }) {
  return (
    <section className="border-b border-line pb-3.5 last:border-b-0">
      <div className="flex items-center gap-2 py-1 font-medium text-ink">{icon}<span className="flex-1">{title}</span>
        {right != null && <span className="min-w-0 truncate text-[12px] font-normal tabular-nums text-faint">{right}</span>}</div>
      {children && <div className="pt-1">{children}</div>}
    </section>
  );
}

const ico = "flex-none text-muted";
const btn = quietButton;

/** Kept for a panel or plugin made before 0.1.64: a titled card. */
export function Card({ title, children }: { title: string; children: ReactNode }) {
  return <Section title={title}>{children}</Section>;
}

/** Opens the usage page (App listens). */
export const openUsage = () => window.dispatchEvent(new CustomEvent("tazuna-usage"));

const name = (path: string) => { const i = path.lastIndexOf("/"); return i < 0 ? [path, ""] : [path.slice(i + 1), path.slice(0, i + 1)]; };

export function ChangesSection() {
  const w = useContext(WorkCtx);
  const [all, setAll] = useState(false);
  if (!w.folder) return null;
  const added = w.files.reduce((n, f) => n + (f.added ?? 0), 0), removed = w.files.reduce((n, f) => n + (f.removed ?? 0), 0);
  const shown = all ? w.files : w.files.slice(0, 8);
  return (
    <Section icon={<GitBranch size={14} strokeWidth={1.5} className={ico} />} title="Changes"
      // a side with nothing is left out: never "+3 −0" (design review 12)
      right={added || removed ? <Counts added={added} removed={removed} /> : undefined}>
      {w.branch && <div className="mb-1 truncate font-mono text-[12px] text-muted" title={`${w.branch}${w.base ? ` → ${w.base}` : ""}`} data-tip-clip>{w.branch}{w.base ? ` → ${w.base}` : ""}</div>}
      {w.files.length === 0 ? <div className="py-0.5 text-[12.5px] text-muted">Nothing changed in this conversation yet.</div> : <>
        {shown.map((f) => {
          const [file, dir] = name(f.path);
          return (
            <div key={f.path} title={f.path} className="flex items-center gap-2 py-[3px] text-[12.5px]">
              <span className="min-w-0 flex-1 truncate"><span className="font-medium text-ink">{file}</span> <span className="text-faint">{dir}</span></span>
              {f.added == null ? <span className="text-[12px] text-faint">binary</span> : <>
                {f.added > 0 && <span className="text-[12px] tabular-nums text-ok">+{f.added}</span>}
                {(f.removed ?? 0) > 0 && <span className="text-[12px] tabular-nums text-bad">−{f.removed}</span>}</>}
            </div>
          );
        })}
        {w.files.length > 8 && !all && <button onClick={() => setAll(true)} className="mt-0.5 text-[12.5px] text-accent-ink">{w.files.length - 8} more</button>}
        {w.review && <div className="mt-2 flex gap-1.5"><button onClick={w.review} title="Go through the uncommitted changes piece by piece: stage or revert" className={btn}>Review</button></div>}
      </>}
    </Section>
  );
}

export function ChecksSection() {
  const w = useContext(WorkCtx);
  if (!w.folder || !w.checks.length) return null;
  const [last, ...before] = w.checks;
  return (
    <Section icon={<SquareTerminal size={14} strokeWidth={1.5} className={ico} />} title="Checks">
      {/* the whole command, wrapped and selectable: cut at 24 characters it read "npm run typecheck --sile" (the user,
          design review 14) */}
      <div className="mb-1 select-text break-all font-mono text-[11.5px] leading-[1.45] text-faint" title={last.cmd}>{last.cmd}</div>
      {[last, ...before.slice(0, 2)].map((c, i) => (
        <div key={i} className="flex items-center gap-2 py-[3px] text-[12.5px] text-ink-2">
          {c.none ? <Minus size={14} strokeWidth={1.75} className="flex-none text-faint" />
            : c.ok == null ? <LoaderCircle size={14} strokeWidth={1.75} className="flex-none animate-spin text-live" />
            : c.ok ? <Check size={14} strokeWidth={1.75} className="flex-none text-ok" /> : <X size={14} strokeWidth={1.75} className="flex-none text-bad" />}
          <span className="min-w-0 flex-1 truncate">{c.none ?? (c.ok == null ? "Running" : c.ok ? "Passed" : "Failed")}{i > 0 ? (c.none ? " earlier" : " before") : ""}</span>
          <span className="text-[12px] tabular-nums text-faint">{tenths(c.secs)}</span>
        </div>
      ))}
    </Section>
  );
}

export function PrSection() {
  const w = useContext(WorkCtx);
  if (!w.folder || !w.pr || w.pr.error) return null;
  const p = w.pr, cs = p.none ? [] : checksOf(p);
  const failed = cs.filter((c) => c.state === "failed").length, running = cs.filter((c) => c.state === "running").length;
  return (
    <Section icon={<GitPullRequest size={14} strokeWidth={1.5} className={ico} />} title="Pull request" right={p.number ? `#${p.number}` : undefined}>
      {p.none ? <div className="py-0.5 text-[12.5px] text-muted">No pull request for this branch yet.</div> : <>
        <div className="truncate py-0.5 text-[12.5px] text-ink" title={p.title}>{p.title}</div>
        <div className="text-[12.5px] text-muted">{p.state === "MERGED" ? "Merged" : p.state === "CLOSED" ? "Closed" : p.isDraft ? "Draft" : "Open"}
          {cs.length > 0 && <><Sep />{failed ? <span className="text-bad">{failed} failed</span> : running ? `${running} running` : <span className="text-ok">checks passed</span>}</>}</div>
      </>}
      {p.none ? w.ask && <div className="mt-2 flex gap-1.5"><button onClick={() => w.ask!(OPEN_PR)} title="tazuna commits what is ready, pushes the branch and opens it with gh" className={btn}>Ask tazuna to open one</button></div>
        : w.openPr && <div className="mt-2 flex gap-1.5"><button onClick={w.openPr} className={btn}>Checks and reviews</button></div>}
    </Section>
  );
}

/** Your messages in this conversation, newest last: a click scrolls the transcript to one (the prompt timeline). */
export function Timeline({ messages }: { messages: Message[] }) {
  const mine = messages.flatMap((m, i) => m.role === "user" && !m.queued ? [{ i, text: m.text.replace(/^\[tazuna: [^\]]*\]\n\n/, "").split("\n")[0] }] : []);
  if (mine.length < 2) return null;
  return (
    <Section title="Your messages" right={mine.length}>
      {/* rows keep their height and the list scrolls: a long list squeezed them and cut the letters' tails (design review 6) */}
      <div className="-mx-1 flex max-h-56 flex-col overflow-y-auto *:shrink-0">
        {mine.map((m) => (
          <button key={m.i} title={m.text} onClick={() => window.dispatchEvent(new CustomEvent("tazuna-jump", { detail: m.i }))}
            className="truncate rounded px-1 py-[3px] text-left text-[12px] leading-[18px] text-ink-2 hover:bg-hover">{m.text || "(attachment)"}</button>
        ))}
      </div>
    </Section>
  );
}

/** A user's panel: its page from the tzwidget: scheme in a sandboxed frame (no same origin: it cannot reach tazuna), the
 * data posted in on every change, its height as it asks (40-600 px). */
export function UserPanel({ w, data, onEdit, onRemove }: { w: UserWidget; data: unknown; onEdit?: () => void; onRemove?: () => void }) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [height, setHeight] = useState(160);
  const [sure, setSure] = useState(false);
  const post = () => frame.current?.contentWindow?.postMessage({ type: "tazuna", data, tokens: tokens() }, "*");
  useEffect(post, [data]);
  useEffect(() => {
    const on = (e: MessageEvent) => {
      if (e.source !== frame.current?.contentWindow || e.data?.type !== "tazuna-height") return;
      const h = Number(e.data.height);
      if (Number.isFinite(h)) setHeight(Math.max(40, Math.min(600, Math.round(h))));
    };
    window.addEventListener("message", on);
    return () => window.removeEventListener("message", on);
  }, []);
  return (
    <div className="group border-b border-line pb-3.5 last:border-b-0">
      <div className="mb-2 flex items-center gap-1 text-[11.5px] font-medium text-faint">
        <span className="flex-1 truncate">{w.title}</span>
        {sure ? <>
          <button onClick={onRemove} className="text-bad">Remove</button><button onClick={() => setSure(false)} className="ml-1.5">Keep</button>
        </> : <span className="hidden gap-0.5 group-hover:flex">
          {onEdit && <button title="Change this panel with tazuna" onClick={onEdit} className="rounded p-0.5 hover:bg-hover hover:text-ink"><Pencil size={12} strokeWidth={1.5} /></button>}
          {onRemove && <button title="Remove this panel (its folder moves to .removed)" onClick={() => setSure(true)} className="rounded p-0.5 hover:bg-hover hover:text-ink"><X size={12} strokeWidth={1.5} /></button>}
        </span>}
      </div>
      <iframe ref={frame} src={widgetUrl(w.id)} sandbox="allow-scripts" title={w.title} onLoad={post}
        style={{ height }} className="block w-full border-0 bg-transparent" />
    </div>
  );
}

/** The right column: the `rail` spot (tazuna's cards and the user's panels and plugins). */
export function Rail({ usage, data, messages, over }: { usage: Usage; data?: unknown; messages: Message[]; over?: boolean }) {
  return (
    // the panel's width (`--tz-panel`, its edge: `PanelEdge`) whatever its column is: the column slides open and shut (App's
    // grid), the cards do not reflow in it. With no card (a plain chat) it says so, not a blank column. Its first heading on
    // the window's top row (design review 12); `over`: on its own sheet over the conversation (B2), even sides
    <aside aria-label="The work" className={`tz-enter flex min-h-0 flex-col gap-3.5 overflow-y-auto pb-4 ${over ? "pt-3.5" : "pt-[calc(28*var(--tz-pt)-14px)]"} ${over ? "w-full px-4" : "w-[var(--tz-panel,300px)] flex-none pl-2.5 pr-4"} empty:after:text-[12.5px] empty:after:leading-relaxed empty:after:text-faint empty:after:content-['Nothing_here_for_a_plain_chat._In_a_project_this_shows_its_changes,_checks_and_pull_request.']`}>
      <Slot name="rail" props={{ usage, data, messages }} />
    </aside>
  );
}

/** The work panel's inner edge: dragged, it sets the panel's width (260 px to `max`, panelState); the arrows by 16 px
 * (Shift: 64), Home and End to the least and the most, Enter or a double click back to 300. While it is dragged the width
 * is a CSS variable on the root (`--tz-panel`, App sets it from `w` otherwise), moved once a frame and given to `onSize`
 * when the pointer lets go: the transcript does not draw again for each move; `tz-sizing` holds the cursor and takes the
 * transitions off (styles.css), so the column follows the pointer and only opening and shutting slide. */
export function PanelEdge({ w, max, onSize, className }: { w: number; max: number; onSize: (w: number) => void; className: string }) {
  const me = useRef<HTMLDivElement>(null), drag = useRef<{ x: number; w: number; at: number; frame: number } | null>(null);
  const fit = (v: number) => Math.round(Math.max(PANEL.min, Math.min(max, v)));
  const put = (v: number) => { document.documentElement.style.setProperty("--tz-panel", `${v}px`); me.current?.setAttribute("aria-valuenow", String(v)); };
  // the panel lies at the window's right: the pointer going left makes it wider
  const now = (d: NonNullable<typeof drag.current>) => fit(d.w + d.x - d.at);
  const end = () => {
    const d = drag.current; if (!d) return;
    drag.current = null; cancelAnimationFrame(d.frame);
    // the last width is laid out before the transitions come back, or the grid would slide the last frame's step
    const v = now(d); put(v); void document.documentElement.offsetWidth;
    document.documentElement.classList.remove("tz-sizing");
    // a press that did not move keeps the width kept (a smaller window's hold is not the user's choice)
    if (d.at !== d.x) onSize(v);
  };
  // taken away mid-drag (⌘\, another conversation), or the window left (⌘Tab, Mission Control: the pointer's release never
  // comes): the drag ends as if let go, never leaving the cursor, the width or a panel that follows the mouse behind
  const ending = useRef(end);
  ending.current = end;
  useEffect(() => { const off = () => ending.current(); window.addEventListener("blur", off); return () => { window.removeEventListener("blur", off); off(); }; }, []);
  return (
    <div ref={me} role="separator" aria-orientation="vertical" aria-label="Width of the work panel" aria-valuenow={w} aria-valuemin={PANEL.min} aria-valuemax={max} tabIndex={0}
      // a press neither focuses it nor selects (focus stays in the box: typed text went nowhere and Enter reset the width);
      // it is a Tab stop for the keyboard. A second pointer while one drags starts nothing
      onMouseDown={(e) => e.preventDefault()}
      onPointerDown={(e) => {
        if (e.button !== 0 || drag.current) return;
        e.preventDefault();
        e.currentTarget.setPointerCapture(e.pointerId);
        drag.current = { x: e.clientX, w, at: e.clientX, frame: 0 };
        document.documentElement.classList.add("tz-sizing");
      }}
      onPointerMove={(e) => {
        const d = drag.current; if (!d) return;
        // a move with the button up: its release went elsewhere
        if (!(e.buttons & 1)) return end();
        d.at = e.clientX;
        if (!d.frame) d.frame = requestAnimationFrame(() => { d.frame = 0; if (drag.current === d) put(now(d)); });
      }}
      onPointerUp={end} onPointerCancel={end} onLostPointerCapture={end}
      onDoubleClick={() => onSize(fit(PANEL.base))}
      onKeyDown={(e) => {
        const by = e.shiftKey ? 64 : 16;
        const v = { ArrowLeft: w + by, ArrowRight: w - by, Home: PANEL.min, End: max, Enter: PANEL.base }[e.key];
        if (v == null) return;
        e.preventDefault(); onSize(fit(v));
      }}
      className={`absolute inset-y-0 z-10 cursor-col-resize outline-none before:pointer-events-none before:absolute before:inset-y-0 before:w-px hover:before:bg-line-strong [.tz-sizing_&]:before:bg-faint focus-visible:before:w-0.5 focus-visible:before:bg-accent ${className}`} />
  );
}
