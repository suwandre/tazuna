// Markdown for model answers: code fences (coloured, with their language and a copy button), headings, lists (nested,
// numbered from where they start; "- [x]" task items with a drawn box), quotes, tables (aligned as their |:--:| line says),
// rules (---), paragraphs (a single newline is a line break, as chat apps have it), `code`, **bold**, *italic*,
// ~~struck~~, links, TeX between $…$ (pandoc's rule), \(…\), $$…$$ or \[…\] (lib/math.tsx). React elements only, never
// raw HTML, so a reply cannot inject markup; a link opens in the browser through the core's opener (http and https only),
// never inside the window.
import { Fragment, useState, type ReactNode } from "react";
import { Check, Copy } from "lucide-react";
import { math } from "./math";
import { core, inTauri } from "./tazuna";

const open = (url: string) => (inTauri ? core.openLink(url) : window.open(url, "_blank", "noopener"));

// TeX in a line, by pandoc's rule: $$…$$ first (taken as $…$ it left a "$" on each side, design review 12), then $…$ whose
// opening $ has no space after it and whose closing $ has none before it and no letter, digit or _ after it: "$O(k)$" and
// "$k$" are math, "$5 per month, or $12" stays money and `echo $HOME/$USER` text. Never over a code span's backtick: code
// comes first. Bold, italic, struck text and a link's words may run over a line's end (one paragraph is read whole).
const TEX = String.raw`\$\$[^$\n\`]+?\$\$|\$(?![\s$])[^$\n\`]*?(?<![\s\\])\$(?!\w)`;
const INLINE = new RegExp(String.raw`(\`[^\`\n]+\`|${TEX}|\\\(.+?\\\)|!\[[^\]\n]*\]\([^)\s]+\)|\*\*[^*]+?\*\*|~~(?![\s~])[^~]+?(?<![\s~])~~|(?<![\w*])\*(?![\s*])[^*]+?(?<![\s*])\*(?![\w*])|\[[^\]]+\]\(https?:\/\/[^)\s]+\)|https?:\/\/[^\s<>()]+[^\s<>().,;:!?'"])`);

/** A newline left in a paragraph's text is a line break (chat apps have it so; a haiku in a quote ran on as one line). */
const breaks = (p: string, key: number) => (p.includes("\n") ? p.split("\n").map((x, j) => <Fragment key={`${key}.${j}`}>{j > 0 && <br />}{x}</Fragment>) : p);

function inline(text: string): ReactNode[] {
  // the split keeps what matched at the odd places: only those are read as marks (a sentence that began and ended with "$"
  // is text)
  return text.split(INLINE).map((p, i) => {
    if (i % 2 === 0) return breaks(p, i);
    if (p.startsWith("`") && p.endsWith("`") && p.length > 1) return <code key={i}>{p.slice(1, -1)}</code>;
    if (p.startsWith("$$")) return math(p.slice(2, -2), "inline", i);
    if (p.startsWith("$")) return math(p.slice(1, -1), false, i);
    if (p.startsWith("~~")) return <del key={i}>{inline(p.slice(2, -2))}</del>;
    if (p.startsWith("\\(") && p.endsWith("\\)")) return math(p.slice(2, -2), false, i);
    // what is inside bold or italic is read again: **`file.ts` lines 3-4** showed its backticks (design review 5)
    if (p.startsWith("**") && p.endsWith("**") && p.length > 3) return <strong key={i}>{inline(p.slice(2, -2))}</strong>;
    if (/^\*[^*].*\*$/.test(p)) return <em key={i}>{inline(p.slice(1, -1))}</em>;
    // a markdown image: its words here, the picture under the answer when it is a file here (Transcript's MessageImages);
    // nothing loads from the network
    const img = p.match(/^!\[([^\]]*)\]\(([^)\s]+)\)$/);
    if (img) return <span key={i} className="text-muted">{img[1] || img[2]}</span>;
    const md = p.match(/^\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)$/);
    const url = md ? md[2] : /^https?:\/\//.test(p) ? p : null;
    if (url) return <a key={i} href={url} title={url} onClick={(e) => { e.preventDefault(); open(url); }}>{md ? md[1] : p}</a>;
    return p;
  });
}

// a few words that read as keywords in most languages; strings, numbers and comments by their usual marks
const KW = new Set(("fn let mut const var function return if else for while loop match in of import export from as use pub " +
  "struct enum impl trait type interface class def self this new await async try catch except finally raise throw " +
  "true false null None True False nil package func go defer break continue switch case default where with yield").split(" "));
// more of them where the file's language is known (in a block without one, "is", "and" or "do" may well be words)
const KW_MORE = new Set(("readonly extends implements typeof keyof instanceof undefined void static private protected public abstract satisfies " +
  "declare namespace elif lambda pass not and or is then fi do done esac local mod crate super dyn unsafe range chan").split(" "));
// SQL's, in any case
const SQL = new Set("select from where join left right inner outer on insert into values update set delete create table index alter drop and or not null is in as order by group having limit offset distinct union all case when then else end primary key references default begin commit".split(" "));

// a file's language by its extension (or its name): the highlighter's comment marks follow it (lib/steps.ts reads a step's
// file by it); none: plain text
const EXT: Record<string, string> = {};
for (const [lang, exts] of Object.entries({ ts: "ts tsx mts cts", js: "js jsx mjs cjs", json: "json jsonc json5", rs: "rs", go: "go", py: "py pyi",
  rb: "rb rake", java: "java", kt: "kt kts", swift: "swift", c: "c h", cpp: "cpp cc cxx hpp hh", cs: "cs", php: "php", sh: "sh bash zsh fish env",
  yaml: "yml yaml", toml: "toml ini cfg", sql: "sql", md: "md mdx markdown txt", css: "css", scss: "scss sass less", html: "html htm xml svg", vue: "vue",
  svelte: "svelte", lua: "lua", dart: "dart", scala: "scala", r: "r" })) for (const e of exts.split(" ")) EXT[e] = lang;
export function langOf(path?: string | null): string | null {
  const name = (path ?? "").slice(Math.max((path ?? "").lastIndexOf("/"), (path ?? "").lastIndexOf("\\")) + 1);
  if (/^(Dockerfile|Makefile|\.env(\..*)?|\.\w*rc)$/.test(name)) return "sh";
  return name.includes(".") ? EXT[name.slice(name.lastIndexOf(".") + 1).toLowerCase()] ?? null : null;
}

// comment marks by language: // (not a URL's, after "(" or ":": `url(//cdn…)`) and /* */ in the C-likes, # after a space or
// at the start in the shells, Python, YAML, TOML and Ruby (not ${#x}), -- in SQL and Lua, <!-- --> in markup; a block
// without its language keeps the usual guess. A JSDoc line's leading * is read in code() (a line is coloured alone)
const SLASH = String.raw`(?<![:(])\/\/.*$|\/\*.*?(?:\*\/|$)`, BLOCK = String.raw`\/\*.*?(?:\*\/|$)`, MARKUP = "<!--.*?(?:-->|$)";
const COMMENTS: Record<string, string> = { any: String.raw`(?<![:(])\/\/.*$|#(?![\w[!]).*$|--\s.*$`, hash: String.raw`(?<!\S)#.*$`, dash: "--.*$", slash: SLASH, css: BLOCK, html: MARKUP, mixed: `${SLASH}|${MARKUP}` };
// data and markup: only their literals read as keywords ("name: Build and test" has none)
const DATA = new Set(["yaml", "toml", "json", "html", "css", "scss"]), LITERALS = new Set("true false null None True False nil".split(" "));
const FAMILY: Record<string, string> = { py: "hash", rb: "hash", sh: "hash", yaml: "hash", toml: "hash", r: "hash", sql: "dash", lua: "dash", css: "css", html: "html", vue: "mixed", svelte: "mixed" };
const TOKENS = String.raw`"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|\`(?:\\.|[^\`\\])*\`|\b\d[\d_.]*\b|\b[A-Za-z_]\w*\b`;
const SPLITS = new Map<string, RegExp>();

/** A code line in colours: comments, strings, numbers, keywords (also the review's diff lines and a step's code). `lang`: the
 * file's (`langOf`), for its comment marks; "md" and "text" stay plain; none: a guess that suits most. */
export function code(line: string, key: number, newline = true, lang?: string | null): ReactNode {
  if (lang === "md" || lang === "text") return <span key={key}>{line}{newline && "\n"}</span>;
  const family = lang ? FAMILY[lang] ?? "slash" : "any";
  // inside a /* */ block: a line that starts with * ("* @param x", "*/"), but not "* factor;", a product's next line
  if (/^(slash|css|mixed)$/.test(family) && /^\s*\*(\/|\s|$)/.test(line) && !/;\s*$/.test(line)) return <span key={key}><span className="tok-comment">{line}</span>{newline && "\n"}</span>;
  let split = SPLITS.get(family);
  if (!split) SPLITS.set(family, (split = new RegExp(`(${COMMENTS[family]})|(${TOKENS})`)));
  // the split keeps a comment at i + 1 and any other token at i + 2
  const parts = line.split(split), out: ReactNode[] = [];
  for (let i = 0; i < parts.length; i += 3) {
    if (parts[i]) out.push(parts[i]);
    const c = parts[i + 1], t = parts[i + 2];
    if (c) out.push(<span key={i} className="tok-comment">{c}</span>);
    else if (t) out.push(/^["'`]/.test(t) ? <span key={i} className="tok-string">{t}</span>
      : /^\d/.test(t) ? <span key={i} className="tok-number">{t}</span>
      : (lang && DATA.has(lang) ? LITERALS.has(t) : KW.has(t) || (lang && KW_MORE.has(t)) || (lang === "sql" && SQL.has(t.toLowerCase()))) ? <span key={i} className="tok-keyword">{t}</span> : t);
  }
  return <span key={key}>{out}{newline && "\n"}</span>;
}

const cells = (row: string) => row.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((c) => c.trim());

/** A fenced block: its language and a copy button over it (until desktop 0.1.74 a bare box). */
function CodeBlock({ lang, lines }: { lang: string; lines: string[] }) {
  const [done, setDone] = useState(false);
  const copy = () => { navigator.clipboard?.writeText(lines.join("\n")).then(() => { setDone(true); setTimeout(() => setDone(false), 1400); }, () => {}); };
  return (
    <div className="code-block">
      <div className="code-head"><span>{lang || "code"}</span>
        <button onClick={copy} title="Copy the code">{done ? <Check size={12} strokeWidth={1.75} /> : <Copy size={12} strokeWidth={1.5} />}{done ? "Copied" : "Copy"}</button></div>
      <pre><code>{lines.map((l, i) => code(l, i))}</code></pre>
    </div>
  );
}

type Item = { text: string; sub?: List };
type List = { ordered: boolean; start: number; indent: number; items: Item[] };

function renderList(l: List, key: number): ReactNode {
  const items = l.items.map((it, i) => {
    // a task item: its box drawn, done or not (not a form's checkbox: nothing here can be ticked)
    const task = it.text.match(/^\[([ xX])\]\s+(.*)/);
    return task ? <li key={i} className="task"><span role="img" aria-label={task[1] === " " ? "to do" : "done"} className="task-box">{task[1] !== " " && <Check size={11} strokeWidth={2.25} />}</span>{inline(task[2])}{it.sub && renderList(it.sub, 0)}</li>
      : <li key={i}>{inline(it.text)}{it.sub && renderList(it.sub, 0)}</li>;
  });
  return l.ordered ? <ol key={key} start={l.start === 1 ? undefined : l.start}>{items}</ol> : <ul key={key}>{items}</ul>;
}

export function Markdown({ text }: { text: string }) {
  const out: ReactNode[] = [];
  const lines = text.split("\n");
  let para: string[] = [], list: List | null = null, quote: string[] = [];
  // a paragraph or a quote is read whole, its newlines line breaks (inline() keeps a mark that runs over one)
  const flush = () => {
    if (para.length) out.push(<p key={out.length}>{inline(para.join("\n"))}</p>);
    if (list) out.push(renderList(list, out.length));
    if (quote.length) out.push(<blockquote key={out.length}>{inline(quote.join("\n"))}</blockquote>);
    para = []; list = null; quote = [];
  };
  // the deepest list an item at this indent belongs to: a deeper indent opens a list under the last item
  const place = (indent: number, ordered: boolean, start: number): List => {
    let l = list!;
    for (;;) {
      const last = l.items[l.items.length - 1];
      if (indent <= l.indent + 1 || !last) return l;
      if (!last.sub || indent < last.sub.indent) return (last.sub ??= { ordered, start, indent, items: [] });
      l = last.sub;
    }
  };
  const deepest = () => { let l = list!; while (l.items.at(-1)?.sub) l = l.items.at(-1)!.sub!; return l.items.at(-1)!; };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.trimStart().startsWith("```")) {
      flush();
      const lang = line.trim().slice(3).trim().split(/\s/)[0] ?? "", body: string[] = [];
      while (++i < lines.length && !lines[i].trimStart().startsWith("```")) body.push(lines[i]);
      out.push(<CodeBlock key={out.length} lang={lang} lines={body} />);
      continue;
    }
    // a display formula: $$…$$ or \[…\] alone, on one line or several, and only once it is closed: "$$x$$ and more" is a
    // line with a formula in it, and an unclosed $$ stays text (it swallowed the rest of the answer)
    const opens = line.trim().startsWith("$$") ? "$$" : line.trim().startsWith("\\[") ? "\\]" : null;
    const first = opens ? line.trim().slice(2) : "", at = opens ? first.indexOf(opens) : -1;
    const end = !opens ? -1 : at >= 0 ? (first.slice(at + 2).trim() ? -1 : i) : lines.findIndex((l, j) => j > i && l.trimEnd().endsWith(opens));
    if (opens && end >= 0) {
      flush();
      const body = [first, ...lines.slice(i + 1, end + 1)].join("\n").trimEnd();
      i = end;
      out.push(<div key={out.length} className="math-block">{math(body.slice(0, body.lastIndexOf(opens)), true)}</div>);
      continue;
    }
    // a table: a row of cells, then a |---|:--:|--:| line that also says how each column aligns
    if (line.trim().startsWith("|") && /^\s*\|?\s*:?-{2,}/.test(lines[i + 1] ?? "")) {
      flush();
      const head = cells(line), align = cells(lines[i + 1]).map((c) => (c.endsWith(":") ? (c.startsWith(":") ? "center" : "right") : "left") as "left" | "right" | "center");
      const rows: string[][] = [];
      i += 1;
      while (i + 1 < lines.length && lines[i + 1].trim().startsWith("|")) rows.push(cells(lines[++i]));
      // a short cell ("+2 −1") stays on one line; a long one wraps, its code too (a long path crushed the other columns)
      const cell = (c: string, j: number) => ({ textAlign: align[j], whiteSpace: c.length <= 12 ? "nowrap" as const : undefined });
      out.push(
        <div key={out.length} className="table-wrap"><table>
          <thead><tr>{head.map((c, j) => <th key={j} style={cell(c, j)}>{inline(c)}</th>)}</tr></thead>
          <tbody>{rows.map((r, k) => <tr key={k}>{r.map((c, j) => <td key={j} style={cell(c, j)}>{inline(c)}</td>)}</tr>)}</tbody>
        </table></div>,
      );
      continue;
    }
    const h = line.match(/^(#{1,6})\s+(.*)/), item = line.match(/^(\s*)(?:([-*+])|(\d+)[.)])\s+(.*)/), q = line.match(/^>\s?(.*)/);
    if (!line.trim()) { if (!(list && /^\s+\S/.test(lines[i + 1] ?? ""))) flush(); }
    // a paragraph's line with === or --- right under it is a heading (setext): # and ## again, not a rule
    else if (para.length && !list && !quote.length && /^ {0,3}(=+|-{2,})\s*$/.test(line)) {
      const title = para.join(" ");
      para = [];
      out.push(line.trim()[0] === "=" ? <h2 key={out.length}>{inline(title)}</h2> : <h3 key={out.length}>{inline(title)}</h3>);
    }
    // a rule: ---, *** or ___ alone on a line (before a list item: "* * *" is one too)
    else if (/^ {0,3}([-*_])(?:\s*\1){2,}\s*$/.test(line)) { flush(); out.push(<hr key={out.length} />); }
    else if (h) {
      // a heading reads as one: # and ## as section titles, the rest as a bold lead-in (until desktop 0.1.74 every heading
      // was a bold paragraph)
      flush();
      const Tag = h[1].length === 1 ? "h2" : h[1].length === 2 ? "h3" : "h4";
      out.push(<Tag key={out.length}>{inline(h[2])}</Tag>);
    }
    else if (q) { if (para.length || list) flush(); quote.push(q[1]); }
    else if (item) {
      const indent = item[1].length, ordered = !!item[3], start = ordered ? Number(item[3]) : 1;
      if (para.length || quote.length || (list && indent <= list.indent + 1 && list.ordered !== ordered)) flush();
      if (!list) list = { ordered, start, indent, items: [] };
      place(indent, ordered, start).items.push({ text: item[4] });
    }
    else if (list && /^\s+/.test(line)) deepest().text += ` ${line.trim()}`;
    else { if (list || quote.length) flush(); para.push(line.trim()); }
  }
  flush();
  return <>{out}</>;
}

/** A check that the parser keeps its shapes: `npx tsx -e 'import("./src/lib/markdown.tsx").then(m => m.demo())'`. */
export async function demo() {
  const { renderToStaticMarkup } = await import("react-dom/server");
  const html = renderToStaticMarkup(<Markdown text={"## Why\n\nFix `add` *now*:\n\n1. first\n\n```py\nreturn a + b # sum\n\nx = \"s\"\n```\n\n2. second\n   - nested\n   - **two**\n3. third\n\n> quoted\n\n| a | b |\n|---|--:|\n| 1 | [docs](https://example.com) |\n\nsee https://tazuna.dev."} />);
  if (!renderToStaticMarkup(<Markdown text="**`a.ts` lines 1-2:** fix" />).includes("<strong><code>a.ts</code> lines 1-2:</strong>")) throw new Error("markdown: code in bold");
  if (/<img|!\[/.test(renderToStaticMarkup(<Markdown text="See ![the chart](out/c.png) here" />))) throw new Error("markdown: an image is its words, nothing loaded");
  if (!renderToStaticMarkup(<Markdown text="total $T = \\sum_i t_i$, and \\(x^2\\)" />).includes("<math")) throw new Error("markdown: inline TeX");
  if (renderToStaticMarkup(<Markdown text="it costs $5 and $10 a month" />).includes("<math")) throw new Error("markdown: money is not math");
  if (!renderToStaticMarkup(<Markdown text={"$$\n\\frac{a}{b}\n$$"} />).includes('display="block"')) throw new Error("markdown: display TeX");
  // design review 12: struck text, rules, task items, line breaks, TeX by pandoc's rule, $$…$$ after $…$, table cells
  const md = (t: string) => renderToStaticMarkup(<Markdown text={t} />);
  const cases: [string, (h: string) => boolean][] = [
    ["and ~~struck~~ text", (h) => h.includes("<del>struck</del>")],
    ["a\n\n---\n\nb\n\n* * *", (h) => (h.match(/<hr\/>/g) ?? []).length === 2 && !h.includes("<li")],
    ["- [x] done item\n- [ ] open item", (h) => h.includes('class="task"') && h.includes('aria-label="done"') && h.includes('aria-label="to do"') && !h.includes("<input") && !h.includes("[x]")],
    ["> An old silent pond\n> a frog jumps into the pond\n> splash! silence again", (h) => (h.match(/<br\/>/g) ?? []).length === 2],
    ["one\ntwo", (h) => h.includes("<p>one<br/>two</p>")],
    ["roughly $O(k)$ for $k$ captured", (h) => (h.match(/<math/g) ?? []).length === 2 && !h.includes("$")],
    ["That costs $5 per month, or $12 with support.", (h) => !h.includes("<math") && h.includes("$5 per month, or $12")],
    ["$20,000 and $30,000, or US$5-$10", (h) => !h.includes("<math")],
    ["the lookup is $O(1)$: $$t_{lookup} = \\Theta(1)$$", (h) => h.includes('displaystyle="true"') && !h.includes("$")],
    // the independent review of design review 12: setext headings, marks over a line's end, code before math, $ before a
    // letter, an unclosed $$, struck text with spaces inside
    ["Title\n===\n\nSub\n---\n\ntext\n\n---", (h) => h.includes("<h2>Title</h2>") && h.includes("<h3>Sub</h3>") && h.includes("<hr/>")],
    ["a **very\nimportant** [link\ntext](https://example.com) end", (h) => h.includes("<strong>very<br/>important</strong>") && h.includes('href="https://example.com"')],
    ["run `echo $x$` and echo $HOME/$USER", (h) => !h.includes("<math") && h.includes("<code>echo $x$</code>")],
    ["$$x\nand the rest of the answer\n\nmore", (h) => !h.includes("<math") && h.includes("the rest of the answer") && h.includes("<p>more</p>")],
    ["$$x$$ and more", (h) => h.includes('displaystyle="true"') && h.includes("and more")],
    ["~~ a ~~ and ~~b~~", (h) => h.includes("~~ a ~~") && h.includes("<del>b</del>")],
    ["| File | Lines |\n|---|--:|\n| `a/very/long/path/that/keeps/going/src/components/SomethingVeryLong.tsx` | +2 −1 |", (h) => h.includes('white-space:nowrap">+2 −1') && !h.includes('nowrap"><code')],
  ];
  for (const [t, ok] of cases) if (!ok(md(t))) throw new Error(`markdown: ${JSON.stringify(t)}\n${md(t)}`);
  // a step's code by its file's language: each its own comment marks, SQL's keywords in any case, markdown plain
  const hl = (line: string, lang: string) => renderToStaticMarkup(<>{code(line, 0, false, lang)}</>);
  const marks: [string, string, boolean][] = [["x = 1  # note", "py", true], ["see https://a.b // no", "py", false], ["echo ${#arr[@]}", "sh", false], ["key: v # why", "yaml", true],
    [" * the doc line", "ts", true], ["this.#count++", "ts", false], ["a /* b */ c", "css", true], ["// not css", "css", false], ["SELECT a -- the id", "sql", true], ["# Title", "md", false],
    // review of review 14: a URL with no scheme, a product's next line, the end of a block
    ["  background: url(//cdn.example.com/a.png);", "scss", false], ["a: url(http://x.y/z); // why", "scss", true], ["    * factor;", "ts", false], [" */", "ts", true], ["see http://x.y", "ts", false]];
  for (const [line, lang, comment] of marks) if (hl(line, lang).includes("tok-comment") !== comment) throw new Error(`markdown: ${lang} ${JSON.stringify(line)}: ${hl(line, lang)}`);
  if (hl("name: Build and test", "yaml").includes("tok-keyword") || !hl("on: true", "yaml").includes('tok-keyword">true') || hl("<p>for this</p>", "html").includes("tok-keyword")) throw new Error("markdown: keywords in data");
  if (!hl("select id from t", "sql").includes('tok-keyword">select') || !hl("readonly x: string", "ts").includes('tok-keyword">readonly') || renderToStaticMarkup(<>{code("it is done and not or", 0)}</>).includes("tok-keyword")) throw new Error("markdown: keywords by language");
  for (const want of ["<h3>Why</h3>", "<em>now</em>", '<ol start="2"><li>second<ul><li>nested</li><li><strong>two</strong></li></ul></li><li>third</li></ol>',
    '<div class="code-head"><span>py</span>', "<pre>", "<blockquote>", 'style="text-align:right"', 'href="https://example.com"', 'href="https://tazuna.dev"',
    "tok-comment", "tok-string", "tok-keyword"]) {
    if (!html.includes(want)) throw new Error(`markdown: missing ${want}\n${html}`);
  }
  console.log("markdown ok");
}
