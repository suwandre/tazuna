// Every account's standing for the status bar (desktop 0.1.79, prototype mockups/v5.html variant A): fine, running low or
// stopped, with what to say about it. A state per account, not one score: 93% of a week and $12.40 left do not add up to
// a number anyone can act on, and a total hides the account that is about to stop you (design review 6).
import type { Accounts, ModelChoice } from "./tazuna";

export type Level = "ok" | "low" | "out" | "unknown";
/** One line of the card: a plan's window, a provider's credit, an OmniRoute window. */
export type Item = { key: string; provider: string; name: string; note?: string; pct?: number; value?: string; level: Level; short: string; page?: string;
  /** when its window starts again (ms since 1970), where known */
  resetsAt?: number;
  /** a plan's window whose reset has passed: it reads 0%, and a new one starts with the next message */
  over?: boolean;
  /** what it means and what to do, for an account running low or stopped (the badge's tooltip) */
  advice?: string;
  /** the account it belongs to, written out whole ("Claude plan", "OmniRoute agy, someone@…, Ultra", "Credit"), and its
   * own name under that heading ("week", "Gemini", "OpenRouter"): the card groups them as the usage page does */
  group: string; label: string };

const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/** claude's /usage reset ("Nov 21 at 2:59am (Europe/Berlin)", "5:29pm (Europe/Berlin)": your own time zone) as a time. */
export function parseReset(text: string, now = Date.now()): number | undefined {
  const m = text.match(/(?:([A-Za-z]{3})\w*\s+(\d{1,2})\s+at\s+)?(\d{1,2})(?::(\d{2}))?\s*(am|pm)/i);
  if (!m) return undefined;
  const d = new Date(now);
  let h = Number(m[3]) % 12;
  if (m[5].toLowerCase() === "pm") h += 12;
  d.setHours(h, Number(m[4] ?? 0), 0, 0);
  if (m[1]) {
    const mo = MONTHS.indexOf(m[1].toLowerCase());
    if (mo < 0) return undefined;
    d.setMonth(mo, Number(m[2]));
    if (d.getTime() < now - 86_400_000) d.setFullYear(d.getFullYear() + 1);
  } else if (d.getTime() < now) d.setDate(d.getDate() + 1);
  return d.getTime();
}

/** The time to a reset in the bar's few characters: "42m", "4h 59m", "5h", "1d 2h" (MonoCode's bar says it so; the
 * words are on hover). */
export function untilShort(at: number | undefined, now = Date.now()): string | undefined {
  if (at == null || Number.isNaN(at)) return undefined;
  const mins = Math.max(0, Math.round((at - now) / 60_000)), d = Math.floor(mins / 1440), h = Math.floor((mins % 1440) / 60), m = mins % 60;
  return d ? `${d}d${h ? ` ${h}h` : ""}` : h ? `${h}h${m ? ` ${m}m` : ""}` : `${m}m`;
}

/** When a window resets, as Claude says it: "in 42 min" and "in 2 h 14 min" within a day, "Thu 3am" within the week,
 * "Nov 26" later (design review 6: the status bar had dropped it, the card and the usage page each wrote it their way). */
export function whenReset(at: number | undefined, now = Date.now()): string | undefined {
  if (at == null || Number.isNaN(at)) return undefined;
  const mins = Math.round((at - now) / 60_000);
  if (mins <= 0) return "now";
  if (mins < 60) return `in ${mins} min`;
  if (mins < 24 * 60) return `in ${Math.floor(mins / 60)} h${mins % 60 ? ` ${mins % 60} min` : ""}`;
  const d = new Date(at);
  if (mins < 7 * 24 * 60) return `${d.toLocaleString("en-US", { weekday: "short" })} ${clock(d)}`;
  return `${d.toLocaleString("en-US", { month: "short" })} ${d.getDate()}`;
}

/** The exact time, for a hover: "Thursday, November 21, 3:00 AM". */
export const exactly = (at?: number) => (at == null ? undefined : new Date(at).toLocaleString("en-US", { weekday: "long", month: "long", day: "numeric", hour: "numeric", minute: "2-digit" }));

// a window from 75% is running low, from 90% about to stop you (the meters' amber and red); credit under $2 is low
// ponytail: a fixed $2, a threshold from your own daily spend when that matters
const byPct = (p: number): Level => (p >= 90 ? "out" : p >= 75 ? "low" : "ok");

/** What to say for the plan's window `label` ("Current week (Fable)" is "week, Fable"). */
const windowOf = (label: string) => { const l = label.replace(/^Current /, ""); if (l === "session") return "5 hours"; const m = l.match(/^week \((.*)\)$/); return m ? (m[1] === "all models" ? "week" : `week, ${m[1]}`) : l; };

/** Every account as items, from the accounts reading, or the plan's windows and SubRouter's balance a conversation carries
 * until the first reading arrives. */
export function items(acc: Accounts | null, fallback: { plan5h: number | null; planWeek: number | null; resetAt?: number | null; balance?: number | null }, now = Date.now()): Item[] {
  const out: Item[] = [];
  const until = (at?: number) => (at ? ` It resets ${/^in /.test(whenReset(at, now)!) ? "" : "on "}${whenReset(at, now)}.` : "");
  const plan = acc?.plan ?? (fallback.plan5h != null ? [{ label: "Current session", used: fallback.plan5h, resets: "" }, ...(fallback.planWeek != null ? [{ label: "Current week (all models)", used: fallback.planWeek, resets: "" }] : [])] : []);
  for (const r of plan) {
    const w = windowOf(r.label);
    let resetsAt = parseReset(r.resets, now) ?? (w === "5 hours" && fallback.resetAt ? fallback.resetAt * 1000 : undefined);
    // a reset further off than its window is long is one that has passed (a time alone is read as tomorrow's, a date
    // more than a day back as next year's)
    if (resetsAt && resetsAt - now > (w === "5 hours" ? 5.5 * 3600_000 : 7.5 * 86_400_000)) resetsAt = now - 1;
    // Its reset has passed: the window is over. claude goes on saying what it held ("100% used") until the next message
    // starts a new one, and the bar said "100% 5 h, resets now" through two refreshes (the user, 2026-11-20).
    const over = resetsAt != null && resetsAt <= now, used = over ? 0 : r.used;
    out.push({ key: `claude:${w}`, provider: "Claude", name: `Claude plan, ${w}`, group: "Claude plan", label: w.charAt(0).toUpperCase() + w.slice(1), over,
      note: over ? "over: a new one starts with your next message" : resetsAt ? `resets ${whenReset(resetsAt, now)}` : undefined, pct: used, value: `${Math.round(used)}%`,
      level: byPct(used), short: `Claude ${w} ${Math.round(used)}%`, resetsAt,
      // past 100% Claude stops, or bills extra usage where it is turned on (it went on silently into extra usage, 2026-11)
      advice: used >= 90 ? `Your Claude ${w} is at ${Math.round(used)}%. Once it is full Claude stops, or bills extra usage if you turned that on.${until(resetsAt)}`
        : `Your Claude ${w} is at ${Math.round(used)}%.${until(resetsAt)}` });
  }
  const credits = acc?.credits ?? (fallback.balance != null ? [{ provider: "SubRouter", left: `$${fallback.balance.toFixed(2)} left`, page: "https://subrouter.ai" }] : []);
  for (const c of credits) {
    const over = c.left?.match(/^\$([\d.]+) over/), left = c.left?.match(/^-?\$([\d.]+) left/);
    const amount = over ? -Number(over[1]) : left ? Number(left[1]) * (c.left!.startsWith("-") ? -1 : 1) : null;
    const level: Level = amount == null ? "unknown" : amount <= 0 ? "out" : amount < 2 ? "low" : "ok";
    out.push({ key: `credit:${c.provider}`, provider: c.provider, name: c.provider, group: "Credit with each provider", label: c.provider, note: amount == null ? "no balance from its API" : undefined,
      value: amount == null ? undefined : amount < 0 ? `$${(-amount).toFixed(2)} below zero` : `$${amount.toFixed(2)} left`, level, page: c.page,
      advice: amount == null ? undefined : amount <= 0 ? `Your ${c.provider} credit is ${amount < 0 ? `$${(-amount).toFixed(2)} past zero` : "used up"}. Add credits, or runs on ${c.provider} will fail.`
        : `${c.provider} has $${amount.toFixed(2)} left. Add credits soon, or runs on ${c.provider} will stop.`,
      // "below zero", not "over": "OpenRouter $0.17 over" read as over a limit (design review 12)
      short: amount == null ? c.provider : amount < 0 ? `${c.provider} $${(-amount).toFixed(2)} below zero` : `${c.provider} $${amount.toFixed(2)} left` });
  }
  for (const a of acc?.omniroute ?? []) for (const w of a.windows) {
    if (w.unlimited || w.used == null) continue;
    // the account by its name ("someone@…" is "someone"): two accounts on one provider read as one row twice; a provider
    // id like "openai-compatible-chat-585fa18d-…" is said as what it is
    const what = (w.group || w.window || "quota").replace(/ Models$/, ""), login = a.status !== "valid";
    const who = a.provider.startsWith("openai-compatible") ? "OpenAI-compatible" : a.provider, acct = a.name ? a.name.split("@")[0] : "";
    const resetsAt = w.resets ? Date.parse(w.resets) : undefined;
    out.push({ key: `omni:${a.provider}:${a.name}:${w.group}:${w.window}`, provider: "OmniRoute", name: `OmniRoute ${who}, ${what}`, resetsAt: Number.isNaN(resetsAt) ? undefined : resetsAt,
      group: ["OmniRoute", who, a.name, a.plan].filter(Boolean).join(", "), label: what === "quota" ? "Quota" : what,
      note: login ? `login ${a.status}` : resetsAt && !Number.isNaN(resetsAt) ? `resets ${whenReset(resetsAt, now)}` : w.window && w.window.toLowerCase() !== what.toLowerCase() ? w.window : undefined,
      pct: w.used, value: `${Math.round(w.used)}%`, level: login ? "out" : byPct(w.used), short: login ? `OmniRoute ${a.provider} login ${a.status}` : `OmniRoute ${what} ${Math.round(w.used)}%`,
      advice: login ? `Your OmniRoute ${who} login${acct ? ` (${acct})` : ""} is ${a.status}. Log in again in OmniRoute, or runs on it will fail.`
        : `OmniRoute ${who} ${what}${acct ? ` (${acct})` : ""} is at ${Math.round(w.used)}%. Runs on it stop once it is full.${until(Number.isNaN(resetsAt) ? undefined : resetsAt)}` });
  }
  return out;
}

/** The accounts reading with the plan's 5 hours and week as a conversation's newer reading has them. A conversation gets
 * them from claude's own rate-limit event while a reply runs: free, and they move as it works. The full reading
 * (`claude -p /usage`) starts the CLI for a few seconds, so it is asked for only at times (App.tsx), and until desktop
 * 0.1.89 it hid the newer numbers once it had come: the bar stood at 15% through a long reply (the user). The week on one
 * model is not in that event and stays as read. `accAt`: when the full reading came, in ms. */
export function withLive(acc: Accounts | null, accAt: number | undefined, live: { limits?: [number, number]; limits_at?: number | null; limits_reset?: number | null } | null | undefined): Accounts | null {
  if (!acc?.plan || !live?.limits || !((live.limits_at ?? 0) * 1000 > (accAt ?? 0))) return acc;
  const [h5, week] = live.limits;
  // the 5 hours' reset from the newer reading too, where it has one (items() takes it from its fallback for a row with none)
  return { ...acc, plan: acc.plan.map((r) => (r.label === "Current session" ? { ...r, used: h5, resets: live.limits_reset ? "" : r.resets }
    : r.label === "Current week (all models)" ? { ...r, used: week } : r)) };
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
  // at a fixed time: the week's reset (Nov 21) was over once that day had passed, and the check failed by the calendar
  const all = items(acc, { plan5h: null, planWeek: null }, new Date(2026, 10, 19, 15, 0).getTime());
  // a conversation's newer reading replaces the 5 hours and the week, an older one nothing; the week on one model stays
  const fable = { ...acc, plan: [...acc.plan!, { label: "Current week (Fable)", used: 15, resets: "" }] };
  const used = (a: Accounts | null) => a!.plan!.map((r) => r.used).join(",");
  if (used(withLive(fable, 1000_000, { limits: [41, 94], limits_at: 1001 })) !== "41,94,15" || used(withLive(fable, 1000_000, { limits: [41, 94], limits_at: 999 })) !== "34,93,15"
    || withLive(fable, 1000_000, { limits: [41, 94], limits_at: 1001, limits_reset: 5000 })!.plan![0].resets !== "" || withLive(null, 0, { limits: [1, 2], limits_at: 9 }) !== null) throw new Error("withLive");
  // a window whose reset has passed reads 0% and says so; a passed time with no date is today's for the 5 hours
  const past = items({ ...acc, plan: [{ label: "Current session", used: 100, resets: "Nov 19 at 2:30pm (Europe/Berlin)" }, { label: "Current week (all models)", used: 83, resets: "Nov 21 at 3am (Europe/Berlin)" }] },
    { plan5h: null, planWeek: null }, new Date(2026, 10, 19, 15, 0).getTime());
  const timeOnly = items({ ...acc, plan: [{ label: "Current session", used: 100, resets: "2:30pm (Europe/Berlin)" }] }, { plan5h: null, planWeek: null }, new Date(2026, 10, 19, 15, 0).getTime());
  if (!(past[0].over && past[0].value === "0%" && past[0].level === "ok" && !past[1].over && past[1].value === "83%" && timeOnly[0].over)) throw new Error("a window that is over");
  const t9 = new Date(2026, 10, 19, 15, 0).getTime();
  const shorts = [untilShort(t9 + 42 * 60_000, t9), untilShort(t9 + 299 * 60_000, t9), untilShort(t9 + 300 * 60_000, t9), untilShort(t9 + 26 * 3600_000, t9), untilShort(t9 - 5000, t9)].join("|");
  if (shorts !== "42m|4h 59m|5h|1d 2h|0m") throw new Error(`untilShort: ${shorts}`);
  // resets: in a day as a countdown, in a week by its day, later by its date
  const t0 = new Date(2026, 10, 19, 15, 0).getTime();
  const r = [whenReset(t0 + 42 * 60_000, t0), whenReset(t0 + 134 * 60_000, t0), whenReset(new Date(2026, 10, 21, 3, 0).getTime(), t0), whenReset(new Date(2026, 10, 26, 17, 13).getTime(), t0),
    whenReset(parseReset("Nov 21 at 2:59am (Europe/Berlin)", t0), t0), whenReset(parseReset("5:29pm (Europe/Berlin)", t0), t0)];
  if (r.join("|") !== "in 42 min|in 2 h 14 min|Sat 3am|Nov 26|Sat 2:59am|in 2 h 29 min") throw new Error(`resets: ${r.join("|")}`);
  const lv = Object.fromEntries(all.map((i) => [i.key, i.level]));
  const ok = lv["claude:5 hours"] === "ok" && lv["claude:week"] === "out" && lv["credit:SubRouter"] === "ok" && lv["credit:OpenRouter"] === "out" && lv["credit:Mortera"] === "unknown"
    && mine(all, { provider: "SubRouter" } as ModelChoice, "deepseek-v4.1-flash")[0]?.key === "credit:SubRouter"
    && mine(all, undefined, "claude-opus-5-5").map((i) => i.key).join() === "claude:5 hours,claude:week"
    && mine(all, { provider: "OmniRoute" } as ModelChoice, "agy/gemini-4-hydrogen-high")[0]?.name === "OmniRoute agy, Gemini"
    && all.find((i) => i.key === "credit:OpenRouter")!.short === "OpenRouter $0.17 below zero";
  if (!ok) throw new Error(`health: ${JSON.stringify(all.map((i) => [i.key, i.level, i.short]))}`);
  console.log("health ok");
}

/** A time of day the one way the window writes it: "9:39pm", "2pm" (the sidebar said "09:39 PM" beside the usage page's
 * "2:39am", design review 11). */
export function clock(d: Date): string {
  const h = d.getHours(), m = d.getMinutes();
  return `${h % 12 || 12}${m ? `:${String(m).padStart(2, "0")}` : ""}${h < 12 ? "am" : "pm"}`;
}
