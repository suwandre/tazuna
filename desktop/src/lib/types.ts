// What the window shows. Today filled from demo data; next from tazuna's app server (the same events the CLI renders).
export type Thread = { id: string; title: string; project?: string; running?: boolean;
  /** where it ran (a project's name, or "chat") and when it last did (seconds since 1970): the row's second line */
  place?: string; at?: number;
  /** the model it ran on: its provider's mark beside the name */
  model?: string };
export type Project = { name: string; path?: string; open: boolean; pending?: number; threads: Thread[] };
/** One step before the answer: a tool call (with what it was given and what came back) or a stretch of thinking. */
export type ToolStep = {
  verb: string; target: string; detail?: string; secs: number; ok: boolean; running?: boolean;
  kind?: "tool" | "thinking"; name?: string; input?: Record<string, unknown> | null; result?: string; text?: string;
};
export type ChangedFile = { path: string; added: number | null; removed: number | null; status: string };
/** What one turn changed (the core's git snapshots before and after it). */
export type Change = { turn: number; files: ChangedFile[]; undone: boolean; canUndo: boolean };
/** One model call of a reply: what it called (tool verbs, or empty for the answer alone), its tokens, timing and cost. */
export type Call = { tools: string; fresh: number; cached: number; out: number; cost: number; firstMs: number | null; tps: number | null; totalMs: number | null };
export type Meta = { cost: number; tokens: number; cachedPct?: number; secs: number; calls?: Call[] };
export type Message =
  | { role: "user"; text: string; queued?: boolean; edit?: { cp: number; block: number } }
  | { role: "agent"; prose: string[]; tools?: ToolStep[]; change?: Change; meta?: Meta; routedChat?: boolean; ask?: string;
      /** a reply that ended without its answer: the provider's error in words (and as it came), or stopped by you */
      failure?: Failure;
      /** errors the reply met and went on past, as the core wrote them */
      notes?: string[] };
/** `text` is the error's message alone ("[404]: … not found."), `raw` the notice as the core wrote it; both empty for a
 * reply you stopped without an error */
export type Failure = { text: string; raw: string; stopped: boolean; /** the message it answered, for Retry */ ask: string };
export type Usage = { contextPct: number; contextUsed: string; contextMax: string; plan5h: number | null; plan5hReset: string; planWeek: number | null;
  threadCost: number; threadTurns: number; cachedPct: number; saved: number; today: number; todayTurns: number; todaySeries: number[];
  /** SubRouter's balance in dollars, where a key is set */
  balance?: number | null };
export type Session = { model: string; effort: string; mode: string; branch: string; changed: number; lastCost: number };

// how a maker writes its own name; a short word in UPPER is an acronym (a quantisation, "vision-language")
const BRAND: Record<string, string> = { deepseek: "DeepSeek", gpt: "GPT", glm: "GLM", qwen: "Qwen", gemini: "Gemini", gemma: "Gemma", grok: "Grok",
  mistral: "Mistral", codestral: "Codestral", devstral: "Devstral", ernie: "ERNIE", nemotron: "Nemotron", kimi: "Kimi", llama: "Llama", muse: "Muse",
  minimax: "MiniMax", phi: "Phi", olmo: "OLMo", hermes: "Hermes", sonar: "Sonar", zai: "Z.ai" };
const UPPER = new Set(["ud", "vl", "hc", "it", "xxs", "xs", "moe", "r1", "v3"]);
const HYPHENED = new Set(["GPT", "GLM"]);

/** A model's name as people say it, from its id: "claude-opus-5-5" is "Opus 5.5", "deepseek-v4.1-flash-1017-fast"
 * "DeepSeek V4.1 Flash 1017 Fast", "gpt-6.1-sol" "GPT-6.1 Sol", "agy/gemini-4-hydrogen-high" "Gemini 4 Hydrogen",
 * "ernie-5.5-flash-preview:free" "ERNIE 5.5 Flash Preview (free)". The terminal has the same rule (src/ui/mod.rs
 * pretty_model); until desktop 0.1.78 every model but Claude's showed its id (design review 6). */
export function modelLabel(id?: string | null): string {
  if (!id) return "";
  let m = id.trim().split(/\s/)[0];
  const c = m.match(/^(?:claude-)?(opus|sonnet|haiku|fable)-(\d+)(?:[-.](\d+))?/i);
  if (c) return `${c[1][0].toUpperCase()}${c[1].slice(1).toLowerCase()} ${c[2]}${c[3] ? `.${c[3]}` : ""}`;
  const route = m.includes("/") ? m.split("/")[0] : "";
  m = m.replace(/^[\w.-]+\//, "");
  const free = m.endsWith(":free");
  m = m.replace(/:free$/, "");
  // OmniRoute's ids carry the level the effort already says
  if (route === "agy") m = m.replace(/-(low|medium|high|xhigh|max)$/, "");
  const t = m.split("-").filter(Boolean), out: string[] = [];
  for (let i = 0; i < t.length; i++) {
    let w = t[i];
    // a version split in two ("v4-1", "5-5", "qwen3-8") is one: "V4.1", "5.5", "Qwen3.8"
    if (/(^v?\d+(\.\d+)?$|[a-z]\d+$)/i.test(w) && /^\d$/.test(t[i + 1] ?? "") && !w.includes(".")) { w = `${w}.${t[i + 1]}`; i++; }
    const lower = w.toLowerCase(), brand = Object.keys(BRAND).find((b) => lower === b || (lower.startsWith(b) && /^\d/.test(lower.slice(b.length))));
    let word = brand ? BRAND[brand] + w.slice(brand.length) : UPPER.has(lower) || /^[a-z]{0,3}\d[a-z0-9_]*$/i.test(w) && /[a-z]/i.test(w) && w.length <= 8 ? w.toUpperCase()
      : /^v\d/i.test(w) ? `V${w.slice(1)}` : w[0].toUpperCase() + w.slice(1);
    if (out.length && HYPHENED.has(out[out.length - 1]) && /^\d/.test(word)) { out[out.length - 1] += `-${word}`; continue; }
    out.push(word);
  }
  return out.join(" ") + (free ? " (free)" : "");
}

/** The cases of modelLabel, in one place for both apps: `npx tsx -e 'import("./src/lib/types.ts").then(m => m.demoModels())'`. */
export const MODEL_NAMES: [string, string][] = [
  ["claude-opus-5-5", "Opus 5.5"], ["claude-haiku-4-5-20251001", "Haiku 4.5"], ["haiku", "Haiku"],
  ["deepseek-v4.1-flash-1017-fast", "DeepSeek V4.1 Flash 1017 Fast"], ["deepseek-v4-1-flash-1017", "DeepSeek V4.1 Flash 1017"],
  ["gpt-6.1-sol", "GPT-6.1 Sol"], ["gpt-5", "GPT-5"], ["glm-5.3-flash-ultrafast", "GLM-5.3 Flash Ultrafast"], ["mistral-small-2603", "Mistral Small 2603"],
  ["codestral-2508", "Codestral 2508"], ["grok-4.7", "Grok 4.7"], ["agy/gemini-4-hydrogen-high", "Gemini 4 Hydrogen"], ["muse-ember", "Muse Ember"],
  ["baidu/ernie-5.5-flash-preview:free", "ERNIE 5.5 Flash Preview (free)"], ["ernie-5-5-flash-preview", "ERNIE 5.5 Flash Preview"],
  ["nemotron-3.5-super-vl", "Nemotron 3.5 Super VL"], ["qwen3.8-flash-next-unsloth-ud-iq3_xxs", "Qwen3.8 Flash Next Unsloth UD IQ3_XXS"],
  ["qwen3-8-omni-flash", "Qwen3.8 Omni Flash"], ["gemma-3-4b-it", "Gemma 3 4B IT"], ["codestral-2508 off 0.3,0.03,0.9", "Codestral 2508"], ["zai-glm-5.3", "Z.ai GLM-5.3"],
];
export function demoModels() {
  const bad = MODEL_NAMES.filter(([id, want]) => modelLabel(id) !== want).map(([id, want]) => `${id}: ${modelLabel(id)} (wanted ${want})`);
  if (bad.length) throw new Error(bad.join("\n"));
  console.log("model names ok");
}

/** A link as a few words: a GitHub pull request is "application PR #1521", an issue "repo issue #12", a repo
 * "owner/repo", a Claude artifact "Claude artifact", anything else its host and last path part. */
export function linkName(u: string): string {
  try {
    const url = new URL(u), p = url.pathname.split("/").filter(Boolean), host = url.hostname.replace(/^www\./, "");
    if (host === "github.com" && p.length >= 4 && (p[2] === "pull" || p[2] === "issues")) return `${p[1]} ${p[2] === "pull" ? "PR" : "issue"} #${p[3]}`;
    if (host === "github.com" && p.length >= 2) return `${p[0]}/${p[1]}`;
    if (/(^|\.)claude\.ai$/.test(host) && p[0] === "artifact") return "Claude artifact";
    return p.length ? `${host}/${decodeURIComponent(p[p.length - 1]).slice(0, 40)}` : host;
  } catch { return u; }
}

/** A conversation's name from its first message: its first line with each link said in words, and when that line was only
 * a link, the next line after it ("application PR #1485: good to approve?"). Until desktop 0.1.77 the first line as
 * typed, so a list of pasted links read "https://github.com/acme/a…" eight times (design review 5). */
export function titleFrom(text?: string | null): string {
  const lines = (text ?? "").replace(/^\[tazuna: [^\]]*\]\n\n/, "").split("\n").map((l) => l.trim()).filter(Boolean);
  if (!lines.length) return "";
  const said = (l: string) => l.replace(/https?:\/\/[^\s<>()]+/g, (u) => linkName(u.replace(/[.,;:!?)]+$/, ""))).replace(/\s+/g, " ").trim();
  const onlyLink = /^https?:\/\/\S+$/.test(lines[0]);
  return onlyLink && lines[1] ? `${said(lines[0])}: ${said(lines[1])}` : said(lines[0]);
}

/** Dollars as tazuna shows them: cents from ten cents up ($0.41, $10.13), three decimals below ($0.042), two significant
 * digits below a tenth of a cent ($0.00012) so a cheap turn does not read as free, and $0 for nothing. Until desktop 0.1.77
 * always three decimals: "$10.127" (design review 5). */
export const usd = (n: number) => (n <= 0 ? "$0" : n < 0.001 ? `$${n.toPrecision(2)}` : n < 0.1 ? `$${n.toFixed(3)}` : `$${n.toFixed(2)}`);
