import { Brain, Check, ChevronRight, Copy, LoaderCircle, Square, X } from "lucide-react";
import { langOf, Markdown } from "../lib/markdown";
import { Fragment, useEffect, useMemo, useState, type ReactNode } from "react";
import { tenths, type ToolStep } from "../lib/types";
import { editPiece, numbered, wordMarks, type Piece } from "../lib/unidiff";
import { editStart, linesOf, readRange, resultCode, saysDone, shortPath, stripCd, type Row } from "../lib/steps";
import { CodeRow, MAX_ROWS, More } from "./DiffView";
import { Sep } from "./Sep";

const box = "mt-1 max-h-72 overflow-auto whitespace-pre-wrap break-words rounded-md bg-field px-2.5 py-2 font-mono text-[11.5px] leading-[1.5] text-ink-2 select-text";
// code (a diff, a file's text) on the card under a hairline, as an answer's code block: its row tints keep every text level
// at 4.5:1 there, which on `field` they did not; each row draws its own gutter and wrapping (CodeRow)
const codeBox = "mt-1 max-h-72 overflow-auto rounded-md border border-line bg-card py-1 font-mono text-[11.5px] leading-[1.5] select-text";

const bare = (name: string) => name.replace(/^mcp__\w+?__/, "");

/** A path from the conversation's folder (~ outside it): the folder faint and cut from its left when long, the file's name in
 * ink and never cut; the whole path on hover. `wrap`: never cut, broken over lines (the approval's: what is asked about is
 * read whole, "~/" and all). */
export function PathLabel({ path, cwd, className = "", wrap }: { path: string; cwd?: string | null; className?: string; wrap?: boolean }) {
  const { dir, name } = shortPath(path, cwd);
  if (wrap) return <span title={path} className={`block break-all font-mono ${className}`}><span className="text-faint">{dir}</span><span className="text-ink">{name}</span></span>;
  return (
    <span title={path} className={`flex min-w-0 font-mono ${className}`}>
      {dir && <span className="tz-ltrunc min-w-0 text-faint"><bdi dir="ltr">{dir}</bdi></span>}
      <span className="flex-none text-ink">{name}</span>
    </span>
  );
}

/** What one box of code may still draw: rows (MAX_ROWS, then "… N more lines") and coloured characters (past 60k its lines
 * are plain: colouring is a span per token). */
type Budget = { rows: number; chars: number };
const budget = (): Budget => ({ rows: MAX_ROWS, chars: 60_000 });
const tint = (b: Budget, text: string, lang: string) => ((b.chars -= text.length) >= 0 ? lang : "text");

/** An edit's piece as diff rows: + and − rows tinted whole, the changed words marked, coloured by the file's language;
 * numbers only where they are known (`numbers`), never made up. A plain function, not a component: it spends the box's
 * budget once per drawing. */
function diffRows(piece: Piece, lang: string, numbers: boolean, b: Budget): ReactNode {
  const nums = numbered(piece), marks = wordMarks(piece), w = String(piece.newStart + piece.lines.length).length;
  const shown = nums.slice(0, Math.max(0, b.rows));
  b.rows -= shown.length;
  return <>{shown.map((l, i) => <CodeRow key={i} text={l.line.slice(1)} kind={l.line[0]} mark={marks.get(i)} language={tint(b, l.line, lang)} nums={numbers ? [l.new ?? l.old] : undefined} width={w} className="pl-1.5" />)}
    <More n={nums.length - shown.length} /></>;
}

/** A file's lines (a read, a printed file, a search): their numbers in the gutter where the result gave them, a search over
 * several files with each file's name over its lines. */
function rowsBox(rows: Row[], lang: string, cwd?: string | null): ReactNode {
  const b = budget(), shown = rows.slice(0, b.rows);
  const gutter = shown.some((r) => r.n != null), w = String(shown.reduce((m, r) => Math.max(m, r.n ?? 0), 0)).length;
  let file: string | undefined;
  return (
    <div className={codeBox}>
      {shown.map((r, i) => {
        const head = r.path !== undefined && r.path !== file ? <PathLabel path={(file = r.path)} cwd={cwd} className="px-2.5 pt-1 text-[11px]" /> : null;
        return <Fragment key={i}>{head}<CodeRow text={r.code} plain={r.plain} language={tint(b, r.code, r.path ? langOf(r.path) ?? "text" : lang)} nums={gutter ? [r.n] : undefined} width={w} className="pl-1.5" /></Fragment>;
      })}
      <More n={rows.length - shown.length} />
    </div>
  );
}

const boxNote = (t: string) => <div className="px-2.5 pb-0.5 font-sans text-[11px] text-faint select-none">{t}</div>;

/** Copies a command whole (its cd too), on hover or focus at the box's corner. */
function CopyCommand({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  const copy = () => navigator.clipboard?.writeText(text).then(() => { setDone(true); setTimeout(() => setDone(false), 1400); }, () => {});
  return (
    <button onClick={copy} title="Copy the command" aria-label="Copy the command"
      className="absolute right-1 top-1 rounded p-1 text-faint opacity-0 hover:text-ink focus-visible:opacity-100 group-hover:opacity-100">
      {done ? <Check size={12} strokeWidth={1.75} /> : <Copy size={12} strokeWidth={1.5} />}</button>
  );
}

/** Whether a tool writes files: its approval shows what would change (`Given`), not only the file's name. */
export const writes = (name: string) => /^(Edit|edit|Write|write|MultiEdit|multi_edit)$/.test(bare(name));

/** Whether a call's change is drawn (an edit's diff, a write's text): its result that only says it worked is then hidden. */
const drawsChange = (name: string, input?: Record<string, unknown> | null) => writes(name) && !!input
  && (typeof (input.old_string ?? input.old) === "string" || Array.isArray(input.edits) || typeof input.content === "string");

/** What a call was given, in its own shape: an edit as a diff (a multi-edit as each of its pieces), a write as its new file,
 * a command as itself (without its cd into the project), else the fields. `cap`: the box's height class (the approval's is
 * lower); `cwd`: the conversation's folder; `result`: the call's, where it tells an edit's line numbers. Drawn once per
 * content: each view of a reply brings new objects with the same text (a 6000-line write cost 170 ms a view). */
export function Given(props: { name: string; input: Record<string, unknown>;
  /** the box's height class where it is lower (the approval's): it takes the focus then, so the keyboard scrolls it */
  cap?: string; cwd?: string | null; result?: string }) {
  const { name, input, cap, cwd, result } = props;
  return useMemo(() => given(props), [name, cap, cwd, result, JSON.stringify(input)]);
}

function given({ name, input, cap, cwd, result }: Parameters<typeof Given>[0]): ReactNode {
  const str = (k: string) => (typeof input[k] === "string" ? (input[k] as string) : null);
  const n = bare(name), file = str("file_path") ?? str("path"), lang = langOf(file) ?? "text";
  const frame = cap ? codeBox.replace("max-h-72", cap) : codeBox;
  // claude's Edit names them old_string and new_string, the own loop's edit old and new
  const before = str("old_string") ?? str("old"), after = str("new_string") ?? str("new");
  const scroll = cap ? { tabIndex: 0, role: "region", "aria-label": "What would change" } : {};
  if (/^(Edit|edit)$/.test(n) && before != null) {
    const start = editStart(result, after ?? "");
    // replace_all changes every place the text is; the diff shows the text once
    return <div {...scroll} className={frame}>{input.replace_all === true && boxNote("all occurrences")}{diffRows(editPiece(before, after ?? "", start ?? 1), lang, start != null, budget())}</div>;
  }
  if (/^(MultiEdit|multi_edit)$/.test(n) && Array.isArray(input.edits)) {
    const edits = input.edits as { old_string?: string; new_string?: string; old?: string; new?: string; replace_all?: boolean }[], b = budget();
    return <div {...scroll} className={frame}>{edits.map((e, i) => <div key={i} className={i ? "mt-1 border-t border-line-soft pt-1" : ""}>
      {e.replace_all === true && boxNote("all occurrences")}{diffRows(editPiece(e.old_string ?? e.old ?? "", e.new_string ?? e.new ?? ""), lang, false, b)}</div>)}</div>;
  }
  if (/^(Write|write)$/.test(n) && str("content") != null) {
    const lines = linesOf(str("content")!), w = String(lines.length).length, b = budget(), shown = lines.slice(0, b.rows);
    // "new file" only where the result says it made one (claude's); before a result, or the own loop's "ok", it may write
    // over a file: "whole file"
    const made = /^File created successfully at: /.test(result ?? "");
    return (
      <div {...scroll} className={frame}>
        {boxNote(`${made ? "new file" : "whole file"} · ${lines.length} line${lines.length === 1 ? "" : "s"}`)}
        {shown.map((l, i) => <CodeRow key={i} text={l} language={tint(b, l, lang)} nums={[i + 1]} width={w} className="pl-1.5" />)}
        <More n={lines.length - shown.length} />
      </div>
    );
  }
  if (str("command") != null) {
    // without its cd into the project (a folder in it said before it); the whole command on hover and in Copy
    const whole = str("command")!, { cmd, dir } = stripCd(whole, cwd);
    return <div title={dir != null ? whole : undefined} className={`${box} group relative pr-8`}>{dir && <span className="text-faint select-none">in {dir} </span>}$ {cmd}<CopyCommand text={whole} /></div>;
  }
  // a read in words: which file, which lines (its offset as the core reads it: "100,140" is lines 100 to 140)
  if (/^(Read|read)$/.test(n) && file != null) {
    const r = readRange(input.offset, input.limit);
    return <div className={`${readLine} flex`}><PathLabel path={file} cwd={cwd} />{r?.count ? <span className="flex-none">, lines {r.from}–{r.from + r.count - 1}</span> : ""}</div>;
  }
  return <Fields value={input} />;
}

const readLine = "mt-1 px-1 font-mono text-[11.5px] text-ink-2 select-text";

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
const RAW = /^(Bash|bash|Read|read|Grep|Glob|search|list|Edit|edit|Write|write|MultiEdit|multi_edit|NotebookEdit|BashOutput|PowerShell)$/;

function Result(props: { name: string; text: string; ok: boolean; input?: Record<string, unknown> | null; cwd?: string | null }) {
  const { name, text, ok, input, cwd } = props;
  // drawn once per content, as Given
  return useMemo(() => result(props), [name, text, ok, cwd, JSON.stringify(input ?? null)]);
}

function result({ name, text, ok, input, cwd }: Parameters<typeof Result>[0]): ReactNode {
  if (RAW.test(bare(name))) {
    // a file's text (a read, a command that prints one file, a numbered search): as code, its numbers in a gutter
    const file = ok ? resultCode(name, input, text) : null;
    return file ? rowsBox(file.rows, file.lang ?? "text", cwd) : <div className={`${box} ${ok ? "" : "text-bad"}`}>{text}</div>;
  }
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
  return <>{tenths(since.secs + (Date.now() - since.at) / 1000)}</>;
}

// a step still going says so: "Running npm test", then "Ran npm test"
// a step that waits for your answer has not started: "Run npm install · waits for you"
const WAIT: Record<string, string> = { Read: "Read", Ran: "Run", Edited: "Edit", Wrote: "Write", Searched: "Search", Listed: "List", Fetched: "Fetch" };
const NOW: Record<string, string> = { Read: "Reading", Ran: "Running", Edited: "Editing", Wrote: "Writing", Searched: "Searching", Listed: "Listing", Fetched: "Fetching" };

// A step before the answer, one line; a click opens what it was given and what came back (or the reasoning, where the
// model shows it). Steps with nothing more to show do not open.
export function Step({ t }: { t: ToolStep }) {
  const [open, setOpen] = useState(false);
  // what the model said between its steps: a quiet line in its place, not the answer
  if (t.kind === "said") return <div className="prose-answer py-0.5 !font-sans !text-[12.5px] !leading-[1.55] text-muted [&_p]:!mb-1"><Markdown text={t.text ?? ""} /></div>;
  const thinking = t.kind === "thinking";
  const more = thinking ? !!t.text?.trim() : !!(t.input && Object.keys(t.input).length) || !!t.result;
  const path = (k: string) => (typeof t.input?.[k] === "string" ? (t.input[k] as string) : null);
  const file = !thinking && /^(Read|read|Edit|edit|Write|write|MultiEdit|multi_edit)$/.test(bare(t.name ?? "")) ? path("file_path") ?? path("path") : null;
  // a subagent: its own conversation opens beside this one (App listens for "tazuna-agent")
  const agent = !thinking && (t.name === "Agent" || t.name === "Task") && !!t.id && !t.stopped;
  return (
    <div className="text-[12.5px] text-muted">
      <div className="flex items-center gap-2">
      <button disabled={!more} onClick={() => setOpen(!open)} className="flex min-w-0 items-center gap-2 py-0.5 text-left disabled:cursor-default">
        {/* still going: the icon moves and the words shimmer (the CLI's spinner, here) */}
        {thinking ? <Brain size={14} strokeWidth={1.5} className={`flex-none ${t.running ? "animate-pulse text-live" : "text-faint"}`} />
          : t.running ? <LoaderCircle size={14} strokeWidth={1.75} className="flex-none animate-spin text-live" />
          // cut off when its reply was stopped: neither done nor failed
          : t.stopped || t.declined ? <Square size={10} strokeWidth={2} fill="currentColor" className="mx-0.5 flex-none text-faint" aria-label={t.declined ? "declined" : "stopped"} />
          : t.ok ? <Check size={14} strokeWidth={1.5} className="flex-none text-ok" />
          : <X size={14} strokeWidth={1.75} className="flex-none text-bad" aria-label="failed" />}
        <span className={t.running && !t.waiting ? "tz-live" : ""}>{t.waiting ? `${WAIT[t.verb] ?? t.verb} · waits for you` : t.running ? (NOW[t.verb] ?? t.verb) : t.declined ? (WAIT[t.verb] ?? t.verb) : t.verb}</span>
        {/* a file from the conversation's folder, its name in ink; anything else as the core named it */}
        {file ? <PathLabel path={file} cwd={t.cwd} className="text-[11.5px]" /> : t.target && <span className="truncate font-mono text-[11.5px] text-ink-2">{t.target}</span>}
        {/* (-ml-2 takes back the row's gap: the hairline has the same room on both sides) */}
        {t.detail && <span className="-ml-2 flex items-center text-faint"><Sep />{t.detail}</span>}
        {t.stopped && <span className="-ml-2 flex items-center text-faint"><Sep />stopped</span>}
        {t.declined && <span className="-ml-2 flex items-center text-faint"><Sep />declined</span>}
        {/* the time beside the step, not at the far right (the eye went 600 px for it) */}
        {!thinking && !t.waiting && <span className="flex-none whitespace-nowrap text-[12px] tabular-nums text-faint">{t.running ? <Elapsed secs={t.secs} /> : tenths(t.secs)}</span>}
        {more && <ChevronRight size={13} strokeWidth={1.5} className={`flex-none text-faint transition-transform duration-150 ${open ? "rotate-90" : ""}`} />}
      </button>
      {agent && <button onClick={() => window.dispatchEvent(new CustomEvent("tazuna-agent", { detail: t.id }))} title="See what this subagent did: its reasoning, steps and answers"
        className="flex-none rounded px-1.5 py-px text-[12px] text-accent-ink">{t.running ? "Watch" : "Open"}</button>}
      </div>
      {open && (thinking ? <div className={`${box} font-serif`}>{t.text}</div> : <>
        {t.input && Object.keys(t.input).length > 0 && <Given name={t.name ?? ""} input={t.input} cwd={t.cwd} result={t.result} />}
        {/* an edit's or a write's "updated successfully" says nothing its diff does not */}
        {t.result && !(t.ok && drawsChange(t.name ?? "", t.input) && saysDone(t.result)) && <Result name={t.name ?? ""} text={t.result} ok={t.ok} input={t.input} cwd={t.cwd} />}
      </>)}
    </div>
  );
}

/** A check: `npx tsx -e 'import("./src/components/Step.tsx").then(m => m.demo())'`. */
export async function demo() {
  const { renderToStaticMarkup: html } = await import("react-dom/server");
  const must = (ok: unknown, what: string) => { if (!ok) throw new Error(`step: ${what}`); };
  // "new file" only where the result says it made one; before it, or after the own loop's "ok", it may have written over one
  const write = (result?: string) => html(<Given name="Write" input={{ file_path: "/p/a.ts", content: "a\n\nb\n" }} result={result} />);
  must(write().includes("whole file · 3 lines") && write("ok").includes("whole file") && write("The file /p/a.ts has been updated successfully.").includes("whole file")
    && write("File created successfully at: /p/a.ts").includes("new file · 3 lines"), "new file or whole file");
  // a blank line is a <br> (a blank line in a copy, no space)
  must(/data-n="2"[^>]*><\/span><span class="[^"]*"[^>]*><br\/><\/span>/.test(write()), "a blank line is not a <br>");
  // long lines plain and unmarked, rows capped, all of it quick (a 200 KB line took 12 s in the approval)
  const minified = "var a=1,b=\"x\";".repeat(15_000), t0 = performance.now();
  const one = html(<Given name="Write" input={{ file_path: "/p/a.js", content: minified }} cap="max-h-40" />);
  const edit = html(<Given name="Edit" input={{ file_path: "/p/a.js", old_string: minified, new_string: `${minified}1` }} />);
  const many = html(<Given name="Write" input={{ file_path: "/p/a.ts", content: Array.from({ length: 6000 }, (_, i) => `const a${i} = "${"x".repeat(80)}"; // ${i}`).join("\n") }} />);
  const ms = performance.now() - t0;
  must(!one.includes("tok-") && !edit.includes("<mark") && !edit.includes("tok-"), "a long line is coloured or marked");
  must(many.includes("… 5000 more lines") && (many.match(/data-n="\d+"/g) ?? []).length === 1000, "a long file's rows are not capped");
  // past the box's 60k characters (about 600 of these lines) its lines are plain
  const tail = many.slice(many.lastIndexOf('data-n="999"'));
  must(many.includes("tok-keyword") && !tail.includes("tok-"), "colours past the budget");
  must(ms < 1500, `three huge inputs took ${Math.round(ms)} ms`);
  // replace_all said; the own loop's range offset in the read's line
  must(html(<Given name="Edit" input={{ file_path: "/p/a.ts", old_string: "a", new_string: "b", replace_all: true }} />).includes("all occurrences"), "replace_all");
  must(html(<Given name="read" input={{ path: "/p/a.ts", offset: "100,140" }} cwd="/p" />).includes("lines 100–140"), "a range offset");
  console.log(`step ok (${Math.round(ms)} ms)`);
}
