import { Check, Search } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type RefObject } from "react";
import type { ModelChoice, Models } from "../lib/tazuna";
import { matchesWords, modelLabel } from "../lib/types";
import { Segmented } from "./Controls";

// The model picker over the composer: the terminal's /model catalog, grouped by provider. With nothing typed it lists the
// models usable now; typing searches all of them, and a row that needs a key or trust says what to do.
/** Where a popover on the composer opens: above the whole message box where it fits, else below it, as tall as the page
 * around it has room for. Every menu there uses it: the start page's box sits high, and the model picker (desktop 0.1.73)
 * and then the permission menu were cut off at the top; above a conversation's box a menu covered its text. */
export function usePlace(box: RefObject<HTMLElement | null>, want = 420, width?: number): { style: CSSProperties; side: string } {
  const [p, setP] = useState<{ down: boolean; lift: number; max: number; horizontal?: CSSProperties } | null>(null);
  useLayoutEffect(() => {
    const el = box.current, anchor = el?.parentElement;
    if (!el || !anchor) return;
    const card = anchor.closest("[data-composer]") ?? anchor;
    let clip: Element | null = card.parentElement;
    while (clip && clip !== document.body && !/auto|scroll|hidden/.test(getComputedStyle(clip).overflowY)) clip = clip.parentElement;
    const boundary = clip ?? document.body;
    const measure = () => {
      const a = anchor.getBoundingClientRect(), b = card.getBoundingClientRect(), v = boundary.getBoundingClientRect();
      const above = b.top - v.top - 12, below = v.bottom - b.bottom - 12, need = Math.min(want, el.scrollHeight);
      const down = above < need && below > above;
      let horizontal: CSSProperties | undefined;
      if (width != null) {
        const left = Math.max(v.left, 0) + 12, right = Math.min(v.right, window.innerWidth) - 12;
        const fit = Math.min(width, Math.max(0, right - left));
        horizontal = { width: fit, marginLeft: Math.max(left, Math.min(a.left, right - fit)) - a.left };
      }
      setP({ down, lift: Math.max(0, down ? b.bottom - a.bottom : a.top - b.top), max: Math.max(120, Math.min(want, down ? below : above)), horizontal });
    };
    measure();
    const resized = width == null ? undefined : new ResizeObserver(measure);
    resized?.observe(boundary);
    if (resized) window.addEventListener("resize", measure);
    // closed: focus back on the button that opened it (it fell to the page)
    // (after the commit, and only if it is really gone: a development remount ran this and took the focus from its search)
    return () => {
      resized?.disconnect();
      if (resized) window.removeEventListener("resize", measure);
      queueMicrotask(() => {
        if (!el.isConnected && (!document.activeElement || document.activeElement === document.body)) anchor.querySelector("button")?.focus();
      });
    };
  }, [box, want, width]);
  // measured before it shows: transparent, not hidden, so its search field can take the focus (hidden, it could not, and
  // typing after a click went nowhere, design review 5)
  if (!p) return { style: { opacity: 0, width }, side: "bottom-full" };
  const style = { maxHeight: p.max, ...p.horizontal };
  return p.down ? { style: { ...style, marginTop: p.lift + 8 }, side: "top-full origin-top" } : { style: { ...style, marginBottom: p.lift + 8 }, side: "bottom-full origin-bottom" };
}

/** "$ per M in / out" said evenly: "0.021 / 0.064" for "0.0213 / 0.0639", "1.4 / 4.4", "free" for "0 / 0". */
export function price(p: string): string {
  const n = p.split("/").map((x) => Number(x.trim()));
  if (n.length !== 2 || n.some(Number.isNaN)) return p;
  if (n.every((x) => x === 0)) return "free";
  return n.map((x) => (x === 0 ? "0" : x >= 1 ? String(+x.toFixed(2)) : String(+x.toPrecision(2)))).join(" / ");
}

/** What a row that cannot be picked needs, by the core's reason: a key, or a sign-in's state (core oauth.rs). */
const NO_KEY: Record<string, string> = { "no key": "No key yet · add one in Settings", "not signed in": "Not signed in · sign in in Settings",
  "plan use not allowed": "Plan use not allowed · allow it in Settings", "model list not loaded": "Its model list has not loaded yet",
  "which key": "Same key as Mistral or Mistral Vibe · say which in Settings" };

const CATALOG_STATUS = {
  loading: { label: "Loading model list", detail: "A key is configured. Models will appear when the list is ready.", tone: "bg-accent" },
  ready: { label: "Models ready", detail: "", tone: "bg-ok" },
  unavailable: { label: "Model list unavailable", detail: "A key is configured, but the list couldn’t load.", tone: "bg-warn" },
};

export function ModelPicker({ models, onPick, onClose, onKey, onRefreshModelList, efforts = [], effort, onEffort }: { models: Models | null; onPick: (c: ModelChoice) => void; onClose: () => void;
  /** a row whose provider has no key: Settings on that provider, its key field open */
  onKey?: (provider: string) => void;
  onRefreshModelList?: (provider: string) => void;
  /** the current model's effort levels, chosen at the top (one pill for model and effort since desktop 0.1.70, as Claude,
   * MonoCode and DeepSeek Harness have it) */
  efforts?: string[]; effort?: string; onEffort?: (e: string) => void }) {
  const [q, setQ] = useState("");
  const [seenPending, setSeenPending] = useState<Set<string>>(() => new Set());
  const box = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const place = usePlace(box, 420, 460);
  useEffect(() => {
    const away = (e: MouseEvent) => { if (box.current && !box.current.contains(e.target as Node)) onClose(); };
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc);
    return () => { document.removeEventListener("mousedown", away); document.removeEventListener("keydown", esc); };
  }, [onClose]);
  useEffect(() => {
    const pending = models?.catalogs?.filter(({ state }) => state !== "ready").map(({ provider }) => provider);
    if (pending?.length) setSeenPending((seen) => pending.every((p) => seen.has(p)) ? seen : new Set([...seen, ...pending]));
  }, [models?.catalogs]);
  const { groups, hidden } = useMemo(() => {
    const all = models?.choices ?? [], f = q.trim().toLowerCase();
    const rows = f ? all.filter((c) => matchesWords(f, [modelLabel(c.model), c.label, c.model, c.place, c.provider].join(" "))) : all.filter((c) => !c.why_not);
    const groups = new Map<string, ModelChoice[]>();
    for (const c of rows.slice(0, 300)) groups.set(c.provider, [...(groups.get(c.provider) ?? []), c]);
    return { groups: [...groups], hidden: f ? Math.max(0, rows.length - 300) : all.length - rows.length };
  }, [models, q]);
  // the score column only where a row has one (a "Score" header stood over an empty column, design review 12)
  const scored = groups.some(([, rows]) => rows.some((c) => c.bench));
  const query = q.trim().toLowerCase();
  const catalogs = models?.catalogs?.filter(({ provider, state }) =>
    state !== "ready" || seenPending.has(provider) || (!!query && (matchesWords(query, provider) || groups.some(([name]) => name === provider)))) ?? [];
  return (
    <div ref={box} style={place.style} className={`tz-pop absolute ${place.side} left-0 z-10 flex flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer`}>
      <div className="flex items-center gap-2 border-b border-line-soft px-3 py-2 text-muted">
        <Search size={14} strokeWidth={1.5} />
        <input ref={search} autoFocus aria-label="Search models and providers" value={q} onChange={(e) => setQ(e.target.value)} placeholder="Search models and providers" className="min-w-0 flex-1 bg-transparent text-ink outline-none placeholder:text-faint" />
      </div>
      {efforts.length > 1 && onEffort && (
        <div className="flex items-center gap-2 border-b border-line-soft px-3 py-2 text-[12px]">
          <span className="text-faint">Effort</span>
          <Segmented label="Effort" wrap value={effort ?? ""} options={efforts.map((e) => [e, e] as const)} onChange={onEffort} />
        </div>
      )}
      {models?.rebuild && <div className="border-b border-line-soft px-3 py-1.5 text-[12px] text-warn">Switching between Claude models: {models.rebuild}</div>}
      <div className="tz-scrollfade overflow-auto">
        {/* what the numbers on the right are (until desktop 0.1.74 only a hover said: "8.4 (12)", "0.14 / 0.56") */}
        {groups.length > 0 && <div className="sticky top-0 z-[1] flex items-center gap-2 bg-[var(--pop)] px-3 pb-0.5 pt-1.5 text-[11px] text-faint">
          <span className="flex-1" />{scored && <span className="w-[64px] text-right" title="tazuna bench: judge score out of 10 (runs)">Score</span>}
          <span className="w-[112px] text-right" title="dollars per million tokens: input / output">$ per M, in / out</span></div>}
        {!models && <div className="px-3 py-2 text-[12.5px] text-faint">Loading…</div>}
        {models && query && groups.length === 0 && <div className="px-3 py-2 text-[12.5px] text-muted">No known models match this search.</div>}
        {groups.map(([provider, rows]) => { const caution = models?.cautions?.[provider]; return (
          <div key={provider}>
            <div className="px-3 pb-0.5 pt-2 text-[11.5px] font-medium text-faint">{provider}</div>
            {/* its terms once, under its name: in the warning colour only where the account may be at risk or it may refuse tazuna */}
            {caution?.note && <div className={`px-3 pb-0.5 text-[11.5px] ${caution.warn ? "text-warn" : "text-faint"}`}>{caution.note}</div>}
            {rows.map((c) => { const toKey = c.no_key && onKey; return (
              <button key={`${c.model}|${c.url}|${c.key_env}|${c.label}`} disabled={!!c.why_not && !toKey} onClick={() => (toKey ? onKey(c.provider) : onPick(c))}
                title={toKey ? (c.why_not === "not signed in" ? `Sign in to ${c.provider} in Settings` : `Add a ${c.provider} key in Settings`) : c.why_not ?? c.place}
                className="flex w-full items-center gap-2 px-3 py-[5px] text-left hover:bg-hover disabled:cursor-default disabled:hover:bg-transparent">
                <span className="w-3.5 flex-none text-accent">{c.current && <Check size={14} strokeWidth={1.75} />}</span>
                <span className="min-w-0 flex-1">
                  {/* its name, not its id ("GPT-6.2 Sol", not "gpt-6.2-sol · subrouter"); the group names the provider */}
                  <span className={`block truncate ${c.why_not ? "text-faint" : "text-ink"}`} title={c.model}>{modelLabel(c.model)}</span>
                  {/* the group names the provider: a row's second line only says why it cannot run (it repeated "api.mistral.ai") */}
                  {/* no key: the desktop's own way to one (the core's words name the terminal's /login) */}
                  {c.why_not && <span className="block truncate text-[11.5px] text-faint">{c.no_key ? NO_KEY[c.why_not ?? ""] ?? NO_KEY["no key"] : c.why_not}</span>}
                </span>
                {scored && <span className="w-[64px] flex-none whitespace-nowrap text-right text-[11.5px] tabular-nums text-faint" title="tazuna bench: judge score (runs)">{c.bench}</span>}
                <span className="w-[112px] flex-none whitespace-nowrap text-right text-[11.5px] tabular-nums text-muted" title="$ per million tokens: input / output">{c.price === "plan" ? "on your plan" : price(c.price)}</span>
              </button>
            ); })}
          </div>
        ); })}
        {hidden > 0 && <div className="px-3 py-2 text-[11.5px] text-faint">{q ? `${hidden.toLocaleString("en-US")} more: narrow the search` : `${hidden.toLocaleString("en-US")} more models: type to search them`}</div>}
      </div>
      {catalogs.length > 0 && (
        <div className="max-h-32 shrink-0 divide-y divide-line-soft overflow-auto border-t border-line-soft" aria-live="polite">
          {catalogs.map(({ provider, state }) => {
            const { label, detail, tone } = CATALOG_STATUS[state];
            return (
              <div key={provider} className="flex items-center gap-3 px-3 py-2">
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-1.5 text-[11.5px] font-medium text-ink">
                    <span aria-hidden="true" className={`size-1.5 shrink-0 rounded-full ${tone}`} />
                    <span>{provider} · {label}</span>
                  </div>
                  {detail && <p className="ml-3 mt-0.5 text-[11.5px] text-muted">{detail}</p>}
                </div>
                {state === "unavailable" && onRefreshModelList && (
                  <button type="button" aria-label={`Retry ${provider} model list`} onClick={() => { search.current?.focus(); onRefreshModelList(provider); }}
                    className="shrink-0 rounded-md border border-line px-2 py-1 text-[11.5px] text-ink hover:bg-hover focus-visible:outline focus-visible:outline-accent">
                    Retry list
                  </button>
                )}
              </div>
            );
          })}
        </div>
      )}
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
        <button key={e} role="menuitemradio" aria-checked={e === current} onClick={() => onPick(e)} className="flex w-full items-center gap-2 px-3 py-[5px] text-left hover:bg-hover">
          <span className="w-3.5 text-accent">{e === current && <Check size={14} strokeWidth={1.75} />}</span>{e}
        </button>
      ))}
    </Menu>
  );
}
