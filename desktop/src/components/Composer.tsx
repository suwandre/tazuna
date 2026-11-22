import { ArrowUp, Check, ChevronDown, FileText, FolderOpen, Gauge, MessageCircle, Paperclip, Plus, Shield, Square, X, Zap } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { quotePath, type ModelChoice, type Models } from "../lib/tazuna";
import { modelLabel, type Session } from "../lib/types";
import { Slot } from "../lib/slots";
import { EffortMenu, Menu, ModelPicker, usePlace } from "./ModelPicker";
import { Sep } from "./Sep";

/** What the controls under the message box get (the `composer.pills` spot's props): the session, the catalog and the
 * calls they make, and `attach` to put file paths into the box. */
/** Where a new conversation runs, before its first message: a chat or a project's folder (the box's first pill). */
export type Where = { label: string; folder: boolean; choices: { label: string; hint: string; path: string | null }[];
  pick: (path: string | null) => void; pickFolder: () => void };

export type ComposerBag = {
  where?: Where;
  /** a plain chat (no project): the model alone, so no permission pill (design review 10) */
  chat?: boolean;
  session: Session; busy?: boolean; models?: Models | null; onModels?: () => void; onPickModel?: (c: ModelChoice) => void;
  /** a model whose provider has no key: Settings on that provider */
  onAddKey?: (provider: string) => void;
  onPickEffort?: (e: string) => void; onMode?: (mode: string) => void; onPickFiles?: () => Promise<string[]>; attach: (paths: string[]) => void;
  /** counts the `/model` (or `/effort`) asks: the model picker opens at each new one */
  picker?: number;
};

type Props = {
  session: Session; threadCost: number; busy?: boolean; onSend?: (text: string) => void; onStop?: () => void;
  /** a new conversation: where it runs (a chat or a project), changed from the box */
  where?: Where;
  /** a plain chat (no project) */
  chat?: boolean;
  /** the model catalog (null until asked for); onModels asks for it when the picker opens */
  models?: Models | null; onModels?: () => void; onPickModel?: (c: ModelChoice) => void; onPickEffort?: (e: string) => void;
  onAddKey?: (provider: string) => void; onMode?: (mode: string) => void;
  /** files to attach (the native picker); dropped files come through onDrop */
  onPickFiles?: () => Promise<string[]>;
  onDrop?: (cb: (paths: string[]) => void) => Promise<() => void>;
  /** text put in the box from outside ("edit from here"): set whenever `n` changes */
  draft?: { text: string; n: number };
  /** shown above the box (the edit banner) */
  above?: React.ReactNode;
  /** shown under the box (where a message goes and what it costs) */
  below?: React.ReactNode;
  /** what `/` and `@` complete to in this conversation (the core's "complete": its commands with what each does, the
   * project's files); null until it answers. onComplete asks for it. */
  complete?: Completions | null; onComplete?: () => void;
  /** an attached image as a data URL, for its chip's thumbnail */
  onImage?: (path: string) => Promise<string>;
  /** where the box's text is kept (the conversation's id, or "new:<folder>" before it has one): it comes back after a
   * restart, and goes along when the conversation gets its id */
  draftKey?: string;
  /** the window's own commands (`/mcp`, `/model`…): what each does, and what runs when one is taken or sent alone */
  commands?: Record<string, [about: string, run: () => void]>;
};

export const DRAFT = "tazuna.draft.";

export type Completions = { commands: [string, string][]; files: string[] };
// the window's own commands, there before (and without) the core's list
const OWN: [string, string][] = [["/create", "Make a command, skill, tool, panel or plugin"], ["/agent", "Send to the coding agent, past the chat router"]];
const IMAGE = /\.(png|jpe?g|gif|webp|svg|bmp|avif)$/i;
type Pick = { text: string; label: string; about: string };

/** What the word at the caret completes to: commands when the box holds only `/…`, files for an `@…` word anywhere. As the
 * terminal ranks them (src/editor.rs): a name that starts with the typed text first, then one that holds it. */
export function completions(text: string, caret: number, c: Completions | null | undefined, extra: [string, string][] = []): { from: number; items: Pick[] } | null {
  const head = text.slice(0, caret);
  const slash = /^\/(\S*)$/.exec(head);
  if (slash) {
    const mine = [...OWN, ...extra], q = slash[1].toLowerCase(), all = [...mine, ...(c?.commands ?? []).filter(([n]) => !mine.some(([o]) => o === n))];
    const hit = (start: boolean) => all.filter(([n]) => { const i = n.slice(1).toLowerCase().indexOf(q); return start ? i === 0 : i > 0; });
    return { from: 0, items: [...hit(true), ...hit(false)].slice(0, 40).map(([n, about]) => ({ text: `${n} `, label: n, about })) };
  }
  const at = /(?:^|\s)@(\S*)$/.exec(head);
  if (!at || !c?.files.length) return null;
  const q = at[1].toLowerCase(), name = (f: string) => f.slice(f.lastIndexOf("/") + 1);
  const ranked = c.files.map((f) => { const l = f.toLowerCase(); return [l.startsWith(q) || name(l).startsWith(q) ? 0 : l.includes(q) ? 1 : 2, f] as const; })
    .filter(([r]) => r < 2).sort((a, b) => a[0] - b[0] || a[1].length - b[1].length).slice(0, 8);
  return { from: caret - at[1].length - 1, items: ranked.map(([, f]) => ({ text: `@${f} `, label: name(f), about: f.slice(0, f.length - name(f).length) })) };
}

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

export function Composer({ session, where, chat, busy, onSend, onStop, models, onModels, onPickModel, onPickEffort, onAddKey, onMode, onPickFiles, onDrop, draft, above, below, draftKey, complete, onComplete, onImage, commands = {} }: Props) {
  const [text, setText] = useState(() => (draftKey && localStorage.getItem(DRAFT + draftKey)) || "");
  // attachments are chips over the text, with a thumbnail for an image (until desktop 0.1.87 their paths were typed into
  // the message, and a screenshot was a line of path)
  const [files, setFiles] = useState<string[]>([]);
  // the menu over the box for the `/` or `@` word at the caret; Esc shuts it until the text changes
  const [caret, setCaret] = useState(text.length), [sel, setSel] = useState(0), [shut, setShut] = useState(false);
  // the box's own: `/model` and `/effort` open the model pill's picker (the effort levels are at its top)
  const [picker, setPicker] = useState(0);
  const ask = () => setPicker((n) => n + 1);
  const mine: Record<string, [string, () => void]> = onPickModel ? { model: ["Pick the model and its effort", ask], effort: ["The model's effort level", ask], ...commands } : commands;
  const extra = useMemo(() => Object.entries(mine).map(([n, [about]]) => [`/${n}`, about] as [string, string]), [commands, !!onPickModel]);
  const found = useMemo(() => (shut ? null : completions(text, caret, complete, extra)), [text, caret, complete, shut, extra]);
  const menu = found?.items.length ? found : null;
  useEffect(() => { setSel(0); }, [menu?.items.length, menu?.from]);
  const asked = useRef(false);
  // asked for at the first `/` or `@` of each message: a conversation's commands are known once its first turn ran
  useEffect(() => { if (!text) asked.current = false; else if (!asked.current && /(^\/|(^|\s)@)/.test(text)) { asked.current = true; onComplete?.(); } }, [text]);
  // a window command runs when taken: it opens a panel, and has nothing to type after it
  const run = (cmd: string) => { const name = cmd.startsWith("/") ? cmd.slice(1) : ""; if (!Object.hasOwn(mine, name)) return false; setText(""); mine[name][1](); return true; };
  const take = (p: Pick) => {
    if (run(p.label)) return;
    const next = text.slice(0, menu!.from) + p.text + text.slice(caret), at = menu!.from + p.text.length;
    setText(next); setCaret(at);
    requestAnimationFrame(() => { box.current?.focus(); box.current?.setSelectionRange(at, at); });
  };
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
  const attach = (paths: string[]) => paths.length && setFiles((f) => [...new Set([...f, ...paths])]);
  useEffect(() => {
    if (!onDrop) return;
    const off = onDrop(attach);
    return () => { off.then((f) => f()); };
  }, [onDrop]);
  const nextMode = () => { const i = MODES.findIndex(([m]) => m === session.mode); onMode?.(MODES[(i + 1) % MODES.length][0]); };
  const bag: ComposerBag = { where, chat, session, busy, models, onModels, onPickModel, onPickEffort, onAddKey, onMode, onPickFiles, attach: (p) => { attach(p); }, picker };
  // a message sent during a reply waits behind it (the core queues it, as the terminal does)
  // attachments go in the message as paths after its text (quoted when they hold a space): the core sends images as images
  const send = () => { if (!files.length && run(text.trim())) return; const t = [text.trim(), ...files.map(quotePath)].filter(Boolean).join(" "); if (!t) return; onSend?.(t); setText(""); setFiles([]); };
  const ready = !!text.trim() || files.length > 0;
  return (
    <footer className="px-7 pb-[22px] pt-2.5">
      <div className="mx-auto max-w-[700px]"><Slot name="composer.dock" props={{}} /></div>
      {above}
      <div data-composer className="mx-auto max-w-[700px] rounded-xl border border-line-strong bg-card py-2 pl-4 pr-3 pt-3 shadow-composer focus-within:border-accent">
        {menu && <div className="relative -ml-4 -mr-3"><CompleteMenu items={menu.items} sel={sel} onPick={take} /></div>}
        {files.length > 0 && (
          <div className="mb-2 flex flex-wrap gap-1.5">
            {files.map((f) => <Chip key={f} path={f} onImage={onImage} onRemove={() => setFiles((all) => all.filter((x) => x !== f))} />)}
          </div>
        )}
        <textarea ref={box} rows={1} autoFocus aria-label="Message" value={text} onChange={(e) => { setText(e.target.value); setCaret(e.target.selectionStart); setShut(false); }}
          onSelect={(e) => setCaret(e.currentTarget.selectionStart)} aria-expanded={!!menu} aria-controls={menu ? "tz-complete" : undefined}
          onKeyDown={(e) => {
            if (menu && !e.nativeEvent.isComposing) {
              const n = menu.items.length;
              if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); setSel((i) => (i + (e.key === "ArrowDown" ? 1 : n - 1)) % n); return; }
              if (e.key === "Enter" || (e.key === "Tab" && !e.shiftKey)) { e.preventDefault(); take(menu.items[sel] ?? menu.items[0]); return; }
              if (e.key === "Escape") { e.preventDefault(); setShut(true); return; }
            }
            if (e.key === "Tab" && e.shiftKey && onMode) { e.preventDefault(); nextMode(); } if (e.key === "Escape" && busy) onStop?.(); if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); send(); } }}
          placeholder={busy ? "Type your next message: it goes after this reply" : "Ask anything, or describe a change…  / for commands, @ for files"} className="max-h-[40vh] w-full resize-none overflow-y-auto bg-transparent pb-3 pt-0.5 text-ink outline-none placeholder:text-faint" />
        <div className="flex items-center gap-1.5">
          <Slot name="composer.pills" props={{ composer: bag }} />
          <span className="flex-1" />
          {busy && !ready
            ? <button onClick={onStop} title="Stop (Esc)" className="grid size-[30px] flex-none place-items-center rounded-full bg-ink text-canvas"><Square size={12} strokeWidth={2} fill="currentColor" /></button>
            // solid only when there is something to send (a faded accent read as broken)
            : <button onClick={send} title={busy ? "Send after this reply (Enter)" : "Send (Enter)"} disabled={!ready}
                className={`grid size-[30px] flex-none place-items-center rounded-full ${ready ? "bg-accent text-on-accent" : "border border-line bg-field text-faint"}`}><ArrowUp size={16} strokeWidth={1.75} /></button>}
        </div>
      </div>
      {below && <div className="mx-auto mt-1.5 max-w-[700px] px-1">{below}</div>}
    </footer>
  );
}

/** A check of what the menu offers: `npx tsx -e 'import("./src/components/Composer.tsx").then(m => m.demo())'`. */
export function demo() {
  const c: Completions = { commands: [["/compact", "c"], ["/review", "r"], ["/security-review", "s"], ["/create", "core's own"]], files: ["src/retry.ts", "tests/retry.test.ts", "docs/policy.md", "src/entry.ts"] };
  const labels = (text: string, caret = text.length) => completions(text, caret, c)?.items.map((i) => i.label).join(",") ?? null;
  const want: [string, string | null][] = [["/", "/create,/agent,/compact,/review,/security-review"], ["/re", "/review,/create,/security-review"], ["/review now", null], ["see /re", null],
    ["why @ret", "retry.ts,retry.test.ts"], ["why @try", "retry.ts,entry.ts,retry.test.ts"], ["mail a@b.c", null], ["@zzz", ""], ["plain", null]];
  for (const [text, exp] of want) if (labels(text) !== exp) throw new Error(`completions(${text}): ${labels(text)}, wanted ${exp}`);
  if (completions("why @ret", 8, null) !== null || labels("/", 1) === null) throw new Error("completions: no list yet");
  // the window's own commands after /create and /agent, one of the core's list with the same name only once
  const mine = completions("/", 1, { ...c, commands: [...c.commands, ["/mcp", "claude's"]] }, [["/mcp", "window"], ["/model", "window"]])!.items;
  if (mine.map((i) => i.label).join(",") !== "/create,/agent,/mcp,/model,/compact,/review,/security-review" || mine[2].about !== "window") throw new Error("completions: window commands");
  const at = completions("why @ret now", 8, c)!;
  if (at.from !== 4 || at.items[0].text !== "@src/retry.ts ") throw new Error("completions: what is taken");
  console.log("completions ok");
}

/** The `/` and `@` menu over the message box: the keyboard stays in the box (arrows, Enter or Tab to take, Esc to shut). */
function CompleteMenu({ items, sel, onPick }: { items: Pick[]; sel: number; onPick: (p: Pick) => void }) {
  const box = useRef<HTMLDivElement>(null);
  const place = usePlace(box, 300);
  return (
    <div ref={box} id="tz-complete" role="listbox" aria-label="Completions" style={place.style} onMouseDown={(e) => e.preventDefault()}
      className={`tz-pop absolute ${place.side} left-0 z-10 flex w-full flex-col overflow-hidden rounded-xl border border-line-strong bg-card`}>
      <div className="overflow-y-auto py-1">
        {items.map((p, i) => (
          <button key={p.text} role="option" aria-selected={i === sel} tabIndex={-1} ref={i === sel ? (el) => el?.scrollIntoView({ block: "nearest" }) : undefined} onClick={() => onPick(p)}
            className={`flex w-full items-baseline gap-2.5 px-3.5 py-[5px] text-left ${i === sel ? "bg-selected" : ""}`}>
            <span className="flex-none font-mono text-[12.5px] text-ink">{p.label}</span>
            <span className="min-w-0 flex-1 truncate text-[12px] text-faint">{p.about}</span>
          </button>
        ))}
      </div>
      <div className="flex gap-3 border-t border-line-soft px-3.5 py-1.5 text-[11.5px] text-faint"><span>↑↓ move</span><span>↵ take</span><span>esc close</span></div>
    </div>
  );
}

/** An attachment: its name, a thumbnail when it is an image, and a way to take it off. */
function Chip({ path, onImage, onRemove }: { path: string; onImage?: (path: string) => Promise<string>; onRemove: () => void }) {
  const [src, setSrc] = useState<string>();
  useEffect(() => { if (IMAGE.test(path)) onImage?.(path).then(setSrc).catch(() => {}); }, [path]);
  const name = path.slice(Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\")) + 1);
  return (
    <span title={path} className="inline-flex max-w-[240px] items-center gap-1.5 rounded-lg border border-line bg-field py-1 pl-1 pr-1 text-[12px] text-ink-2">
      {src ? <img src={src} alt="" className="size-7 flex-none rounded-md object-cover" />
        : <span className="grid size-7 flex-none place-items-center rounded-md bg-card text-muted"><FileText size={14} strokeWidth={1.5} /></span>}
      <span className="truncate">{name}</span>
      <button onClick={onRemove} aria-label={`Remove ${name}`} className="flex-none rounded p-0.5 text-faint hover:text-ink"><X size={13} strokeWidth={1.5} /></button>
    </span>
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
        className={pill}>
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
  // a plain chat runs the model alone: nothing there asks for a permission (shift-tab and the palette still reach it)
  if (c.chat) return null;
  return (
    <span className="relative">
      <button disabled={!c.onMode} onClick={() => setOpen(!open)} title="Permission mode (shift-tab)" className={pill}>
        <Shield size={14} strokeWidth={1.5} />{c.session.mode}<ChevronDown size={13} strokeWidth={1.5} className="text-faint" />
      </button>
      {open && (
        <Menu onClose={() => setOpen(false)} width={320}>
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
  useEffect(() => { if (c.picker && !c.busy && c.onPickModel) { c.onModels?.(); setOpen(true); } }, [c.picker]);
  return (
    <span className="relative">
      <button disabled={c.busy || !c.onPickModel} onClick={toggle} title="Model and effort" className={pill}>
        <Zap size={14} strokeWidth={1.5} />{modelLabel(current?.model ?? c.session.model)}
        {c.session.effort && <span className="flex items-center text-faint"><Sep />{c.session.effort}</span>}
        <ChevronDown size={13} strokeWidth={1.5} className="text-faint" />
      </button>
      {open && <ModelPicker models={c.models ?? null} onClose={() => setOpen(false)} onPick={(m) => { setOpen(false); c.onPickModel?.(m); }}
        onKey={c.onAddKey && ((p) => { setOpen(false); c.onAddKey!(p); })}
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
