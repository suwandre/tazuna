import { openUsage } from "./Rail";
import { usd, type Usage } from "../lib/types";

// The status bar along the window's bottom (prototype mockups/v4.html, variant B, chosen 2026-11-19): the plan's 5 hours
// and week with what is left of them, SubRouter's balance, this conversation's context, cache and cost, the app's version.
// Always in sight, as MonoCode and Hermes keep theirs (the comparison with the harnesses on the Mac); until desktop 0.1.71
// the plan sat in a small bar by the send button with its card on hover, and the cost in the header. A click on the plan
// or the cost opens the usage page.

function Meter({ pct }: { pct: number }) {
  return <span className="inline-block h-1 w-[30px] overflow-hidden rounded-sm bg-line-strong" aria-hidden="true">
    <i className={`block h-full ${pct >= 75 ? "bg-warn" : "bg-accent"}`} style={{ width: `${Math.min(100, Math.max(0, pct))}%` }} /></span>;
}

export function StatusBar({ usage: u, version }: { usage: Usage; version: string }) {
  const item = "flex flex-none items-center gap-1.5";
  const button = `${item} rounded px-1 -mx-1 hover:bg-hover hover:text-ink`;
  const b = "font-medium text-ink-2";
  return (
    <footer aria-label="Status" className="flex h-7 flex-none items-center gap-4 overflow-hidden whitespace-nowrap border-t border-line bg-sidebar px-3.5 text-[12px] tabular-nums text-muted">
      {u.plan5h != null && (
        <button onClick={openUsage} title="Claude plan · Usage (⌘⇧U)" className={button}>
          <span className={`size-1.5 rounded-full ${u.plan5h >= 90 ? "bg-warn" : "bg-ok"}`} aria-hidden="true" />Claude plan
          <Meter pct={u.plan5h} /><span className={b}>{u.plan5h}%</span>5 h{u.plan5hReset && ` · ${u.plan5hReset.replace(/^in /, "")}`}
          {u.planWeek != null && <><Meter pct={u.planWeek} /><span className={b}>{u.planWeek}%</span>week</>}
        </button>
      )}
      {u.balance != null && <button onClick={openUsage} title="SubRouter's balance · Usage" className={button}>SubRouter <span className={b}>${u.balance.toFixed(2)}</span></button>}
      <span className="flex-1" />
      <span className={`${item} max-[900px]:hidden`} title="This conversation's context">context <span className={b}>{u.contextPct}%</span> of {u.contextMax}</span>
      <span className={`${item} max-[1100px]:hidden`} title="Input read from the cache in this conversation">cache <span className={b}>{u.cachedPct}%</span></span>
      {u.threadCost > 0 && <button onClick={openUsage} title="This conversation's cost · Usage" className={button}>this chat <span className={b}>{usd(u.threadCost)}</span></button>}
      {version && <span className={`${item} text-faint max-[1100px]:hidden`}>tazuna {version}</span>}
    </footer>
  );
}
