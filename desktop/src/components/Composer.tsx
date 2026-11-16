import { ArrowUp, Check, Gauge, Paperclip, Shield, Square, Zap } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { quotePath, type ModelChoice, type Models } from "../lib/tazuna";
import { usd, type Session } from "../lib/types";
import { EffortMenu, Menu, ModelPicker } from "./ModelPicker";

type Props = {
  session: Session; threadCost: number; busy?: boolean; onSend?: (text: string) => void; onStop?: () => void;
  /** the model catalog (null until asked for); onModels asks for it when the picker opens */
  models?: Models | null; onModels?: () => void; onPickModel?: (c: ModelChoice) => void; onPickEffort?: (e: string) => void;
  onMode?: (mode: string) => void;
  /** files to attach (the native picker); dropped files come through onDrop */
  onPickFiles?: () => Promise<string[]>;
  onDrop?: (cb: (paths: string[]) => void) => Promise<() => void>;
  /** text put in the box from outside ("edit from here"): set whenever `n` changes */
  draft?: { text: string; n: number };
  /** shown above the box (the edit banner) */
  above?: React.ReactNode;
};

// the permission modes in tazuna's words, in the order shift-tab steps through them (as in the terminal)
const MODES: [string, string][] = [
  ["auto", "runs commands and edits without asking"],
  ["ask", "asks before each command and edit"],
  ["edits", "edits freely, asks before commands"],
  ["plan", "plans first, asks before starting (Claude only)"],
];

const pill = "inline-flex items-center gap-1.5 rounded-full border border-line bg-field px-2.5 py-[3px] text-[12px] text-ink-2";

export function Composer({ session, threadCost, busy, onSend, onStop, models, onModels, onPickModel, onPickEffort, onMode, onPickFiles, onDrop, draft, above }: Props) {
  const [text, setText] = useState("");
  const [menu, setMenu] = useState<"model" | "effort" | "mode" | null>(null);
  const box = useRef<HTMLTextAreaElement>(null);
  useEffect(() => { if (draft) { setText(draft.text); box.current?.focus(); } }, [draft?.n]);
  // attachments go into the message as paths (quoted when they hold a space): the core sends images as images
  const attach = (paths: string[]) => paths.length && setText((t) => `${t}${t && !/\s$/.test(t) ? " " : ""}${paths.map(quotePath).join(" ")} `);
  useEffect(() => {
    if (!onDrop) return;
    const off = onDrop(attach);
    return () => { off.then((f) => f()); };
  }, [onDrop]);
  const nextMode = () => { const i = MODES.findIndex(([m]) => m === session.mode); onMode?.(MODES[(i + 1) % MODES.length][0]); };
  const current = models?.choices.find((c) => c.current);
  const efforts = current?.efforts ?? [];
  // a message sent during a reply waits behind it (the core queues it, as the terminal does)
  const send = () => { const t = text.trim(); if (!t) return; onSend?.(t); setText(""); };
  return (
    <footer className="px-7 pb-[22px] pt-2.5">
      {above}
      <div className="mx-auto max-w-[700px] rounded-xl border border-line-strong bg-card py-2 pl-4 pr-3 pt-3 shadow-composer">
        <textarea ref={box} rows={1} autoFocus value={text} onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Tab" && e.shiftKey && onMode) { e.preventDefault(); nextMode(); } if (e.key === "Escape" && busy) onStop?.(); if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); send(); } }}
          placeholder={busy ? "Type your next message: it goes after this reply" : "Ask anything, or describe a change…"} className="w-full resize-none bg-transparent pb-3 pt-0.5 text-ink outline-none placeholder:text-faint" />
        <div className="flex items-center gap-1.5">
          <span className="relative">
            <button disabled={!onMode} onClick={() => setMenu(menu === "mode" ? null : "mode")} title="Permission mode (shift-tab)" className={`${pill} hover:bg-hover`}>
              <Shield size={14} strokeWidth={1.5} />{session.mode}
            </button>
            {menu === "mode" && (
              <Menu onClose={() => setMenu(null)}>
                {MODES.map(([m, about]) => (
                  <button key={m} onClick={() => { setMenu(null); onMode?.(m); }} className="flex w-full items-start gap-2 px-3 py-[6px] text-left hover:bg-hover">
                    <span className="mt-0.5 w-3.5 flex-none text-accent">{m === session.mode && <Check size={14} strokeWidth={1.75} />}</span>
                    <span><span className="block text-ink">{m}</span><span className="block text-[11.5px] text-faint">{about}</span></span>
                  </button>
                ))}
              </Menu>
            )}
          </span>
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
          <button disabled={!onPickFiles} onClick={() => onPickFiles?.().then(attach)} title="Attach files or images (or drop them on the window)"
            className="rounded-full px-2 py-[3px] text-muted hover:bg-hover disabled:opacity-40"><Paperclip size={14} strokeWidth={1.5} /></button>
          <span className="flex-1" />
          <span className="mr-1.5 font-mono text-[11.5px] text-faint">last {usd(session.lastCost)} · thread {usd(threadCost)}</span>
          {busy && !text.trim()
            ? <button onClick={onStop} title="Stop (Esc)" className="grid size-[30px] place-items-center rounded-full bg-ink text-canvas"><Square size={12} strokeWidth={2} fill="currentColor" /></button>
            : <button onClick={send} title={busy ? "Send after this reply (Enter)" : "Send (Enter)"} disabled={!text.trim()} className="grid size-[30px] place-items-center rounded-full bg-accent text-on-accent disabled:opacity-40"><ArrowUp size={16} strokeWidth={1.75} /></button>}
        </div>
      </div>
    </footer>
  );
}
