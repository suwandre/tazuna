// Every account's standing for the status bar (desktop 0.1.79, prototype mockups/v5.html variant A): fine, running low or
// stopped, with what to say about it. A state per account, not one score: 93% of a week and $12.40 left do not add up to
// a number anyone can act on, and a total hides the account that is about to stop you (design review 6).
import type { Accounts, ModelChoice } from "./tazuna";

export type Level = "ok" | "low" | "out" | "unknown";
/** One line of the card: a plan's window, a provider's credit, an OmniRoute window. */
export type Item = { key: string; provider: string; name: string; note?: string; pct?: number; value?: string; level: Level; short: string; page?: string };

// a window from 75% is running low, from 90% about to stop you (the meters' amber and red); credit under $2 is low
// ponytail: a fixed $2, a threshold from your own daily spend when that matters
const byPct = (p: number): Level => (p >= 90 ? "out" : p >= 75 ? "low" : "ok");

/** What to say for the plan's window `label` ("Current week (Fable)" is "week, Fable"). */
const windowOf = (label: string) => { const l = label.replace(/^Current /, ""); if (l === "session") return "5 hours"; const m = l.match(/^week \((.*)\)$/); return m ? (m[1] === "all models" ? "week" : `week, ${m[1]}`) : l; };

/** Every account as items, from the accounts reading, or the plan's windows and SubRouter's balance a conversation carries
 * until the first reading arrives. */
export function items(acc: Accounts | null, fallback: { plan5h: number | null; planWeek: number | null; reset: string; balance?: number | null }): Item[] {
  const out: Item[] = [];
  const plan = acc?.plan ?? (fallback.plan5h != null ? [{ label: "Current session", used: fallback.plan5h, resets: fallback.reset }, ...(fallback.planWeek != null ? [{ label: "Current week (all models)", used: fallback.planWeek, resets: "" }] : [])] : []);
  for (const r of plan) {
    const w = windowOf(r.label), resets = r.resets.split(" (")[0];
    out.push({ key: `claude:${w}`, provider: "Claude", name: `Claude plan, ${w}`, note: resets ? `resets ${resets.replace(/^in /, "in ")}` : undefined, pct: r.used, value: `${Math.round(r.used)}%`,
      level: byPct(r.used), short: `Claude ${w} ${Math.round(r.used)}%` });
  }
  const credits = acc?.credits ?? (fallback.balance != null ? [{ provider: "SubRouter", left: `$${fallback.balance.toFixed(2)} left`, page: "https://subrouter.ai" }] : []);
  for (const c of credits) {
    const over = c.left?.match(/^\$([\d.]+) over/), left = c.left?.match(/^-?\$([\d.]+) left/);
    const amount = over ? -Number(over[1]) : left ? Number(left[1]) * (c.left!.startsWith("-") ? -1 : 1) : null;
    const level: Level = amount == null ? "unknown" : amount <= 0 ? "out" : amount < 2 ? "low" : "ok";
    out.push({ key: `credit:${c.provider}`, provider: c.provider, name: c.provider, note: amount == null ? "no balance from its API" : "credit",
      value: amount == null ? undefined : amount < 0 ? `$${(-amount).toFixed(2)} over` : `$${amount.toFixed(2)} left`, level, page: c.page,
      short: amount == null ? c.provider : amount < 0 ? `${c.provider} $${(-amount).toFixed(2)} over` : `${c.provider} $${amount.toFixed(2)} left` });
  }
  for (const a of acc?.omniroute ?? []) for (const w of a.windows) {
    if (w.unlimited || w.used == null) continue;
    // the account by its name ("someone@…" is "someone"): two accounts on one provider read as one row twice; a provider
    // id like "openai-compatible-chat-585fa18d-…" is said as what it is
    const what = (w.group || w.window || "quota").replace(/ Models$/, ""), login = a.status !== "valid";
    const who = a.provider.startsWith("openai-compatible") ? "OpenAI-compatible" : a.provider, acct = a.name ? a.name.split("@")[0] : "";
    out.push({ key: `omni:${a.provider}:${a.name}:${w.group}:${w.window}`, provider: "OmniRoute", name: `OmniRoute ${who}, ${what}`, note: login ? `${acct ? `${acct}, ` : ""}login ${a.status}` : [acct, w.window].filter(Boolean).join(", ") || undefined,
      pct: w.used, value: `${Math.round(w.used)}%`, level: login ? "out" : byPct(w.used), short: login ? `OmniRoute ${a.provider} login ${a.status}` : `OmniRoute ${what} ${Math.round(w.used)}%` });
  }
  return out;
}

/** The items of the account the conversation runs on: the plan for Claude, its credit for an API provider, the matching
 * OmniRoute window (Gemini's for a Gemini model). */
export function mine(all: Item[], choice: ModelChoice | undefined, model: string): Item[] {
  const provider = choice?.provider ?? (/^(claude-)?(opus|sonnet|haiku|fable)/.test(model) || !model ? "Claude" : "");
  if (provider === "Claude") return all.filter((i) => i.provider === "Claude" && !/week, /.test(i.name));
  if (provider === "OmniRoute") {
    const family = /gemini/i.test(model) ? /gemini/i : /claude|gpt/i, omni = all.filter((i) => i.provider === "OmniRoute");
    return omni.filter((i) => family.test(i.name)).slice(0, 1).concat(omni.length && !omni.some((i) => family.test(i.name)) ? omni.slice(0, 1) : []);
  }
  return all.filter((i) => i.key === `credit:${provider}`);
}

export const worst = (xs: Item[]): Level => (xs.some((i) => i.level === "out") ? "out" : xs.some((i) => i.level === "low") ? "low" : "ok");

/** A check: `npx tsx -e 'import("./src/lib/health.ts").then(m => m.demo())'`. */
export function demo() {
  const acc: Accounts = { plan: [{ label: "Current session", used: 34, resets: "in 4 h" }, { label: "Current week (all models)", used: 93, resets: "Nov 21 at 3am (Europe/Berlin)" }], plan_notes: [],
    credits: [{ provider: "SubRouter", left: "$12.40 left", page: "" }, { provider: "OpenRouter", left: "$0.17 over its credit", page: "" }, { provider: "Mortera", left: null, page: "" }],
    omniroute: [{ provider: "agy", name: "a", plan: "Ultra", status: "valid", windows: [{ group: "Gemini Models", window: "weekly", used: 12, resets: null, unlimited: false }] }] };
  const all = items(acc, { plan5h: null, planWeek: null, reset: "" });
  const lv = Object.fromEntries(all.map((i) => [i.key, i.level]));
  const ok = lv["claude:5 hours"] === "ok" && lv["claude:week"] === "out" && lv["credit:SubRouter"] === "ok" && lv["credit:OpenRouter"] === "out" && lv["credit:Mortera"] === "unknown"
    && mine(all, { provider: "SubRouter" } as ModelChoice, "deepseek-v4.1-flash")[0]?.key === "credit:SubRouter"
    && mine(all, undefined, "claude-opus-5-5").map((i) => i.key).join() === "claude:5 hours,claude:week"
    && mine(all, { provider: "OmniRoute" } as ModelChoice, "agy/gemini-4-hydrogen-high")[0]?.name === "OmniRoute agy, Gemini"
    && all.find((i) => i.key === "credit:OpenRouter")!.short === "OpenRouter $0.17 over";
  if (!ok) throw new Error(`health: ${JSON.stringify(all.map((i) => [i.key, i.level, i.short]))}`);
  console.log("health ok");
}
