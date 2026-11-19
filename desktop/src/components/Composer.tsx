import { ArrowUp, Check, ChevronDown, FolderOpen, Gauge, MessageCircle, Paperclip, Plus, Shield, Square, Zap } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { quotePath, type ModelChoice, type Models } from "../lib/tazuna";
import { modelLabel, type Session } from "../lib/types";
import { Slot } from "../lib/slots";
import { EffortMenu, Menu, ModelPicker } from "./ModelPicker";
import { Sep } from "./Sep";

/** What the controls under the message box get (the `composer.pills` spot's props): the session, the catalog and the
 * calls they make, and `attach` to put file paths into the box. */
/** Where a new conversation runs, before its first message: a chat or a project's folder (the box's first pill). */
export type Where = { label: string; folder: boolean; choices: { label: string; hint: string; path: string | null }[];
  pick: (path: string | null) => void; pickFolder: () => void };

export type ComposerBag = {
  where?: Where;
  session: Session; busy?: boolean; models?: Models | null; onModels?: () => void; onPickModel?: (c: ModelChoice) => void;
  onPickEffort?: (e: string) => void; onMode?: (mode: string) => void; onPickFiles?: () => Promise<string[]>; attach: (paths: string[]) => void;
};

type Props = {
  session: Session; threadCost: number; busy?: boolean; onSend?: (text: string) => void; onStop?: () => void;
  /** a new conversation: where it runs (a chat or a project), changed from the box */
  where?: Where;
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
  /** shown under the box (where a message goes and what it costs) */
  below?: React.ReactNode;
  /** where the box's text is kept (the conversation's id, or "new:<folder>" before it has one): it comes back after a
   * restart, and goes along when the conversation gets its id */
  draftKey?: string;
};

export const DRAFT = "tazuna.draft.";

// the permission modes in tazuna's words, in the order shift-tab steps through them (as in the terminal)
export const MODES: [string, string][] = [
  ["auto", "runs commands and edits without asking"],
  ["ask", "asks before each command and edit"],
  ["edits", "edits freely, asks before commands"],
  ["review", "edits freely; a reviewer model answers command requests, run in a sandbox"],
  ["plan", "reads only: no edits, asks before commands"],
];

// one line always (a narrow window wrapped "Opus 5.5" onto two), and readable while a reply runs (faded, they read as broken)
const pill = "inline-flex flex-none items-center gap-1.5 whitespace-nowrap rounded-full border border-line bg-field px-2.5 py-[3px] text-[12px] text-ink-2 enabled:hover:bg-hover enabled:hover:text-ink disabled:cursor-default disabled:text-muted";

export function Composer({ session, where, busy, onSend, onStop, models, onModels, onPickModel, onPickEffort, onMode, onPickFiles, onDrop, draft, above, below, draftKey }: Props) {
  const [text, setText] = useState(() => (draftKey && localStorage.getItem(DRAFT + draftKey)) || "");
  const box = useRef<HTMLTextAreaElement>(null);
  // the box grows with what is typed, up to 40% of the window, then scrolls (it was one line, and earlier lines went out
  // of sight); at the window's bottom it grows upwards
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, Math.round(window.innerHeight * 0.4))}px`;
  }, [text]);
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
  const bag: ComposerBag = { where, session, busy, models, onModels, onPickModel, onPickEffort, onMode, onPickFiles, attach: (p) => { attach(p); } };
  // a message sent during a reply waits behind it (the core queues it, as the terminal does)
  const send = () => { const t = text.trim(); if (!t) return; onSend?.(t); setText(""); };
  return (
    <footer className="px-7 pb-[22px] pt-2.5">
      <div className="mx-auto max-w-[700px]"><Slot name="composer.dock" props={{}} /></div>
      {above}
      <div data-composer className="mx-auto max-w-[700px] rounded-xl border border-line-strong bg-card py-2 pl-4 pr-3 pt-3 shadow-composer focus-within:border-accent">
        <textarea ref={box} rows={1} autoFocus aria-label="Message" value={text} onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Tab" && e.shiftKey && onMode) { e.preventDefault(); nextMode(); } if (e.key === "Escape" && busy) onStop?.(); if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); send(); } }}
          placeholder={busy ? "Type your next message: it goes after this reply" : "Ask anything, or describe a change…"} className="max-h-[40vh] w-full resize-none overflow-y-auto bg-transparent pb-3 pt-0.5 text-ink outline-none placeholder:text-faint" />
        <div className="flex items-center gap-1.5">
          <Slot name="composer.pills" props={{ composer: bag }} />
          <span className="flex-1" />
          {busy && !text.trim()
            ? <button onClick={onStop} title="Stop (Esc)" className="grid size-[30px] flex-none place-items-center rounded-full bg-ink text-canvas"><Square size={12} strokeWidth={2} fill="currentColor" /></button>
            // solid only when there is something to send (a faded accent read as broken)
            : <button onClick={send} title={busy ? "Send after this reply (Enter)" : "Send (Enter)"} disabled={!text.trim()}
                className={`grid size-[30px] flex-none place-items-center rounded-full ${text.trim() ? "bg-accent text-on-accent" : "border border-line bg-field text-faint"}`}><ArrowUp size={16} strokeWidth={1.75} /></button>}
        </div>
      </div>
      {below && <div className="mx-auto mt-1.5 max-w-[700px] px-1">{below}</div>}
    </footer>
  );
}

// tazuna's own controls under the message box, registered in the composer.pills spot (src/builtins.tsx)

/** A new conversation's place: a chat, or one of the projects (its own conversation there), or a folder to open. */
export function WherePill({ composer: c }: { composer: ComposerBag }) {
  const [open, setOpen] = useState(false);
  const w = c.where;
  if (!w) return null;
  return (
    <span className="relative">
      <button onClick={() => setOpen(!open)} title="Where it runs: a chat, or a project's folder"
        className={`${pill} border-dashed border-line-strong bg-transparent`}>
        {w.folder ? <FolderOpen size={14} strokeWidth={1.5} /> : <MessageCircle size={14} strokeWidth={1.5} />}{w.label}<ChevronDown size={13} strokeWidth={1.5} className="text-faint" />
      </button>
      {open && (
        // downward: on the start screen the box sits under the heading, which an upward menu covered
        <Menu onClose={() => setOpen(false)} width={280}>
          {w.choices.map((ch) => (
            <button key={ch.path ?? "chat"} onClick={() => { setOpen(false); w.pick(ch.path); }} title={ch.hint} className="flex w-full items-center gap-2 px-3 py-[6px] text-left hover:bg-hover">
              <span className="w-3.5 flex-none text-accent">{ch.label === w.label && <Check size={14} strokeWidth={1.75} />}</span>
              <span className="min-w-0 flex-1"><span className="block truncate text-ink">{ch.label}</span><span className="block truncate text-[11.5px] text-faint">{ch.hint}</span></span>
            </button>
          ))}
          <button onClick={() => { setOpen(false); w.pickFolder(); }} className="flex w-full items-center gap-2 border-t border-line-soft px-3 py-[6px] text-left text-ink hover:bg-hover">
            <span className="w-3.5 flex-none text-muted"><Plus size={14} strokeWidth={1.5} /></span>Open a folder…</button>
        </Menu>
      )}
    </span>
  );
}

export function ModePill({ composer: c }: { composer: ComposerBag }) {
  const [open, setOpen] = useState(false);
  return (
    <span className="relative">
      <button disabled={!c.onMode} onClick={() => setOpen(!open)} title="Permission mode (shift-tab)" className={pill}>
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

/** The model and its effort in one pill ("Opus 5.5 · high"), both chosen in its picker: the effort levels at the top. */
export function ModelPill({ composer: c }: { composer: ComposerBag }) {
  const [open, setOpen] = useState(false);
  const current = c.models?.choices.find((m) => m.current);
  const toggle = () => { c.onModels?.(); setOpen(!open); };
  return (
    <span className="relative">
      <button disabled={c.busy || !c.onPickModel} onClick={toggle} title="Model and effort" className={pill}>
        <Zap size={14} strokeWidth={1.5} />{modelLabel(current?.model ?? c.session.model)}
        {c.session.effort && <span className="flex items-center text-faint"><Sep />{c.session.effort}</span>}
        <ChevronDown size={13} strokeWidth={1.5} className="text-faint" />
      </button>
      {open && <ModelPicker models={c.models ?? null} onClose={() => setOpen(false)} onPick={(m) => { setOpen(false); c.onPickModel?.(m); }}
        efforts={current?.efforts ?? []} effort={c.session.effort} onEffort={c.onPickEffort} />}
    </span>
  );
}

export function EffortPill({ composer: c }: { composer: ComposerBag }) {
  const [open, setOpen] = useState(false);
  const efforts = c.models?.choices.find((m) => m.current)?.efforts ?? [];
  if (!c.session.effort) return null;
  return (
    <span className="relative">
      <button disabled={c.busy || !c.onPickEffort || efforts.length < 2} onClick={() => { c.onModels?.(); setOpen(!open); }} title="Effort" className={pill}>
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
