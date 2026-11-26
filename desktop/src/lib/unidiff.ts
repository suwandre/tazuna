// A unified diff (git's) split into files and their pieces (hunks), each piece a patch of its own: the review keeps or
// reverts them one by one.
export type Piece = { header: string; lines: string[]; oldStart: number; newStart: number };
export type FileDiff = { path: string; head: string[]; pieces: Piece[]; added: number; removed: number; binary: boolean };

export function parse(text: string): FileDiff[] {
  const files: FileDiff[] = [];
  let f: FileDiff | null = null, p: Piece | null = null;
  for (const line of text.split("\n")) {
    if (line.startsWith("diff --git ")) {
      f = { path: line.replace(/^diff --git a\/.* b\//, ""), head: [line], pieces: [], added: 0, removed: 0, binary: false };
      files.push(f); p = null;
    } else if (!f) continue;
    else if (line.startsWith("@@")) {
      const m = line.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)/);
      p = { header: line, lines: [], oldStart: m ? +m[1] : 1, newStart: m ? +m[2] : 1 };
      f.pieces.push(p);
    } else if (p) {
      if (line.startsWith("\\")) { p.lines.push(line); continue; } // "\ No newline at end of file"
      if (!/^[ +-]/.test(line)) continue; // the end of the diff ("… (cut)") or a stray blank
      p.lines.push(line);
      if (line[0] === "+") f.added++; else if (line[0] === "-") f.removed++;
    } else {
      f.head.push(line);
      // a name with a space ends in a tab there ("+++ b/my file.ts\t")
      if (line.startsWith("+++ b/")) f.path = line.slice(6).replace(/\t$/, "");
      else if (line.startsWith("--- a/") && f.path === "") f.path = line.slice(6).replace(/\t$/, "");
      if (line.startsWith("Binary files")) f.binary = true;
    }
  }
  return files;
}

/** One piece as a patch git applies: its file's header lines (up to ---/+++) and the piece. */
export function patchOf(f: FileDiff, p: Piece): string {
  const head = f.head.filter((l) => /^(diff --git|--- |\+\+\+ |new file mode|deleted file mode|old mode|new mode)/.test(l));
  return [...head, p.header, ...p.lines].join("\n") + "\n";
}

/** A whole file's pieces as one patch: the review's "Stage file" and "Revert file". */
export function patchOfFile(f: FileDiff): string {
  const head = f.head.filter((l) => /^(diff --git|--- |\+\+\+ |new file mode|deleted file mode|old mode|new mode)/.test(l));
  return [...head, ...f.pieces.flatMap((p) => [p.header, ...p.lines])].join("\n") + "\n";
}

/** Where a changed line differs from the line it replaced, by words: [start, end) in each, or null when nearly all of it
 * changed (then the line's own colour says it). The review marks those words (design review 9: a whole line green for
 * `.mockResolvedValue("ok")` added at its end). */
export function changed(before: string, after: string): { before: [number, number]; after: [number, number] } | null {
  // a line too long to colour is too long to mark (DiffView's LONG): its row says it changed
  if (before.length > 2000 || after.length > 2000) return null;
  const tok = (s: string) => s.match(/\w+|\s+|[^\w\s]/g) ?? [];
  const a = tok(before), b = tok(after);
  let pre = 0;
  while (pre < a.length && pre < b.length && a[pre] === b[pre]) pre++;
  let suf = 0;
  while (suf < a.length - pre && suf < b.length - pre && a[a.length - 1 - suf] === b[b.length - 1 - suf]) suf++;
  const len = (t: string[], from: number, to: number) => t.slice(from, to).join("").length;
  const range = (t: string[]): [number, number] => [len(t, 0, pre), len(t, 0, t.length - suf)];
  const r = { before: range(a), after: range(b) };
  // nothing in common, or most of the line: no marks
  const share = (r.after[1] - r.after[0] + r.before[1] - r.before[0]) / Math.max(1, before.length + after.length);
  return pre + suf === 0 || share > 0.7 ? null : r;
}

/** The changed words of a piece, by line index: each removed line with the first added line after it in the same run that
 * shares its start or end (in order), so a line moved down by new lines above it still finds its partner. */
export function wordMarks(p: Piece): Map<number, [number, number]> {
  const marks = new Map<number, [number, number]>();
  let i = 0;
  while (i < p.lines.length) {
    if (p.lines[i][0] !== "-") { i++; continue; }
    const dels: number[] = [], adds: number[] = [];
    while (i < p.lines.length && p.lines[i][0] === "-") dels.push(i++);
    while (i < p.lines.length && p.lines[i][0] === "+") adds.push(i++);
    let from = 0;
    for (const d of dels) {
      for (let k = from; k < adds.length; k++) {
        const w = changed(p.lines[d].slice(1), p.lines[adds[k]].slice(1));
        if (w) { marks.set(d, w.before); marks.set(adds[k], w.after); from = k + 1; break; }
      }
    }
  }
  return marks;
}

/** A piece's lines side by side: context on both sides, a run of removed lines beside the added ones after it. Each side
 * is an index into the piece's lines, or null where that side has none. */
export function sideBySide(p: Piece): { left: number | null; right: number | null }[] {
  const rows: { left: number | null; right: number | null }[] = [];
  let i = 0;
  while (i < p.lines.length) {
    const c = p.lines[i][0];
    if (c === "-" || c === "+") {
      const dels: number[] = [], adds: number[] = [];
      while (i < p.lines.length && p.lines[i][0] === "-") dels.push(i++);
      while (i < p.lines.length && p.lines[i][0] === "+") adds.push(i++);
      for (let k = 0; k < Math.max(dels.length, adds.length); k++) rows.push({ left: dels[k] ?? null, right: adds[k] ?? null });
    } else if (c === "\\") i++;
    else { rows.push({ left: i, right: i }); i++; }
  }
  return rows;
}

/** Each line of a piece with its line numbers (old, new; null where the line is not on that side). */
export function numbered(p: Piece): { line: string; old: number | null; new: number | null }[] {
  let o = p.oldStart, n = p.newStart;
  return p.lines.map((line) => {
    if (line[0] === "+") return { line, old: null, new: n++ };
    if (line[0] === "-") return { line, old: o++, new: null };
    if (line[0] === "\\") return { line, old: null, new: null };
    return { line, old: o++, new: n++ };
  });
}

/** An edit (the text it replaced and its replacement) as a piece, for a step's diff: the lines both share as context, the
 * rest removed then added, by the longest common run of lines. `start`: the line the text begins at in the file, where that
 * is known (the numbers are drawn only then). */
export function editPiece(before: string, after: string, start = 1): Piece {
  const split = (t: string) => (t === "" ? [] : t.replace(/\r?\n$/, "").split(/\r?\n/));
  const a = split(before), b = split(after), n = a.length, m = b.length, lines: string[] = [];
  const piece = { header: "", lines, oldStart: start, newStart: start };
  // ponytail: an n·m table; past 250k cells (two texts of 500 lines) all of it is removed and added, no context
  if (n * m > 250_000) { lines.push(...a.map((l) => `-${l}`), ...b.map((l) => `+${l}`)); return piece; }
  const L = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) L[i][j] = a[i] === b[j] ? L[i + 1][j + 1] + 1 : Math.max(L[i + 1][j], L[i][j + 1]);
  const dels: string[] = [], adds: string[] = [];
  const flush = () => { lines.push(...dels.splice(0).map((l) => `-${l}`), ...adds.splice(0).map((l) => `+${l}`)); };
  for (let i = 0, j = 0; i < n || j < m;) {
    if (i < n && j < m && a[i] === b[j]) { flush(); lines.push(` ${a[i]}`); i++; j++; }
    else if (j < m && (i === n || L[i][j + 1] >= L[i + 1][j])) adds.push(b[j++]);
    else dels.push(a[i++]);
  }
  flush();
  return piece;
}

/** Where an edit's piece (editPiece) is in its file's real diff for the turn (git's, the review's): each of its lines' old
 * and new numbers as the review numbers them, or null where that is not certain. Exact or nothing: the piece's lines are
 * one run of the diff's lines, each of the same kind (removed on removed, unchanged on unchanged) and text; an unchanged
 * line past git's three lines of context is counted, not compared (the diff has no text for it: the turn left it as it
 * was). Found in no place (the file changed there again later in the turn, or git drew the change otherwise) or in more
 * than one: null. An edit's text may begin and end inside a line: its first lines end the file's, its last ones begin
 * them, with the same rest of the line before and after the change. In a file the turn made, its new text found once in
 * the file's text (an edit inside a line found by its words, not by a line that equals them), new numbers alone (the
 * review has no old ones there). */
export function placeEdit(edit: Piece, file: FileDiff): [number | null, number | null][] | null {
  // every line of the file as far as the diff tells it, by its numbers: the pieces' lines, and between them (and up to the
  // edit's length past the last) the unchanged lines git leaves out, their text unknown (t null)
  type Row = { k: string; t: string | null; o: number | null; n: number | null };
  const rows: Row[] = [];
  let o = 1, n = 1;
  const same = (to: number) => { for (; o < to; o++, n++) rows.push({ k: " ", t: null, o, n }); };
  for (const p of file.pieces) {
    const lines = p.lines.filter((l) => l[0] !== "\\"), m = p.header.match(/^@@ -\d+(?:,(\d+))? \+\d+(?:,(\d+))? @@/);
    const olds = lines.filter((l) => l[0] !== "+").length, news = lines.filter((l) => l[0] !== "-").length;
    // a piece cut short (the diff past its size) or out of order: no numbers from this diff
    if (!m || olds !== +(m[1] ?? 1) || news !== +(m[2] ?? 1)) return null;
    const o0 = olds ? p.oldStart : p.oldStart + 1, n0 = news ? p.newStart : p.newStart + 1;
    if (o0 < o || o0 - o !== n0 - n) return null;
    same(o0);
    for (const x of numbered({ ...p, lines })) rows.push({ k: x.line[0], t: x.line.slice(1).replace(/\r$/, ""), o: x.old, n: x.new });
    o = o0 + olds; n = n0 + news;
  }
  same(o + edit.lines.length);
  const at = (want: { k: string; x: string }[]): number[] => {
    if (!want.some((w) => w.k !== " ")) return []; // nothing changed: nothing to find it by
    const olds = want.flatMap((w, i) => (w.k !== "+" ? [i] : [])), news = want.flatMap((w, i) => (w.k !== "-" ? [i] : []));
    const end = (ix: number[], i: number) => i === ix[0] || i === ix[ix.length - 1];
    // a line that may be part of the file's: an end on every side it is on (a removed line is on the old side alone)
    const loose = want.map((w, i) => (w.k !== "+" ? end(olds, i) : true) && (w.k !== "-" ? end(news, i) : true));
    // one side's lines at `s` as the file's: [the rest of its first line before the edit, of its last after it] for each way
    // they fit (undefined: not known, an unchanged line git left out)
    const side = (ix: number[], s: number): (string | undefined)[][] => {
      if (!ix.length) return [[undefined, undefined]];
      const f = ix[0], l = ix[ix.length - 1], tf = rows[s + f].t, tl = rows[s + l].t, xf = want[f].x, xl = want[l].x;
      if (f === l) {
        if (tf == null) return [[undefined, undefined]];
        const ways: string[][] = [];
        for (let p = 0; p + xf.length <= tf.length; p++) if (tf.startsWith(xf, p)) ways.push([tf.slice(0, p), tf.slice(p + xf.length)]);
        return ways;
      }
      const pre = tf == null ? undefined : tf.endsWith(xf) ? tf.slice(0, tf.length - xf.length) : null;
      const post = tl == null ? undefined : tl.startsWith(xl) ? tl.slice(xl.length) : null;
      return pre === null || post === null ? [] : [[pre, post]];
    };
    const agree = (a?: string, b?: string) => a === undefined || b === undefined || a === b;
    const found: number[] = [];
    for (let s = 0; s + want.length <= rows.length && found.length < 2; s++) {
      if (!want.every((w, i) => rows[s + i].k === w.k && (rows[s + i].t == null || loose[i] || rows[s + i].t === w.x))) continue;
      const ns = side(news, s);
      if (side(olds, s).some(([a, b]) => ns.some(([c, d]) => agree(a, c) && agree(b, d)))) found.push(s);
    }
    return found;
  };
  const want = edit.lines.map((l) => ({ k: l[0], x: l.slice(1) }));
  const s = at(want);
  if (s.length === 1) return want.map((_, i) => [rows[s[0] + i].o, rows[s[0] + i].n]);
  if (s.length || !file.head.some((h) => h.startsWith("new file"))) return null;
  // a file the turn made (an edit after its write): the edit's new text once in the file's, numbered from where it begins
  const added = rows.filter((r) => r.k === "+"), text = added.map((r) => r.t).join("\n"), mine = want.filter((w) => w.k !== "-").map((w) => w.x).join("\n");
  const from = text.indexOf(mine);
  if (!mine || from < 0 || text.indexOf(mine, from + 1) >= 0) return null;
  let line = added[0].n! + text.slice(0, from).split("\n").length - 1;
  return want.map((w) => (w.k === "-" ? [null, null] : [null, line++]));
}

/** A check: `npx tsx -e 'import("./src/lib/unidiff.ts").then(m => m.demo())'`. */
export function demo() {
  const d = "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n@@ -10,2 +10,3 @@\n ten\n+new\n eleven\n" +
    "diff --git a/n.txt b/n.txt\nnew file mode 100644\n--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1 @@\n+n\n";
  const fs = parse(d);
  const ok = fs.length === 2 && fs[0].path === "a.txt" && fs[0].pieces.length === 2 && fs[0].added === 2 && fs[0].removed === 1 && fs[1].path === "n.txt"
    && patchOf(fs[0], fs[0].pieces[1]) === "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -10,2 +10,3 @@\n ten\n+new\n eleven\n"
    && JSON.stringify(numbered(fs[0].pieces[0]).map((x) => [x.old, x.new])) === "[[1,1],[2,null],[null,2],[3,3]]";
  if (!ok) throw new Error(`unidiff: ${JSON.stringify(fs, null, 1)}`);
  const w = changed('const fn = vi.fn().mockRejectedValueOnce(new Error("boom"));', 'const fn = vi.fn().mockRejectedValueOnce(new Error("boom")).mockResolvedValue("ok");');
  if (JSON.stringify(w) !== JSON.stringify({ before: [59, 59], after: [59, 83] })) throw new Error(`changed: ${JSON.stringify(w)}`);
  if (changed("alpha beta", "gamma delta") !== null) throw new Error("changed: a line rewritten whole has no marks");
  const moved = parse("diff --git a/t.ts b/t.ts\n--- a/t.ts\n+++ b/t.ts\n@@ -1,2 +1,3 @@\n-const fn = make(1);\n-await run(fn);\n+setup();\n+const fn = make(1).twice();\n+const p = run(fn);\n")[0].pieces[0];
  if (JSON.stringify([...wordMarks(moved)]) !== JSON.stringify([[0, [18, 18]], [3, [18, 26]], [1, [0, 5]], [4, [0, 9]]])) throw new Error(`wordMarks: ${JSON.stringify([...wordMarks(moved)])}`);
  if (patchOfFile(fs[0]) !== "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n@@ -10,2 +10,3 @@\n ten\n+new\n eleven\n") throw new Error("patchOfFile");
  if (JSON.stringify(sideBySide(fs[0].pieces[0])) !== JSON.stringify([{ left: 0, right: 0 }, { left: 1, right: 2 }, { left: 3, right: 3 }])) throw new Error(`sideBySide: ${JSON.stringify(sideBySide(fs[0].pieces[0]))}`);
  // an edit's lines in the turn's diff: numbered as the review numbers them, exact or not at all
  const file = (body: string, head = "") => parse(`diff --git a/f.ts b/f.ts\n${head}--- a/f.ts\n+++ b/f.ts\n${body}`)[0];
  const place = (before: string, after: string, f: FileDiff) => JSON.stringify(placeEdit(editPiece(before, after), f));
  const want = (what: string, got: string, w: unknown) => { if (got !== JSON.stringify(w)) throw new Error(`placeEdit, ${what}: ${got}`); };
  // unique, its first unchanged line past git's three of context (counted, not compared)
  const ten = file("@@ -3,7 +3,7 @@\n c\n d\n e\n-f\n+F\n g\n h\n i\n");
  want("a unique block", place("b\nc\nd\ne\nf", "b\nc\nd\ne\nF", ten), [[2, 2], [3, 3], [4, 4], [5, 5], [6, null], [null, 6]]);
  // the same change in two places: which one is not known
  const twice = file("@@ -1,3 +1,3 @@\n x\n-foo\n+bar\n y\n@@ -20,3 +20,3 @@\n x\n-foo\n+bar\n y\n");
  want("a duplicate block", place("foo", "bar", twice), null);
  want("a duplicate block with its context", place("x\nfoo", "x\nbar", twice), null);
  // changed again later in the turn: bar became baz, or the line after it changed too
  want("a later re-edit", place("foo", "bar", file("@@ -1,3 +1,3 @@\n x\n-foo\n+baz\n y\n")), null);
  want("a later edit beside it", place("foo", "bar", file("@@ -1,3 +1,3 @@\n x\n-foo\n-y\n+bar\n+Y\n")), null);
  // a multi-edit: each piece on its own, the second's new numbers after the first's added line
  const two = file("@@ -1,5 +1,6 @@\n import a;\n-import b;\n+import b, c;\n+import d;\n \n fn x() {\n   one();\n@@ -18,5 +19,5 @@\n l18\n l19\n fn y() {\n-  two();\n+  three();\n }\n");
  want("a multi-edit's first piece", place("import b;", "import b, c;\nimport d;", two), [[2, null], [null, 2], [null, 3]]);
  want("a multi-edit's second piece", place("fn y() {\n  two();", "fn y() {\n  three();", two), [[20, 21], [21, null], [null, 22]]);
  // CRLF; the file's first line and its last (no newline at its end; git's count left out for one line)
  want("CRLF", place("a\r\nc", "b\r\nc", file("@@ -1,2 +1,2 @@\n-a\r\n+b\r\n c\r\n")), [[1, null], [null, 1], [2, 2]]);
  want("the file's end", place("b\nc", "b\nC", file("@@ -2,2 +2,2 @@\n b\n-c\n\\ No newline at end of file\n+C\n\\ No newline at end of file\n")), [[2, 2], [3, null], [null, 3]]);
  // an edit inside a line: the rest of the line the same before and after it, or no numbers
  const mid = file("@@ -5 +5 @@\n-  const x = foo(1);\n+  const x = foo(2);\n");
  want("inside a line", place("foo(1)", "foo(2)", mid), [[5, null], [null, 5]]);
  want("not this line's change", place("foo(1)", "bar(2)", mid), null);
  want("an edit that ends inside a line", place("y();\n  const x = foo(1", "y();\n  const x = foo(2", file("@@ -4,2 +4,2 @@\n   y();\n-  const x = foo(1);\n+  const x = foo(2);\n")), [[4, 4], [5, null], [null, 5]]);
  // a file the turn made, edited after its write: new numbers alone; a piece cut short: none
  const made = file("@@ -0,0 +1,3 @@\n+a\n+B\n+c\n", "new file mode 100644\n");
  want("a new file", place("b", "B", made), [[null, null], [null, 2]]);
  want("a new file, lines added", place("B", "B\nB2", file("@@ -0,0 +1,4 @@\n+a\n+B\n+B2\n+c\n", "new file mode 100644\n")), [[null, 2], [null, 3]]);
  want("a new file, inside a line", place("2", "3", file("@@ -0,0 +1,2 @@\n+x = 1;\n+y = 3;\n", "new file mode 100644\n")), [[null, null], [null, 2]]);
  // review: its new text inside another line too, or twice: which one is not known (it read line 1 for line 4, 2 for 3)
  want("a new file, the text in two lines", place("debug: true", "debug: false", file("@@ -0,0 +1,4 @@\n+debug: false\n+server:\n+  port: 80\n+  debug: false\n", "new file mode 100644\n")), null);
  want("a new file, a line that equals it and one that holds it", place("fox", "foo", file("@@ -0,0 +1,3 @@\n+a\n+foo\n+foo bar\n", "new file mode 100644\n")), null);
  // a name with a space: git ends it with a tab on the ---/+++ lines
  const spaced = parse("diff --git a/my file.ts b/my file.ts\n--- a/my file.ts\t\n+++ b/my file.ts\t\n@@ -1 +1 @@\n-a\n+b\n")[0];
  want("a name with a space", JSON.stringify([spaced.path, placeEdit(editPiece("a", "b"), spaced)]), ["my file.ts", [[1, null], [null, 1]]]);
  want("a cut piece", place("b", "B", file("@@ -1,3 +1,3 @@\n a\n-b\n+B\n")), null);
  console.log("unidiff ok");
}
