import { Check, ChevronDown, ChevronRight, ExternalLink, X } from "lucide-react";
import { Fragment, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { core, type Accounts, type ClaudeSetup, type ModelChoice } from "../lib/tazuna";
import { useDialog } from "../lib/dialog";
import { RANGES, barName, fieldOf, floorTo, fromField, needOf, perUnit, planName, rangeWords, spanOf, summarize, used, type Bar, type Choice, type Row, type Unit } from "../lib/usage";
import { modelLabel, usd } from "../lib/types";
import { exactly, parseReset, whenReset } from "../lib/health";
import { Sep } from "./Sep";
import { MenuRule, PopMenu, Segmented, dialogShell, menuItem, quietButton } from "./Controls";

// The usage page (Cmd-Shift-U, the menu, a click on the plan bar or the cost): what each account has left right now
// (Claude's plan from claude's own /usage, OmniRoute's accounts, the providers' credits: src/accounts.rs), what a range
// cost (the last 15 minutes to All, or one picked by hand), and how the user's own runs went. In Settings' language since desktop 0.1.68 (the second design review,
// prototype mockups/v3.html): a section is a heading, a line on what it is, and a card of rows; a zero is a quiet line,
// not a big "$0.000".
const KEEP = "tazuna.usage-range", CUSTOM = "tazuna.usage-custom";
/** A kept range, or null: a hand-picked one from 1970 on, its start not after its end. */
const read = (key: string): Choice | null => {
  try {
    const c = JSON.parse(localStorage.getItem(key) ?? "null");
    if (c?.key === "custom" ? Number.isFinite(c.from) && Number.isFinite(c.to) && 0 <= c.from && c.from <= c.to : c?.key === "all" || RANGES.some(([k]) => k === c?.key)) return c;
  } catch { /* a broken entry is none */ }
  return null;
};
/** The range last chosen (a hand-picked one as it was), the last 30 days at first. */
const kept = (): Choice => read(KEEP) ?? { key: "30d" };
const big = (n: number) => (n >= 1e9 ? `${(n / 1e9).toFixed(1)} B` : n >= 1e6 ? `${(n / 1e6).toFixed(1)} M` : n >= 1e3 ? `${Math.round(n / 1e3)}k` : `${n}`);
// totals in cents: "$0" for nothing, "< $0.01" below a cent ("$0.000072" beside "$0.26" in one column, design review 5)
const money = usd;

/** claude's window names, shorter: "Current session" is the 5 hours, "Current week (Fable)" the week on Fable. */
const windowName = (label: string) => {
  const l = label.replace(/^Current /, "");
  if (l === "session") return "5 hours";
  const m = l.match(/^week \((.*)\)$/);
  return m ? (m[1] === "all models" ? "Week" : `Week, ${m[1]}`) : l.charAt(0).toUpperCase() + l.slice(1);
};

function Section({ title, note, action, children }: { title: string; note?: ReactNode; action?: ReactNode; children: ReactNode }) {
  return (
    <section className="mt-6 first:mt-0">
      <div className="flex items-center gap-3"><h3 className="flex-1 text-[13px] font-semibold text-ink">{title}</h3>{action}</div>
      {note && <p className="mt-0.5 text-[12px] leading-relaxed text-muted">{note}</p>}
      <div className="mt-2.5 divide-y divide-line-soft overflow-hidden rounded-lg border border-line bg-card">{children}</div>
    </section>
  );
}

/** One account's window: its name and whose, the bar (amber from 75%), the share used and when it resets. */
function Meter({ name, of, used, at }: { name: string; of?: ReactNode; used: number | null; at?: number }) {
  const resets = whenReset(at);
  const pct = Math.max(0, Math.min(100, used ?? 0));
  return (
    <div className="flex items-center gap-3.5 px-3.5 py-2.5 text-[12.5px]">
      <span className="w-[200px] flex-none"><span className="block truncate text-ink" title={name}>{name}</span>{of && <span className="block truncate text-[11.5px] text-faint">{of}</span>}</span>
      <span className="h-1.5 min-w-0 flex-1 overflow-hidden rounded bg-line"><i className={`block h-full rounded ${pct >= 90 ? "bg-bad" : pct >= 75 ? "bg-warn" : "bg-live"}`} style={{ width: `${pct}%` }} /></span>
      <span className="w-[220px] flex-none truncate text-right tabular-nums text-muted"><span className="font-medium text-ink">{used == null ? "–" : `${Math.round(used)}% used`}</span>{resets && <><Sep /><span title={exactly(at)}>resets {resets}</span></>}</span>
    </div>
  );
}

/** A figure in a strip: its label, the number with what it is beside it ("3%  clear misses"), and the smaller numbers
 * under it each with its word ("3% pushback   467 runs"), or a quiet line when there is nothing. Until desktop 0.1.78 one
 * caption strung them with middle dots ("clear misses · 3% pushback · 467 runs", design review 6). */
function Figure({ label, value, caption, more = [], quiet }: { label: string; value: string; caption?: string; more?: [string, string][]; quiet?: boolean }) {
  return (
    <div className="min-w-0 px-3.5 py-3">
      <div className="text-[11.5px] text-faint">{label}</div>
      {quiet ? <div className="pt-1.5 text-[13px] text-muted">{value}</div> : <>
        <div className="mt-0.5 flex items-baseline gap-2"><span className="text-[20px] font-semibold tabular-nums tracking-[-0.02em] text-ink">{value}</span>
          {caption && <span className="truncate text-[12px] text-muted">{caption}</span>}</div>
        {more.length > 0 && <div className="mt-1 flex flex-wrap gap-x-4 gap-y-0.5 text-[12px]">{more.map(([n, what]) => (
          <span key={what} className="whitespace-nowrap"><span className="font-medium tabular-nums text-ink-2">{n}</span> <span className="text-faint">{what}</span></span>))}</div>}
      </>}
    </div>
  );
}

export function UsageView({ onClose, limits, balance, choices = [], claude }: { onClose: () => void; limits?: [number, number] | null; balance?: number | null;
  /** Claude Code as setup found it: why there is no plan reading, said truly (not installed, not signed in) */
  claude?: ClaudeSetup | null;
  /** the model catalog: names the provider of an API run the run log did not record (one recorded since 0.1.182) */
  choices?: ModelChoice[] }) {
  // the rows, the second they start at (the last 90 days, the whole log once All or an older hand-picked range asks) and
  // whether they carry the signals (read from the recordings: only while How the runs went is open, for the range)
  const [got, setGot] = useState<{ rows: Row[]; since: number; signals: boolean } | null>(null);
  const [acc, setAcc] = useState<Accounts | null | undefined>(undefined);
  const dialog = useDialog();
  const [choice, setChoice] = useState(kept);
  const choose = (c: Choice) => { setChoice(c); localStorage.setItem(KEEP, JSON.stringify(c)); if (c.key === "custom") localStorage.setItem(CUSTOM, JSON.stringify(c)); };
  const [fold, setFold] = useState(false);
  const [base] = useState(() => Math.floor(Date.now() / 1000) - 90 * 86_400);
  const need = needOf(choice, base);
  // what the page asked last, every start it asked for (a late answer to one is no news), and what it needs now
  const asked = useRef({ since: Infinity, signals: false }), mine = useRef(new Set<number>()), want = useRef({ since: need, signals: fold });
  want.current = { since: need, signals: fold };
  const ask = (w: { since: number; signals: boolean }) => {
    asked.current = w; mine.current.add(w.since);
    core.usage(w.signals, w.since).catch(() => setGot({ rows: [], since: w.since, signals: w.signals }));
  };
  // a rolling range moves on while the page is open
  const [tick, setTick] = useState(0);
  const start = useRef<HTMLInputElement>(null);
  const [allModels, setAllModels] = useState(false);
  const providerOf = (model: string) => {
    if (model.startsWith("agy/")) return "OmniRoute";
    const ps = new Set(choices.filter((c) => c.model === model).map((c) => c.provider));
    return ps.size === 1 ? [...ps][0] : null;
  };
  // asked once when opened (the window re-renders on every streamed view; onClose is new each time)
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    // the page's own answer, or one reaching as far back with what it needs; the start page's (90 days from later, no
    // signals) is its refresh after a turn: the page asks its own range again, so a run just ended shows
    const off = core.onUsage((u) => {
      const a = asked.current, since = u.since ?? a.since, signals = u.signals ?? true;
      if (since <= a.since && (signals || !a.signals)) setGot({ rows: u.rows, since, signals });
      else if (since > a.since && !mine.current.has(since)) ask(want.current);
    });
    const minute = setInterval(() => setTick((n) => n + 1), 60_000);
    const offAcc = core.onAccounts(setAcc);
    core.accounts().catch(() => setAcc(null));
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") close.current(); };
    window.addEventListener("keydown", esc);
    return () => { off.then((f) => f()); offAcc.then((f) => f()); window.removeEventListener("keydown", esc); clearInterval(minute); };
  }, []);
  useEffect(() => { const a = asked.current; if (need < a.since || (fold && !a.signals)) ask({ since: need, signals: fold }); }, [need, fold]);
  const runs = useMemo(() => (got ? used(got.rows) : null), [got]);
  const ready = !!got && got.since <= need;
  const page = useMemo(() => {
    if (!runs || !ready) return null;
    const now = Date.now() / 1000, sp = spanOf(choice, now, runs[0]?.ts);
    return { sp, s: summarize(runs, sp), words: rangeWords(choice, sp, now) };
  }, [runs, ready, choice, tick]);
  const s = page?.s, words = page?.words ?? rangeWords(choice, spanOf(choice, Date.now() / 1000));
  // a range picked by hand is the last one picked (kept as it was), else the one shown to this minute; the focus goes to
  // its start
  const custom = () => {
    const now = Date.now() / 1000;
    if (choice.key !== "custom") choose(read(CUSTOM) ?? { key: "custom", from: floorTo(page?.sp.from ?? now - 7 * 86_400, "minute"), to: floorTo(now, "minute") });
    setTimeout(() => start.current?.focus());
  };
  // start never after end: moving one past the other takes the other along
  const setFrom = (v: string) => { const t = fromField(v); if (choice.key === "custom" && !Number.isNaN(t)) choose({ key: "custom", from: t, to: Math.max(t, choice.to) }); };
  const setTo = (v: string) => { const t = fromField(v); if (choice.key === "custom" && !Number.isNaN(t)) choose({ key: "custom", from: Math.min(t, choice.from), to: t }); };
  const field = "rounded-md border border-line bg-field px-2 py-[2px] text-[12.5px] tabular-nums text-ink outline-none focus:border-accent";
  const th = "px-3.5 pb-1.5 pt-2.5 text-left text-[11.5px] font-medium text-faint", td = "border-t border-line-soft px-3.5 py-[7px]";
  const num = `${td} text-right tabular-nums`;
  const plans = s?.plans.length === 1 ? s.plans[0].name : "Plans";
  const apiRuns = s ? s.byModel.filter((m) => !m.plan).reduce((a, m) => a + m.runs, 0) : 0, planRuns = s ? s.byModel.filter((m) => m.plan).reduce((a, m) => a + m.runs, 0) : 0;
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-30 grid place-items-center bg-scrim p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Usage" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className={`${dialogShell(780)} h-[900px]`}>
        <div className="flex items-center gap-3 border-b border-line-soft px-5 py-3">
          <span className="flex-1 font-medium">Usage</span>
          <button onClick={onClose} title="Close (Esc)" aria-label="Close" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="overflow-auto bg-main px-6 pb-7 pt-5">
          {/* in words for anyone: it named "your OmniRoute accounts" to a new user (design review 14) */}
          <Section title="Right now" note="How much of each plan and account is used, and the credit left with each provider.">
            <RightNow acc={acc} limits={limits} balance={balance} claude={claude} />
          </Section>

          {/* the range by the sections it changes, not in the page's header: there it read as changing "Right now" too,
              which it never did (the user, desktop 0.1.72) */}
          <Section title="Spending" action={<RangeMenu choice={choice} label={words.button} onChoose={(k) => (k === "custom" ? custom() : choose({ key: k }))} />}
            note={<>
              {choice.key === "custom" && <span className="mb-1.5 mt-1 flex items-center gap-2">From
                <input ref={start} type="datetime-local" aria-label="From" min="1970-01-01T00:00" value={fieldOf(choice.from)} onChange={(e) => setFrom(e.target.value)} className={field} />to
                <input type="datetime-local" aria-label="To" min="1970-01-01T00:00" value={fieldOf(choice.to)} onChange={(e) => setTo(e.target.value)} className={field} /></span>}
              {page ? words.note : "From the run log."} A plan's runs are counted at API prices, not billed.</>}>
            {!s || !page ? <div className="px-3.5 py-2.5 text-[12.5px] text-faint">Reading the run log…</div>
              // nothing in the range: one quiet line, no empty figures or chart frame
              : !s.runs ? <div className="px-3.5 py-2.5 text-[12.5px] text-muted">{choice.key === "all" ? "Nothing has run yet." : `Nothing ran ${words.within}.`}</div> : <>
              <div className="grid grid-cols-3 divide-x divide-line-soft">
                {/* nothing is said once, under the figures ("Nothing spent in these days."), not in each of them (design review 14) */}
                <Figure label="API providers" value={s.api > 0 ? money(s.api) : "–"} quiet={s.api <= 0} caption="billed" more={[[`${apiRuns}`, "runs"]]} />
                {/* each plan by its provider (the user, 2026-11-25: a Mistral Vibe run read "Claude plan"): one by name, several
                    as Plans with each one's part */}
                {/* plans with no reference price run at $0: counted by their runs, "not billed", never "$0" (the review of 2026-11-25) */}
                <Figure label={plans} value={!planRuns ? "–" : s.plan > 0 ? money(s.plan) : "not billed"} quiet={!planRuns} caption={s.plan > 0 ? "at API prices, not billed" : undefined}
                  more={[...(s.plans.length > 1 ? s.plans.map((p) => [p.cost > 0 ? money(p.cost) : "not billed", p.name] as [string, string]) : []), [`${planRuns}`, "runs"]]} />
                <Figure label="From the cache" value={s.read ? `${Math.round((100 * s.cached) / s.read)}%` : "Nothing read yet"} quiet={!s.read} caption="of the input" more={[[big(s.cached), "tokens"]]} />
              </div>
              <Bars bars={s.bars} unit={page.sp.unit} plans={plans} within={words.within} />
              {s.byModel.length > 0 && (
                <table className="w-full text-[12.5px]"><thead><tr><th className={th}>Model</th><th className={th}>Provider</th>
                  <th className={`${th} text-right`}>Runs</th><th className={`${th} text-right`}>Cost</th><th className={`${th} text-right`}>From cache</th></tr></thead>
                  <tbody>{(allModels ? s.byModel : s.byModel.slice(0, 8)).map((m) => (
                    <tr key={`${m.plan}${m.model}${m.provider}`}><td className={td} title={m.model}>{modelLabel(m.model)}</td>
                      <td className={`${td} text-muted`}>{m.plan ? planName(m.provider) : m.provider ?? providerOf(m.model) ?? <span className="text-faint" title="An API run from before tazuna 0.1.182, which did not record its provider; several providers serve this model">Not recorded</span>}</td>
                      <td className={num}>{m.runs}</td><td className={num}>{m.plan && !m.cost ? "not billed" : money(m.cost)}</td><td className={num}>{m.cachePct}%</td></tr>
                  ))}</tbody>
                  {/* the eight most used, the rest on request: every model ever tried made a list of 40 (design review 5) */}
                  {/* a button that looks like one: it read as a line of text until hovered (design review 6) */}
                  {s.byModel.length > 8 && <tfoot><tr><td colSpan={5} className="border-t border-line-soft p-2">
                    <button onClick={() => setAllModels(!allModels)} aria-expanded={allModels}
                      className="flex w-full items-center justify-center gap-1.5 rounded-md border border-line bg-field/60 py-1.5 text-[12.5px] text-ink-2 hover:bg-hover hover:text-ink">
                      {allModels ? "Show fewer" : `Show all ${s.byModel.length} models`}
                      <ChevronDown size={14} strokeWidth={1.75} className={`transition-transform duration-150 ${allModels ? "rotate-180" : ""}`} /></button></td></tr></tfoot>}
                </table>
              )}
            </>}
          </Section>

          {/* how the runs went is for tuning tazuna: folded at the bottom, closed (design review 9; it was a section as
              big as Spending, with a table per version) */}
          <details onToggle={(e) => setFold(e.currentTarget.open)} className="group mt-6 overflow-hidden rounded-lg border border-line bg-card">
            <summary className="flex cursor-pointer list-none items-center gap-2 px-3.5 py-2.5 text-[12.5px] hover:bg-hover [&::-webkit-details-marker]:hidden">
              <ChevronRight size={14} strokeWidth={1.5} className="flex-none text-faint transition-transform group-open:rotate-90" />
              {/* the hint under its title, not after a middle dot (design review 10) */}
              <span className="flex-1"><span className="block font-medium text-ink-2">How the runs went</span><span className="block text-[11.5px] text-faint">For tuning tazuna: clear misses and pushback by version and model</span></span>
            </summary>
            <p className="border-t border-line-soft px-3.5 py-2.5 text-[12px] leading-relaxed text-muted">The same range as Spending, read from what happened next, with nothing for you to label. A clear miss: you stopped or rewound it, it ended in an error,
            its change was thrown away, or your next message corrected it or asked the same again. Pushback: "wait, …", "why …?", "progress?".</p>
            <div className="border-t border-line-soft">
            {!s || !got?.signals ? <div className="px-3.5 py-2.5 text-[12.5px] text-faint">Reading the run log…</div>
              : s.byVersion.length === 0 ? <div className="px-3.5 py-2.5 text-[12.5px] text-muted">No runs of your own {words.within}.</div> : <>
              {s.lanes.length > 0 && <div className="grid divide-x divide-line-soft" style={{ gridTemplateColumns: `repeat(${s.lanes.length}, minmax(0, 1fr))` }}>
                {s.lanes.map((l) => <Figure key={l.lane} label={l.lane} value={`${l.missPct}%`} caption="clear misses" more={[[`${l.softPct}%`, "pushback"], [`${l.runs}`, "runs"]]} />)}
              </div>}
              <table className="w-full text-[12.5px]"><thead><tr><th className={th}>Version</th><th className={th}>Model</th>
                <th className={`${th} text-right`}>Runs</th><th className={`${th} text-right`}>Clear misses</th><th className={`${th} text-right`}>Pushback</th>
                <th className={th}>Mostly</th></tr></thead>
                <tbody>{s.byVersion.slice(0, 20).map((v) => (
                  <tr key={`${v.version}${v.model}`}><td className={td}>{v.version}</td>
                    <td className={`${td} text-ink-2`} title={v.model}>{modelLabel(v.model)}</td><td className={num}>{v.runs}</td><td className={num}>{v.missPct}%</td>
                    <td className={num}>{v.softPct}%</td><td className={`${td} text-[12px] text-muted`}>{v.top}</td></tr>
                ))}</tbody></table>
            </>}
</div>
          </details>
        </div>
      </div>
    </div>
  );
}

/** A group's heading in the account list: the account written out whole, wrapping, never cut. */
const groupHead = "flex flex-wrap items-center gap-y-0.5 bg-field/40 px-3.5 py-1.5 text-[11.5px] text-muted";

/** An OmniRoute provider as people say it: its id "openai-compatible-chat-585fa18d-…" is "OpenAI-compatible". */
const who = (p: string) => (p.startsWith("openai-compatible") ? "OpenAI-compatible" : p);

/** Every account's windows and credits, one row each. */
function RightNow({ acc, limits, balance, claude }: { acc: Accounts | null | undefined; limits?: [number, number] | null; balance?: number | null; claude?: ClaudeSetup | null }) {
  if (acc === undefined) return <div className="px-3.5 py-2.5 text-[12.5px] text-faint">Asking your accounts…</div>;
  // no reading from claude's /usage: the plan as the last reply's rate limits said it, if any
  const plan = acc?.plan ?? (limits ? [{ label: "Current session", used: limits[0], resets: "" }, { label: "Current week (all models)", used: limits[1], resets: "" }] : []);
  const credits = acc?.credits ?? (balance != null ? [{ provider: "SubRouter", left: `$${balance.toFixed(2)} left`, page: "https://subrouter.ai" }] : []);
  // an OmniRoute account with nothing used in any window (and a valid login) folds away; so do providers with no balance
  const idle = (a: Accounts["omniroute"][number]) => a.status === "valid" && a.windows.length > 0 && a.windows.every((w) => !w.unlimited && (w.used ?? 0) === 0);
  const busy = acc?.omniroute.filter((a) => !idle(a)) ?? [], quiet = acc?.omniroute.filter(idle) ?? [];
  const quietWindows = quiet.reduce((n, a) => n + a.windows.length, 0);
  const unread = credits.filter((c) => !c.left);
  // the true state of Claude: not installed, or not signed in (it said "not logged in to a subscription" with no claude at all)
  const why = claude?.claude === null ? "Claude Code isn't installed" : claude?.claude_login === false ? "Claude isn't signed in" : "no plan has reported its use yet";
  if (!plan.length && !credits.length && !acc?.omniroute.length) return <div className="px-3.5 py-2.5 text-[12.5px] text-muted">Nothing to read yet: {why}, and no provider key is set.</div>;
  return <>
    {/* each account a group under its heading, as OmniRoute's (until desktop 0.1.82 "Claude plan" and "credit" under every row) */}
    {plan.length > 0 && <div className={groupHead}><span className="font-medium text-ink-2">Claude plan</span></div>}
    {plan.map((r) => <Meter key={r.label} name={windowName(r.label)} used={r.used} at={parseReset(r.resets)} />)}
    {/* each OmniRoute account under its own heading, written out whole and wrapping (provider, account, plan, login); its
        windows below by name alone. In one 200 px line under each window the account and plan were cut to "…" with no way
        to read them (the user, desktop 0.1.80) */}
    {busy.map((a) => (
      <div key={`${a.provider}${a.name}`}>
        <div className={groupHead}>
          <span className="font-medium text-ink-2">OmniRoute</span><Sep />{who(a.provider)}{a.name && <><Sep /><span className="break-all">{a.name}</span></>}
          {a.plan && <><Sep />{a.plan}</>}{a.status !== "valid" && <><Sep /><span className="text-bad">login {a.status}</span></>}
        </div>
        {a.windows.map((w, i) => w.unlimited
          ? <div key={i} className="flex items-center gap-3.5 px-3.5 py-2.5 text-[12.5px]"><span className="w-[200px] flex-none text-ink">{[w.group, w.window].filter(Boolean).join(", ") || "Quota"}</span><span className="text-muted">unlimited</span></div>
          : <Meter key={i} name={[w.group, w.window].filter(Boolean).join(", ").replace(/^quota$/, "Quota") || "Quota"} used={w.used} at={w.resets ? Date.parse(w.resets) : undefined} />)}
      </div>
    ))}
    {quiet.length > 0 && <Fold label={`${quietWindows} window${quietWindows === 1 ? "" : "s"} at 0%`} note={`${quiet.length} OmniRoute account${quiet.length === 1 ? "" : "s"}`}>
      {quiet.map((a) => <div key={`${a.provider}${a.name}`}>
        <div className={groupHead}><span className="font-medium text-ink-2">OmniRoute</span><Sep />{who(a.provider)}{a.name && <><Sep /><span className="break-all">{a.name}</span></>}{a.plan && <><Sep />{a.plan}</>}</div>
        {a.windows.map((w, i) => <Meter key={i} name={[w.group, w.window].filter(Boolean).join(", ").replace(/^quota$/, "Quota") || "Quota"} used={w.used} at={w.resets ? Date.parse(w.resets) : undefined} />)}
      </div>)}
    </Fold>}
    {credits.length > 0 && <div className={groupHead}><span className="font-medium text-ink-2">Credit with each provider</span></div>}
    {credits.filter((c) => c.left).map((c) => credit(c))}
    {unread.length > 0 && <Fold label={`${unread.length} provider${unread.length === 1 ? " doesn't" : "s don't"} report a balance`} note={unread.map((c) => c.provider).join(", ")} right="their pages">
      {unread.map((c) => credit(c))}
    </Fold>}
    {acc && acc.plan_notes.length > 0 && <Contributing notes={acc.plan_notes} />}
  </>;
}

/** Rows that need no look, folded under one line that opens on a click: accounts with nothing used, providers whose
 * balance no API tells (design review 9: eight empty 0% bars in a row on the user's page). */
function Fold({ label, note, right, children }: { label: string; note?: string; right?: string; children: ReactNode }) {
  return (
    <details className="group">
      <summary className="flex cursor-pointer list-none items-center gap-2 px-3.5 py-2.5 text-[12.5px] text-ink-2 hover:bg-hover [&::-webkit-details-marker]:hidden">
        <ChevronRight size={14} strokeWidth={1.5} className="flex-none text-faint transition-transform group-open:rotate-90" />
        <span className="flex flex-1 items-center">{label}{note && <><Sep /><span className="text-faint">{note}</span></>}</span>
        {right && <span className="text-[11.5px] text-faint">{right}</span>}
      </summary>
      <div className="divide-y divide-line-soft border-t border-line-soft">{children}</div>
    </details>
  );
}

/** One provider's credit: what is left, or, where its API does not tell, its page. */
function credit(c: { provider: string; left?: string | null; page: string }) {
  return (
      <div key={c.provider} className="flex items-center gap-3.5 px-3.5 py-2.5 text-[12.5px]">
        <span className="w-[200px] flex-none text-ink">{c.provider}</span>
        {c.left ? <><span className="flex-1" /><span className={`tabular-nums ${/ over /.test(c.left) || c.left.startsWith("-") ? "text-bad" : "text-muted"}`}><span className="font-medium text-ink">{c.left.replace(/ (left|over).*/, "")}</span>{c.left.match(/ (left|over).*/)?.[0]}</span></>
          : <><span className="flex-1" />{/* why is on the fold these rows sit under */}
            <button onClick={() => core.openLink(c.page).catch(() => {})} className={`flex items-center gap-1 ${quietButton}`}>{/* OpenAI's wording for its usage page */}{c.provider === "ChatGPT" ? "Manage usage" : "Open its page"}<ExternalLink size={12} strokeWidth={1.5} /></button></>}
      </div>
  );
}

type Period = { period: string; stats: string; items: { pct: number; title: string; detail?: string; run?: string }[]; tops: { label: string; items: [string, string][] }[] };

/** claude's "What's contributing to your usage?" lines as periods: "Last 24h · 865 requests · 7 sessions", then
 * "  98%  Start fresh when you switch tasks · run /clear" and "  Top skills: /a 3%, /b 1%". */
export function periods(notes: string[]): Period[] {
  const out: Period[] = [];
  for (const n of notes) {
    const head = n.match(/^Last (\S+) · (.*)$/);
    if (head) { out.push({ period: head[1], stats: head[2], items: [], tops: [] }); continue; }
    const p = out.at(-1);
    const item = n.match(/^\s*(\d+)%\s+(.*?)(?: · (.*))?$/);
    const top = n.match(/^\s*Top ([^:]+):\s*(.*)$/);
    if (p && item) {
      const run = item[3]?.match(/^run (\/\S+)$/)?.[1];
      p.items.push({ pct: +item[1], title: item[2], detail: run ? undefined : item[3], run });
    } else if (p && top) p.tops.push({ label: top[1], items: top[2].split(/,\s*/).map((x) => { const m = x.match(/^(.*?)\s+(\d+%)$/); return m ? [m[1], m[2]] : [x, ""]; }) });
  }
  return out;
}

/** What made the plan's use, as claude's /usage tells it: one period at a time, each cause with its share as a bar and
 * the command that helps as a chip, the top skills, subagents and MCP servers as tags. Until desktop 0.1.72 claude's text
 * as it came, indented lines under a "What's contributing" fold (the user: "cluttered and ugly"). */
function Contributing({ notes }: { notes: string[] }) {
  const all = useMemo(() => periods(notes), [notes]);
  const [at, setAt] = useState(0);
  const p = all[Math.min(at, all.length - 1)];
  const name = (x: string) => x.replace(/^(\d+)h$/, "$1 hours").replace(/^(\d+)d$/, "$1 days");
  const lead = "w-[96px] flex-none";
  return (
    <details className="group">
      <summary className="flex cursor-pointer list-none items-center gap-2 px-3.5 py-2.5 text-[12.5px] text-ink-2 hover:bg-hover [&::-webkit-details-marker]:hidden">
        <ChevronRight size={14} strokeWidth={1.5} className="flex-none text-faint transition-transform group-open:rotate-90" />
        <span className="flex-1">What's contributing to your Claude usage</span>
        <span className="text-[11.5px] text-faint">approximate, this machine only</span>
      </summary>
      {!p ? <pre className="border-t border-line-soft px-3.5 py-2.5 whitespace-pre-wrap font-sans text-[12px] leading-relaxed text-muted">{notes.filter((n) => !n.startsWith("Approximate")).join("\n")}</pre> : (
        <div className="border-t border-line-soft pb-1.5">
          <div className="flex items-center gap-3 px-3.5 pb-1 pt-2.5">
            {all.length > 1 && <Segmented label="Period" value={String(all.indexOf(p))} options={all.map((q, i) => [String(i), `Last ${name(q.period)}`] as const)} onChange={(i) => setAt(Number(i))} />}
            <span className="ml-auto text-[11.5px] tabular-nums text-faint">{p.stats}</span>
          </div>
          {p.items.map((it) => (
            <div key={it.title} className="flex items-center gap-3.5 px-3.5 py-[7px] text-[12.5px]">
              <span className={`${lead} flex items-center gap-2`} title={`${it.pct}% of the period's use`}>
                <span className="w-9 text-right font-medium tabular-nums text-ink">{it.pct}%</span>
                <span className="h-1 flex-1 overflow-hidden rounded bg-line"><i className="block h-full rounded bg-live" style={{ width: `${Math.min(100, it.pct)}%` }} /></span>
              </span>
              <span className="min-w-0 flex-1"><span className="block text-ink">{it.title}</span>{it.detail && <span className="block text-[11.5px] text-faint">{it.detail}</span>}</span>
              {it.run && <code className="flex-none rounded-[5px] border border-line bg-sidebar px-1.5 py-px font-mono text-[11.5px] text-muted" title={`run ${it.run} in Claude Code`}>{it.run}</code>}
            </div>
          ))}
          {p.tops.length > 0 && <div className="mx-3.5 mt-1 space-y-1.5 border-t border-line-soft pt-2.5 pb-1">
            {p.tops.map((t) => (
              <div key={t.label} className="flex items-baseline gap-3.5 text-[12px]">
                <span className={`${lead} text-faint`}>{t.label.charAt(0).toUpperCase() + t.label.slice(1)}</span>
                <span className="flex min-w-0 flex-wrap gap-1.5">{t.items.map(([n, pct]) => (
                  <span key={n} className="rounded-md border border-line-soft bg-sidebar px-1.5 py-px"><span className="text-ink-2">{n}</span>{pct && <span className="ml-1 tabular-nums text-faint">{pct}</span>}</span>
                ))}</span>
              </div>
            ))}
          </div>}
        </div>
      )}
    </details>
  );
}

/** The range's button and its menu (PopMenu: the arrows, Esc): the presets, All and a range picked by hand, the chosen one
 * ticked. Nine keys in a Segmented were wider than the heading's row. */
function RangeMenu({ choice, label, onChoose }: { choice: Choice; label: string; onChoose: (k: Choice["key"]) => void }) {
  const [at, setAt] = useState<{ x: number; y: number } | null>(null), button = useRef<HTMLButtonElement>(null);
  const items: [Choice["key"], string][] = [...RANGES.map(([k, , l]) => [k, l] as [Choice["key"], string]), ["all", "All time"], ["custom", "Custom range…"]];
  return <>
    <button ref={button} onClick={(e) => { const r = e.currentTarget.getBoundingClientRect(); setAt(at ? null : { x: r.right - 200, y: r.bottom + 6 }); }} aria-haspopup="menu" aria-expanded={!!at}
      className={`flex items-center gap-1 tabular-nums ${quietButton}`}>{label}<ChevronDown size={13} strokeWidth={1.5} className="text-faint" /></button>
    {at && <PopMenu at={at} label="Range" anchor={button.current} onClose={() => setAt(null)}>
      {/* the minutes and hours, the days, then all of it and a range by hand, apart under rules */}
      {items.map(([k, l], i) => <Fragment key={k}>{(i === 4 || i === 7) && <MenuRule />}
        <button role="menuitem" onClick={() => { setAt(null); onChoose(k); }} className={menuItem}><span className="flex-1">{l}</span>
          {choice.key === k && <Check size={14} strokeWidth={1.75} className="text-muted" />}</button></Fragment>)}
    </PopMenu>}
  </>;
}

/** One bar per unit of the range (a minute to a month, lib/usage.ts) from a shared baseline, the plan's part and the paid
 * part stacked; hover or focus shows the bar. Its times on the app's one clock, its dates as every other date in the
 * window ("Nov 6", not "2026-11-06": design review 10). */
function Bars({ bars, unit, plans, within }: { bars: Bar[]; unit: Unit; /** what the plans' part is called: "Claude plan", "Plans" */ plans: string;
  /** the range in words, for its empty line: "in the last hour" */ within: string }) {
  const [at, setAt] = useState<number | null>(null);
  const max = Math.max(...bars.map((d) => d.plan + d.api), 0);
  if (max === 0) return <div className="px-3.5 py-2.5 text-[12.5px] text-muted">Nothing spent {within}.</div>;
  const d = at == null ? null : bars[at], now = Date.now() / 1000, name = (b: Bar) => barName(b.at, unit, now);
  return (
    <div className="relative px-3.5 pb-2.5 pt-3.5">
      {/* one stop for the keyboard, the arrows along the bars (each bar was a stop of its own and a click did nothing,
          design review 7) */}
      <div className="flex h-[110px] items-end gap-[3px] rounded-sm" onMouseLeave={() => setAt(null)} tabIndex={0} role="group"
        aria-label={d ? `${name(d)}: ${money(d.plan + d.api)}, ${d.runs} runs` : `Spending per ${unit}, ${name(bars[0])} to ${name(bars[bars.length - 1])}: arrows move along the bars`}
        onFocus={() => setAt((a) => a ?? bars.length - 1)} onBlur={() => setAt(null)}
        onKeyDown={(e) => { const n = e.key === "ArrowLeft" ? -1 : e.key === "ArrowRight" ? 1 : e.key === "Home" ? -bars.length : e.key === "End" ? bars.length : 0;
          if (n) { e.preventDefault(); setAt((a) => Math.max(0, Math.min(bars.length - 1, (a ?? bars.length - 1) + n))); } }}>
        {bars.map((bar, i) => (
          <span key={bar.at} onMouseEnter={() => setAt(i)} aria-hidden="true" className="flex h-full min-w-0 flex-1 flex-col justify-end">
            {/* the paid part solid on top, the plan's (counted at API prices, not billed) light under it: one colour in two
                strengths; the paid part was a dark pixel-high cap nobody could see (design review 10) */}
            <span className={`block w-full rounded-t-[3px] ${at === i ? "bg-live-ink" : "bg-live"}`} style={{ height: bar.api ? `${Math.max(3, (100 * bar.api) / max)}%` : 0 }} />
            <span className={`block w-full ${bar.api ? "" : "rounded-t-[3px]"} ${at === i ? "bg-[color-mix(in_srgb,var(--live)_55%,var(--card))]" : "bg-[color-mix(in_srgb,var(--live)_38%,var(--card))]"}`} style={{ height: bar.plan ? `${Math.max(2, (100 * bar.plan) / max)}%` : 0 }} />
          </span>
        ))}
      </div>
      <div className="mt-1.5 flex justify-between text-[11px] tabular-nums text-faint"><span>{name(bars[0])}</span><span>max {money(max)} {perUnit(unit)}</span><span>{name(bars[bars.length - 1])}</span></div>
      <div className="mt-1.5 flex gap-3.5 text-[11.5px] text-muted">
        <span className="flex items-center gap-1.5"><i className="size-2 rounded-sm bg-[color-mix(in_srgb,var(--live)_38%,var(--card))]" />{plans}, at API prices</span>
        <span className="flex items-center gap-1.5"><i className="size-2 rounded-sm bg-live" />Paid APIs</span>
      </div>
      {d && (
        <div className="pointer-events-none absolute top-0 z-10 -translate-x-1/2 whitespace-nowrap rounded-md border border-line-strong bg-card px-2.5 py-1.5 text-[12px] tz-float"
          style={{ left: `${Math.min(88, Math.max(12, ((at! + 0.5) / bars.length) * 100))}%` }}>
          <div className="font-semibold tabular-nums text-ink">{money(d.plan + d.api)}</div>
          <div className="text-muted">{name(d)}, {d.runs} runs</div>
          {d.plan > 0 && d.api > 0 && <div className="tabular-nums text-muted">plan {money(d.plan)}, API {money(d.api)}</div>}
        </div>
      )}
    </div>
  );
}
