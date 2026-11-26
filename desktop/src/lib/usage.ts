// The usage page's numbers (docs/DESKTOP.md 0.2), grouped from the run log's rows (the core's {"op": "usage"}: one row per
// run since the asked second, the last 90 days unless asked). Runs on a plan (the claude CLI on its login, or the own loop on a plan's key: Mistral Vibe, a sign-in;
// the row's `plan`) are counted at API prices, not billed; the rest ran on the own loop against a provider's API: billed.
import { clock } from "./health";
import { pct } from "./types";

/** [ts, model, version, plan, cost, input, cache write, cache read, output, stop, verify, cache ttl, mode, real use, strong
 * signals, soft signals, provider] (src/main.rs usage_rows; the signals from src/realuse.rs; the provider of an API run
 * since 0.1.182, "SubRouter") */
export type Row = [number, string | null, string | null, boolean, number | null, number | null, number | null, number | null, number | null, string | null, string | null, string | null, string | null,
  boolean?, string[]?, string[]?, (string | null)?];

export type Run = { ts: number; day: string; model: string; version: string; plan: boolean; cost: number; read: number; cached: number; real: boolean; strong: string[]; soft: string[]; provider: string | null };

const dayOf = (ts: number) => { const d = new Date(ts * 1000); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`; };

/** One run. The cache's share is in tokens, not dollars: worked back from each run's cost, Anthropic's price ratios gave input
 * prices from a tenth to twenty times the list (2026-11-16, 560 runs), so a dollar figure would be invented. */
export function run(r: Row): Run {
  const [ts, model, version, plan, cost, input, write, read, , , , , , real, strong, soft, provider] = r;
  const i = input ?? 0, w = write ?? 0, cr = read ?? 0;
  return { ts, day: dayOf(ts), model: model || "(default)", version: version || "?", plan, cost: cost ?? 0, read: i + w + cr, cached: cr,
    real: !!real, strong: strong ?? [], soft: soft ?? [], provider: provider ?? null };
}

/** A plan by its provider: "Mistral Vibe plan", "Z.ai Coding Plan" (not a second "plan"), "Claude plan" where the row names
 * none (the claude CLI on its login). Every plan run read "Claude plan" (the user, 2026-11-25). */
export const planName = (provider: string | null) => { const n = provider ?? "Claude"; return / plan$/i.test(n) ? n : `${n} plan`; };

export type Bar = { at: number; plan: number; api: number; runs: number };
export type ByModel = { model: string; plan: boolean; provider: string | null; runs: number; cost: number; cachePct: number };
export type ByVersion = { version: string; model: string; runs: number; missPct: number; softPct: number; top: string };
export type Lane = { lane: string; runs: number; missPct: number; softPct: number };

const vkey = (v: string) => v.split(/\D+/).filter(Boolean).map(Number);
const newer = (a: string, b: string) => { const x = vkey(a), y = vkey(b); for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (y[i] ?? 0) - (x[i] ?? 0); return 0; };

/** The runs that called a model, oldest first: a run that read and wrote nothing (a lean start, a stopped start) is not
 * usage. */
export const used = (rows: Row[]) => rows.filter((r) => (r[5] ?? 0) + (r[6] ?? 0) + (r[7] ?? 0) + (r[8] ?? 0) > 0).map(run).sort((a, b) => a.ts - b.ts);

/** The page's ranges (the menu's order): the last so many seconds, then All (since the first run) and one picked by hand. */
export const RANGES = [["15m", 900, "Last 15 minutes"], ["1h", 3600, "Last hour"], ["3h", 10_800, "Last 3 hours"], ["24h", 86_400, "Last 24 hours"],
  ["7d", 7 * 86_400, "Last 7 days"], ["30d", 30 * 86_400, "Last 30 days"], ["90d", 90 * 86_400, "Last 90 days"]] as const;
/** A range as chosen and kept; a hand-picked one from its first minute to its last (seconds, each a minute's start). */
export type Choice = { key: (typeof RANGES)[number][0] | "all" } | { key: "custom"; from: number; to: number };
/** What one bar holds. */
export type Unit = "minute" | "5 minutes" | "15 minutes" | "hour" | "day" | "week" | "month" | "year";
/** A range's seconds (both ends in it) and its bars' unit. */
export type Span = { from: number; to: number; unit: Unit };

const SECS: Partial<Record<Unit, number>> = { minute: 60, "5 minutes": 300, "15 minutes": 900, hour: 3600 };
// a preset's bars, ~12 to 60 of them (7 days stay 7, 90 stay 90: a day is the unit people count spending in)
const PRESET: Record<(typeof RANGES)[number][0], Unit> = { "15m": "minute", "1h": "5 minutes", "3h": "15 minutes", "24h": "hour", "7d": "day", "30d": "day", "90d": "day" };
/** All and a hand-picked range: the finest unit that draws at most 60 bars, days up to 90 (as the 90 days do: All over 70
 * days drew 10 weeks beside them). */
const unitFor = (secs: number): Unit => secs <= 3600 ? "minute" : secs <= 5 * 3600 ? "5 minutes" : secs <= 15 * 3600 ? "15 minutes" : secs <= 60 * 3600 ? "hour"
  : secs <= 90 * 86_400 ? "day" : secs <= 60 * 7 * 86_400 ? "week" : secs <= 60 * 30.4 * 86_400 ? "month" : "year";

/** The start of the bar `t` falls in, on this machine's clock: minutes and hours on the local clock's marks, days from a
 * local midnight (23 or 25 hours on a day the clocks change), weeks from a Monday, months from the 1st. */
export function floorTo(t: number, unit: Unit): number {
  const d = new Date(t * 1000), s = SECS[unit];
  if (s && s < 3600) { d.setSeconds(0, 0); d.setMinutes(d.getMinutes() - (d.getMinutes() % (s / 60))); }
  else if (unit === "hour") d.setMinutes(0, 0, 0);
  else {
    d.setHours(0, 0, 0, 0);
    if (unit === "week") d.setDate(d.getDate() - ((d.getDay() + 6) % 7));
    if (unit === "month") d.setDate(1);
    if (unit === "year") d.setMonth(0, 1);
  }
  return d.getTime() / 1000;
}
/** The next bar's start: minutes by the clock's seconds; an hour on the local clock's next mark (Lord Howe's clocks move
 * half an hour: that hour is a half), and an hour the clocks go back two bars, as it was two hours; days, weeks, months
 * and years by the calendar. */
function next(t: number, unit: Unit): number {
  const s = SECS[unit];
  if (unit === "hour") { const n = floorTo(t + 3600, "hour"); return n > t ? n : t + 3600; }
  if (s) return t + s;
  const d = new Date(t * 1000);
  if (unit === "year") d.setFullYear(d.getFullYear() + 1, 0, 1); else if (unit === "month") d.setMonth(d.getMonth() + 1, 1); else d.setDate(d.getDate() + (unit === "week" ? 7 : 1));
  d.setHours(0, 0, 0, 0); // a midnight the clocks skip is 1am: the next day starts at its own midnight again
  return d.getTime() / 1000;
}

/** A choice's seconds and bars at `now`, `first` the first run's time: the last minutes and hours up to now (the first bar
 * a part of one); days from a midnight (7 days: today and the six before, seven bars, as the page drew them before); All
 * from the first run; a hand-picked range to the end of its last minute. */
export function spanOf(c: Choice, now: number, first?: number): Span {
  if (c.key === "custom") return { from: c.from, to: c.to + 59, unit: unitFor(c.to + 60 - c.from) };
  if (c.key === "all") { const from = Math.min(first ?? now, now); return { from, to: now, unit: unitFor(now - from) }; }
  const [, secs] = RANGES.find(([k]) => k === c.key)!, unit = PRESET[c.key];
  if (unit !== "day") return { from: now - secs, to: now, unit };
  const d = new Date(now * 1000);
  d.setDate(d.getDate() - (secs / 86_400 - 1));
  return { from: floorTo(d.getTime() / 1000, "day"), to: now, unit };
}

/** The first second the page needs rows from: the core's 90 days (`base`), or further back for All and an older range
 * picked by hand; never before 1970 (the core reads no negative second, and answered 90 days to one). */
export const needOf = (c: Choice, base: number) => Math.max(0, Math.floor(c.key === "all" ? 0 : c.key === "custom" ? Math.min(base, c.from) : base));
/** The start page's rows: the last 90 days, whatever the usage page asked for (its All fed the route's estimate). */
export const lastDays = (rows: Row[], now = Date.now() / 1000) => rows.filter((r) => r[0] >= now - 90 * 86_400);

/** The page for a span (seconds), from `used`'s runs. */
export function summarize(all: Run[], sp: Span) {
  const runs = all.filter((r) => r.ts >= sp.from && r.ts <= sp.to);
  const sum = (xs: Run[], f: (r: Run) => number) => xs.reduce((a, r) => a + f(r), 0);
  const group = (xs: Run[], key: (r: Run) => string) => { const m = new Map<string, Run[]>(); for (const r of xs) { const k = key(r), g = m.get(k); if (g) g.push(r); else m.set(k, [r]); } return m; };
  // every bar of the range, empty ones too (a bar chart's gaps are information)
  const bars: Bar[] = [];
  for (let t = floorTo(sp.from, sp.unit); t <= sp.to; t = next(t, sp.unit)) bars.push({ at: t, plan: 0, api: 0, runs: 0 });
  for (const r of runs) {
    let lo = 0, hi = bars.length - 1;
    while (lo < hi) { const mid = (lo + hi + 1) >> 1; if (bars[mid].at <= r.ts) lo = mid; else hi = mid - 1; }
    bars[lo][r.plan ? "plan" : "api"] += r.cost; bars[lo].runs++;
  }
  const byModel: ByModel[] = [...group(runs, (r) => `${r.plan}|${r.model}|${r.provider ?? ""}`).values()].map((rs) => {
    const read = sum(rs, (r) => r.read);
    return { model: rs[0].model, plan: rs[0].plan, provider: rs[0].provider, runs: rs.length, cost: sum(rs, (r) => r.cost), cachePct: read ? Math.round((100 * sum(rs, (r) => r.cached)) / read) : 0 };
  }).sort((a, b) => b.cost - a.cost);
  // real use only (your own work, src/realuse.rs): how often a run missed (strong signals) or met pushback (soft only)
  const mine = runs.filter((r) => r.real);
  const rate = (rs: Run[]) => ({ runs: rs.length, missPct: Math.round((100 * rs.filter((r) => r.strong.length).length) / rs.length),
    softPct: Math.round((100 * rs.filter((r) => r.soft.length && !r.strong.length).length) / rs.length) });
  const byVersion: ByVersion[] = [...group(mine, (r) => `${r.version}|${r.model}`).values()].map((rs) => {
    const count = new Map<string, number>();
    for (const s of rs.flatMap((r) => [...r.strong, ...r.soft])) count.set(s, (count.get(s) ?? 0) + 1);
    const top = [...count.entries()].sort((a, b) => b[1] - a[1]).slice(0, 3).map(([s, n]) => `${s} ${n}`).join(", ");
    return { version: rs[0].version, model: rs[0].model, ...rate(rs), top };
  }).sort((a, b) => newer(a.version, b.version) || b.runs - a.runs);
  // the plans apart, by provider, the costliest first
  const plans = [...group(runs.filter((r) => r.plan), (r) => planName(r.provider))].map(([name, rs]) => ({ name, cost: sum(rs, (r) => r.cost), runs: rs.length })).sort((a, b) => b.cost - a.cost);
  const minePlans = new Set(mine.filter((r) => r.plan).map((r) => planName(r.provider)));
  const lanes: Lane[] = [[minePlans.size === 1 ? [...minePlans][0] : "Plans", mine.filter((r) => r.plan)], ["API providers", mine.filter((r) => !r.plan)]]
    .filter(([, rs]) => (rs as Run[]).length).map(([lane, rs]) => ({ lane: lane as string, ...rate(rs as Run[]) }));
  return {
    runs: runs.length, api: sum(runs.filter((r) => !r.plan), (r) => r.cost), plan: sum(runs.filter((r) => r.plan), (r) => r.cost),
    cached: sum(runs, (r) => r.cached), read: sum(runs, (r) => r.read), bars, byModel, byVersion, lanes, plans,
  };
}

/** A date as the window says it ("Nov 6"), with its year when that is not this one's ("Nov 6, 2025"). */
export const dateName = (t: number, now = Date.now() / 1000) => new Date(t * 1000).toLocaleDateString([], { month: "short", day: "numeric",
  year: new Date(t * 1000).getFullYear() === new Date(now * 1000).getFullYear() ? undefined : "numeric" });
/** A time with its date where it is not today ("2pm", "Nov 25, 2pm"; the app's one clock). */
export const timeName = (t: number, now = Date.now() / 1000) => (dayOf(t) === dayOf(now) ? "" : `${dateName(t, now)}, `) + clock(new Date(t * 1000));
/** A bar's start in its unit: "9:35pm", "Nov 25, 2pm", "Nov 6", "week of Nov 3", "Nov 2026", "2026". */
export const barName = (t: number, unit: Unit, now = Date.now() / 1000) => SECS[unit] ? timeName(t, now) : unit === "day" ? dateName(t, now)
  : unit === "week" ? `week of ${dateName(t, now)}` : unit === "year" ? String(new Date(t * 1000).getFullYear()) : new Date(t * 1000).toLocaleDateString([], { month: "short", year: "numeric" });
/** "max $X a day": a bar's unit after "a", or "per 5 minutes". */
export const perUnit = (unit: Unit) => (/^\d/.test(unit) ? `per ${unit}` : unit === "hour" ? "an hour" : `a ${unit}`);

/** What the page says about its range: the button ("Last hour", "Nov 20, 2pm – Nov 26, 1pm"), the line under the heading
 * ("The last hour from the run log.", "Everything in the run log, since Nov 3 (the first run).") and an empty one's words
 * ("in the last hour"). */
export function rangeWords(c: Choice, sp: Span, now = Date.now() / 1000) {
  // both ends with their dates: "Nov 20, 2pm – 1pm" read as one day
  const at = (t: number) => `${dateName(t, now)}, ${clock(new Date(t * 1000))}`;
  if (c.key === "custom") { const a = at(c.from), b = at(c.to); return { button: `${a} – ${b}`, note: `${a} to ${b} from the run log.`, within: `from ${a} to ${b}` }; }
  if (c.key === "all") return { button: "All time", note: `Everything in the run log, since ${dateName(sp.from, now)} (the first run).`, within: `since ${dateName(sp.from, now)}` };
  const label = RANGES.find(([k]) => k === c.key)![2];
  return { button: label, note: `The ${label.toLowerCase()} from the run log.`, within: `in the ${label.toLowerCase()}` };
}

/** A datetime-local field's value for a second, and back (this machine's clock; NaN for a cleared field and a year before
 * 1970: years 1 to 99 read as 1901 to 1999, and a second before 1970 is none the core reads). */
export const fieldOf = (t: number) => { const d = new Date(t * 1000), p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`; };
export const fromField = (v: string) => { const m = v.match(/^(\d{4})-(\d\d)-(\d\d)T(\d\d):(\d\d)/); return m && +m[1] >= 1970 ? new Date(+m[1], +m[2] - 1, +m[3], +m[4], +m[5]).getTime() / 1000 : NaN; };

/** Today on this machine, for the start page's ledger line: the runs that called a model since local midnight, what
 * they cost on paid APIs, and the share of their input that came from the cache. Null without a run today: the page
 * shows no line then, never a zero. */
export function today(rows: Row[] | null, now = Date.now() / 1000): { runs: number; paid: number; cachePct: number | null } | null {
  const day = dayOf(now);
  const runs = used(rows ?? []).filter((r) => r.day === day);
  if (!runs.length) return null;
  // a share that says nothing (no input read, or none of it from the cache) is null: the line leaves it out
  const read = runs.reduce((a, r) => a + r.read, 0), cached = runs.reduce((a, r) => a + r.cached, 0);
  return { runs: runs.length, paid: runs.reduce((a, r) => a + (r.plan ? 0 : r.cost), 0), cachePct: cached ? pct(cached, read) || null : null };
}

/** A check of the grouping: `npx tsx -e 'import("./src/lib/usage.ts").then(m => m.demo())'`. */
export function demo() {
  // a noon on this machine, whatever its time zone: a fixed second was another zone's midnight, and the day's runs fell
  // on two days there
  const now = new Date(2027, 0, 15, 12).getTime() / 1000;
  const rows: Row[] = [
    [now - 100, "claude-opus-5-5", "0.1.130", true, 0.5, 0, 0, 1_000_000, 0, "completed", "claimed", "1h", "live", true, [], ["pushback"]],
    [now - 200, "deepseek-v4.1-flash", "0.1.130", false, 0.01, 1000, 0, 0, 100, "stopped", "claimed", null, "live", true, ["stopped", "correction"], []],
    [now - 300, "deepseek-v4.1-flash", "0.1.131", false, 0.02, 1000, 0, 0, 100, "completed", "verify_failed", null, "headless", false, [], []],
    [now - 40 * 86400, "claude-opus-5-5", "0.1.120", true, 9, 10, 0, 0, 0, "completed", null, "1h", "live"],
    [now - 50, "", "0.1.138", false, 0, 0, 0, 0, 0, "completed", "claimed", "1h", "live"],
  ];
  const d30 = spanOf({ key: "30d" }, now), sum = (rs: Row[]) => summarize(used(rs), d30);
  const s = sum(rows);
  const near = (a: number, b: number) => Math.abs(a - b) < 1e-9;
  const ok = s.runs === 3 && near(s.api, 0.03) && near(s.plan, 0.5) && s.cached === 1_000_000 && s.read === 1_002_000 && s.bars.length === 30
    && s.bars[29].runs === 3 && s.byModel[0].model === "claude-opus-5-5" && s.byModel[1].runs === 2
    && s.byVersion.length === 2 && s.byVersion[0].version === "0.1.130" && s.byVersion.find((v) => v.model.startsWith("deepseek"))!.missPct === 100
    && s.byVersion.find((v) => v.model.startsWith("claude"))!.softPct === 100 && s.lanes.length === 2 && s.lanes[1].missPct === 100
    && s.byVersion.find((v) => v.model.startsWith("deepseek"))!.top === "stopped 1, correction 1";
  if (!ok) throw new Error(`usage: ${JSON.stringify(s)}`);
  // the ledger: today's three runs (the empty one is no run), the paid ones' cost, the cache's share; none the day after
  const t = today(rows, now);
  if (!t || t.runs !== 3 || !near(t.paid, 0.03) || t.cachePct !== 99 || today(rows, now + 86400) !== null || today(rows.slice(1, 3), now)!.cachePct !== null) throw new Error(`today: ${JSON.stringify(t)}`);
  // a claude run on an API key: the run log says it is not the plan (core, design review 12), so it is paid, to Anthropic
  const keyed: Row = [now - 150, "claude-opus-5-5", "0.1.193", false, 0.42, 2000, 0, 0, 500, "completed", "claimed", "5m", "live", true, [], [], "Anthropic API"];
  const k = today([...rows, keyed], now), byKey = sum([...rows, keyed]).byModel.find((m) => m.provider === "Anthropic API");
  if (!k || !near(k.paid, 0.45) || !byKey || byKey.plan || !near(byKey.cost, 0.42)) throw new Error(`keyed: ${JSON.stringify([k, byKey])}`);
  // a plan's own loop (Mistral Vibe): plan money under its own name, apart from Claude's; not paid on the start page
  const vibe: Row = [now - 120, "deepseek-v4.1-flash-1017", "0.1.226", true, 0.05, 1000, 0, 9000, 300, "completed", "claimed", null, "live", true, [], [], "Mistral Vibe"];
  const v = sum([...rows, vibe]);
  if (JSON.stringify(v.plans) !== JSON.stringify([{ name: "Claude plan", cost: 0.5, runs: 1 }, { name: "Mistral Vibe plan", cost: 0.05, runs: 1 }]) || v.lanes[0].lane !== "Plans"
    || !near(today([...rows, vibe], now)!.paid, 0.03) || planName("Z.ai Coding Plan") !== "Z.ai Coding Plan" || s.lanes[0].lane !== "Claude plan") throw new Error(`plans: ${JSON.stringify(v.plans)}`);
  ranges(now);
  console.log("usage ok");
}

/** The ranges' bars and edges, in this machine's zone (run it under TZ=America/New_York and TZ=Asia/Kolkata too): each
 * preset's unit and bar count, All and a hand-picked range by their span, a run on each edge, a range over midnight, and the
 * days the clocks change (23 and 25 hours: an hour bar per real hour, a day bar from each day's start, 1am where the
 * clocks skip midnight). */
function ranges(now: number) {
  const fail = (what: string, got: unknown) => { throw new Error(`ranges, ${what}: ${JSON.stringify(got)}`); };
  const at = (y: number, m: number, d: number, h = 0, mi = 0, sec = 0) => new Date(y, m, d, h, mi, sec).getTime() / 1000;
  const row = (ts: number, cost = 0.01): Row => [ts, "m", "0.1.1", false, cost, 1, 0, 0, 1, null, null, null, "live"];
  const page = (c: Choice, rs: Row[], first?: number) => summarize(used(rs), spanOf(c, now, first));
  // each preset: its unit, its bars (the minutes and hours a part bar at the start, the days whole from a midnight)
  const want: Record<string, [Unit, number[]]> = { "15m": ["minute", [15, 16]], "1h": ["5 minutes", [12, 13]], "3h": ["15 minutes", [12, 13]], "24h": ["hour", [24, 25]],
    "7d": ["day", [7]], "30d": ["day", [30]], "90d": ["day", [90]] };
  for (const [k] of RANGES) {
    const sp = spanOf({ key: k }, now), n = summarize([], sp).bars.length;
    if (sp.unit !== want[k][0] || !want[k][1].includes(n)) fail(k, [sp.unit, n]);
  }
  // All by its span from the first run; a hand-picked range by its own
  const unitAll = (back: number) => spanOf({ key: "all" }, now, now - back).unit;
  if (unitAll(1800) !== "minute" || unitAll(40 * 86_400) !== "day" || unitAll(200 * 86_400) !== "week" || unitAll(3 * 365 * 86_400) !== "month" || spanOf({ key: "all" }, now).from !== now)
    fail("all", [unitAll(1800), unitAll(40 * 86_400), unitAll(200 * 86_400), unitAll(3 * 365 * 86_400)]);
  for (const [secs, unit] of [[2 * 3600, "5 minutes"], [12 * 3600, "15 minutes"], [30 * 3600, "hour"], [10 * 86_400, "day"], [100 * 86_400, "week"], [500 * 86_400, "month"]] as const) {
    const sp = spanOf({ key: "custom", from: now - secs, to: now - 60 }, now), n = summarize([], sp).bars.length;
    if (sp.unit !== unit || n > 61) fail(`custom ${secs}s`, [sp.unit, n]);
  }
  // the edges: 15 minutes take a run 900 s old, not 901; 7 days take the midnight six days back, not the second before it
  const q = page({ key: "15m" }, [row(now - 900), row(now - 901), row(now)]);
  if (q.runs !== 2 || q.bars.reduce((a, b) => a + b.runs, 0) !== 2) fail("15 minutes' edge", q.runs);
  const six = new Date(now * 1000); six.setDate(six.getDate() - 6); six.setHours(0, 0, 0, 0);
  const w = page({ key: "7d" }, [row(six.getTime() / 1000), row(six.getTime() / 1000 - 1)]);
  if (w.runs !== 1 || w.bars[0].runs !== 1) fail("7 days' edge", w);
  // a hand-picked range over midnight, 10pm to 1:59am (its last minute whole): 5-minute bars, the runs either side of
  // midnight in bars of their own, those outside it left out
  const ten = at(2027, 0, 14, 22), late = { key: "custom", from: ten, to: at(2027, 0, 15, 1, 59) } as const;
  const mid = page(late, [row(ten - 1), row(ten), row(at(2027, 0, 14, 23, 59, 30)), row(at(2027, 0, 15, 0, 0, 10)), row(at(2027, 0, 15, 1, 59, 59)), row(at(2027, 0, 15, 2))]);
  const hit = mid.bars.filter((b) => b.runs).map((b) => new Date(b.at * 1000).getHours() * 60 + new Date(b.at * 1000).getMinutes());
  if (mid.runs !== 4 || mid.bars.length !== 48 || JSON.stringify(hit) !== JSON.stringify([22 * 60, 23 * 60 + 55, 0, 60 + 55])) fail("over midnight", [mid.runs, mid.bars.length, hit]);
  // the days the clocks change, where this zone has them (none: a 24-hour day, checked the same way)
  const days = Array.from({ length: 365 }, (_, i) => at(2026, 0, 1 + i));
  const odd = days.filter((d, i) => i < 364 && days[i + 1] - d !== 86_400);
  for (const d of [...odd, at(2026, 6, 1)]) {
    const [y, m, dd] = [new Date(d * 1000).getFullYear(), new Date(d * 1000).getMonth(), new Date(d * 1000).getDate()], end = at(y, m, dd + 1), hours = Math.ceil((end - d) / 3600);
    // a run each half hour: every one in one bar, a bar holding as many as its half hours (Lord Howe's 24.5-hour day has a
    // half-hour bar), a bar per hour of the clock and each on its mark but the one the clocks moved to
    const runs = Array.from({ length: (end - d) / 1800 }, (_, k) => row(d + k * 1800 + 900));
    const day = page({ key: "custom", from: d, to: end - 60 }, runs), len = (i: number) => (day.bars[i + 1]?.at ?? end) - day.bars[i].at;
    if (day.bars.length !== hours || day.runs !== runs.length || day.bars.some((b, i) => b.runs !== len(i) / 1800 || len(i) > 3600)
      || day.bars.filter((b) => new Date(b.at * 1000).getMinutes()).length > 1) fail(`the day of ${new Date(d * 1000)}`, [hours, day.bars.length, day.bars.map((b) => b.runs)]);
    const around = page({ key: "custom", from: at(y, m, dd - 4), to: at(y, m, dd + 6) - 60 }, [row(d), row(end - 1)]);
    if (around.bars.length !== 10 || around.bars.some((b, i) => b.at !== at(y, m, dd - 4 + i)) || around.runs !== 2 || around.bars.filter((b) => b.runs).length !== 1) fail(`the days around ${new Date(d * 1000)}`, around.bars.map((b) => new Date(b.at * 1000).toString()));
  }
  // weeks from a Monday, months from the 1st, at midnight
  const wk = summarize([], spanOf({ key: "custom", from: at(2026, 0, 7), to: at(2026, 5, 30) }, now)).bars, mo = summarize([], spanOf({ key: "custom", from: at(2025, 0, 7), to: at(2026, 5, 30) }, now)).bars;
  if (wk.some((b) => new Date(b.at * 1000).getDay() !== 1 || new Date(b.at * 1000).getHours()) || mo.some((b) => new Date(b.at * 1000).getDate() !== 1 || new Date(b.at * 1000).getHours()) || mo.length !== 18)
    fail("weeks and months", [wk.map((b) => new Date(b.at * 1000).toString()).slice(0, 3), mo.length]);
  // All over 61 to 90 days by the day, as the 90 days; a 26-year range by the year, never hundreds of bars
  const long = summarize([], spanOf({ key: "custom", from: at(2000, 0, 1), to: at(2026, 0, 1) }, now)).bars;
  if (unitAll(75 * 86_400) !== "day" || unitAll(91 * 86_400) !== "week" || long.length !== 27 || barName(long[0].at, "year", now) !== "2000" || long.some((b) => new Date(b.at * 1000).getMonth() || new Date(b.at * 1000).getDate() !== 1))
    fail("long ranges", [unitAll(75 * 86_400), unitAll(91 * 86_400), long.length]);
  // never before 1970: a field's year 50 (1950 to new Date) or 1969 is no time; the page's start not below 0
  if (!Number.isNaN(fromField("0050-01-01T00:00")) || !Number.isNaN(fromField("1969-12-31T23:59")) || fromField("1970-01-02T00:00") !== at(1970, 0, 2)
    || needOf({ key: "custom", from: -86_400, to: 0 }, 5000) !== 0 || needOf({ key: "all" }, 5000) !== 0 || needOf({ key: "7d" }, 5000) !== 5000 || needOf({ key: "custom", from: 6000.5, to: 7000 }, 5000) !== 5000)
    fail("before 1970", [fromField("0050-01-01T00:00"), fromField("1969-12-31T23:59")]);
  // the start page keeps 90 days of what the usage page asked for
  if (lastDays([row(now - 91 * 86_400), row(now - 89 * 86_400)], now).length !== 1) fail("the start page's 90 days", null);
  // the words
  const words = rangeWords({ key: "1h" }, spanOf({ key: "1h" }, now), now);
  if (words.button !== "Last hour" || words.note !== "The last hour from the run log." || words.within !== "in the last hour" || perUnit("5 minutes") !== "per 5 minutes" || perUnit("hour") !== "an hour" || perUnit("week") !== "a week" || perUnit("year") !== "a year"
    || rangeWords({ key: "all" }, spanOf({ key: "all" }, now, now - 86_400), now).within !== `since ${dateName(now - 86_400, now)}` || rangeWords(late, spanOf(late, now), now).button !== `${dateName(ten, now)}, 10pm – ${dateName(late.to, now)}, 1:59am` || fromField(fieldOf(ten)) !== ten || !Number.isNaN(fromField("")))
    fail("words", words);
}
