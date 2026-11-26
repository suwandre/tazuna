import { ShieldQuestion } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { View } from "../lib/tazuna";
import { Sep } from "./Sep";
import { Given, PathLabel, writes } from "./Step";

// how long before a key the one before it went down, window-wide and first (capture), so keys typed before the card came
// count too; the window coming back counts as a key (a key pressed as it comes back was meant for where it was)
let lastKey = 0, gap = Infinity;
addEventListener("keydown", (e) => { gap = e.timeStamp - lastKey; lastKey = e.timeStamp; }, true);
const back = () => { lastKey = performance.now(); };
addEventListener("focus", back);
document.addEventListener("visibilitychange", back);

/** Whether a key answers the question: only after a pause of 800 ms, never one typed into a text field (the box too, empty:
 * "explain why" answered allow+always, deny and allow, and a first "y" of "you…" would have; design review 13), nor one
 * something else took: a menu, a dialog, the palette, an input method composing. Clicks, or Enter on a button, still do;
 * a key on the card's own buttons (reached with Shift+Tab: meant for it) needs no pause. */
function answers(e: KeyboardEvent, onCard: boolean): boolean {
  const t = e.target as HTMLElement | null;
  if (e.defaultPrevented || e.isComposing || e.keyCode === 229 || e.metaKey || e.ctrlKey || e.altKey) return false;
  if (t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t?.isContentEditable) return false;
  // typing in the side chat answers nothing; nor a key in a dialog, a menu or a popover (a native one, the narrow window's
  // sidebar, too: Escape there shuts it)
  if (t?.closest?.("[role=dialog], [role=alertdialog], dialog[open], [role=menu], .tz-pop, [data-side]")) return false;
  return onCard || gap >= 800;
}

// tazuna's permission question over the composer: allow once, always (this command or tool, for the session), or
// decline. y / a / n answer from the keyboard (see `answers`); nothing answers in the first 600 ms of each question, so a
// key meant for something else cannot approve a question that just appeared (the terminal's ANSWER_DELAY_MS).
export function Approval({ asking, onAnswer, keys = true, input, cwd }: { asking: NonNullable<View["asking"]>; onAnswer: (allow: boolean, always: boolean) => void;
  /** y / a / n answer it (the side chat's question takes clicks only, so keys stay the conversation's) */ keys?: boolean;
  /** what the asked call was given (its step's input): an edit or a write shows what would change */
  input?: Record<string, unknown> | null;
  /** the conversation's folder: the file's path is shown from it */ cwd?: string }) {
  // auto asks before a destructive command alone (the core's `risky` and `risky_why`; an older core's note says so):
  // why, under the title, as the terminal says it; Allow then no filled button, as plain as Decline, so a reflex click
  // does not take it, and never "always" (design review 13). Risky with no reason said: its note, as before
  const why = asking.risky_why ?? asking.note.match(/auto asks: (.+?)(?: · always = .*)?$/)?.[1];
  const risky = asking.risky ?? !!why;
  const always = !asking.once && !risky;
  const card = useRef<HTMLDivElement>(null);
  // each question its own: the same command asked again after an answer waits its 600 ms again. Ready only once that
  // question's own timer ran, set before paint, so no frame shows the buttons enabled
  const [answered, setAnswered] = useState(0);
  const key = `${asking.tool}\n${asking.what}\n${answered}`;
  const [readyFor, setReadyFor] = useState<string | null>(null);
  useLayoutEffect(() => { const t = setTimeout(() => setReadyFor(key), 600); return () => clearTimeout(t); }, [key]);
  const ready = readyFor === key;
  // answered from the card's buttons (or with nothing focused): the focus goes to the box, not to the page, where the
  // next words typed answered the next question ("yes but use npm ci" allowed it). The box's own textarea: a hidden
  // mirror comes first
  const answer = (allow: boolean, all: boolean) => {
    const at = document.activeElement;
    setAnswered((n) => n + 1);
    onAnswer(allow, all);
    if (keys && (!at || at === document.body || card.current?.contains(at))) document.querySelector<HTMLTextAreaElement>("[data-composer] textarea:not([aria-hidden])")?.focus();
  };
  useEffect(() => {
    if (!ready || !keys) return;
    const press = (e: KeyboardEvent) => {
      if (!answers(e, !!card.current?.contains(e.target as Node))) return;
      const k = e.key.toLowerCase();
      const pick = k === "y" ? [true, false] : k === "a" && always ? [true, true] : k === "n" || k === "escape" ? [false, false] : null;
      if (pick) { e.preventDefault(); answer(pick[0], pick[1]); }
    };
    window.addEventListener("keydown", press);
    return () => window.removeEventListener("keydown", press);
  }, [ready, asking, onAnswer, keys]);
  // the core sends the tool, what it would do and the note apart (each made safe): shown whole, nothing cut (an edit's path
  // from the project's folder, wrapped where it is long: PathLabel's `wrap`)
  const tool = asking.tool.replace(/^mcp__lean__/, "");
  const what = asking.what;
  const btn = "rounded-md px-2.5 py-1 text-[12px] font-medium disabled:opacity-50";
  return (
    <div ref={card} data-asking={keys || undefined} className="mx-auto mb-2 w-full max-w-[756px] px-7">
      {/* the one thing waiting on you: the warn colour on its edge, its words in ink (it read as one more card) */}
      <div className="rounded-lg border border-[color-mix(in_srgb,var(--warn)_55%,var(--line-strong))] bg-card px-3.5 py-3 shadow-[var(--shadow-composer)]">
        <div className="mb-1.5 flex items-center gap-2 text-[12.5px] font-medium text-ink">
          <ShieldQuestion size={15} strokeWidth={1.5} className="text-warn" />
          {/^bash$/i.test(tool) ? "Allow a shell command?" : writes(asking.tool) ? "Allow this edit?" : `Allow ${tool}?`}
          {asking.queued > 0 && <span className="flex items-center text-faint"><Sep />{asking.queued} more waiting</span>}
          {asking.reviewing && <span role="status" className="flex items-center text-muted"><Sep />the reviewer is checking it; you can answer first</span>}
        </div>
        {why && <div className="-mt-0.5 mb-1.5 pl-[23px] text-[12px] text-warn">{why}</div>}
        {/* an edit or a write: the file, then what would change in it, as the step shows it (it showed the file's name
            alone, design review 12); its own scroll, so the buttons stay in sight */}
        {input && writes(asking.tool) ? <div className="mb-2.5"><PathLabel path={what} cwd={cwd} wrap className="text-[12px] select-text" />
          <Given name={asking.tool} input={input} cap="max-h-[min(14rem,28vh)]" cwd={cwd} /></div>
          : <pre className="mb-2.5 max-h-40 overflow-auto whitespace-pre-wrap break-all rounded-md bg-field px-2.5 py-2 font-mono text-[12px] text-ink select-text">{what}</pre>}
        <div className="flex items-center gap-1.5">
          <button disabled={!ready} onClick={() => answer(true, false)} className={`${btn} border ${risky ? "border-line text-ink" : "border-transparent bg-accent text-on-accent"}`}>Allow <kbd className={`ml-1 font-mono ${risky ? "text-faint" : "opacity-70"}`}>y</kbd></button>
          {always && (
            <button disabled={!ready} onClick={() => answer(true, true)} className={`${btn} border border-line text-ink`}
              title={`${asking.note}, for the rest of the session`}>
              Always <kbd className="ml-1 font-mono text-faint">a</kbd>
            </button>
          )}
          <button disabled={!ready} onClick={() => answer(false, false)} className={`${btn} border border-line ${risky ? "text-ink" : "text-muted"}`}>Decline <kbd className="ml-1 font-mono text-faint">n</kbd></button>
          {(asking.once || risky) && !why && <span className="ml-1 text-[11.5px] text-faint">{asking.note}</span>}
        </div>
      </div>
    </div>
  );
}
