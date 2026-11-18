import { Brain, Check, ChevronRight, LoaderCircle, X } from "lucide-react";
import { Markdown } from "../lib/markdown";
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
  // a read in words: which file, which lines
  if (/^(Read|read)$/.test(n) && (str("file_path") ?? str("path")) != null) {
    const from = Number(input.offset ?? 1) || 1, count = Number(input.limit ?? 0);
    return <div className={line}>{str("file_path") ?? str("path")}{count ? ` · lines ${from}–${from + count - 1}` : ""}</div>;
  }
  return <Fields value={input} />;
}

const line = "mt-1 px-1 font-mono text-[11.5px] text-ink-2 select-text";

/** `inline code` in a value drawn as code, not with its backticks */
const code = (t: string) => t.split(/(`[^`]+`)/).map((p, i) => (p.startsWith("`") && p.endsWith("`") && p.length > 2 ? <code key={i} className="rounded bg-hover px-1 font-mono text-[11.5px]">{p.slice(1, -1)}</code> : p));

/** "failure_scenario" → "Failure scenario" */
const label = (k: string) => k.replace(/[_-]+/g, " ").replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase().replace(/^./, (c) => c.toUpperCase());

/** Data as labelled lines (a list of findings as one card each), not JSON: what a call was given, what came back. */
function Fields({ value }: { value: unknown }) {
  if (Array.isArray(value)) return <>{value.map((v, i) => <Fields key={i} value={v} />)}</>;
  if (value && typeof value === "object") {
    return (
      <div className="mt-1 rounded-md bg-field px-2.5 py-2 text-[12px] leading-[1.5] text-ink-2 select-text">
        {Object.entries(value as Record<string, unknown>).map(([k, v]) => (
          <div key={k} className="py-0.5"><span className="text-faint">{label(k)}</span>{" "}
            {v && typeof v === "object" ? <Fields value={v} /> : <span className="whitespace-pre-wrap break-words">{code(String(v))}</span>}</div>
        ))}
      </div>
    );
  }
  return <div className={`${box} font-sans`}>{String(value)}</div>;
}

// what files and commands return is shown as it is; other tools' text (a skill's, a subagent's, a page's) is read as
// prose: its markdown drawn, its JSON as labelled lines (a skill's findings came as ```json in a code box, 2026-11-18)
const RAW = /^(Bash|bash|Read|read|Grep|Glob|search|list|Edit|edit|Write|write|NotebookEdit|BashOutput|PowerShell)$/;

function Result({ name, text, ok }: { name: string; text: string; ok: boolean }) {
  if (RAW.test(name.replace(/^mcp__\w+?__/, ""))) return <div className={`${box} ${ok ? "" : "text-bad"}`}>{text}</div>;
  try {
    const v = JSON.parse(text);
    if (v && typeof v === "object") return <Fields value={v} />;
  } catch { /* not JSON as a whole: prose with perhaps a block of it */ }
  // a fenced block of JSON becomes labelled lines; the rest is markdown, without claude's wrapper around a skill's answer
  const parts = text.replace(/^Skill "[^"]*" completed[^\n]*\n+(Result:\s*\n)?/, "").split(/```(?:json)?[ \t]*\n([\s\S]*?)```/);
  return (
    <div className={`mt-1 rounded-md bg-field px-2.5 py-2 text-[12.5px] ${ok ? "text-ink-2" : "text-bad"}`}>
      {parts.map((p, i) => {
        if (i % 2 === 0) return p.trim() ? <div key={i} className="prose-answer !font-sans !text-[12.5px] !leading-[1.55]"><Markdown text={p.trim()} /></div> : null;
        try { return <Fields key={i} value={JSON.parse(p)} />; } catch { return <pre key={i} className="font-mono text-[11.5px]">{p}</pre>; }
      })}
    </div>
  );
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

// a step still going says so: "Running npm test", then "Ran npm test"
const NOW: Record<string, string> = { Read: "Reading", Ran: "Running", Edited: "Editing", Wrote: "Writing", Searched: "Searching", Listed: "Listing", Fetched: "Fetching" };

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
          : t.ok ? <Check size={14} strokeWidth={1.5} className="flex-none text-ok" />
          : <X size={14} strokeWidth={1.75} className="flex-none text-bad" aria-label="failed" />}
        <span className={t.running ? "tz-live" : ""}>{t.running ? (NOW[t.verb] ?? t.verb) : t.verb}</span> {t.target && <span className="truncate font-mono text-[11.5px] text-ink-2">{t.target}</span>}
        {t.detail && <span className="text-faint">· {t.detail}</span>}
        {/* the time beside the step, not at the far right (the eye went 600 px for it) */}
        {!thinking && <span className="flex-none whitespace-nowrap text-[12px] tabular-nums text-faint">{t.running ? <Elapsed secs={t.secs} /> : `${t.secs.toFixed(1)} s`}</span>}
        {more && <ChevronRight size={13} strokeWidth={1.5} className={`flex-none text-faint transition-transform duration-150 ${open ? "rotate-90" : ""}`} />}
      </button>
      {open && (thinking ? <div className={`${box} font-serif`}>{t.text}</div> : <>
        {t.input && Object.keys(t.input).length > 0 && <Given name={t.name ?? ""} input={t.input} />}
        {t.result && <Result name={t.name ?? ""} text={t.result} ok={t.ok} />}
      </>)}
    </div>
  );
}
