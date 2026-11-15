import type { ReactNode } from "react";
import type { Usage } from "../lib/types";

// The right rail is a slot of widgets (docs/DESKTOP.md "Customization"): tazuna's own panels are widgets too, so the
// widget API that users and creator mode build on is the one these use.
export type Widget = { id: string; title: string; render: (u: Usage) => ReactNode };

function Card({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="rounded-lg border border-line bg-card px-3.5 py-3">
      <div className="mb-2 text-[11.5px] font-medium text-faint">{title}</div>
      {children}
    </div>
  );
}

function Bar({ pct }: { pct: number }) {
  return <div className="h-1.5 overflow-hidden rounded bg-line"><i className="block h-full rounded bg-accent transition-[width] duration-200" style={{ width: `${pct}%` }} /></div>;
}

function Ring({ pct }: { pct: number }) {
  return (
    <div className="relative my-0.5 mb-1.5 size-16">
      <svg viewBox="0 0 36 36" className="size-16 -rotate-90">
        <circle cx="18" cy="18" r="15.9" fill="none" strokeWidth="3.2" stroke="var(--line)" />
        <circle cx="18" cy="18" r="15.9" fill="none" strokeWidth="3.2" stroke="var(--accent)" strokeLinecap="round" strokeDasharray={`${pct} 100`} />
      </svg>
      <span className="absolute inset-0 grid place-items-center text-[14px] font-semibold">{pct}%</span>
    </div>
  );
}

function Spark({ series }: { series: number[] }) {
  const max = Math.max(...series), step = 120 / (series.length - 1);
  const d = series.map((v, i) => `${i ? "L" : "M"}${(i * step).toFixed(1)} ${(30 - (v / max) * 26).toFixed(1)}`).join(" ");
  return <svg viewBox="0 0 120 32" preserveAspectRatio="none" className="h-[34px] w-full"><path d={d} fill="none" stroke="var(--accent)" strokeWidth="1.6" /></svg>;
}

const small = "mt-1.5 text-[11.5px] text-muted";

export const builtInWidgets: Widget[] = [
  { id: "context", title: "Context", render: (u) => <><Ring pct={u.contextPct} /><div className={small}>{u.contextUsed} of {u.contextMax}</div></> },
  { id: "plan", title: "Plan · 5 hours", render: (u) => <>
      <Bar pct={u.plan5h} /><div className={small}>{u.plan5h}% used · resets {u.plan5hReset}</div>
      <div className="mb-2 mt-3 text-[11.5px] font-medium text-faint">Plan · week</div><Bar pct={u.planWeek} /><div className={small}>{u.planWeek}% used</div></> },
  { id: "thread", title: "This thread", render: (u) => <>
      <div className="text-[22px] font-semibold tracking-[-0.02em]">${u.threadCost.toFixed(3)}</div>
      <div className={small}>{u.threadTurns} turns · {u.cachedPct}% cached · saved ${u.saved.toFixed(2)}</div></> },
  { id: "today", title: "Today", render: (u) => <><Spark series={u.todaySeries} /><div className={small}>${u.today.toFixed(2)} · {u.todayTurns} turns</div></> },
];

export function Rail({ usage, widgets }: { usage: Usage; widgets: Widget[] }) {
  return (
    <aside className="flex flex-col gap-3 border-l border-line bg-sidebar px-3.5 py-4">
      {widgets.map((w) => <Card key={w.id} title={w.title}>{w.render(usage)}</Card>)}
    </aside>
  );
}
