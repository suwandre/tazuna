// TeX in an answer as MathML, which the window's own engine draws (design review 10: "$T = \sum_i t_i$" stayed raw). No
// library: the common part of TeX only (scripts, \frac, \sqrt, Greek, operators and relations, \text); a command it does not
// know shows as its name, so nothing is lost. React elements only, as the rest of the markdown.
// ponytail: a subset parser; take a real TeX engine (temml) if answers need matrices, \begin{…} or \color.
import { createElement as h, type ReactNode } from "react";

const SYM: Record<string, string> = {
  alpha: "α", beta: "β", gamma: "γ", delta: "δ", epsilon: "ϵ", varepsilon: "ε", zeta: "ζ", eta: "η", theta: "θ", iota: "ι", kappa: "κ",
  lambda: "λ", mu: "μ", nu: "ν", xi: "ξ", pi: "π", rho: "ρ", sigma: "σ", tau: "τ", upsilon: "υ", phi: "ϕ", varphi: "φ", chi: "χ", psi: "ψ",
  omega: "ω", Gamma: "Γ", Delta: "Δ", Theta: "Θ", Lambda: "Λ", Xi: "Ξ", Pi: "Π", Sigma: "Σ", Phi: "Φ", Psi: "Ψ", Omega: "Ω",
  infty: "∞", partial: "∂", nabla: "∇", ell: "ℓ", hbar: "ℏ", emptyset: "∅",
};
const OP: Record<string, string> = {
  sum: "∑", prod: "∏", int: "∫", oint: "∮", cdot: "·", times: "×", div: "÷", pm: "±", mp: "∓", le: "≤", leq: "≤", ge: "≥", geq: "≥",
  ne: "≠", neq: "≠", approx: "≈", sim: "∼", equiv: "≡", propto: "∝", in: "∈", notin: "∉", subset: "⊂", subseteq: "⊆", cup: "∪",
  cap: "∩", to: "→", rightarrow: "→", leftarrow: "←", Rightarrow: "⇒", Leftrightarrow: "⇔", mapsto: "↦", ldots: "…", cdots: "⋯",
  forall: "∀", exists: "∃", neg: "¬", land: "∧", lor: "∨", circ: "∘", ast: "∗", lt: "<", gt: ">", lvert: "|", rvert: "|", mid: "|",
  langle: "⟨", rangle: "⟩", lceil: "⌈", rceil: "⌉", lfloor: "⌊", rfloor: "⌋", log: "log", ln: "ln", exp: "exp", sin: "sin", cos: "cos",
  tan: "tan", max: "max", min: "min", lim: "lim", arg: "arg", det: "det",
};
const WORD = new Set(["log", "ln", "exp", "sin", "cos", "tan", "max", "min", "lim", "arg", "det"]);
const BIG = new Set(["∑", "∏", "∫", "∮"]);

/** `tex` as a <math> element; `block` for a display formula. */
export function math(tex: string, block = false, key?: number): ReactNode {
  const toks = tex.match(/\\[a-zA-Z]+|\\.|\d+(?:\.\d+)?|\s+|./g) ?? [];
  let i = 0;
  const group = (): ReactNode[] => {
    const out: ReactNode[] = [];
    while (i < toks.length && toks[i] !== "}") {
      let base = atom();
      if (base === null) continue;
      let sub: ReactNode = null, sup: ReactNode = null;
      while (toks[i] === "_" || toks[i] === "^") {
        const t = toks[i++];
        while (toks[i]?.trim() === "") i++;
        const arg = atom();
        if (t === "_") sub = arg; else sup = arg;
      }
      if (sub !== null || sup !== null) {
        const big = typeof base === "object" && base !== null && BIG.has(String((base as { props?: { children?: unknown } }).props?.children));
        // a key of its own kind: an atom's is its token's number, and the row's count met it ("two children with the same key")
        base = sub !== null && sup !== null ? h(big && block ? "munderover" : "msubsup", { key: `s${out.length}` }, base, sub, sup)
          : h(sub !== null ? (big && block ? "munder" : "msub") : big && block ? "mover" : "msup", { key: `s${out.length}` }, base, sub ?? sup);
      }
      out.push(base);
    }
    return out;
  };
  const arg = (): ReactNode => {
    while (toks[i]?.trim() === "") i++;
    return atom();
  };
  const atom = (): ReactNode => {
    const t = toks[i++];
    if (t === undefined) return null;
    if (/^\s+$/.test(t)) return null;
    if (t === "{") { const g = group(); i++; return h("mrow", { key: i }, ...g); }
    if (/^\d/.test(t)) return h("mn", { key: i }, t);
    if (/^[a-zA-Z]$/.test(t)) return h("mi", { key: i }, t);
    if (t.startsWith("\\")) {
      const name = t.slice(1);
      if (name === "frac" || name === "dfrac" || name === "tfrac") { const a = arg(), b = arg(); return h("mfrac", { key: i }, a, b); }
      if (name === "sqrt") {
        if (toks[i] === "[") { i++; const n: string[] = []; while (i < toks.length && toks[i] !== "]") n.push(toks[i++]); i++; const a = arg(); return h("mroot", { key: i }, a, h("mn", null, n.join(""))); }
        return h("msqrt", { key: i }, arg());
      }
      if (name === "text" || name === "mathrm" || name === "operatorname" || name === "textbf" || name === "mathbf") {
        while (toks[i]?.trim() === "") i++;
        if (toks[i] !== "{") return h("mtext", { key: i }, toks[i++] ?? "");
        i++; const words: string[] = []; let depth = 0;
        while (i < toks.length && (toks[i] !== "}" || depth > 0)) { if (toks[i] === "{") depth++; if (toks[i] === "}") depth--; words.push(toks[i++]); }
        i++;
        return h(name === "text" ? "mtext" : "mi", { key: i, mathvariant: name.endsWith("bf") ? "bold" : "normal" }, words.join(""));
      }
      if (name === "left" || name === "right" || name === "big" || name === "Big" || name === "bigl" || name === "bigr") return null;
      if (name === "," || name === ";" || name === ":" || name === "quad" || name === "qquad" || name === " " || t === "\\ " || t === "\\,") return h("mspace", { key: i, width: name === "qquad" ? "2em" : name === "quad" ? "1em" : "0.2em" });
      if (SYM[name]) return h("mi", { key: i }, SYM[name]);
      if (OP[name]) return h("mo", { key: i, ...(WORD.has(name) ? { lspace: "0.1em", rspace: "0.1em" } : {}) }, OP[name]);
      if (t.length === 2) return h("mo", { key: i }, t[1]); // \{ \} \% \$ \|
      return h("mi", { key: i, mathvariant: "normal" }, name);
    }
    return h("mo", { key: i }, t === "*" ? "∗" : t === "-" ? "−" : t);
  };
  const body = group();
  return h("math", { key, display: block ? "block" : undefined }, h("mrow", null, ...body));
}

/** Whether `$…$` holds TeX, not money: "$5 and $10" stays text (design review 10). */
export const isTex = (s: string) => /[\\^_]/.test(s);

/** A check: `npx tsx -e 'import("./src/lib/math.tsx").then(m => m.demo())'`. */
export async function demo() {
  const { renderToStaticMarkup } = await import("react-dom/server");
  const out = (t: string, b = false) => renderToStaticMarkup(<>{math(t, b)}</>);
  const cases: [string, string][] = [
    ["T = \\sum_i t_i", "<msub><mo>∑</mo><mi>i</mi></msub>"],
    ["\\frac{a}{b^2}", "<mfrac><mrow><mi>a</mi></mrow><mrow><msup><mi>b</mi><mn>2</mn></msup></mrow></mfrac>"],
    ["\\sqrt{x}", "<msqrt><mrow><mi>x</mi></mrow></msqrt>"],
    ["\\alpha \\le 2", "<mi>α</mi><mo>≤</mo><mn>2</mn>"],
    ["\\text{cost} = 3", "<mtext mathvariant=\"normal\">cost</mtext>"],
    ["\\foo", "<mi mathvariant=\"normal\">foo</mi>"],
  ];
  for (const [t, want] of cases) if (!out(t).includes(want)) throw new Error(`math: ${t}\n${out(t)}`);
  if (!out("\\sum_{i=1}^n x_i", true).includes("<munderover>")) throw new Error("math: display limits");
  if (isTex("5 and ") || !isTex("x^2")) throw new Error("math: money is not TeX");
  console.log("math ok");
}
