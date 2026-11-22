import { ChevronLeft, MessageSquare, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { core, type Review } from "../lib/tazuna";
import { numbered, parse, patchOf, patchOfFile, sideBySide, wordMarks, type FileDiff } from "../lib/unidiff";
import { code } from "../lib/markdown";
import { Counts, Parts, Sep } from "./Sep";
import { keysOf } from "../lib/types";
import { accentButton, Segmented, quietButton } from "./Controls";

type Comment = { path: string; line: number; text: string; quote: string };
type Writing = { path: string; line: number; quote: string; text: string };

const SPLIT = "tazuna.review.split";

// The review: a pane over the conversation (Esc goes back to it), not a dialog over the window, since desktop 0.1.94
// (design review 9, variant A of mockups/v6.html). Every changed file one under another, the list on the left jumps to
// one; the words that changed inside a line are marked; unified or split. Each piece, or a whole file, can be staged or
// reverted in the work tree; a click on a line leaves a comment, and the comments go to the agent together. Scopes:
// uncommitted, the branch against its base, one turn (from its change card).
export function ReviewView({ start, turns, onClose, onSend }: {
  /** with `path`: that file in view once the diff is in (a change card's "Diff") */
  start: { scope: string; turn?: number; path?: string }; turns: number[]; onClose: () => void; onSend: (text: string) => void;
}) {
  const [scope, setScope] = useState(start);
  const [review, setReview] = useState<Review | null>(null);
  const [sel, setSel] = useState<string | null>(start.path ?? null);
  const [comments, setComments] = useState<Comment[]>([]);
  const [writing, setWriting] = useState<Writing | null>(null);
  const [split, setSplitNow] = useState(() => localStorage.getItem(SPLIT) === "1");
  const setSplit = (on: boolean) => { localStorage.setItem(SPLIT, on ? "1" : "0"); setSplitNow(on); };
  // a whole file reverted takes a second click (its changes leave the work tree)
  const [armed, setArmed] = useState<string | null>(null);
  const pane = useRef<HTMLElement>(null), files_ = useRef<Record<string, HTMLDivElement | null>>({});
  const writingNow = useRef(writing);
  writingNow.current = writing;
  const load = () => core.review(scope.scope, scope.turn).catch(() => {});
  useEffect(() => {
    const off = core.onReview(setReview);
    load();
    pane.current?.focus();
    // Esc goes back to the conversation; while a comment is written it drops the comment instead (the editor's own)
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape" && !writingNow.current) onClose(); };
    window.addEventListener("keydown", esc);
    return () => { off.then((f) => f()); window.removeEventListener("keydown", esc); };
  }, [scope.scope, scope.turn]);
  const files = useMemo(() => parse(review?.text ?? ""), [review]);
  // a change card's "Diff" opens here at its file
  const [wanted, setWanted] = useState(start.path);
  useEffect(() => {
    if (!wanted || !files.length) return;
    files_.current[wanted]?.scrollIntoView({ block: "start" });
    setWanted(undefined);
  }, [files, wanted]);
  const act = (patch: string, stage: boolean) => core.piece(patch, stage).then(() => setTimeout(load, 300)).catch(() => {});
  const revertFile = (f: FileDiff) => {
    if (armed !== f.path) { setArmed(f.path); setTimeout(() => setArmed((a) => (a === f.path ? null : a)), 3000); return; }
    setArmed(null);
    act(patchOfFile(f), false);
  };
  const send = () => {
    const body = comments.map((c) => `- \`${c.path}\` line ${c.line}: ${c.text}\n  > ${c.quote.slice(1).trim()}`).join("\n");
    onSend(`Review comments on the changes; please address each:\n\n${body}`);
    onClose();
  };
  const keep = (w: Writing) => {
    const rest = comments.filter((x) => !(x.path === w.path && x.line === w.line));
    setComments(w.text.trim() ? [...rest, { ...w, text: w.text.trim() }] : rest);
    setWriting(null);
  };
  // the scopes as one switch: "uncommitted", "branch", then a turn by its number
  const scopes: (readonly [string, string])[] = [["uncommitted", "Uncommitted"], ["branch", "Branch"], ...turns.map((t) => [`turn:${t}`, `Turn ${t + 1}`] as const)];
  const added = files.reduce((n, f) => n + f.added, 0), removed = files.reduce((n, f) => n + f.removed, 0);
  const textButton = "rounded px-2 py-0.5 text-[12px] hover:bg-hover";

  /** One side of a line: its number, its sign, its code with the changed words marked; a click comments on it. */
  const line = (f: FileDiff, i: number, marks: Map<number, [number, number]>, nums: ReturnType<typeof numbered>, num: "old" | "new" | "both") => {
    const l = nums[i], kind = l.line[0], at = l.new ?? l.old ?? 0;
    const c = comments.find((x) => x.path === f.path && x.line === at && x.quote === l.line);
    const text = l.line.slice(1), m = marks.get(i);
    const body: ReactNode = m && m[1] > m[0]
      ? <>{code(text.slice(0, m[0]), 0, false)}<mark className={`rounded-sm text-inherit ${kind === "+" ? "bg-ok/25" : "bg-bad/25"}`}>{code(text.slice(m[0], m[1]), 1, false)}</mark>{code(text.slice(m[1]), 2, false)}</>
      : code(text, 0, false);
    // a wrapped line hangs under its own indent, two columns in (design review 10: it went back to the code's first column)
    const lead = (text.match(/^[ \t]*/)?.[0] ?? "").replace(/\t/g, "    ").length + 2;
    const comment = () => kind !== "\\" && setWriting({ path: f.path, line: at, quote: l.line, text: c?.text ?? "" });
    return (
      <div onClick={comment} role="button" tabIndex={0} aria-label={`${c ? "Edit the comment on" : "Comment on"} line ${at}: ${l.line.slice(0, 80)}`}
        onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); comment(); } }}
        className={`group flex min-w-0 cursor-pointer ${kind === "+" ? "bg-ok/10" : kind === "-" ? "bg-bad/10" : ""} hover:bg-accent-soft`}>
        {num !== "new" && <span className="w-10 flex-none select-none pr-2 text-right text-faint">{l.old ?? ""}</span>}
        {num !== "old" && <span className="w-10 flex-none select-none pr-2 text-right text-faint">{l.new ?? ""}</span>}
        <span className={`w-4 flex-none select-none ${kind === "+" ? "text-ok" : kind === "-" ? "text-bad" : "text-faint"}`}>{kind === " " ? "" : kind}</span>
        <span className="min-w-0 flex-1 whitespace-pre-wrap pr-3 text-ink-2 [overflow-wrap:anywhere] [tab-size:4]" style={{ paddingLeft: `${lead}ch`, textIndent: `-${lead}ch` }}>{body}</span>
        <MessageSquare size={12} strokeWidth={1.5} className={`mr-2 mt-1 flex-none ${c ? "text-accent" : "invisible text-faint group-hover:visible group-focus-visible:visible"}`} />
      </div>
    );
  };
  /** The comment kept on a line, or the editor while one is written, under that line. */
  const thread = (f: FileDiff, nums: ReturnType<typeof numbered>, i: number | null) => {
    if (i == null) return null;
    const l = nums[i], at = l.new ?? l.old ?? 0;
    const c = comments.find((x) => x.path === f.path && x.line === at && x.quote === l.line);
    if (writing && writing.path === f.path && writing.line === at && writing.quote === l.line) {
      return (
        <div className="border-y border-line-soft bg-main px-3 py-2.5 pl-24 font-sans">
          <div className="max-w-[640px] rounded-lg border border-accent bg-card">
            <textarea autoFocus rows={2} value={writing.text} onChange={(e) => setWriting({ ...writing, text: e.target.value })}
              onKeyDown={(e) => {
                if (e.key === "Escape") { e.stopPropagation(); setWriting(null); }
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) keep(writing);
              }}
              placeholder={`Ask tazuna to change line ${at}…`}
              className="block w-full resize-none rounded-t-lg bg-transparent px-3 py-2 text-[13px] text-ink outline-none placeholder:text-faint" />
            <div className="flex items-center gap-2 border-t border-line-soft px-3 py-1.5 text-[11.5px] text-faint">
              <span className="flex flex-1 items-center">Goes with your other comments when you send them<Sep />{keysOf("⌘↵")}</span>
              <button onClick={() => setWriting(null)} className={quietButton}>Cancel</button>
              <button onClick={() => keep(writing)} className={quietButton}>{c ? "Save" : "Add comment"}</button>
            </div>
          </div>
        </div>
      );
    }
    return c ? (
      <div className="flex items-start gap-2 border-y border-line-soft bg-main px-3 py-2 pl-24 font-sans text-[12.5px]">
        <span className="flex-none pt-px text-[11.5px] text-faint">You</span><span className="max-w-[640px] flex-1 text-ink-2">{c.text}</span>
        <button onClick={() => setComments(comments.filter((x) => x !== c))} title="Remove the comment" aria-label="Remove the comment" className="text-faint hover:text-ink"><X size={12} strokeWidth={1.5} /></button>
      </div>
    ) : null;
  };

  return (
    <section ref={pane} tabIndex={-1} aria-label="Review changes" className="tz-enter absolute inset-0 z-10 flex min-h-0 flex-col bg-main outline-none">
      <div className="flex h-[49px] flex-none items-center gap-2.5 border-b border-line-soft pl-3 pr-4">
        <button onClick={onClose} title="Back to the conversation (Esc)" className="flex items-center gap-1.5 rounded-md px-2 py-1 text-[12.5px] text-muted hover:bg-hover hover:text-ink">
          <ChevronLeft size={14} strokeWidth={1.5} />Conversation<kbd className="rounded-[5px] border border-line px-[5px] font-mono text-[11px] text-faint">Esc</kbd></button>
        <span className="h-4 w-px bg-line-strong" aria-hidden="true" />
        <span className="font-medium">Review</span>
        <Segmented label="What to review" value={scope.scope === "turn" ? `turn:${scope.turn}` : scope.scope} options={scopes}
          onChange={(v) => { setScope(v.startsWith("turn:") ? { scope: "turn", turn: Number(v.slice(5)) } : { scope: v, turn: undefined }); setSel(null); }} />
        <span className="min-w-0 flex-1 truncate text-[12px] text-faint">{review?.scope === "branch" && review.base ? `against ${review.base}` : ""}</span>
        <Segmented label="Layout" value={split ? "split" : "unified"} options={[["unified", "Unified"], ["split", "Split"]]} onChange={(v) => setSplit(v === "split")} />
        {comments.length > 0 && <button onClick={send} className={accentButton}>
          Send {comments.length} comment{comments.length === 1 ? "" : "s"}</button>}
      </div>
      <div className="flex min-h-0 flex-1">
        <nav aria-label="Changed files" className="flex w-60 flex-none flex-col overflow-auto border-r border-line-soft p-2">
          {!review && <div className="px-2 py-1.5 text-[12.5px] text-faint">Loading…</div>}
          {review && files.length === 0 && <div className="px-2 py-1.5 text-[12.5px] text-faint">{review.scope === "branch" && review.base && !review.text ? review.base : "No changes."}</div>}
          {files.map((f) => {
            const st = f.head.some((h) => h.startsWith("new file")) ? ["A", "text-ok"] : f.head.some((h) => h.startsWith("deleted file")) ? ["D", "text-bad"] : ["M", "text-warn"];
            const name = f.path.slice(f.path.lastIndexOf("/") + 1), dir = f.path.slice(0, f.path.length - name.length);
            return (
              <button key={f.path} title={f.path} onClick={() => { setSel(f.path); files_.current[f.path]?.scrollIntoView({ block: "start", behavior: "smooth" }); }}
                className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12.5px] ${sel === f.path ? "bg-selected text-ink" : "text-ink-2 hover:bg-hover"}`}>
                <span className={`w-3 flex-none font-mono text-[11px] ${st[1]}`}>{st[0]}</span>
                <span className="min-w-0 flex-1 truncate">{name}<span className="ml-1 text-[11.5px] text-faint">{dir}</span></span>
                <Counts added={f.added} removed={f.removed} className="text-[11.5px]" />
              </button>
            );
          })}
          {/* hairlines, not middle dots (design review 10) */}
          {files.length > 0 && <Parts className="mt-2 flex border-t border-line-soft px-2 pt-2 text-[11.5px] text-faint"
            items={[`${files.length} file${files.length === 1 ? "" : "s"}`, (added > 0 || removed > 0) && <span key="n"><Counts added={added} removed={removed} /></span>,
              comments.length > 0 && `${comments.length} comment${comments.length === 1 ? "" : "s"}`]} />}
        </nav>
        <div className="min-w-0 flex-1 overflow-auto px-5 py-4">
          {files.map((f) => (
            <div key={f.path} ref={(el) => { files_.current[f.path] = el; }} className="mb-4 scroll-mt-4 overflow-hidden rounded-lg border border-line bg-card">
              <div className="flex items-center gap-2.5 border-b border-line bg-field px-3 py-2">
                <span className="min-w-0 truncate font-mono text-[12.5px] text-ink">{f.path}</span>
                <Counts added={f.added} removed={f.removed} className="text-[12px]" />
                <span className="flex-1" />
                {!f.binary && <>
                  <button onClick={() => act(patchOfFile(f), true)} title="Add the whole file's changes to the next commit" className={`${textButton} text-muted hover:text-ink`}>Stage file</button>
                  <button onClick={() => revertFile(f)} title="Take the whole file's changes out of the work tree"
                    className={`${textButton} ${armed === f.path ? "bg-bad/15 font-medium text-bad" : "text-bad"}`}>{armed === f.path ? "Click again to revert" : "Revert file"}</button>
                </>}
              </div>
              {f.binary && <div className="px-3 py-2 text-[12.5px] text-faint">A binary file: no text to review.</div>}
              {f.pieces.map((p, k) => {
                const nums = numbered(p), rows = sideBySide(p), marks = wordMarks(p);
                return (
                  <div key={`${k}:${p.header}`} className={k > 0 ? "border-t border-line" : ""}>
                    <div className="flex items-center gap-2 border-b border-line-soft bg-accent-soft/40 px-3 py-1">
                      <span className="min-w-0 flex-1 truncate font-mono text-[11.5px] text-faint">{p.header}</span>
                      <button onClick={() => act(patchOf(f, p), true)} title="Add this piece to the next commit (git add, for this piece)" className={`${textButton} text-muted hover:text-ink`}>Stage</button>
                      <button onClick={() => act(patchOf(f, p), false)} title="Take this piece out of the files" className={`${textButton} text-bad`}>Revert</button>
                    </div>
                    <div className="font-mono text-[12px] leading-[1.6] select-text">
                      {split
                        ? rows.map((r, n) => (
                          <div key={n}>
                            <div className="grid grid-cols-2 divide-x divide-line-soft">
                              {r.left != null ? line(f, r.left, marks, nums, "old") : <span />}
                              {r.right != null ? line(f, r.right, marks, nums, "new") : <span />}
                            </div>
                            {thread(f, nums, r.left)}{r.right !== r.left && thread(f, nums, r.right)}
                          </div>
                        ))
                        : nums.map((_, i) => <div key={i}>{line(f, i, marks, nums, "both")}{thread(f, nums, i)}</div>)}
                    </div>
                  </div>
                );
              })}
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
