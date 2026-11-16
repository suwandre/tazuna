import { ArrowUp, Check, Gauge, Paperclip, Shield, Square, Zap } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { quotePath, type ModelChoice, type Models } from "../lib/tazuna";
import { usd, type Session } from "../lib/types";
import { Slot } from "../lib/slots";
import { EffortMenu, Menu, ModelPicker } from "./ModelPicker";

/** What the controls under the message box get (the `composer.pills` spot's props): the session, the catalog and the
 * calls they make, and `attach` to put file paths into the box. */
export type ComposerBag = {
  session: Session; busy?: boolean; models?: Models | null; onModels?: () => void; onPickModel?: (c: ModelChoice) => void;
  onPickEffort?: (e: string) => void; onMode?: (mode: string) => void; onPickFiles?: () => Promise<string[]>; attach: (paths: string[]) => void;
};

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
  /** where the box's text is kept (the conversation's id, or "new:<folder>" before it has one): it comes back after a
   * restart, and goes along when the conversation gets its id */
  draftKey?: string;
};

export const DRAFT = "tazuna.draft.";

// the permission modes in tazuna's words, in the order shift-tab steps through them (as in the terminal)
const MODES: [string, string][] = [
  ["auto", "runs commands and edits without asking"],
  ["ask", "asks before each command and edit"],
  ["edits", "edits freely, asks before commands"],
  ["plan", "plans first, asks before starting (Claude only)"],
];

const pill = "inline-flex items-center gap-1.5 rounded-full border border-line bg-field px-2.5 py-[3px] text-[12px] text-ink-2";

export function Composer({ session, threadCost, busy, onSend, onStop, models, onModels, onPickModel, onPickEffort, onMode, onPickFiles, onDrop, draft, above, draftKey }: Props) {
  const [text, setText] = useState(() => (draftKey && localStorage.getItem(DRAFT + draftKey)) || "");
  const box = useRef<HTMLTextAreaElement>(null);
  // draft recovery: what is in the box is saved as it changes, under the conversation's key
  const savedAt = useRef(draftKey);
  useEffect(() => {
    if (!draftKey) return;
    if (savedAt.current && savedAt.current !== draftKey) localStorage.removeItem(DRAFT + savedAt.current);
    savedAt.current = draftKey;
    if (text) localStorage.setItem(DRAFT + draftKey, text); else localStorage.removeItem(DRAFT + draftKey);
  }, [text, draftKey]);
  // a draft from outside applies when it is new (not again when the box of another conversation mounts)
  const seen = useRef(draft?.n);
  useEffect(() => { if (draft && draft.n !== seen.current) { seen.current = draft.n; setText(draft.text); box.current?.focus(); } }, [draft?.n]);
  // attachments go into the message as paths (quoted when they hold a space): the core sends images as images
  const attach = (paths: string[]) => paths.length && setText((t) => `${t}${t && !/\s$/.test(t) ? " " : ""}${paths.map(quotePath).join(" ")} `);
  useEffect(() => {
    if (!onDrop) return;
    const off = onDrop(attach);
    return () => { off.then((f) => f()); };
  }, [onDrop]);
  const nextMode = () => { const i = MODES.findIndex(([m]) => m === session.mode); onMode?.(MODES[(i + 1) % MODES.length][0]); };
  const bag: ComposerBag = { session, busy, models, onModels, onPickModel, onPickEffort, onMode, onPickFiles, attach: (p) => { attach(p); } };
  // a message sent during a reply waits behind it (the core queues it, as the terminal does)
  const send = () => { const t = text.trim(); if (!t) return; onSend?.(t); setText(""); };
  return (
    <footer className="px-7 pb-[22px] pt-2.5">
      <div className="mx-auto max-w-[700px]"><Slot name="composer.dock" props={{}} /></div>
      {above}
      <div className="mx-auto max-w-[700px] rounded-xl border border-line-strong bg-card py-2 pl-4 pr-3 pt-3 shadow-composer">
        <textarea ref={box} rows={1} autoFocus value={text} onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Tab" && e.shiftKey && onMode) { e.preventDefault(); nextMode(); } if (e.key === "Escape" && busy) onStop?.(); if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); send(); } }}
          placeholder={busy ? "Type your next message: it goes after this reply" : "Ask anything, or describe a change…"} className="w-full resize-none bg-transparent pb-3 pt-0.5 text-ink outline-none placeholder:text-faint" />
        <div className="flex items-center gap-1.5">
          <Slot name="composer.pills" props={{ composer: bag }} />
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

// tazuna's own controls under the message box, registered in the composer.pills spot (src/builtins.tsx)

export function ModePill({ composer: c }: { composer: ComposerBag }) {
  const [open, setOpen] = useState(false);
  return (
    <span className="relative">
      <button disabled={!c.onMode} onClick={() => setOpen(!open)} title="Permission mode (shift-tab)" className={`${pill} hover:bg-hover`}>
        <Shield size={14} strokeWidth={1.5} />{c.session.mode}
      </button>
      {open && (
        <Menu onClose={() => setOpen(false)}>
          {MODES.map(([m, about]) => (
            <button key={m} onClick={() => { setOpen(false); c.onMode?.(m); }} className="flex w-full items-start gap-2 px-3 py-[6px] text-left hover:bg-hover">
              <span className="mt-0.5 w-3.5 flex-none text-accent">{m === c.session.mode && <Check size={14} strokeWidth={1.75} />}</span>
              <span><span className="block text-ink">{m}</span><span className="block text-[11.5px] text-faint">{about}</span></span>
            </button>
          ))}
        </Menu>
      )}
    </span>
  );
}

export function ModelPill({ composer: c }: { composer: ComposerBag }) {
  const [open, setOpen] = useState(false);
  const current = c.models?.choices.find((m) => m.current);
  return (
    <span className="relative">
      <button disabled={c.busy || !c.onPickModel} onClick={() => { c.onModels?.(); setOpen(!open); }} title="Model" className={`${pill} hover:bg-hover`}>
        <Zap size={14} strokeWidth={1.5} />{current?.label ?? c.session.model}
      </button>
      {open && <ModelPicker models={c.models ?? null} onClose={() => setOpen(false)} onPick={(m) => { setOpen(false); c.onPickModel?.(m); }} />}
    </span>
  );
}

export function EffortPill({ composer: c }: { composer: ComposerBag }) {
  const [open, setOpen] = useState(false);
  const efforts = c.models?.choices.find((m) => m.current)?.efforts ?? [];
  if (!c.session.effort) return null;
  return (
    <span className="relative">
      <button disabled={c.busy || !c.onPickEffort || efforts.length < 2} onClick={() => { c.onModels?.(); setOpen(!open); }} title="Effort" className={`${pill} hover:bg-hover`}>
        <Gauge size={14} strokeWidth={1.5} />{c.session.effort}
      </button>
      {open && <EffortMenu efforts={efforts} current={c.session.effort} onClose={() => setOpen(false)} onPick={(e) => { setOpen(false); c.onPickEffort?.(e); }} />}
    </span>
  );
}

export function AttachButton({ composer: c }: { composer: ComposerBag }) {
  return (
    <button disabled={!c.onPickFiles} onClick={() => c.onPickFiles?.().then(c.attach)} title="Attach files or images (or drop them on the window)"
      className="rounded-full px-2 py-[3px] text-muted hover:bg-hover disabled:opacity-40"><Paperclip size={14} strokeWidth={1.5} /></button>
  );
}
