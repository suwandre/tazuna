// What the window shows. Today filled from demo data; next from tazuna's app server (the same events the CLI renders).
export type Thread = { id: string; title: string; project?: string; running?: boolean;
  /** where it ran (a project's name, or "chat") and when it last did (seconds since 1970): the row's second line */
  place?: string; at?: number;
  /** the model it ran on (no longer on the row since desktop 0.1.94: in the conversation and the palette) */
  model?: string;
  /** its first message in full, where the title is shortened from it: the row's tooltip */
  full?: string };
export type Project = { name: string; path?: string; open: boolean; pending?: number; threads: Thread[] };
/** One step before the answer: a tool call (with what it was given and what came back), a stretch of thinking, or what
 * the model said between steps (`said`: its `text`, a quiet line in the fold; only prose after the last step is the answer). */
export type ToolStep = {
  verb: string; target: string; detail?: string; secs: number; ok: boolean; running?: boolean;
  /** cut off when its reply was stopped: it has no result */
  stopped?: boolean;
  /** you said no to it: neither done nor failed (its result is the sentence written for the model, not shown) */
  declined?: boolean;
  /** still open while a question to you is: it has not started (it read "Running … 3.0 s" while the card asked) */
  waiting?: boolean;
  /** the call's id: a subagent's own conversation is asked for by it */
  id?: string;
  kind?: "tool" | "thinking" | "said"; name?: string; input?: Record<string, unknown> | null; result?: string; text?: string;
};
export type ChangedFile = { path: string; added: number | null; removed: number | null; status: string };
/** What one turn changed (the core's git snapshots before and after it). */
export type Change = { turn: number; files: ChangedFile[]; undone: boolean; canUndo: boolean };
/** One model call of a reply: what it called (tool verbs, or empty for the answer alone), its tokens, timing and cost. */
/** `model`, `role`: the call's own, where the core sends them; only then is `cost` the call's price as charged (else an
 * estimate at the conversation's prices of now: not shown). `cost` null: not known (no usage reported, or cut off by a stop) */
export type Call = { tools: string; fresh: number; cached: number; out: number; cost: number | null; firstMs: number | null; tps: number | null; totalMs: number | null;
  model?: string; role?: string | null; /** `cost` is at list prices, not as charged */ estimate?: boolean };
/** How one turn was paid, as the core says it ran: on the Claude plan (not billed), or to a provider, at `perM` dollars per
 * million input, cached input and output tokens (a cache write's surcharge not in them). `providers`: a turn whose calls
 * went to more than one, in order (then no `provider` and no `perM`). Where the core does not say, there is no Billing at
 * all, and a receipt shows its total alone; a `plan` it does not send is not known, never "not on the plan". */
export type Billing = { plan?: boolean; provider?: string; perM?: [number, number, number]; providers?: string[] };
/** Who a turn paid, in words: "SubRouter", "SubRouter and OpenRouter"; undefined on the plan, where that is not known, and
 * where nobody is named. */
export const paidTo = (b?: Billing) => (b?.plan !== false ? undefined : b.provider ?? (b.providers?.length ? new Intl.ListFormat("en").format(b.providers) : undefined));
export function demoBilling() {
  // a `plan` the core did not send is not known: nobody is named as paid (absent was read as "not on the plan")
  const got = [paidTo({ plan: false, provider: "Anthropic API" }), paidTo({ provider: "SubRouter" }), paidTo({ plan: true, provider: "x" }), paidTo(undefined), paidTo({ plan: false })];
  if (got.join("|") !== "Anthropic API||||") throw new Error(`billing: ${JSON.stringify(got)}`);
  console.log("billing ok");
}
/** `in`: all of the reply's input tokens, the cached ones included, `cached` those read from the cache; `out`: its output
 * tokens (the receipt's line); `subCalls`: its subagents' model calls, in `cost` but not in `calls`. `cost` null: no amount
 * can be told; `estimate`: it is tazuna's at list prices (nobody reported one); `uncounted`: tries the provider may have
 * billed that are not in it; `unknownCalls`: calls whose cost is not known, not in it either */
export type Meta = { cost: number | null; tokens: number; in?: number; cached?: number; out?: number; cachedPct?: number; secs: number; calls?: Call[]; billing?: Billing; subCalls?: number;
  estimate?: boolean; uncounted?: number; unknownCalls?: number };
/** A share as a whole percent that never rounds up to all of it: 99.6% of the input cached is "99%", only all of it "100%". */
export const pct = (part: number, whole: number) => (part >= whole ? 100 : Math.min(99, Math.round((100 * part) / whole)));
export type Message =
  | { role: "user"; text: string; queued?: boolean; edit?: { cp: number; block: number };
      /** waiting in the window behind the reply (App's queue): "Send now" can stop the reply and send it at once */
      held?: boolean }
  | { role: "agent"; prose: string[]; tools?: ToolStep[]; change?: Change; meta?: Meta; routedChat?: boolean; ask?: string;
      /** a plain chat's answer: "Send to agent" beside Copy */
      chat?: boolean;
      /** the agent answered in the chat's place: why ("needs live data or this computer"; empty when you sent it there) */
      handedOff?: string;
      /** a reply that ended without its answer: the provider's error in words (and as it came), or stopped by you */
      failure?: Failure;
      /** errors the reply met and went on past, as the core wrote them */
      notes?: string[];
      /** a call being tried again while the reply runs, as the core says it ("…: 2 tries failed, trying again (HTTP 429…)") */
      retrying?: string;
      /** what the core says about the conversation itself, not an error: a saved one whose model cannot run here now */
      info?: string[] }
  /** what the core says between turns: a model switch that went on in a new session ("continued on X"), a quiet divider */
  | { role: "notice"; text: string };
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
  // a thinking variant (Antigravity's "claude-opus-4-6-thinking") says so: it sat beside its plain sibling as the same name
  if (c) return `${c[1][0].toUpperCase()}${c[1].slice(1).toLowerCase()} ${c[2]}${c[3] ? `.${c[3]}` : ""}${/-thinking(?:-|$)/i.test(m) ? " thinking" : ""}`;
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
  ["claude-opus-5-5", "Opus 5.5"], ["claude-haiku-4-5-20251001", "Haiku 4.5"], ["haiku", "Haiku"], ["claude-opus-4-6-thinking", "Opus 4.6 thinking"],
  ["claude-opus-4.6", "Opus 4.6"],
  ["deepseek-v4.1-flash-1017-fast", "DeepSeek V4.1 Flash 1017 Fast"], ["deepseek-v4-1-flash-1017", "DeepSeek V4.1 Flash 1017"],
  ["gpt-6.1-sol", "GPT-6.1 Sol"], ["gpt-5", "GPT-5"], ["glm-5.3-flash-ultrafast", "GLM-5.3 Flash Ultrafast"], ["mistral-small-2603", "Mistral Small 2603"],
  ["codestral-2508", "Codestral 2508"], ["grok-4.7", "Grok 4.7"], ["agy/gemini-4-hydrogen-high", "Gemini 4 Hydrogen"], ["muse-ember", "Muse Ember"],
  ["baidu/ernie-5.5-flash-preview:free", "ERNIE 5.5 Flash Preview (free)"], ["ernie-5-5-flash-preview", "ERNIE 5.5 Flash Preview"],
  ["nemotron-3.5-super-vl", "Nemotron 3.5 Super VL"], ["qwen3.8-flash-next-unsloth-ud-iq3_xxs", "Qwen3.8 Flash Next Unsloth UD IQ3_XXS"],
  ["qwen3-8-omni-flash", "Qwen3.8 Omni Flash"], ["gemma-3-4b-it", "Gemma 3 4B IT"], ["codestral-2508 off 0.3,0.03,0.9", "Codestral 2508"], ["zai-glm-5.3", "Z.ai GLM-5.3"],
];
export function demoTitles() {
  const got = [titleFrom("github.com/acme/app/pull/1476 good to approve now?"), titleFrom("https://acme.atlassian.net/browse/T-1214\nwhat is this about?"), titleFrom("acme.atlassian.net/T-1214")];
  if (got.join("|") !== "app PR #1476 good to approve now?|Jira T-1214: what is this about?|Jira T-1214") throw new Error(`titles: ${got.join("|")}`);
  const short: [string, string][] = [
    ["can you please check my laptop and see if i can save some storage somehow? e.g. caches", "Check my laptop and see if i can save some storage somehow?"],
    ["The retry test fails on CI but passes locally. Can you find out why and fix it?", "The retry test fails on CI but passes locally"],
    ["what is your model name", "What is your model name"], ["hey, could you fix the login page", "Fix the login page"],
    ["Bump the TLS library to the latest minor", "Bump the TLS library to the latest minor"], ["test", "Test"],
    ["Refactor the session store so that every write goes through one queue and the reads never block on a disk flush", "Refactor the session store so that every write goes through…"],
  ];
  for (const [from, want] of short) if (shortTitle(from) !== want) throw new Error(`shortTitle(${from}): ${shortTitle(from)}, wanted ${want}`);
  if (shortTitle("my lowercase name", true) !== "my lowercase name") throw new Error("a given name stays");
  console.log("titles ok");
}

export function demoModels() {
  const bad = MODEL_NAMES.filter(([id, want]) => modelLabel(id) !== want).map(([id, want]) => `${id}: ${modelLabel(id)} (wanted ${want})`);
  if (bad.length) throw new Error(bad.join("\n"));
  console.log("model names ok");
}

/** Whether a model row is what was typed, by what the row shows (`shown`: its name as drawn, its label, its id, its place,
 * its provider): every word typed is in it, in any case and order, where "-", "_", "/" and ":" read as a space too. The whole
 * text as one substring found nothing for "deepseek v4.1" in "deepseek-v4.1-flash-1017" rows (2026-11-21). The terminal's
 * /model has the same rule (src/main.rs, model_matches). */
export function matchesWords(typed: string, shown: string): boolean {
  const hay = shown.toLowerCase(), both = `${hay} ${hay.replace(/[-_/:]/g, " ")}`;
  return typed.toLowerCase().split(/\s+/).filter(Boolean).every((w) => both.includes(w));
}
export function demoSearch() {
  const row = `${modelLabel("deepseek-v4.1-flash-1017")} deepseek-v4.1-flash-1017 deepseek-v4.1-flash-1017 SubRouter · kriora SubRouter`;
  const yes = ["", "deepseek v4.1", "DeepSeek V4.1", "deepseek-v4.1", "flash deepseek", "v4.1 1017", "  v4.1   flash ", "subrouter flash"], no = ["deepseek v4.2", "pro deepseek", "openrouter"];
  const bad = [...yes.filter((q) => !matchesWords(q, row)), ...no.filter((q) => matchesWords(q, row)).map((q) => `${q} (matched)`)];
  if (bad.length) throw new Error(`search: ${bad.join(", ")}`);
  console.log("search ok");
}

/** A link as a few words: a GitHub pull request is "app PR #1521", an issue "repo issue #12", a repo
 * "owner/repo", a Claude artifact "Claude artifact", anything else its host and last path part. */
export function linkName(u: string): string {
  try {
    const url = new URL(u), p = url.pathname.split("/").filter(Boolean), host = url.hostname.replace(/^www\./, "");
    if (host === "github.com" && p.length >= 4 && (p[2] === "pull" || p[2] === "issues")) return `${p[1]} ${p[2] === "pull" ? "PR" : "issue"} #${p[3]}`;
    if (host === "github.com" && p.length >= 2) return `${p[0]}/${p[1]}`;
    if (/(^|\.)claude\.ai$/.test(host) && p[0] === "artifact") return "Claude artifact";
    if (host.endsWith(".atlassian.net")) { const k = u.match(/\b([A-Z][A-Z0-9]*-\d+)\b/); if (k) return `Jira ${k[1]}`; }
    return p.length ? `${host}/${decodeURIComponent(p[p.length - 1]).slice(0, 40)}` : host;
  } catch { return u; }
}

/** A conversation's name from its first message: its first line with each link said in words, and when that line was only
 * a link, the next line after it ("app PR #1485: good to approve?"). Until desktop 0.1.77 the first line as
 * typed, so a list of pasted links read "https://github.com/acme/a…" eight times (design review 5). */
export function titleFrom(text?: string | null): string {
  const lines = (text ?? "").replace(/^\[tazuna: [^\]]*\]\n\n/, "").split("\n").map((l) => l.trim()).filter(Boolean);
  if (!lines.length) return "";
  // a link pasted without its scheme ("github.com/acme/app/pull/1476") reads as one too (design review 7)
  const said = (l: string) => l.replace(/(?:https?:\/\/|\b(?=(?:www\.)?[a-z0-9-]+(?:\.[a-z0-9-]+)*\.(?:com|net|org|io|ai|dev|de)\/))[^\s<>()]+/gi,
    (u) => linkName((/^https?:/i.test(u) ? u : `https://${u}`).replace(/[.,;:!?)]+$/, ""))).replace(/\s+/g, " ").trim();
  const onlyLink = /^(https?:\/\/)?[a-z0-9.-]+\.[a-z]{2,}\/\S+$/i.test(lines[0]);
  return onlyLink && lines[1] ? `${said(lines[0])}: ${said(lines[1])}` : said(lines[0]);
}

/** A conversation's title from its first message, made short by rules on this machine (no model call): the name from
 * `titleFrom`, without a polite opening ("can you please …", "hey, …"), its first sentence, at most ~60 characters cut at a
 * word, with a capital. The sidebar listed eight rows of "what is your model name" and full sentences cut at the edge
 * (design review 9, variant A of mockups/v6.html). A name the user gave is shown as it is (`named`). */
export function shortTitle(text?: string | null, named = false): string {
  const t = titleFrom(text);
  if (named || !t) return t;
  const filler = /^(?:(?:hey|hi|hello|ok(?:ay)?|so|quick question|please|pls)\b[,:!]?\s+|(?:can|could|would|will) (?:you|u)\s+(?:please\s+)?|i (?:want|need|'d like|would like) you to\s+|help me\s+(?=\w+)(?!out\b))/i;
  let s = t;
  for (let i = 0; i < 4 && filler.test(s); i++) s = s.replace(filler, "");
  // the first sentence: up to ". ", "? " or "! " (the question mark stays, a full stop goes)
  s = s.split(/(?<=[.?!])\s+/)[0].replace(/\.$/, "").trim() || t;
  if (s.length > 60) s = `${s.slice(0, 60).replace(/\s+\S*$/, "")}…`;
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** Dollars as tazuna shows them: cents from ten cents up ($0.41, $10.13), three decimals below ($0.042), two significant
 * digits below a tenth of a cent ($0.00012) so a cheap turn does not read as free, and $0 for nothing. Until desktop 0.1.77
 * always three decimals: "$10.127" (design review 5). */
export const usd = (n: number) => (n <= 0 ? "$0" : n < 0.001 ? `$${n.toPrecision(2)}` : n < 0.1 ? `$${n.toFixed(3)}` : `$${n.toFixed(2)}`);

const onMac = typeof navigator !== "undefined" && navigator.userAgent.includes("Mac");
/** A shortcut as this machine's keyboard says it: written the Mac's way in the code ("⌘⇧U"), "Ctrl+Shift+U" on Windows
 * and Linux, where the window showed ⌘ on keyboards that have none (design review 11). */
export const keysOf = (k: string, mac = onMac) => (mac ? k : k.replace(/⌘/g, "Ctrl+").replace(/⇧/g, "Shift+").replace(/↵/g, "Enter"));
export function demoKeys() {
  const got = [keysOf("⌘⇧U", false), keysOf("⌘K", false), keysOf("⇧Tab", false), keysOf("⌘↵", false), keysOf("⌘\\", true)].join(" ");
  if (got !== "Ctrl+Shift+U Ctrl+K Shift+Tab Ctrl+Enter ⌘\\") throw new Error(`keys: ${got}`);
  console.log("keys ok");
}

/** The work panel's column, by rule (design reviews 11 and 12: it opened by itself at a turn's end and moved the reading
 * column under the reader, and Undo moved it back):
 * 1. a side chat or a subagent's sheet has the column: no panel, no tab;
 * 2. a choice made by hand in this conversation (the header's button, ⌘\, the tab) stands, in this conversation only;
 * 3. else it is never open: a project in a window of 1280 px or more has the folded tab on the sheet's edge ("Changes 2"),
 *    a plain chat and a narrower window have nothing. It never opens or closes by itself.
 * Closing it by hand is no choice: it goes back to its tab (`panelChoice`).
 * Open, it lies on the desk beside the sheet, and the reading column keeps the left edge centring gave it with the panel
 * shut (`left`: from the sheet's edge to the column's 756 px box, its 28 px sides included) and gives up width on the right
 * (700 → 614 px of text at 1440). Where that leaves under 580 px of text (a window under 1372 px with the sidebar, any
 * without it), the panel floats over the conversation instead, under its tab, and nothing moves (`over`). Design review
 * 12, B3 and B2 of mockups/v9.html: the column re-centred and its left edge jumped 146 px at 1440. */
export function panelState(a: { project: boolean; width: number; choice?: boolean; pane?: boolean }): { open: boolean; tab: boolean; over: boolean; left: number } {
  const sheet = a.width - (a.width < 1000 ? 16 : 256);   // the sheet with the panel shut: 8 px from the window's edges, the sidebar's 248 px
  const left = Math.max(0, (sheet - 756) / 2);
  const over = sheet - 300 - left - 56 < 580;              // the panel takes 300 px from the sheet's right
  const tab = a.project && a.width >= 1280;
  if (a.pane) return { open: false, tab: false, over, left };
  if (a.choice) return { open: true, tab: tab && over, over, left };
  return { open: false, tab, over, left };
}
/** The choice a click leaves for a conversation: open, or none (shut: its tab, where it has one). */
export const panelChoice = (open: boolean): boolean | undefined => (open ? undefined : true);
/** What Esc shuts, the innermost first: an open menu, popover or dialog (each shuts itself), then the work panel over the
 * conversation, and with neither it is the message box's Stop for a running reply (review of design review 12: the panel
 * went first and took Stop's Esc). */
export const escFor = (a: { menu: boolean; over: boolean }): "menu" | "panel" | "box" => (a.menu ? "menu" : a.over ? "panel" : "box");
export function demoPanel() {
  const st = (a: Parameters<typeof panelState>[0]) => { const s = panelState(a); return `${s.open ? "open" : "shut"}${s.tab ? "+tab" : ""}${s.open ? (s.over ? "+over" : `@${s.left}`) : ""}`; };
  const P = { project: true, width: 1280 }, by: Record<string, boolean | undefined> = {};
  // the tab opened in one conversation, then another opened: the choice is the first one's only
  by.a = panelChoice(false);
  const cases: [string, string, string][] = [
    ["a plain chat", st({ project: false, width: 1680 }), "shut"],
    ["a plain chat, opened by hand: beside, the column 334 px in", st({ project: false, width: 1680, choice: true }), "open@334"],
    ["a plain chat opened by hand, closed again", st({ project: false, width: 1680, choice: panelChoice(true) }), "shut"],
    ["a project, whatever it changed: folded to its tab", st(P), "shut+tab"],
    ["a project opened by its tab, 1280 px: over the conversation, under its tab", st({ ...P, choice: by.a }), "open+tab+over"],
    ["then another conversation in the project", st({ ...P, choice: by.b }), "shut+tab"],
    ["then a plain chat", st({ project: false, width: 1280, choice: by.c }), "shut"],
    ["the opened one closed again: its tab is back", st({ ...P, choice: panelChoice(true) }), "shut+tab"],
    ["a side chat or a subagent has the column", st({ ...P, choice: true, pane: true }), "shut"],
    ["1440 px: beside, the column stays 214 px in (614 px of text)", st({ project: true, width: 1440, choice: true }), "open@214"],
    ["1372 px: 580 px of text, still beside", st({ project: true, width: 1372, choice: true }), "open@180"],
    ["1371 px: over", st({ project: true, width: 1371, choice: true }), "open+tab+over"],
    ["1100 px", st({ project: true, width: 1100 }), "shut"],
    ["900 px, opened by hand: over (no sidebar)", st({ project: true, width: 900, choice: true }), "open+over"],
    ["1000 px, opened: over, no tab under 1280", st({ project: true, width: 1000, choice: true }), "open+over"],
    ["1279 px, opened: over, no tab", st({ project: true, width: 1279, choice: true }), "open+over"],
    ["1000 px: the sheet (744) is narrower than the column, left 0", `${panelState({ project: true, width: 1000 }).left}`, "0"],
    ["1011 px: still 0", `${panelState({ project: true, width: 1011 }).left}`, "0"],
    ["1014 px: 1", `${panelState({ project: true, width: 1014 }).left}`, "1"],
    ["Esc with a menu open over the panel: the menu", escFor({ menu: true, over: true }), "menu"],
    ["then Esc: the panel", escFor({ menu: false, over: true }), "panel"],
    ["then Esc: the box (Stop while a reply runs)", escFor({ menu: false, over: false }), "box"],
  ];
  const bad = cases.filter(([, got, want]) => got !== want).map(([what, got, want]) => `${what}: ${got} (wanted ${want})`);
  if (bad.length) throw new Error(bad.join("\n"));
  console.log("panel ok");
}
