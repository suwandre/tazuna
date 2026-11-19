import { Check, GitBranch, GitPullRequest, LoaderCircle, Pencil, SquareTerminal, X } from "lucide-react";
import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { Slot } from "../lib/slots";
import { tokens, widgetUrl, type UserWidget } from "../lib/widgets";
import { OPEN_PR, checksOf, type Pr } from "../lib/pr";
import type { ChangedFile, Message, Usage } from "../lib/types";
import { Sep } from "./Sep";

// The right column is the work panel (the design review of 2026-11-18): what the agent changed, the checks it ran, the
// branch's pull request, then your messages and the user's own panels; it closes (⌘\), and a plain chat opens without
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
  checks: { cmd: string; ok?: boolean; secs: number }[];
  pr?: Pr | null;
  folder?: string | null;
  review?: () => void; openPr?: () => void;
  /** a message to the agent (the PR section's "Ask tazuna to open one") */
  ask?: (text: string) => void;
};
export const WorkCtx = createContext<Work>({ files: [], checks: [] });

/** One section of the panel: a heading with an icon and a figure on the right, its body under a line. */
export function Section({ icon, title, right, children }: { icon?: ReactNode; title: string; right?: ReactNode; children?: ReactNode }) {
  return (
    <section className="rounded-lg border border-line bg-card">
      <div className="flex items-center gap-2 px-3 py-2.5 font-medium text-ink">{icon}<span className="flex-1">{title}</span>
        {right != null && <span className="min-w-0 truncate text-[12px] font-normal tabular-nums text-faint">{right}</span>}</div>
      {children && <div className="border-t border-line-soft px-3 pb-2.5 pt-1.5">{children}</div>}
    </section>
  );
}

const ico = "flex-none text-muted";
const btn = "rounded-md border border-line px-2.5 py-1 text-[12.5px] text-ink";

/** Kept for a panel or plugin made before 0.1.64: a titled card. */
export function Card({ title, children }: { title: string; children: ReactNode }) {
  return <Section title={title}>{children}</Section>;
}

/** A bar for a used share (the plan card's meters). */
export function Bar({ pct }: { pct: number }) {
  return <div className="h-1.5 overflow-hidden rounded bg-line"><i className="block h-full rounded bg-accent transition-[width] duration-200" style={{ width: `${Math.min(100, Math.max(0, pct))}%` }} /></div>;
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
      right={w.files.length ? <><span className="text-ok">+{added}</span> <span className="text-bad">−{removed}</span></> : undefined}>
      {w.branch && <div className="mb-1 truncate font-mono text-[12px] text-muted">{w.branch}{w.base ? ` → ${w.base}` : ""}</div>}
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
    <Section icon={<SquareTerminal size={14} strokeWidth={1.5} className={ico} />} title="Checks" right={<span className="font-mono">{last.cmd.slice(0, 24)}</span>}>
      {[last, ...before.slice(0, 2)].map((c, i) => (
        <div key={i} className="flex items-center gap-2 py-[3px] text-[12.5px] text-ink-2">
          {c.ok == null ? <LoaderCircle size={14} strokeWidth={1.75} className="flex-none animate-spin text-accent" />
            : c.ok ? <Check size={14} strokeWidth={1.75} className="flex-none text-ok" /> : <X size={14} strokeWidth={1.75} className="flex-none text-bad" />}
          <span className="min-w-0 flex-1 truncate">{c.ok == null ? "Running" : c.ok ? "Passed" : "Failed"}{i > 0 ? " before" : ""}</span>
          <span className="text-[12px] tabular-nums text-faint">{c.secs.toFixed(1)} s</span>
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
    <div className="group rounded-lg border border-line bg-card px-3.5 py-3">
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
export function Rail({ usage, data, messages }: { usage: Usage; data?: unknown; messages: Message[] }) {
  return (
    <aside aria-label="The work" className="tz-enter flex min-h-0 flex-col gap-3 overflow-y-auto border-l border-line bg-sidebar px-3.5 py-4">
      <Slot name="rail" props={{ usage, data, messages }} />
    </aside>
  );
}
