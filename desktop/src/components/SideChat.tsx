import { ArrowUp, Square, X } from "lucide-react";
import { useEffect, useState } from "react";
import { messages as viewMessages } from "../lib/fromView";
import type { View } from "../lib/tazuna";
import { Approval } from "./Approval";
import { Transcript } from "./Transcript";

/** A side chat beside the conversation (docs/DESKTOP.md 0.2): a fork of it on the same model, so it knows everything said
 * so far, in plan mode, so it changes no file; nothing in it goes back to the conversation. */
export function SideChat({ view, onSend, onStop, onAnswer, onClose }: {
  view: View | null; onSend: (text: string) => void; onStop: () => void; onAnswer: (allow: boolean, always: boolean) => void; onClose: () => void;
}) {
  const [text, setText] = useState("");
  const busy = !!view && !["Ready", "Done"].includes(view.activity);
  // nothing goes to it until its core says plan mode (it set that with the fork; a failed fork is not sent to)
  const ready = view?.mode === "plan";
  const messages = view ? viewMessages(view) : [];
  const send = () => { const t = text.trim(); if (t && ready) { onSend(t); setText(""); } };
  useEffect(() => {
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape" && !busy && (e.target as HTMLElement)?.closest?.("[data-side]")) onClose(); };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [busy, onClose]);
  return (
    <aside data-side className="flex min-h-0 flex-col border-l border-line bg-sidebar">
      <div className="flex items-start gap-2 border-b border-line-soft px-4 py-3">
        <span className="flex-1"><span className="font-medium">Side chat</span>
          <span className="block text-[11.5px] text-muted">Knows this conversation. Nothing here goes back to it, and it changes no files.</span></span>
        <button onClick={onClose} title="Close the side chat" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
      </div>
      {messages.length === 0 && !busy && <div className="px-4 pt-4 text-[12.5px] text-faint">{!view ? "Starting…" : ready ? "Ask about the conversation: what it decided and why, what is left, what a step did." : "Not in plan mode, so nothing is sent."}</div>}
      <Transcript messages={messages} busy={busy} activity={view?.asking ? undefined : view?.activity} jumps={false} />
      {view?.asking && <Approval asking={view.asking} onAnswer={onAnswer} keys={false} />}
      <div className="px-3 pb-3">
        <div className="flex items-end gap-2 rounded-xl border border-line-strong bg-card py-2 pl-3 pr-2">
          <textarea rows={2} autoFocus value={text} onChange={(e) => setText(e.target.value)} placeholder="Ask on the side…"
            onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) { e.preventDefault(); send(); } }}
            className="min-w-0 flex-1 resize-none bg-transparent text-ink outline-none placeholder:text-faint" />
          {busy && !text.trim()
            ? <button onClick={onStop} title="Stop" className="grid size-[28px] flex-none place-items-center rounded-full bg-ink text-canvas"><Square size={11} strokeWidth={2} fill="currentColor" /></button>
            : <button onClick={send} disabled={!text.trim() || !ready} title="Send (Enter)" className="grid size-[28px] flex-none place-items-center rounded-full bg-accent text-on-accent disabled:opacity-40"><ArrowUp size={15} strokeWidth={1.75} /></button>}
        </div>
      </div>
    </aside>
  );
}
