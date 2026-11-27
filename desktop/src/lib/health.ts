// Every account's standing for the status bar (desktop 0.1.79, prototype mockups/v5.html variant A): fine, running low or
// stopped, with what to say about it. A state per account, not one score: 93% of a week and $12.40 left do not add up to
// a number anyone can act on, and a total hides the account that is about to stop you (design review 6).
import type { Accounts, Learned, ModelChoice, UsageMethod, UsageSource } from "./tazuna";
import { usd } from "./types";

export type Level = "ok" | "low" | "out" | "unknown";
/** One line of the card: a plan's window, a provider's credit, an OmniRoute window. */
export type Item = { key: string; provider: string; name: string; note?: string; pct?: number; value?: string; level: Level; short: string; page?: string;
  /** when its window starts again (ms since 1970), where known */
  resetsAt?: number;
  /** a plan's window whose reset has passed: it reads 0%, and a new one starts with the next message */
  over?: boolean;
  /** what it means and what to do, for an account running low or stopped (the badge's tooltip) */
  advice?: string;
  /** it stops runs now (a credit used up or past zero, a login gone, a window full), where "out" also covers a window at
   * 90%: the badge names these first (`urgent`) */
  stopped?: boolean;
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

// a window from 75% is running low, from 90% about to stop you (the meters' amber and red); credit under $2 is low (the
// core decides that one: src/accounts.rs `row`)
const byPct = (p: number): Level => (p >= 90 ? "out" : p >= 75 ? "low" : "ok");

/** A provider's row as the core sends it (src/accounts.rs `row`): the line to show (`left`), a short one, its plan's
 * windows, and the numbers and state the core decided, which the levels go by, not the words: parsing "$… left" read
 * rupiah, yuan, a plan's windows and a key without its scope as no balance at all (review of the balances, 2026-11-26). */
export type Credit = Accounts["credits"][number] & { short?: string | null; usd_left?: number | null; max_used_pct?: number | null; key_limit?: boolean;
  free_requests?: boolean; state?: "ok" | "low" | "out" | "over" | "needs_scope" | "unknown"; windows?: Accounts["omniroute"][number]["windows"] };

/** Dollars the way the status bar says them: "$12.40 left", "$0.17 below zero". */
const said = (d: number) => (d < 0 ? `${usd(-d)} below zero` : `${usd(d)} left`);

/** A row's state and dollars: the core's; from a core before them (TAZUNA_BIN, an older tazuna on PATH), read from its
 * words ("$1,234.50 left", "$0.17 over its credit") as until desktop 0.1.131, with the core's thresholds. */
export function creditState(c: Credit): { state: NonNullable<Credit["state"]>; usd?: number } {
  const num = (t: string) => Number(t.replace(/,/g, "")), over = c.left?.match(/^\$([\d.,]+) over/), left = c.left?.match(/^(-?)\$([\d.,]+) left/);
  const dollars = c.usd_left ?? (c.state ? undefined : over ? -num(over[1]) : left ? num(left[2]) * (left[1] ? -1 : 1) : undefined);
  return { state: c.state ?? (dollars == null ? "unknown" : dollars < 0 ? "over" : dollars < 0.01 ? "out" : dollars < 2 ? "low" : "ok"), usd: dollars };
}

/** A row's level: a plan's windows by their fullest (byPct, as every meter), the rest by its state. The usage page's row
 * and the status bar take it from here: the row went amber by the core's limits while its meter went red (review). */
export function creditLevel(c: Credit): Level {
  const st = creditState(c).state;
  return c.max_used_pct != null ? byPct(c.max_used_pct) : st === "over" || st === "out" ? "out" : st === "low" || st === "needs_scope" ? "low" : st === "ok" ? "ok" : "unknown";
}

/** SubRouter's balance a conversation carries until the first accounts reading, as a row the core would send. */
export const fallbackCredit = (b: number): Credit => ({ provider: "SubRouter", left: said(b), short: said(b), usd_left: b, page: "https://subrouter.ai" });

/** The order the badge names accounts in: stopped first, then about to stop (a window at 90%), then low. By level alone a
 * plan at 90% named the badge ahead of an account that had stopped (review of the balances). */
export const urgent = (a: Item, b: Item) => rank(b) - rank(a);
const rank = (i: Item) => (i.stopped ? 2 : i.level === "out" ? 1 : 0);

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
      level: byPct(used), short: `Claude ${w} ${Math.round(used)}%`, resetsAt, stopped: used >= 100,
      // past 100% Claude stops, or bills extra usage where it is turned on (it went on silently into extra usage, 2026-11)
      advice: used >= 90 ? `Your Claude ${w} is at ${Math.round(used)}%. Once it is full Claude stops, or bills extra usage if you turned that on.${until(resetsAt)}`
        : `Your Claude ${w} is at ${Math.round(used)}%.${until(resetsAt)}` });
  }
  const credits: Credit[] = acc?.credits ?? (fallback.balance != null ? [fallbackCredit(fallback.balance)] : []);
  for (const c of credits) {
    const pct = c.max_used_pct ?? undefined, p = c.provider, { state: st, usd: dollars } = creditState(c), level = creditLevel(c);
    // the core's short line; from a core before it, the dollars said the way the desktop says them
    const value = pct != null ? `${Math.round(pct)}%` : c.short ?? (dollars != null ? said(dollars) : c.left ?? undefined);
    const top = c.windows?.filter((w) => w.used != null).sort((a, b) => b.used! - a.used!)[0], at = top?.resets ? Date.parse(top.resets) : NaN;
    const resetsAt = Number.isNaN(at) ? undefined : at;
    out.push({ key: `credit:${p}`, provider: p, name: p, group: "Credit with each provider", label: p, note: !c.left ? "no balance from its API" : c.left !== value ? c.left : undefined,
      value, level, page: c.page, pct, resetsAt, stopped: pct != null ? pct >= 100 : st === "over" || st === "out",
      advice: !value ? undefined
        : st === "needs_scope" ? `${p} can't tell this key's balance: ${c.left}.`
        : pct != null ? `${p} is at ${Math.round(pct)}% of its plan${top?.window ? ` (${top.window})` : ""}. Runs on it stop once it is full.${until(resetsAt)}`
        : c.free_requests ? (level === "out" ? `Your ${p} free requests for today are used up. They come back tomorrow; credit lifts the limit.` : `${p}: ${c.left}.`)
        : c.key_limit ? (level === "out" ? `Your ${p} key has used up its own limit. Raise the key's limit on ${p}, or runs on it will fail.`
          : `Your ${p} key limit has ${usd(dollars ?? 0)} left. Raise it soon, or runs on ${p} will stop.`)
        : level === "out" ? `Your ${p} credit is ${st === "over" ? value : "used up"}. Add credits, or runs on ${p} will fail.`
        : `${p} has ${value}. Add credits soon, or runs on ${p} will stop.`,
      // "Z.ai 90% used": a bare "90%" beside a provider's name read as what is left
      short: pct != null ? `${p} ${Math.round(pct)}% used` : value ? `${p} ${value}` : p });
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
      pct: w.used, value: `${Math.round(w.used)}%`, level: login ? "out" : byPct(w.used), stopped: login || w.used >= 100, short: login ? `OmniRoute ${a.provider} login ${a.status}` : `OmniRoute ${what} ${Math.round(w.used)}%`,
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

/** The accounts reading without the providers the user does not track (Settings › Providers, Usage): they show nothing in the
 * status bar or on the usage page. The reading names them (`untracked`); a switch turned since, by the sources. From a core
 * before either, every row. */
export function trackedOnly(acc: Accounts | null, sources: UsageSource[] | null): Accounts | null {
  if (!acc) return acc;
  const off = new Set(acc.untracked ?? []);
  for (const s of sources ?? []) if (s.track.on) off.delete(s.provider); else off.add(s.provider);
  return off.size ? { ...acc, credits: acc.credits.filter((c) => !off.has(c.provider)) } : acc;
}

/** The providers tracked by their console whose last reading (`last`, ms by provider) is 15 minutes old or more: the console
 * reader reads them in a hidden window, one at a time (App.tsx). */
export const dueConsoles = (sources: UsageSource[], last: Record<string, number>, now = Date.now()) =>
  sources.filter((s) => s.track.on && s.track.method === "console" && now - (last[s.provider] ?? 0) >= 15 * 60_000).map((s) => s.provider);

/** The commands a usage method's steps name in backticks ("run `qwencloud auth login`"), for Settings' Copy. */
export const commandsIn = (steps: string) => [...steps.matchAll(/`([^`]+)`/g)].map((m) => m[1]);
/** Settings' usage requests in flight (review 14, fix): each provider's newest of each kind (`pend`: its button waits; a
 * track request's choice is what the line shows) and every request's provider until its own reply (`sent`: a refusal is
 * said on its row even when a later reply came first). A reply to `req` answers it and every one before it (the core saves
 * in the order asked, then reads); one with no number (an older core) answers nothing. A request 20 s old, or every one
 * when the core exits, is let go: the line shows the last state known, never a wait that does not end. */
export type Pend = { provider: string; kind: "track" | "check" | "save"; req: number; at: number; track?: { on: boolean; method: string } };
export type Asked = { pend: Pend[]; sent: Record<number, { provider: string; at: number }> };
export const asked0: Asked = { pend: [], sent: {} };
export const askUsage = (a: Asked, p: Pend): Asked =>
  ({ pend: [...a.pend.filter((x) => !(x.provider === p.provider && x.kind === p.kind)), p], sent: { ...a.sent, [p.req]: { provider: p.provider, at: p.at } } });
export function usageAnswer(a: Asked, req: number | undefined): { a: Asked; provider?: string } {
  if (req == null) return { a };
  const { [req]: mine, ...sent } = a.sent;
  return { a: { pend: a.pend.filter((x) => x.req > req), sent }, provider: mine?.provider };
}
const young = (now: number) => ([, x]: [unknown, { at: number }]) => now - x.at < 20_000;
export const usageExpire = (a: Asked, now: number): Asked =>
  ({ pend: a.pend.filter((x) => young(now)([0, x])), sent: Object.fromEntries(Object.entries(a.sent).filter(young(now))) });

/** A usage line's state (Settings › Providers; review 14, fix): one short status, the one action it offers ("use": the
 * method's `instead`), and the commands to run (the CLI's, each with Copy). The console's from what its visits left (`learned`, main.rs
 * console_learned: null when never opened, no page when nothing usage-like was found), never the core's "no reading":
 * a visit without usage data said "not signed in yet" (the user). Empty text while `learned` is still asked. */
export type UsageState = { text: string; action?: "check" | "signin" | "open" | "use"; commands?: string[] };
/** A line from the core with its first letter a capital, as a line on its own reads ("logged in" is "Logged in"). */
export const cap = (t: string) => t.charAt(0).toUpperCase() + t.slice(1);
export function usageState(m: UsageMethod, u: UsageSource, learned: Learned | null | undefined): UsageState {
  const why = cap(m.why ?? "Not ready");
  switch (m.id) {
    case "api": return { text: m.ready ? "Reads automatically" : why };
    case "api_scope": return m.ready ? { text: "Reads automatically" } : { text: why, action: u.has_key ? "check" : undefined };
    case "cli": {
      if (m.ready) return { text: "Signed in" };
      // signed in, and its usage shows no plan: it may not see one (CLI 1.7.0 and an Individual plan), so the console
      // in its place where the provider has one (core 0.1.233)
      if (m.instead && u.methods.some((x) => x.id === m.instead)) return { text: why, action: "use" };
      return { text: why, action: "check", commands: m.commands ?? commandsIn(m.steps) };
    }
    case "console": {
      if (learned === undefined) return { text: "" };
      if (learned === null) return { text: "Not set up", action: "signin" };
      // its sign-in forgotten (the store gone, the learn file kept): signing in is what opening means now
      const again = learned.store === false ? "signin" : "open";
      if (!learned.page || !learned.fields) return { text: "No usage data found yet", action: again };
      if (m.ready) return { text: `Reads ${learned.page}`, action: again };
      // a reading that says signed out, or failed; none yet is the page found, waiting for its reader
      if (m.why && !/^no reading/i.test(m.why)) return { text: why, action: /signed out/i.test(m.why) ? "signin" : again };
      return { text: `Found ${learned.page}`, action: again };
    }
    case "local": return { text: u.allowance ? "Counts use through tazuna" : "Not set up" };
    default: return { text: m.why ? why : "No usage API" };
  }
}

export const worst = (xs: Item[]): Level => (xs.some((i) => i.level === "out") ? "out" : xs.some((i) => i.level === "low") ? "low" : "ok");

/** A check: `npx tsx -e 'import("./src/lib/health.ts").then(m => m.demo())'`. */
export function demo() {
  const acc: Accounts = { plan: [{ label: "Current session", used: 34, resets: "in 4 h" }, { label: "Current week (all models)", used: 93, resets: "Nov 21 at 3am (Europe/Berlin)" }], plan_notes: [],
    credits: [{ provider: "SubRouter", left: "$12.40 left", short: "$12.40 left", usd_left: 12.4, state: "ok", page: "" }, { provider: "OpenRouter", left: "$0.17 over its credit", short: "$0.17 below zero", usd_left: -0.17, state: "over", page: "" },
      { provider: "Mortera", left: null, short: null, state: "unknown", page: "" }] as Credit[],
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
  // the levels by the core's numbers and state, not its words: rupiah low, a plan's fullest window, a key without its
  // scope, spending that is not a balance, a key's own limit (review of the balances)
  const rows = items({ credits: [{ provider: "Mortera", left: "Rp 797 left (about $0.05)", short: "Rp 797 left", usd_left: 0.05, state: "low", page: "" },
    { provider: "Z.ai Coding Plan", left: "5-hour 12%, weekly 95% used (experimental)", short: "95% used", max_used_pct: 95, state: "low", page: "",
      windows: [{ group: "", window: "5-hour", used: 12, resets: null, unlimited: false }, { group: "", window: "weekly", used: 95, resets: "2026-11-21T02:00:00Z", unlimited: false }] },
    { provider: "Cheaper Inference", left: "turn on account:read for this key", short: "needs account:read", state: "needs_scope", page: "" },
    { provider: "OpenRouter", left: "$1.25 used this month", short: "$1.25 this month", state: "unknown", page: "" },
    { provider: "Venice", left: "$25.40 left ($0.40 + $25.00 bundled)", short: "$25.40 left", usd_left: 25.4, state: "ok", page: "" },
    { provider: "K", left: "$0.40 over its key limit", short: "key limit used up", usd_left: -0.4, state: "over", key_limit: true, page: "" }] as Credit[] } as Accounts, { plan5h: null, planWeek: null }, t0);
  const got = rows.map((i) => `${i.level} ${i.value}`).join("|");
  if (got !== "low Rp 797 left|out 95%|low needs account:read|unknown $1.25 this month|ok $25.40 left|out key limit used up"
    || rows[1].resetsAt !== Date.parse("2026-11-21T02:00:00Z") || rows[0].note !== "Rp 797 left (about $0.05)" || !/Raise the key's limit/.test(rows[5].advice ?? "")) throw new Error(`credits: ${got}`);
  // a conversation's SubRouter balance before the first reading, past zero said as the status bar says it
  const fb = items(null, { plan5h: null, planWeek: null, balance: -0.17 }, t0)[0];
  if (items(null, { plan5h: null, planWeek: null, balance: 1.5 }, t0)[0].level !== "low" || fb.level !== "out" || fb.value !== "$0.17 below zero" || fallbackCredit(-0.17).left !== "$0.17 below zero") throw new Error(`fallback balance: ${fb.value}`);
  // a core before the numbers: its words read as before, so its alerts stay
  const old = items({ credits: [{ provider: "OpenRouter", left: "$0.17 over its credit", page: "" }, { provider: "SubRouter", left: "$1,234.50 left", page: "" }] } as Accounts, { plan5h: null, planWeek: null }, t0);
  if (old.map((i) => `${i.level} ${i.value}`).join("|") !== "out $0.17 below zero|ok $1,234.50 left") throw new Error(`older core: ${old.map((i) => `${i.level} ${i.value}`).join("|")}`);
  // the advice says what the reading is: free requests come back, a key's limit is raised; a plan says "% used"
  const adv = items({ credits: [{ provider: "OpenRouter", left: "0 of 50 free requests left today", short: "0/50 free today", state: "out", free_requests: true, page: "" },
    { provider: "K", left: "$1.50 left of its $5.00 key limit", short: "$1.50 left (key)", usd_left: 1.5, state: "low", key_limit: true, page: "" }] as Credit[] } as Accounts, { plan5h: null, planWeek: null }, t0);
  if (!/come back tomorrow/.test(adv[0].advice!) || adv[1].advice !== "Your K key limit has $1.50 left. Raise it soon, or runs on K will stop." || rows[1].short !== "Z.ai Coding Plan 95% used") throw new Error(`advice: ${adv.map((i) => i.advice).join("|")}`);
  // the badge names an account that stopped before a plan at 95%, and that before one running low
  const order = [rows[0], rows[1], rows[5]].sort(urgent).map((i) => i.key).join();
  if (order !== "credit:K,credit:Z.ai Coding Plan,credit:Mortera") throw new Error(`badge order: ${order}`);
  const lv = Object.fromEntries(all.map((i) => [i.key, i.level]));
  const ok = lv["claude:5 hours"] === "ok" && lv["claude:week"] === "out" && lv["credit:SubRouter"] === "ok" && lv["credit:OpenRouter"] === "out" && lv["credit:Mortera"] === "unknown"
    && mine(all, { provider: "SubRouter" } as ModelChoice, "deepseek-v4.1-flash")[0]?.key === "credit:SubRouter"
    && mine(all, undefined, "claude-opus-5-5").map((i) => i.key).join() === "claude:5 hours,claude:week"
    && mine(all, { provider: "OmniRoute" } as ModelChoice, "agy/gemini-4-hydrogen-high")[0]?.name === "OmniRoute agy, Gemini"
    && all.find((i) => i.key === "credit:OpenRouter")!.short === "OpenRouter $0.17 below zero";
  if (!ok) throw new Error(`health: ${JSON.stringify(all.map((i) => [i.key, i.level, i.short]))}`);
  // untracked providers show nothing; before the sources come, every row; the console reader's due list, 15 minutes apart
  const src = (provider: string, on: boolean, method = "api") => ({ provider, has_key: true, methods: [], track: { on, method }, allowance: null }) as UsageSource;
  const sources = [src("SubRouter", true), src("OpenRouter", false), src("Qwen Cloud Token Plan", true, "console"), src("Caffeine", false, "console")];
  const names = (a: Accounts | null) => a!.credits.map((c) => c.provider).join();
  if (names(trackedOnly(acc, sources)) !== "SubRouter,Mortera" || trackedOnly(acc, null) !== acc || trackedOnly(null, sources) !== null
    || names(trackedOnly({ ...acc, untracked: ["Mortera"] }, null)) !== "SubRouter,OpenRouter" || names(trackedOnly({ ...acc, untracked: ["OpenRouter"] }, [src("OpenRouter", true)])) !== "SubRouter,OpenRouter,Mortera"
    || dueConsoles(sources, {}, t0).join() !== "Qwen Cloud Token Plan" || dueConsoles(sources, { "Qwen Cloud Token Plan": t0 - 14 * 60_000 }, t0).length
    || dueConsoles(sources, { "Qwen Cloud Token Plan": t0 - 15 * 60_000 }, t0).length !== 1) throw new Error("tracked providers");
  const words = commandsIn("Run `npm install -g @qwencloud/qwencloud-cli`, then `qwencloud auth login`.").join("|");
  if (words !== "npm install -g @qwencloud/qwencloud-cli|qwencloud auth login") throw new Error(`usage words: ${words}`);
  // a usage line's state: the console's by what its visits left (never "not signed in" after one), the CLI's three ways
  // not to read and its account without the plan
  const M = (id: UsageMethod["id"], ready: boolean, why: string | null = null, more: Partial<UsageMethod> = {}): UsageMethod => ({ id, ready, why, steps: "", link: "", ...more });
  const qwen = { ...src("Qwen Cloud Token Plan", true, "console"), methods: [M("cli", false), M("console", false)] };
  const said = (m: UsageMethod, l?: Learned | null) => { const s = usageState(m, qwen, l); return `${s.text}${s.action ? ` [${s.action}]` : ""}${s.commands?.length ? ` {${s.commands.join(", ")}}` : ""}`; };
  const states = [said(M("console", false, "No reading yet"), undefined), said(M("console", false, "No reading yet"), null), said(M("console", false, "No reading yet"), { page: null, fields: 0, at: 1, store: true }),
    said(M("console", false, "No reading yet"), { page: "/usage", fields: 3, at: 1, store: true }), said(M("console", true), { page: "/usage", fields: 3, at: 1 }),
    said(M("console", false, "Signed out"), { page: "/usage", fields: 3, at: 1 }), said(M("console", false, "not signed in yet: Sign in…"), { page: null, fields: 0, at: 1 }),
    said(M("cli", false, "Not installed", { commands: ["npm i", "qwencloud auth login"] })), said(M("cli", false, "Not signed in", { commands: ["qwencloud auth login"] })),
    said(M("cli", false, "CLI doesn't show this plan", { commands: [], instead: "console" })), said(M("cli", true)),
    said(M("api_scope", false, "needs account:read")), said(M("local", true)), said(M("none", false, null)), said(M("console", false, "No reading yet"), { page: "/usage", fields: 3, at: 1, store: false })];
  if (states.join("|") !== "|Not set up [signin]|No usage data found yet [open]|Found /usage [open]|Reads /usage [open]|Signed out [signin]|No usage data found yet [open]"
    + "|Not installed [check] {npm i, qwencloud auth login}|Not signed in [check] {qwencloud auth login}"
    + "|CLI doesn't show this plan [use]|Signed in|Needs account:read [check]|Not set up|No usage API|Found /usage [signin]") throw new Error(`usage states: ${states.join("|")}`);
  // the requests in flight: Console then CLI quickly, the first reply keeps the second's choice; a refusal found by its own
  // reply after a later one; an older core's reply (no number) answers nothing; 20 s on, let go
  const P = (provider: string, kind: Pend["kind"], req: number, at = 0, track?: Pend["track"]): Pend => ({ provider, kind, req, at, track });
  let a = askUsage(askUsage(asked0, P("Qwen", "track", 1, 0, { on: true, method: "console" })), P("Qwen", "track", 2, 50, { on: true, method: "cli" }));
  const r1 = usageAnswer(a, 1);
  if (r1.a.pend.length !== 1 || r1.a.pend[0].track?.method !== "cli" || r1.provider !== "Qwen") throw new Error(`a reply to the first click: ${JSON.stringify(r1)}`);
  if (usageAnswer(r1.a, 2).a.pend.length) throw new Error("the second click's reply");
  a = askUsage(askUsage(asked0, P("Caffeine", "track", 3)), P("SubRouter", "check", 4));
  const r4 = usageAnswer(a, 4), r3 = usageAnswer(r4.a, 3);
  if (r4.a.pend.length || r4.provider !== "SubRouter" || r3.provider !== "Caffeine" || Object.keys(r3.a.sent).length) throw new Error(`replies out of order: ${JSON.stringify([r4, r3])}`);
  if (usageAnswer(a, undefined).a !== a) throw new Error("an older core's reply");
  a = askUsage(asked0, P("DeepSeek", "track", 5, 1000));
  if (usageExpire(a, 20_999).pend.length !== 1 || usageExpire(a, 21_000).pend.length || Object.keys(usageExpire(a, 21_000).sent).length) throw new Error("20 s on");
  console.log("health ok");
}

/** A time of day the one way the window writes it: "9:39pm", "2pm" (the sidebar said "09:39 PM" beside the usage page's
 * "2:39am", design review 11). */
export function clock(d: Date): string {
  const h = d.getHours(), m = d.getMinutes();
  return `${h % 12 || 12}${m ? `:${String(m).padStart(2, "0")}` : ""}${h < 12 ? "am" : "pm"}`;
}
