import { ArrowUpCircle, Check, LoaderCircle, RotateCw, TriangleAlert } from "lucide-react";
import { openUsage } from "./Rail";
import { usd, type Usage } from "../lib/types";
import { useEffect, useState } from "react";
import { exactly, whenReset, worst, type Item, type Level } from "../lib/health";
import { checkUpdate, useUpdate } from "../lib/update";

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
  // each window with its own reset after a small ↻ ("5 h ↻ 3 h 34 min", "week ↻ Sat 3am"); the words and the exact time
  // on hover. Until desktop 0.1.83 one reset for the bar, so the 5 hours' was missing beside a full week (the user)
  const reset = (i: Item) => i.resetsAt != null && (
    <span className="flex items-center gap-1 text-faint" title={`Resets ${whenReset(i.resetsAt)}: ${exactly(i.resetsAt)}`}>
      <RotateCw size={10} strokeWidth={2} aria-hidden="true" /><span className="sr-only">resets</span>{whenReset(i.resetsAt)!.replace(/^in /, "")}</span>);
  if (here[0].provider === "Claude") return <>Claude plan{here.map((i) => <span key={i.key} className="flex items-center gap-1.5"><Meter pct={i.pct ?? 0} /><span className={b}>{i.value}</span>{i.name.replace(/^Claude plan, /, "").replace("5 hours", "5 h")}{reset(i)}</span>)}</>;
  const i = here[0];
  if (i.pct != null) return <>{i.name.split(",")[0]}<Meter pct={i.pct} /><span className={b}>{i.value}</span>{i.name.split(", ")[1]}{reset(i)}</>;
  return <>{i.name} {i.value ? <span className={b}>{i.value.replace(/ (left|over)$/, "")}</span> : <span className="text-faint">balance not reported</span>}{i.value && <span className="text-faint">{i.value.match(/ (left|over)$/)?.[1]}</span>}</>;
}

/** Every account, the conversation's own marked, the ones that need you in their colour: in groups under their account
 * written out whole, as the usage page has them, every line wrapping (a flat list of 13 rows cut "OmniRoute
 * OpenAI-compatibl…" and "this conversation, resets Sat 2:59…", design review 7). */
function Card({ all, here }: { all: Item[]; here: Item[] }) {
  const groups = [...new Set(all.map((i) => i.group))];
  return (
    <span className="invisible absolute bottom-full left-2 z-30 pb-2 opacity-0 transition-opacity duration-150 group-hover/use:visible group-hover/use:opacity-100 group-focus-within/use:visible group-focus-within/use:opacity-100">
      <span id="usage-card" role="tooltip" className="block max-h-[70vh] w-[400px] overflow-y-auto whitespace-normal rounded-xl border border-line-strong bg-card pb-1.5 text-[12.5px] text-ink-2 shadow-composer">
        {groups.map((g) => (
          <span key={g} className="block">
            <span className="block bg-field/40 px-3.5 py-1 text-[11.5px] text-muted [overflow-wrap:anywhere]">{g}</span>
            {all.filter((i) => i.group === g).map((i) => (
              <span key={i.key} className={`grid grid-cols-[1fr_70px_96px] items-center gap-2.5 px-3.5 py-[5px] ${here.includes(i) ? "bg-accent-soft" : ""}`}>
                <span className="min-w-0"><span className="block text-ink">{i.label}</span>
                  {(here.includes(i) || i.note) && <span className="block text-[11.5px] text-faint" title={exactly(i.resetsAt)}>{[here.includes(i) && "this conversation", i.note].filter(Boolean).join(", ")}</span>}</span>
                <span>{i.pct != null && <span className="block h-1 w-[70px] overflow-hidden rounded-sm bg-line-strong"><i className={`block h-full ${i.level === "out" ? "bg-bad" : i.level === "low" ? "bg-warn" : "bg-accent"}`} style={{ width: `${Math.min(100, i.pct)}%` }} /></span>}</span>
                <span className={`text-right tabular-nums ${i.level === "ok" ? "text-ink" : ink[i.level]}`}>{i.value ?? <span className="text-faint">not reported</span>}</span>
              </span>
            ))}
          </span>
        ))}
        {all.length === 0 && <span className="block px-3.5 py-1.5 text-faint">Reading your accounts…</span>}
        <span className="mt-1.5 flex items-center justify-between border-t border-line-soft px-3.5 pt-2 text-ink">Usage and spending<kbd className="rounded-[5px] border border-line px-[5px] font-mono text-[11px] text-faint">⌘⇧U</kbd></span>
      </span>
    </span>
  );
}

/** The app's version, and what a click on it does: look for an update and answer in place ("up to date"), or offer the
 * one found (its dialog, as Settings' button); an update found by the hourly check shows here by itself, "update to
 * 0.1.86" in the accent, as VS Code and MonoCode show theirs. Until desktop 0.1.86 a line of text that did nothing (the
 * user: "should be clickable"). */
function Version({ version }: { version: string }) {
  const up = useUpdate();
  const [said, setSaid] = useState<"checking" | "none" | "error" | null>(null);
  const check = async () => {
    if (said === "checking" || up.installing) return;
    setSaid("checking");
    const r = await checkUpdate("inline");
    setSaid(r === "none" ? "none" : null);
    if (r === "none") setTimeout(() => setSaid((s) => (s === "none" ? null : s)), 4000);
  };
  const newer = up.version && up.version !== version;
  const base = "flex flex-none items-center gap-1.5 rounded px-1 -mx-1 hover:bg-hover";
  if (up.installing) return <span className="flex flex-none items-center gap-1.5 text-faint"><LoaderCircle size={11} strokeWidth={2} className="animate-spin" aria-hidden="true" />updating</span>;
  if (newer) return <button onClick={() => checkUpdate(true)} title={`tazuna desktop ${up.version} is out: see what changed and install`} className={`${base} font-medium text-accent-ink`}>
    <ArrowUpCircle size={12} strokeWidth={1.75} aria-hidden="true" />update to {up.version}</button>;
  return (
    <button onClick={check} title="Check for updates" aria-live="polite" className={`${base} text-faint hover:text-ink max-[1100px]:hidden`}>
      {said === "checking" ? <><LoaderCircle size={11} strokeWidth={2} className="animate-spin" aria-hidden="true" />checking</>
        : said === "none" ? <><Check size={12} strokeWidth={2} className="text-ok" aria-hidden="true" />up to date</>
        : <>tazuna {version}</>}
    </button>
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
  // the other accounts that need you, the stopped ones first: the badge names the first and counts the rest
  const others = accounts.filter((i) => !here.includes(i) && (i.level === "low" || i.level === "out")).sort((a, b) => Number(b.level === "out") - Number(a.level === "out"));
  // the countdown moves on by itself
  const [, tick] = useState(0);
  useEffect(() => { const t = setInterval(() => tick((n) => n + 1), 30_000); return () => clearInterval(t); }, []);
  return (
    <footer aria-label="Status" className="relative z-20 flex h-7 flex-none items-center gap-4 overflow-x-clip overflow-y-visible whitespace-nowrap border-t border-line bg-sidebar px-3.5 text-[12px] tabular-nums text-muted">
      <span className="group/use relative flex flex-none items-center gap-3">
        <button onClick={openUsage} aria-describedby="usage-card" className={button}>
          <span className={`size-1.5 rounded-full ${tone[here.length && here.every((i) => i.level === "unknown") ? "unknown" : worst(here)]}`} aria-hidden="true" /><Here here={here} />
        </button>
        <Card all={accounts} here={here} />
      </span>
      {/* the badge says what it means and what to do, on its own hover (it opened the account card too, the user). Not a
          button: the account beside it opens the usage page. Several accounts: the most urgent by name, the rest counted
          ("OpenRouter $0.17 over +2"), each in the hover; one badge, not one each, and not "3 accounts need you" */}
      {others.length > 0 && (
        <span className="group/adv relative flex flex-none">
          <span tabIndex={0} aria-describedby="usage-advice" className={`flex flex-none cursor-default items-center gap-1.5 rounded-full border px-2 leading-[18px] ${worst(others) === "out" ? "border-bad/45 text-bad" : "border-warn/45 text-warn"}`}>
            <TriangleAlert size={12} strokeWidth={1.75} aria-hidden="true" />{others[0].short}
            {others.length > 1 && <span className="rounded-full bg-current/15 px-1.5 text-[11px] font-medium leading-4">+{others.length - 1}</span>}
          </span>
          <span className="invisible absolute bottom-full left-0 z-30 pb-2 opacity-0 transition-opacity duration-150 group-hover/adv:visible group-hover/adv:opacity-100 group-focus-within/adv:visible group-focus-within/adv:opacity-100">
            <span id="usage-advice" role="tooltip" className="block w-[340px] whitespace-normal rounded-xl border border-line-strong bg-card px-3.5 py-2.5 text-[12.5px] leading-relaxed text-ink-2 shadow-composer">
              {others.length > 1 && <span className="mb-1.5 block text-[11.5px] font-medium text-faint">{others.length} accounts need you</span>}
              {others.map((o) => <span key={o.key} className="mb-1.5 flex items-start gap-2 last:mb-0">
                <span className={`mt-[7px] size-1.5 flex-none rounded-full ${tone[o.level]}`} aria-hidden="true" />{o.advice ?? o.short}</span>)}
            </span>
          </span>
        </span>
      )}
      <span className="flex-1" />
      {!idle && <span className={`${item} max-[900px]:hidden`} title="This conversation's context">context <span className={b}>{u.contextPct}%</span> of {u.contextMax}</span>}
      {!idle && <span className={`${item} max-[1100px]:hidden`} title="Input read from the cache in this conversation">cache <span className={b}>{u.cachedPct}%</span></span>}
      {u.threadCost > 0 && <button onClick={openUsage} title="This conversation's cost · Usage" className={button}>this chat <span className={b}>{usd(u.threadCost)}</span></button>}
      {version && <Version version={version} />}
    </footer>
  );
}
