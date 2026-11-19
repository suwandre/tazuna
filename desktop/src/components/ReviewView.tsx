import { MessageSquare, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { core, type Review } from "../lib/tazuna";
import { useDialog } from "../lib/dialog";
import { numbered, parse, patchOf, type FileDiff, type Piece } from "../lib/unidiff";
import { code } from "../lib/markdown";

type Comment = { path: string; line: number; text: string; quote: string };

// The review: a diff gone through piece by piece. Each piece can be reverted in the work tree or staged; a click on a
// line leaves a comment, and the comments go to the agent together. Scopes: uncommitted, the branch against its base,
// one turn (from its change card).
export function ReviewView({ start, turns, onClose, onSend }: {
  /** with `path`: that file selected once the diff is in (a change card's "Diff") */
  start: { scope: string; turn?: number; path?: string }; turns: number[]; onClose: () => void; onSend: (text: string) => void;
}) {
  const [scope, setScope] = useState(start);
  const dialog = useDialog();
  const [review, setReview] = useState<Review | null>(null);
  const [sel, setSel] = useState(0);
  const [comments, setComments] = useState<Comment[]>([]);
  const [writing, setWriting] = useState<{ path: string; line: number; quote: string; text: string } | null>(null);
  const load = () => core.review(scope.scope, scope.turn).catch(() => {});
  useEffect(() => {
    const off = core.onReview(setReview);
    load();
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape" && !writing) onClose(); };
    window.addEventListener("keydown", esc);
    return () => { off.then((f) => f()); window.removeEventListener("keydown", esc); };
  }, [scope.scope, scope.turn]);
  const files = useMemo(() => parse(review?.text ?? ""), [review]);
  const file = files[Math.min(sel, files.length - 1)];
  // a change card's "Diff" opens here at its file (until desktop 0.1.69 it opened a second, simpler diff without line numbers)
  const [wanted, setWanted] = useState(start.path);
  useEffect(() => { if (!wanted || !files.length) return; const i = files.findIndex((f) => f.path === wanted); if (i >= 0) setSel(i); setWanted(undefined); }, [files, wanted]);
  const act = (f: FileDiff, p: Piece, stage: boolean) => core.piece(patchOf(f, p), stage).then(() => setTimeout(load, 300)).catch(() => {});
  const send = () => {
    const body = comments.map((c) => `- \`${c.path}\` line ${c.line}: ${c.text}\n  > ${c.quote.slice(1).trim()}`).join("\n");
    onSend(`Review comments on the changes; please address each:\n\n${body}`);
    onClose();
  };
  const tab = (s: string, label: string, turn?: number) => (
    <button key={label} onClick={() => { setScope({ scope: s, turn }); setSel(0); }}
      className={`rounded-md px-2.5 py-1 text-[12px] ${scope.scope === s && scope.turn === turn ? "bg-selected text-ink" : "text-muted hover:bg-hover"}`}>{label}</button>
  );
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-20 flex bg-scrim p-8">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Review changes" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className="tz-dialog flex min-h-0 w-full flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
        <div className="flex items-center gap-1.5 border-b border-line-soft px-4 py-2.5">
          <span className="mr-2 font-medium">Review</span>
          {tab("uncommitted", "Uncommitted")}{tab("branch", "Branch")}
          {turns.map((t) => tab("turn", `Turn ${t + 1}`, t))}
          <span className="flex-1 truncate pl-2 text-[12px] text-faint">{review?.scope === "branch" && review.base ? `against ${review.base}` : ""}</span>
          {comments.length > 0 && <button onClick={send} className="rounded-md bg-accent px-2.5 py-1 text-[12px] font-medium text-on-accent">
            Send {comments.length} comment{comments.length === 1 ? "" : "s"} to the agent</button>}
          <button onClick={onClose} title="Close (Esc)" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="flex min-h-0 flex-1">
          <div className="w-64 flex-none overflow-auto border-r border-line-soft py-1">
            {!review && <div className="px-3 py-2 text-[12.5px] text-faint">Loading…</div>}
            {review && files.length === 0 && <div className="px-3 py-2 text-[12.5px] text-faint">{review.scope === "branch" && review.base && !review.text ? review.base : "No changes."}</div>}
            {files.map((f, i) => (
              <button key={f.path} onClick={() => setSel(i)} title={f.path}
                className={`flex w-full items-center gap-2 px-3 py-1.5 text-left text-[12.5px] ${i === sel ? "bg-selected text-ink" : "text-ink-2 hover:bg-hover"}`}>
                <span className="min-w-0 flex-1 truncate font-mono text-[12px]">{f.path}</span>
                <span className="font-mono text-[11px] text-ok">+{f.added}</span><span className="font-mono text-[11px] text-bad">−{f.removed}</span>
              </button>
            ))}
          </div>
          <div className="min-w-0 flex-1 overflow-auto px-4 py-3">
            {file?.binary && <div className="text-[12.5px] text-faint">A binary file: no text to review.</div>}
            {file?.pieces.map((p, k) => (
              <div key={`${file.path}:${k}:${p.header}`} className="mb-4 overflow-hidden rounded-lg border border-line">
                <div className="flex items-center gap-2 border-b border-line-soft bg-field px-3 py-1.5">
                  <span className="flex-1 truncate font-mono text-[11.5px] text-faint">{p.header}</span>
                  <button onClick={() => act(file, p, true)} title="Add this piece to the next commit (git add, for this piece)" className="rounded-md border border-line px-2 py-0.5 text-[12px] text-ink-2 hover:bg-hover">Stage</button>
                  <button onClick={() => act(file, p, false)} title="Take this piece out of the files" className="rounded-md border border-line px-2 py-0.5 text-[12px] text-bad hover:bg-hover">Revert</button>
                </div>
                <div className="font-mono text-[12px] leading-[1.55] select-text">
                  {numbered(p).map((l, i) => {
                    const at = l.new ?? l.old ?? 0;
                    const c = comments.find((x) => x.path === file.path && x.line === at && l.line[0] !== "-");
                    return (
                      <div key={i}>
                        <div onClick={() => l.line[0] !== "\\" && setWriting({ path: file.path, line: at, quote: l.line, text: c?.text ?? "" })}
                          {...(l.line[0] !== "\\" && { role: "button", tabIndex: 0, "aria-label": `${c ? "Edit the comment on" : "Comment on"} line ${at}: ${l.line.slice(0, 80)}`,
                            onKeyDown: (e: React.KeyboardEvent) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setWriting({ path: file.path, line: at, quote: l.line, text: c?.text ?? "" }); } } })}
                          className={`group flex cursor-pointer ${l.line[0] === "+" ? "bg-ok/10" : l.line[0] === "-" ? "bg-bad/10" : ""} hover:bg-accent-soft`}>
                          <span className="w-10 flex-none select-none pr-2 text-right text-faint">{l.old ?? ""}</span>
                          <span className="w-10 flex-none select-none pr-2 text-right text-faint">{l.new ?? ""}</span>
                          {/* the code in its colours, the +/- in the change's (until desktop 0.1.74 a whole line green or red) */}
                          <span className="min-w-0 flex-1 whitespace-pre-wrap break-all pr-3 text-ink-2"><span className={l.line[0] === "+" ? "text-ok" : l.line[0] === "-" ? "text-bad" : ""}>{l.line[0] ?? " "}</span>{code(l.line.slice(1), 0, false)}</span>
                          <MessageSquare size={12} strokeWidth={1.5} className={`mr-2 mt-1 flex-none ${c ? "text-accent" : "invisible text-faint group-hover:visible group-focus-visible:visible"}`} />
                        </div>
                        {c && !(writing && writing.path === file.path && writing.line === at) && (
                          <div className="flex items-start gap-2 border-y border-line-soft bg-accent-soft px-3 py-1.5 font-sans text-[12.5px] text-ink-2">
                            <MessageSquare size={12} strokeWidth={1.5} className="mt-[3px] flex-none text-accent" /><span className="flex-1">{c.text}</span>
                            <button onClick={() => setComments(comments.filter((x) => x !== c))} title="Remove the comment" className="text-faint hover:text-ink"><X size={12} strokeWidth={1.5} /></button>
                          </div>
                        )}
                        {writing && writing.path === file.path && writing.line === at && writing.quote === l.line && (
                          <div className="border-y border-line-soft bg-card px-3 py-2 font-sans">
                            <textarea autoFocus rows={2} value={writing.text} onChange={(e) => setWriting({ ...writing, text: e.target.value })}
                              onKeyDown={(e) => {
                                if (e.key === "Escape") { e.stopPropagation(); setWriting(null); }
                                if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                                  const rest = comments.filter((x) => !(x.path === writing.path && x.line === writing.line));
                                  setComments(writing.text.trim() ? [...rest, { ...writing, text: writing.text.trim() }] : rest);
                                  setWriting(null);
                                }
                              }}
                              placeholder="A comment for the agent on this line (⌘↵ keeps it, Esc drops it)"
                              className="w-full resize-none rounded-md border border-line bg-field px-2 py-1.5 text-[12.5px] text-ink outline-none focus:border-accent" />
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
