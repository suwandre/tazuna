import { Check, FileText, MessageCircle } from "lucide-react";
import type { Message } from "../lib/types";

function inlineCode(text: string) {
  return text.split(/(`[^`]+`)/).map((part, i) => part.startsWith("`") ? <code key={i}>{part.slice(1, -1)}</code> : part);
}

function meta(m: { cost: number; tokens: number; cachedPct?: number; secs: number }) {
  const tokens = m.tokens >= 1000 ? `${(m.tokens / 1000).toFixed(1)}k` : `${m.tokens}`;
  return [`$${m.cost.toFixed(3)}`, `${tokens} tokens`, m.cachedPct != null ? `${m.cachedPct}% cached` : null, `${m.secs} s`].filter(Boolean).join(" · ");
}

export function Transcript({ messages }: { messages: Message[] }) {
  return (
    <section className="flex-1 overflow-auto pb-3 pt-7">
      {messages.map((m, i) => (
        <div key={i} className="mx-auto mb-[22px] max-w-[700px] px-7">
          {m.role === "user" ? (
            <>
              <div className="mb-1.5 text-[12px] font-medium text-faint">You</div>
              <div className="inline-block max-w-full select-text rounded-lg bg-field px-3.5 py-2.5 text-ink">{inlineCode(m.text)}</div>
            </>
          ) : (
            <>
              <div className="mb-1.5 flex items-center gap-2.5 text-[12px] font-medium text-faint">
                tazuna
                {m.routedChat && (
                  <span className="inline-flex items-center gap-1.5 rounded-full bg-accent-soft px-2 py-px text-[11.5px] font-normal text-muted">
                    <MessageCircle size={13} strokeWidth={1.5} /> answered as a chat · <button className="font-medium text-accent-ink">send to agent</button>
                  </span>
                )}
              </div>
              {m.tools && (
                <div className="mb-2.5 mt-0.5 flex flex-col gap-0.5">
                  {m.tools.map((t, j) => (
                    <div key={j} className="flex items-center gap-2 py-0.5 text-[12.5px] text-muted">
                      <Check size={14} strokeWidth={1.5} className={t.ok ? "text-ok" : "text-bad"} />
                      {t.verb} <span className="font-mono text-[11.5px] text-ink-2">{t.target}</span>
                      {t.detail && <span>· {t.detail}</span>}
                      <span className="ml-auto font-mono text-[11px] text-faint">{t.secs.toFixed(1)} s</span>
                    </div>
                  ))}
                </div>
              )}
              <div className="prose-answer text-ink">{m.prose.map((p, j) => <p key={j}>{p}</p>)}</div>
              {m.change && (
                <div className="mt-1 flex items-center gap-2.5 rounded-lg border border-line bg-card px-3 py-[9px] text-[12.5px]">
                  <span className="flex flex-1 items-center gap-[7px] font-mono text-[12px] text-ink-2"><FileText size={14} strokeWidth={1.5} />{m.change.file}</span>
                  <span className="font-mono text-[12px] font-semibold text-ok">+{m.change.added}</span>
                  <span className="font-mono text-[12px] font-semibold text-bad">−{m.change.removed}</span>
                  <button className="rounded-md bg-ink px-2.5 py-1 text-[12px] font-medium text-canvas">View diff</button>
                  <button className="rounded-md border border-line px-2.5 py-1 text-[12px] text-muted">Undo</button>
                </div>
              )}
              {m.meta && <div className="mt-2 font-mono text-[11.5px] text-faint">{meta(m.meta)}</div>}
            </>
          )}
        </div>
      ))}
    </section>
  );
}
