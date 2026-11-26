import { ArrowDown, CircleAlert, Check, ChevronRight, Copy, FileText, Info, MessageCircle, Pencil, RotateCcw, Square, SquareTerminal } from "lucide-react";
import { createContext, useContext, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Slot } from "../lib/slots";
import { readableError } from "../lib/fromView";
import { attachments, IMAGE_FILE, usd, usdExact, type Change, type Message, type ToolStep } from "../lib/types";
import { AnswerSlip, dur, priceSaid, refless, tok } from "./Receipt";
import { Parts, Sep } from "./Sep";
import { quietButton } from "./Controls";
import { Mark } from "./Mark";
import { Tokens } from "./Stats";
import type { Live } from "../lib/live";

function inlineCode(text: string) {
  return text.split(/(`[^`]+`)/).map((part, i) => part.startsWith("`") ? <code key={i}>{part.slice(1, -1)}</code> : part);
}

// one "thinking" from send to the first output (no "starting", then "waiting", then "thinking")
const doing: Record<string, string> = { Starting: "thinking", Waiting: "thinking", Thinking: "thinking", Writing: "writing", Tool: "working", Verifying: "running the checks" };

type Props = {
  messages: Message[]; busy?: boolean; activity?: string;
  /** the reply running waits for your answer (an approval): its fold says so */
  asking?: boolean;
  /** when the reply running began (ms), for its time */
  since?: number;
  /** the running reply's output tokens so far, said on its Working row when there are any (its finished calls'), and the
   * call in flight, whose estimate counts with them (Live stats) */
  out?: number; live?: Live | null;
  onSendToAgent?: (text: string) => void;
  onDiff?: (turn: number, path: string) => void; onUndo?: (turn: number) => void; onReview?: (turn: number) => void;
  /** "edit from here" on one of your messages */
  onEdit?: (edit: { cp: number; block: number }, text: string) => void;
  /** the checkpoint being edited: that message and everything after it fade */
  editFrom?: number;
  /** follows the rail's "tazuna-jump" (not the side chat's transcript) */
  jumps?: boolean;
  /** runs the last failed turn again, without a second message (core C11) */
  onRetry?: () => void;
  /** a message waiting behind the reply: stop the reply and send this one now */
  onSendNow?: (text: string) => void;
  /** the message whose Retry works: the last reply (set by Transcript) */
  lastReply?: number;
  /** the last chat answer with a receipt, which keeps its line in a plain chat (set by Transcript) */
  lastChatReceipt?: number;
  /** an image file an answer names, as a data URL (it fails for a file that is not there) */
  onImage?: (path: string) => Promise<string>;
  /** Stop, and Stop with the model picker after it: the actions of a call being tried again */
  onStop?: () => void; onChangeModel?: () => void;
};

const STATUS: Record<string, string> = { A: "added", D: "deleted", M: "changed" };
/** A quiet action in a line of text ("Send now", a retry's Stop): words, underlined on hover. */
const textAction = "rounded px-1 text-muted underline-offset-2 hover:text-ink hover:underline";

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
          {f.added == null ? <span className="text-[12px] text-faint">binary</span> : <>
            {f.added > 0 && <span className="text-[12px] font-medium tabular-nums text-ok">+{f.added}</span>}
            {(f.removed ?? 0) > 0 && <span className="text-[12px] font-medium tabular-nums text-bad">−{f.removed}</span>}
          </>}
          {/* undone: the files are as they were, so there is no diff to show */}
          <button disabled={!onDiff || change.undone} title={!onDiff ? "Diff — available in the desktop app" : undefined} onClick={() => onDiff?.(change.turn, f.path)} className={quietButton}>Diff</button>
        </div>
      ))}
      <div className="flex items-center gap-2 px-3 py-[7px] text-[12px] text-muted">
        <span className="flex-1">{change.files.length} file{change.files.length === 1 ? "" : "s"} changed in this turn
          {change.files.length > 6 && !all && <button onClick={() => setAll(true)} className="ml-2 text-accent-ink">Show all</button>}</span>
        {!change.undone && onReview && <button onClick={() => onReview(change.turn)} title="Go through this turn's changes piece by piece"
          className={quietButton}>Review</button>}
        {change.undone ? <span className="text-faint">Undone</span>
          : change.canUndo && <button disabled={busy || !onUndo} onClick={() => onUndo?.(change.turn)} title={!onUndo ? "Undo — available in the desktop app" : "Put these files back as they were before this turn"}
              className={quietButton}>Undo</button>}
      </div>
    </div>
  );
}

/** What the transcript's built-in parts need besides their slot props (callbacks, the busy state, the message being
 * edited): one context for the parts, so a plugin replacing one can use the same through `useTranscript`. */
export const TranscriptCtx = createContext<Omit<Props, "messages" | "activity">>({});
export const useTranscript = () => useContext(TranscriptCtx);

export function Transcript({ messages, activity, jumps = true, ...props }: Props) {
  const { busy, editFrom } = props;
  // Retry runs the last failed turn again: only the last reply's error card offers it
  // and in a plain chat the last answer with a receipt keeps its line: not the last reply, which a failure or a stop at once
  // leaves with none, and hid every line (design review 13, D3)
  const rest = { ...props, lastReply: messages.reduce((n, m, i) => (m.role === "agent" ? i : n), -1),
    lastChatReceipt: messages.reduce((n, m, i) => (m.role === "agent" && m.chat && hasReceipt(m) ? i : n), -1) };
  // a reply with steps says it is working in its fold; the line under the transcript only until the first step
  const folded = !!busy && messages.some((m) => m.role === "agent" && !m.meta && (m.tools?.length ?? 0) > 0);
  // a call being tried again: the live status says so, with the time to the next try (design review 14)
  const last = busy ? [...messages].reverse().find((m) => m.role === "agent") : undefined, retry = last?.role === "agent" && last.retrying ? last : undefined;
  // the live status under its reply, above the messages queued behind it (it sat under them, as if one of them were being
  // written: design review 14)
  let queuedFrom = messages.length;
  while (queuedFrom > 0 && messages[queuedFrom - 1].role === "user" && (messages[queuedFrom - 1] as { queued?: boolean }).queued) queuedFrom--;
  const working = busy && !folded && activity && doing[activity]
    ? <Working what={retry ? "waiting to retry" : doing[activity]} since={rest.since} out={rest.out} live={rest.live} retryAt={retry?.retryAt} above={queuedFrom < messages.length} /> : null;
  // follow new output while the reader is at the bottom; stay put once they scroll up
  const box = useRef<HTMLElement>(null), stick = useRef(true);
  // a long conversation draws its latest messages; earlier ones on request (a three-day session is ~1,900 blocks)
  const [limit, setLimit] = useState(40);
  // scrolled well up from the end: a way back down (new output no longer pulls the page there)
  const [away, setAway] = useState(false);
  const hidden = Math.max(0, messages.length - limit);
  // before the paint: after it, a scroll event of the last follow could see the new content's height first, read the reader
  // as scrolled up, and leave a reply's change card and receipt under the box (design review 12, now and then)
  useLayoutEffect(() => { if (stick.current && box.current) box.current.scrollTop = box.current.scrollHeight; });
  // and when the box changes size with no render of its own: the work panel sliding open beside the column re-wraps it
  // taller, and the reply being followed stopped being followed, its way back down not shown (review of design review 12).
  // The browser's scroll anchoring is off here, so a re-wrap is no scroll of the reader's
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      if (stick.current) el.scrollTop = el.scrollHeight;
      setAway(el.scrollHeight - el.scrollTop - el.clientHeight > 240);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  // the rail's list of your messages (or a plugin) asks for message i with a "tazuna-jump" event: drawn if it was not, then
  // scrolled to
  const [jump, setJump] = useState<number | null>(null);
  useEffect(() => {
    if (!jumps) return;
    const on = (e: Event) => { const i = Number((e as CustomEvent).detail); if (!Number.isInteger(i) || i < 0 || i >= messages.length) return; stick.current = false; setLimit((l) => Math.max(l, messages.length - i)); setJump(i); };
    window.addEventListener("tazuna-jump", on);
    return () => window.removeEventListener("tazuna-jump", on);
  }, [messages.length, jumps]);
  useEffect(() => {
    if (jump == null) return;
    const target = box.current?.querySelector<HTMLElement>(`[data-msg="${jump}"]`);
    target?.scrollIntoView({ block: "start" });
    target?.focus({ preventScroll: true });
    setJump(null);
  }, [jump]);
  const shown = messages.slice(hidden), cut = Math.max(0, queuedFrom - hidden);
  // relative: an older chat answer's receipt shows in the gap under it, taking no room (design review 14)
  const row = (m: Message, i: number) => (
        <div key={hidden + i} data-msg={hidden + i} tabIndex={-1} className={`group relative mx-auto mb-[22px] max-w-[756px] scroll-mt-7 px-7 transition-opacity duration-150 ${editFrom != null && shown.slice(0, i + 1).some((x) => x.role === "user" && x.edit?.cp === editFrom) ? "opacity-35" : ""}`}>
          {m.role === "user" ? <Slot name="message.user" props={{ message: m, index: hidden + i }} only /> : m.role === "notice" ? (
            // a model switch that went on in a new session: a quiet divider, the conversation above it is still the one
            <div className="flex items-center gap-3 text-[12px] text-faint"><span aria-hidden="true" className="h-px flex-1 bg-line" />{m.text}<span aria-hidden="true" className="h-px flex-1 bg-line" /></div>
          ) : (
            <>
              {/* no "tazuna" over each answer (nor "You" over each message): the field block says which is yours */}
              {m.routedChat && (
                <div className="mb-1.5">
                  <span className="inline-flex items-center gap-1.5 rounded-full bg-accent-soft px-2 py-px text-[11.5px] text-muted">
                    <MessageCircle size={13} strokeWidth={1.5} /> answered as a chat<Sep /><button disabled={busy || !rest.onSendToAgent} title={!rest.onSendToAgent ? "Available in the desktop app" : undefined} onClick={() => m.ask && rest.onSendToAgent?.(m.ask)} className="font-medium text-accent-ink disabled:opacity-50">send to agent</button>
                  </span>
                </div>
              )}
              {/* the agent answered in the chat's place, and why */}
              {m.handedOff != null && (
                <div className="mb-1.5">
                  <span className="inline-flex items-center gap-1.5 rounded-full bg-accent-soft px-2 py-px text-[11.5px] text-muted">
                    <SquareTerminal size={13} strokeWidth={1.5} /> answered by the agent{m.handedOff && <><Sep />{m.handedOff}</>}
                  </span>
                </div>
              )}
              {m.tools && m.tools.length > 0 && <WorkFold tools={m.tools} secs={m.meta?.secs} live={!!busy && !m.meta} asking={rest.asking} since={rest.since} out={rest.out} liveCall={rest.live} retry={m.retrying ? m.retryAt ?? 0 : undefined} />}
              <div className="prose-answer text-ink">{m.prose.map((p, j) => <Slot key={j} name="message.text" props={{ text: p }} only />)}</div>
              <Slot name="message.after" props={{ message: m, index: hidden + i }} />
            </>
          )}
        </div>
  );
  return (
    <TranscriptCtx.Provider value={rest}>
    <div className={`tz-doc-edges relative flex min-h-0 flex-1 flex-col ${away ? "away" : ""}`}>
    <section ref={box} onScroll={(e) => { const el = e.currentTarget, gap = el.scrollHeight - el.scrollTop - el.clientHeight; stick.current = gap < 40; setAway(gap > 240); }}
      className="tz-col tz-scroller min-h-0 flex-1 overflow-auto pb-9 pt-7 [overflow-anchor:none]">
      {hidden > 0 && (
        <div className="mx-auto mb-5 max-w-[756px] px-7">
          <button onClick={() => { stick.current = false; setLimit((l) => l + 40); }} className={quietButton}>
            Show {Math.min(40, hidden)} earlier messages <span className="text-faint">({hidden} not shown)</span>
          </button>
        </div>
      )}
      {/* the messages, the live status, then the messages queued behind the reply: the status stays in one place in the tree
          (a queued message remounted it, and its mark started again) */}
      {shown.slice(0, cut).map((m, j) => row(m, j))}
      {working}
      {shown.slice(cut).map((m, j) => row(m, cut + j))}
    </section>
      {/* over the scroller, not in it: the transcript's own fade is under the button (in it, the fade took the button
          too; before the fade it sat on a message's words, "public API s↓ame", design review 9) */}
      {away && (
        <div className="tz-col pointer-events-none absolute inset-x-0 bottom-1 z-[2] flex justify-center">
          <button onClick={() => { stick.current = true; box.current?.scrollTo({ top: box.current.scrollHeight, behavior: "smooth" }); }} title="Jump to the latest" aria-label="Jump to the latest"
            className="tz-enter pointer-events-auto grid size-8 place-items-center rounded-full border border-line-strong bg-card text-ink-2 shadow-[var(--shadow-pop)]"><ArrowDown size={15} strokeWidth={1.75} /></button>
        </div>
      )}
    </div>
    </TranscriptCtx.Provider>
  );
}

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** What a reply's steps did, in a few words: "read 2 files, edited 1, ran 3 commands". */
export function workSummary(tools: ToolStep[]): string {
  const n = new Map<string, number>();
  // a step you declined did not happen: said apart ("declined 1"), not "ran 1 command" (real window, 2026-11-22)
  for (const t of tools) if (t.kind !== "thinking" && t.kind !== "said" && !t.declined && !t.waiting) n.set(t.verb, (n.get(t.verb) ?? 0) + 1);
  const declined = tools.filter((t) => t.declined).length;
  const edits = (n.get("Edited") ?? 0) + (n.get("Wrote") ?? 0);
  n.delete("Edited"); n.delete("Wrote");
  const parts: string[] = [];
  if (n.has("Read")) parts.push(`read ${plural(n.get("Read")!, "file")}`);
  if (edits) parts.push(`edited ${plural(edits, "file")}`);
  if (n.has("Ran")) parts.push(`ran ${plural(n.get("Ran")!, "command")}`);
  const searched = (n.get("Searched") ?? 0) + (n.get("Listed") ?? 0);
  if (searched) parts.push(searched === 1 ? "searched" : `searched ${searched} times`);
  for (const k of ["Read", "Ran", "Searched", "Listed"]) n.delete(k);
  // tools without a verb of their own (an MCP server's "toolsearch", a skill's "guide") as a count, not their raw names
  // (design review 5: "ran 15 commands, toolsearch ×3, guide")
  const known = ["Asked a subagent", "Fetched", "Used skill", "Searched the web"];
  for (const [verb, count] of n) if (known.includes(verb)) parts.push(`${verb.toLowerCase()}${count > 1 ? ` ×${count}` : ""}`);
  const other = [...n].filter(([v]) => !known.includes(v)).reduce((a, [, c]) => a + c, 0);
  if (other) parts.push(`used ${plural(other, "other tool")}`);
  if (declined) parts.push(`declined ${declined}`);
  // not while a step waits for you: "Waiting for you | thought it through" (design review 12)
  if (!parts.length && tools.some((t) => t.kind === "thinking") && !tools.some((t) => t.waiting)) parts.push("thought it through");
  return parts.slice(0, 4).join(", ");
}

/** A part that unfolds and folds in height (180 ms open, 150 ms shut; at once under reduced motion): its body's ref and
 * the click. The steps had it first (design review 10: they appeared and went at once); the receipt opens the same way. */
const still = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;
function useFold(shown: boolean, set: (open: boolean) => void) {
  const body = useRef<HTMLDivElement>(null), clicked = useRef(false);
  useLayoutEffect(() => {
    const el = body.current;
    if (!el || !clicked.current || still()) return;
    clicked.current = false;
    // clipped only while it moves: a step's focus ring reaches past the column
    el.style.overflow = "hidden";
    el.animate([{ height: "0px", opacity: 0 }, { height: `${el.scrollHeight}px`, opacity: 1 }], { duration: 180, easing: "cubic-bezier(0.2, 0, 0, 1)" }).onfinish = () => { el.style.overflow = ""; };
  }, [shown]);
  const toggle = () => {
    const el = body.current;
    if (shown && el && !still()) { el.style.overflow = "hidden"; el.animate([{ height: `${el.scrollHeight}px` }, { height: "0px", opacity: 0 }], { duration: 150, easing: "cubic-bezier(0.2, 0, 0, 1)" }).onfinish = () => set(false); return; }
    clicked.current = !shown;
    set(!shown);
  };
  return [body, toggle] as const;
}

/** A reply's steps as one line, "Worked 41 s · read 2 files, ran 3 commands ›": open while the reply runs, folded once it
 * is done (a click keeps it as chosen). Until desktop 0.1.64 each step was a line of its own above every answer. */
function WorkFold({ tools, secs, live, asking, since, out, liveCall, retry }: { tools: ToolStep[]; secs?: number; live: boolean; asking?: boolean; since?: number; out?: number; liveCall?: Live | null;
  /** a call being tried again: when the next try is (epoch ms; 0 where the core does not say) */
  retry?: number }) {
  const [open, setOpen] = useState<boolean | null>(null);
  const shown = open ?? live;
  const what = workSummary(tools);
  // a failed step said folded too, in the result colour, as the terminal has it (design review 13: only the open list showed
  // the ✗ of a `cat nope.txt`)
  const failed = tools.filter((t) => t.kind !== "thinking" && t.kind !== "said" && !t.ok && !t.running && !t.stopped && !t.declined).length;
  const took = secs == null ? "" : ` ${dur(secs)}`;
  const [body, toggle] = useFold(shown, setOpen);
  return (
    <div className="mb-2.5">
      {/* no gap between the parts: each hairline has the same room on both sides (design review 14: 12 px before it, 6 after) */}
      <button onClick={toggle} aria-expanded={shown} title={shown ? "Fold the steps" : "Show each step"}
        className="-ml-1.5 inline-flex max-w-full items-center rounded-md px-1.5 py-0.5 text-left text-[12.5px] text-muted hover:bg-hover hover:text-ink">
        <ChevronRight size={13} strokeWidth={1.5} className={`mr-1.5 flex-none transition-transform duration-150 ${shown ? "rotate-90" : ""}`} />
        {/* the mark at work where the spinning ring was, "Working" under a slow shimmer, and its tokens so far (design review
            13, D1 of mockups/v10.html); with reduced motion the mark is drawn whole and the words plain */}
        {live && !asking && <span className="mr-1.5 flex flex-none"><Mark size={MARK_AT_WORK} motion="write" /></span>}
        <span className={`flex-none ${live && asking ? "font-medium text-warn" : ""}`}>{live ? (asking ? "Waiting for you"
          : retry != null ? <><span className="tz-shimmer font-medium">Waiting to retry</span>{retry > 0 && <> <span className="tabular-nums"><Countdown at={retry} /></span></>}</>
          : <><span className="tz-shimmer font-medium">Working</span>{since && <> <Ticking since={since} /></>}</>) : `Worked${took}`}</span>
        {live && !asking && <span className="flex flex-none items-center text-faint"><Tokens out={out} live={liveCall} /></span>}
        {what && <span className="flex min-w-0 items-center truncate text-faint"><Sep />{what}</span>}
        {failed > 0 && <span className="flex flex-none items-center text-bad"><Sep />{failed} failed</span>}
      </button>
      {shown && (
        <div ref={body} className="ml-[5px] mt-1 flex flex-col gap-0.5 border-l border-line pl-3.5">
          {/* a short thought with nothing to read is no step of its own (rows of "Thought ~14 tokens" between the commands) */}
          {tools.filter((t) => !(t.kind === "thinking" && !t.running && !t.text?.trim() && !/k tokens/.test(t.detail ?? ""))).map((t, j) => <Slot key={j} name="tool.view" props={{ step: t }} only />)}
        </div>
      )}
    </div>
  );
}

/** The working mark's tile on a 12.5 px line (18.75 px high): 16 px, a CSS pixel for each cell of its 16-cell grid (at 10 px
 * it read as a grey smudge, design review 14; a 20 px tile would not fit the line). */
const MARK_AT_WORK = 16;

/** Under a reply that is still going: the mark at work, what it is doing with dots that count, how long the reply has run
 * (it stays mounted from the reply's start to its end) and its tokens so far. */
function Working({ what, since, out, live, retryAt, above }: { what: string; since?: number; out?: number; live?: Live | null;
  /** a call being tried again: the next try (epoch ms), counted down in place of the time */
  retryAt?: number;
  /** messages queued behind the reply follow it */
  above?: boolean }) {
  // the reply's own start where App knows it (it leaves out the time a question waited for you); else when this mounted
  const [mounted] = useState(Date.now), start = since ?? mounted;
  return (
    <div role="status" className={`mx-auto flex max-w-[756px] items-center gap-2 px-7 text-[12.5px] text-muted ${above ? "mb-[22px]" : ""}`}>
      <Mark size={MARK_AT_WORK} motion="write" />
      <span className="tz-shimmer font-medium">{what}</span><span className="tz-dots -ml-2" aria-hidden="true"><span>.</span><span>.</span><span>.</span></span>
      <span className="tabular-nums text-faint">{retryAt ? <Countdown at={retryAt} /> : <Ticking since={start} />}<Tokens out={out} live={live} /></span>
    </div>
  );
}

/** "next try in 12 s" until `at` (epoch ms), counting; "trying again" once it is past. */
function Countdown({ at }: { at: number }) {
  const [, tick] = useState(0);
  useEffect(() => { const t = setInterval(() => tick((n) => n + 1), 1000); return () => clearInterval(t); }, []);
  const left = Math.ceil((at - Date.now()) / 1000);
  return <>{left > 0 ? `next try in ${dur(left)}` : "trying again"}</>;
}

/** The seconds since `since`, counting. */
function Ticking({ since }: { since: number }) {
  const [, tick] = useState(0);
  useEffect(() => { const t = setInterval(() => tick((n) => n + 1), 1000); return () => clearInterval(t); }, []);
  return <span className="tabular-nums">{dur(Math.max(0, Math.floor((Date.now() - since) / 1000)))}</span>;
}

// tazuna's own parts of a message, registered in their spots (src/builtins.tsx)

export function UserMessage({ message: m }: { message: Message }) {
  const { busy, onEdit, onSendNow } = useTranscript();
  const [all, setAll] = useState(false);
  if (m.role !== "user") return null;
  // the note tazuna adds after an undo ("[tazuna: ...]") is shown as what it is, apart from the message
  const note = m.text.match(/^\[tazuna: ([^\]]*)\]\n\n/);
  const text = note ? m.text.slice(note[0].length) : m.text;
  // the files it carries show as tiles under it, not as their paths in its text (the user, 2026-11-24)
  const { text: said, files } = attachments(text);
  const long = said.length > 600 || said.split("\n").length > 6;
  return (
    <>
      {note && <div className="mb-1 text-[11.5px] text-faint">told the model: {note[1]}</div>}
      <div className="flex items-start gap-1">
        {/* a long message (a pasted log, eight lines of links) folds to six lines; all of it on a click (design review 5) */}
        <div className="min-w-0">
          {said && <div className="inline-block max-w-full select-text rounded-lg bg-field px-3.5 py-2.5 text-ink">
            {/* as typed or pasted: its line breaks and tabs kept (a pasted table or list ran together into one paragraph) */}
            <div className={`whitespace-pre-wrap break-words ${!all && long ? "line-clamp-6" : ""}`}>{inlineCode(said)}</div>
            {long && <button onClick={() => setAll(!all)} className="mt-1 text-[12px] text-muted hover:text-ink">{all ? "Show less" : "Show all"}</button>}
          </div>}
          {files.length > 0 && <Attachments files={files} />}
        </div>
        {m.edit && onEdit && !busy && (
          <button onClick={() => onEdit(m.edit!, text)} title="Edit and send again from here: the conversation goes back to before this message" aria-label="Edit from here"
            className="mt-1.5 flex-none rounded p-1 text-faint opacity-0 hover:bg-hover hover:text-ink focus-visible:opacity-100 group-hover:opacity-100"><Pencil size={13} strokeWidth={1.5} /></button>
        )}
      </div>
      {m.queued && <div className="mt-1 flex items-center gap-2 text-[11.5px] text-faint">Queued: goes after this reply
        {m.held && onSendNow && <button onClick={() => onSendNow(m.text)} title="Stop the reply and send this message now (a command still running stops too)"
          className={textAction}>Send now</button>}</div>}
    </>
  );
}

/** A message's attachments: an image as a thumbnail that opens whole over the window (a click or Esc closes it), any
 * other file, or an image no longer there, as its name. */
function Attachments({ files }: { files: string[] }) {
  const { onImage } = useTranscript();
  const [src, setSrc] = useState<Record<string, string>>({}), [big, setBig] = useState<string | null>(null);
  useEffect(() => { for (const f of files) if (onImage && IMAGE_FILE.test(f) && !src[f]) onImage(f).then((s) => setSrc((m) => ({ ...m, [f]: s }))).catch(() => {}); }, [files.join("|")]);
  useEffect(() => {
    if (!big) return;
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopPropagation(); setBig(null); } };
    window.addEventListener("keydown", key, true);
    return () => window.removeEventListener("keydown", key, true);
  }, [big]);
  const name = (f: string) => f.slice(Math.max(f.lastIndexOf("/"), f.lastIndexOf("\\")) + 1);
  return (
    <>
      <div className="mt-1.5 flex flex-wrap gap-2">
        {files.map((f) => src[f] ? (
          <button key={f} onClick={() => setBig(f)} title={f} aria-label={`${name(f)}: show it whole`}
            className="overflow-hidden rounded-lg border border-line bg-card p-0 hover:border-line-strong">
            <img src={src[f]} alt={name(f)} className="block h-[72px] max-w-[160px] object-cover" />
          </button>
        ) : (
          <span key={f} title={f} className="inline-flex max-w-[240px] items-center gap-1.5 rounded-lg border border-line bg-field py-1 pl-1 pr-2 text-[12px] text-ink-2">
            <span className="grid size-7 flex-none place-items-center rounded-md bg-card text-muted"><FileText size={14} strokeWidth={1.5} /></span>
            <span className="truncate">{name(f)}</span>
          </span>
        ))}
      </div>
      {/* over the whole window, not inside the scrolling transcript (which clipped it) */}
      {big && src[big] && createPortal(
        <div role="dialog" aria-modal="true" aria-label={name(big)} onClick={() => setBig(null)} className="tz-fade fixed inset-0 z-50 flex flex-col items-center justify-center gap-2 bg-scrim p-10">
          <img src={src[big]} alt={name(big)} className="max-h-[85vh] max-w-full rounded-lg shadow-[var(--shadow-pop)]" />
          <span className="max-w-full truncate text-[12px] text-ink-2">{big}</span>
        </div>, document.body)}
    </>
  );
}

/** Why a reply ended without its answer: the provider's error in words, its reply as it came on request, and Retry; a
 * reply you stopped says only that. Until desktop 0.1.74 the error was its raw JSON in the answer's font, after a "⚠". */
export function MessageFailure({ message: m, index }: { message: Message; index?: number }) {
  const { busy, onRetry, lastReply, onStop, onChangeModel } = useTranscript();
  const [raw, setRaw] = useState(false);
  if (m.role !== "agent") return null;
  const notes = (m.notes ?? []).filter((n) => n !== m.failure?.raw);
  const f = m.failure, r = m.retrying?.match(/^([\s\S]*?): (\d+ (?:try|tries) failed)(, trying again)?/);
  return <>
    {/* a call being tried again, while it is: "The provider is rate limiting this model | 2 tries failed | next try in 12 s",
        then Stop and Change model (design review 14); without the core's time, "trying again" */}
    {/* (no spinner of its own: the reply's one live status is under it) */}
    {m.retrying && <div role="status" className="mt-1 flex items-center gap-2 text-[12.5px] text-muted"><RotateCcw size={12} strokeWidth={1.75} className="flex-none text-faint" aria-hidden="true" />
      <Parts className="min-w-0" items={r ? [readableError(r[1]), r[2], m.retryAt ? <Countdown at={m.retryAt} /> : r[3] && "trying again"] : [readableError(m.retrying)]} />
      {busy && index === lastReply && onStop && <span className="ml-1 flex flex-none items-center gap-1">
        <button onClick={onStop} title="Stop the reply (Esc)" className={textAction}>Stop</button>
        {onChangeModel && <button onClick={onChangeModel} title="Stop the reply and choose another model" className={textAction}>Change model</button>}</span>}</div>}
    {m.info?.map((n, i) => <div key={`i${i}`} className="mt-1 flex items-start gap-2 text-[12.5px] text-muted"><Info size={14} strokeWidth={1.5} className="mt-0.5 flex-none" />{n}</div>)}
    {notes.map((n, i) => <div key={i} className="mb-2 flex items-start gap-2 text-[12.5px] text-bad"><CircleAlert size={14} strokeWidth={1.5} className="mt-0.5 flex-none" />{readableError(n)}</div>)}
    {f && !f.text && <div className="mt-1 flex items-center gap-1.5 text-[12.5px] text-muted"><Square size={11} strokeWidth={2} className="fill-current text-warn" />Stopped</div>}
    {f && f.text && (
      <div role="alert" className="mt-1 rounded-lg border border-bad/35 bg-bad/[0.06] px-3.5 py-2.5 text-[13px]">
        <div className="flex items-start gap-2.5">
          <CircleAlert size={16} strokeWidth={1.5} className="mt-px flex-none text-bad" />
          <div className="min-w-0 flex-1">
            <div className="font-medium text-ink">{f.stopped ? (f.retrying ? "Stopped while retrying" : "Stopped after an error from the provider") : "The provider could not answer"}</div>
            <div className="mt-0.5 text-ink-2">{inlineCode(f.text)}</div>
            {f.raw && f.raw !== f.text && <button onClick={() => setRaw(!raw)} aria-expanded={raw} className="mt-1 text-[12px] text-muted hover:text-ink">{raw ? "Hide" : "Show"} its reply <ChevronRight size={12} strokeWidth={1.5} aria-hidden="true" className={`inline-block align-[-1px] transition-transform duration-150 ${raw ? "rotate-90" : ""}`} /></button>}
            {raw && <pre className="mt-1.5 max-h-48 overflow-auto whitespace-pre-wrap break-all rounded-md bg-field px-2.5 py-2 font-mono text-[11.5px] text-muted">{f.raw}</pre>}
          </div>
          {onRetry && index === lastReply && <button disabled={busy} onClick={onRetry} title="Run this turn again: the error makes way for the new reply"
            className={`flex flex-none items-center gap-1.5 ${quietButton}`}><RotateCcw size={12} strokeWidth={1.75} />Retry</button>}
        </div>
      </div>
    )}
  </>;
}

export function MessageChange({ message: m }: { message: Message }) {
  const { busy, onDiff, onUndo, onReview } = useTranscript();
  return m.role === "agent" && m.change ? <ChangeCard change={m.change} busy={busy} onDiff={onDiff} onUndo={onUndo} onReview={onReview} /> : null;
}

/** Copies an answer's text as markdown (DeepSeek Harness puts copy under each answer; tazuna had only selecting it). */
function CopyAnswer({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  const copy = async () => {
    try { await navigator.clipboard.writeText(text); }
    catch {
      // a webview without the clipboard API: the old way, through a selected field
      const t = document.createElement("textarea");
      t.value = text; document.body.appendChild(t); t.select(); document.execCommand("copy"); t.remove();
    }
    setDone(true); setTimeout(() => setDone(false), 1500);
  };
  return (
    <button onClick={copy} title="Copy the answer (markdown)" className="inline-flex items-center gap-1 rounded px-1 hover:bg-hover hover:text-ink">
      {done ? <Check size={12} strokeWidth={1.75} className="text-ok" /> : <Copy size={12} strokeWidth={1.5} />}{done ? "Copied" : "Copy"}
    </button>
  );
}

/** The image files an answer names (`out/chart.png` in code marks, or a markdown image), shown under it: up to four, each
 * only if it is there to read. Until desktop 0.1.87 a picture the agent made was a line of path. A click shows it whole. */
export function MessageImages({ message: m }: { message: Message }) {
  const { onImage } = useTranscript();
  const paths = m.role === "agent" ? [...new Set([...m.prose.join("\n").matchAll(/`([^`\s]+\.(?:png|jpe?g|gif|webp|svg))`|!\[[^\]\n]*\]\(([^)\s]+\.(?:png|jpe?g|gif|webp|svg))\)/gi)]
    .map((x) => x[1] ?? x[2]).filter((p) => !/^(https?|data):/.test(p)))].slice(0, 4) : [];
  const [got, setGot] = useState<Record<string, string>>({}), [big, setBig] = useState<string | null>(null);
  useEffect(() => { for (const p of paths) if (onImage && !got[p]) onImage(p).then((src) => setGot((g) => ({ ...g, [p]: src }))).catch(() => {}); }, [paths.join("|")]);
  const shown = paths.filter((p) => got[p]);
  if (!shown.length) return null;
  return (
    <div className="mt-1 flex flex-wrap gap-2">
      {shown.map((p) => (
        <button key={p} onClick={() => setBig(big === p ? null : p)} title={big === p ? "Smaller" : "Show it whole"} aria-label={`${p}: ${big === p ? "smaller" : "show it whole"}`}
          className={`overflow-hidden rounded-lg border border-line bg-card p-0 text-left hover:border-line-strong ${big === p ? "w-full" : ""}`}>
          <img src={got[p]} alt={p} className={big === p ? "block w-full" : "block max-h-[180px] max-w-[320px] object-contain"} />
          <span className="block truncate border-t border-line-soft px-2 py-1 font-mono text-[11.5px] text-faint">{p}</span>
        </button>
      ))}
    </div>
  );
}

/** A turn that gave no answer and cost nothing has no receipt ("$0 | 0 s" under a failed attempt). */
const hasReceipt = (m: Message) => m.role === "agent" && !!m.meta && (!!m.prose.join("\n\n") || !!m.meta.in || !!m.meta.out || (m.meta.cost ?? 0) > 0);

export function MessageMeta({ message: m, index }: { message: Message; index?: number }) {
  const { busy, onSendToAgent, lastChatReceipt } = useTranscript();
  const [open, setOpen] = useState(false);
  const [body, toggle] = useFold(open, setOpen);
  // the slip opens into view (under the last answer it opened below the message box's edge)
  const line = useRef<HTMLDivElement>(null);
  // (once it has unfolded: asked for while its height is still 0, only its top came into view)
  useEffect(() => { if (!open) return; const t = setTimeout(() => body.current?.scrollIntoView({ block: "nearest", behavior: still() ? "auto" : "smooth" }), still() ? 0 : 200); return () => clearTimeout(t); }, [open]);
  if (m.role !== "agent" || !m.meta || !hasReceipt(m)) return null;
  const text = m.prose.join("\n\n"), a = m.meta;
  // in a plain chat the last answer keeps its line and older ones show theirs on hover or focus, as their actions do
  // (design review 13, D3 of mockups/v10.html: every chat answer's numbers read as noise); an agent's run keeps it, its
  // cost is the work's
  const quiet = m.chat && index !== lastChatReceipt && !open;
  return (
    // the answer's receipt: one faint line, always there (design review 11, variant A of mockups/v8.html; until then its
    // numbers showed on the answer's hover only), a click unfolds every model call and what it cost. Copy stays on hover
    // and on the keyboard's focus.
    // a hidden one takes no room: it lies in the gap under the answer and shows there, nothing moving (it kept its row, and
    // the space over the next message was 54 px, not 22: design review 14)
    <div ref={line} className={`scroll-mb-3 text-[12px] tabular-nums text-faint ${quiet ? "absolute inset-x-7 top-full opacity-0 transition-opacity duration-150 group-focus-within:opacity-100 group-hover:opacity-100" : "mt-2"}`}>
      <div className="flex items-center">
        <button onClick={toggle} aria-expanded={open} title={`The receipt: every model call of this reply and what it cost${a.cost != null && usd(a.cost) !== usdExact(a.cost) ? ` (${usdExact(a.cost)})` : ""}`}
          className="-ml-1.5 inline-flex min-w-0 items-center rounded-md px-1.5 py-0.5 hover:bg-hover hover:text-ink-2">
          <ChevronRight size={12} strokeWidth={1.5} aria-hidden="true" className={`mr-1.5 flex-none transition-transform duration-150 ${open ? "rotate-90" : ""}`} />
          {/* a plan without a reference price: no "$0" (the user, 2026-11-25) */}
          <span className="font-medium text-muted">{refless(a.billing) ? "not billed" : priceSaid(a.cost, a.estimate)}</span>
          {a.in ? <><Sep />{tok(a.in)} in{a.cachedPct != null && <span className="ml-1">({a.cachedPct}% cached)</span>}<Sep />{tok(a.out ?? 0)} out</> : null}
          <Sep />{dur(a.secs)}
        </button>
        <span className="flex-1" />
        {text && <span className="inline-flex gap-1 opacity-0 transition-opacity duration-150 focus-within:opacity-100 group-hover:opacity-100">
          {/* a plain chat's answer: the agent can take its question (it has a shell and the web) */}
          {m.chat && m.ask && onSendToAgent && <button disabled={busy} onClick={() => onSendToAgent(m.ask!)} title="Ask the agent instead: it can run commands and read the web"
            className="inline-flex items-center gap-1 rounded px-1 hover:bg-hover hover:text-ink disabled:opacity-50"><SquareTerminal size={12} strokeWidth={1.5} />Send to agent</button>}
          <CopyAnswer text={text} /></span>}
      </div>
      {open && <div ref={body} className="scroll-mb-10 pb-1 pt-2"><AnswerSlip meta={a} tools={m.tools} toolsSaid={workSummary(m.tools ?? [])} /></div>}
    </div>
  );
}
