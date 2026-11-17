import { X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { core } from "../lib/tazuna";
import { useDialog } from "../lib/dialog";
import { summarize, type Day, type Row } from "../lib/usage";
import { usd } from "../lib/types";

// The usage page (Cmd-K, the View menu): what the last days cost by day and model, how much input the cache served, the plan
// now, and what the run log knows of trouble per version and model. Read from the run log (runs.jsonl) by the core.
const RANGES = [7, 30, 90];
const big = (n: number) => (n >= 1e9 ? `${(n / 1e9).toFixed(1)} B` : n >= 1e6 ? `${(n / 1e6).toFixed(1)} M` : n >= 1e3 ? `${Math.round(n / 1e3)}k` : `${n}`);
const money = (n: number) => (n >= 100 ? `$${Math.round(n)}` : n >= 1 ? `$${n.toFixed(2)}` : usd(n));

export function UsageView({ onClose, limits, balance }: { onClose: () => void; limits?: [number, number] | null; balance?: number | null }) {
  const [rows, setRows] = useState<Row[] | null>(null);
  const dialog = useDialog();
  const [days, setDays] = useState(30);
  // asked once when opened (the window re-renders on every streamed view; onClose is new each time)
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const off = core.onUsage((u) => setRows(u.rows));
    core.usage(true).catch(() => setRows([]));
    const esc = (e: KeyboardEvent) => { if (e.key === "Escape") close.current(); };
    window.addEventListener("keydown", esc);
    return () => { off.then((f) => f()); window.removeEventListener("keydown", esc); };
  }, []);
  const s = useMemo(() => (rows ? summarize(rows, days) : null), [rows, days]);
  const label = "mb-2 mt-6 text-[11.5px] font-medium text-faint";
  const th = "py-1.5 pr-3 text-left font-medium text-faint", td = "py-1.5 pr-3", num = "py-1.5 pr-3 text-right font-mono text-[12px]";
  return (
    <div onMouseDown={onClose} className="fixed inset-0 z-30 grid place-items-center bg-ink/15 p-10">
      <div ref={dialog} role="dialog" aria-modal="true" aria-label="Usage" tabIndex={-1} onMouseDown={(e) => e.stopPropagation()} className="flex max-h-full w-[780px] flex-col overflow-hidden rounded-xl border border-line-strong bg-card shadow-composer">
        <div className="flex items-center gap-3 border-b border-line-soft px-5 py-3">
          <span className="flex-1 font-medium">Usage</span>
          <span className="flex rounded-md border border-line p-0.5 text-[12px]">
            {RANGES.map((d) => <button key={d} onClick={() => setDays(d)} className={`rounded px-2 py-0.5 ${d === days ? "bg-selected text-ink" : "text-muted hover:bg-hover"}`}>{d} days</button>)}
          </span>
          <button onClick={onClose} title="Close (Esc)" className="rounded p-1 text-muted hover:bg-hover"><X size={15} strokeWidth={1.5} /></button>
        </div>
        <div className="overflow-auto px-5 pb-5 pt-4">
          {!s && <div className="text-[12.5px] text-faint">Reading the run log…</div>}
          {s && <>
            <div className="grid grid-cols-4 gap-3">
              <Tile title="API spend" value={money(s.api)} note={`billed · ${s.byModel.filter((m) => !m.plan).reduce((a, m) => a + m.runs, 0)} runs`} />
              <Tile title="Claude plan" value={money(s.plan)} note={`at API prices, not billed · ${s.byModel.filter((m) => m.plan).reduce((a, m) => a + m.runs, 0)} runs`} />
              <Tile title="From the cache" value={`${s.read ? Math.round((100 * s.cached) / s.read) : 0}%`} note={`of input · ${big(s.cached)} tokens`} />
              <Tile title="Plan now" value={limits ? `${Math.round(limits[0])}%` : "–"} note={limits ? `of 5 hours used · week ${Math.round(limits[1])}%${balance != null ? ` · SubRouter $${balance.toFixed(2)}` : ""}` : "known after a Claude reply"} />
            </div>

            <div className={label}>Cost per day, at API prices (plan runs included, not billed)</div>
            <Bars days={s.days} />

            <div className={label}>By model</div>
            {s.byModel.length === 0 ? <div className="text-[12.5px] text-faint">No runs in these days.</div> : (
              <table className="w-full text-[12.5px]"><thead><tr className="border-b border-line-soft"><th className={th}>Model</th><th className={th}>Runs on</th>
                <th className={`${th} text-right`}>Runs</th><th className={`${th} text-right`}>Cost</th><th className={`${th} text-right`}>From cache</th></tr></thead>
                <tbody>{s.byModel.map((m) => (
                  <tr key={`${m.plan}${m.model}`} className="border-b border-line-soft last:border-0"><td className={`${td} font-mono text-[12px]`}>{m.model}</td>
                    <td className={`${td} text-muted`}>{m.plan ? "Claude plan" : "API"}</td><td className={num}>{m.runs}</td><td className={num}>{money(m.cost)}</td><td className={num}>{m.cachePct}%</td></tr>
                ))}</tbody></table>
            )}

            <div className={label}>How your own runs went (real use, not benchmarks)</div>
            <div className="mb-1.5 text-[12px] text-muted">Read from what happened next, with nothing for you to label. A clear miss: you stopped or rewound it, it ended
              in an error, its change was thrown away, or your next message corrected it or asked the same again. Pushback: "wait, …", "why …?", "progress?".</div>
            {s.lanes.length > 0 && <div className="mb-2 flex gap-5 text-[12.5px]">{s.lanes.map((l) => (
              <span key={l.lane}><span className="text-ink-2">{l.lane}</span> <span className="font-mono">{l.missPct}%</span> <span className="text-muted">clear misses, {l.softPct}% pushback · {l.runs} runs</span></span>
            ))}</div>}
            {s.byVersion.length === 0 ? <div className="text-[12.5px] text-faint">No runs of your own in these days.</div> : (
              <table className="w-full text-[12.5px]"><thead><tr className="border-b border-line-soft"><th className={th}>Version</th><th className={th}>Model</th>
                <th className={`${th} text-right`}>Runs</th><th className={`${th} text-right`}>Clear misses</th><th className={`${th} text-right`}>Pushback</th>
                <th className={th}>Mostly</th></tr></thead>
                <tbody>{s.byVersion.slice(0, 20).map((v) => (
                  <tr key={`${v.version}${v.model}`} className="border-b border-line-soft last:border-0"><td className={`${td} font-mono text-[12px]`}>{v.version}</td>
                    <td className={`${td} font-mono text-[12px] text-ink-2`}>{v.model}</td><td className={num}>{v.runs}</td><td className={num}>{v.missPct}%</td>
                    <td className={num}>{v.softPct}%</td><td className={`${td} text-[12px] text-muted`}>{v.top}</td></tr>
                ))}</tbody></table>
            )}
          </>}
        </div>
      </div>
    </div>
  );
}

function Tile({ title, value, note }: { title: string; value: string; note: string }) {
  return (
    <div className="rounded-lg border border-line px-3.5 py-3">
      <div className="mb-1 text-[11.5px] font-medium text-faint">{title}</div>
      <div className="text-[22px] font-semibold tracking-[-0.02em]">{value}</div>
      <div className="mt-0.5 text-[11.5px] text-muted">{note}</div>
    </div>
  );
}

/** One bar per day from a shared baseline, the day's total; hover or focus shows the day, its plan and API parts and runs. */
function Bars({ days }: { days: Day[] }) {
  const [at, setAt] = useState<number | null>(null);
  const max = Math.max(...days.map((d) => d.plan + d.api), 0);
  if (max === 0) return <div className="text-[12.5px] text-faint">Nothing spent in these days.</div>;
  const d = at == null ? null : days[at];
  return (
    <div className="relative">
      <div className="flex h-[120px] items-end gap-[2px] border-b border-line" onMouseLeave={() => setAt(null)}>
        {days.map((day, i) => {
          const total = day.plan + day.api;
          return (
            <button key={day.day} onMouseEnter={() => setAt(i)} onFocus={() => setAt(i)} onBlur={() => setAt(null)}
              aria-label={`${day.day}: ${money(total)}, ${day.runs} runs`} className="flex h-full min-w-0 flex-1 items-end justify-center">
              <span className={`block w-full max-w-[24px] rounded-t-[4px] ${at === i ? "bg-accent-ink" : "bg-accent"}`} style={{ height: total ? `${Math.max(2, (100 * total) / max)}%` : 0 }} />
            </button>
          );
        })}
      </div>
      <div className="mt-1 flex justify-between font-mono text-[11px] text-faint"><span>{days[0].day.slice(5)}</span><span>max {money(max)}</span><span>{days[days.length - 1].day.slice(5)}</span></div>
      {d && (
        <div className="pointer-events-none absolute -top-2 z-10 -translate-x-1/2 -translate-y-full whitespace-nowrap rounded-md border border-line-strong bg-card px-2.5 py-1.5 text-[12px] shadow-composer"
          style={{ left: `${Math.min(88, Math.max(12, ((at! + 0.5) / days.length) * 100))}%` }}>
          <div className="font-semibold text-ink">{money(d.plan + d.api)}</div>
          <div className="text-muted">{d.day} · {d.runs} runs</div>
          {d.plan > 0 && d.api > 0 && <div className="text-muted">plan {money(d.plan)} · API {money(d.api)}</div>}
        </div>
      )}
    </div>
  );
}
