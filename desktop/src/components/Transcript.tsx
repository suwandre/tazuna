import { CircleAlert, Check, ChevronRight, Copy, FileText, LoaderCircle, MessageCircle, Pencil, RotateCcw, Square } from "lucide-react";
import { createContext, useContext, useEffect, useRef, useState } from "react";
import { Slot } from "../lib/slots";
import { readableError } from "../lib/fromView";
import { usd, type Call, type Change, type Message, type ToolStep } from "../lib/types";

function inlineCode(text: string) {
  return text.split(/(`[^`]+`)/).map((part, i) => part.startsWith("`") ? <code key={i}>{part.slice(1, -1)}</code> : part);
}

function meta(m: { cost: number; tokens: number; cachedPct?: number; secs: number }) {
  const tokens = m.tokens >= 1000 ? `${(m.tokens / 1000).toFixed(1)}k` : `${m.tokens}`;
  return [usd(m.cost), `${tokens} tokens`, m.cachedPct != null ? `${m.cachedPct}% cached` : null, `${m.secs} s`].filter(Boolean).join(" · ");
}

// one "thinking" from send to the first output (no "starting", then "waiting", then "thinking")
const doing: Record<string, string> = { Starting: "thinking", Waiting: "thinking", Thinking: "thinking", Writing: "writing", Tool: "working", Verifying: "running the checks" };

type Props = {
  messages: Message[]; busy?: boolean; activity?: string;
  /** the reply running waits for your answer (an approval): its fold says so */
  asking?: boolean;
  /** when the reply running began (ms), for its time */
  since?: number; onSendToAgent?: (text: string) => void;
  onDiff?: (turn: number, path: string) => void; onUndo?: (turn: number) => void; onReview?: (turn: number) => void;
  /** "edit from here" on one of your messages */
  onEdit?: (edit: { cp: number; block: number }, text: string) => void;
  /** the checkpoint being edited: that message and everything after it fade */
  editFrom?: number;
  /** follows the rail's "tazuna-jump" (not the side chat's transcript) */
  jumps?: boolean;
  /** sends a failed reply's message again */
  onRetry?: (text: string) => void;
};

const STATUS: Record<string, string> = { A: "added", D: "deleted", M: "changed" };

/** What a turn changed: each file with its lines added and removed and its diff; undo for the last turn. */
function ChangeCard({ change, busy, onDiff, onUndo, onReview }: { change: Change; busy?: boolean; onDiff?: Props["onDiff"]; onUndo?: Props["onUndo"]; onReview?: Props["onReview"] }) {
  const [all, setAll] = useState(false);
  const files = all ? change.files : change.files.slice(0, 6);
  return (
    <div className={`mt-1 rounded-lg border border-line bg-card text-[12.5px] ${change.undone ? "opacity-60" : ""}`}>
      {files.map((f) => (
        <div key={f.path} className="flex items-center gap-2.5 border-b border-line-soft px-3 py-[7px] last:border-b-0">
          <span className="flex min-w-0 flex-1 items-center gap-[7px] font-mono text-[12px] text-ink-2" title={`${STATUS[f.status] ?? f.status}: ${f.path}`}>
            <FileText size={14} strokeWidth={1.5} className="flex-none" /><span className="truncate">{f.path}</span>
            {f.status !== "M" && <span className="flex-none font-sans text-[11px] text-faint">{STATUS[f.status] ?? f.status}</span>}
          </span>
          {f.added == null ? <span className="text-[12px] text-faint">binary</span> : <>
            {f.added > 0 && <span className="text-[12px] font-medium tabular-nums text-ok">+{f.added}</span>}
            {(f.removed ?? 0) > 0 && <span className="text-[12px] font-medium tabular-nums text-bad">−{f.removed}</span>}
          </>}
          <button onClick={() => onDiff?.(change.turn, f.path)} className="rounded-md border border-line px-2 py-0.5 text-[12px] text-ink-2 hover:bg-hover">Diff</button>
        </div>
      ))}
      <div className="flex items-center gap-2 px-3 py-[7px] text-[12px] text-muted">
        <span className="flex-1">{change.files.length} file{change.files.length === 1 ? "" : "s"} changed in this turn
          {change.files.length > 6 && !all && <button onClick={() => setAll(true)} className="ml-2 text-accent-ink">show all</button>}</span>
        {!change.undone && onReview && <button onClick={() => onReview(change.turn)} title="Go through this turn's changes piece by piece"
          className="rounded-md border border-line px-2.5 py-0.5 text-ink hover:bg-hover">Review</button>}
        {change.undone ? <span className="text-faint">undone</span>
          : change.canUndo && <button disabled={busy} onClick={() => onUndo?.(change.turn)} title="Put these files back as they were before this turn"
              className="rounded-md border border-line px-2.5 py-0.5 text-ink hover:bg-hover disabled:opacity-50">Undo</button>}
      </div>
    </div>
  );
}

/** What the transcript's built-in parts need besides their slot props (callbacks, the busy state, the message being
 * edited): one context for the parts, so a plugin replacing one can use the same through `useTranscript`. */
export const TranscriptCtx = createContext<Omit<Props, "messages" | "activity">>({});
export const useTranscript = () => useContext(TranscriptCtx);

export function Transcript({ messages, activity, jumps = true, ...rest }: Props) {
  const { busy, editFrom } = rest;
  // a reply with steps says it is working in its fold; the line under the transcript only until the first step
  const folded = !!busy && messages.some((m) => m.role === "agent" && !m.meta && (m.tools?.length ?? 0) > 0);
  // follow new output while the reader is at the bottom; stay put once they scroll up
  const box = useRef<HTMLElement>(null), stick = useRef(true);
  // a long conversation draws its latest messages; earlier ones on request (a three-day session is ~1,900 blocks)
  const [limit, setLimit] = useState(40);
  const hidden = Math.max(0, messages.length - limit);
  useEffect(() => { if (stick.current && box.current) box.current.scrollTop = box.current.scrollHeight; });
  // the rail's list of your messages (or a plugin) asks for message i with a "tazuna-jump" event: drawn if it was not, then
  // scrolled to
  const [jump, setJump] = useState<number | null>(null);
  useEffect(() => {
    if (!jumps) return;
    const on = (e: Event) => { const i = Number((e as CustomEvent).detail); if (!(i >= 0)) return; stick.current = false; setLimit((l) => Math.max(l, messages.length - i)); setJump(i); };
    window.addEventListener("tazuna-jump", on);
    return () => window.removeEventListener("tazuna-jump", on);
  }, [messages.length, jumps]);
  useEffect(() => { if (jump == null) return; box.current?.querySelector(`[data-msg="${jump}"]`)?.scrollIntoView({ block: "start" }); setJump(null); }, [jump]);
  return (
    <TranscriptCtx.Provider value={rest}>
    <section ref={box} onScroll={(e) => { const el = e.currentTarget; stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40; }}
      className="min-h-0 flex-1 overflow-auto pb-3 pt-7">
      {hidden > 0 && (
        <div className="mx-auto mb-5 max-w-[756px] px-7">
          <button onClick={() => { stick.current = false; setLimit((l) => l + 40); }} className="rounded-md border border-line px-2.5 py-1 text-[12px] text-muted hover:bg-hover">
            Show {Math.min(40, hidden)} earlier messages <span className="text-faint">({hidden} not shown)</span>
          </button>
        </div>
      )}
      {messages.slice(hidden).map((m, i, shown) => (
        <div key={hidden + i} data-msg={hidden + i} className={`group mx-auto mb-[22px] max-w-[756px] px-7 transition-opacity duration-150 ${editFrom != null && shown.slice(0, i + 1).some((x) => x.role === "user" && x.edit?.cp === editFrom) ? "opacity-35" : ""}`}>
          {m.role === "user" ? <Slot name="message.user" props={{ message: m, index: hidden + i }} only /> : (
            <>
              {/* no "tazuna" over each answer (nor "You" over each message): the field block says which is yours */}
              {m.routedChat && (
                <div className="mb-1.5">
                  <span className="inline-flex items-center gap-1.5 rounded-full bg-accent-soft px-2 py-px text-[11.5px] text-muted">
                    <MessageCircle size={13} strokeWidth={1.5} /> answered as a chat · <button disabled={busy} onClick={() => m.ask && rest.onSendToAgent?.(m.ask)} className="font-medium text-accent-ink disabled:opacity-50">send to agent</button>
                  </span>
                </div>
              )}
              {m.tools && m.tools.length > 0 && <WorkFold tools={m.tools} secs={m.meta?.secs} live={!!busy && !m.meta} asking={rest.asking} since={rest.since} />}
              <div className="prose-answer text-ink">{m.prose.map((p, j) => <Slot key={j} name="message.text" props={{ text: p }} only />)}</div>
              <Slot name="message.after" props={{ message: m, index: hidden + i }} />
            </>
          )}
        </div>
      ))}
      {busy && !folded && activity && doing[activity] && <Working what={doing[activity]} since={rest.since} />}
    </section>
    </TranscriptCtx.Provider>
  );
}

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** What a reply's steps did, in a few words: "read 2 files, edited 1, ran 3 commands". */
export function workSummary(tools: ToolStep[]): string {
  const n = new Map<string, number>();
  for (const t of tools) if (t.kind !== "thinking") n.set(t.verb, (n.get(t.verb) ?? 0) + 1);
  const edits = (n.get("Edited") ?? 0) + (n.get("Wrote") ?? 0);
  n.delete("Edited"); n.delete("Wrote");
  const parts: string[] = [];
  if (n.has("Read")) parts.push(`read ${plural(n.get("Read")!, "file")}`);
  if (edits) parts.push(`edited ${plural(edits, "file")}`);
  if (n.has("Ran")) parts.push(`ran ${plural(n.get("Ran")!, "command")}`);
  const searched = (n.get("Searched") ?? 0) + (n.get("Listed") ?? 0);
  if (searched) parts.push(searched === 1 ? "searched" : `searched ${searched} times`);
  for (const k of ["Read", "Ran", "Searched", "Listed"]) n.delete(k);
  for (const [verb, count] of n) parts.push(`${verb.toLowerCase()}${count > 1 ? ` ×${count}` : ""}`);
  if (!parts.length && tools.some((t) => t.kind === "thinking")) parts.push("thought it through");
  return parts.slice(0, 4).join(", ");
}

/** A reply's steps as one line, "Worked 41 s · read 2 files, ran 3 commands ›": open while the reply runs, folded once it
 * is done (a click keeps it as chosen). Until desktop 0.1.64 each step was a line of its own above every answer. */
function WorkFold({ tools, secs, live, asking, since }: { tools: ToolStep[]; secs?: number; live: boolean; asking?: boolean; since?: number }) {
  const [open, setOpen] = useState<boolean | null>(null);
  const shown = open ?? live;
  const what = workSummary(tools);
  const took = secs == null ? "" : ` ${dur(secs)}`;
  return (
    <div className="mb-2.5">
      <button onClick={() => setOpen(!shown)} aria-expanded={shown} title={shown ? "Fold the steps" : "Show each step"}
        className="-ml-1.5 inline-flex max-w-full items-center gap-1.5 rounded-md px-1.5 py-0.5 text-left text-[12.5px] text-muted hover:bg-hover hover:text-ink">
        <ChevronRight size={13} strokeWidth={1.5} className={`flex-none transition-transform duration-150 ${shown ? "rotate-90" : ""}`} />
        {live && !asking && <LoaderCircle size={13} strokeWidth={1.75} className="flex-none animate-spin text-accent" aria-hidden="true" />}
        <span className={`flex-none ${live && asking ? "font-medium text-warn" : ""}`}>{live ? (asking ? "Waiting for you" : <>Working{since && <> <Ticking since={since} /></>}</>) : `Worked${took}`}</span>
        {what && <span className="truncate text-faint">· {what}</span>}
      </button>
      {shown && (
        <div className="tz-enter ml-[5px] mt-1 flex flex-col gap-0.5 border-l border-line pl-3.5">
          {tools.map((t, j) => <Slot key={j} name="tool.view" props={{ step: t }} only />)}
        </div>
      )}
    </div>
  );
}

/** Under a reply that is still going: a spinner, what it is doing with dots that count, and how long the reply has run
 * (it stays mounted from the reply's start to its end). */
function Working({ what, since }: { what: string; since?: number }) {
  const [start] = useState(() => since ?? Date.now());
  return (
    <div role="status" className="mx-auto flex max-w-[756px] items-center gap-2 px-7 text-[12.5px] text-muted">
      <LoaderCircle size={13} strokeWidth={1.75} className="animate-spin text-accent" aria-hidden="true" />
      <span className="tz-live">{what}</span><span className="tz-dots -ml-2" aria-hidden="true"><span>.</span><span>.</span><span>.</span></span>
      <span className="tabular-nums text-faint"><Ticking since={start} /></span>
    </div>
  );
}

/** "41 s", "2 min 5 s" */
const dur = (s: number) => (s < 60 ? `${s} s` : `${Math.floor(s / 60)} min ${s % 60} s`);

/** The seconds since `since`, counting. */
function Ticking({ since }: { since: number }) {
  const [, tick] = useState(0);
  useEffect(() => { const t = setInterval(() => tick((n) => n + 1), 1000); return () => clearInterval(t); }, []);
  return <span className="tabular-nums">{dur(Math.max(0, Math.floor((Date.now() - since) / 1000)))}</span>;
}

// tazuna's own parts of a message, registered in their spots (src/builtins.tsx)

export function UserMessage({ message: m }: { message: Message }) {
  const { busy, onEdit } = useTranscript();
  if (m.role !== "user") return null;
  // the note tazuna adds after an undo ("[tazuna: ...]") is shown as what it is, apart from the message
  const note = m.text.match(/^\[tazuna: ([^\]]*)\]\n\n/);
  const text = note ? m.text.slice(note[0].length) : m.text;
  return (
    <>
      {note && <div className="mb-1 text-[11.5px] text-faint">told the model: {note[1]}</div>}
      <div className="flex items-start gap-1">
        <div className="inline-block max-w-full select-text rounded-lg bg-field px-3.5 py-2.5 text-ink">{inlineCode(text)}</div>
        {m.edit && onEdit && !busy && (
          <button onClick={() => onEdit(m.edit!, text)} title="Edit and send again from here: the conversation goes back to before this message" aria-label="Edit from here"
            className="mt-1.5 flex-none rounded p-1 text-faint opacity-0 hover:bg-hover hover:text-ink focus-visible:opacity-100 group-hover:opacity-100"><Pencil size={13} strokeWidth={1.5} /></button>
        )}
      </div>
      {m.queued && <div className="mt-1 text-[11.5px] text-faint">Queued: goes after this reply</div>}
    </>
  );
}

/** Why a reply ended without its answer: the provider's error in words, its reply as it came on request, and Retry; a
 * reply you stopped says only that. Until desktop 0.1.74 the error was its raw JSON in the answer's font, after a "⚠". */
export function MessageFailure({ message: m }: { message: Message }) {
  const { busy, onRetry } = useTranscript();
  const [raw, setRaw] = useState(false);
  if (m.role !== "agent") return null;
  const notes = (m.notes ?? []).filter((n) => n !== m.failure?.raw);
  const f = m.failure;
  return <>
    {notes.map((n, i) => <div key={i} className="mb-2 flex items-start gap-2 text-[12.5px] text-bad"><CircleAlert size={14} strokeWidth={1.5} className="mt-0.5 flex-none" />{readableError(n)}</div>)}
    {f && !f.text && <div className="mt-1 flex items-center gap-1.5 text-[12.5px] text-muted"><Square size={11} strokeWidth={2} className="fill-current text-warn" />Stopped</div>}
    {f && f.text && (
      <div role="alert" className="mt-1 rounded-lg border border-bad/35 bg-bad/[0.06] px-3.5 py-2.5 text-[13px]">
        <div className="flex items-start gap-2.5">
          <CircleAlert size={16} strokeWidth={1.5} className="mt-px flex-none text-bad" />
          <div className="min-w-0 flex-1">
            <div className="font-medium text-ink">{f.stopped ? "Stopped after an error from the provider" : "The provider could not answer"}</div>
            <div className="mt-0.5 text-ink-2">{f.text}</div>
            {f.raw && f.raw !== f.text && <button onClick={() => setRaw(!raw)} aria-expanded={raw} className="mt-1 text-[12px] text-muted hover:text-ink">{raw ? "Hide" : "Show"} its reply {raw ? "▾" : "▸"}</button>}
            {raw && <pre className="mt-1.5 max-h-48 overflow-auto whitespace-pre-wrap break-all rounded-md bg-field px-2.5 py-2 font-mono text-[11.5px] text-muted">{f.raw}</pre>}
          </div>
          {onRetry && f.ask && <button disabled={busy} onClick={() => onRetry(f.ask)} title="Send the same message again"
            className="flex flex-none items-center gap-1.5 rounded-md border border-line px-2.5 py-1 text-[12px] text-ink hover:bg-hover disabled:opacity-50"><RotateCcw size={12} strokeWidth={1.75} />Retry</button>}
        </div>
      </div>
    )}
  </>;
}

export function MessageChange({ message: m }: { message: Message }) {
  const { busy, onDiff, onUndo, onReview } = useTranscript();
  return m.role === "agent" && m.change ? <ChangeCard change={m.change} busy={busy} onDiff={onDiff} onUndo={onUndo} onReview={onReview} /> : null;
}

const tok = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : `${n}`);
const secs = (ms: number | null) => (ms == null ? "–" : `${(ms / 1000).toFixed(1)} s`);
const HEAD: [string, string][] = [["#", ""], ["called", "the tools this call asked for; none: it wrote the answer"],
  ["new in", "input tokens sent uncached or written into the cache"], ["cached", "input tokens read from the cache"], ["out", "output tokens"],
  ["first", "from its input being ready to its first token"], ["tok/s", "output while writing, after the first token (– where it cannot be measured)"],
  ["time", "the whole call, the wait included"], ["cost", "at this model's prices"]];

/** A reply's model calls, one row each: what it called, its tokens, timing and cost. */
function Calls({ calls }: { calls: Call[] }) {
  return (
    <table className="mt-1.5 w-full border-collapse font-mono text-[11px] text-muted">
      <thead className="text-faint">
        <tr>{HEAD.map(([h, why], i) => <th key={h} title={why || undefined} scope="col" className={`px-1.5 py-0.5 font-normal ${i < 2 ? "text-left" : "text-right"}`}>{h}</th>)}</tr>
      </thead>
      <tbody>
        {calls.map((c, i) => (
          <tr key={i} className="border-t border-line-soft">
            <td className="px-1.5 py-0.5">{i + 1}</td>
            <td className="max-w-[200px] truncate px-1.5 py-0.5" title={c.tools || undefined}>{c.tools || "answer"}</td>
            {[tok(c.fresh), tok(c.cached), tok(c.out), secs(c.firstMs), c.tps == null ? "–" : `${Math.round(c.tps)}`,
              c.totalMs == null ? "…" : secs(c.totalMs), usd(c.cost)].map((x, j) => <td key={j} className="px-1.5 py-0.5 text-right">{x}</td>)}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

/** Copies an answer's text as markdown (DeepSeek Harness puts copy under each answer; tazuna had only selecting it). */
function CopyAnswer({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  const copy = async () => {
    try { await navigator.clipboard.writeText(text); }
    catch {
      // a webview without the clipboard API: the old way, through a selected field
      const t = document.createElement("textarea");
      t.value = text; document.body.appendChild(t); t.select(); document.execCommand("copy"); t.remove();
    }
    setDone(true); setTimeout(() => setDone(false), 1500);
  };
  return (
    <button onClick={copy} title="Copy the answer (markdown)" className="inline-flex items-center gap-1 rounded px-1 hover:bg-hover hover:text-ink">
      {done ? <Check size={12} strokeWidth={1.75} className="text-ok" /> : <Copy size={12} strokeWidth={1.5} />}{done ? "Copied" : "Copy"}
    </button>
  );
}

export function MessageMeta({ message: m }: { message: Message }) {
  const [open, setOpen] = useState(false);
  if (m.role !== "agent" || !m.meta) return null;
  const n = m.meta.calls?.length ?? 0;
  const text = m.prose.join("\n\n");
  return (
    // the answer's numbers show on its hover (or focus, or with its calls open): under every answer they were the busiest
    // line on the screen, and the thread's cost stays in the header
    <div className={`mt-2 text-[12px] tabular-nums text-faint transition-opacity duration-150 ${open ? "" : "opacity-0 focus-within:opacity-100 group-hover:opacity-100"}`}>
      {text && <><CopyAnswer text={text} /> · </>}{meta(m.meta)}
      {n > 0 && <> · <button onClick={() => setOpen(!open)} aria-expanded={open} title="Each model call of this reply: tokens, timing and cost"
        className="rounded px-1 hover:bg-hover hover:text-ink">{n} call{n === 1 ? "" : "s"} {open ? "▾" : "▸"}</button></>}
      {open && m.meta.calls && <Calls calls={m.meta.calls} />}
    </div>
  );
}
