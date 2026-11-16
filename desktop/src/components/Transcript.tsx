import { FileText, MessageCircle, Pencil } from "lucide-react";
import { createContext, useContext, useEffect, useRef, useState } from "react";
import { Slot } from "../lib/slots";
import { usd, type Change, type Message } from "../lib/types";

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
  messages: Message[]; busy?: boolean; activity?: string; onSendToAgent?: (text: string) => void;
  onDiff?: (turn: number, path: string) => void; onUndo?: (turn: number) => void; onReview?: (turn: number) => void;
  /** "edit from here" on one of your messages */
  onEdit?: (edit: { cp: number; block: number }, text: string) => void;
  /** the checkpoint being edited: that message and everything after it fade */
  editFrom?: number;
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
          {f.added == null ? <span className="font-mono text-[11.5px] text-faint">binary</span> : <>
            <span className="font-mono text-[12px] font-semibold text-ok">+{f.added}</span>
            <span className="font-mono text-[12px] font-semibold text-bad">−{f.removed}</span>
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

export function Transcript({ messages, activity, ...rest }: Props) {
  const { busy, editFrom } = rest;
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
    const on = (e: Event) => { const i = Number((e as CustomEvent).detail); if (!(i >= 0)) return; stick.current = false; setLimit((l) => Math.max(l, messages.length - i)); setJump(i); };
    window.addEventListener("tazuna-jump", on);
    return () => window.removeEventListener("tazuna-jump", on);
  }, [messages.length]);
  useEffect(() => { if (jump == null) return; box.current?.querySelector(`[data-msg="${jump}"]`)?.scrollIntoView({ block: "start" }); setJump(null); }, [jump]);
  return (
    <TranscriptCtx.Provider value={rest}>
    <section ref={box} onScroll={(e) => { const el = e.currentTarget; stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40; }}
      className="min-h-0 flex-1 overflow-auto pb-3 pt-7">
      {hidden > 0 && (
        <div className="mx-auto mb-5 max-w-[700px] px-7">
          <button onClick={() => { stick.current = false; setLimit((l) => l + 40); }} className="rounded-md border border-line px-2.5 py-1 text-[12px] text-muted hover:bg-hover">
            Show {Math.min(40, hidden)} earlier messages <span className="text-faint">({hidden} not shown)</span>
          </button>
        </div>
      )}
      {messages.slice(hidden).map((m, i, shown) => (
        <div key={hidden + i} data-msg={hidden + i} className={`group mx-auto mb-[22px] max-w-[700px] px-7 transition-opacity duration-150 ${editFrom != null && shown.slice(0, i + 1).some((x) => x.role === "user" && x.edit?.cp === editFrom) ? "opacity-35" : ""}`}>
          {m.role === "user" ? <Slot name="message.user" props={{ message: m, index: hidden + i }} only /> : (
            <>
              <div className="mb-1.5 flex items-center gap-2.5 text-[12px] font-medium text-faint">
                tazuna
                {m.routedChat && (
                  <span className="inline-flex items-center gap-1.5 rounded-full bg-accent-soft px-2 py-px text-[11.5px] font-normal text-muted">
                    <MessageCircle size={13} strokeWidth={1.5} /> answered as a chat · <button disabled={busy} onClick={() => m.ask && rest.onSendToAgent?.(m.ask)} className="font-medium text-accent-ink disabled:opacity-50">send to agent</button>
                  </span>
                )}
              </div>
              {m.tools && m.tools.length > 0 && (
                <div className="mb-2.5 mt-0.5 flex flex-col gap-0.5">
                  {m.tools.map((t, j) => <Slot key={j} name="tool.view" props={{ step: t }} only />)}
                </div>
              )}
              <div className="prose-answer text-ink">{m.prose.map((p, j) => <Slot key={j} name="message.text" props={{ text: p }} only />)}</div>
              <Slot name="message.after" props={{ message: m, index: hidden + i }} />
            </>
          )}
        </div>
      ))}
      {busy && activity && doing[activity] && (
        <div className="mx-auto max-w-[700px] px-7 font-mono text-[11.5px] text-faint">{doing[activity]}…</div>
      )}
    </section>
    </TranscriptCtx.Provider>
  );
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
      <div className="mb-1.5 flex items-center gap-2 text-[12px] font-medium text-faint">You{m.queued && <span className="font-normal">· queued, goes after this reply</span>}
        {m.edit && onEdit && !busy && (
          <button onClick={() => onEdit(m.edit!, text)} title="Edit and send again from here: the conversation goes back to before this message"
            className="hidden items-center gap-1 rounded px-1.5 py-0.5 font-normal text-muted hover:bg-hover hover:text-ink group-hover:inline-flex"><Pencil size={12} strokeWidth={1.5} /> Edit</button>
        )}</div>
      {note && <div className="mb-1 text-[11.5px] text-faint">told the model: {note[1]}</div>}
      <div className="inline-block max-w-full select-text rounded-lg bg-field px-3.5 py-2.5 text-ink">{inlineCode(text)}</div>
    </>
  );
}

export function MessageChange({ message: m }: { message: Message }) {
  const { busy, onDiff, onUndo, onReview } = useTranscript();
  return m.role === "agent" && m.change ? <ChangeCard change={m.change} busy={busy} onDiff={onDiff} onUndo={onUndo} onReview={onReview} /> : null;
}

export function MessageMeta({ message: m }: { message: Message }) {
  return m.role === "agent" && m.meta ? <div className="mt-2 font-mono text-[11.5px] text-faint">{meta(m.meta)}</div> : null;
}
