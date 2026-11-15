import { ShieldQuestion } from "lucide-react";
import { useEffect, useState } from "react";
import type { View } from "../lib/tazuna";

// tazuna's permission question over the composer: allow once, always (this command or tool, for the session), or
// decline. y / a / n answer from the keyboard; nothing answers in the first 600 ms, so a key meant for something else
// cannot approve a question that just appeared (the terminal's ANSWER_DELAY_MS).
export function Approval({ asking, onAnswer }: { asking: NonNullable<View["asking"]>; onAnswer: (allow: boolean, always: boolean) => void }) {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    setReady(false);
    const t = setTimeout(() => setReady(true), 600);
    return () => clearTimeout(t);
  }, [asking.text]);
  useEffect(() => {
    if (!ready) return;
    const key = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const k = e.key.toLowerCase();
      const pick = k === "y" ? [true, false] : k === "a" && !asking.once ? [true, true] : k === "n" || k === "escape" ? [false, false] : null;
      if (pick) { e.preventDefault(); onAnswer(pick[0], pick[1]); }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [ready, asking, onAnswer]);
  const tool = asking.tool.replace(/^mcp__\w+?__/, "");
  const what = asking.text.replace(/^[^:]+:\s*/, "").replace(/\s+\((always|once only)[^)]*\)$/, "");
  const btn = "rounded-md px-2.5 py-1 text-[12px] font-medium disabled:opacity-50";
  return (
    <div className="mx-auto mb-2 w-full max-w-[700px] px-7">
      <div className="rounded-lg border border-line-strong bg-card px-3.5 py-3">
        <div className="mb-1.5 flex items-center gap-2 text-[12.5px] text-ink-2">
          <ShieldQuestion size={15} strokeWidth={1.5} className="text-warn" />
          tazuna asks to use <span className="font-medium text-ink">{tool}</span>
          {asking.queued > 0 && <span className="text-faint">· {asking.queued} more waiting</span>}
        </div>
        <pre className="mb-2.5 max-h-40 overflow-auto whitespace-pre-wrap break-all rounded-md bg-field px-2.5 py-2 font-mono text-[12px] text-ink select-text">{what}</pre>
        <div className="flex items-center gap-1.5">
          <button disabled={!ready} onClick={() => onAnswer(true, false)} className={`${btn} bg-accent text-on-accent`}>Allow <kbd className="ml-1 font-mono opacity-70">y</kbd></button>
          {!asking.once && (
            <button disabled={!ready} onClick={() => onAnswer(true, true)} className={`${btn} border border-line text-ink`}
              title={/bash|powershell/i.test(tool) ? "this command, for the rest of the session" : "this tool, for the rest of the session"}>
              Always <kbd className="ml-1 font-mono text-faint">a</kbd>
            </button>
          )}
          <button disabled={!ready} onClick={() => onAnswer(false, false)} className={`${btn} border border-line text-muted`}>Decline <kbd className="ml-1 font-mono text-faint">n</kbd></button>
          {asking.once && <span className="ml-1 text-[11.5px] text-faint">once only: the model wrote this call as text</span>}
        </div>
      </div>
    </div>
  );
}
