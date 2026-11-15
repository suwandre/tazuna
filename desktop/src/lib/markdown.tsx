// Just enough markdown for model answers: code fences, headings, lists, paragraphs, `code`, **bold**. React elements
// only, never raw HTML, so a reply cannot inject markup. ponytail: no tables or links yet; add a real parser (with no
// raw HTML) when answers need them.
import type { ReactNode } from "react";

function inline(text: string): ReactNode[] {
  return text.split(/(`[^`\n]+`|\*\*[^*\n]+\*\*)/).map((p, i) =>
    p.startsWith("`") && p.endsWith("`") && p.length > 1 ? <code key={i}>{p.slice(1, -1)}</code>
    : p.startsWith("**") && p.endsWith("**") && p.length > 3 ? <strong key={i}>{p.slice(2, -2)}</strong>
    : p);
}

export function Markdown({ text }: { text: string }) {
  const out: ReactNode[] = [];
  const lines = text.split("\n");
  let para: string[] = [], list: { ordered: boolean; items: string[] } | null = null;
  const flush = () => {
    if (para.length) out.push(<p key={out.length}>{inline(para.join(" "))}</p>);
    if (list) out.push(list.ordered
      ? <ol key={out.length}>{list.items.map((t, i) => <li key={i}>{inline(t)}</li>)}</ol>
      : <ul key={out.length}>{list.items.map((t, i) => <li key={i}>{inline(t)}</li>)}</ul>);
    para = []; list = null;
  };
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.trimStart().startsWith("```")) {
      flush();
      const code: string[] = [];
      while (++i < lines.length && !lines[i].trimStart().startsWith("```")) code.push(lines[i]);
      out.push(<pre key={out.length}><code>{code.join("\n")}</code></pre>);
      continue;
    }
    const h = line.match(/^#{1,6}\s+(.*)/), item = line.match(/^\s*(?:([-*])|(\d+)[.)])\s+(.*)/);
    if (!line.trim()) flush();
    else if (h) { flush(); out.push(<p key={out.length}><strong>{inline(h[1])}</strong></p>); }
    else if (item) {
      if (para.length || (list && list.ordered !== !!item[2])) flush();
      (list ??= { ordered: !!item[2], items: [] }).items.push(item[3]);
    }
    else if (list && /^\s+/.test(line)) list.items[list.items.length - 1] += ` ${line.trim()}`;
    else { if (list) flush(); para.push(line.trim()); }
  }
  flush();
  return <>{out}</>;
}

/** A check that the parser keeps its shapes: `npx tsx src/lib/markdown.tsx` (no test framework). */
export function demo() {
  const s = (n: ReactNode) => JSON.stringify(n, (k, v) => (k === "_owner" || k === "_store" ? undefined : v));
  const r = s(Markdown({ text: "Fix `add`:\n\n```py\nreturn a + b\n\nx\n```\n- one\n- **two**\n1. a" }));
  for (const want of ['"type":"pre"', 'return a + b\\n\\nx', '"type":"ul"', '"type":"ol"', '"type":"strong"', '"type":"code"']) {
    if (!r.includes(want)) throw new Error(`markdown: missing ${want}`);
  }
}
