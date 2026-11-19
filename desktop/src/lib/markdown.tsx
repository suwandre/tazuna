// Markdown for model answers: code fences (coloured, with their language and a copy button), headings, lists (nested,
// numbered from where they start), quotes, tables (aligned as their |:--:| line says), paragraphs, `code`, **bold**,
// *italic*, links. React elements only, never raw HTML, so a reply cannot inject markup; a link opens in the browser through the
// core's opener (http and https only), never inside the window.
import { useState, type ReactNode } from "react";
import { Check, Copy } from "lucide-react";
import { core, inTauri } from "./tazuna";

const open = (url: string) => (inTauri ? core.openLink(url) : window.open(url, "_blank", "noopener"));

function inline(text: string): ReactNode[] {
  return text.split(/(`[^`\n]+`|\*\*[^*\n]+\*\*|(?<![\w*])\*(?![\s*])[^*\n]+(?<![\s*])\*(?![\w*])|\[[^\]\n]+\]\(https?:\/\/[^)\s]+\)|https?:\/\/[^\s<>()]+[^\s<>().,;:!?'"])/).map((p, i) => {
    if (p.startsWith("`") && p.endsWith("`") && p.length > 1) return <code key={i}>{p.slice(1, -1)}</code>;
    if (p.startsWith("**") && p.endsWith("**") && p.length > 3) return <strong key={i}>{p.slice(2, -2)}</strong>;
    if (/^\*[^*].*\*$/.test(p)) return <em key={i}>{p.slice(1, -1)}</em>;
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

/** A code line in colours: comments, strings, numbers, keywords (also the review's diff lines). */
export function code(line: string, key: number, newline = true): ReactNode {
  const parts = line.split(/(\/\/.*$|#(?![\w[!]).*$|--\s.*$|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`|\b\d[\d_.]*\b|\b[A-Za-z_]\w*\b)/);
  return (
    <span key={key}>
      {parts.map((p, i) => !p ? null
        : /^(\/\/|#|--\s)/.test(p) ? <span key={i} className="tok-comment">{p}</span>
        : /^["'`]/.test(p) ? <span key={i} className="tok-string">{p}</span>
        : /^\d/.test(p) ? <span key={i} className="tok-number">{p}</span>
        : KW.has(p) ? <span key={i} className="tok-keyword">{p}</span>
        : p)}
      {newline && "\n"}
    </span>
  );
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
  const items = l.items.map((it, i) => <li key={i}>{inline(it.text)}{it.sub && renderList(it.sub, 0)}</li>);
  return l.ordered ? <ol key={key} start={l.start === 1 ? undefined : l.start}>{items}</ol> : <ul key={key}>{items}</ul>;
}

export function Markdown({ text }: { text: string }) {
  const out: ReactNode[] = [];
  const lines = text.split("\n");
  let para: string[] = [], list: List | null = null, quote: string[] = [];
  const flush = () => {
    if (para.length) out.push(<p key={out.length}>{inline(para.join(" "))}</p>);
    if (list) out.push(renderList(list, out.length));
    if (quote.length) out.push(<blockquote key={out.length}>{inline(quote.join(" "))}</blockquote>);
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
    // a table: a row of cells, then a |---|:--:|--:| line that also says how each column aligns
    if (line.trim().startsWith("|") && /^\s*\|?\s*:?-{2,}/.test(lines[i + 1] ?? "")) {
      flush();
      const head = cells(line), align = cells(lines[i + 1]).map((c) => (c.endsWith(":") ? (c.startsWith(":") ? "center" : "right") : "left") as "left" | "right" | "center");
      const rows: string[][] = [];
      i += 1;
      while (i + 1 < lines.length && lines[i + 1].trim().startsWith("|")) rows.push(cells(lines[++i]));
      out.push(
        <div key={out.length} className="table-wrap"><table>
          <thead><tr>{head.map((c, j) => <th key={j} style={{ textAlign: align[j] }}>{inline(c)}</th>)}</tr></thead>
          <tbody>{rows.map((r, k) => <tr key={k}>{r.map((c, j) => <td key={j} style={{ textAlign: align[j] }}>{inline(c)}</td>)}</tr>)}</tbody>
        </table></div>,
      );
      continue;
    }
    const h = line.match(/^(#{1,6})\s+(.*)/), item = line.match(/^(\s*)(?:([-*+])|(\d+)[.)])\s+(.*)/), q = line.match(/^>\s?(.*)/);
    if (!line.trim()) { if (!(list && /^\s+\S/.test(lines[i + 1] ?? ""))) flush(); }
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
  for (const want of ["<h3>Why</h3>", "<em>now</em>", '<ol start="2"><li>second<ul><li>nested</li><li><strong>two</strong></li></ul></li><li>third</li></ol>',
    '<div class="code-head"><span>py</span>', "<pre>", "<blockquote>", 'style="text-align:right"', 'href="https://example.com"', 'href="https://tazuna.dev"',
    "tok-comment", "tok-string", "tok-keyword"]) {
    if (!html.includes(want)) throw new Error(`markdown: missing ${want}\n${html}`);
  }
  console.log("markdown ok");
}
