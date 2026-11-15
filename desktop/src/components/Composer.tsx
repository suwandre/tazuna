import { ArrowUp, Paperclip, Shield, Square, Zap } from "lucide-react";
import { useState } from "react";
import { usd, type Session } from "../lib/types";

type Props = { session: Session; threadCost: number; busy?: boolean; onSend?: (text: string) => void; onStop?: () => void };

export function Composer({ session, threadCost, busy, onSend, onStop }: Props) {
  const [text, setText] = useState("");
  const send = () => { const t = text.trim(); if (!t || busy) return; onSend?.(t); setText(""); };
  return (
    <footer className="px-7 pb-[22px] pt-2.5">
      <div className="mx-auto max-w-[700px] rounded-xl border border-line-strong bg-card py-2 pl-4 pr-3 pt-3 shadow-composer">
        <textarea rows={1} autoFocus value={text} onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Escape" && busy) onStop?.(); if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); send(); } }}
          placeholder="Ask anything, or describe a change…" className="w-full resize-none bg-transparent pb-3 pt-0.5 text-ink outline-none placeholder:text-faint" />
        <div className="flex items-center gap-1.5">
          <span className="inline-flex items-center gap-1.5 rounded-full border border-line bg-field px-2.5 py-[3px] text-[12px] text-ink-2"><Shield size={14} strokeWidth={1.5} />{session.mode}</span>
          <span className="inline-flex items-center gap-1.5 rounded-full border border-line bg-field px-2.5 py-[3px] text-[12px] text-ink-2"><Zap size={14} strokeWidth={1.5} />{session.model} · {session.effort}</span>
          <button className="rounded-full px-2 py-[3px] text-muted"><Paperclip size={14} strokeWidth={1.5} /></button>
          <span className="flex-1" />
          <span className="mr-1.5 font-mono text-[11.5px] text-faint">last {usd(session.lastCost)} · thread {usd(threadCost)}</span>
          {busy
            ? <button onClick={onStop} title="Stop (Esc)" className="grid size-[30px] place-items-center rounded-full bg-ink text-canvas"><Square size={12} strokeWidth={2} fill="currentColor" /></button>
            : <button onClick={send} title="Send (Enter)" disabled={!text.trim()} className="grid size-[30px] place-items-center rounded-full bg-accent text-on-accent disabled:opacity-40"><ArrowUp size={16} strokeWidth={1.75} /></button>}
        </div>
      </div>
    </footer>
  );
}
