import { Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

/** One thing the palette can do: shown under its group, found by its label and `keywords`. */
export type Action = { id: string; group: string; label: string; hint?: string; keys?: string; keywords?: string; run: () => void };

// Cmd-K: every action of the window in one searchable list (new chat, folders, conversations, model, effort, mode, undo,
// theme). Arrows move, Enter runs, Esc closes. With nothing typed it shows the first few of each group.
export function Palette({ actions, onClose }: { actions: Action[]; onClose: () => void }) {
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
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
    <div onMouseDown={onClose} className="fixed inset-0 z-30 flex justify-center bg-ink/15 pt-[12vh]">
      <div onMouseDown={(e) => e.stopPropagation()} className="flex max-h-[60vh] w-[560px] flex-col self-start overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
        <div className="flex items-center gap-2 border-b border-line-soft px-3.5 py-2.5 text-muted">
          <Search size={15} strokeWidth={1.5} />
          <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder="Type a command, a conversation or a model"
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") { e.preventDefault(); setSel((s) => Math.min(s + 1, rows.length - 1)); }
              else if (e.key === "ArrowUp") { e.preventDefault(); setSel((s) => Math.max(s - 1, 0)); }
              else if (e.key === "Enter") { e.preventDefault(); run(rows[sel]); }
              else if (e.key === "Escape") { e.preventDefault(); onClose(); }
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
                <span className="min-w-0 flex-1 truncate">{a.label}{a.hint && <span className="ml-2 text-[12px] text-faint">{a.hint}</span>}</span>
                {a.keys && <kbd className="rounded-[5px] border border-line px-[5px] py-px font-mono text-[11px] text-faint">{a.keys}</kbd>}
              </button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

const SHORTCUTS: [string, string][] = [
  ["⌘K", "Command palette"], ["⌘N", "New chat"], ["⌘O", "Open a folder"], ["⌘/", "These shortcuts"], ["⌘,", "Settings"],
  ["Enter", "Send"], ["Shift Enter", "New line"], ["Esc", "Stop the reply"], ["Shift Tab", "Next permission mode"],
  ["y / a / n", "Allow, always, decline a question"], ["⌘⇧L", "Light or dark"],
];

/** Cmd-/: the keyboard shortcuts. */
export function Shortcuts({ onClose }: { onClose: () => void }) {
  useEffect(() => {
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onClose]);
  return (
    <div onMouseDown={onClose} className="fixed inset-0 z-30 grid place-items-center bg-ink/15">
      <div onMouseDown={(e) => e.stopPropagation()} className="w-[380px] rounded-xl border border-line-strong bg-card px-4 py-3 shadow-composer">
        <div className="mb-2 text-[11.5px] font-medium text-faint">Keyboard shortcuts</div>
        {SHORTCUTS.map(([k, what]) => (
          <div key={k} className="flex items-center py-[5px] text-ink-2">
            <span className="flex-1">{what}</span>
            <kbd className="rounded-[5px] border border-line px-[6px] py-px font-mono text-[11.5px] text-muted">{k}</kbd>
          </div>
        ))}
      </div>
    </div>
  );
}
