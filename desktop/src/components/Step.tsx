import { Brain, Check, ChevronRight, Circle } from "lucide-react";
import { useState } from "react";
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

// A step before the answer, one line; a click opens what it was given and what came back (or the reasoning, where the
// model shows it). Steps with nothing more to show do not open.
export function Step({ t }: { t: ToolStep }) {
  const [open, setOpen] = useState(false);
  const thinking = t.kind === "thinking";
  const more = thinking ? !!t.text?.trim() : !!(t.input && Object.keys(t.input).length) || !!t.result;
  return (
    <div className="text-[12.5px] text-muted">
      <button disabled={!more} onClick={() => setOpen(!open)} className="flex w-full items-center gap-2 py-0.5 text-left disabled:cursor-default">
        {thinking ? <Brain size={14} strokeWidth={1.5} className="text-faint" />
          : t.running ? <Circle size={10} strokeWidth={2} className="mx-0.5 text-accent" />
          : <Check size={14} strokeWidth={1.5} className={t.ok ? "text-ok" : "text-bad"} />}
        {t.verb} {t.target && <span className="truncate font-mono text-[11.5px] text-ink-2">{t.target}</span>}
        {t.detail && <span className="text-faint">· {t.detail}</span>}
        {more && <ChevronRight size={13} strokeWidth={1.5} className={`text-faint transition-transform duration-150 ${open ? "rotate-90" : ""}`} />}
        {!t.running && !thinking && <span className="ml-auto font-mono text-[11px] text-faint">{t.secs.toFixed(1)} s</span>}
      </button>
      {open && (thinking ? <div className={`${box} font-serif`}>{t.text}</div> : <>
        {t.input && Object.keys(t.input).length > 0 && <Given name={t.name ?? ""} input={t.input} />}
        {t.result && <div className={`${box} ${t.ok ? "" : "text-bad"}`}>{t.result}</div>}
      </>)}
    </div>
  );
}
