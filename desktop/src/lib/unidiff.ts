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
      if (line.startsWith("+++ b/")) f.path = line.slice(6);
      else if (line.startsWith("--- a/") && f.path === "") f.path = line.slice(6);
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
  console.log("unidiff ok");
}
