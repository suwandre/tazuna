import { Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useDialog } from "../lib/dialog";
import { keysOf } from "../lib/types";
import { dialogShell } from "./Controls";

/** One thing the palette can do: shown under its group, found by its label and `keywords`. */
export type Action = { id: string; group: string; label: string; hint?: string; keys?: string; keywords?: string; unavailable?: string; run: () => void };

// Cmd-K: every action of the window in one searchable list (new chat, folders, conversations, model, effort, mode, undo,
// theme). Arrows move, Enter runs, Esc closes. With nothing typed it shows the first few of each group.
/** The typed words marked in a row's label, so it shows why the row matched. */
function marked(label: string, q: string) {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return label;
  const re = new RegExp(`(${words.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})`, "ig");
  return label.split(re).map((p, i) => (i % 2 ? <mark key={i} className="rounded-[2px] bg-accent-soft text-ink">{p}</mark> : p));
}

/** The rows for what was typed: every word somewhere in the row (its group, label, hint or keywords), the label's matches
 * first (a label that starts with a word, then a word of it that does, then one that holds it), then those found only in
 * the hint or keywords; each group where its best row falls, in its own order. "set" listed Auto (its hint says "reset")
 * above Settings (design review 14). */
export function ranked(actions: Action[], q: string): Action[] {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean);
  const hits = actions.filter((a) => { const t = `${a.group} ${a.label} ${a.hint ?? ""} ${a.keywords ?? ""}`.toLowerCase(); return words.every((w) => t.includes(w)); });
  const rank = (a: Action) => Math.max(...words.map((w) => { const l = a.label.toLowerCase(), i = l.indexOf(w); return i === 0 ? 0 : new RegExp(`\\b${w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`).test(l) ? 1 : i > 0 ? 2 : 3; }));
  const r = new Map(hits.map((a) => [a, rank(a)])), best = new Map<string, number>();
  for (const a of hits) best.set(a.group, Math.min(best.get(a.group) ?? 9, r.get(a)!));
  const groups = [...best.keys()];
  return hits.map((a, i) => [a, i] as const).sort(([a, i], [b, j]) => best.get(a.group)! - best.get(b.group)! || groups.indexOf(a.group) - groups.indexOf(b.group) || r.get(a)! - r.get(b)! || i - j).map(([a]) => a);
}
export function demo() {
  const run = () => {};
  const a: Action[] = [{ id: "auto", group: "Permission mode", label: "Auto", hint: "asks before rm, reset --hard", run }, { id: "settings", group: "Window", label: "Settings: provider keys", run },
    { id: "theme", group: "Window", label: "Light or dark", run }, { id: "up", group: "Window", label: "Check for updates", keywords: "upgrade version", run }, { id: "asset", group: "Projects", label: "my-assets", run }];
  const got = (q: string) => ranked(a, q).map((x) => x.id).join(",");
  if (got("set") !== "settings,asset,auto" || got("up") !== "up" || got("version") !== "up" || got("") !== "auto,settings,theme,up,asset") throw new Error(`palette: ${got("set")} | ${got("")}`);
  console.log("palette ok");
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
    return ranked(actions, q).slice(0, 60);
  }, [actions, q]);
  useEffect(() => setSel(0), [q]);
  useEffect(() => { list.current?.querySelector(`[data-i="${sel}"]`)?.scrollIntoView({ block: "nearest" }); }, [sel]);
  const run = (a?: Action) => { if (!a || a.unavailable) return; onClose(); a.run(); };
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-30 flex justify-center bg-scrim pt-[12vh]">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Commands" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => { if (e.key === "Escape") { e.preventDefault(); onClose(); } }} className="tz-dialog flex max-h-[60vh] w-[560px] max-w-[calc(100vw-5rem)] flex-col self-start overflow-hidden rounded-xl border border-line-strong bg-main">
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
              <button data-i={i} disabled={!!a.unavailable} title={a.unavailable} onMouseMove={() => setSel(i)} onClick={() => run(a)}
                className={`flex w-full items-center gap-3 px-3.5 py-[6px] text-left disabled:cursor-default disabled:text-muted ${i === sel ? "bg-selected text-ink" : "text-ink-2"}`}>
                <span className="min-w-0 flex-1 truncate">{marked(a.label, q)}{(a.unavailable || a.hint) && <span className="ml-2 text-[12px] text-faint">{a.unavailable || a.hint}</span>}</span>
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
export function Shortcuts({ onClose, readOnly = false }: { onClose: () => void; readOnly?: boolean }) {
  const dialog = useDialog();
  useEffect(() => {
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onClose]);
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-30 grid place-items-center bg-scrim">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Keyboard shortcuts" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className={`${dialogShell(420)} overflow-y-auto! px-4 py-3`}>
        <div className="mb-2 text-[11.5px] font-medium text-faint">Keyboard shortcuts</div>
        {readOnly && <p className="mb-2 text-[12px] text-muted">Read-only demo · shortcuts for this preview</p>}
        {(readOnly ? SHORTCUTS.filter(([k]) => ["⌘K", "⌘/", "⇧Enter", "⌘\\", "⌘⇧L"].includes(k)) : SHORTCUTS).map(([k, what]) => (
          <div key={k} className="flex items-center py-[5px] text-ink-2">
            <span className="flex-1">{what}</span>
            <kbd className="rounded-[5px] border border-line px-[6px] py-px font-mono text-[11.5px] text-muted">{keysOf(k)}</kbd>
          </div>
        ))}
      </div>
    </div>
  );
}
