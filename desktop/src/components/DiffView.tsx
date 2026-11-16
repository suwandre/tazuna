import { X } from "lucide-react";
import { useEffect } from "react";
import type { Diff } from "../lib/tazuna";

// One file's diff for a turn, over the transcript; Esc or a click outside closes it.
export function DiffView({ diff, onClose }: { diff: Diff; onClose: () => void }) {
  useEffect(() => {
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onClose]);
  const lines = diff.text.split("\n");
  const body = lines.slice(Math.max(0, lines.findIndex((l) => l.startsWith("@@"))));
  const tone = (l: string) => l.startsWith("+") ? "bg-ok/10 text-ok" : l.startsWith("-") ? "bg-bad/10 text-bad" : l.startsWith("@@") ? "text-faint" : "text-ink-2";
  return (
    <div onMouseDown={onClose} className="fixed inset-0 z-20 grid place-items-center bg-ink/20 p-10">
      <div onMouseDown={(e) => e.stopPropagation()} className="flex max-h-full w-full max-w-[900px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
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
