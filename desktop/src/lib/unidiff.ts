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
  console.log("unidiff ok");
}
