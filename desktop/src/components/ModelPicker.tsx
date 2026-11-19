import { Check, Search } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type RefObject } from "react";
import type { ModelChoice, Models } from "../lib/tazuna";
import { modelLabel } from "../lib/types";

// The model picker over the composer: the terminal's /model catalog, grouped by provider. With nothing typed it lists the
// models usable now; typing searches all of them, and a row that needs a key or trust says what to do.
/** Where a popover on the composer opens: above the whole message box where it fits, else below it, as tall as the page
 * around it has room for. Every menu there uses it: the start page's box sits high, and the model picker (desktop 0.1.73)
 * and then the permission menu were cut off at the top; above a conversation's box a menu covered its text. */
export function usePlace(box: RefObject<HTMLElement | null>, want = 420): { style: CSSProperties; side: string } {
  const [p, setP] = useState<{ down: boolean; lift: number; max: number } | null>(null);
  useLayoutEffect(() => {
    const el = box.current, anchor = el?.parentElement;
    if (!el || !anchor) return;
    const card = anchor.closest("[data-composer]") ?? anchor;
    let clip: Element | null = card.parentElement;
    while (clip && clip !== document.body && !/auto|scroll|hidden/.test(getComputedStyle(clip).overflowY)) clip = clip.parentElement;
    const a = anchor.getBoundingClientRect(), b = card.getBoundingClientRect(), v = (clip ?? document.body).getBoundingClientRect();
    const above = b.top - v.top - 12, below = v.bottom - b.bottom - 12, need = Math.min(want, el.scrollHeight);
    const down = above < need && below > above;
    setP({ down, lift: Math.max(0, down ? b.bottom - a.bottom : a.top - b.top), max: Math.max(120, Math.min(want, down ? below : above)) });
    // closed: focus back on the button that opened it (it fell to the page)
    // (after the commit, and only if it is really gone: a development remount ran this and took the focus from its search)
    return () => queueMicrotask(() => {
      if (!el.isConnected && (!document.activeElement || document.activeElement === document.body)) anchor.querySelector("button")?.focus();
    });
  }, [box, want]);
  // measured before it shows: transparent, not hidden, so its search field can take the focus (hidden, it could not, and
  // typing after a click went nowhere, design review 5)
  if (!p) return { style: { opacity: 0 }, side: "bottom-full" };
  return p.down ? { style: { marginTop: p.lift + 8, maxHeight: p.max }, side: "top-full origin-top" } : { style: { marginBottom: p.lift + 8, maxHeight: p.max }, side: "bottom-full origin-bottom" };
}

export function ModelPicker({ models, onPick, onClose, efforts = [], effort, onEffort }: { models: Models | null; onPick: (c: ModelChoice) => void; onClose: () => void;
  /** the current model's effort levels, chosen at the top (one pill for model and effort since desktop 0.1.70, as Claude,
   * MonoCode and DeepSeek Harness have it) */
  efforts?: string[]; effort?: string; onEffort?: (e: string) => void }) {
  const [q, setQ] = useState("");
  const box = useRef<HTMLDivElement>(null);
  const place = usePlace(box);
  useEffect(() => {
    const away = (e: MouseEvent) => { if (box.current && !box.current.contains(e.target as Node)) onClose(); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc);
    return () => { document.removeEventListener("mousedown", away); document.removeEventListener("keydown", esc); };
  }, [onClose]);
  const { groups, hidden } = useMemo(() => {
    const all = models?.choices ?? [], f = q.trim().toLowerCase();
    const rows = f ? all.filter((c) => `${c.label} ${modelLabel(c.model)} ${c.place}`.toLowerCase().includes(f)) : all.filter((c) => !c.why_not);
    const groups = new Map<string, ModelChoice[]>();
    for (const c of rows.slice(0, 300)) groups.set(c.provider, [...(groups.get(c.provider) ?? []), c]);
    return { groups: [...groups], hidden: f ? Math.max(0, rows.length - 300) : all.length - rows.length };
  }, [models, q]);
  return (
    <div ref={box} style={place.style} className={`tz-pop absolute ${place.side} left-0 z-10 flex w-[460px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer`}>
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
      <div className="overflow-auto pb-1">
        {/* what the numbers on the right are (until desktop 0.1.74 only a hover said: "8.4 (12)", "0.14 / 0.56") */}
        {groups.length > 0 && <div className="sticky top-0 z-[1] flex items-center gap-2 bg-card px-3 pb-0.5 pt-1.5 text-[11px] text-faint">
          <span className="flex-1" /><span className="w-[64px] text-right" title="tazuna bench: judge score out of 10 (runs)">Score</span>
          <span className="w-[112px] text-right" title="dollars per million tokens: input / output">$ per M, in / out</span></div>}
        {!models && <div className="px-3 py-2 text-[12.5px] text-faint">Loading…</div>}
        {groups.map(([provider, rows]) => (
          <div key={provider}>
            <div className="px-3 pb-0.5 pt-2 text-[11.5px] font-medium text-faint">{provider}</div>
            {rows.map((c) => (
              <button key={`${c.model}|${c.url}|${c.key_env}|${c.label}`} disabled={!!c.why_not} onClick={() => onPick(c)} title={c.why_not ?? c.place}
                className="flex w-full items-center gap-2 px-3 py-[5px] text-left hover:bg-hover disabled:cursor-default disabled:hover:bg-transparent">
                <span className="w-3.5 flex-none text-accent">{c.current && <Check size={14} strokeWidth={1.75} />}</span>
                <span className="min-w-0 flex-1">
                  {/* its name, not its id ("GPT-6.2 Sol", not "gpt-6.2-sol · subrouter"); the group names the provider */}
                  <span className={`block truncate ${c.why_not ? "text-faint" : "text-ink"}`} title={c.model}>{modelLabel(c.model)}</span>
                  {/* the group names the provider: a row's second line only says why it cannot run (it repeated "api.mistral.ai") */}
                  {c.why_not && <span className="block truncate text-[11.5px] text-faint">{c.why_not}</span>}
                </span>
                <span className="w-[64px] flex-none whitespace-nowrap text-right text-[11.5px] tabular-nums text-faint" title="tazuna bench: judge score (runs)">{c.bench}</span>
                <span className="w-[112px] flex-none whitespace-nowrap text-right text-[11.5px] tabular-nums text-muted" title="$ per million tokens: input / output">{c.price === "plan" ? "on your plan" : c.price}</span>
              </button>
            ))}
          </div>
        ))}
        {hidden > 0 && <div className="px-3 py-2 text-[11.5px] text-faint">{q ? `${hidden} more: narrow the search` : `${hidden} more need a key or trust: type to search them`}</div>}
      </div>
    </div>
  );
}

/** A small menu on the composer that closes on a click outside or Esc, placed as the model picker is (usePlace). The
 * keyboard moves through it: focus on the checked item (or the first) when it opens, arrows and Home/End between items,
 * Tab closes it (until desktop 0.1.77 focus stayed on the button and the arrows did nothing). */
export function Menu({ children, onClose, width = 240 }: { children: React.ReactNode; onClose: () => void; width?: number }) {
  const box = useRef<HTMLDivElement>(null);
  const place = usePlace(box);
  const items = () => [...(box.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
  useEffect(() => { const all = items(); (all.find((b) => b.querySelector("svg.lucide-check")) ?? all[0])?.focus(); }, []);
  const keys = (e: React.KeyboardEvent) => {
    const all = items(), i = all.indexOf(document.activeElement as HTMLButtonElement);
    const go = (n: number) => { e.preventDefault(); all[(n + all.length) % all.length]?.focus(); };
    if (e.key === "ArrowDown") go(i + 1); else if (e.key === "ArrowUp") go(i - 1); else if (e.key === "Home") go(0); else if (e.key === "End") go(all.length - 1);
    else if (e.key === "Tab") onClose();
  };
  useEffect(() => {
    const away = (e: MouseEvent) => { if (box.current && !box.current.contains(e.target as Node)) onClose(); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc);
    return () => { document.removeEventListener("mousedown", away); document.removeEventListener("keydown", esc); };
  }, [onClose]);
  return <div ref={box} role="menu" onKeyDown={keys} style={{ width, ...place.style }} className={`tz-pop absolute ${place.side} left-0 z-10 overflow-y-auto rounded-xl border border-line-strong bg-card py-1 shadow-composer [&_button]:outline-offset-[-2px] [&_button:focus-visible]:bg-hover`}>{children}</div>;
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
