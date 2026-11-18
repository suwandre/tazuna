import { Brain, Check, ChevronRight, LoaderCircle } from "lucide-react";
import { useEffect, useState } from "react";
import type { ToolStep } from "../lib/types";

const box = "mt-1 max-h-72 overflow-auto whitespace-pre-wrap break-words rounded-md bg-field px-2.5 py-2 font-mono text-[11.5px] leading-[1.5] text-ink-2 select-text";

/** What a call was given, in its own shape: an edit as removed and added lines, a command as itself, else the fields. */
function Given({ name, input }: { name: string; input: Record<string, unknown> }) {
  const str = (k: string) => (typeof input[k] === "string" ? (input[k] as string) : null);
  const n = name.replace(/^mcp__\w+?__/, "");
  // claude's Edit names them old_string and new_string, the own loop's edit old and new
  const before = str("old_string") ?? str("old"), after = str("new_string") ?? str("new");
  if (/^(Edit|edit)$/.test(n) && before != null) {
    return (
      <div className={box}>
        {before.split("\n").map((l, i) => <div key={`o${i}`} className="text-bad">- {l}</div>)}
        {(after ?? "").split("\n").map((l, i) => <div key={`n${i}`} className="text-ok">+ {l}</div>)}
      </div>
    );
  }
  if (/^(Write|write)$/.test(n) && str("content") != null) return <div className={box}>{str("content")}</div>;
  if (str("command") != null) return <div className={box}>$ {str("command")}</div>;
  return <div className={box}>{JSON.stringify(input, null, 2)}</div>;
}

/** Seconds since a running step began, ticking between the core's views (it sends one only when something changes, so a
 * long command's time stood still): its last count plus the time since that count came. */
export function Elapsed({ secs }: { secs: number }) {
  const [since, setSince] = useState(() => ({ secs, at: Date.now() }));
  const [, tick] = useState(0);
  useEffect(() => setSince({ secs, at: Date.now() }), [secs]);
  useEffect(() => { const t = setInterval(() => tick((n) => n + 1), 500); return () => clearInterval(t); }, []);
  return <>{(since.secs + (Date.now() - since.at) / 1000).toFixed(1)} s</>;
}

// A step before the answer, one line; a click opens what it was given and what came back (or the reasoning, where the
// model shows it). Steps with nothing more to show do not open.
export function Step({ t }: { t: ToolStep }) {
  const [open, setOpen] = useState(false);
  const thinking = t.kind === "thinking";
  const more = thinking ? !!t.text?.trim() : !!(t.input && Object.keys(t.input).length) || !!t.result;
  return (
    <div className="text-[12.5px] text-muted">
      <button disabled={!more} onClick={() => setOpen(!open)} className="flex w-full items-center gap-2 py-0.5 text-left disabled:cursor-default">
        {/* still going: the icon moves and the words shimmer (the CLI's spinner, here) */}
        {thinking ? <Brain size={14} strokeWidth={1.5} className={`flex-none ${t.running ? "animate-pulse text-accent" : "text-faint"}`} />
          : t.running ? <LoaderCircle size={14} strokeWidth={1.75} className="flex-none animate-spin text-accent" />
          : <Check size={14} strokeWidth={1.5} className={`flex-none ${t.ok ? "text-ok" : "text-bad"}`} />}
        <span className={t.running ? "tz-live" : ""}>{t.verb}</span> {t.target && <span className="truncate font-mono text-[11.5px] text-ink-2">{t.target}</span>}
        {t.detail && <span className="text-faint">· {t.detail}</span>}
        {more && <ChevronRight size={13} strokeWidth={1.5} className={`text-faint transition-transform duration-150 ${open ? "rotate-90" : ""}`} />}
        {!thinking && <span className="ml-auto flex-none whitespace-nowrap pl-2 font-mono text-[11px] text-faint">{t.running ? <Elapsed secs={t.secs} /> : `${t.secs.toFixed(1)} s`}</span>}
      </button>
      {open && (thinking ? <div className={`${box} font-serif`}>{t.text}</div> : <>
        {t.input && Object.keys(t.input).length > 0 && <Given name={t.name ?? ""} input={t.input} />}
        {t.result && <div className={`${box} ${t.ok ? "" : "text-bad"}`}>{t.result}</div>}
      </>)}
    </div>
  );
}
