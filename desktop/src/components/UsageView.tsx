import { ChevronDown, ChevronRight, ExternalLink, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { core, type Accounts, type ModelChoice } from "../lib/tazuna";
import { useDialog } from "../lib/dialog";
import { summarize, type Day, type Row } from "../lib/usage";
import { modelLabel } from "../lib/types";
import { exactly, parseReset, whenReset } from "../lib/health";
import { Sep } from "./Sep";

// The usage page (Cmd-Shift-U, the menu, a click on the plan bar or the cost): what each account has left right now
// (Claude's plan from claude's own /usage, OmniRoute's accounts, the providers' credits: src/accounts.rs), what the last
// days cost, and how the user's own runs went. In Settings' language since desktop 0.1.68 (the second design review,
// prototype mockups/v3.html): a section is a heading, a line on what it is, and a card of rows; a zero is a quiet line,
// not a big "$0.000".
const RANGES = [7, 30, 90];
const big = (n: number) => (n >= 1e9 ? `${(n / 1e9).toFixed(1)} B` : n >= 1e6 ? `${(n / 1e6).toFixed(1)} M` : n >= 1e3 ? `${Math.round(n / 1e3)}k` : `${n}`);
// totals in cents: "$0" for nothing, "< $0.01" below a cent ("$0.000072" beside "$0.26" in one column, design review 5)
const money = (n: number) => (n <= 0 ? "$0" : n < 0.01 ? "< $0.01" : n >= 100 ? `$${Math.round(n)}` : `$${n.toFixed(2)}`);

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
      <span className="h-1.5 min-w-0 flex-1 overflow-hidden rounded bg-line"><i className={`block h-full rounded ${pct >= 90 ? "bg-bad" : pct >= 75 ? "bg-warn" : "bg-accent"}`} style={{ width: `${pct}%` }} /></span>
      <span className="w-[220px] flex-none truncate text-right tabular-nums text-muted"><span className="font-medium text-ink">{used == null ? "–" : `${Math.round(used)}%`}</span>{resets && <><Sep /><span title={exactly(at)}>resets {resets}</span></>}</span>
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

export function UsageView({ onClose, limits, balance, choices = [] }: { onClose: () => void; limits?: [number, number] | null; balance?: number | null;
  /** the model catalog: names the provider of an API run the run log did not record (one recorded since 0.1.182) */
  choices?: ModelChoice[] }) {
  const [rows, setRows] = useState<Row[] | null>(null);
  const [acc, setAcc] = useState<Accounts | null | undefined>(undefined);
  const dialog = useDialog();
  const [days, setDays] = useState(30);
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
    const off = core.onUsage((u) => setRows(u.rows));
    core.usage(true).catch(() => setRows([]));
    const offAcc = core.onAccounts(setAcc);
    core.accounts().catch(() => setAcc(null));
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") close.current(); };
    window.addEventListener("keydown", esc);
    return () => { off.then((f) => f()); offAcc.then((f) => f()); window.removeEventListener("keydown", esc); };
  }, []);
  const s = useMemo(() => (rows ? summarize(rows, days) : null), [rows, days]);
  const th = "px-3.5 pb-1.5 pt-2.5 text-left text-[11.5px] font-medium text-faint", td = "border-t border-line-soft px-3.5 py-[7px]";
  const num = `${td} text-right tabular-nums`;
  const apiRuns = s ? s.byModel.filter((m) => !m.plan).reduce((a, m) => a + m.runs, 0) : 0, planRuns = s ? s.byModel.filter((m) => m.plan).reduce((a, m) => a + m.runs, 0) : 0;
  return (
    <div onMouseDown={onClose} className="tz-fade fixed inset-0 z-30 grid place-items-center bg-scrim p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Usage" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className="tz-dialog flex h-full max-h-[900px] w-[780px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
        <div className="flex items-center gap-3 border-b border-line-soft px-5 py-3">
          <span className="flex-1 font-medium">Usage</span>
          <button onClick={onClose} title="Close (Esc)" aria-label="Close" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="overflow-auto bg-sidebar px-6 pb-7 pt-5">
          <Section title="Right now" note="What each account has left: your Claude plan, your OmniRoute accounts and the credit with each provider.">
            <RightNow acc={acc} limits={limits} balance={balance} />
          </Section>

          {/* the range by the sections it changes, not in the page's header: there it read as changing "Right now" too,
              which it never did (the user, desktop 0.1.72) */}
          <Section title="Spending" action={
            <span role="radiogroup" aria-label="Days" className="flex rounded-md border border-line p-0.5 text-[12px]">
              {RANGES.map((d) => <button key={d} role="radio" aria-checked={d === days} onClick={() => setDays(d)} className={`rounded px-2 py-0.5 ${d === days ? "bg-selected text-ink" : "text-muted hover:bg-hover"}`}>{d} days</button>)}
            </span>} note={`The last ${days} days from the run log. Claude on your plan is counted at API prices, not billed.`}>
            {!s ? <div className="px-3.5 py-2.5 text-[12.5px] text-faint">Reading the run log…</div> : <>
              <div className="grid grid-cols-3 divide-x divide-line-soft">
                <Figure label="API providers" value={s.api > 0 ? money(s.api) : "Nothing billed in these days"} quiet={s.api <= 0} caption="billed" more={[[`${apiRuns}`, "runs"]]} />
                <Figure label="Claude plan" value={s.plan > 0 ? money(s.plan) : "No plan runs in these days"} quiet={s.plan <= 0} caption="at API prices, not billed" more={[[`${planRuns}`, "runs"]]} />
                <Figure label="From the cache" value={s.read ? `${Math.round((100 * s.cached) / s.read)}%` : "Nothing read yet"} quiet={!s.read} caption="of the input" more={[[big(s.cached), "tokens"]]} />
              </div>
              <Bars days={s.days} />
              {s.byModel.length > 0 && (
                <table className="w-full text-[12.5px]"><thead><tr><th className={th}>Model</th><th className={th}>Provider</th>
                  <th className={`${th} text-right`}>Runs</th><th className={`${th} text-right`}>Cost</th><th className={`${th} text-right`}>From cache</th></tr></thead>
                  <tbody>{(allModels ? s.byModel : s.byModel.slice(0, 8)).map((m) => (
                    <tr key={`${m.plan}${m.model}${m.provider}`}><td className={td} title={m.model}>{modelLabel(m.model)}</td>
                      <td className={`${td} text-muted`}>{m.plan ? "Claude plan" : m.provider ?? providerOf(m.model) ?? <span className="text-faint" title="An API run from before tazuna 0.1.182, which did not record its provider; several providers serve this model">Not recorded</span>}</td>
                      <td className={num}>{m.runs}</td><td className={num}>{money(m.cost)}</td><td className={num}>{m.cachePct}%</td></tr>
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

          <Section title="How your runs went" note={<>The same {days} days as Spending, read from what happened next, with nothing for you to label. A clear miss: you stopped or rewound it, it ended in an error,
            its change was thrown away, or your next message corrected it or asked the same again. Pushback: "wait, …", "why …?", "progress?".</>}>
            {!s ? <div className="px-3.5 py-2.5 text-[12.5px] text-faint">Reading the run log…</div>
              : s.byVersion.length === 0 ? <div className="px-3.5 py-2.5 text-[12.5px] text-muted">No runs of your own in these days.</div> : <>
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
          </Section>
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
function RightNow({ acc, limits, balance }: { acc: Accounts | null | undefined; limits?: [number, number] | null; balance?: number | null }) {
  if (acc === undefined) return <div className="px-3.5 py-2.5 text-[12.5px] text-faint">Asking your accounts…</div>;
  // no reading from claude's /usage: the plan as the last reply's rate limits said it, if any
  const plan = acc?.plan ?? (limits ? [{ label: "Current session", used: limits[0], resets: "" }, { label: "Current week (all models)", used: limits[1], resets: "" }] : []);
  const credits = acc?.credits ?? (balance != null ? [{ provider: "SubRouter", left: `$${balance.toFixed(2)} left`, page: "https://subrouter.ai" }] : []);
  if (!plan.length && !credits.length && !acc?.omniroute.length) return <div className="px-3.5 py-2.5 text-[12.5px] text-muted">No account readings: claude is not logged in to a subscription, and no provider key is set.</div>;
  return <>
    {/* each account a group under its heading, as OmniRoute's (until desktop 0.1.82 "Claude plan" and "credit" under every row) */}
    {plan.length > 0 && <div className={groupHead}><span className="font-medium text-ink-2">Claude plan</span></div>}
    {plan.map((r) => <Meter key={r.label} name={windowName(r.label)} used={r.used} at={parseReset(r.resets)} />)}
    {/* each OmniRoute account under its own heading, written out whole and wrapping (provider, account, plan, login); its
        windows below by name alone. In one 200 px line under each window the account and plan were cut to "…" with no way
        to read them (the user, desktop 0.1.80) */}
    {acc?.omniroute.map((a) => (
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
    {credits.length > 0 && <div className={groupHead}><span className="font-medium text-ink-2">Credit with each provider</span></div>}
    {credits.map((c) => (
      <div key={c.provider} className="flex items-center gap-3.5 px-3.5 py-2.5 text-[12.5px]">
        <span className="w-[200px] flex-none text-ink">{c.provider}</span>
        {c.left ? <><span className="flex-1" /><span className={`tabular-nums ${/ over /.test(c.left) || c.left.startsWith("-") ? "text-bad" : "text-muted"}`}><span className="font-medium text-ink">{c.left.replace(/ (left|over).*/, "")}</span>{c.left.match(/ (left|over).*/)?.[0]}</span></>
          : <><span className="flex-1 text-faint">its API does not report a balance</span>
            <button onClick={() => core.openLink(c.page).catch(() => {})} className="flex items-center gap-1 rounded-md border border-line px-2 py-0.5 text-[12px] text-ink">Open its page<ExternalLink size={12} strokeWidth={1.5} /></button></>}
      </div>
    ))}
    {acc && acc.plan_notes.length > 0 && <Contributing notes={acc.plan_notes} />}
  </>;
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
            {all.length > 1 && <span role="radiogroup" aria-label="Period" className="flex rounded-md border border-line p-0.5 text-[12px]">
              {all.map((q, i) => <button key={q.period} role="radio" aria-checked={q === p} onClick={() => setAt(i)}
                className={`rounded px-2 py-0.5 ${q === p ? "bg-selected text-ink" : "text-muted hover:bg-hover"}`}>Last {name(q.period)}</button>)}</span>}
            <span className="ml-auto text-[11.5px] tabular-nums text-faint">{p.stats}</span>
          </div>
          {p.items.map((it) => (
            <div key={it.title} className="flex items-center gap-3.5 px-3.5 py-[7px] text-[12.5px]">
              <span className={`${lead} flex items-center gap-2`} title={`${it.pct}% of the period's use`}>
                <span className="w-9 text-right font-medium tabular-nums text-ink">{it.pct}%</span>
                <span className="h-1 flex-1 overflow-hidden rounded bg-line"><i className="block h-full rounded bg-accent" style={{ width: `${Math.min(100, it.pct)}%` }} /></span>
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

/** One bar per day from a shared baseline, the plan's part and the paid part stacked; hover or focus shows the day. */
function Bars({ days }: { days: Day[] }) {
  const [at, setAt] = useState<number | null>(null);
  const max = Math.max(...days.map((d) => d.plan + d.api), 0);
  if (max === 0) return <div className="px-3.5 py-2.5 text-[12.5px] text-muted">Nothing spent in these days.</div>;
  const d = at == null ? null : days[at];
  return (
    <div className="relative px-3.5 pb-2.5 pt-3.5">
      {/* one stop for the keyboard, the arrows along the days (each bar was a stop of its own and a click did nothing,
          design review 7) */}
      <div className="flex h-[110px] items-end gap-[3px] rounded-sm" onMouseLeave={() => setAt(null)} tabIndex={0} role="group"
        aria-label={d ? `${d.day}: ${money(d.plan + d.api)}, ${d.runs} runs` : `Spending per day, ${days[0].day} to ${days[days.length - 1].day}: arrows move along the days`}
        onFocus={() => setAt((a) => a ?? days.length - 1)} onBlur={() => setAt(null)}
        onKeyDown={(e) => { const n = e.key === "ArrowLeft" ? -1 : e.key === "ArrowRight" ? 1 : e.key === "Home" ? -days.length : e.key === "End" ? days.length : 0;
          if (n) { e.preventDefault(); setAt((a) => Math.max(0, Math.min(days.length - 1, (a ?? days.length - 1) + n))); } }}>
        {days.map((day, i) => (
          <span key={day.day} onMouseEnter={() => setAt(i)} aria-hidden="true" className="flex h-full min-w-0 flex-1 flex-col justify-end">
            <span className={`block w-full rounded-t-[3px] bg-accent-soft outline outline-1 -outline-offset-1 outline-accent/35 ${at === i ? "opacity-80" : ""}`} style={{ height: day.api ? `${Math.max(2, (100 * day.api) / max)}%` : 0 }} />
            <span className={`block w-full ${day.api ? "" : "rounded-t-[3px]"} ${at === i ? "bg-accent-ink" : "bg-accent"}`} style={{ height: day.plan ? `${Math.max(2, (100 * day.plan) / max)}%` : 0 }} />
          </span>
        ))}
      </div>
      <div className="mt-1.5 flex justify-between text-[11px] tabular-nums text-faint"><span>{days[0].day.slice(5)}</span><span>max {money(max)} a day</span><span>{days[days.length - 1].day.slice(5)}</span></div>
      <div className="mt-1.5 flex gap-3.5 text-[11.5px] text-muted">
        <span className="flex items-center gap-1.5"><i className="size-2 rounded-sm bg-accent" />Claude plan</span>
        <span className="flex items-center gap-1.5"><i className="size-2 rounded-sm bg-accent-soft outline outline-1 -outline-offset-1 outline-accent/35" />paid APIs</span>
      </div>
      {d && (
        <div className="pointer-events-none absolute top-0 z-10 -translate-x-1/2 whitespace-nowrap rounded-md border border-line-strong bg-card px-2.5 py-1.5 text-[12px] shadow-composer"
          style={{ left: `${Math.min(88, Math.max(12, ((at! + 0.5) / days.length) * 100))}%` }}>
          <div className="font-semibold tabular-nums text-ink">{money(d.plan + d.api)}</div>
          <div className="text-muted">{d.day}, {d.runs} runs</div>
          {d.plan > 0 && d.api > 0 && <div className="tabular-nums text-muted">plan {money(d.plan)}, API {money(d.api)}</div>}
        </div>
      )}
    </div>
  );
}
