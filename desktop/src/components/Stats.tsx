import { Activity, Coins, Database, Gauge } from "lucide-react";
import { useContext, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { STATS, useLook } from "../lib/chrome";
import { costSaid, stats, usage, type Point } from "../lib/fromView";
import type { ModelChoice, View } from "../lib/tazuna";
import { median, tenths, usd } from "../lib/types";
import { Segmented } from "./Controls";
import { WorkCtx } from "./Rail";
import { dur, perSec, tok, type Turn } from "./Receipt";
import { ChatCost } from "./StatusBar";

// The work panel's Stats (the user, 2026-11-25: "a real-time calculation and statistics section for all numerical
// metrics… maybe smooth"; mockups/v11.html): this turn's or the whole conversation's tokens, cost, calls and speeds, a
// sparkline for each number that varies by call, live while a reply runs. Real numbers only: what the view does not say is
// left out ("–" only for a cost it cannot sum); a zero is quiet. Data marks are `live`, never the accent. As tiles (the
// default) or as rows on the desk (Settings › Appearance › Stats; design review 14, S4 of mockups/v12.html).

const still = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

/** A number that moves to a new value in ~450 ms, easing out, while a reply runs (`live`); else, and under reduced motion,
 * it jumps (another conversation's numbers do not slide in). A change mid-way goes on from where it is. */
function useTween(to: number, live: boolean) {
  const [at, setAt] = useState(to);
  const now = useRef(to);
  useEffect(() => {
    if (!live || still()) { now.current = to; setAt(to); return; }
    const from = now.current, t0 = performance.now();
    let raf = 0;
    const tick = (t: number) => {
      const p = Math.min(1, (t - t0) / 450);
      now.current = from + (to - from) * (1 - (1 - p) ** 3);
      setAt(now.current);
      if (p < 1) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [to, live]);
  return at;
}

const box = "min-w-0 rounded-[10px] border border-line-soft bg-main px-2.5";
const label = "text-[11.5px] text-faint";
const Bar = ({ pct }: { pct: number }) => (
  <div className="mt-[7px] h-1.5 overflow-hidden rounded-[3px] bg-field"><i className="block h-full rounded-[3px] bg-live transition-[width] duration-[450ms] ease-out" style={{ width: `${Math.min(100, Math.max(0, pct))}%` }} /></div>
);

/** A zero is quiet text, not a big number (design review 14), on the big one's line. */
function Tile({ name, value, sub, zero, children }: { name: string; value: ReactNode; sub?: string; zero?: boolean; children?: ReactNode }) {
  return (
    <div className={`${box} py-[9px]`}>
      <div className={label}>{name}</div>
      <div className={`truncate ${zero ? "text-[13.5px] leading-[25px] text-faint" : "text-[20px] font-semibold leading-tight tracking-[-0.02em] text-ink"}`}>{value}</div>
      {sub && <div className="text-[11.5px] leading-snug text-muted">{sub}</div>}
      {children}
    </div>
  );
}

/** One line in the live colour, its last point marked, no axes. The pointer, or the arrows once it has the focus (the usage
 * page's bars' way), shows a call's value in a tip over it, and a screen reader hears it. While a reply runs the newest
 * point grows in from the one before. */
function Spark({ points, fmt, name, max, live }: { points: Point[]; fmt: (v: number) => string; name: string; max?: number; live: boolean }) {
  const [at, setAt] = useState<number | null>(null);
  const n = points.length, ref = useRef<HTMLDivElement>(null);
  const shown = useTween(n, live), hi = useTween(max ?? (Math.max(0, ...points.map((p) => p[1])) * 1.1 || 1), live);
  // the newest point's way in: those past `e` (a fractional index) sit at the line's head, between the two last
  const e = Math.max(0, Math.min(n - 1, shown - 1)), k = Math.floor(e), fr = e - k;
  const y = (v: number) => 100 - Math.min(100, (100 * v) / hi);
  const xy = (i: number): [number, number] => [e ? (100 * i) / e : 50, y(points[i][1])];
  const line = n ? [...points.slice(0, k + 1).map((_, i) => xy(i)), ...(fr > 0 && k + 1 < n ? [[100, y(points[k][1] + (points[k + 1][1] - points[k][1]) * fr)] as [number, number]] : [])] : [];
  const tip = at == null || at >= n ? null : points[at], head = line[line.length - 1];
  const pick = (clientX: number) => { const r = ref.current!.getBoundingClientRect(); setAt(Math.max(0, Math.min(n - 1, Math.round(((clientX - r.left) / r.width) * (n - 1))))); };
  return (
    <div className="relative mt-1 h-[34px] rounded-sm" tabIndex={n ? 0 : -1} role="group"
      aria-label={tip ? `${name}: call ${tip[0]}, ${fmt(tip[1])}` : `${name}, call by call: arrows move along the calls`}
      onPointerMove={(ev) => n && pick(ev.clientX)} onPointerLeave={() => setAt(null)} onFocus={() => setAt((a) => a ?? n - 1)} onBlur={() => setAt(null)}
      onKeyDown={(ev) => { const d = ev.key === "ArrowLeft" ? -1 : ev.key === "ArrowRight" ? 1 : ev.key === "Home" ? -n : ev.key === "End" ? n : 0;
        if (d && n) { ev.preventDefault(); setAt((a) => Math.max(0, Math.min(n - 1, (a ?? n - 1) + d))); } }}>
      {/* inset so the last point's mark is not cut at the edges */}
      <div ref={ref} aria-hidden="true" className="absolute inset-x-1 inset-y-1">
        {line.length > 1 && <svg viewBox="0 0 100 100" preserveAspectRatio="none" className="absolute inset-0 size-full overflow-visible">
          <path d={line.map(([x, v], i) => `${i ? "L" : "M"}${x.toFixed(2)},${v.toFixed(2)}`).join("")} fill="none" stroke="var(--live)" strokeWidth={2}
            strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" /></svg>}
        {tip && <i className="absolute inset-y-[-4px] w-px bg-line-strong" style={{ left: `${xy(at!)[0]}%` }} />}
        {head && <i className="absolute size-[7px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-live shadow-[0_0_0_2px_var(--main)]" style={{ left: `${head[0]}%`, top: `${head[1]}%` }} />}
        {/* the tip goes along with its call, its edges inside the chart's (left x% less x% of its own width) */}
        {tip && <div className="tz-float pointer-events-none absolute bottom-[calc(100%+6px)] z-10 whitespace-nowrap rounded-md border border-line-strong px-2 py-[3px] text-[11.5px] tabular-nums text-ink"
          style={{ left: `${xy(at!)[0]}%`, transform: `translateX(-${xy(at!)[0]}%)` }}>call {tip[0]} · {fmt(tip[1])}</div>}
      </div>
    </div>
  );
}

function Metric({ name, value, sub, zero, children }: { name: string; value: string; sub?: string; zero?: boolean; children?: ReactNode }) {
  return (
    <div className={`${box} pb-[7px] pt-[9px]`}>
      {/* the value and its median go under the name where the row is too narrow for all three (over the conversation,
          "Writing speed 226 t/s median 219 t/s"), never cutting the name */}
      <div className="flex flex-wrap items-baseline justify-end gap-x-1.5"><span className={`${label} mr-auto`}>{name}</span>
        <span className="flex items-baseline gap-1.5"><span className={`tabular-nums ${zero ? "text-[13.5px] text-faint" : "text-[15px] font-medium text-ink"}`}>{value}</span>{sub && <span className="text-[11.5px] text-muted">{sub}</span>}</span></div>
      {children}
    </div>
  );
}

const secs = tenths;
// a share of the input that never rounds up to all of it (pct's rule)
const share = (x: number) => `${x >= 100 ? 100 : Math.min(99, Math.round(x))}%`;
const last = (p: Point[]) => (p.length ? p[p.length - 1][1] : null);

/** One small line in the live colour, no marks: a row's own (rows, from 8 calls). */
function Line({ points, max }: { points: Point[]; max?: number }) {
  const hi = max ?? (Math.max(0, ...points.map((p) => p[1])) * 1.1 || 1), n = points.length;
  return (
    <svg aria-hidden="true" viewBox="0 0 56 14" preserveAspectRatio="none" className="block h-3.5 w-14 overflow-visible">
      <path d={points.map(([, v], i) => `${i ? "L" : "M"}${(1 + (54 * i) / (n - 1)).toFixed(1)},${(14 - (14 * Math.min(v, hi)) / hi).toFixed(1)}`).join("")} fill="none" stroke="var(--live)"
        strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" vectorEffect="non-scaling-stroke" /></svg>
  );
}

type Scope = "turn" | "all";
/** Opens the work panel at Stats over this scope (a click on one of the composer's numbers; `from`: where the focus goes back
 * when a panel over the conversation shuts). App opens it unless a side chat or a subagent has the column. */
export const openStats = (scope: Scope, from: HTMLElement) => window.dispatchEvent(new CustomEvent("tazuna-stats", { detail: { scope, from } }));
// the last ask taken, so each is taken once (a Stats mounted later does not take it again)
let taken = 0;

export function StatsSection() {
  const { view, active, statsAsk } = useContext(WorkCtx);
  const [scope, setScope] = useState<Scope>("turn");
  const rows = useLook(STATS) === "rows";
  useEffect(() => { if (statsAsk && statsAsk.n > taken) { taken = statsAsk.n; setScope(statsAsk.scope); } }, [statsAsk?.n]);
  const live = !!view && !["Ready", "Done"].includes(view.activity);
  // a conversation with no model call yet, nor one running: nothing real to show
  if (!view || (!view.calls?.length && !live)) return null;
  return (
    <section aria-label="Stats" className="border-b border-line pb-3.5 last:border-b-0">
      {/* the heading on the window's top row, as every first section's: the switch takes no more height than the words. The
          rows have no live dot: their numbers moving say it */}
      <div className="flex items-center gap-2 py-1 font-medium text-ink">
        <span className="flex flex-1 items-center gap-2">Stats{live && !rows && <span className="inline-flex items-center gap-1.5 text-[11.5px] font-normal text-live-ink">
          <i className="size-1.5 rounded-full bg-live shadow-[0_0_0_3px_var(--live-soft)]" />live</span>}</span>
        <span className="-my-1 font-normal"><Segmented label="Stats for" value={scope} options={[["turn", "This turn"], ["all", "Conversation"]]} onChange={setScope} /></span>
      </div>
      {/* another conversation or scope starts afresh: its numbers jump, never slide from the last ones' (the review of
          2026-11-25); the switch stays outside, so it keeps the keyboard's focus */}
      <Numbers key={`${active ?? ""}:${scope}`} view={view} scope={scope} live={live} rows={rows} />
    </section>
  );
}

function Numbers({ view, scope, live, rows }: { view: View; scope: Scope; live: boolean; rows: boolean }) {
  const s = stats(view, scope), u = usage(view);
  // every number tweens while a reply runs
  const t = { input: useTween(s.input, live), hit: useTween(s.hit ?? 0, live), out: useTween(s.out, live), tps: useTween(s.tps ?? 0, live),
    cost: useTween(s.cost ?? 0, live), calls: useTween(s.calls, live), secs: useTween(s.secs, live), ctx: useTween(view.context, live),
    ttft: useTween(last(s.ttft) ?? 0, live), speed: useTween(last(s.speed) ?? 0, live), hits: useTween(last(s.hits) ?? 0, live) };
  const ttftMed = median(s.ttft.map((p) => p[1])), speedMed = median(s.speed.map((p) => p[1]));
  // "about" where a call's price is at list prices, as the receipt says it
  const about = s.about && <span className="text-[13px] font-normal tracking-normal text-muted">about </span>;
  if (rows) {
    // a slip in spirit: label left, value right, rules between; a row with nothing to say is left out, a zero is quiet, a
    // line only from 8 calls (fewer read as noise)
    const lines = s.calls >= 8;
    const all: [string, string | undefined, ReactNode, number | null | undefined, ReactNode?][] = [
      ["In", undefined, tok(Math.round(t.input)), s.input],
      ["From the cache", undefined, share(t.hit), s.hit, lines && s.hits.length > 1 && <Line points={s.hits} max={100} />],
      ["Out", undefined, tok(Math.round(t.out)), s.out],
      ["Writing speed", undefined, perSec(t.tps), s.tps, lines && s.speed.length > 1 && <Line points={s.speed} />],
      ["Time to first token", "median", ttftMed == null ? "" : secs(ttftMed), ttftMed, lines && s.ttft.length > 1 && <Line points={s.ttft} />],
      ["Cost", s.how, <>{about}{usd(t.cost)}</>, s.cost],
      ["Calls", undefined, String(Math.round(t.calls)), s.calls],
      ["Working", undefined, dur(t.secs), s.secs || null],
      ["Context", undefined, `${tok(Math.round(t.ctx))}${u.contextMax ? ` of ${u.contextMax}` : ""}`, view.context],
    ];
    return (
      <div className="pt-0.5 text-[12.5px]">
        {all.filter((r) => r[3] != null).map(([name, q, value, raw, line]) => (
          <div key={name} className="grid min-h-[29px] grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-2.5 border-t border-line text-ink-2 first:border-t-0">
            <span className="min-w-0">{name}{q && <> <span className="ml-0.5 text-faint">{q}</span></>}</span>
            <span>{line || null}</span>
            <span className={`whitespace-nowrap text-right tabular-nums ${raw === 0 ? "text-faint" : "text-ink"}`}>{value}</span>
          </div>
        ))}
      </div>
    );
  }
  return (
    <div className="pt-1">
      <div className="mb-2 grid grid-cols-2 gap-2">
        <Tile name="In" value={tok(Math.round(t.input))} zero={!s.input} sub={s.hit == null ? undefined : `${share(t.hit)} from cache`}>{s.hit != null && <Bar pct={s.hit} />}</Tile>
        <Tile name="Out" value={tok(Math.round(t.out))} zero={!s.out} sub={s.tps == null ? undefined : `${perSec(t.tps)} writing`} />
        <Tile name="Cost" value={s.cost == null ? "–" : <>{about}{usd(t.cost)}</>} zero={s.cost === 0} sub={s.how} />
        <Tile name="Calls" value={String(Math.round(t.calls))} zero={!s.calls} sub={s.secs ? `${dur(t.secs)} working` : undefined} />
      </div>
      <div className="flex flex-col gap-2">
        {/* a line of fewer than 3 points says nothing: its tile is left out, never an empty box with "–" (design review 14) */}
        {s.ttft.length >= 3 && <Metric name="Time to first token" value={secs(t.ttft)} sub={ttftMed == null ? undefined : `median ${secs(ttftMed)}`}>
          <Spark name="Time to first token" points={s.ttft} fmt={secs} live={live} /></Metric>}
        {s.speed.length >= 3 && <Metric name="Writing speed" value={perSec(t.speed)} sub={speedMed == null ? undefined : `median ${perSec(speedMed)}`}>
          <Spark name="Writing speed" points={s.speed} fmt={perSec} live={live} /></Metric>}
        {s.hits.length >= 3 && <Metric name="Cache hit rate" value={share(t.hits)} sub={s.hit == null ? undefined : `${share(s.hit)} overall`}>
          <Spark name="Cache hit rate" points={s.hits} fmt={share} max={100} live={live} /></Metric>}
        {/* the last call's input against the model's window; a window the core does not know (a local model) has no bar */}
        <Metric name="Context" value={tok(Math.round(t.ctx))} zero={!view.context} sub={u.contextMax ? `of ${u.contextMax}` : undefined}>
          {u.contextMax && <Bar pct={u.contextPct} />}</Metric>
      </div>
    </div>
  );
}

/** The conversation's numbers in the composer's own strip, along its lower edge or its top (design review 14, M-A of
 * mockups/v13.html; Settings › Appearance › Conversation numbers): its context against the model's window, its cost
 * (`costSaid`), the cache's share, and the reply's speed while it runs, appended last so nothing moves when it comes. The
 * cost opens the conversation's receipt (as the status bar's "this chat" does), the speed Stats over this turn, the rest
 * Stats over the conversation. The Stats' and the status bar's own numbers: no sums of its own. A short row drops the
 * cache, then the cost's words, then "of 128k" (`data-drop`, styles.css); never wraps. */
export function ConvNumbers({ view, choice, busy, turns, top }: { view: View; choice?: ModelChoice; busy: boolean; turns: Turn[]; top?: boolean }) {
  const row = useRef<HTMLDivElement>(null);
  const s = stats(view, "all"), u = usage(view), tps = busy ? stats(view, "turn").tps : null, paid = costSaid(s, choice, busy);
  // [its name, the tooltip, what it shows]; the cache's part goes first where the row is short (tz-n1)
  const parts: [string, string, ReactNode][] = [];
  if (view.context > 0) parts.push(["context", `Context: ${tok(view.context)}${u.contextMax ? ` of the model's ${u.contextMax} window, ${u.contextPct}%` : ""}`, <>
    <Gauge size={14} strokeWidth={1.5} className="text-faint" /><span className="font-medium text-ink">{tok(view.context)}</span>
    {u.contextMax && <><span className="tz-n3 -ml-0.5">of {u.contextMax}</span><span className="h-[5px] w-10 flex-none overflow-hidden rounded-[3px] bg-line-strong">
      <i className="block h-full rounded-[3px] bg-live" style={{ width: `${u.contextPct}%` }} /></span><span>{u.contextPct}%</span></>}</>]);
  if (paid) parts.push(["cost", paid[2], <><Coins size={14} strokeWidth={1.5} className="text-faint" /><span className="font-medium text-ink">{paid[0]}</span>
    {paid[1] && <span className="tz-n2 -ml-0.5">{paid[1]}</span>}</>]);
  if (u.cachedPct > 0) parts.push(["cache", "Input read from the provider's cache in this conversation", <>
    <Database size={14} strokeWidth={1.5} className="text-faint" /><span className="font-medium text-ink">{u.cachedPct}%</span><span className="-ml-0.5">cached</span></>]);
  if (tps) parts.push(["speed", "The reply's writing speed", <><Activity size={14} strokeWidth={1.5} className="text-live" /><span className="font-medium text-ink">{Math.round(tps)}</span><span className="-ml-0.5">t/s</span></>]);
  useLayoutEffect(() => { fit(row.current); });
  useEffect(() => { const el = row.current; if (!el) return; const r = new ResizeObserver(() => fit(el)); r.observe(el); return () => r.disconnect(); }, [!parts.length]);
  if (!parts.length) return null;
  // tabIndex: WebKit's Tab skips buttons; data-panel: a press here is not "outside" a work panel open over the conversation
  const part = "flex h-6 items-center gap-1.5 rounded-md px-[7px] hover:bg-hover hover:text-ink-2";
  return (
    <div ref={row} aria-label="This conversation's numbers" role="group"
      className={`tz-nums -ml-4 -mr-3 flex h-8 items-center whitespace-nowrap bg-[color-mix(in_srgb,var(--field)_40%,var(--card))] pl-[9px] pr-[5px] text-[12.5px] tabular-nums text-muted ${top ? "-mt-3 mb-2.5 rounded-t-[11px] border-b border-line" : "-mb-2 mt-2 rounded-b-[11px] border-t border-line"}`}>
      {parts.map(([k, title, inner], i) => (
        <span key={k} className={`flex flex-none items-center ${k === "cache" ? "tz-n1" : ""}`}>
          {i > 0 && <i aria-hidden="true" className="mx-[3px] h-3.5 w-px bg-line" />}
          {k === "cost" && turns.length ? <ChatCost cost={view.cost} turns={turns} busy={busy} className={part} title={`${title} · its receipt`} left>{inner}</ChatCost>
            : <button tabIndex={0} data-panel onClick={(e) => openStats(k === "speed" ? "turn" : "all", e.currentTarget)} title={`${title} · Stats`} className={part}>{inner}</button>}
        </span>
      ))}
    </div>
  );
}
/** Drops parts until the row fits: the cache, the cost's words, "of 128k" (scrollWidth counts what spills out). */
const fit = (el: HTMLElement | null) => { if (!el) return; el.dataset.drop = "0"; for (let d = 1; d <= 3 && el.scrollWidth > el.clientWidth; d++) el.dataset.drop = String(d); };
