import { Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useDialog } from "../lib/dialog";
import { keysOf } from "../lib/types";

/** One thing the palette can do: shown under its group, found by its label and `keywords`. */
export type Action = { id: string; group: string; label: string; hint?: string; keys?: string; keywords?: string; run: () => void };

// Cmd-K: every action of the window in one searchable list (new chat, folders, conversations, model, effort, mode, undo,
// theme). Arrows move, Enter runs, Esc closes. With nothing typed it shows the first few of each group.
/** The typed words marked in a row's label, so it shows why the row matched. */
function marked(label: string, q: string) {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return label;
  const re = new RegExp(`(${words.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})`, "ig");
  return label.split(re).map((p, i) => (i % 2 ? <mark key={i} className="rounded-[2px] bg-accent-soft text-ink">{p}</mark> : p));
}

export function Palette({ actions, onClose, query = "" }: { actions: Action[]; onClose: () => void; query?: string }) {
  const [q, setQ] = useState(query);
  const [sel, setSel] = useState(0);
  const dialog = useDialog();
  const list = useRef<HTMLDivElement>(null);
  const rows = useMemo(() => {
    const words = q.toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) {
      const seen = new Map<string, number>();
      return actions.filter((a) => { const n = seen.get(a.group) ?? 0; seen.set(a.group, n + 1); return n < 5; });
    }
    return actions.filter((a) => { const t = `${a.group} ${a.label} ${a.hint ?? ""} ${a.keywords ?? ""}`.toLowerCase(); return words.every((w) => t.includes(w)); }).slice(0, 60);
  }, [actions, q]);
  useEffect(() => setSel(0), [q]);
  useEffect(() => { list.current?.querySelector(`[data-i="${sel}"]`)?.scrollIntoView({ block: "nearest" }); }, [sel]);
  const run = (a?: Action) => { if (!a) return; onClose(); a.run(); };
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-30 flex justify-center bg-scrim pt-[12vh]">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Commands" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => { if (e.key === "Escape") { e.preventDefault(); onClose(); } }} className="tz-dialog flex max-h-[60vh] w-[560px] max-w-[calc(100vw-5rem)] flex-col self-start overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
        <div className="flex items-center gap-2 border-b border-line-soft px-3.5 py-2.5 text-muted">
          <Search size={15} strokeWidth={1.5} />
          <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder="Type a command, a conversation or a model"
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") { e.preventDefault(); setSel((s) => Math.min(s + 1, rows.length - 1)); }
              else if (e.key === "ArrowUp") { e.preventDefault(); setSel((s) => Math.max(s - 1, 0)); }
              else if (e.key === "Enter") { e.preventDefault(); run(rows[sel]); }
            }}
            className="flex-1 bg-transparent text-[14px] text-ink outline-none placeholder:text-faint" />
        </div>
        <div ref={list} className="overflow-auto py-1">
          {rows.length === 0 && <div className="px-3.5 py-3 text-[12.5px] text-faint">Nothing matches</div>}
          {rows.map((a, i) => (
            <div key={a.id}>
              {(i === 0 || rows[i - 1].group !== a.group) && <div className="px-3.5 pb-0.5 pt-2 text-[11.5px] font-medium text-faint">{a.group}</div>}
              <button data-i={i} onMouseMove={() => setSel(i)} onClick={() => run(a)}
                className={`flex w-full items-center gap-3 px-3.5 py-[6px] text-left ${i === sel ? "bg-selected text-ink" : "text-ink-2"}`}>
                <span className="min-w-0 flex-1 truncate">{marked(a.label, q)}{a.hint && <span className="ml-2 text-[12px] text-faint">{a.hint}</span>}</span>
                {a.keys && <kbd className="rounded-[5px] border border-line px-[5px] py-px font-mono text-[11px] text-faint">{keysOf(a.keys)}</kbd>}
              </button>
            </div>
          ))}
        </div>
        <div className="flex gap-3.5 border-t border-line-soft px-3.5 py-1.5 text-[11.5px] text-faint">
          <span><kbd className="font-mono">↑↓</kbd> move</span><span><kbd className="font-mono">↵</kbd> run</span><span><kbd className="font-mono">esc</kbd> close</span>
        </div>
      </div>
    </div>
  );
}

const SHORTCUTS: [string, string][] = [
  ["⌘K", "Command palette"], ["⌘N", "New chat"], ["⌘O", "Open a folder"], ["⌘/", "These shortcuts"], ["⌘,", "Settings"],
  ["Enter", "Send"], ["⇧Enter", "New line"], ["Esc", "Stop the reply"], ["⇧Tab", "Next permission mode"],
  ["y / a / n", "Allow, always, decline a question"], ["⌘\\", "The work panel"], ["⌘⇧L", "Light or dark"], ["⌘⇧U", "Usage"],
];

/** Cmd-/: the keyboard shortcuts. */
export function Shortcuts({ onClose }: { onClose: () => void }) {
  const dialog = useDialog();
  useEffect(() => {
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onClose]);
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-30 grid place-items-center bg-scrim">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Keyboard shortcuts" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className="tz-dialog w-[380px] max-w-[calc(100vw-5rem)] rounded-xl border border-line-strong bg-card px-4 py-3 shadow-composer">
        <div className="mb-2 text-[11.5px] font-medium text-faint">Keyboard shortcuts</div>
        {SHORTCUTS.map(([k, what]) => (
          <div key={k} className="flex items-center py-[5px] text-ink-2">
            <span className="flex-1">{what}</span>
            <kbd className="rounded-[5px] border border-line px-[6px] py-px font-mono text-[11.5px] text-muted">{keysOf(k)}</kbd>
          </div>
        ))}
      </div>
    </div>
  );
}
