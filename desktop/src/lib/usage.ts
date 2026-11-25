// The usage page's numbers (docs/DESKTOP.md 0.2), grouped from the run log's rows (the core's {"op": "usage"}: one row per
// run, last 90 days). Runs on a plan (the claude CLI on its login, or the own loop on a plan's key: Mistral Vibe, a sign-in;
// the row's `plan`) are counted at API prices, not billed; the rest ran on the own loop against a provider's API: billed.
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

export type Day = { day: string; plan: number; api: number; runs: number };
export type ByModel = { model: string; plan: boolean; provider: string | null; runs: number; cost: number; cachePct: number };
export type ByVersion = { version: string; model: string; runs: number; missPct: number; softPct: number; top: string };
export type Lane = { lane: string; runs: number; missPct: number; softPct: number };

const vkey = (v: string) => v.split(/\D+/).filter(Boolean).map(Number);
const newer = (a: string, b: string) => { const x = vkey(a), y = vkey(b); for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (y[i] ?? 0) - (x[i] ?? 0); return 0; };

/** The page for the last `days` days up to `now` (seconds). */
export function summarize(rows: Row[], days: number, now = Date.now() / 1000) {
  // a run that read and wrote nothing called no model (a lean start, a stopped start): not usage
  const runs = rows.filter((r) => (r[5] ?? 0) + (r[6] ?? 0) + (r[7] ?? 0) + (r[8] ?? 0) > 0).map(run).filter((r) => r.ts >= now - days * 86400);
  const sum = (xs: Run[], f: (r: Run) => number) => xs.reduce((a, r) => a + f(r), 0);
  // every day of the range, empty ones too (a bar chart's gaps are information)
  const byDay = new Map<string, Day>();
  for (let k = days - 1; k >= 0; k--) { const d = dayOf(now - k * 86400); byDay.set(d, { day: d, plan: 0, api: 0, runs: 0 }); }
  for (const r of runs) { const d = byDay.get(r.day); if (d) { d[r.plan ? "plan" : "api"] += r.cost; d.runs++; } }
  const models = new Map<string, Run[]>();
  for (const r of runs) { const k = `${r.plan}|${r.model}|${r.provider ?? ""}`; models.set(k, [...(models.get(k) ?? []), r]); }
  const byModel: ByModel[] = [...models.values()].map((rs) => {
    const read = sum(rs, (r) => r.read);
    return { model: rs[0].model, plan: rs[0].plan, provider: rs[0].provider, runs: rs.length, cost: sum(rs, (r) => r.cost), cachePct: read ? Math.round((100 * sum(rs, (r) => r.cached)) / read) : 0 };
  }).sort((a, b) => b.cost - a.cost);
  // real use only (your own work, src/realuse.rs): how often a run missed (strong signals) or met pushback (soft only)
  const mine = runs.filter((r) => r.real);
  const rate = (rs: Run[]) => ({ runs: rs.length, missPct: Math.round((100 * rs.filter((r) => r.strong.length).length) / rs.length),
    softPct: Math.round((100 * rs.filter((r) => r.soft.length && !r.strong.length).length) / rs.length) });
  const versions = new Map<string, Run[]>();
  for (const r of mine) { const k = `${r.version}|${r.model}`; versions.set(k, [...(versions.get(k) ?? []), r]); }
  const byVersion: ByVersion[] = [...versions.values()].map((rs) => {
    const count = new Map<string, number>();
    for (const s of rs.flatMap((r) => [...r.strong, ...r.soft])) count.set(s, (count.get(s) ?? 0) + 1);
    const top = [...count.entries()].sort((a, b) => b[1] - a[1]).slice(0, 3).map(([s, n]) => `${s} ${n}`).join(", ");
    return { version: rs[0].version, model: rs[0].model, ...rate(rs), top };
  }).sort((a, b) => newer(a.version, b.version) || b.runs - a.runs);
  // the plans apart, by provider, the costliest first
  const by = new Map<string, Run[]>();
  for (const r of runs.filter((r) => r.plan)) by.set(planName(r.provider), [...(by.get(planName(r.provider)) ?? []), r]);
  const plans = [...by].map(([name, rs]) => ({ name, cost: sum(rs, (r) => r.cost), runs: rs.length })).sort((a, b) => b.cost - a.cost);
  const minePlans = new Set(mine.filter((r) => r.plan).map((r) => planName(r.provider)));
  const lanes: Lane[] = [[minePlans.size === 1 ? [...minePlans][0] : "Plans", mine.filter((r) => r.plan)], ["API providers", mine.filter((r) => !r.plan)]]
    .filter(([, rs]) => (rs as Run[]).length).map(([lane, rs]) => ({ lane: lane as string, ...rate(rs as Run[]) }));
  return {
    runs: runs.length, api: sum(runs.filter((r) => !r.plan), (r) => r.cost), plan: sum(runs.filter((r) => r.plan), (r) => r.cost),
    cached: sum(runs, (r) => r.cached), read: sum(runs, (r) => r.read), days: [...byDay.values()], byModel, byVersion, lanes, plans,
  };
}

/** Today on this machine, for the start page's ledger line: the runs that called a model since local midnight, what
 * they cost on paid APIs, and the share of their input that came from the cache. Null without a run today: the page
 * shows no line then, never a zero. */
export function today(rows: Row[] | null, now = Date.now() / 1000): { runs: number; paid: number; cachePct: number | null } | null {
  const day = dayOf(now);
  const runs = (rows ?? []).filter((r) => (r[5] ?? 0) + (r[6] ?? 0) + (r[7] ?? 0) + (r[8] ?? 0) > 0).map(run).filter((r) => r.day === day);
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
  const s = summarize(rows, 30, now);
  const near = (a: number, b: number) => Math.abs(a - b) < 1e-9;
  const ok = s.runs === 3 && near(s.api, 0.03) && near(s.plan, 0.5) && s.cached === 1_000_000 && s.read === 1_002_000 && s.days.length === 30
    && s.days[29].runs === 3 && s.byModel[0].model === "claude-opus-5-5" && s.byModel[1].runs === 2
    && s.byVersion.length === 2 && s.byVersion[0].version === "0.1.130" && s.byVersion.find((v) => v.model.startsWith("deepseek"))!.missPct === 100
    && s.byVersion.find((v) => v.model.startsWith("claude"))!.softPct === 100 && s.lanes.length === 2 && s.lanes[1].missPct === 100
    && s.byVersion.find((v) => v.model.startsWith("deepseek"))!.top === "stopped 1, correction 1";
  if (!ok) throw new Error(`usage: ${JSON.stringify(s)}`);
  // the ledger: today's three runs (the empty one is no run), the paid ones' cost, the cache's share; none the day after
  const t = today(rows, now);
  if (!t || t.runs !== 3 || !near(t.paid, 0.03) || t.cachePct !== 99 || today(rows, now + 86400) !== null || today(rows.slice(1, 3), now)!.cachePct !== null) throw new Error(`today: ${JSON.stringify(t)}`);
  // a claude run on an API key: the run log says it is not the plan (core, design review 12), so it is paid, to Anthropic
  const keyed: Row = [now - 150, "claude-opus-5-5", "0.1.193", false, 0.42, 2000, 0, 0, 500, "completed", "claimed", "5m", "live", true, [], [], "Anthropic API"];
  const k = today([...rows, keyed], now), byKey = summarize([...rows, keyed], 30, now).byModel.find((m) => m.provider === "Anthropic API");
  if (!k || !near(k.paid, 0.45) || !byKey || byKey.plan || !near(byKey.cost, 0.42)) throw new Error(`keyed: ${JSON.stringify([k, byKey])}`);
  // a plan's own loop (Mistral Vibe): plan money under its own name, apart from Claude's; not paid on the start page
  const vibe: Row = [now - 120, "deepseek-v4.1-flash-1017", "0.1.226", true, 0.05, 1000, 0, 9000, 300, "completed", "claimed", null, "live", true, [], [], "Mistral Vibe"];
  const v = summarize([...rows, vibe], 30, now);
  if (JSON.stringify(v.plans) !== JSON.stringify([{ name: "Claude plan", cost: 0.5, runs: 1 }, { name: "Mistral Vibe plan", cost: 0.05, runs: 1 }]) || v.lanes[0].lane !== "Plans"
    || !near(today([...rows, vibe], now)!.paid, 0.03) || planName("Z.ai Coding Plan") !== "Z.ai Coding Plan" || s.lanes[0].lane !== "Claude plan") throw new Error(`plans: ${JSON.stringify(v.plans)}`);
  console.log("usage ok");
}
