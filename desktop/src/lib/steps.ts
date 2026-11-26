// What a step's details read from its call (components/Step.tsx draws them): a path from the project's folder, a command
// without its cd into that folder, the one file a command prints or searches (so its language), the line numbers a read or
// a search gives (into a gutter), and whether an edit's result only says it worked.
import { langOf } from "./markdown";
import { editPiece, wordMarks } from "./unidiff";

/** A line of a file's text in a step: its number where the result gives it, the file it is from (a search over several). */
export type Row = { n: number | null; code: string; path?: string; plain?: boolean };

/** A text's lines, CRLF or LF, without the newline that ends the last one. */
export const linesOf = (text: string) => text.replace(/\r?\n$/, "").split(/\r?\n/);
const trim = (p?: string | null) => (p ?? "").replace(/[/\\]+$/, "");
/** The home folder, as far as the project's folder tells it. */
const homeOf = (cwd?: string | null) => trim(cwd).match(/^(\/Users\/[^/]+|\/home\/[^/]+|[A-Za-z]:\\Users\\[^\\]+)/)?.[1] ?? null;
const inside = (p: string, dir: string) => p.startsWith(`${dir}/`) || p.startsWith(`${dir}\\`);

/** A path as a step shows it: from the project's folder inside it, else with ~ for the home folder; its folder ("" for none,
 * else ending in its separator) and its name. A plain chat's folder is the home folder: its paths keep the ~. */
export function shortPath(path: string, cwd?: string | null): { dir: string; name: string } {
  const root = trim(cwd), home = homeOf(cwd);
  let p = path;
  if (root && root !== home && inside(p, root)) p = p.slice(root.length + 1);
  else if (home && (p === home || inside(p, home))) p = `~${p.slice(home.length)}`;
  const cut = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\")) + 1;
  return { dir: p.slice(0, cut), name: p.slice(cut) };
}

/** A path from the project's folder as git names it ("src/a.ts", a relative path as it is); null outside the folder. */
export function projectPath(path: string, cwd?: string | null): string | null {
  const root = trim(cwd).replace(/\\/g, "/"), p = path.replace(/\\/g, "/");
  if (!/^([/~]|[A-Za-z]:)/.test(p)) return p.replace(/^\.\//, "");
  return root && p.startsWith(`${root}/`) ? p.slice(root.length + 1) : null;
}

const CD = /^\s*cd\s+(?:"([^"]*)"|'([^']*)'|([^\s;&|]+))\s*&&\s*/;
/** A command without a leading `cd <dir> &&` into the project's folder or a folder in it; `dir`: that folder from the
 * project's ("" for the folder itself), null where the command has no such cd (one elsewhere stays in it). */
export function stripCd(command: string, cwd?: string | null): { cmd: string; dir: string | null } {
  const m = command.match(CD);
  if (!m) return { cmd: command, dir: null };
  const to = trim(m[1] ?? m[2] ?? m[3]), root = trim(cwd);
  const dir = root && to === root ? "" : root && inside(to, root) ? to.slice(root.length + 1)
    // a relative folder is in the project; not one that leaves it or names another root, a variable, home
    : !/^([/\\~$]|[A-Za-z]:|\.\.)/.test(to) ? to.replace(/^\.(\/|$)/, "") : null;
  return dir == null ? { cmd: command, dir: null } : { cmd: command.slice(m[0].length), dir };
}

/** A command's words, quotes taken off (enough for the few commands read here). */
const words = (s: string) => (s.match(/"(?:\\.|[^"\\])*"|'[^']*'|\S+/g) ?? []).map((w) => w.replace(/^(["'])(.*)\1$/, "$2"));
/** The command's own pipeline, without a cd at its start (any: the output is what is read) and its stderr's redirection;
 * null where it runs more than one command or writes to a file. */
function pipeline(command: string): string[][] | null {
  const cmd = command.replace(CD, "").replace(/\s+2>(&1|\s*\/dev\/null)/g, "");
  if (/[;&<>`]|\$\(/.test(cmd)) return null;
  return cmd.split(/\s+\|\s+/).map(words);
}

const PRINT = /^(cat|head|tail|nl|bat|batcat|sed)$/, SED_P = /^\d+(,(\d+|\$))?p$/;
/** The one file a command prints, whole or in part (cat, head, tail, nl, `sed -n …p`, bat, and a pipe on into more of
 * them), and whether its lines come numbered (nl, cat -n); null for anything else. */
export function printedFile(command: string): { file: string; numbered: boolean } | null {
  const segs = pipeline(command);
  if (!segs) return null;
  let file: string | null = null, numbered = false;
  for (const [k, [tool, ...rest]] of segs.entries()) {
    if (!tool || !PRINT.test(tool)) return null;
    if (tool === "sed" && !(rest.includes("-n") && rest.some((w) => SED_P.test(w)))) return null;
    numbered ||= tool === "nl" || (tool === "cat" && rest.some((w) => /^-\w*n/.test(w)));
    // what is left once the options and their numbers are out: the files (`head -n 40 x`, `tail -n +5 x`, `bat -r 1:9 x`)
    const files = rest.filter((w) => !w.startsWith("-") && !/^\+?\d+(:\d*)?$/.test(w) && !SED_P.test(w));
    // a glob is several files (`cat src/*.ts`), whatever it matches
    if (k === 0 ? files.length !== 1 || /[*?[]/.test(files[0]) : files.length) return null;
    if (k === 0) file = files[0];
  }
  return file ? { file, numbered } : null;
}

const VALUE = /^-(e|f|A|B|C|m|g|t|T|-glob|-type|-max-count|-context|-after-context|-before-context|-regexp|-file|-include|-exclude)$/;
/** A grep or rg that numbers its lines (-n): the one file it searches, or null for a folder, several files or none (the
 * lines then say their file). Null for any other command. */
export function searchOf(command: string): { file: string | null } | null {
  const segs = pipeline(command);
  if (!segs || !/^(grep|egrep|rg)$/.test(segs[0][0] ?? "") || segs.slice(1).some((s) => !/^(head|tail)$/.test(s[0] ?? ""))) return null;
  const [, ...rest] = segs[0];
  let pattern = false, numbered = false, deep = false;
  const free: string[] = [];
  for (let i = 0; i < rest.length; i++) {
    const w = rest[i];
    if (w === "--") { free.push(...rest.slice(i + 1)); break; }
    if (!w.startsWith("-") || w === "-") { free.push(w); continue; }
    if (w === "--line-number" || /^-[a-zA-Z]*n/.test(w)) numbered = true;
    if (/^-[a-zA-Z]*[rR]/.test(w) || w === "--recursive") deep = true;
    if (/^-[a-zA-Z]*e$/.test(w) || w === "--regexp") pattern = true;
    if (VALUE.test(w) || /^-[a-zA-Z]*[efABCmgtT]$/.test(w)) i++;
  }
  if (!numbered) return null;
  const paths = pattern ? free : free.slice(1);
  return { file: paths.length === 1 && !deep && !/[*?]/.test(paths[0]) && /\.\w+$/.test(paths[0]) ? paths[0] : null };
}

const NUMBERED = /^\s*(\d+)(?:→|\t)(.*)$/;
/** A file's lines as claude's Read, `cat -n` and nl number them ("    12→code", "    12\tcode"), the number apart; null
 * where the first line has none. A line without one (a note after them) stays, unnumbered and plain. */
export function numberedRows(text: string): Row[] | null {
  const lines = linesOf(text);
  if (!NUMBERED.test(lines[0] ?? "")) return null;
  return lines.map((l) => { const m = l.match(NUMBERED); return m ? { n: Number(m[1]), code: m[2] } : { n: null, code: l, plain: true }; });
}

/** Where the own loop's read starts and how many lines it asks for, as the core reads its offset and limit (src/mcp.rs
 * read_range: a number, a number in a string, or a range "100,140", "100-140", "100:140"); null where the core refuses it. */
export function readRange(offset: unknown, limit: unknown): { from: number; count: number | null } | null {
  const num = (v: unknown) => (typeof v === "number" && Number.isInteger(v) && v >= 0 ? v : typeof v === "string" && /^\s*\+?\d+\s*$/.test(v) ? Number(v) : null);
  const count = limit == null ? null : num(limit);
  if (limit != null && count == null) return null;
  if (offset == null) return { from: 1, count };
  const o = num(offset);
  if (o != null) return { from: Math.max(1, o), count };
  const r = typeof offset === "string" ? offset.match(/^\s*(\d+)\s*[,:-]\s*(\d+)\s*$/) : null;
  return r && Number(r[2]) >= Number(r[1]) ? { from: Math.max(1, Number(r[1])), count: count ?? Number(r[2]) - Number(r[1]) + 1 } : null;
}

// the own loop's notes around a result: before it ("[read #2]", "[the same call 3 times in a row: …]"), after it ("[lines
// 1-40 of 90; …]", "[truncated 120 chars]"), or in its place ("[unchanged since read #2 above: …]")
const LEAD = /^\[(read #\d+|the same call \d+ times in a row\b.*|unchanged since read #\d+\b.*)\]$/, TAIL = /^\[(lines \d+-\d+ of \d+\b.*|truncated \d+ chars)\]$/;
/** The own loop's read: the file's text alone (never "12→" numbers: a TSV's first column is its own), numbered from line
 * `from` (null: not known, no numbers), its notes before and after it unnumbered. */
export function fromLine(text: string, from: number | null): Row[] {
  const lines = linesOf(text), note = (code: string): Row => ({ n: null, code, plain: true });
  let i = 0, end = lines.length;
  while (i < end && LEAD.test(lines[i])) i++;
  while (end > i && TAIL.test(lines[end - 1])) end--;
  let n = from ?? 0;
  return [...lines.slice(0, i).map(note), ...lines.slice(i, end).map((code) => ({ n: from == null ? null : n++, code })), ...lines.slice(end).map(note)];
}

/** grep -n and rg -n output: "12:code" (one file), "path:12:code" (several), a context line with "-" for ":", "--" between
 * groups; each line's number apart and its file where it says one. A file's name is a word with an extension or a slash
 * ("001-init.sql:5:…" is that file's line 5, not line 1); `one`: a search over one file, whose lines name none. */
export function grepRows(text: string, one = false): Row[] {
  return linesOf(text).map((l) => {
    const many = one ? null : l.match(/^([^\s:]*(?:\.\w+|\/)[^\s:]*?)([:-])(\d+)\2(.*)$/);
    if (many) return { n: Number(many[3]), code: many[4], path: many[1] };
    const single = l.match(/^(\d+)[:-](.*)$/);
    if (single) return { n: Number(single[1]), code: single[2] };
    return { n: null, code: l, plain: true };
  });
}

const bare = (name: string) => name.replace(/^mcp__\w+?__/, "");
/** A result that is a file's text, as rows to draw with a gutter and its language: a read, a command that prints one file,
 * a numbered search (claude's Grep with -n too). Null: the result is shown as it came. */
export function resultCode(name: string, input: Record<string, unknown> | null | undefined, text: string): { rows: Row[]; lang: string | null } | null {
  const s = (k: string) => (typeof input?.[k] === "string" ? (input[k] as string) : null), n = bare(name);
  if (/^(Read|read)$/.test(n)) {
    // claude's numbers its lines; the own loop's comes bare, from its offset as the core read it
    const rows = n === "read" ? fromLine(text, readRange(input?.offset, input?.limit)?.from ?? null) : numberedRows(text);
    return rows && { rows, lang: langOf(s("file_path") ?? s("path")) };
  }
  if (/^(Bash|bash)$/.test(n) && s("command")) {
    const printed = printedFile(s("command")!), lang = printed ? langOf(printed.file) : null;
    if (printed && printed.numbered) return { rows: numberedRows(text) ?? fromLine(text, null), lang };
    if (printed && lang) return { rows: fromLine(text, null), lang };
    const search = searchOf(s("command")!);
    if (search) return { rows: grepRows(text, !!search.file), lang: langOf(search.file) };
  }
  if (n === "Grep" && input?.["-n"] === true && s("output_mode") === "content") return { rows: grepRows(text), lang: langOf(s("path")) };
  return null;
}

/** Whether an edit's or a write's result only says it worked, all of it (claude's "The file … has been updated
 * successfully.", "File created successfully at: …", its MultiEdit's "Applied 2 edits to …:" with each "N. Replaced …",
 * its older Edit's numbered snippet, the own loop's "ok"): hidden under the diff. Anything more (a hook's words, replace_all's
 * "All occurrences …") is shown. */
export const saysDone = (text?: string) => !!text && [
  /^The file [^\n]+ has been updated successfully\.$/,
  /^File created successfully at: [^\n]+$/,
  /^ok$/,
  /^Applied \d+ edits? to [^\n]+:(\r?\n\d+\. Replaced "[^\n]*" with "[^\n]*")*$/,
  /^The file [^\n]+ has been updated\. Here's the result of running `cat -n` on a snippet of the edited file:(\r?\n[ \t]*\d+(→|\t)[^\n]*)*$/,
].some((r) => r.test(text.trim()));

/** Where an edit's new text begins in the file, where its result says: claude's older Edit added "…a snippet of the edited
 * file:" with the lines numbered as its Read numbers them. Null where nothing tells (then no numbers are drawn). */
export function editStart(result: string | undefined, after: string): number | null {
  const at = result?.indexOf("snippet of the edited file:") ?? -1;
  const rows = at < 0 ? null : numberedRows(result!.slice(at).replace(/^[^\n]*\n/, ""));
  const lines = linesOf(after), k = lines.findIndex((l) => l.trim());
  if (!rows || k < 0) return null;
  // the first place its lines (three at most) follow one another
  const want = lines.slice(k, k + 3);
  const i = rows.findIndex((_, i) => want.every((l, j) => rows[i + j]?.code === l && rows[i + j].n != null));
  return i < 0 ? null : rows[i].n! - k;
}

type Call = { name?: string; input?: Record<string, unknown> | null };
type EditPiece = { old_string?: string; new_string?: string; old?: string; new?: string; replace_all?: boolean };
/** An edit's pieces as the tool was given them (claude's names and the own loop's); [] for anything else. */
const piecesOf = (c: Call): EditPiece[] => {
  const n = bare(c.name ?? ""), i = c.input ?? {};
  return /^(Edit|edit)$/.test(n) ? [i as EditPiece] : /^(MultiEdit|multi_edit)$/.test(n) && Array.isArray(i.edits) ? i.edits as EditPiece[] : [];
};
/** A text's lines that say something (a word in them: "}" and blank lines are in every edit), trimmed. */
const worded = (t = "") => linesOf(t).map((l) => l.trim()).filter((l) => /\w/.test(l));

/** Which of an edit's pieces (an edit: one) may not be where the turn's diff has them: replace_all (every place the text
 * was), or rewritten later in the turn: a later piece of the same multi-edit or a later edit of the same file whose old
 * text shares a line with its new text (one line holding the other: an edit inside a line too), any later write of the
 * file. The diff then no longer has its lines, and an identical change elsewhere would be found in their place (review of
 * N1): those get no numbers. `later`: the steps after it in its turn that may have changed files. */
export function rewritten(step: Call, later: Call[], cwd?: string | null): boolean[] {
  const fileOf = (c: Call) => { const p = c.input?.file_path ?? c.input?.path; return typeof p === "string" ? projectPath(p, cwd) ?? p : null; };
  const file = fileOf(step), mine = piecesOf(step), same = later.filter((c) => fileOf(c) === file);
  const written = same.some((c) => /^(Write|write)$/.test(bare(c.name ?? "")));
  const after = (k: number) => [...mine.slice(k + 1), ...same.flatMap(piecesOf)];
  return mine.map((p, k) => {
    const lines = worded(p.new_string ?? p.new);
    return p.replace_all === true || written || after(k).some((q) => worded(q.old_string ?? q.old).some((y) => lines.some((x) => x.includes(y) || y.includes(x))));
  });
}

/** A check: `npx tsx -e 'import("./src/lib/steps.ts").then(m => m.demo())'`. */
export function demo() {
  const eq = (got: unknown, want: unknown, what: string) => { if (JSON.stringify(got) !== JSON.stringify(want)) throw new Error(`steps: ${what}: ${JSON.stringify(got)}`); };
  const P = "/Users/x/api-gateway";
  // the language from a path, and from a command
  eq(["a/b.tsx", "x.PY", "ci.yml", "Dockerfile", "notes.md", "LICENSE", "C:\\w\\q.rs"].map(langOf), ["ts", "py", "yaml", "sh", "md", null, "rs"], "langOf");
  eq(printedFile("sed -n 1,40p summary.unit.ts"), { file: "summary.unit.ts", numbered: false }, "sed -n");
  eq(printedFile(`cd "${P}/src" && cat -n routes/a.ts | head -50 2>/dev/null`), { file: "routes/a.ts", numbered: true }, "cat -n into head");
  eq([printedFile("head -n 40 a.py")?.file, printedFile("tail -n +5 a.go")?.file, printedFile("nl -ba x.rs")?.numbered], ["a.py", "a.go", true], "head, tail, nl");
  eq([printedFile("ls; sed -n 1,40p a.ts"), printedFile("cat a.ts b.ts"), printedFile("sed 's/a/b/' a.ts"), printedFile("cat a.ts > b.ts"), printedFile("npm test")], [null, null, null, null, null], "not one file printed");
  eq([searchOf("grep -n latestFetch src/a.ts"), searchOf("rg -n -C 2 'x y' src/"), searchOf("grep -rn foo src/a.ts"), searchOf("grep foo a.ts"), searchOf("grep -n -e x a.ts | head -5")],
    [{ file: "src/a.ts" }, { file: null }, { file: null }, null, { file: "a.ts" }], "searchOf");
  // the cd into the project or a folder in it; elsewhere it stays
  eq(stripCd(`cd "${P}" && npm test`, P), { cmd: "npm test", dir: "" }, "cd to the project");
  eq(stripCd(`cd '${P}/src/routes/orders' && ls`, `${P}/`), { cmd: "ls", dir: "src/routes/orders" }, "cd into a folder in it");
  eq(stripCd("cd src && make", P), { cmd: "make", dir: "src" }, "a relative cd");
  eq([stripCd("cd /tmp && ls", P).dir, stripCd("cd .. && ls", P).dir, stripCd("cd $HOME && ls", P).dir, stripCd("ls", P).dir], [null, null, null, null], "a cd elsewhere stays");
  // paths from the project, ~ outside it, the home folder's chat with ~
  eq(shortPath(`${P}/src/routes/a.ts`, P), { dir: "src/routes/", name: "a.ts" }, "in the project");
  eq(shortPath("/Users/x/other/b.py", P), { dir: "~/other/", name: "b.py" }, "outside it");
  eq([projectPath(`${P}/src/a.ts`, P), projectPath("./src/a.ts", P), projectPath("/Users/x/other/b.py", P), projectPath("C:\\w\\app\\src\\a.rs", "C:\\w\\app\\")], ["src/a.ts", "src/a.ts", null, "src/a.rs"], "projectPath");
  eq([shortPath("/Users/x/notes/c.md", "/Users/x").dir, shortPath("/etc/hosts", P).dir, shortPath("src/a.ts", P).dir, shortPath(`${P}/a.ts`, P).dir], ["~/notes/", "/etc/", "src/", ""], "home, root, relative");
  // a read's and a search's numbers into the gutter
  eq(numberedRows("   988→  // clearing holds.\n   989→}\n\n<system-reminder>x</system-reminder>"), [{ n: 988, code: "  // clearing holds." }, { n: 989, code: "}" }, { n: null, code: "", plain: true }, { n: null, code: "<system-reminder>x</system-reminder>", plain: true }], "Read's numbers");
  eq(numberedRows("     7\tlet a = 1;")?.[0], { n: 7, code: "let a = 1;" }, "cat -n's");
  eq(fromLine("[read #2]\nfn a() {}\n}\n[lines 40-41 of 90; read with offset=42 for more]", 40).map((r) => r.n), [null, 40, 41, null], "the own loop's read");
  // review: the own loop's offsets as the core reads them, its notes unnumbered, a TSV's first column its own, CRLF
  eq([readRange("100,140", undefined), readRange(" 7 ", 5), readRange("100-90", undefined), readRange(undefined, "x"), readRange(0, undefined)],
    [{ from: 100, count: 41 }, { from: 7, count: 5 }, null, null, { from: 1, count: null }], "readRange");
  eq(resultCode("read", { path: "a.ts", offset: "100,140" }, "[the same call 3 times in a row: use what it returned, or do something different]\n[read #4]\nlet a;\nlet b;")?.rows.map((r) => r.n),
    [null, null, 100, 101], "the own loop's notes and string offset");
  eq(resultCode("read", { path: "fruit.tsv" }, "1\tapple\t3\n2\tpear\t5")?.rows[0], { n: 1, code: "1\tapple\t3" }, "a TSV read by the own loop keeps its first column");
  eq(resultCode("read", { path: "Cargo.toml" }, "[package]\nname = \"x\"")?.rows.map((r) => r.n), [1, 2], "a TOML table is line 1, not a note");
  eq([numberedRows("     1→a\r\n     2→b\r\n")?.map((r) => r.code), grepRows("3:x\r\n4:y").map((r) => r.code), editPiece("a\r\nb\r\n", "a\r\nc\r\n").lines],
    [["a", "b"], ["x", "y"], [" a", "-b", "+c"]], "CRLF");
  eq([printedFile("cat src/*.ts"), printedFile("head -n 5 src/?.ts"), printedFile("cat a[12].ts")], [null, null, null], "a glob is no one file");
  eq(grepRows("001-init.sql:5:CREATE TABLE a;\nsrc/a-b.ts-12-  x\n141:  const a = b.c:3"), [{ n: 5, code: "CREATE TABLE a;", path: "001-init.sql" }, { n: 12, code: "  x", path: "src/a-b.ts" }, { n: 141, code: "  const a = b.c:3" }], "a file named with digits");
  eq(grepRows("5-init.sql:x", true)[0], { n: 5, code: "init.sql:x" }, "one file's lines name none");
  eq(grepRows("1141:    const latestFetch = a:b;\n1142-  next\n--\nsrc/x-y.ts:12:let z;"), [{ n: 1141, code: "    const latestFetch = a:b;" }, { n: 1142, code: "  next" }, { n: null, code: "--", plain: true }, { n: 12, code: "let z;", path: "src/x-y.ts" }], "grepRows");
  eq(resultCode("Bash", { command: "sed -n 1,2p a.ts" }, "import x;\nlet y;\n"), { rows: [{ n: null, code: "import x;" }, { n: null, code: "let y;" }], lang: "ts" }, "a printed file");
  eq([resultCode("Bash", { command: "npm test" }, "ok"), resultCode("Read", { file_path: "a.ts" }, "plain words")], [null, null], "nothing to read as a file");
  // an edit's rows: shared lines as context, the changed words marked, numbers only where the result tells
  const p = editPiece("import { plan } from './summary';\nconst a = 1;\n", "import { plan, sortLines } from './summary';\nconst a = 1;\nconst b = 2;\n");
  eq(p.lines, ["-import { plan } from './summary';", "+import { plan, sortLines } from './summary';", " const a = 1;", "+const b = 2;"], "editPiece");
  eq([...wordMarks(p)], [[0, [13, 13]], [1, [13, 24]]], "the changed words");
  eq(editPiece("", "a\nb").lines, ["+a", "+b"], "an edit into an empty text");
  eq(editStart("The file a.ts has been updated. Here's the result of running `cat -n` on a snippet of the edited file:\n    40→}\n    41→const b = 2;\n    42→x", "}\nconst b = 2;"), 40, "editStart");
  eq([editStart("The file a.ts has been updated successfully.", "x"), saysDone("The file /a/b.ts has been updated successfully."), saysDone("File created successfully at: /a"), saysDone("ok"), saysDone("ok, but the hook failed")],
    [null, true, true, true, false], "saysDone");
  // anything said beyond "it worked" is shown: a hook's words, replace_all's sentence
  eq([saysDone("The file a.ts has been updated successfully.\n\nPostToolUse hook: eslint found 3 problems"), saysDone("The file a.ts has been updated. All occurrences of 'x' were successfully replaced with 'y'."),
    saysDone("Applied 2 edits to /a.ts:\n1. Replaced \"a\" with \"b\"\n2. Replaced \"c\" with \"d\""), saysDone("Applied 1 edit to /a.ts:\n1. Replaced \"a\" with \"b\"\nwarning: hook"),
    saysDone("The file a.ts has been updated. Here's the result of running `cat -n` on a snippet of the edited file:\n    40→}\n    41→x"), saysDone("File created successfully at: /a\nbut the formatter failed")],
    [false, false, true, false, true, false], "saysDone with more words");
  // review of N1: an edit rewritten later in its turn, or replace_all, has no numbers (an identical change elsewhere would
  // be found in its place); an edit elsewhere, or of another file, leaves it numbered
  const ed = (old_string: string, new_string: string, file = `${P}/a.ts`, more = {}) => ({ name: "Edit", input: { file_path: file, old_string, new_string, ...more } });
  eq(rewritten(ed("foo();", "bar();"), [ed("baz();", "qux();")], P), [false], "a later edit elsewhere");
  eq(rewritten(ed("foo();", "bar();"), [ed("  bar();\n  next();", "  baz();\n  next();")], P), [true], "a later edit of its new line");
  eq(rewritten(ed("x = foo(1);", "x = foo(2);"), [ed("foo(2)", "foo(3)")], P), [true], "a later edit inside its line");
  eq(rewritten(ed("foo();", "bar();"), [ed("bar();", "baz();", `${P}/b.ts`)], P), [false], "a later edit of another file");
  eq(rewritten(ed("foo();", "bar();"), [{ name: "Write", input: { file_path: `${P}/a.ts`, content: "" } }], P), [true], "a later write of the file");
  eq(rewritten(ed("foo();", "bar();"), [{ name: "edit", input: { path: "a.ts", old: "bar();", new: "x" } }], P), [true], "the own loop's relative path is the same file");
  eq(rewritten(ed("foo", "bar", `${P}/a.ts`, { replace_all: true }), [], P), [true], "replace_all");
  eq(rewritten({ name: "MultiEdit", input: { file_path: `${P}/a.ts`, edits: [{ old_string: "a1", new_string: "b1" }, { old_string: "b1", new_string: "c1" }, { old_string: "z9", new_string: "y9" }] } }, [], P),
    [true, false, false], "a later piece of the same multi-edit");
  eq(rewritten(ed("}", "  x();\n}"), [ed("}\n", "}\n\n")], P), [false], "a line with no word in it is in every edit");
  console.log("steps ok");
}
