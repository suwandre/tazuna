import { ExternalLink, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { core, type Accounts } from "../lib/tazuna";
import { useDialog } from "../lib/dialog";
import { summarize, type Day, type Row } from "../lib/usage";
import { usd } from "../lib/types";

// The usage page (Cmd-Shift-U, the menu, a click on the plan bar or the cost): what each account has left right now
// (Claude's plan from claude's own /usage, OmniRoute's accounts, the providers' credits: src/accounts.rs), what the last
// days cost, and how the user's own runs went. In Settings' language since desktop 0.1.68 (the second design review,
// prototype mockups/v3.html): a section is a heading, a line on what it is, and a card of rows; a zero is a quiet line,
// not a big "$0.000".
const RANGES = [7, 30, 90];
const big = (n: number) => (n >= 1e9 ? `${(n / 1e9).toFixed(1)} B` : n >= 1e6 ? `${(n / 1e6).toFixed(1)} M` : n >= 1e3 ? `${Math.round(n / 1e3)}k` : `${n}`);
const money = (n: number) => (n >= 100 ? `$${Math.round(n)}` : n >= 1 ? `$${n.toFixed(2)}` : usd(n));

/** OmniRoute's resets come as UTC ISO strings: "in 6 d", "in 5 h", "in 12 min". */
const until = (iso: string | null) => {
  const t = iso ? new Date(iso).getTime() : NaN;
  if (Number.isNaN(t)) return null;
  const m = Math.max(0, Math.round((t - Date.now()) / 60000));
  return m >= 1440 ? `in ${Math.floor(m / 1440)} d` : m >= 60 ? `in ${Math.floor(m / 60)} h` : `in ${m} min`;
};

/** claude's window names, shorter: "Current session" is the 5 hours, "Current week (Fable)" the week on Fable. */
const windowName = (label: string) => {
  const l = label.replace(/^Current /, "");
  if (l === "session") return "5 hours";
  const m = l.match(/^week \((.*)\)$/);
  return m ? (m[1] === "all models" ? "Week" : `Week · ${m[1]}`) : l.charAt(0).toUpperCase() + l.slice(1);
};

function Section({ title, note, children }: { title: string; note?: ReactNode; children: ReactNode }) {
  return (
    <section className="mt-6 first:mt-0">
      <h3 className="text-[13px] font-semibold text-ink">{title}</h3>
      {note && <p className="mt-0.5 text-[12px] leading-relaxed text-muted">{note}</p>}
      <div className="mt-2.5 divide-y divide-line-soft overflow-hidden rounded-lg border border-line bg-card">{children}</div>
    </section>
  );
}

/** One account's window: its name and whose, the bar (amber from 75%), the share used and when it resets. */
function Meter({ name, of, used, resets }: { name: string; of: string; used: number | null; resets?: string | null }) {
  const pct = Math.max(0, Math.min(100, used ?? 0));
  return (
    <div className="flex items-center gap-3.5 px-3.5 py-2.5 text-[12.5px]">
      <span className="w-[200px] flex-none"><span className="block truncate text-ink" title={name}>{name}</span><span className="block truncate text-[11.5px] text-faint">{of}</span></span>
      <span className="h-1.5 min-w-0 flex-1 overflow-hidden rounded bg-line"><i className={`block h-full rounded ${pct >= 75 ? "bg-warn" : "bg-accent"}`} style={{ width: `${pct}%` }} /></span>
      <span className="w-[170px] flex-none truncate text-right tabular-nums text-muted"><span className="font-medium text-ink">{used == null ? "–" : `${Math.round(used)}%`}</span>{resets ? ` · ${resets}` : ""}</span>
    </div>
  );
}

/** A figure in a strip of three: its label, the number (or a quiet line when there is nothing), a note. */
function Figure({ label, value, note, quiet }: { label: string; value: string; note?: string; quiet?: boolean }) {
  return (
    <div className="min-w-0 px-3.5 py-3">
      <div className="text-[11.5px] text-faint">{label}</div>
      {quiet ? <div className="pt-1.5 text-[13px] text-muted">{value}</div> : <div className="mt-0.5 text-[20px] font-semibold tabular-nums tracking-[-0.02em] text-ink">{value}</div>}
      {note && !quiet && <div className="mt-0.5 truncate text-[11.5px] text-muted">{note}</div>}
    </div>
  );
}

export function UsageView({ onClose, limits, balance }: { onClose: () => void; limits?: [number, number] | null; balance?: number | null }) {
  const [rows, setRows] = useState<Row[] | null>(null);
  const [acc, setAcc] = useState<Accounts | null | undefined>(undefined);
  const dialog = useDialog();
  const [days, setDays] = useState(30);
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
    <div onMouseDown={onClose} className="fixed inset-0 z-30 grid place-items-center bg-ink/15 p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Usage" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className="flex max-h-full w-[780px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
        <div className="flex items-center gap-3 border-b border-line-soft px-5 py-3">
          <span className="flex-1 font-medium">Usage</span>
          <span className="flex rounded-md border border-line p-0.5 text-[12px]">
            {RANGES.map((d) => <button key={d} onClick={() => setDays(d)} className={`rounded px-2 py-0.5 ${d === days ? "bg-selected text-ink" : "text-muted hover:bg-hover"}`}>{d} days</button>)}
          </span>
          <button onClick={onClose} title="Close (Esc)" aria-label="Close" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="overflow-auto bg-sidebar px-6 pb-7 pt-5">
          <Section title="Right now" note="What each account has left: Claude's plan from claude's own /usage (no model call), OmniRoute's accounts, the providers' credits.">
            <RightNow acc={acc} limits={limits} balance={balance} />
          </Section>

          <Section title="Spending" note={`The last ${days} days from the run log. Claude on your plan is counted at API prices, not billed.`}>
            {!s ? <div className="px-3.5 py-2.5 text-[12.5px] text-faint">Reading the run log…</div> : <>
              <div className="grid grid-cols-3 divide-x divide-line-soft">
                <Figure label="Paid APIs, billed" value={s.api > 0 ? money(s.api) : "Nothing billed in these days"} quiet={s.api <= 0} note={`${apiRuns} runs`} />
                <Figure label="Claude plan, at API prices" value={s.plan > 0 ? money(s.plan) : "No plan runs in these days"} quiet={s.plan <= 0} note={`not billed · ${planRuns} runs`} />
                <Figure label="From the cache" value={s.read ? `${Math.round((100 * s.cached) / s.read)}%` : "Nothing read yet"} quiet={!s.read} note={`of input · ${big(s.cached)} tokens`} />
              </div>
              <Bars days={s.days} />
              {s.byModel.length > 0 && (
                <table className="w-full text-[12.5px]"><thead><tr><th className={th}>Model</th><th className={th}>Runs on</th>
                  <th className={`${th} text-right`}>Runs</th><th className={`${th} text-right`}>Cost</th><th className={`${th} text-right`}>From cache</th></tr></thead>
                  <tbody>{s.byModel.map((m) => (
                    <tr key={`${m.plan}${m.model}`}><td className={`${td} font-mono text-[12px]`}>{m.model}</td>
                      <td className={td}><span className="rounded-full border border-line px-2 text-[11px] text-muted">{m.plan ? "Claude plan" : "API"}</span></td>
                      <td className={num}>{m.runs}</td><td className={num}>{money(m.cost)}</td><td className={num}>{m.cachePct}%</td></tr>
                  ))}</tbody></table>
              )}
            </>}
          </Section>

          <Section title="How your runs went" note={<>Read from what happened next, with nothing for you to label. A clear miss: you stopped or rewound it, it ended in an error,
            its change was thrown away, or your next message corrected it or asked the same again. Pushback: "wait, …", "why …?", "progress?".</>}>
            {!s ? <div className="px-3.5 py-2.5 text-[12.5px] text-faint">Reading the run log…</div>
              : s.byVersion.length === 0 ? <div className="px-3.5 py-2.5 text-[12.5px] text-muted">No runs of your own in these days.</div> : <>
              {s.lanes.length > 0 && <div className="grid divide-x divide-line-soft" style={{ gridTemplateColumns: `repeat(${s.lanes.length}, minmax(0, 1fr))` }}>
                {s.lanes.map((l) => <Figure key={l.lane} label={l.lane} value={`${l.missPct}%`} note={`clear misses · ${l.softPct}% pushback · ${l.runs} runs`} />)}
              </div>}
              <table className="w-full text-[12.5px]"><thead><tr><th className={th}>Version</th><th className={th}>Model</th>
                <th className={`${th} text-right`}>Runs</th><th className={`${th} text-right`}>Clear misses</th><th className={`${th} text-right`}>Pushback</th>
                <th className={th}>Mostly</th></tr></thead>
                <tbody>{s.byVersion.slice(0, 20).map((v) => (
                  <tr key={`${v.version}${v.model}`}><td className={`${td} font-mono text-[12px]`}>{v.version}</td>
                    <td className={`${td} font-mono text-[12px] text-ink-2`}>{v.model}</td><td className={num}>{v.runs}</td><td className={num}>{v.missPct}%</td>
                    <td className={num}>{v.softPct}%</td><td className={`${td} text-[12px] text-muted`}>{v.top}</td></tr>
                ))}</tbody></table>
            </>}
          </Section>
        </div>
      </div>
    </div>
  );
}

/** Every account's windows and credits, one row each. */
function RightNow({ acc, limits, balance }: { acc: Accounts | null | undefined; limits?: [number, number] | null; balance?: number | null }) {
  if (acc === undefined) return <div className="px-3.5 py-2.5 text-[12.5px] text-faint">Asking your accounts…</div>;
  // no reading from claude's /usage: the plan as the last reply's rate limits said it, if any
  const plan = acc?.plan ?? (limits ? [{ label: "Current session", used: limits[0], resets: "" }, { label: "Current week (all models)", used: limits[1], resets: "" }] : []);
  const credits = acc?.credits ?? (balance != null ? [{ provider: "SubRouter", left: `$${balance.toFixed(2)} left`, page: "https://subrouter.ai" }] : []);
  if (!plan.length && !credits.length && !acc?.omniroute.length) return <div className="px-3.5 py-2.5 text-[12.5px] text-muted">No account readings: claude is not logged in to a subscription, and no provider key is set.</div>;
  return <>
    {plan.map((r) => <Meter key={r.label} name={windowName(r.label)} of="Claude plan" used={r.used} resets={r.resets.split(" (")[0]} />)}
    {acc?.omniroute.flatMap((a) => a.windows.map((w, i) => w.unlimited
      ? <div key={`${a.provider}${a.name}${i}`} className="flex items-center gap-3.5 px-3.5 py-2.5 text-[12.5px]"><span className="w-[200px] flex-none"><span className="block text-ink">{[w.group, w.window].filter(Boolean).join(" · ")}</span>
          <span className="block truncate text-[11.5px] text-faint">OmniRoute · {a.provider}{a.plan ? ` · ${a.plan}` : ""}</span></span><span className="text-muted">unlimited</span></div>
      : <Meter key={`${a.provider}${a.name}${i}`} name={[w.group, w.window].filter(Boolean).join(" · ")} used={w.used} resets={until(w.resets)}
          of={`OmniRoute · ${a.provider}${a.plan ? ` · ${a.plan}` : ""}${a.status !== "valid" ? ` · login ${a.status}` : ""}`} />))}
    {credits.map((c) => (
      <div key={c.provider} className="flex items-center gap-3.5 px-3.5 py-2.5 text-[12.5px]">
        <span className="w-[200px] flex-none"><span className="block text-ink">{c.provider}</span><span className="block text-[11.5px] text-faint">credit</span></span>
        {c.left ? <><span className="flex-1" /><span className={`tabular-nums ${c.left.startsWith("-") ? "text-bad" : "text-muted"}`}><span className="font-medium text-ink">{c.left.replace(/ left.*/, "")}</span>{c.left.match(/ left.*/)?.[0]}</span></>
          : <><span className="flex-1 text-faint">its API does not report a balance</span>
            <button onClick={() => core.openLink(c.page).catch(() => {})} className="flex items-center gap-1 rounded-md border border-line px-2 py-0.5 text-[12px] text-ink">Open its page<ExternalLink size={12} strokeWidth={1.5} /></button></>}
      </div>
    ))}
    {acc && acc.plan_notes.length > 0 && (
      <details className="px-3.5 py-2.5 text-[12.5px]"><summary className="cursor-pointer text-muted hover:text-ink">What's contributing to your Claude usage</summary>
        <pre className="mt-1.5 whitespace-pre-wrap font-sans text-[12px] leading-relaxed text-muted">{acc.plan_notes.filter((n) => !n.startsWith("Approximate")).join("\n")}</pre></details>
    )}
  </>;
}

/** One bar per day from a shared baseline, the plan's part and the paid part stacked; hover or focus shows the day. */
function Bars({ days }: { days: Day[] }) {
  const [at, setAt] = useState<number | null>(null);
  const max = Math.max(...days.map((d) => d.plan + d.api), 0);
  if (max === 0) return <div className="px-3.5 py-2.5 text-[12.5px] text-muted">Nothing spent in these days.</div>;
  const d = at == null ? null : days[at];
  return (
    <div className="relative px-3.5 pb-2.5 pt-3.5">
      <div className="flex h-[110px] items-end gap-[3px]" onMouseLeave={() => setAt(null)}>
        {days.map((day, i) => (
          <button key={day.day} onMouseEnter={() => setAt(i)} onFocus={() => setAt(i)} onBlur={() => setAt(null)}
            aria-label={`${day.day}: ${money(day.plan + day.api)}, ${day.runs} runs`} className="flex h-full min-w-0 flex-1 flex-col justify-end">
            <span className={`block w-full rounded-t-[3px] bg-accent-soft outline outline-1 -outline-offset-1 outline-accent/35 ${at === i ? "opacity-80" : ""}`} style={{ height: day.api ? `${Math.max(2, (100 * day.api) / max)}%` : 0 }} />
            <span className={`block w-full ${day.api ? "" : "rounded-t-[3px]"} ${at === i ? "bg-accent-ink" : "bg-accent"}`} style={{ height: day.plan ? `${Math.max(2, (100 * day.plan) / max)}%` : 0 }} />
          </button>
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
          <div className="text-muted">{d.day} · {d.runs} runs</div>
          {d.plan > 0 && d.api > 0 && <div className="tabular-nums text-muted">plan {money(d.plan)} · API {money(d.api)}</div>}
        </div>
      )}
    </div>
  );
}
