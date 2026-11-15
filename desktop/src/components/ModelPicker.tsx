import { Check, Search } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ModelChoice, Models } from "../lib/tazuna";

// The model picker over the composer: the terminal's /model catalog, grouped by provider. With nothing typed it lists the
// models usable now; typing searches all of them, and a row that needs a key or trust says what to do.
export function ModelPicker({ models, onPick, onClose }: { models: Models | null; onPick: (c: ModelChoice) => void; onClose: () => void }) {
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
    <div ref={box} className="absolute bottom-full left-0 z-10 mb-2 flex max-h-[420px] w-[460px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
      <div className="flex items-center gap-2 border-b border-line-soft px-3 py-2 text-muted">
        <Search size={14} strokeWidth={1.5} />
        <input autoFocus value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search models and providers" className="flex-1 bg-transparent text-ink outline-none placeholder:text-faint" />
      </div>
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

/** The current model's effort levels, as a small menu. */
export function EffortMenu({ efforts, current, onPick, onClose }: { efforts: string[]; current: string; onPick: (e: string) => void; onClose: () => void }) {
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const away = (e: MouseEvent) => { if (box.current && !box.current.contains(e.target as Node)) onClose(); };
    document.addEventListener("mousedown", away);
    return () => document.removeEventListener("mousedown", away);
  }, [onClose]);
  return (
    <div ref={box} className="absolute bottom-full left-0 z-10 mb-2 w-[160px] rounded-xl border border-line-strong bg-card py-1 shadow-composer">
      {efforts.map((e) => (
        <button key={e} onClick={() => onPick(e)} className="flex w-full items-center gap-2 px-3 py-[5px] text-left hover:bg-hover">
          <span className="w-3.5 text-accent">{e === current && <Check size={14} strokeWidth={1.75} />}</span>{e}
        </button>
      ))}
    </div>
  );
}
