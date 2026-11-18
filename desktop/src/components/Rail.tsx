import { Pencil, X } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { Slot } from "../lib/slots";
import { tokens, widgetUrl, type UserWidget } from "../lib/widgets";
import { usd, type Message, type Usage } from "../lib/types";

// The right rail is a slot of widgets (docs/DESKTOP.md "Customization"): tazuna's own panels are widgets too, so the
// widget API that users and creator mode build on is the one these use. A widget with nothing real to show returns null
// and its card is left out (DESIGN.md: real numbers only).
export type Widget = { id: string; title: string; render: (u: Usage) => ReactNode | null };

/** A card; with onOpen, the whole card is a button (the plan and cost cards open the usage page). */
export function Card({ title, children, onOpen }: { title: string; children: ReactNode; onOpen?: () => void }) {
  const body = <><div className="mb-2 text-[11.5px] font-medium text-faint">{title}</div>{children}</>;
  return onOpen
    ? <button onClick={onOpen} title="Usage: all your accounts (⌘⇧U)" className="block w-full rounded-lg border border-line bg-card px-3.5 py-3 text-left hover:border-line-strong hover:bg-hover">{body}</button>
    : <div className="rounded-lg border border-line bg-card px-3.5 py-3">{body}</div>;
}

/** Opens the usage page (App listens). */
export const openUsage = () => window.dispatchEvent(new CustomEvent("tazuna-usage"));

function Bar({ pct }: { pct: number }) {
  return <div className="h-1.5 overflow-hidden rounded bg-line"><i className="block h-full rounded bg-accent transition-[width] duration-200" style={{ width: `${pct}%` }} /></div>;
}

function Ring({ pct }: { pct: number }) {
  return (
    <div className="relative my-0.5 mb-1.5 size-16">
      <svg viewBox="0 0 36 36" className="size-16 -rotate-90">
        <circle cx="18" cy="18" r="15.9" fill="none" strokeWidth="3.2" stroke="var(--line)" />
        <circle cx="18" cy="18" r="15.9" fill="none" strokeWidth="3.2" stroke="var(--accent)" strokeLinecap="round" strokeDasharray={`${pct} 100`} opacity={pct > 0 ? 1 : 0} />
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
  { id: "plan", title: "Plan · 5 hours", render: (u) => u.plan5h == null ? null : <>
      <Bar pct={u.plan5h} /><div className={small}>{u.plan5h}% used{u.plan5hReset && ` · resets ${u.plan5hReset}`}</div>
      {u.planWeek != null && <><div className="mb-2 mt-3 text-[11.5px] font-medium text-faint">Plan · week</div><Bar pct={u.planWeek} /><div className={small}>{u.planWeek}% used</div></>}</> },
  { id: "thread", title: "This thread", render: (u) => <>
      <div className="text-[22px] font-semibold tracking-[-0.02em]">{usd(u.threadCost)}</div>
      <div className={small}>{u.threadTurns} {u.threadTurns === 1 ? "turn" : "turns"} · {u.cachedPct}% cached{u.saved > 0 && ` · saved $${u.saved.toFixed(2)}`}</div></> },
  { id: "balance", title: "SubRouter", render: (u) => u.balance == null ? null : <>
      <div className="text-[22px] font-semibold tracking-[-0.02em]">${u.balance.toFixed(2)}</div><div className={small}>balance left</div></> },
  { id: "today", title: "Today", render: (u) => u.todaySeries.length < 2 ? null : <><Spark series={u.todaySeries} /><div className={small}>${u.today.toFixed(2)} · {u.todayTurns} turns</div></> },
];

/** Your messages in this conversation, newest last: a click scrolls the transcript to one (the prompt timeline). */
export function Timeline({ messages }: { messages: Message[] }) {
  const mine = messages.flatMap((m, i) => m.role === "user" && !m.queued ? [{ i, text: m.text.replace(/^\[tazuna: [^\]]*\]\n\n/, "").split("\n")[0] }] : []);
  if (mine.length < 2) return null;
  return (
    <Card title={`Your messages · ${mine.length}`}>
      <div className="-mx-1 flex max-h-56 flex-col overflow-y-auto">
        {mine.map((m) => (
          <button key={m.i} title={m.text} onClick={() => window.dispatchEvent(new CustomEvent("tazuna-jump", { detail: m.i }))}
            className="truncate rounded px-1 py-[3px] text-left text-[12px] text-ink-2 hover:bg-hover">{m.text || "(attachment)"}</button>
        ))}
      </div>
    </Card>
  );
}

/** A user's panel: its page from the tzwidget: scheme in a sandboxed frame (no same origin: it cannot reach tazuna), the
 * data posted in on every change, its height as it asks (40-600 px). */
export function UserPanel({ w, data, onEdit, onRemove }: { w: UserWidget; data: unknown; onEdit?: () => void; onRemove?: () => void }) {
  const frame = useRef<HTMLIFrameElement>(null);
  const [height, setHeight] = useState(160);
  const [sure, setSure] = useState(false);
  const post = () => frame.current?.contentWindow?.postMessage({ type: "tazuna", data, tokens: tokens() }, "*");
  useEffect(post, [data]);
  useEffect(() => {
    const on = (e: MessageEvent) => {
      if (e.source !== frame.current?.contentWindow || e.data?.type !== "tazuna-height") return;
      const h = Number(e.data.height);
      if (Number.isFinite(h)) setHeight(Math.max(40, Math.min(600, Math.round(h))));
    };
    window.addEventListener("message", on);
    return () => window.removeEventListener("message", on);
  }, []);
  return (
    <div className="group rounded-lg border border-line bg-card px-3.5 py-3">
      <div className="mb-2 flex items-center gap-1 text-[11.5px] font-medium text-faint">
        <span className="flex-1 truncate">{w.title}</span>
        {sure ? <>
          <button onClick={onRemove} className="text-bad">Remove</button><button onClick={() => setSure(false)} className="ml-1.5">Keep</button>
        </> : <span className="hidden gap-0.5 group-hover:flex">
          {onEdit && <button title="Change this panel with tazuna" onClick={onEdit} className="rounded p-0.5 hover:bg-hover hover:text-ink"><Pencil size={12} strokeWidth={1.5} /></button>}
          {onRemove && <button title="Remove this panel (its folder moves to .removed)" onClick={() => setSure(true)} className="rounded p-0.5 hover:bg-hover hover:text-ink"><X size={12} strokeWidth={1.5} /></button>}
        </span>}
      </div>
      <iframe ref={frame} src={widgetUrl(w.id)} sandbox="allow-scripts" title={w.title} onLoad={post}
        style={{ height }} className="block w-full border-0 bg-transparent" />
    </div>
  );
}

/** The right column: the `rail` spot (tazuna's cards and the user's panels and plugins). */
export function Rail({ usage, data, messages }: { usage: Usage; data?: unknown; messages: Message[] }) {
  return (
    <aside aria-label="This conversation" className="flex min-h-0 flex-col gap-3 overflow-y-auto border-l border-line bg-sidebar px-3.5 py-4">
      <Slot name="rail" props={{ usage, data, messages }} />
    </aside>
  );
}
