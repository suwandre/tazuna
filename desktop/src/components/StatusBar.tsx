import { TriangleAlert } from "lucide-react";
import { openUsage } from "./Rail";
import { usd, type Usage } from "../lib/types";
import { worst, type Item, type Level } from "../lib/health";

// The status bar along the window's bottom: on the left the account this conversation runs on (the plan's 5 hours and
// week on Claude, the credit on SubRouter, the window on OmniRoute) and, when another account is running low or stopped,
// one badge that names it (or counts them); every account in the card on hover. On the right this conversation's
// context, cache and cost, and the app's version. Variant A of mockups/v5.html (chosen 2026-11-19): until desktop 0.1.79
// it showed Claude's plan whatever the conversation ran on (v4.html, variant B).

function Meter({ pct }: { pct: number }) {
  return <span className="inline-block h-1 w-[30px] overflow-hidden rounded-sm bg-line-strong" aria-hidden="true">
    <i className={`block h-full ${pct >= 90 ? "bg-bad" : pct >= 75 ? "bg-warn" : "bg-accent"}`} style={{ width: `${Math.min(100, Math.max(0, pct))}%` }} /></span>;
}

const tone: Record<Level, string> = { ok: "bg-ok", low: "bg-warn", out: "bg-bad", unknown: "bg-faint" };
const ink: Record<Level, string> = { ok: "text-ink", low: "text-warn", out: "text-bad", unknown: "text-faint" };

/** The account's reading in the bar: "Claude plan ▬ 34% 5 h ▬ 93% week", "SubRouter $12.40 left", "OmniRoute ▬ 12% Gemini". */
function Here({ here }: { here: Item[] }) {
  const b = "font-medium text-ink-2";
  if (!here.length) return <>Usage</>;
  if (here[0].provider === "Claude") return <>Claude plan{here.map((i) => <span key={i.key} className="flex items-center gap-1.5"><Meter pct={i.pct ?? 0} /><span className={b}>{i.value}</span>{i.name.replace(/^Claude plan, /, "").replace("5 hours", "5 h")}</span>)}</>;
  const i = here[0];
  if (i.pct != null) return <>{i.name.split(",")[0]}<Meter pct={i.pct} /><span className={b}>{i.value}</span>{i.name.split(", ")[1]}</>;
  return <>{i.name} {i.value ? <span className={b}>{i.value.replace(/ (left|over)$/, "")}</span> : <span className="text-faint">balance not reported</span>}{i.value && <span className="text-faint">{i.value.match(/ (left|over)$/)?.[1]}</span>}</>;
}

/** Every account, the conversation's own marked, the ones that need you in their colour. */
function Card({ all, here }: { all: Item[]; here: Item[] }) {
  return (
    <span className="invisible absolute bottom-full left-2 z-30 pb-2 opacity-0 transition-opacity duration-150 group-hover/use:visible group-hover/use:opacity-100 group-focus-within/use:visible group-focus-within/use:opacity-100">
      <span id="usage-card" role="tooltip" className="block w-[400px] rounded-xl border border-line-strong bg-card py-1.5 text-[12.5px] text-ink-2 shadow-composer">
        <span className="block px-3.5 pb-1 pt-1.5 text-[11.5px] font-medium text-faint">Your accounts</span>
        {all.map((i) => (
          <span key={i.key} className={`grid grid-cols-[1fr_70px_96px] items-center gap-2.5 px-3.5 py-[5px] ${here.includes(i) ? "bg-accent-soft" : ""}`}>
            <span className="min-w-0"><span className="block truncate text-ink">{i.name}</span>
              <span className="block truncate text-[11.5px] text-faint">{here.includes(i) ? "this conversation" : i.note}</span></span>
            <span>{i.pct != null && <span className="block h-1 w-[70px] overflow-hidden rounded-sm bg-line-strong"><i className={`block h-full ${i.level === "out" ? "bg-bad" : i.level === "low" ? "bg-warn" : "bg-accent"}`} style={{ width: `${Math.min(100, i.pct)}%` }} /></span>}</span>
            <span className={`text-right tabular-nums ${i.level === "ok" ? "text-ink" : ink[i.level]}`}>{i.value ?? <span className="text-faint">not reported</span>}</span>
          </span>
        ))}
        {all.length === 0 && <span className="block px-3.5 py-1.5 text-faint">Reading your accounts…</span>}
        <span className="mt-1.5 flex items-center justify-between border-t border-line-soft px-3.5 pt-2 text-ink">Usage and spending<kbd className="rounded-[5px] border border-line px-[5px] font-mono text-[11px] text-faint">⌘⇧U</kbd></span>
      </span>
    </span>
  );
}

export function StatusBar({ usage: u, version, idle, accounts = [], here = [] }: { usage: Usage; version: string;
  /** no conversation on screen (a new one, before its first message): its context and cache are not shown as 0% */
  idle?: boolean;
  /** every account (lib/health.ts) and the conversation's own among them */
  accounts?: Item[]; here?: Item[] }) {
  const item = "flex flex-none items-center gap-1.5";
  const button = `${item} rounded px-1 -mx-1 hover:bg-hover hover:text-ink`;
  const b = "font-medium text-ink-2";
  const others = accounts.filter((i) => !here.includes(i) && (i.level === "low" || i.level === "out"));
  return (
    <footer aria-label="Status" className="relative z-20 flex h-7 flex-none items-center gap-4 overflow-x-clip overflow-y-visible whitespace-nowrap border-t border-line bg-sidebar px-3.5 text-[12px] tabular-nums text-muted">
      <span className="group/use relative flex flex-none items-center gap-3">
        <button onClick={openUsage} aria-describedby="usage-card" className={button}>
          <span className={`size-1.5 rounded-full ${tone[here.length && here.every((i) => i.level === "unknown") ? "unknown" : worst(here)]}`} aria-hidden="true" /><Here here={here} />
        </button>
        {others.length > 0 && (
          <button onClick={openUsage} aria-describedby="usage-card" className={`flex flex-none items-center gap-1.5 rounded-full border px-2 leading-[18px] ${worst(others) === "out" ? "border-bad/45 text-bad" : "border-warn/45 text-warn"} hover:bg-hover`}>
            <TriangleAlert size={12} strokeWidth={1.75} />{others.length === 1 ? others[0].short : `${others.length} accounts need you`}
          </button>
        )}
        <Card all={accounts} here={here} />
      </span>
      <span className="flex-1" />
      {!idle && <span className={`${item} max-[900px]:hidden`} title="This conversation's context">context <span className={b}>{u.contextPct}%</span> of {u.contextMax}</span>}
      {!idle && <span className={`${item} max-[1100px]:hidden`} title="Input read from the cache in this conversation">cache <span className={b}>{u.cachedPct}%</span></span>}
      {u.threadCost > 0 && <button onClick={openUsage} title="This conversation's cost · Usage" className={button}>this chat <span className={b}>{usd(u.threadCost)}</span></button>}
      {version && <span className={`${item} text-faint max-[1100px]:hidden`}>tazuna {version}</span>}
    </footer>
  );
}
