// Routing that explains itself (docs/DESKTOP.md 0.3): the line under the message box says where a message goes and what it
// costs before it is sent. Claude on the plan: the 5-hour window's use (and a cheaper own-loop model offered when it runs
// low); a free model: free; a paid one: about what a message cost on it before (the run log's median, else this
// conversation's), else its price per million tokens.
import type { ModelChoice, View } from "./tazuna";
import type { Row } from "./usage";
import { modelLabel, usd } from "./types";

export type RouteLine = { text: string; paid: boolean; low?: string; offer?: ModelChoice; est?: number };

export const isFree = (c: ModelChoice) => c.price === "free" || /^0(\.0+)? \/ 0(\.0+)?$/.test(c.price.trim());

const median = (xs: number[]) => { const s = [...xs].sort((a, b) => a - b); return s.length ? s[Math.floor(s.length / 2)] : undefined; };

/** What a message to `c` cost before: the median of its own-loop runs in your own use (3 or more), else the mean of this
 * conversation's finished turns. */
export function estimate(c: ModelChoice, v: View | null, rows: Row[] | null): number | undefined {
  const runs = (rows ?? []).filter((r) => r[1] === c.model && !r[3] && r[12] !== "headless" && (r[4] ?? 0) > 0).map((r) => r[4] ?? 0);
  if (runs.length >= 3) return median(runs);
  const done = (v?.blocks ?? []).flatMap((b) => (b.kind === "done" && b.cost > 0 ? [b.cost] : []));
  return done.length ? done.reduce((a, b) => a + b, 0) / done.length : undefined;
}

export function routeLine(c: ModelChoice | undefined, v: View | null, rows: Row[] | null, catalog: ModelChoice[]): RouteLine | null {
  if (!c) return null;
  if (!c.url) {
    const used = v?.limits ? Math.round(v.limits[0]) : null;
    const line: RouteLine = { text: `${modelLabel(c.model)} on your Claude plan${used != null ? `, ${used}% of 5 hours used` : ""}`, paid: false };
    if (used != null && used >= 90) {
      line.low = "The plan's 5 hours are nearly used up.";
      // the own-loop model you used most in the last 30 days, if it can be picked now
      const since = Date.now() / 1000 - 30 * 86400, count = new Map<string, number>();
      for (const r of rows ?? []) if (!r[3] && r[0] >= since && r[1]) count.set(r[1], (count.get(r[1]) ?? 0) + 1);
      // never a sign-in's row (core oauth.rs): tazuna does not put a plan login forward on its own
      const best = [...count.entries()].sort((a, b) => b[1] - a[1]).map(([m]) => catalog.find((x) => x.model === m && x.url && !x.why_not && !x.key_env?.startsWith("TAZUNA_SIGNIN_"))).find(Boolean);
      line.offer = best ?? undefined;
    }
    return line;
  }
  if (isFree(c)) return { text: `${modelLabel(c.model)} on ${c.provider}, free`, paid: false };
  // a subscription's model (a plan key, a sign-in such as ChatGPT's): its plan pays, not per token
  if (c.price === "plan") return { text: `${modelLabel(c.model)} on your ${c.provider} plan`, paid: false };
  const est = estimate(c, v, rows);
  return { text: `${modelLabel(c.model)} on ${c.provider}, paid, ${est != null ? `about ${usd(est)} a message` : `$${c.price} per million tokens`}`, paid: true, est };
}
