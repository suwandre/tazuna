import type { ReactNode } from "react";
import { median, modelLabel, paidTo, pct1, pctText, tenths, usd, usdExact, writing, type Billing, type Meta, type ToolStep } from "../lib/types";
import { Mark } from "./Mark";

/** A row number's column, as wide as the largest number and a gap: a fixed 16-20 px ran "111" into the title (the user,
 * design review 14). */
const gutter = (most: number) => ({ minWidth: `${String(Math.max(1, most)).length + 1}ch` });

// The receipt (design review 11, variant A of mockups/v8.html): what an answer cost, itemised, on a slip of paper with a
// torn lower edge. No other harness shows the price of a reply honestly; here it is one faint line under every answer
// that opens into its model calls, and the conversation's own from the status bar. Real numbers only: a row whose number
// the core does not send is left out, never guessed.

/** What a turn would have cost with nothing read from the cache: its cost, with its cached tokens at the input price
 * instead of the cached one. Null where that cannot be told: no prices with the turn (none known, or several providers),
 * its subagents' calls in its cost, its cost not known. Its tokens at list prices were set beside a provider's billed
 * figure until design review 12 ("$1.00 without the cache" under a $0.70 total with nothing cached). */
export function withoutCache(t: { cost: number | null; cached?: number; billing?: Billing; subCalls?: number }): number | null {
  const p = t.billing?.perM;
  return p && t.cost != null && t.cached != null && !t.subCalls ? t.cost + (t.cached * (p[0] - p[1])) / 1e6 : null;
}

/** A price in words: "about" where it is tazuna's estimate (nobody reported one), "not known" where nobody can tell (never
 * $0 for that). `exact`: a slip's own cell, the figure as it is (`usdExact`); else said short (`usd`). */
export const priceSaid = (cost: number | null, estimate?: boolean, exact = false) => (cost == null ? "not known" : `${estimate ? "about " : ""}${(exact ? usdExact : usd)(cost)}`);

/** Why a total is "about" or not known, and what is not in it: calls whose cost is not known, tries the provider may have
 * billed. */
export function gaps(m: { cost: number | null; estimate?: boolean; uncounted?: number; unknownCalls?: number }): string[] {
  const n = m.unknownCalls ?? 0, t = m.uncounted ?? 0;
  return [
    m.cost == null ? "No cost was reported for this turn, and tazuna has no list prices for its model." : m.estimate ? "No cost was reported for this turn: an estimate at list prices." : "",
    n ? `${n} call${n === 1 ? "'s cost is" : "s' costs are"} not known (no usage reported, or cut off by a stop): not in the total.` : "",
    t ? `${t} ${t === 1 ? "try is" : "tries are"} not in the total (sent again after an empty or timed-out reply; the provider may bill ${t === 1 ? "it" : "them"}).` : "",
  ].filter(Boolean);
}

/** A conversation's sums from its turns: the plan's part and each provider's, where every turn says how it was paid;
 * `known` is false as soon as one turn (or a part still running) does not, and then only the total stands. A turn whose
 * cost is not known adds nothing (nor is it in the core's total) and is counted in `unknown`. */
export function sumsOf(turns: Turn[], rest = 0): { total: number; known: boolean; plan: number; paid: [string, number][]; bare: number | null; about: boolean; unknown: number; unknownCalls: number; uncounted: number } {
  const cost = (t: Turn) => t.cost ?? 0;
  const total = turns.reduce((a, t) => a + cost(t), 0) + rest, known = rest === 0 && turns.length > 0 && turns.every((t) => t.billing && (t.billing.plan === true || paidTo(t.billing)));
  const paid = new Map<string, number>();
  let plan = 0;
  // a turn that changed provider inside it is paid to both, together ("SubRouter and OpenRouter")
  if (known) for (const t of turns) { if (t.billing!.plan) plan += cost(t); else paid.set(paidTo(t.billing)!, (paid.get(paidTo(t.billing)!) ?? 0) + cost(t)); }
  const bares = turns.map(withoutCache), bare = known && bares.every((b) => b != null) ? bares.reduce((a, b) => a! + b!, 0) : null;
  const sum = (f: (t: Turn) => number | undefined) => turns.reduce((a, t) => a + (f(t) ?? 0), 0);
  return { total, known, plan, paid: [...paid], bare, about: turns.some((t) => t.estimate), unknown: turns.filter((t) => t.cost == null).length,
    unknownCalls: sum((t) => t.unknownCalls), uncounted: sum((t) => t.uncounted) };
}

export function demoReceipt() {
  const plan: Billing = { plan: true }, sub: Billing = { plan: false, provider: "SubRouter", perM: [0.14, 0.014, 0.56] }, two: Billing = { plan: false, providers: ["SubRouter", "OpenRouter"] };
  const t = (cost: number | null, billing?: Billing, more: Partial<Turn> = {}): Turn => ({ title: "", secs: 1, cost, in: 1_000_000, cached: 1_000_000, out: 500_000, billing, ...more });
  const near = (a: number | null, b: number) => a != null && Math.abs(a - b) < 1e-9;
  const moved = sumsOf([t(0.02, sub), t(0.04, two)]);
  const mixed = sumsOf([t(0.5, plan), t(0.02, sub), t(0.03, sub)]), old = sumsOf([t(0.5, plan), t(0.02)]), running = sumsOf([t(0.5, plan)], 0.1), paid = sumsOf([t(0.02, sub)]);
  const stopped = sumsOf([t(0.18, { plan: false, provider: "Anthropic API", perM: [4, 0.4, 20] }, { estimate: true }), t(null, plan, { estimate: true }), t(0.01, sub, { unknownCalls: 1, uncounted: 2 })]);
  const ok =
    // without the cache: the turn's cost with its cached tokens at the input price, not its tokens at list prices beside a
    // billed figure ($0.70 billed, 30% under the list, nothing cached: $0.70, not $1.00)
    near(withoutCache(t(0.7, { plan: false, provider: "X", perM: [0.6, 0.06, 0.8] }, { cached: 0 })), 0.7) && near(withoutCache(t(0.02, sub)), 0.146)
    && withoutCache(t(0.02, sub, { subCalls: 2 })) === null && withoutCache(t(0.02, two)) === null && withoutCache(t(null, sub)) === null && withoutCache(t(0.02, { plan: false, provider: "X" })) === null
    // a plan turn and two paid ones: each part on its own, and the total is their sum
    && mixed.known && near(mixed.plan, 0.5) && mixed.paid.length === 1 && mixed.paid[0][0] === "SubRouter" && near(mixed.paid[0][1], 0.05) && near(mixed.total, 0.55) && mixed.bare === null
    // one turn that does not say how it was paid, or a part still running: the total alone
    && !old.known && near(old.total, 0.52) && !running.known && near(running.total, 0.6)
    && paid.known && near(paid.bare, 0.146) && !sumsOf([]).known
    // a turn on two providers (a route switch inside it): paid to both, no price list, so no line without the cache
    && paidTo(two) === "SubRouter and OpenRouter" && paidTo({ plan: false, providers: ["A", "B", "C"] }) === "A, B, and C" && paidTo(plan) === undefined
    && moved.known && moved.paid.map(([w]) => w).join("|") === "SubRouter|SubRouter and OpenRouter" && moved.bare === null
    // a `plan` the core did not send is not known: nobody is named as paid, and the conversation's parts are not split
    && paidTo({ provider: "SubRouter" }) === undefined && !sumsOf([t(0.02, { provider: "SubRouter" })]).known
    // a stopped turn's estimate is about; a cost nobody can tell is neither $0 nor in the total; what is not in it is said
    && stopped.about && stopped.unknown === 1 && near(stopped.total, 0.19) && stopped.unknownCalls === 1 && stopped.uncounted === 2 && stopped.bare === null
    && priceSaid(0.18, true) === "about $0.18" && priceSaid(null) === "not known" && priceSaid(0) === "$0"
    && gaps({ cost: 0, unknownCalls: 1, uncounted: 2 }).join(" ") === "1 call's cost is not known (no usage reported, or cut off by a stop): not in the total. 2 tries are not in the total (sent again after an empty or timed-out reply; the provider may bill them)."
    && gaps({ cost: null, estimate: true })[0].startsWith("No cost was reported for this turn, and tazuna has no list prices") && gaps({ cost: 0.1 }).length === 0
    && note([], 0) === undefined && note([{ estimate: false }], 3) === "3 subagent calls are in the total, not in the rows."
    && note([{ estimate: true }], 1) === "1 subagent call is in the total, not in the rows; the rows are at list prices."
    && note([{ estimate: true }], 0, true) === "The total is the CLI's exact one; the rows are at list prices.";
  if (!ok) throw new Error(`receipt: ${JSON.stringify([mixed, old, running, paid, moved, stopped])}`);
  console.log("receipt ok");
}

export const tok = (n: number) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1000 ? `${(n / 1000).toFixed(1)}k` : `${n}`);
/** "41 s", "2 min 5 s"; "<1 s" under a second: the core's time is its own run's, without the model's start, and a quick
 * local answer of 0.25 s read "0 s" (design review 12) */
export const dur = (secs: number) => {
  if (secs < 1) return "<1 s";
  const s = Math.round(secs);
  // an hour and more in hours and minutes ("1 h 38 min", not "98 min 35 s": the slip's column ran past its edge)
  if (s >= 3600) { const m = Math.round((s % 3600) / 60), h = Math.floor(s / 3600) + (m === 60 ? 1 : 0); return m % 60 ? `${h} h ${m} min` : `${h} h`; }
  return s < 60 ? `${s} s` : s % 60 ? `${Math.floor(s / 60)} min ${s % 60} s` : `${s / 60} min`;
};
const took = (ms: number | null) => (ms == null ? "" : ms < 10_000 ? tenths(ms / 1000) : dur(Math.round(ms / 1000)));

const th = "border-b border-line pb-[5px] text-[11.5px] font-medium text-faint";
const num = "whitespace-nowrap pl-3 pt-1 text-right", head = "whitespace-nowrap pl-3 text-right", first = "truncate pt-1";
// the totals row: under a rule, a little apart
const tot = "whitespace-nowrap border-t border-line pl-3 pt-[5px] text-right";
// the columns a narrow slip leaves out, speed first (the user, 2026-11-25): by the slip's own width, where the call's name
// would have under ~120 px (the Call column takes what the numbers leave, and is cut)
const speed = "@max-[500px]:hidden", ttft = "@max-[430px]:hidden";
/** "219 t/s": the user's way, never "/s" or "tok/s" (2026-11-25) */
export const perSec = (tps: number) => `${Math.round(tps)} t/s`;
/** The cache's share beside a cached count, small and faint: "99.8%" ("–" with no input). */
const Share = ({ part, whole }: { part: number; whole: number }) => <span className="ml-1 text-[11px] font-normal text-faint">{whole ? `${pctText(pct1(part, whole))}%` : "–"}</span>;

/** The slip itself: its title beside the mark, a word on the right, the rows, the sums under a dashed rule. `float`: over
 * the window (the conversation's, from the status bar), on the popover's fill with its shadow. */
export function Slip({ title, right, float, wide, children }: { title: string; right?: ReactNode; float?: boolean;
  /** an answer's, with its time to first token and speed (the user, 2026-11-25): wider, its own width deciding which
   * columns fit (`@container`) */
  wide?: boolean; children: ReactNode }) {
  return (
    <div className={`tz-slip-wrap ${float ? "float" : ""} ${wide ? "wide" : ""}`}>
      <div className="tz-slip @container text-[12.5px] tabular-nums text-ink-2">
        <div className="flex items-center justify-between text-[12px] text-faint">
          <span className="inline-flex items-center gap-[7px] text-[12.5px] font-medium text-ink"><Mark size={20} />{title}</span>{right != null && <span>{right}</span>}</div>
        {children}
      </div>
    </div>
  );
}

const Rule = () => <div className="mb-[7px] mt-[9px] border-t border-dashed border-line-strong" />;
const Sum = ({ what, children, quiet, strong }: { what: string; children: ReactNode; quiet?: boolean; strong?: boolean }) => (
  <div className={`flex justify-between gap-4 py-px ${quiet ? "text-faint" : ""}`}><span>{what}</span><span className={strong ? "font-semibold text-ink" : ""}>{children}</span></div>
);

/** Whose plan a turn ran on: the provider a plan's own loop names (core: Mistral Vibe's key, the user, 2026-11-25), else
 * Claude's (the core names no provider on the Claude plan). */
const planOf = (b: Billing) => b.provider ?? "Claude";
/** "On your Mistral Vibe plan", "On your Z.ai Coding Plan": a name that says plan already is not given a second (the
 * terminal's plan_label). */
export const onYour = (plan: string) => `On your ${plan}${/ plan$/i.test(plan) ? "" : " plan"}`;
/** A plan's own loop with no reference price (its catalog lists none): the plan, no money, never "$0 at API prices" (the
 * user, 2026-11-25; the terminal says "on Mistral Vibe plan" alone). */
export const refless = (b?: Billing) => b?.plan === true && !!b.provider && !b.perM;

/** The sums under one answer's receipt, from how that turn itself was paid: on the plan its price at API prices and that
 * nothing was billed, to a provider what it was paid, and what the cache saved where that can be told. A turn that does
 * not say (older data) has its total alone: nothing is taken from the conversation's model of now. An estimate says
 * "about", a cost nobody can tell is no amount, and what is not in the total is said under it. */
function Sums({ meta, note, rows }: { meta: Meta; /** why the rows above do not add up to the total */ note?: string;
  /** the rows' own totals row says the price already: a bare "Total" under it would say it twice (design review 14) */
  rows?: boolean }) {
  const { cost, billing } = meta, bare = withoutCache(meta);
  // a provider that reported no usage for any call: not "Paid to X $0"
  const said = cost === 0 && meta.unknownCalls ? "not reported" : priceSaid(cost, meta.estimate);
  const notes = [note, ...gaps(meta)].filter(Boolean), cheaper = bare != null && cost != null && bare > cost * 1.005;
  if (rows && billing?.plan !== true && !paidTo(billing) && !notes.length && !cheaper) return null;
  return <>
    <Rule />
    {/* any plan as Claude's (mockups/v11.html, the user, 2026-11-25): its total at the provider's API prices, and that the
        plan paid ("Paid $0" under it said the same again); a plan's own loop without a reference price has the plan alone */}
    {billing?.plan === true ? <>
      {!refless(billing) && <Sum what={`Total at ${planOf(billing)}'s API prices`} strong>{said}</Sum>}
      <Sum what={onYour(planOf(billing))} quiet>not billed</Sum>
    </> : paidTo(billing) ? <Sum what={`Paid to ${paidTo(billing)}`} strong>{said}</Sum> : !rows && <Sum what="Total" strong>{said}</Sum>}
    {notes.map((n) => <div key={n} className="pt-px text-[11.5px] text-faint">{n}</div>)}
    {cheaper && <><Rule /><Sum what="The same without the cache" quiet>{priceSaid(bare, meta.estimate)}</Sum></>}
  </>;
}

/** Why a reply's rows do not add up to its total: its subagents' calls are in the total and not among the rows (boss mode's
 * workers too), and claude's rows are at list prices against the CLI's exact total; the own loop's rows are what it was
 * charged. */
export function note(calls: { estimate?: boolean }[], subs: number, plan = false): string | undefined {
  const listed = calls.some((c) => c.estimate);
  if (subs > 0) return `${subs} subagent call${subs === 1 ? " is" : "s are"} in the total, not in the rows${listed ? "; the rows are at list prices" : ""}.`;
  if (listed) return plan ? "The total is the CLI's exact one; the rows are at list prices." : "The total is as reported; the rows are at list prices.";
  return undefined;
}

/** One answer's receipt: a row per model call (its model and role where the core sends them, else what it called; its
 * input, the part read from the cache and its share, its output, its time to first token, its speed while writing, its
 * time, and its price where it was fixed when the call ended), the tools' own time, the totals, then the sums. */
export function AnswerSlip({ meta, tools = [], toolsSaid }: { meta: Meta; tools?: ToolStep[]; /** the steps in words, "ran 2 commands, searched" */ toolsSaid?: string }) {
  const calls = meta.calls ?? [], ran = tools.filter((t) => t.kind !== "thinking" && t.kind !== "said");
  const toolMs = Math.round(ran.reduce((a, t) => a + t.secs, 0) * 1000);
  // a price per call only where each call brings its own (with its model); else the column would be an estimate at the
  // prices of the model the conversation runs on now, 20 to 100 times off after a change of model. A plan with no reference
  // price has none: its calls come at $0 (the core's [0,0,0]), never shown (the review of 2026-11-25)
  const priced = !refless(meta.billing) && calls.length > 0 && calls.every((c) => c.model);
  // the rows fall short of the total, or are off it by more than rounding: why, said under them
  const off = priced && meta.cost != null && Math.abs(calls.reduce((a, c) => a + (c.cost ?? 0), 0) - meta.cost) > Math.max(0.0005, meta.cost * 0.005);
  // a narrow slip drops the speed first, then the time to first token; the cache's share stays (the user, 2026-11-25)
  const cols: [string, string, string?][] = [["In", "input tokens, the cached ones included"], ["Cached", "input tokens read from the cache, and their share of the input"], ["Out", "output tokens"],
    ["TTFT", "time to the first token", ttft], ["Speed", "output tokens a second while writing (after the first token)", speed], ["Time", "the whole call, the wait included"],
    ...(priced ? [["Price", "as charged when the call ended"] as [string, string]] : [])];
  const sum = (f: (c: (typeof calls)[number]) => number | null) => { const xs = calls.map(f).filter((x): x is number => x != null); return xs.length ? xs.reduce((a, b) => a + b, 0) : null; };
  const input = sum((c) => c.fresh + c.cached) ?? 0, cached = sum((c) => c.cached) ?? 0, med = median(calls.flatMap((c) => (c.firstMs == null ? [] : [c.firstMs]))), tps = writing(calls);
  const time = sum((c) => c.totalMs), price = sum((c) => c.cost);
  return (
    <Slip wide title="Receipt" right={calls.length ? `${calls.length} model call${calls.length === 1 ? "" : "s"}` : undefined}>
      {calls.length > 0 ? (
        <table className="mt-2 w-full border-collapse">
          <thead><tr><th scope="col" className={`${th} w-full text-left`}>Call</th>
            {cols.map(([h, why, hide]) => <th key={h} scope="col" title={why} className={`${th} ${head} ${hide ?? ""}`}>{h}</th>)}</tr></thead>
          <tbody>
            {calls.map((c, i) => (
              <tr key={i}>
                <td className={`${first} max-w-0`} title={c.tools || "It wrote the answer"}><span className="inline-block text-faint" style={gutter(calls.length)}>{i + 1}</span>
                  {c.model ? <>{modelLabel(c.model)} <span className="text-faint">{c.role || c.tools || "Answer"}</span></> : c.tools || "Answer"}</td>
                <td className={num}>{tok(c.fresh + c.cached)}</td><td className={num}>{tok(c.cached)}<Share part={c.cached} whole={c.fresh + c.cached} /></td><td className={num}>{tok(c.out)}</td>
                <td className={`${num} ${ttft}`}>{took(c.firstMs) || "–"}</td><td className={`${num} ${speed}`}>{c.tps ? perSec(c.tps) : "–"}</td>
                {/* what a narrow slip leaves out, on hover */}
                <td className={num} title={[c.firstMs != null && `first token after ${took(c.firstMs)}`, c.tps && `${perSec(c.tps)} while writing`].filter(Boolean).join(", ") || undefined}>{took(c.totalMs) || "–"}</td>
                {priced && <td className={num} title={c.cost == null ? "not known: no usage reported, or cut off by a stop" : undefined}>{c.cost == null ? "–" : usdExact(c.cost)}</td>}
              </tr>
            ))}
            {ran.length > 0 && (
              <tr>
                <td className={`${first} max-w-0`} colSpan={4} title="The time the tools themselves took"><span className="inline-block" style={gutter(calls.length)} />Tools <span className="text-faint">{toolsSaid}</span></td>
                <td className={ttft} /><td className={speed} /><td className={num}>{took(toolMs)}</td>{priced && <td />}
              </tr>
            )}
            {/* the sums of the rows: the cache's share overall, the middle time to first token, the speed over all the
                writing time, the time with the tools' (the user, 2026-11-25, mockups/v11.html) */}
            {calls.length + (ran.length ? 1 : 0) > 1 && (
              <tr className="font-medium text-ink">
                <td className="max-w-0 truncate border-t border-line pt-[5px]"><span className="inline-block" style={gutter(calls.length)} />Total</td>
                <td className={tot}>{tok(input)}</td><td className={tot}>{tok(cached)}<Share part={cached} whole={input} /></td><td className={tot}>{tok(sum((c) => c.out) ?? 0)}</td>
                <td className={`${tot} ${ttft}`} title="the median of the calls">{med == null ? "–" : <>{took(med)}<span className="ml-1 text-[11px] font-normal text-faint">med</span></>}</td>
                <td className={`${tot} ${speed}`}>{tps ? perSec(tps) : "–"}</td>
                <td className={tot}>{time == null && !ran.length ? "–" : took((time ?? 0) + toolMs)}</td>
                {priced && <td className={tot}>{price == null ? "–" : usdExact(price)}</td>}
              </tr>
            )}
          </tbody>
        </table>
      // a reply without its calls (an older saved one) still knows how long its tools took
      ) : ran.length > 0 && <div className="mt-2 flex justify-between gap-4" title="The time the tools themselves took"><span className="min-w-0 truncate">Tools <span className="text-faint">{toolsSaid}</span></span><span>{took(toolMs)}</span></div>}
      <Sums meta={meta} note={off ? note(calls, meta.subCalls ?? 0, meta.billing?.plan === true) : undefined} rows={priced && !off && price != null && calls.length + (ran.length ? 1 : 0) > 1} />
    </Slip>
  );
}

/** One turn of a conversation, for its receipt: its reply's Meta, the calls counted. */
export type Turn = Omit<Meta, "calls" | "tokens"> & { title: string; calls?: number; in: number; out: number };

/** The conversation's receipt: a row per turn (and one for what is not a finished turn: a reply running now, or one that
 * ended without its sum), then the sums, the plan's part and each provider's on a line of their own. Its total is the sum
 * of its rows. */
export function ChatSlip({ turns, cost, busy, children }: { turns: Turn[]; /** the conversation's cost so far, the running reply's estimate included */ cost: number; busy?: boolean;
  /** under the sums: the way on to the usage page */ children?: ReactNode }) {
  const listed = turns.reduce((a, t) => a + (t.cost ?? 0), 0), rest = cost - listed > Math.max(0.0005, cost * 0.005) ? cost - listed : 0;
  const s = sumsOf(turns, rest);
  // whose plans: one by its name, several as plans (a Mistral Vibe turn read "On your Claude plan", the user, 2026-11-25)
  const plans = [...new Set(turns.flatMap((t) => (t.billing?.plan === true ? [planOf(t.billing)] : [])))];
  // a short window (the smallest one at the largest text size is 384 px tall) lists fewer turns: the slip has no scroll
  const last = Math.max(2, Math.min(8, Math.floor((window.innerHeight - 300) / 22)));
  return (
    <Slip float title="This conversation" right={`${turns.length} turn${turns.length === 1 ? "" : "s"}`}>
      <table className="mt-2 w-full border-collapse">
        <thead><tr><th scope="col" className={`${th} text-left`}>Turn</th>{["Calls", "Time", "Price"].map((h) => <th key={h} scope="col" className={`${th} ${head}`}>{h}</th>)}</tr></thead>
        <tbody>
          {/* a long conversation: its last turns, the earlier ones as one row */}
          {turns.length > last && <tr className="text-faint"><td className={first} colSpan={3}>{turns.length - last} earlier turn{turns.length - last === 1 ? "" : "s"}</td>
            <td className={num}>{priceSaid(turns.slice(0, -last).reduce((a, t) => a + (t.cost ?? 0), 0), turns.slice(0, -last).some((t) => t.estimate), true)}</td></tr>}
          {turns.slice(-last).map((t, i) => (
            <tr key={i}>
              <td className={`w-full max-w-0 ${first}`} title={t.title}><span className="inline-block text-faint" style={gutter(turns.length)}>{Math.max(0, turns.length - last) + i + 1}</span>{t.title}</td>
              <td className={num}>{t.calls ?? "–"}</td><td className={num}>{dur(t.secs)}</td><td className={num}>{refless(t.billing) ? "not billed" : priceSaid(t.cost, t.estimate, true)}</td>
            </tr>
          ))}
          {rest > 0 && <tr className="text-faint"><td className={first} colSpan={3}><span className="inline-block" style={gutter(turns.length)} />{busy ? "Running now, so far" : "Not in a finished turn"}</td><td className={num}>{usdExact(rest)}</td></tr>}
        </tbody>
      </table>
      <Rule />
      <Sum what={s.known && s.plan > 0 ? "Total, at API prices" : "Total"} strong>{priceSaid(s.total, s.about)}</Sum>
      {s.known && <>
        {/* a plan with no reference price keeps its line, without an amount (the review of 2026-11-25) */}
        {plans.length > 0 && <Sum what={plans.length === 1 ? onYour(plans[0]) : "On your plans"}>{s.paid.length && s.plan > 0 ? `${usd(s.plan)}, not billed` : "not billed"}</Sum>}
        {s.paid.map(([who, n]) => <Sum key={who} what={`Paid to ${who}`}>{n === 0 && s.unknownCalls ? "not reported" : usd(n)}</Sum>)}
        {!s.paid.length && <Sum what="Paid">$0</Sum>}
        {s.bare != null && s.bare > s.total * 1.005 && <><Rule /><Sum what="The same without the cache" quiet>{priceSaid(s.bare, s.about)}</Sum></>}
      </>}
      {/* what is not in the total, said under it */}
      {[s.unknown ? `${s.unknown} turn${s.unknown === 1 ? "'s cost is" : "s' costs are"} not known: not in the total.` : "", ...gaps({ cost: 0, unknownCalls: s.unknownCalls, uncounted: s.uncounted })]
        .filter(Boolean).map((n) => <div key={n} className="pt-px text-[11.5px] text-faint">{n}</div>)}
      {children}
    </Slip>
  );
}
