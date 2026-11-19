import { X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useDialog } from "../lib/dialog";
import type { ModelChoice, View } from "../lib/tazuna";
import { usd } from "../lib/types";

// Best-of-N (docs/DESKTOP.md 1.1): one task on several models, each in a worktree of its own, compared on cost, time,
// what each changed and whether its checks passed: price to performance on the spot.

export type Lane = { model: ModelChoice; path?: string; tid?: string };
export type Lanes = { task: string; origin: string; lanes: Lane[] };

function useEscape(onClose: () => void) {
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") close.current(); };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, []);
}

/** The task and the models (two to four of those usable now). */
export function BestOfStart({ choices, onStart, onClose }: { choices: ModelChoice[]; onStart: (task: string, models: ModelChoice[]) => void; onClose: () => void }) {
  const dialog = useDialog();
  useEscape(onClose);
  const [task, setTask] = useState("");
  const [picked, setPicked] = useState<string[]>([]);
  const id = (c: ModelChoice) => `${c.model}|${c.url}|${c.key_env}`;
  const usable = choices.filter((c) => !c.why_not);
  const ok = task.trim() && picked.length >= 2;
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-30 grid place-items-center bg-scrim p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Try on several models" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()}
        className="tz-dialog flex max-h-full w-[600px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
        <div className="flex items-center border-b border-line-soft px-5 py-3">
          <span className="flex-1 font-medium">Try on several models</span>
          <button onClick={onClose} aria-label="Close" title="Close (Esc)" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="overflow-auto px-5 py-4">
          <textarea autoFocus value={task} onChange={(e) => setTask(e.target.value)} rows={3} aria-label="Task" placeholder="The task, as you would send it"
            className="w-full resize-none rounded-md border border-line bg-field px-2.5 py-2 text-ink outline-none focus:border-accent placeholder:text-faint" />
          <div className="mb-1.5 mt-3 text-[11.5px] font-medium text-faint">Models: two to four</div>
          {usable.map((c) => {
            const on = picked.includes(id(c));
            return (
              <label key={id(c)} className="flex items-center gap-2 py-1">
                <input type="checkbox" checked={on} disabled={!on && picked.length >= 4}
                  onChange={() => setPicked((p) => (on ? p.filter((x) => x !== id(c)) : [...p, id(c)]))} />
                <span className="text-ink">{c.label}</span><span className="text-[12px] text-muted">{c.place}{c.price ? ` · ${c.price}` : ""}</span>
              </label>
            );
          })}
          <div className="mt-3 text-[12px] text-muted">Each lane gets a worktree of its own (a branch and folder; the setup script does not run) and the same
            message, in auto mode: the agent works without asking, inside its worktree. Each lane bills as its model does.</div>
        </div>
        <div className="flex justify-end gap-2 border-t border-line-soft px-5 py-3">
          <button onClick={onClose} className="rounded-md px-2.5 py-1 text-muted hover:bg-hover">Cancel</button>
          <button disabled={!ok} onClick={() => onStart(task.trim(), usable.filter((c) => picked.includes(id(c))))}
            className="rounded-md bg-accent px-2.5 py-1 font-medium text-on-accent disabled:opacity-40">Start {picked.length || ""} lanes</button>
        </div>
      </div>
    </div>
  );
}

/** One lane's numbers from its conversation's view. */
export function laneStats(v: View | null | undefined) {
  if (!v) return null;
  const done = v.blocks.filter((b) => b.kind === "done");
  const files = (v.changes ?? []).filter((c) => !c.undone).flatMap((c) => c.files);
  const paths = new Set(files.map((f) => f.path));
  const checks = v.blocks.filter((b) => b.kind === "verify") as Extract<View["blocks"][number], { kind: "verify" }>[];
  const last = checks[checks.length - 1];
  return {
    running: !["Ready", "Done"].includes(v.activity) || done.length === 0,
    cost: v.cost, secs: done.reduce((n, b) => n + (b.kind === "done" ? b.secs : 0), 0),
    files: paths.size, added: files.reduce((n, f) => n + (f.added ?? 0), 0), removed: files.reduce((n, f) => n + (f.removed ?? 0), 0),
    check: last ? (last.ok ? "passed" : last.ok === false ? "failed" : "running") : "none",
    error: v.blocks.some((b) => b.kind === "notice" && b.error),
  };
}

/** The lanes side by side: open one, keep one (the others' worktrees and changes go, after a yes). */
export function LanesView({ lanes, views, onOpen, onKeep, onClose }: {
  lanes: Lanes; views: Record<string, View | null | undefined>; onOpen: (tid: string) => void; onKeep: (lane: Lane) => void; onClose: () => void;
}) {
  const dialog = useDialog();
  useEscape(onClose);
  const [keep, setKeep] = useState<Lane | null>(null);
  const rows = lanes.lanes.map((l) => ({ l, s: laneStats(l.tid ? views[l.tid] : null) }));
  const cheapest = Math.min(...rows.filter((r) => r.s && !r.s.running).map((r) => r.s!.cost));
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-30 grid place-items-center bg-scrim p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Lanes" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()}
        className="tz-dialog flex max-h-full w-[820px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
        <div className="flex items-center border-b border-line-soft px-5 py-3">
          <span className="min-w-0 flex-1"><span className="font-medium">Lanes</span> <span className="truncate text-[12px] text-muted">· {lanes.task}</span></span>
          <button onClick={onClose} aria-label="Close" title="Close (Esc)" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="overflow-auto px-5 py-3">
          <table className="w-full border-collapse text-[12.5px]">
            <thead className="text-[11.5px] text-faint">
              <tr>{["Model", "State", "Cost", "Time", "Changed", "Checks", "Actions"].map((h, i) => <th key={i} scope="col" className={`py-1 pr-3 font-normal ${i >= 2 && i <= 4 ? "text-right" : "text-left"} ${i === 6 ? "sr-only" : ""}`}>{h}</th>)}</tr>
            </thead>
            <tbody>
              {rows.map(({ l, s }, i) => (
                <tr key={i} className="border-t border-line-soft">
                  <td className="py-2 pr-3"><span className="text-ink">{l.model.label}</span><span className="block text-[11.5px] text-faint">{l.model.place}</span></td>
                  <td className="py-2 pr-3 text-muted">{!l.tid ? "starting…" : !s ? "starting…" : s.running ? "working…" : s.error ? "ended with an error" : "done"}</td>
                  <td className={`py-2 pr-3 text-right font-mono ${s && !s.running && s.cost === cheapest ? "text-ok" : ""}`}>{s ? usd(s.cost) : "–"}</td>
                  <td className="py-2 pr-3 text-right font-mono">{s && s.secs ? `${Math.round(s.secs)} s` : "–"}</td>
                  <td className="py-2 pr-3 text-right font-mono">{s ? `${s.files} files · +${s.added} −${s.removed}` : "–"}</td>
                  <td className={`py-2 pr-3 ${s?.check === "passed" ? "text-ok" : s?.check === "failed" ? "text-bad" : "text-muted"}`}>{s ? s.check : "–"}</td>
                  <td className="py-2 text-right whitespace-nowrap">
                    {l.tid && <button onClick={() => onOpen(l.tid!)} className="rounded-md border border-line px-2 py-0.5 text-[12px] text-ink-2 hover:bg-hover">Open</button>}
                    {l.tid && s && !s.running && <button onClick={() => setKeep(l)} className="ml-1.5 rounded-md border border-line-strong px-2 py-0.5 text-[12px] text-ink hover:bg-hover">Keep</button>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {keep && (
            <div role="alert" className="mt-3 flex items-center gap-2 rounded-md border border-line-strong px-3 py-2 text-[12.5px]">
              <span className="flex-1 text-ink-2">Keep {keep.model.label}'s lane? The other lanes' worktrees go, with their uncommitted changes (their branches stay).</span>
              <button onClick={() => { onKeep(keep); setKeep(null); }} className="rounded-md border border-line-strong px-2.5 py-1 font-medium text-bad hover:bg-hover">Remove the others</button>
              <button onClick={() => setKeep(null)} className="rounded-md px-2 py-1 text-muted hover:bg-hover">Cancel</button>
            </div>
          )}
          <div className="mt-3 text-[11.5px] text-faint">Changed: lines the turns changed, from the core's snapshots. Checks: tazuna's tests after the agent said it was done, where the project has them.</div>
        </div>
      </div>
    </div>
  );
}
