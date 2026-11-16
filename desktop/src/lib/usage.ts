// The usage page's numbers (docs/DESKTOP.md 0.2), grouped from the run log's rows (the core's {"op": "usage"}: one row per
// run, last 90 days). Runs on the claude CLI are the plan: their dollars are claude's estimate at API prices, not billed.
// The rest ran on the own loop against a provider's API: billed.

/** [ts, model, version, plan, cost, input, cache write, cache read, output, stop, verify, cache ttl, mode, real use, strong
 * signals, soft signals] (src/main.rs usage_rows; the signals from src/realuse.rs) */
export type Row = [number, string | null, string | null, boolean, number | null, number | null, number | null, number | null, number | null, string | null, string | null, string | null, string | null,
  boolean?, string[]?, string[]?];

export type Run = { ts: number; day: string; model: string; version: string; plan: boolean; cost: number; read: number; cached: number; real: boolean; strong: string[]; soft: string[] };

const dayOf = (ts: number) => { const d = new Date(ts * 1000); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`; };

/** One run. The cache's share is in tokens, not dollars: worked back from each run's cost, Anthropic's price ratios gave input
 * prices from a tenth to twenty times the list (2026-11-16, 560 runs), so a dollar figure would be invented. */
export function run(r: Row): Run {
  const [ts, model, version, plan, cost, input, write, read, , , , , , real, strong, soft] = r;
  const i = input ?? 0, w = write ?? 0, cr = read ?? 0;
  return { ts, day: dayOf(ts), model: model || "(default)", version: version || "?", plan, cost: cost ?? 0, read: i + w + cr, cached: cr,
    real: !!real, strong: strong ?? [], soft: soft ?? [] };
}

export type Day = { day: string; plan: number; api: number; runs: number };
export type ByModel = { model: string; plan: boolean; runs: number; cost: number; cachePct: number };
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
  for (const r of runs) { const k = `${r.plan}|${r.model}`; models.set(k, [...(models.get(k) ?? []), r]); }
  const byModel: ByModel[] = [...models.values()].map((rs) => {
    const read = sum(rs, (r) => r.read);
    return { model: rs[0].model, plan: rs[0].plan, runs: rs.length, cost: sum(rs, (r) => r.cost), cachePct: read ? Math.round((100 * sum(rs, (r) => r.cached)) / read) : 0 };
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
  const lanes: Lane[] = [["Claude (plan)", mine.filter((r) => r.plan)], ["own loop (API)", mine.filter((r) => !r.plan)]]
    .filter(([, rs]) => (rs as Run[]).length).map(([lane, rs]) => ({ lane: lane as string, ...rate(rs as Run[]) }));
  return {
    runs: runs.length, api: sum(runs.filter((r) => !r.plan), (r) => r.cost), plan: sum(runs.filter((r) => r.plan), (r) => r.cost),
    cached: sum(runs, (r) => r.cached), read: sum(runs, (r) => r.read), days: [...byDay.values()], byModel, byVersion, lanes,
  };
}

/** A check of the grouping: `npx tsx -e 'import("./src/lib/usage.ts").then(m => m.demo())'`. */
export function demo() {
  const now = 1_800_000_000;
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
  console.log("usage ok");
}
