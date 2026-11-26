import { X } from "lucide-react";
import { useEffect, type HTMLAttributes, type ReactNode } from "react";
import type { Diff } from "../lib/tazuna";
import { code } from "../lib/markdown";
import { useDialog } from "../lib/dialog";
import { dialogShell } from "./Controls";

// One file's diff for a turn, over the transcript; Esc or a click outside closes it.
/** `plain`: any text, shown as it is (a plugin's code), not as a diff. */
export function DiffView({ diff, onClose, plain }: { diff: Diff; onClose: () => void; plain?: boolean }) {
  const dialog = useDialog();
  useEffect(() => {
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onClose]);
  const lines = diff.text.split("\n");
  const body = plain ? lines : lines.slice(Math.max(0, lines.findIndex((l) => l.startsWith("@@"))));
  const tone = (l: string) => plain ? "text-ink-2" : l.startsWith("+") ? "bg-ok/10 text-ok" : l.startsWith("-") ? "bg-bad/10 text-bad" : l.startsWith("@@") ? "text-faint" : "text-ink-2";
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-20 grid place-items-center bg-scrim p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Changes" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className={`${dialogShell(860)} w-full! max-w-[min(860px,calc(100vw-5rem))]!`}>
        <div className="flex items-center gap-2 border-b border-line-soft px-4 py-2.5">
          <span className="flex-1 truncate font-mono text-[12.5px] text-ink">{diff.path}</span>
          <button onClick={onClose} title="Close (Esc)" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <pre className="overflow-auto py-2 font-mono text-[12px] leading-[1.55] select-text">
          {diff.text.trim() === "" ? <div className="px-4 text-faint">No text diff (a binary file, or nothing left to compare).</div>
            : body.map((l, i) => <div key={i} className={`px-4 ${tone(l)}`}>{l || " "}</div>)}
        </pre>
      </div>
    </div>
  );
}

/** Lines longer than this are drawn plain: no colours, no word marks (a 200 KB line of minified JS was thousands of spans and
 * froze an approval for 12 s). */
export const LONG = 2000;
/** At most this many rows of a step's code are drawn; the rest is said in one line ("… 400 more lines"). */
export const MAX_ROWS = 1000;

/** One row of code, the review's and a step's alike: its line numbers in a gutter (`nums`, `width` digits wide), a diff's
 * +/− in a narrow column of its own (`kind`; none for a file's text), the code coloured by its language with the changed
 * words marked (`mark`: in ink, no syntax colours, on the stronger tint), and a wrapped line hanging under its own indent in
 * the code's column, never under the gutter. A + or − row is tinted whole (--diff-add, --diff-del). The gutter is drawn by
 * CSS (`tz-gut`): a copy takes the code alone. */
export function CodeRow({ text, kind, nums, width = 3, mark, language, plain, className = "", children, ...rest }: {
  text: string; kind?: string; nums?: (number | null)[]; width?: number; mark?: [number, number]; language?: string | null;
  /** a note among the lines (a search's "--", a read's "[lines …]"): no colours */ plain?: boolean; children?: ReactNode;
} & HTMLAttributes<HTMLDivElement>) {
  const add = kind === "+", del = kind === "-", long = text.length > LONG, lang = long ? "text" : language;
  // an empty line is a <br>: its height, and one blank line in a copy (a space was copied; an empty cell was dropped)
  const body: ReactNode = !text ? <br /> : plain ? text : mark && mark[1] > mark[0] && !long
    ? <>{code(text.slice(0, mark[0]), 0, false, lang)}<mark className={`rounded-sm text-ink ${add ? "bg-diff-add-word" : "bg-diff-del-word"}`}>{text.slice(mark[0], mark[1])}</mark>{code(text.slice(mark[1]), 2, false, lang)}</>
    : code(text, 0, false, lang);
  // a wrapped line hangs under its own indent, two columns in (design review 10: it went back to the code's first column)
  const lead = (text.match(/^[ \t]*/)?.[0] ?? "").replace(/\t/g, "    ").length + 2;
  return (
    <div {...rest} className={`flex min-w-0 ${add ? "bg-diff-add" : del ? "bg-diff-del" : ""} ${className}`}>
      {nums?.map((n, i) => <span key={i} data-n={n ?? ""} className="tz-gut flex-none pr-2 text-right tabular-nums text-faint" style={{ width: `calc(${width}ch + 0.5rem)` }} />)}
      {kind != null && <span data-n={kind === " " ? "" : del ? "−" : kind} className={`tz-gut w-4 flex-none ${add ? "text-ok" : del ? "text-bad" : "text-faint"}`} />}
      <span className={`min-w-0 flex-1 whitespace-pre-wrap pr-3 [overflow-wrap:anywhere] [tab-size:4] ${plain ? "text-faint" : "text-ink-2"}`} style={{ paddingLeft: `${lead}ch`, textIndent: `-${lead}ch` }}>{body}</span>
      {children}
    </div>
  );
}

/** "… 400 more lines": what a step leaves undrawn past MAX_ROWS. */
export const More = ({ n }: { n: number }) => n > 0 ? <div className="px-2.5 py-0.5 font-sans text-[11px] text-faint select-none">… {n} more line{n === 1 ? "" : "s"}</div> : null;
