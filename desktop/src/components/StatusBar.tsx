import { ArrowUpCircle, Check, LoaderCircle, RefreshCw } from "lucide-react";
import { openUsage } from "./Rail";
import { usd, type Usage, keysOf } from "../lib/types";
import { useEffect, useRef, useState } from "react";
import { ChatSlip, priceSaid, type Turn } from "./Receipt";
import { exactly, untilShort, whenReset, worst, type Item, type Level } from "../lib/health";
import { Sep } from "./Sep";
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

/** The account's reading in the bar: "Claude ▬ 34% 3h 34m | ▬ 93% 1d 2h", "SubRouter $12.40 left", "OmniRoute ▬ 12% Gemini 4d 2h". */
function Here({ here }: { here: Item[] }) {
  const b = "font-medium text-ink-2";
  if (!here.length) return <>Usage</>;
  // A window is its meter, how full it is and how long until it resets, nothing else: "36% 2h 8m", the 5 hours first,
  // then the week, as MonoCode's bar has it (the user, 2026-11-20: "way too much text"). Until desktop 0.1.91 each also
  // carried its name and a ↻ before the time ("36% 5 h ↻ 2 h 8 min 79% week ↻ Sat 3am"). Which window, when exactly and
  // the words are on its hover, and in the card.
  const about = (i: Item) => `${i.label}: ${i.value} used. ${i.over ? "This window is over: a new one starts with your next message." : i.resetsAt != null ? `Resets ${whenReset(i.resetsAt)} (${exactly(i.resetsAt)}).` : ""}`;
  const until = (i: Item) => (i.over ? <span className="text-faint">new</span> : i.resetsAt != null && <span className="text-faint">{untilShort(i.resetsAt)}</span>);
  // which window each is, in two letters ("5h", "wk"): two unnamed meters read as one number twice (design review 9)
  const tag = (l: string) => (l === "5 hours" ? "5h" : l.replace(/^week,? ?/i, "wk ").trim());
  if (here[0].provider === "Claude") return <>Claude{here.map((i, n) => <span key={i.key} title={about(i)} className="flex items-center gap-1.5">{n > 0 && <Sep />}<span className="text-faint">{tag(i.label)}</span><Meter pct={i.pct ?? 0} /><span className={b}>{i.value}</span>{until(i)}</span>)}</>;
  const i = here[0];
  if (i.pct != null) return <span title={about(i)} className="flex items-center gap-1.5">{i.name.split(",")[0]}<Meter pct={i.pct} /><span className={b}>{i.value}</span>{i.name.split(", ")[1]}{until(i)}</span>;
  // a balance its API does not tell is no news: the provider's name alone ("Mortera balance not reported" on every screen)
  return <>{i.name} {i.value && <><span className={b}>{i.value.replace(/ (left|below zero)$/, "")}</span><span className="text-faint">{i.value.match(/ (left|below zero)$/)?.[1]}</span></>}</>;
}

/** Every account, the conversation's own marked, the ones that need you in their colour: in groups under their account
 * written out whole, as the usage page has them, every line wrapping (a flat list of 13 rows cut "OmniRoute
 * OpenAI-compatibl…" and "this conversation, resets Sat 2:59…", design review 7). */
function Card({ all, here }: { all: Item[]; here: Item[] }) {
  const groups = [...new Set(all.map((i) => i.group))];
  return (
    <span className="invisible absolute bottom-full left-2 z-30 pb-2 opacity-0 transition-opacity duration-150 group-hover/use:visible group-hover/use:opacity-100 group-focus-within/use:visible group-focus-within/use:opacity-100">
      <span id="usage-card" role="tooltip" className="block max-h-[70vh] w-[400px] overflow-y-auto whitespace-normal rounded-xl border border-line-strong bg-card pb-1.5 text-[12.5px] text-ink-2 tz-float">
        {groups.map((g) => (
          <span key={g} className="block">
            <span className="block bg-field/40 px-3.5 py-1 text-[11.5px] text-muted [overflow-wrap:anywhere]">{g}</span>
            {all.filter((i) => i.group === g).map((i) => (
              <span key={i.key} className={`grid grid-cols-[1fr_70px_112px] items-center gap-2.5 px-3.5 py-[5px] ${here.includes(i) ? "bg-accent-soft" : ""}`}>
                <span className="min-w-0"><span className="block text-ink">{i.label}</span>
                  {(here.includes(i) || i.note) && <span className="block text-[11.5px] text-faint" title={exactly(i.resetsAt)}>{[here.includes(i) && "this conversation", i.note].filter(Boolean).join(", ")}</span>}</span>
                <span>{i.pct != null && <span className="block h-1 w-[70px] overflow-hidden rounded-sm bg-line-strong"><i className={`block h-full ${i.level === "out" ? "bg-bad" : i.level === "low" ? "bg-warn" : "bg-accent"}`} style={{ width: `${Math.min(100, i.pct)}%` }} /></span>}</span>
                <span className={`text-right tabular-nums ${i.level === "ok" ? "text-ink" : ink[i.level]}`}>{i.value}</span>
              </span>
            ))}
          </span>
        ))}
        {all.length === 0 && <span className="block px-3.5 py-1.5 text-faint">Reading your accounts…</span>}
        <span className="mt-1.5 flex items-center justify-between border-t border-line-soft px-3.5 pt-2 text-ink">Usage and spending<kbd className="rounded-[5px] border border-line px-[5px] font-mono text-[11px] text-faint">{keysOf("⌘⇧U")}</kbd></span>
      </span>
    </span>
  );
}

/** "this chat $0.051": a click opens the conversation's receipt over the bar (its turns, the sums, the way on to the usage
 * page; until design review 11 the click went straight to the usage page). Esc or a click elsewhere shuts it, and the
 * keyboard's focus is back on the button. */
function ChatCost({ cost, turns, busy, className, strong }: { cost: number; turns: Turn[]; busy?: boolean; className: string; strong: string }) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLSpanElement>(null), button = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    const key = (e: KeyboardEvent) => { if (e.key === "Escape") { e.stopPropagation(); setOpen(false); button.current?.focus(); } };
    const away = (e: MouseEvent) => { if (!box.current?.contains(e.target as Node)) setOpen(false); };
    window.addEventListener("keydown", key, true); window.addEventListener("mousedown", away);
    return () => { window.removeEventListener("keydown", key, true); window.removeEventListener("mousedown", away); };
  }, [open]);
  return (
    <span ref={box} onBlur={(e) => { if (e.relatedTarget && !e.currentTarget.contains(e.relatedTarget)) setOpen(false); }} className="relative flex flex-none">
      <button ref={button} onClick={() => setOpen(!open)} aria-expanded={open} aria-haspopup="dialog" title="This conversation's receipt: its turns and what they cost"
        className={`${className} ${open ? "bg-hover text-ink" : ""}`}>this chat <span className={strong}>{priceSaid(cost, turns.some((t) => t.estimate))}</span></button>
      {open && (
        <div role="dialog" aria-label="This conversation's receipt" className="tz-fade absolute bottom-full right-0 z-30 mb-2 w-[452px] max-w-[calc(100vw-1.5rem)] whitespace-normal">
          <ChatSlip turns={turns} cost={cost} busy={busy}>
            <button autoFocus onClick={() => { setOpen(false); openUsage(); }} className="-mx-1.5 mt-2 flex w-[calc(100%+0.75rem)] items-center justify-between rounded-md px-1.5 py-0.5 text-ink hover:bg-hover">
              Usage and spending<kbd className="rounded-[5px] border border-line px-[5px] font-mono text-[11px] text-faint">{keysOf("⌘⇧U")}</kbd></button>
          </ChatSlip>
        </div>
      )}
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

/** "just now", "4 min ago", "2 h ago". */
const ago = (at: number) => { const m = Math.floor((Date.now() - at) / 60_000); return m < 1 ? "just now" : m < 60 ? `${m} min ago` : `${Math.floor(m / 60)} h ago`; };

export function StatusBar({ usage: u, version, idle, accounts = [], here = [], onRefresh, refreshing, readAt, turns = [], busy }: { usage: Usage; version: string;
  /** the conversation's finished turns and how its model is paid, for its receipt */
  turns?: Turn[]; busy?: boolean;
  /** reads every account again (a few seconds, no model call), while `refreshing`; `readAt`: when the reading on screen came */
  onRefresh?: () => void; refreshing?: boolean; readAt?: number;
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
    <footer aria-label="Status" className="relative z-20 flex h-7 flex-none items-center gap-4 overflow-x-clip overflow-y-visible whitespace-nowrap bg-desk px-3.5 text-[12px] tabular-nums text-muted">
      <span className="group/use relative flex flex-none items-center gap-3">
        <button onClick={openUsage} aria-describedby="usage-card" className={button}>
          <span className={`size-1.5 rounded-full ${tone[here.length && here.every((i) => i.level === "unknown") ? "unknown" : worst(here)]}`} aria-hidden="true" /><Here here={here} />
        </button>
        <Card all={accounts} here={here} />
      </span>
      {/* the reading again, now: it came at the start and every 15 minutes, and a plan moves faster than that while you
          work (the user, 2026-11-20: "it might change along the way", as MonoCode's bar has it). Two arrows, not the one
          that marks a reset. */}
      {onRefresh && (
        <button onClick={() => !refreshing && onRefresh()} aria-disabled={refreshing} aria-label="Refresh usage"
          title={refreshing ? "Reading your accounts…" : `Refresh usage${readAt ? ` (read ${ago(readAt)})` : ""}`}
          className="-ml-2.5 grid size-5 flex-none place-items-center rounded text-faint hover:bg-hover hover:text-ink">
          <RefreshCw size={11} strokeWidth={2} className={refreshing ? "animate-spin" : ""} aria-hidden="true" />
        </button>
      )}
      {/* the badge says what it means and what to do, on its own hover (it opened the account card too, the user). Not a
          button: the account beside it opens the usage page. Several accounts: the most urgent by name, the rest counted
          ("OpenRouter $0.17 below zero and 2 more"), each in the hover; one badge, not one each, and not "3 accounts need you" */}
      {others.length > 0 && (
        <span className="group/adv relative flex flex-none">
          {/* quiet: an account this conversation does not run on is a dot in its colour and faint words, not a red chip on
              every screen (design review 10: the badge in red shouted while the conversation ran on Claude) */}
          <span tabIndex={0} aria-describedby="usage-advice" className="flex flex-none cursor-default items-center gap-1.5 rounded px-1 -mx-1 text-faint hover:text-ink-2">
            <span className={`size-1.5 flex-none rounded-full ${tone[worst(others)]}`} aria-hidden="true" />{others[0].short}
            {/* the rest counted in words ("+4" did not say what it counted) */}
            {others.length > 1 && <span className="tabular-nums">and {others.length - 1} more</span>}
          </span>
          <span className="invisible absolute bottom-full left-0 z-30 pb-2 opacity-0 transition-opacity duration-150 group-hover/adv:visible group-hover/adv:opacity-100 group-focus-within/adv:visible group-focus-within/adv:opacity-100">
            <span id="usage-advice" role="tooltip" className="block w-[340px] whitespace-normal rounded-xl border border-line-strong bg-card px-3.5 py-2.5 text-[12.5px] leading-relaxed text-ink-2 tz-float">
              {others.length > 1 && <span className="mb-1.5 block text-[11.5px] font-medium text-faint">{others.length} accounts need you</span>}
              {others.map((o) => <span key={o.key} className="mb-1.5 flex items-start gap-2 last:mb-0">
                <span className={`mt-[7px] size-1.5 flex-none rounded-full ${tone[o.level]}`} aria-hidden="true" />{o.advice ?? o.short}</span>)}
            </span>
          </span>
        </span>
      )}
      <span className="flex-1" />
      {/* a zero says nothing yet (a conversation before its first reply, a provider that reports no cache): not shown */}
      {!idle && u.contextPct > 0 && <span className={`${item} max-[900px]:hidden`} title="This conversation's context">context <span className={b}>{u.contextPct}%</span> of {u.contextMax}</span>}
      {!idle && u.cachedPct > 0 && <span className={`${item} max-[1100px]:hidden`} title="Input read from the cache in this conversation">cache <span className={b}>{u.cachedPct}%</span></span>}
      {u.threadCost > 0 && (turns.length ? <ChatCost cost={u.threadCost} turns={turns} busy={busy} className={button} strong={b} />
        : <button onClick={openUsage} title="This conversation's cost · Usage" className={button}>this chat <span className={b}>{usd(u.threadCost)}</span></button>)}
      {version && <Version version={version} />}
    </footer>
  );
}
