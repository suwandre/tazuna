import { ExternalLink, X } from "lucide-react";
import { useEffect, useRef } from "react";
import { core } from "../lib/tazuna";
import { checksOf, type Pr } from "../lib/pr";

// The PR panel (docs/DESKTOP.md 0.3): the pull request of this folder's branch through gh. Its checks, reviews and
// comments; a failing check or the review comments go to the agent in one click; no PR yet: ask tazuna to open one.
export function PrView({ pr, worktree, onClose, onSend, onRefresh, onRemoveWorktree }: {
  pr: Pr | null; worktree: boolean; onClose: () => void; onSend: (text: string) => void; onRefresh: () => void; onRemoveWorktree: () => void;
}) {
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") close.current(); };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, []);
  const checks = pr && !pr.none && !pr.error ? checksOf(pr) : [];
  const failing = checks.filter((c) => c.state === "failed");
  const notes = pr && !pr.none && !pr.error ? [
    ...(pr.reviews ?? []).filter((r) => r.body?.trim()).map((r) => ({ who: r.author?.login ?? "?", where: r.state.toLowerCase().replace("_", " "), body: r.body })),
    ...(pr.inline ?? []).map((c) => ({ who: c.author ?? "?", where: `${c.path}${c.line ? `:${c.line}` : ""}`, body: c.body })),
    ...(pr.comments ?? []).map((c) => ({ who: c.author?.login ?? "?", where: "comment", body: c.body })),
  ] : [];
  const fix = (c: { name: string; url?: string }) => onSend(`The CI check "${c.name}" fails on PR #${pr!.number} (${pr!.url}). Read its log (gh pr checks ${pr!.number}, then gh run view <run id> --log-failed) and fix the cause.`);
  const address = () => onSend(`Address the review comments on PR #${pr!.number} (${pr!.url}):\n\n${notes.slice(0, 40).map((n) => `- ${n.who} (${n.where}): ${n.body.trim().slice(0, 600)}`).join("\n")}\n\nFix what is right; for anything you disagree with, say why instead of changing it.`);
  const h = "mb-2 mt-5 text-[11.5px] font-medium text-faint";
  return (
    <div onMouseDown={onClose} className="fixed inset-0 z-30 grid place-items-center bg-ink/15 p-10">
      <div onMouseDown={(e) => e.stopPropagation()} className="flex max-h-full w-[720px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
        <div className="flex items-center gap-2 border-b border-line-soft px-5 py-3">
          <span className="flex-1 truncate font-medium">{pr && !pr.none && !pr.error ? `#${pr.number} ${pr.title}` : "Pull request"}</span>
          <button onClick={onRefresh} className="rounded-md px-2 py-0.5 text-[12px] text-muted hover:bg-hover">Refresh</button>
          {pr?.url && <button onClick={() => core.openLink(pr.url!)} title="Open on GitHub" className="rounded p-1 text-muted hover:bg-hover"><ExternalLink size={14} strokeWidth={1.5} /></button>}
          <button onClick={onClose} title="Close (Esc)" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="overflow-auto px-5 pb-5 pt-3 text-[12.5px]">
          {!pr && <div className="text-faint">Asking GitHub…</div>}
          {pr?.error && <div className="text-bad">{pr.error}</div>}
          {pr?.none && <>
            <div className="text-ink-2">No pull request for the branch <span className="font-mono">{pr.branch}</span> yet.</div>
            <button onClick={() => onSend("Commit what is ready, push this branch and open a pull request with gh pr create: a short title, and a description of what changed and why.")}
              className="mt-3 rounded-md border border-line-strong px-2.5 py-1 text-ink hover:bg-hover">Ask tazuna to open one</button>
          </>}
          {pr && !pr.none && !pr.error && <>
            <div className="flex flex-wrap gap-x-3 text-muted">
              <span className={pr.state === "MERGED" ? "text-ok" : pr.state === "CLOSED" ? "text-bad" : "text-ink-2"}>{pr.isDraft ? "draft" : pr.state.toLowerCase()}</span>
              <span className="font-mono">{pr.headRefName} → {pr.baseRefName}</span>
              {pr.reviewDecision && <span>{pr.reviewDecision.toLowerCase().replace(/_/g, " ")}</span>}
            </div>
            {pr.state === "MERGED" && worktree && (
              <div className="mt-3 flex items-center gap-2 rounded-md border border-line px-3 py-2"><span className="flex-1 text-ink-2">Merged. This worktree's work is in; its branch stays.</span>
                <button onClick={onRemoveWorktree} className="rounded-md border border-line-strong px-2.5 py-1 text-ink hover:bg-hover">Remove the worktree</button></div>
            )}
            <div className={h}>Checks {checks.length > 0 && `· ${checks.filter((c) => c.state === "passed").length} of ${checks.length} passed`}</div>
            {checks.length === 0 && <div className="text-faint">None reported.</div>}
            {checks.map((c, i) => (
              <div key={i} className="flex items-center gap-2 border-b border-line-soft py-1.5 last:border-0">
                <span className={`w-14 text-[11.5px] ${c.state === "failed" ? "text-bad" : c.state === "passed" ? "text-ok" : "text-muted"}`}>{c.state}</span>
                <span className="flex-1 truncate text-ink-2">{c.name}</span>
                {c.url && <button onClick={() => core.openLink(c.url!)} className="text-[12px] text-muted hover:text-ink">log</button>}
                {c.state === "failed" && <button onClick={() => fix(c)} className="rounded-md border border-line-strong px-2 py-0.5 text-[12px] text-ink hover:bg-hover">Fix</button>}
              </div>
            ))}
            {failing.length > 1 && <button onClick={() => onSend(`These CI checks fail on PR #${pr.number} (${pr.url}): ${failing.map((c) => `"${c.name}"`).join(", ")}. Read their logs (gh pr checks ${pr.number}, gh run view <run id> --log-failed) and fix the causes.`)}
              className="mt-2 rounded-md border border-line-strong px-2.5 py-1 text-[12px] text-ink hover:bg-hover">Fix all {failing.length}</button>}
            <div className={h}>Reviews and comments {notes.length > 0 && `· ${notes.length}`}</div>
            {notes.length === 0 && <div className="text-faint">None yet.</div>}
            {notes.slice(0, 60).map((n, i) => (
              <div key={i} className="border-b border-line-soft py-2 last:border-0">
                <div className="mb-0.5 text-[11.5px] text-faint"><span className="text-ink-2">{n.who}</span> · <span className="font-mono">{n.where}</span></div>
                <div className="whitespace-pre-wrap text-ink-2">{n.body.trim().slice(0, 1200)}</div>
              </div>
            ))}
            {notes.length > 0 && <button onClick={address} className="mt-2 rounded-md bg-accent px-2.5 py-1 font-medium text-on-accent">Address them with tazuna</button>}
          </>}
        </div>
      </div>
    </div>
  );
}
