import { Square, X } from "lucide-react";
import { messages as viewMessages } from "../lib/fromView";
import type { Task, View } from "../lib/tazuna";
import { Parts } from "./Sep";
import { Transcript } from "./Transcript";

const tokens = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k tokens` : n > 0 ? `${n} tokens` : null);

/** A subagent's own conversation beside the one that started it (desktop 0.1.92; the terminal's ctrl-b, enter): what it
 * was asked, its reasoning, steps and answers so far, drawn as any conversation. Read only: it answers to the agent that
 * started it, not to you. While it runs it can be stopped here; the conversation and its other tasks go on. */
export function AgentPanel({ view, task, title, running, onStop, onClose }: {
  view: View | null; task?: Task; title: string; running: boolean; onStop?: () => void; onClose: () => void;
}) {
  const messages = view ? viewMessages(view) : [];
  return (
    <aside data-side aria-label="Subagent" className="tz-enter mr-2 mt-2 flex min-h-0 w-[432px] flex-col overflow-hidden rounded-xl bg-main shadow-sheet">
      <div className="flex items-start gap-2 border-b border-line-soft px-4 py-3">
        <span className="min-w-0 flex-1"><span className="block truncate font-medium" title={title}>{title || "Subagent"}</span>
          <Parts className="block truncate text-[11.5px] text-muted" items={[task?.agent || "subagent", running ? "working" : task?.status ?? "ended",
            task && tokens(task.tokens), task && task.tools > 0 && `${task.tools} step${task.tools === 1 ? "" : "s"}`]} /></span>
        {running && onStop && <button onClick={onStop} title="Stop this subagent: the conversation and its other tasks go on"
          className="flex flex-none items-center gap-1.5 rounded-md border border-line px-2 py-1 text-[12px] text-ink-2"><Square size={9} strokeWidth={2} fill="currentColor" />Stop</button>}
        <button onClick={onClose} title="Close" aria-label="Close" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
      </div>
      {!view && <div className="px-4 pt-4 text-[12.5px] text-faint">Reading what it has done so far…</div>}
      {view && messages.length === 0 && <div className="px-4 pt-4 text-[12.5px] text-faint">Nothing from it yet.</div>}
      {/* it works until its task ends: the transcript's own "working" line under what it has said so far */}
      <Transcript messages={messages} busy={running} activity={running ? "Tool" : undefined} jumps={false} />
    </aside>
  );
}
