// Markdown for model answers: code fences (coloured), headings, lists, quotes, tables, paragraphs, `code`, **bold**,
// links. React elements only, never raw HTML, so a reply cannot inject markup; a link opens in the browser through the
// core's opener (http and https only), never inside the window.
import type { ReactNode } from "react";
import { core, inTauri } from "./tazuna";

const open = (url: string) => (inTauri ? core.openLink(url) : window.open(url, "_blank", "noopener"));

function inline(text: string): ReactNode[] {
  return text.split(/(`[^`\n]+`|\*\*[^*\n]+\*\*|\[[^\]\n]+\]\(https?:\/\/[^)\s]+\)|https?:\/\/[^\s<>()]+[^\s<>().,;:!?'"])/).map((p, i) => {
    if (p.startsWith("`") && p.endsWith("`") && p.length > 1) return <code key={i}>{p.slice(1, -1)}</code>;
    if (p.startsWith("**") && p.endsWith("**") && p.length > 3) return <strong key={i}>{p.slice(2, -2)}</strong>;
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

/** A code line in colours: comments, strings, numbers, keywords. */
function code(line: string, key: number): ReactNode {
  const parts = line.split(/(\/\/.*$|#(?![\w[!]).*$|--\s.*$|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`|\b\d[\d_.]*\b|\b[A-Za-z_]\w*\b)/);
  return (
    <span key={key}>
      {parts.map((p, i) => !p ? null
        : /^(\/\/|#|--\s)/.test(p) ? <span key={i} className="tok-comment">{p}</span>
        : /^["'`]/.test(p) ? <span key={i} className="tok-string">{p}</span>
        : /^\d/.test(p) ? <span key={i} className="tok-number">{p}</span>
        : KW.has(p) ? <span key={i} className="tok-keyword">{p}</span>
        : p)}
      {"\n"}
    </span>
  );
}

const cells = (row: string) => row.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((c) => c.trim());

export function Markdown({ text }: { text: string }) {
  const out: ReactNode[] = [];
  const lines = text.split("\n");
  let para: string[] = [], list: { ordered: boolean; items: string[] } | null = null, quote: string[] = [];
  const flush = () => {
    if (para.length) out.push(<p key={out.length}>{inline(para.join(" "))}</p>);
    if (list) out.push(list.ordered
      ? <ol key={out.length}>{list.items.map((t, i) => <li key={i}>{inline(t)}</li>)}</ol>
      : <ul key={out.length}>{list.items.map((t, i) => <li key={i}>{inline(t)}</li>)}</ul>);
    if (quote.length) out.push(<blockquote key={out.length}>{inline(quote.join(" "))}</blockquote>);
    para = []; list = null; quote = [];
  };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.trimStart().startsWith("```")) {
      flush();
      const body: string[] = [];
      while (++i < lines.length && !lines[i].trimStart().startsWith("```")) body.push(lines[i]);
      out.push(<pre key={out.length}><code>{body.map(code)}</code></pre>);
      continue;
    }
    // a table: a row of cells, then a |---|---| line
    if (line.trim().startsWith("|") && /^\s*\|?\s*:?-{2,}/.test(lines[i + 1] ?? "")) {
      flush();
      const head = cells(line), rows: string[][] = [];
      i += 1;
      while (i + 1 < lines.length && lines[i + 1].trim().startsWith("|")) rows.push(cells(lines[++i]));
      out.push(
        <div key={out.length} className="table-wrap"><table>
          <thead><tr>{head.map((c, j) => <th key={j}>{inline(c)}</th>)}</tr></thead>
          <tbody>{rows.map((r, k) => <tr key={k}>{r.map((c, j) => <td key={j}>{inline(c)}</td>)}</tr>)}</tbody>
        </table></div>,
      );
      continue;
    }
    const h = line.match(/^#{1,6}\s+(.*)/), item = line.match(/^\s*(?:([-*])|(\d+)[.)])\s+(.*)/), q = line.match(/^>\s?(.*)/);
    if (!line.trim()) flush();
    else if (h) { flush(); out.push(<p key={out.length}><strong>{inline(h[1])}</strong></p>); }
    else if (q) { if (para.length || list) flush(); quote.push(q[1]); }
    else if (item) {
      if (para.length || quote.length || (list && list.ordered !== !!item[2])) flush();
      (list ??= { ordered: !!item[2], items: [] }).items.push(item[3]);
    }
    else if (list && /^\s+/.test(line)) list.items[list.items.length - 1] += ` ${line.trim()}`;
    else { if (list || quote.length) flush(); para.push(line.trim()); }
  }
  flush();
  return <>{out}</>;
}

/** A check that the parser keeps its shapes: `npx tsx -e 'import("./src/lib/markdown.tsx").then(m => m.demo())'`. */
export function demo() {
  const s = (n: ReactNode) => JSON.stringify(n, (k, v) => (k === "_owner" || k === "_store" ? undefined : v));
  const r = s(Markdown({ text: "Fix `add`:\n\n```py\nreturn a + b # sum\n\nx = \"s\"\n```\n- one\n- **two**\n1. a\n\n> quoted\n\n| a | b |\n|---|---|\n| 1 | [docs](https://example.com) |\n\nsee https://tazuna.dev." }));
  for (const want of ['"type":"pre"', '"type":"ul"', '"type":"ol"', '"type":"strong"', '"type":"code"', '"type":"blockquote"', '"type":"table"',
    '"href":"https://example.com"', '"href":"https://tazuna.dev"', 'tok-comment', 'tok-string', 'tok-keyword']) {
    if (!r.includes(want)) throw new Error(`markdown: missing ${want}`);
  }
  console.log("markdown ok");
}
