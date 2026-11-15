import { ArrowUp, Paperclip, Shield, Zap } from "lucide-react";
import type { Session } from "../lib/types";

export function Composer({ session, threadCost }: { session: Session; threadCost: number }) {
  return (
    <footer className="px-7 pb-[22px] pt-2.5">
      <div className="mx-auto max-w-[700px] rounded-xl border border-line-strong bg-card py-2 pl-4 pr-3 pt-3 shadow-composer">
        <textarea rows={1} placeholder="Ask anything, or describe a change…" className="w-full resize-none bg-transparent pb-3 pt-0.5 text-ink outline-none placeholder:text-faint" />
        <div className="flex items-center gap-1.5">
          <span className="inline-flex items-center gap-1.5 rounded-full border border-line bg-field px-2.5 py-[3px] text-[12px] text-ink-2"><Shield size={14} strokeWidth={1.5} />{session.mode}</span>
          <span className="inline-flex items-center gap-1.5 rounded-full border border-line bg-field px-2.5 py-[3px] text-[12px] text-ink-2"><Zap size={14} strokeWidth={1.5} />{session.model} · {session.effort}</span>
          <button className="rounded-full px-2 py-[3px] text-muted"><Paperclip size={14} strokeWidth={1.5} /></button>
          <span className="flex-1" />
          <span className="mr-1.5 font-mono text-[11.5px] text-faint">last ${session.lastCost.toFixed(3)} · thread ${threadCost.toFixed(3)}</span>
          <button className="grid size-[30px] place-items-center rounded-full bg-accent text-on-accent"><ArrowUp size={16} strokeWidth={1.75} /></button>
        </div>
      </div>
    </footer>
  );
}
