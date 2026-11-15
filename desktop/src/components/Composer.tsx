import { ArrowUp, Gauge, Paperclip, Shield, Square, Zap } from "lucide-react";
import { useState } from "react";
import type { ModelChoice, Models } from "../lib/tazuna";
import { usd, type Session } from "../lib/types";
import { EffortMenu, ModelPicker } from "./ModelPicker";

type Props = {
  session: Session; threadCost: number; busy?: boolean; onSend?: (text: string) => void; onStop?: () => void;
  /** the model catalog (null until asked for); onModels asks for it when the picker opens */
  models?: Models | null; onModels?: () => void; onPickModel?: (c: ModelChoice) => void; onPickEffort?: (e: string) => void;
};

const pill = "inline-flex items-center gap-1.5 rounded-full border border-line bg-field px-2.5 py-[3px] text-[12px] text-ink-2";

export function Composer({ session, threadCost, busy, onSend, onStop, models, onModels, onPickModel, onPickEffort }: Props) {
  const [text, setText] = useState("");
  const [menu, setMenu] = useState<"model" | "effort" | null>(null);
  const current = models?.choices.find((c) => c.current);
  const efforts = current?.efforts ?? [];
  const send = () => { const t = text.trim(); if (!t || busy) return; onSend?.(t); setText(""); };
  return (
    <footer className="px-7 pb-[22px] pt-2.5">
      <div className="mx-auto max-w-[700px] rounded-xl border border-line-strong bg-card py-2 pl-4 pr-3 pt-3 shadow-composer">
        <textarea rows={1} autoFocus value={text} onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Escape" && busy) onStop?.(); if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); send(); } }}
          placeholder="Ask anything, or describe a change…" className="w-full resize-none bg-transparent pb-3 pt-0.5 text-ink outline-none placeholder:text-faint" />
        <div className="flex items-center gap-1.5">
          <span className={pill}><Shield size={14} strokeWidth={1.5} />{session.mode}</span>
          <span className="relative">
            <button disabled={busy || !onPickModel} onClick={() => { onModels?.(); setMenu(menu === "model" ? null : "model"); }} title="Model" className={`${pill} hover:bg-hover`}>
              <Zap size={14} strokeWidth={1.5} />{current?.label ?? session.model}
            </button>
            {menu === "model" && <ModelPicker models={models ?? null} onClose={() => setMenu(null)} onPick={(c) => { setMenu(null); onPickModel?.(c); }} />}
          </span>
          {session.effort && (
            <span className="relative">
              <button disabled={busy || !onPickEffort || efforts.length < 2} onClick={() => { onModels?.(); setMenu(menu === "effort" ? null : "effort"); }} title="Effort" className={`${pill} hover:bg-hover`}>
                <Gauge size={14} strokeWidth={1.5} />{session.effort}
              </button>
              {menu === "effort" && <EffortMenu efforts={efforts} current={session.effort} onClose={() => setMenu(null)} onPick={(e) => { setMenu(null); onPickEffort?.(e); }} />}
            </span>
          )}
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
