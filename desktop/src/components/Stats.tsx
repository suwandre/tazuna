import { Activity, Coins, Database, Gauge, Timer } from "lucide-react";
import { useContext, useEffect, useLayoutEffect, useMemo, useReducer, useRef, useState, type ReactNode } from "react";
import { LIVE, STATS, useLook } from "../lib/chrome";
import { CONTEXT_WARN, compactable, costSaid, stats, usage, type Point } from "../lib/fromView";
import { blend, lastSpeed, liveRow, outNow, settled, still, useFrame, useLive, useWarm, warmSaid, type Live } from "../lib/live";
import type { ModelChoice, View } from "../lib/tazuna";
import { median, pctText, tenths, usd } from "../lib/types";
import { Segmented } from "./Controls";
import { WorkCtx } from "./Rail";
import { dur, perSec, tok, type Turn } from "./Receipt";
import { Sep } from "./Sep";
import { ChatCost, CompactButton, useCompact } from "./StatusBar";

// The work panel's Stats (the user, 2026-11-25: "a real-time calculation and statistics section for all numerical
// metrics… maybe smooth"; mockups/v11.html): this turn's or the whole conversation's tokens, cost, calls and speeds, a
// sparkline for each number that varies by call, live while a reply runs. Real numbers only: what the view does not say is
// left out ("–" only for a cost it cannot sum); a zero is quiet. Data marks are `live`, never the accent. As tiles (the
// default) or as rows on the desk (Settings › Appearance › Stats; design review 14, S4 of mockups/v12.html). With Live stats
// on, the call in flight's estimate moves them every frame while it streams (lib/live.ts), drawn in `muted` until exact.

/** A number that moves to a new value in ~450 ms, easing out, while a reply runs (`live`); else, and under reduced motion,
 * it jumps (another conversation's numbers do not slide in). A change mid-way goes on from where it is. `follow`: it is the
 * live estimate, drawn as it is, eased onto from where it stood when following begins (a call's start moves the context
 * and the cost by its input), and eased from where it stood to the exact value when that comes. `count`: a count (tokens,
 * money, time) never counts back down: an estimate that ran over its exact value settles there at once. */
function useTween(to: number, live: boolean, follow = false, count = false) {
  const [at, setAt] = useState(to);
  const [, kick] = useReducer((n: number) => n + 1, 0);
  const now = useRef(to), began = useRef<{ from: number; t0: number } | null>(null), followed = useRef(false), ended = useRef(false);
  if (follow !== followed.current) {
    if (follow) began.current = live && !still() ? { from: now.current, t0: performance.now() } : null;
    else ended.current = true;
    followed.current = follow;
  }
  const b = follow ? began.current : null, ms = b ? performance.now() - b.t0 : 450;
  const shown = follow ? (b ? blend(b.from, to, ms) : to) : at;
  if (follow) now.current = shown;
  // a frame each while it eases in (the live frames come only when a digit of the estimate moves)
  useEffect(() => { if (ms >= 450) return; const r = requestAnimationFrame(kick); return () => cancelAnimationFrame(r); });
  // before paint: the frame after `follow` ends starts where the estimate stood, not at a value from before it
  useLayoutEffect(() => {
    if (follow) return;
    const settle = ended.current;
    ended.current = false;
    if (!live || still() || (count && settle && to < now.current)) { now.current = to; setAt(to); return; }
    const from = now.current, t0 = performance.now();
    setAt(from);
    let raf = 0;
    const tick = (t: number) => {
      const p = Math.min(1, (t - t0) / 450);
      now.current = from + (to - from) * (1 - (1 - p) ** 3);
      setAt(now.current);
      if (p < 1) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [to, live, follow]);
  return shown;
}

/** A number that may change every frame: while `hold` (a reply runs) its box only grows, so nothing beside it moves as its
 * digits change (tabular figures do the rest); it fits its text again when the reply ends. `est`: an estimate, in `muted`. */
function Num({ hold, est, className = "", children }: { hold: boolean; est?: boolean; className?: string; children: ReactNode }) {
  const el = useRef<HTMLSpanElement>(null), w = useRef(0), len = useRef(-1);
  // measured only when its text's length changes (a measure every frame is a layout every frame)
  useLayoutEffect(() => {
    const e = el.current;
    if (!e) return;
    if (!hold) { if (w.current) { w.current = 0; e.style.minWidth = ""; } len.current = -1; return; }
    const n = e.textContent?.length ?? 0;
    if (n === len.current) return;
    len.current = n;
    const now = e.getBoundingClientRect().width;
    if (now > w.current + 0.5) { w.current = now; e.style.minWidth = `${now}px`; }
  });
  return <span ref={el} className={`inline-block text-right ${est ? "text-muted" : ""} ${className}`}>{children}</span>;
}

const box = "min-w-0 rounded-[10px] border border-line-soft bg-main px-2.5";
const label = "text-[11.5px] text-faint";
/** `warn`: a limit near (the context from CONTEXT_WARN), in the warning colour */
const Bar = ({ pct, warn }: { pct: number; warn?: boolean }) => (
  <div className="mt-[7px] h-1.5 overflow-hidden rounded-[3px] bg-field"><i className={`block h-full rounded-[3px] ${warn ? "bg-warn" : "bg-live"} transition-[width] duration-[450ms] ease-out`} style={{ width: `${Math.min(100, Math.max(0, pct))}%` }} /></div>
);

/** A zero is quiet text, not a big number (design review 14), on the big one's line. */
function Tile({ name, value, sub, zero, est, children }: { name: string; value: ReactNode; sub?: string; zero?: boolean; est?: boolean; children?: ReactNode }) {
  // a figure too wide for its tile steps down in size until it fits, never cut ("about $243.…" in a narrow panel, the user
  // 2026-11-27): measured when its text's length or the tile's width changes, not every frame of a live reply
  const ref = useRef<HTMLDivElement>(null), len = useRef(-1);
  useLayoutEffect(() => {
    const el = ref.current, n = el?.textContent?.length ?? 0;
    if (el && n !== len.current) { len.current = n; fitText(el); }
  });
  useEffect(() => { const el = ref.current; if (!el) return; const r = new ResizeObserver(() => fitText(el)); r.observe(el); return () => r.disconnect(); }, []);
  return (
    <div className={`${box} py-[9px]`}>
      <div className={label}>{name}</div>
      <div ref={ref} className={`truncate tabular-nums ${zero ? "text-[13.5px] leading-[25px] text-faint" : `text-[20px] font-semibold leading-tight tracking-[-0.02em] ${est ? "text-muted" : "text-ink"}`}`}>{value}</div>
      {sub && <div className="text-[11.5px] leading-snug text-muted">{sub}</div>}
      {children}
    </div>
  );
}

/** A tile's figure at 20 px, else the first of 18, 16, 14 px at which it fits its width; still too wide at 14 px (a work
 * panel at its narrowest), it ends in an ellipsis and says itself whole on hover. */
const fitText = (el: HTMLElement) => {
  el.style.fontSize = "";
  for (const px of [18, 16, 14]) { if (el.scrollWidth <= el.clientWidth) break; el.style.fontSize = `${px}px`; }
  el.title = el.scrollWidth > el.clientWidth ? el.textContent ?? "" : "";
};

/** One line in the live colour, its last point marked, no axes. The pointer, or the arrows once it has the focus (the usage
 * page's bars' way), shows a call's value in a tip over it, and a screen reader hears it. While a reply runs the newest
 * point grows in from the one before. */
function Spark({ points, fmt, name, max, live, follow }: { points: Point[]; fmt: (v: number) => string; name: string; max?: number; live: boolean; follow: boolean }) {
  const [at, setAt] = useState<number | null>(null);
  const n = points.length, ref = useRef<HTMLDivElement>(null);
  // the call in flight's point moves with the live estimate (`follow`): the scale follows it
  const shown = useTween(n, live), hi = useTween(max ?? (Math.max(0, ...points.map((p) => p[1])) * 1.1 || 1), live, follow);
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

function Metric({ name, value, sub, zero, est, children }: { name: string; value: string; sub?: string; zero?: boolean; est?: boolean; children?: ReactNode }) {
  return (
    <div className={`${box} pb-[7px] pt-[9px]`}>
      {/* the value and its median go under the name where the row is too narrow for all three (over the conversation,
          "Writing speed 226 t/s median 219 t/s"), never cutting the name */}
      <div className="flex flex-wrap items-baseline justify-end gap-x-1.5"><span className={`${label} mr-auto`}>{name}</span>
        <span className="flex items-baseline gap-1.5"><span className={`tabular-nums ${zero ? "text-[13.5px] text-faint" : `text-[15px] font-medium ${est ? "text-muted" : "text-ink"}`}`}>{value}</span>{sub && <span className="text-[11.5px] text-muted">{sub}</span>}</span></div>
      {children}
    </div>
  );
}

const secs = tenths;
// a share of the input that never rounds up to all of it (pct1's rule), to a tenth (the user, 2026-11-26)
const share = (x: number) => `${pctText(x >= 100 ? 100 : Math.min(99.9, Math.round(x * 10) / 10))}%`;
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
          rows have no live dot: their numbers moving say it. In a narrow panel (260 px) the switch goes under the words; the
          dot keeps its room while nothing runs, so the switch does not move between replies */}
      <div className="flex flex-wrap items-center gap-x-2 gap-y-2.5 py-1 font-medium text-ink">
        <span className="flex flex-1 items-center gap-2">Stats{!rows && <span className={`inline-flex items-center gap-1.5 text-[11.5px] font-normal text-live-ink ${live ? "" : "invisible"}`}>
          <i className="size-1.5 rounded-full bg-live shadow-[0_0_0_3px_var(--live-soft)]" />live</span>}</span>
        <span className="-my-1 font-normal"><Segmented label="Stats for" value={scope} options={[["turn", "This turn"], ["all", "Conversation"]]} onChange={setScope} /></span>
      </div>
      {/* another conversation or scope starts afresh: its numbers jump, never slide from the last ones' (the review of
          2026-11-25); the switch stays outside, so it keeps the keyboard's focus */}
      <Numbers key={`${active ?? ""}:${scope}`} view={view} scope={scope} live={live} rows={rows} />
    </section>
  );
}

function Numbers({ view: core, scope, live, rows }: { view: View; scope: Scope; live: boolean; rows: boolean }) {
  // the call in flight drawn every frame while it streams (Live stats): its numbers estimates (`f`), in `muted`
  const { view, streaming: f } = useLive(core, (lv) => { const s = stats(lv, scope), m = (p: Point[]) => median(p.map((x) => x[1])) ?? 0;
    // what it shows that the call moves: a frame that changes none of it draws nothing
    return [tok(s.input), share(s.hit ?? 0), tok(s.out), Math.round(s.tps ?? 0), usd(s.cost ?? 0), dur(s.secs), tok(lv.context), usage(lv).contextPct,
      secs(last(s.ttft) ?? 0), secs(m(s.ttft)), Math.round(last(s.speed) ?? 0), Math.round(m(s.speed)), share(last(s.hits) ?? 0), s.ttft.length, s.speed.length].join(); });
  const s = stats(view, scope), u = usage(view);
  // a sparkline's 3 points are finished calls': the one in flight (live) is no reason for its tile to come, then go at its end
  const base = useMemo(() => stats(core, scope), [core, scope]);
  // every number tweens while a reply runs, and follows the estimate while a call streams; the counts never count back
  const t = { input: useTween(s.input, live, f, true), hit: useTween(s.hit ?? 0, live, f), out: useTween(s.out, live, f, true), tps: useTween(s.tps ?? 0, live, f),
    cost: useTween(s.cost ?? 0, live, f, true), calls: useTween(s.calls, live), secs: useTween(s.secs, live, f, true), ctx: useTween(view.context, live, f, true),
    ttft: useTween(last(s.ttft) ?? 0, live, f), speed: useTween(last(s.speed) ?? 0, live, f), hits: useTween(last(s.hits) ?? 0, live, f) };
  const ttftMed = median(s.ttft.map((p) => p[1])), speedMed = median(s.speed.map((p) => p[1]));
  // "about" where a call's price is at list prices, as the receipt says it
  const about = s.about && <span className="text-[0.65em] font-normal tracking-normal text-muted">about </span>;
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
            <Num hold={live} className={`whitespace-nowrap tabular-nums ${raw === 0 ? "text-faint" : name === "Context" && u.contextMax && u.contextPct >= CONTEXT_WARN ? "text-warn" : f && name !== "Calls" ? "text-muted" : "text-ink"}`}>{value}</Num>
          </div>
        ))}
      </div>
    );
  }
  return (
    <div className="pt-1">
      <div className="mb-2 grid grid-cols-2 gap-2">
        <Tile name="In" value={tok(Math.round(t.input))} zero={!s.input} est={f} sub={s.hit == null ? undefined : `${share(t.hit)} from cache`}>{s.hit != null && <Bar pct={s.hit} />}</Tile>
        <Tile name="Out" value={tok(Math.round(t.out))} zero={!s.out} est={f} sub={s.tps == null ? undefined : `${perSec(t.tps)} writing`} />
        <Tile name="Cost" value={s.cost == null ? "–" : <>{about}{usd(t.cost)}</>} zero={s.cost === 0} est={f} sub={s.how} />
        <Tile name="Calls" value={String(Math.round(t.calls))} zero={!s.calls} sub={s.secs ? `${dur(t.secs)} working` : undefined} />
      </div>
      <div className="flex flex-col gap-2">
        {/* a line of fewer than 3 points says nothing: its tile is left out, never an empty box with "–" (design review 14) */}
        {base.ttft.length >= 3 && <Metric name="Time to first token" value={secs(t.ttft)} est={f} sub={ttftMed == null ? undefined : `median ${secs(ttftMed)}`}>
          <Spark name="Time to first token" points={s.ttft} fmt={secs} live={live} follow={f} /></Metric>}
        {base.speed.length >= 3 && <Metric name="Writing speed" value={perSec(t.speed)} est={f} sub={speedMed == null ? undefined : `median ${perSec(speedMed)}`}>
          <Spark name="Writing speed" points={s.speed} fmt={perSec} live={live} follow={f} /></Metric>}
        {base.hits.length >= 3 && <Metric name="Cache hit rate" value={share(t.hits)} est={f} sub={s.hit == null ? undefined : `${share(s.hit)} overall`}>
          <Spark name="Cache hit rate" points={s.hits} fmt={share} max={100} live={live} follow={f} /></Metric>}
        {/* the last call's input against the model's window (with the call in flight's output so far, live); a window the
            core does not know (a local model) has no bar */}
        <Metric name="Context" value={tok(Math.round(t.ctx))} zero={!view.context} est={f} sub={u.contextMax ? `of ${u.contextMax}` : undefined}>
          {u.contextMax && <Bar pct={u.contextPct} warn={u.contextPct >= CONTEXT_WARN} />}</Metric>
      </div>
    </div>
  );
}

/** The conversation's numbers in the composer's own strip, along its lower edge or its top (design review 14, M-A of
 * mockups/v13.html; Settings › Appearance › Conversation numbers): its context against the model's window, its cost
 * (`costSaid`), the cache's share, and a speed, appended last so nothing moves when it comes: while a call streams (Live
 * stats) its time to first token, counting, then its writing speed; between calls the reply's; once it ended the last
 * reply's average. The cost opens the conversation's receipt (as the status bar's "this chat" does), the speed Stats over
 * this turn, the rest Stats over the conversation. The Stats' and the status bar's own numbers: no sums of its own. A
 * short row drops the cache's share, then the cost's words, then "of 128k", then the cache's warmth (`data-drop`,
 * styles.css); never wraps. With Live stats on the numbers follow the call in flight every frame, in `muted` while
 * estimates, each in a box that only grows during the reply, and ease to the exact ones when the call ends. On Claude (the
 * core's `cache`), after the cache's share, how long its cache stays warm ("warm 4m", "cold"); from CONTEXT_WARN of the
 * window the bar and the percent in the warning colour and, between replies where claude takes it, a Compact button
 * (`onCompact` sends /compact; none while an earlier message is being edited), pending until the turn runs. */
export function ConvNumbers({ view: core, choice, busy, turns, top, onCompact }: { view: View; choice?: ModelChoice; busy: boolean; turns: Turn[]; top?: boolean; onCompact?: () => Promise<unknown> }) {
  const row = useRef<HTMLDivElement>(null);
  const warm = useWarm(core.cache);
  const [compacting, compact] = useCompact(core.blocks.length, busy, onCompact);
  const { view, streaming: est, on } = useLive(core, (lv) => { const s = stats(lv, "all"), c = liveRow(lv);
    return [tok(lv.context), usage(lv).contextPct, usd(s.cost ?? s.done), c?.tps ? Math.round(c.tps) : secs((c?.first_ms ?? 0) / 1000)].join(); });
  const settle = on && busy, ctx = Math.round(useTween(view.context, settle, est, true));
  const s = stats(view, "all"), u = usage({ ...view, context: ctx }), n = useTween(s.cost ?? s.done, settle, est, true);
  // while a reply runs the strip says "so far" (or, on a plan, at API prices): the cost or the finished turns' sum, eased as one number
  const paid = costSaid(busy ? { ...s, cost: s.cost == null ? null : n, done: n } : s, choice, busy);
  // the speed: the call's own once timed (eased onto from the one before), between calls the reply's, else the last reply's
  // with a speed; eased between them (it jumped at each call's end)
  const last = useMemo(() => lastSpeed(core), [core.blocks, core.calls]), c = est ? liveRow(view) : undefined;
  const turnTps = busy ? stats(view, "turn").tps : null, target = c?.tps ?? turnTps ?? last?.tps ?? null;
  const tps = Math.round(useTween(target ?? 0, settle, !!c?.tps));
  const sp: [string, string, string] | null = c && !c.tps && c.first_ms != null ? [secs(c.first_ms / 1000), "", core.live?.first == null ? "Time to first token, counting" : "Time to first token"]
    : target ? [String(tps), "t/s", c?.tps ? "This call's writing speed (live estimate)" : turnTps ? "The reply's writing speed"
      : last?.last ? "Average writing speed of the last reply" : "Average writing speed of the last reply long enough to time"] : null;
  // the tooltips say the core's figures, which change as a call ends, never every frame
  const cu = usage(settled(core)), live = est ? " (live estimate)" : "";
  // [its name, the tooltip, what it shows, its text: the row is fitted again when a text's length changes]
  const parts: [string, string, ReactNode, string][] = [];
  const full = !!u.contextMax && u.contextPct >= CONTEXT_WARN;
  if (ctx > 0) parts.push(["context", `Context: ${tok(settled(core).context)}${cu.contextMax ? ` of the model's ${cu.contextMax} window, ${cu.contextPct}%` : ""}${live}`, <>
    <Gauge size={14} strokeWidth={1.5} className="text-faint" /><Num hold={settle} est={est} className={`font-medium ${est ? "" : "text-ink"}`}>{tok(ctx)}</Num>
    {u.contextMax && <><span className="tz-n3 -ml-0.5">of {u.contextMax}</span><span className="h-[5px] w-10 flex-none overflow-hidden rounded-[3px] bg-line-strong">
      <i className={`block h-full rounded-[3px] ${full ? "bg-warn" : "bg-live"}`} style={{ width: `${u.contextPct}%` }} /></span><Num hold={settle} className={full ? "text-warn" : ""}>{u.contextPct}%</Num></>}</>, `${tok(ctx)}${u.contextPct}`]);
  // Claude's own compaction, between replies (CompactButton draws it)
  if (ctx > 0 && onCompact && compactable(core.cache, u, busy)) parts.push(["compact", "", null, compacting ? "Compacting…" : "Compact"]);
  if (paid) parts.push(["cost", paid[2] + live, <><Coins size={14} strokeWidth={1.5} className="text-faint" /><Num hold={settle} est={est} className={`font-medium ${est ? "" : "text-ink"}`}>{paid[0]}</Num>
    {paid[1] && <span className="tz-n2 -ml-0.5">{paid[1]}</span>}</>, paid.join()]);
  if (u.cachedPct > 0) parts.push(["cache", "Input read from the provider's cache in this conversation", <>
    <Database size={14} strokeWidth={1.5} className="text-faint" /><span className="font-medium text-ink">{pctText(u.cachedPct)}%</span><span className="-ml-0.5">cached</span></>, `${u.cachedPct}`]);
  // under a minute and cold in the warning colour: the next message re-reads the whole context
  if (core.cache && warm != null) { const left = warm < 1 ? "<1m" : `${warm}m`;
    parts.push(["warm", warmSaid(core.cache, warm), <>
      <Timer size={14} strokeWidth={1.5} className={warm < 1 ? "text-warn" : "text-faint"} />{warm < 0 ? <span className="text-warn">cold</span>
        : <span className={warm < 1 ? "text-warn" : ""}>warm <span className={`font-medium ${warm < 1 ? "" : "text-ink"}`}>{left}</span></span>}</>, warm < 0 ? "cold" : `warm ${left}`]); }
  if (sp) parts.push(["speed", sp[2], <><Activity size={14} strokeWidth={1.5} className="text-live" /><Num hold={settle} est={est} className={`font-medium ${est ? "" : "text-ink"}`}>{sp[0]}</Num>
    {sp[1] && <span className="-ml-0.5">{sp[1]}</span>}</>, sp.slice(0, 2).join()]);
  // fitted when a part comes or goes or a text's length changes, not every frame
  const shape = parts.map((p) => `${p[0]}${p[3].length}`).join();
  useLayoutEffect(() => { fit(row.current); }, [shape]);
  useEffect(() => { const el = row.current; if (!el) return; const r = new ResizeObserver(() => fit(el)); r.observe(el); return () => r.disconnect(); }, [!parts.length]);
  if (!parts.length) return null;
  // tabIndex: WebKit's Tab skips buttons; data-panel: a press here is not "outside" a work panel open over the conversation
  const part = "flex h-6 items-center gap-1.5 rounded-md px-[7px] hover:bg-hover hover:text-ink-2";
  return (
    <div ref={row} aria-label="This conversation's numbers" role="group"
      className={`tz-nums -ml-4 -mr-3 flex h-8 items-center whitespace-nowrap bg-[color-mix(in_srgb,var(--field)_40%,var(--card))] pl-[9px] pr-[5px] text-[12.5px] tabular-nums text-muted ${top ? "-mt-3 mb-2.5 rounded-t-[11px] border-b border-line" : "-mb-2 mt-2 rounded-b-[11px] border-t border-line"}`}>
      {parts.map(([k, title, inner], i) => (
        <span key={k} className={`flex flex-none items-center ${k === "cache" ? "tz-n1" : k === "warm" ? "tz-n4" : ""}`}>
          {i > 0 && k !== "compact" && <i aria-hidden="true" className="mx-[3px] h-3.5 w-px bg-line" />}
          {k === "compact" ? <CompactButton pending={compacting} onClick={compact} className="ml-0.5 mr-1 h-[22px] px-2 text-[12px]" />
            : k === "cost" && turns.length ? <ChatCost cost={view.cost} turns={turns} busy={busy} className={part} title={`${title}. Opens its receipt`} left>{inner}</ChatCost>
            // the warmth's tooltip is its sentence alone; a click is Stats, as the rest
            : <button tabIndex={0} data-panel onClick={(e) => openStats(k === "speed" ? "turn" : "all", e.currentTarget)} title={k === "warm" ? title : `${title}. Opens Stats`} className={part}>{inner}</button>}
        </span>
      ))}
    </div>
  );
}

/** The running reply's output tokens on its Working row and work fold ("↓ 1.2k tokens", design review 13, D1): its finished
 * calls' as each ends and, with Live stats on, the call in flight's estimate counting with it (eased to the exact figure
 * when the call ends), in a box that only grows, so what follows on the line stays put. */
export function Tokens({ out = 0, live }: { out?: number; live?: Live | null }) {
  const on = useLook(LIVE) !== "off", streaming = on && !!live;
  useFrame(streaming, () => tok(Math.round(out + outNow(live!))));
  const n = Math.round(useTween(out + (streaming ? outNow(live!) : 0), on, streaming, true));
  return n ? <><Sep /><Num hold={on} className="tabular-nums">↓ {tok(n)} tokens</Num></> : null;
}
/** Drops parts until the row fits: the cache's share, the cost's words, "of 128k", the cache's warmth (scrollWidth counts
 * what spills out). The warmth goes last: at 85% of the window, with Compact and a plan's cost words, a 700 px row had no
 * room for it, just when the next message would re-read the most. */
const fit = (el: HTMLElement | null) => { if (!el) return; el.dataset.drop = "0"; for (let d = 1; d <= 4 && el.scrollWidth > el.clientWidth; d++) el.dataset.drop = String(d); };
