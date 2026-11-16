import { Check, Circle, FileText, MessageCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Markdown } from "../lib/markdown";
import { usd, type Change, type Message } from "../lib/types";

function inlineCode(text: string) {
  return text.split(/(`[^`]+`)/).map((part, i) => part.startsWith("`") ? <code key={i}>{part.slice(1, -1)}</code> : part);
}

function meta(m: { cost: number; tokens: number; cachedPct?: number; secs: number }) {
  const tokens = m.tokens >= 1000 ? `${(m.tokens / 1000).toFixed(1)}k` : `${m.tokens}`;
  return [usd(m.cost), `${tokens} tokens`, m.cachedPct != null ? `${m.cachedPct}% cached` : null, `${m.secs} s`].filter(Boolean).join(" · ");
}

const doing: Record<string, string> = { Starting: "starting", Waiting: "waiting for the model", Thinking: "thinking", Writing: "writing", Tool: "working", Verifying: "running the checks" };

type Props = {
  messages: Message[]; busy?: boolean; activity?: string; onSendToAgent?: (text: string) => void;
  onDiff?: (turn: number, path: string) => void; onUndo?: (turn: number) => void;
};

const STATUS: Record<string, string> = { A: "added", D: "deleted", M: "changed" };

/** What a turn changed: each file with its lines added and removed and its diff; undo for the last turn. */
function ChangeCard({ change, busy, onDiff, onUndo }: { change: Change; busy?: boolean; onDiff?: Props["onDiff"]; onUndo?: Props["onUndo"] }) {
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
        {change.undone ? <span className="text-faint">undone</span>
          : change.canUndo && <button disabled={busy} onClick={() => onUndo?.(change.turn)} title="Put these files back as they were before this turn"
              className="rounded-md border border-line px-2.5 py-0.5 text-ink hover:bg-hover disabled:opacity-50">Undo</button>}
      </div>
    </div>
  );
}

export function Transcript({ messages, busy, activity, onSendToAgent, onDiff, onUndo }: Props) {
  // follow new output while the reader is at the bottom; stay put once they scroll up
  const box = useRef<HTMLElement>(null), stick = useRef(true);
  useEffect(() => { if (stick.current && box.current) box.current.scrollTop = box.current.scrollHeight; });
  return (
    <section ref={box} onScroll={(e) => { const el = e.currentTarget; stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40; }}
      className="flex-1 overflow-auto pb-3 pt-7">
      {messages.map((m, i) => (
        <div key={i} className="mx-auto mb-[22px] max-w-[700px] px-7">
          {m.role === "user" ? (
            <>
              <div className="mb-1.5 text-[12px] font-medium text-faint">You</div>
              {(() => {
                // the note tazuna adds after an undo ("[tazuna: ...]") is shown as what it is, apart from the message
                const note = m.text.match(/^\[tazuna: ([^\]]*)\]\n\n/);
                const text = note ? m.text.slice(note[0].length) : m.text;
                return <>
                  {note && <div className="mb-1 text-[11.5px] text-faint">told the model: {note[1]}</div>}
                  <div className="inline-block max-w-full select-text rounded-lg bg-field px-3.5 py-2.5 text-ink">{inlineCode(text)}</div>
                </>;
              })()}
            </>
          ) : (
            <>
              <div className="mb-1.5 flex items-center gap-2.5 text-[12px] font-medium text-faint">
                tazuna
                {m.routedChat && (
                  <span className="inline-flex items-center gap-1.5 rounded-full bg-accent-soft px-2 py-px text-[11.5px] font-normal text-muted">
                    <MessageCircle size={13} strokeWidth={1.5} /> answered as a chat · <button disabled={busy} onClick={() => m.ask && onSendToAgent?.(m.ask)} className="font-medium text-accent-ink disabled:opacity-50">send to agent</button>
                  </span>
                )}
              </div>
              {m.tools && (
                <div className="mb-2.5 mt-0.5 flex flex-col gap-0.5">
                  {m.tools.map((t, j) => (
                    <div key={j} className="flex items-center gap-2 py-0.5 text-[12.5px] text-muted">
                      {t.running ? <Circle size={10} strokeWidth={2} className="mx-0.5 text-accent" /> : <Check size={14} strokeWidth={1.5} className={t.ok ? "text-ok" : "text-bad"} />}
                      {t.verb} <span className="font-mono text-[11.5px] text-ink-2">{t.target}</span>
                      {t.detail && <span>· {t.detail}</span>}
                      {!t.running && <span className="ml-auto font-mono text-[11px] text-faint">{t.secs.toFixed(1)} s</span>}
                    </div>
                  ))}
                </div>
              )}
              <div className="prose-answer text-ink">{m.prose.map((p, j) => <Markdown key={j} text={p} />)}</div>
              {m.change && <ChangeCard change={m.change} busy={busy} onDiff={onDiff} onUndo={onUndo} />}
              {m.meta && <div className="mt-2 font-mono text-[11.5px] text-faint">{meta(m.meta)}</div>}
            </>
          )}
        </div>
      ))}
      {busy && activity && doing[activity] && (
        <div className="mx-auto max-w-[700px] px-7 font-mono text-[11.5px] text-faint">{doing[activity]}…</div>
      )}
    </section>
  );
}
