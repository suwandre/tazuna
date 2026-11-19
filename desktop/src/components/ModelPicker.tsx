import { Check, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ModelChoice, Models } from "../lib/tazuna";

// The model picker over the composer: the terminal's /model catalog, grouped by provider. With nothing typed it lists the
// models usable now; typing searches all of them, and a row that needs a key or trust says what to do.
export function ModelPicker({ models, onPick, onClose, efforts = [], effort, onEffort, lift = 0, down, max = 420 }: { models: Models | null; onPick: (c: ModelChoice) => void; onClose: () => void;
  /** the current model's effort levels, chosen at the top (one pill for model and effort since desktop 0.1.70, as Claude,
   * MonoCode and DeepSeek Harness have it) */
  efforts?: string[]; effort?: string; onEffort?: (e: string) => void;
  /** px from the pill's edge to the composer's: the picker opens above (or, `down`, below) the whole box, not over its
   * text; `max` is its height where the page has less room than 420 px */
  lift?: number; down?: boolean; max?: number }) {
  const [q, setQ] = useState("");
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const away = (e: MouseEvent) => { if (box.current && !box.current.contains(e.target as Node)) onClose(); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc);
    return () => { document.removeEventListener("mousedown", away); document.removeEventListener("keydown", esc); };
  }, [onClose]);
  const { groups, hidden } = useMemo(() => {
    const all = models?.choices ?? [], f = q.trim().toLowerCase();
    const rows = f ? all.filter((c) => `${c.label} ${c.place}`.toLowerCase().includes(f)) : all.filter((c) => !c.why_not);
    const groups = new Map<string, ModelChoice[]>();
    for (const c of rows.slice(0, 300)) groups.set(c.provider, [...(groups.get(c.provider) ?? []), c]);
    return { groups: [...groups], hidden: f ? Math.max(0, rows.length - 300) : all.length - rows.length };
  }, [models, q]);
  return (
    <div ref={box} style={down ? { marginTop: lift + 8, maxHeight: max } : { marginBottom: lift + 8, maxHeight: max }} className={`absolute ${down ? "top-full" : "bottom-full"} left-0 z-10 flex w-[460px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer`}>
      <div className="flex items-center gap-2 border-b border-line-soft px-3 py-2 text-muted">
        <Search size={14} strokeWidth={1.5} />
        <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search models and providers" className="flex-1 bg-transparent text-ink outline-none placeholder:text-faint" />
      </div>
      {efforts.length > 1 && onEffort && (
        <div className="flex items-center gap-2 border-b border-line-soft px-3 py-2 text-[12px]">
          <span className="text-faint">Effort</span>
          <span role="radiogroup" aria-label="Effort" className="flex flex-wrap rounded-md border border-line p-0.5">
            {efforts.map((e) => (
              <button key={e} role="radio" aria-checked={e === effort} onClick={() => onEffort(e)}
                className={`rounded px-2 py-0.5 ${e === effort ? "bg-selected text-ink" : "text-muted hover:text-ink"}`}>{e}</button>
            ))}
          </span>
        </div>
      )}
      {models?.rebuild && <div className="border-b border-line-soft px-3 py-1.5 text-[12px] text-warn">Switching between Claude models: {models.rebuild}</div>}
      <div className="overflow-auto py-1">
        {!models && <div className="px-3 py-2 text-[12.5px] text-faint">Loading…</div>}
        {groups.map(([provider, rows]) => (
          <div key={provider}>
            <div className="px-3 pb-0.5 pt-2 text-[11.5px] font-medium text-faint">{provider}</div>
            {rows.map((c) => (
              <button key={`${c.model}|${c.url}|${c.key_env}|${c.label}`} disabled={!!c.why_not} onClick={() => onPick(c)} title={c.why_not ?? c.place}
                className="flex w-full items-center gap-2 px-3 py-[5px] text-left hover:bg-hover disabled:cursor-default disabled:hover:bg-transparent">
                <span className="w-3.5 flex-none text-accent">{c.current && <Check size={14} strokeWidth={1.75} />}</span>
                <span className="min-w-0 flex-1">
                  <span className={`block truncate ${c.why_not ? "text-faint" : "text-ink"}`}>{c.label}</span>
                  <span className="block truncate text-[11.5px] text-faint">{c.why_not ?? c.place}</span>
                </span>
                {c.bench && <span className="flex-none whitespace-nowrap font-mono text-[11px] text-faint" title="tazuna bench: judge score (runs)">{c.bench}</span>}
                <span className="w-[112px] flex-none whitespace-nowrap text-right font-mono text-[11px] text-muted" title="$ per million tokens: input / output">{c.price}</span>
              </button>
            ))}
          </div>
        ))}
        {hidden > 0 && <div className="px-3 py-2 text-[11.5px] text-faint">{q ? `${hidden} more: narrow the search` : `${hidden} more need a key or trust: type to search them`}</div>}
      </div>
    </div>
  );
}

/** A small menu over the composer that closes on a click outside or Esc. */
export function Menu({ children, onClose, width = 240, down }: { children: React.ReactNode; onClose: () => void; width?: number; down?: boolean }) {
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const away = (e: MouseEvent) => { if (box.current && !box.current.contains(e.target as Node)) onClose(); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc);
    return () => { document.removeEventListener("mousedown", away); document.removeEventListener("keydown", esc); };
  }, [onClose]);
  return <div ref={box} style={{ width }} className={`absolute left-0 z-10 rounded-xl border border-line-strong bg-card py-1 shadow-composer ${down ? "top-full mt-2" : "bottom-full mb-2"}`}>{children}</div>;
}

/** The current model's effort levels. */
export function EffortMenu({ efforts, current, onPick, onClose }: { efforts: string[]; current: string; onPick: (e: string) => void; onClose: () => void }) {
  return (
    <Menu onClose={onClose} width={160}>
      {efforts.map((e) => (
        <button key={e} onClick={() => onPick(e)} className="flex w-full items-center gap-2 px-3 py-[5px] text-left hover:bg-hover">
          <span className="w-3.5 text-accent">{e === current && <Check size={14} strokeWidth={1.75} />}</span>{e}
        </button>
      ))}
    </Menu>
  );
}
