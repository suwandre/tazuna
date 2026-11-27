// test data for the before/after comparison: the desktop app as it runs on tazuna's core, without the core
if (!window.__TAURI_INTERNALS__) {
Object.defineProperty(navigator, "userAgent", { get: () => "Mozilla/5.0 (X11; Linux x86_64) Chrome/140" });
// set up already, but for the sweep's first-run step (it showed the start page in its place until design review 11)
if (!sessionStorage.getItem("tz.firstRun")) localStorage.setItem("tazuna.setup", "1");
// The v2 main screen's mock: a project conversation mid-reply (steps, changes, checks, no PR yet), one waiting for an
// answer, saved chats and project conversations for the sidebar.
(() => {
window.__SENT__ = [];
const now = Math.floor(Date.now() / 1000);
const HOME = "/Users/x", PROJ = "/Users/x/api-gateway";
const u = (i, c, o) => ({ input: i, cache_write: 0, cache_read: c, output: o });
const base = { type: "view", from: 0, model: "claude-opus-5-5", effort: "high", cwd: HOME, profile: "chat", mode: "auto", cost: 0, usage: u(0, 0, 0), turns: 0,
  context: 0, context_window: 200000, changes: [], edit_points: [], blocks: [], activity: "Ready", session_id: null, limits: [36, 79], limits_reset: now + 2 * 3600 + 14 * 60, balance: 12.4 };
const tool = (id, name, input, secs, running = false, result = "ok") => ({ kind: "tool", id, name, input, result: running ? null : result, error: false, secs, running });
const fix = [
  { kind: "prompt", text: "The retry test fails on CI but passes locally. Can you find out why and fix it?" },
  tool("t1", "Read", { file_path: "src/retry.ts" }, 0.1), tool("t2", "Read", { file_path: "tests/retry.test.ts" }, 0.1),
  tool("t3", "Bash", { command: "npm test -- retry" }, 6.2, false, "1 failed"), tool("t4", "Edit", { file_path: "tests/retry.test.ts", old_string: "await sleep(100)", new_string: "vi.advanceTimersByTime(100)" }, 0.2),
  tool("t5", "Bash", { command: "npm test -- retry" }, 5.8, false, "ok"),
  { kind: "text", text: "The test waits a fixed 100 ms for the second attempt, but the backoff starts at 80 ms with up to 25% jitter. On a slow CI runner the second attempt lands after the wait, so the assertion sees one call instead of two.\n\nI made the test use the fake clock and advance it past the longest possible backoff with `vi.advanceTimersByTime(100)` instead of sleeping. It passed ten runs in a row.", live: false },
  { kind: "verify", cmd: "npm test", ok: true, secs: 12.1 },
  { kind: "done", ok: true, secs: 41, turns: 5, cost: 0.042, usage: u(2100, 12900, 1400), stopped: false },
  { kind: "prompt", text: "Run the whole suite once more, then commit it." },
  tool("t6", "Bash", { command: "git status --short" }, 0.1), tool("t7", "Bash", { command: "npm test" }, 7.6, true),
];
const views = {
  p1: { ...base, cwd: PROJ, profile: "repo", session_id: "p1", activity: "Tool", blocks: fix, cost: 0.051, turns: 6, usage: u(2600, 15100, 1700), context: 68000,
    changes: [{ turn: 0, at_block: 8, undone: false, can_undo: true, files: [{ path: "tests/retry.test.ts", added: 9, removed: 3, status: "M" }, { path: "src/retry.ts", added: 3, removed: 0, status: "M" }] }],
    calls: [{ block: 1, fresh: 2100, cached: 0, out: 300, cost: 0.02, first_ms: 900, tps: 60, total_ms: 5000 }] },
  p2: { ...base, cwd: PROJ, profile: "repo", session_id: "p2", activity: "Tool", blocks: [{ kind: "prompt", text: "Bump the TLS library to the latest minor" }, tool("u1", "Bash", { command: "npm outdated" }, 1.2),
    // the asked command's own step, open while the card asks: "Run npm install … · waits for you", no clock (0.1.100)
    { ...tool("u2", "Bash", { command: "npm install tls-lib@2.4" }, 0), running: true, result: undefined }],
    asking: { tool: "Bash", what: "npm install tls-lib@2.4", note: "", once: false, queued: 0 } },
};
const cwdOf = {};
window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
window.__TAURI_INTERNALS__ = { metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
  cbs: {}, n: 0, h: {},
  transformCallback(cb) { const id = ++this.n; this.cbs[id] = cb; return id; },
  emit(event, payload) { for (const id of this.h[event] || []) this.cbs[id]({ event, id: 1, payload: JSON.parse(JSON.stringify(payload)) }); },
  view(tid, v) { this.emit("tazuna-view", { tid, msg: v }); },
  msg(tid, m) { setTimeout(() => this.emit("tazuna-view", { tid, msg: m }), 10); },
  async invoke(cmd, args) {
    window.__SENT__.push([cmd, args]);
    if (cmd === "plugin:event|listen") { (this.h[args.event] ??= []).push(args.handler); return this.n; }
    if (cmd === "update_check") return null;
    if (cmd === "plugin:app|version") return "0.1.64";
    if (cmd === "widgets_list") return [];
    if (cmd === "start") { cwdOf[args.tid] = args.cwd ?? HOME; this.view(args.tid, { ...base, cwd: args.cwd ?? HOME, profile: args.cwd ? "repo" : "chat" }); await new Promise((r) => setTimeout(r, 20)); return args.cwd ?? HOME; }
    if (cmd !== "op") return null;
    const op = args.msg.op;
    if (op === "resume") { setTimeout(() => this.view(args.tid, views[args.msg.id] ?? { ...base, session_id: args.msg.id, activity: "Done", blocks: [{ kind: "prompt", text: "Names for a grey cat" }, { kind: "text", text: "Ash, Smoke, Pebble.", live: false }, { kind: "done", ok: true, secs: 3, turns: 1, cost: 0, usage: u(300, 0, 40), stopped: false }] }), 10); return null; }
    if (op === "sessions") { this.msg(args.tid, { type: "sessions", dirs: [
      { dir: HOME, list: [{ id: "c1", first: "Names for a grey cat", last_at: now - 3600, messages: 2, model: "agy/gemini-4-argon" }, { id: "c2", first: "Refund for a train ticket", last_at: now - 86400, messages: 6, model: "gpt-6.2-sol" }] },
      { dir: PROJ, list: [{ id: "p1", first: "Fix the flaky retry test", last_at: now - 20, messages: 4, model: "claude-opus-5-5" }, { id: "p2", first: "Bump the TLS library to the latest minor", last_at: now - 120, messages: 2, model: "deepseek-v4.1-flash" }, { id: "p3", first: "Speed up the CI build", last_at: now - 3 * 3600, messages: 9, model: "mistral-large-3" }] }] }); return null; }
    if (op === "pr") { this.msg(args.tid, { type: "pr", none: true, branch: "fix/retry-backoff" }); return null; }
    if (op === "models") { const c = (o) => ({ efforts: ["low", "medium", "high", "max"], effort: "high", bench: "", why_not: null, key_env: null, url: null, current: false, ...o });
      this.msg(args.tid, { type: "models", rebuild: null, choices: [c({ model: "claude-opus-5-5", label: "Opus 5.5", place: "Claude · subscription", provider: "Claude", price: "plan", current: true })] }); return null; }
    if (op === "usage") { this.msg(args.tid, { type: "usage", rows: [] }); return null; }
    return null;
  },
};
localStorage.setItem("tazuna.projects", JSON.stringify([PROJ]));
})();

(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T);
  // resets as claude writes them, from now: the 5 hours in 2 h 14 min, the week in 1 day 2 hours
  const resets = (ms) => { const d = new Date(Date.now() + ms), h = d.getHours(); return `${d.toLocaleString("en-US", { month: "short" })} ${d.getDate()} at ${h % 12 || 12}:${String(d.getMinutes()).padStart(2, "0")}${h < 12 ? "am" : "pm"} (Europe/Berlin)`; };
  window.__ACCOUNTS__ = { plan: [{ label: "Current session", used: 36, resets: resets(134 * 60_000) }, { label: "Current week (all models)", used: 79, resets: resets(26 * 3600_000) }],
    plan_notes: ["Approximate · sessions on this machine only", "Last 24h · 865 requests · 7 sessions", "  98%  Review sessions active over many hours · whole sessions with requests in 8 or more separate hours", "  94%  Start fresh when you switch tasks · run /clear", "  13%  Compact before you step away · run /compact", "  Top skills: /ticket 6%",
      "Last 7d · 5894 requests · 60 sessions", "  98%  Check which models your subagents use · share covers whole sessions, not only subagents", "  91%  Review sessions active over many hours · whole sessions with requests in 8 or more separate hours", "  88%  Start fresh when you switch tasks · run /clear", "  11%  Compact before you step away · run /compact", "  Top skills: /code-review 3%, /ticket 1%, /find-docs 1%", "  Top subagents: general-purpose 2%, explorer 1%", "  Top MCP servers: playwright 1%, docs 1%"],
    credits: [{ provider: "SubRouter", left: "$12.40 left", page: "https://subrouter.ai" }, { provider: "Mortera", left: null, page: "https://mortera.cloud" },
      // Qwen's CLI signed in, its usage without the plan (core 0.1.233): a quiet row, nothing claimed
      { provider: "Qwen Cloud Token Plan", left: null, short: null, state: "unknown", cli_plan: false, page: "https://home.qwencloud.com" }],
    omniroute: [{ provider: "agy", name: "someone@example.com", plan: "Ultra", status: "valid", windows: [{ group: "Gemini Models", window: "weekly", used: 12, resets: "2026-11-25T11:03:10.000Z", unlimited: false }] }] };
  const day = (i) => Math.floor(Date.now() / 1000) - i * 86400;
  window.__ROWS__ = Array.from({ length: 60 }, (_, i) => [day(i % 20) , i % 4 ? "claude-opus-5-5" : "deepseek-v4.1-flash", 0.02 + (i % 7) * 0.01, i % 4 !== 0]);
  T.invoke = async (cmd, args) => {
    if (cmd === "op" && args.msg.op === "accounts") { setTimeout(() => T.emit("tazuna-view", { tid: args.tid, msg: { type: "accounts", ...window.__ACCOUNTS__ } }), 300); return null; }
    return inv(cmd, args);
  };
})();

(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T), now = Math.floor(Date.now() / 1000);
  // made when first asked: the sweep's knobs are set after this script
  let rows = null;
  const build = () => { rows = [];
  for (let d = 0; d < 30; d++) for (let k = 0; k < (d * 7) % 5 + 1; k++) {
    const plan = (d + k) % 6 !== 0, model = plan ? (k % 3 ? "claude-opus-5-5" : "claude-sonnet-5-5") : "deepseek-v4.1-flash";
    // __BIG__ (the sweep): a heavy month, $12,345.67 paid and billions of tokens from the cache, for figures that must not be cut
    if (window.__BIG__) { rows.push([now - d * 86400 - k * 600, model, "0.1.176", plan, plan ? 12 + ((d * k) % 7) * 2 : 800 + (k % 3) * 20, 2000, 500, 18_000_000, 1200, null, null, null, "auto", true, [], [], plan ? null : "SubRouter"]); continue; }
    rows.push([now - d * 86400 - k * 600, model, d < 10 ? "0.1.176" : "0.1.170", plan, plan ? 0.3 + ((d * k) % 7) * 0.12 : 0.02 + (k % 3) * 0.01, 2000, 500, 40000, 1200, null, null, null, "auto", true, (d + k) % 9 === 0 ? ["stopped"] : [], (d + k) % 7 === 0 ? ["why"] : []]);
  }
  return rows; };
  T.invoke = async (cmd, args) => {
    if (cmd === "op" && args.msg.op === "usage") { const r = rows ?? build(); setTimeout(() => T.emit("tazuna-view", { tid: args.tid, msg: { type: "usage", rows: r } }), 20); return null; }
    return inv(cmd, args);
  };
})();

// Settings' providers as the core lists them since 0.1.196 (models.rs LOGINS): three with a key, a model server running
// here, the rest without one; their terms, in the warning colour where the account may be at risk or tazuna refused
const KEYS = { SubRouter: "managed key (tazuna subrouter --manage)", DeepSeek: "key in the environment", Groq: "logged in", "Mistral Vibe": "logged in",
  "Cheaper Inference": "logged in", "Qwen Cloud Token Plan": "logged in" };
const PROVIDERS = [["SubRouter", "subrouter.ai", "sk-..."], ["Mortera", "mortera.cloud"], ["OpenRouter", "openrouter.ai", "sk-or-v1-..."], ["Kenari", "kenari.id"],
  ["Mistral", "api.mistral.ai"], ["Mistral Vibe", "api.mistral.ai"], ["Cheaper Inference", "api.cheaperinference.com"], ["OmniRoute", "localhost:20128"], ["DeepSeek", "api.deepseek.com", "sk-..."],
  ["Moonshot", "api.moonshot.ai", "sk-..."], ["Kimi Code", "api.kimi.ai", "", "Kimi may refuse tazuna as a client: check before buying the plan"],
  ["Kimi Code China", "api.kimi.com", "", "Kimi may refuse tazuna as a client: check before buying the plan"], ["Z.ai", "api.z.ai"],
  ["Z.ai Coding Plan", "api.z.ai", "", "Z.ai allows the Coding Plan only in its listed tools and tazuna is not listed: the account may be at risk"],
  ["Zhipu Coding Plan", "open.bigmodel.cn", "", "Zhipu allows the Coding Plan only in its listed tools and tazuna is not listed: the account may be at risk"],
  ["MiniMax", "api.minimax.io"], ["MiniMax China", "api.minimax.cn"], ["MiniMax Plan", "api.minimax.io"], ["MiniMax Plan China", "api.minimax.cn"],
  ["Alibaba Model Studio", "dashscope-intl.aliyuncs.com", "sk-..."], ["Alibaba Model Studio China", "dashscope.aliyuncs.com", "sk-..."],
  ["Alibaba Coding Plan", "coding-intl.dashscope.aliyuncs.com", "sk-sp-...", "Alibaba allows the Coding Plan only for interactive use in coding tools, never scripts or benchmarks; it may refuse tazuna"],
  ["Alibaba Coding Plan China", "coding.dashscope.aliyuncs.com", "sk-sp-...", "Alibaba allows the Coding Plan only for interactive use in coding tools, never scripts or benchmarks; it may refuse tazuna"],
  ["Alibaba Token Plan", "token-plan.ap-southeast-1.maas.aliyuncs.com", "sk-sp-...", "Alibaba allows the Token Plan only for interactive use, not scripts or app backends: the key may be suspended"],
  ["xAI", "api.x.ai", "xai-..."], ["OpenAI", "api.openai.com", "sk-..."], ["Google Gemini", "generativelanguage.googleapis.com", "AIza..."], ["Meta", "api.meta.ai"],
  ["Xiaomi MiMo", "api.xiaomimimo.com"], ["Xiaomi Token Plan", "token-plan-sgp.xiaomimimo.com", "tp-...", "Xiaomi allows the Token Plan only in coding tools, not scripts or backends: the key may be blocked"],
  ["Xiaomi Token Plan Europe", "token-plan-ams.xiaomimimo.com", "tp-...", "Xiaomi allows the Token Plan only in coding tools, not scripts or backends: the key may be blocked"],
  ["StepFun", "api.stepfun.ai"], ["Inception", "api.inceptionlabs.ai"], ["Sakana AI", "api.sakana.ai"], ["Groq", "api.groq.com", "gsk_..."], ["Cerebras", "api.cerebras.ai", "csk-..."],
  ["SambaNova", "api.sambanova.ai"], ["Together", "api.together.ai"], ["Fireworks", "api.fireworks.ai", "fw_..."],
  ["Fire Pass", "api.fireworks.ai", "fpk_...", "Fireworks allows Fire Pass for personal coding with agent tools, not production work: the pass may be revoked"],
  ["DeepInfra", "api.deepinfra.com"], ["Novita", "api.novita.ai"], ["SiliconFlow", "api.siliconflow.com", "sk-..."], ["SiliconFlow China", "api.siliconflow.cn", "sk-..."],
  ["Chutes", "llm.chutes.ai", "cpk_...", "Chutes' terms: interactive and low-volume use, not large automated runs"], ["Baseten", "inference.baseten.co"],
  ["Hugging Face", "router.huggingface.co", "hf_..."], ["NVIDIA", "integrate.api.nvidia.com", "nvapi-..."], ["Scaleway", "api.scaleway.ai"], ["Modular", "api.modular.com"],
  ["GMI Cloud", "api.gmi-serving.com"], ["ai&", "api.aiand.com"], ["CoralBricks", "inference.coralbricks.ai", "cb_..."], ["Abliteration", "api.abliteration.ai", "ak_..."],
  ["Venice", "api.venice.ai"], ["Wafer", "pass.wafer.ai", "wfr_..."], ["Ollama Cloud", "ollama.com"], ["Synthetic", "api.synthetic.new"], ["Umans Coding Plan", "api.code.umans.ai", "sk-..."],
  ["Vercel AI Gateway", "ai-gateway.vercel.sh", "vck_..."], ["NanoGPT", "nano-gpt.com"], ["AIML API", "api.aimlapi.com"], ["ZenMux", "zenmux.ai", "sk-..."], ["Kilo Gateway", "api.kilo.ai"],
  ["OpenCode Zen", "opencode.ai", "sk-...", "OpenCode's terms keep Zen's free models for OpenCode itself; its paid models work from other tools"],
  ["OpenCode Go", "opencode.ai", "sk-...", "OpenCode Go asks coding agents for a session header tazuna does not send; OpenCode watches its traffic for abuse"],
  ["ClinePass", "api.cline.bot", "sk_..."], ["Command Code", "api.commandcode.ai", "user_..."], ["Charm Hyper", "hyper.charm.land", "sk-hyper-..."], ["Qianfan", "qianfan.baidubce.com", "bce-v3/ALTAK-..."],
  ["Helmcode", "api.helmcode.com", "sk-..."], ["Otari", "api.otari.ai", "tk_..."], ["SingularityAPI", "api.singularityapi.dev", "sk-sapi-..."],
  ["Qwen Cloud Token Plan", "token-plan.maas.qwencloudapi.com", "sk-sp-..."], ["Caffeine", "inference.caffeine.ai"]]
  .map(([name, host, example = "", note = ""]) => ({ name, host, example, note, warn: /at risk|refuse|suspended|blocked|revoked/.test(note), key: KEYS[name] ?? "", trusted: !!KEYS[name],
    page: `https://${host}`, place: `${name}'s console (API keys)` }))
  // sign-ins (core oauth.rs, 2026-11-22): ChatGPT's own flow, four that send another app's identity behind a warning
  .concat([["ChatGPT", "api.openai.com", "chatgpt", "OpenAI's own sign-in for open-source apps: requests use your ChatGPT plan (manage usage in ChatGPT's settings)"],
    ["GitHub Copilot", "api.githubcopilot.com", "copilot", "tazuna signs in as OpenCode's GitHub app and calls Copilot as Copilot CLI. Whether GitHub's terms allow this, or GitHub acts on accounts for it, is unclear. Use at your own risk."],
    ["SuperGrok", "api.x.ai", "xai", "tazuna signs in as xAI's Grok CLI. Whether xAI's terms allow this, or xAI acts on accounts for it, is unclear. Use at your own risk."],
    ["Gemini CLI", "cloudcode-pa.googleapis.com", "gemini", "Google's terms forbid using this login outside Gemini CLI, and Google may suspend or end the account for it. tazuna signs in as Gemini CLI. Use at your own risk."],
    ["Antigravity", "daily-cloudcode-pa.googleapis.com", "antigravity", "Google's terms forbid signing in to Antigravity from other tools, and Google has suspended accounts that did. tazuna signs in as Antigravity: you do this at your own risk."]]
    .map(([name, host, sign_in, note]) => ({ name, host, example: "", note, warn: /own risk/.test(note), key: "", trusted: false, page: `https://${host}`, place: `${name}'s settings`, sign_in, confirmed: false })))
  .concat([{ name: "Ollama", host: "localhost:11434", example: "", note: "", warn: false, key: "running on this machine", trusted: true, page: "", place: "", local: true }]);

// Usage sources (core usage_sources, review 14 and its fix) as the core builds them: the API where one tells usage, Cheaper
// Inference's with account:read, Qwen Cloud's CLI and console, Mistral Vibe's admin key (never shown) and console, Caffeine's
// console; every provider can be counted here; Caffeine without a key, read by its console all the same; each reply
// numbered (`seq`) and saying its request's `req` back, a change answered with the sources and the credits again
// (accounts_credits); the console reader's commands (main.rs). Knobs for the sweep: __CLI__ (Qwen's CLI: missing,
// signed_out, no_plan, ready), __CONSOLE_READY__ (Qwen's console has a reading), __DELAY__ (ms before a usage reply; a
// function of the op for one slower than another), __OPEN_DELAY__ and __FORGET_DELAY__ (the console's commands), __VISIT__
// (what a console visit finds: {page, fields}; nothing by default)
(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T);
  const M = (id, ready, why = null, steps = "", link = "", more = {}) => ({ id, ready, why, steps, link, ...more });
  const local = M("local", true, null, "tazuna counts its own use against an allowance you enter (amount, unit, period). Use through other apps is not counted.");
  const api = M("api", true, null, "tazuna reads it from the provider's API with your key, at most once a minute. Nothing to do once the key is saved.");
  const cliSteps = "Install Qwen's CLI (npm install -g @qwencloud/qwencloud-cli) and sign in with the Token Plan's account (qwencloud auth login); tazuna reads qwencloud usage summary.";
  const CLI = { missing: ["Not installed", ["npm install -g @qwencloud/qwencloud-cli", "qwencloud auth login"]], signed_out: ["Not signed in", ["qwencloud auth login"]],
    no_plan: ["CLI doesn't show this plan", [], "console"], ready: [null, []] };
  const cli = () => { const [why, commands, instead] = CLI[window.__CLI__ || "missing"]; return M("cli", !why, why, cliSteps, "https://github.com/QwenCloud/qwencloud-cli", { commands, ...(instead ? { instead } : {}) }); };
  const consoleM = (name, ready = false) => M("console", ready, ready ? null : "No reading yet", `Sign in to ${name}'s console once in tazuna's own window (Sign in…); tazuna reads its usage page there, at most every 15 minutes.`);
  const SOURCES = window.__SOURCES__ = [
    { provider: "SubRouter", has_key: true, methods: [api, local], track: { on: true, method: "api" }, allowance: null },
    { provider: "Mistral Vibe", has_key: true, methods: [M("admin_key", false, "tazuna does not read Admin keys", "Mistral Vibe's API tells only this month's spend, and only to an Admin key.", "https://admin.mistral.ai"),
      consoleM("Mistral Vibe"), local], track: { on: true, method: "local" }, allowance: { amount: 150, unit: "usd", period: "month", reset: 1 } },
    { provider: "Cheaper Inference", has_key: true, methods: [M("api_scope", false, "Needs account:read", "tazuna reads it from Cheaper Inference's API once the key may read the account: turn on account:read for this key on its dashboard, then check again.", "https://platform.cheaperinference.com/keys"), local], track: { on: true, method: "api_scope" }, allowance: null },
    { provider: "DeepSeek", has_key: true, methods: [api, local], track: { on: true, method: "api" }, allowance: null },
    { provider: "Groq", has_key: true, methods: [local, M("none", false, "No usage API", "Groq offers no way to read its usage: see its page.", "https://console.groq.com/settings/billing")], track: { on: false, method: "local" }, allowance: null },
    { provider: "Qwen Cloud Token Plan", has_key: true, methods: [null, null, local], track: { on: true, method: "console" }, allowance: null },
    { provider: "Caffeine", has_key: false, methods: [consoleM("Caffeine"), local, M("none", false, "No usage API", "", "https://caffeine.ai/inference/usage")], track: { on: false, method: "console" }, allowance: null },
  ];
  // the sources as the core reads them when the op comes: Qwen's CLI and console by the knobs
  const snapshot = () => JSON.parse(JSON.stringify(SOURCES.map((x) => (x.provider === "Qwen Cloud Token Plan" ? { ...x, methods: [cli(), consoleM("Qwen Cloud", !!window.__CONSOLE_READY__), local] } : x))));
  let SEQ = 0;
  const LEARNED = window.__LEARNED__ = { "Qwen Cloud Token Plan": { page: "/usage", fields: 3, at: Math.floor(Date.now() / 1000) - 600, store: true } };
  const wait = (ms) => new Promise((r) => setTimeout(r, ms || 0));
  T.invoke = async (cmd, args) => {
    const op = cmd === "op" ? args.msg.op : null, src = op && SOURCES.find((x) => x.provider === args.msg.provider);
    const d = window.__DELAY__, ms = typeof d === "function" ? d(args.msg) : d ?? 20;
    const m = (msg) => setTimeout(() => T.emit("tazuna-view", { tid: args.tid, msg }), ms);
    const seq = ++SEQ, req = args?.msg?.req, sources = (error) => m({ type: "usage_sources", seq, ...(req != null ? { req } : {}), providers: snapshot(), ...(error ? { error } : {}) });
    const credits = () => { const off = SOURCES.filter((x) => !x.track.on).map((x) => x.provider);
      m({ type: "accounts_credits", seq, credits: window.__ACCOUNTS__.credits.filter((c) => !off.includes(c.provider)), untracked: off }); };
    if (/^usage_(sources|track|allowance)$/.test(op) || cmd.startsWith("console_")) window.__SENT__.push([cmd, args]);
    if (op === "usage_sources") { sources(); return null; }
    // a refusal comes back as the sources unchanged, with its error (Caffeine counted here: a settings file it cannot write)
    if (op === "usage_track" && src && src.provider === "Caffeine" && args.msg.method === "local") { sources("Not saved: ~/.config/tazuna/usage.json is read-only"); return null; }
    if (op === "usage_track" && src) { src.track = { on: args.msg.on, method: args.msg.method }; sources(); credits(); return null; }
    if (op === "usage_allowance" && src) { src.allowance = args.msg.allowance; sources(); credits(); return null; }
    if (cmd === "console_learned") return LEARNED[args.provider] ?? null;
    // the window can't open here: closed a moment later, with what the visit found (nothing by default)
    if (cmd === "console_open") { await wait(window.__OPEN_DELAY__);
      const v = window.__VISIT__, was = LEARNED[args.provider];
      LEARNED[args.provider] = { page: v?.page ?? was?.page ?? null, fields: v?.fields ?? was?.fields ?? 0, at: Math.floor(Date.now() / 1000), store: true };
      setTimeout(() => T.emit("console-closed", args.provider), 300); return null; }
    // the window's store goes; the learn file (shapes only) stays, as main.rs keeps it
    if (cmd === "console_forget") { await wait(window.__FORGET_DELAY__); if (LEARNED[args.provider]) LEARNED[args.provider].store = false; return null; }
    if (cmd === "console_refresh") return "skipped";
    return inv(cmd, args);
  };
})();

(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T);
  const diff = "diff --git a/tests/retry.test.ts b/tests/retry.test.ts\n--- a/tests/retry.test.ts\n+++ b/tests/retry.test.ts\n@@ -12,9 +12,15 @@ describe(\"retry\", () => {\n   it(\"retries once after a failure\", async () => {\n-    const fn = vi.fn().mockRejectedValueOnce(new Error(\"boom\"));\n-    await retry(fn);\n-    await sleep(100);\n+    vi.useFakeTimers();\n+    const fn = vi.fn().mockRejectedValueOnce(new Error(\"boom\")).mockResolvedValue(\"ok\");\n+    const p = retry(fn);\n+    // the backoff is 80 ms with up to 25% jitter: past its longest\n+    await vi.advanceTimersByTimeAsync(100);\n+    await p;\n     expect(fn).toHaveBeenCalledTimes(2);\n+    vi.useRealTimers();\n   });\n";
  T.invoke = async (cmd, args) => {
    const m = (msg) => setTimeout(() => T.emit("tazuna-view", { tid: args.tid, msg }), 20);
    if (cmd === "op" && args.msg.op === "diff") { m({ type: "diff", turn: args.msg.turn, path: args.msg.path, text: diff }); return null; }
    if (cmd === "op" && args.msg.op === "review") { m({ type: "review", scope: args.msg.scope, turn: args.msg.turn ?? null, base: "main", text: diff }); return null; }
    if (cmd === "op" && args.msg.op === "settings") { m({ type: "settings", version: "0.1.176", data_any: false, route: true, spend_cap: 5, spent_24h: 0.41, providers: PROVIDERS }); return null; }
    // a sign-in: ChatGPT through the browser, then its welcome; Copilot and SuperGrok show their code and wait there
    if (cmd === "op" && args.msg.op === "login" && PROVIDERS.find((x) => x.name === args.msg.provider)?.sign_in) {
      const p = PROVIDERS.find((x) => x.name === args.msg.provider), say = (o, ms = 20) => setTimeout(() => T.emit("tazuna-view", { tid: args.tid, msg: { type: "signin", provider: p.name, ok: null, first: false, ...o } }), ms);
      if (args.msg.sign_out) { p.key = ""; say({ ok: true, text: `signed out of ${p.name}` }); return null; }
      if (p.warn && !p.confirmed && !args.msg.confirm) { say({ ok: false, text: `${p.name}: confirm its warning first` }); return null; }
      if (args.msg.confirm) p.confirmed = true;
      // Google's two: the browser, the account's Code Assist project, then signed in (the window's words: no "signed in" again)
      if (p.sign_in === "gemini" || p.sign_in === "antigravity") {
        say({ text: `${p.name}: sign in with Google in the browser tab that just opened (5 minutes)` });
        say({ text: `${p.name}: signed in; finding the account's Code Assist project` }, 400);
        setTimeout(() => { p.key = "signed in"; p.trusted = true; }, 690);
        say({ ok: true, text: `someone@example.com: ${p.sign_in === "gemini" ? "2 models in the model picker; they use your Gemini Code Assist quota" : "4 models in the model picker; they use your Antigravity plan's quota"}` }, 700);
        return null;
      }
      if (p.sign_in !== "chatgpt") { say({ text: `${p.name}: enter the code ABCD-1234 at https://github.com/login/device (the page just opened in your browser; the code is on the clipboard)` }); return null; }
      say({ text: "ChatGPT: continue in the browser tab that just opened (5 minutes)" });
      setTimeout(() => { p.key = "signed in"; p.trusted = true; }, 690);
      say({ ok: true, first: true, text: "someone@example.com: requests on its models use your ChatGPT plan (3 models in the model picker); manage usage at https://chatgpt.com/settings/usage" }, 700);
      return null;
    }
    // a key saved, kept or removed (core 0.1.198): how it went, as a line on its row
    if (cmd === "op" && args.msg.op === "login") {
      const p = PROVIDERS.find((x) => x.name === args.msg.provider);
      if (p && args.msg.remove) { p.key = ""; p.unsure = null; m({ type: "signin", provider: p.name, ok: true, text: "the key from before is removed", first: false }); return null; }
      if (p && (args.msg.key || p.key)) { if (args.msg.key) p.key = "logged in"; const was = p.unsure; for (const x of PROVIDERS) if (x.unsure) x.unsure = null;
        m({ type: "signin", provider: p.name, ok: true, first: false, text: was ? `the key saved for both is ${p.name}'s now; its models are in the model picker` : `logged in to ${p.name}: key saved in /Users/x/.config/tazuna/keys.json (readable by you only); its models are in the model picker` }); return null; }
      if (p) { m({ type: "signin", provider: p.name, ok: false, text: "no key typed", first: false }); return null; }
    }
    if (cmd === "op" && args.msg.op === "mcp") { m({ type: "mcp", on: true, claude: true, servers: [{ name: "github", target: "https://api.githubcopilot.com/mcp/" }], note: "" }); return null; }
    if (cmd === "op" && args.msg.op === "hook") { m({ type: "hook", cwd: "/Users/x/api-gateway", after_turn: "npm run lint -- --fix", note: "" }); return null; }
    if (cmd === "op" && args.msg.op === "setup") { m({ type: "setup", claude: "2.1.330 (Claude Code)" }); return null; }
    if (cmd === "op" && args.msg.op === "fork") { (T.sides ??= new Set()).add(args.tid); setTimeout(() => T.emit("tazuna-view", { tid: args.tid, msg: { type: "view", from: 0, model: "claude-opus-5-5", effort: "high", cwd: "/Users/x/api-gateway", profile: "repo", mode: "plan", cost: 0.004, usage: { input: 1, cache_write: 0, cache_read: 9, output: 2 }, turns: 1, context: 9000, context_window: 200000, changes: [], edit_points: [], activity: "Done", session_id: "f1",
      blocks: [{ kind: "prompt", text: "Why 100 ms and not 120?" }, { kind: "text", text: "The longest backoff is 80 ms + 25% = 100 ms, so advancing exactly 100 ms covers every jitter draw. 120 would also pass but hides the bound.", live: false }, { kind: "done", ok: true, secs: 4, turns: 1, cost: 0.004, usage: { input: 1, cache_write: 0, cache_read: 9, output: 2 }, stopped: false }] } }), 30); return null; }
    return inv(cmd, args);
  };
})();

// richer data for the A-Z design review: many models, a markdown-heavy chat, a failed run with a raw provider error,
// a long conversation, a thinking block
(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T), now = Math.floor(Date.now() / 1000);
  const u = (i, c, o) => ({ input: i, cache_write: 0, cache_read: c, output: o });
  const base = { type: "view", from: 0, model: "claude-opus-5-5", effort: "high", cwd: "/Users/x", profile: "chat", mode: "auto", cost: 0, usage: u(0, 0, 0), turns: 0,
    context: 0, context_window: 200000, changes: [], edit_points: [], blocks: [], activity: "Done", limits: [36, 79], limits_reset: now + 8040, balance: 12.4 };
  const done = (secs, cost, ok = true, stopped = false) => ({ kind: "done", ok, secs, turns: 1, cost, usage: u(900, 4000, 300), stopped });
  const md = "## Why the build is slow\n\nThree things dominate, measured on the last 20 CI runs:\n\n| Step | Median | Share |\n|---|---:|---:|\n| `npm ci` | 94 s | 41% |\n| Type check | 61 s | 27% |\n| Tests | 48 s | 21% |\n\n1. **Cache the npm store**, keyed on `package-lock.json`:\n\n```yaml\n- uses: actions/cache@v4\n  with:\n    path: ~/.npm\n    key: npm-${{ hashFiles('package-lock.json') }}\n```\n\n2. **Run the type check in parallel** with the tests: they share nothing.\n3. Split the slowest test file (`gateway.e2e.test.ts`, 31 s).\n\n> Expect roughly 230 s → 120 s. The cache alone saves about a minute on a warm runner.\n\nThe math: total $T = \\sum_i t_i$, with the cache term dropping from 94 to ~15 s. See [the docs](https://docs.github.com/actions).";
  const views = {
    c3: { ...base, session_id: "c3", cwd: "/Users/x/api-gateway", profile: "repo", cost: 0.031, turns: 1, usage: u(4000, 22000, 900), context: 26000,
      blocks: [{ kind: "prompt", text: "Why is the CI build so slow? Give me the top three fixes." }, { kind: "thinking", text: "Look at the workflow file and the timings first.", secs: 4 },
        { kind: "tool", id: "a1", name: "Read", input: { file_path: ".github/workflows/ci.yml" }, result: "…", error: false, secs: 0.1, running: false },
        { kind: "tool", id: "a2", name: "Grep", input: { pattern: "describe\\(", path: "tests" }, result: "…", error: false, secs: 0.2, running: false },
        { kind: "tool", id: "a3", name: "Bash", input: { command: "gh run list --limit 20 --json durationMs" }, result: "…", error: false, secs: 2.4, running: false },
        { kind: "text", text: md, live: false }, done(38, 0.031)] },
    c4: { ...base, session_id: "c4", model: "agy/gemini-4-hydrogen", cost: 0, turns: 0,
      blocks: [{ kind: "prompt", text: "test" }, { kind: "notice", error: true, text: "provider · provider error in stream: {\"message\":\"[404]: Antigravity upstream error (404): Requested entity was not found.\",\"type\":\"invalid_request_error\",\"code\":\"model_not_found\"} · trying again in 2s" }, done(4, 0, false, true)] },
    c5: { ...base, session_id: "c5", cost: 0.12, turns: 8, blocks: Array.from({ length: 8 }, (_, i) => [{ kind: "prompt", text: i === 3 ? "Can you write the whole thing again but this time with the error handling we discussed, the retries capped at three, the logging through our logger and not console, and the tests updated so they cover the timeout path as well as the happy path? Also keep the public API the same." : `Question ${i + 1}: what about edge case ${i + 1}?` }, { kind: "text", text: `Edge case ${i + 1} is covered by the guard in \`parse()\`: an empty input returns \`null\` before the loop. ` .repeat(3), live: false }, done(6 + i, 0.015)]).flat() },
  };
  // the model picked (op "model") stays picked: its row comes back `current`
  let picked = null;
  T.invoke = async (cmd, args) => {
    const m = (msg) => setTimeout(() => T.emit("tazuna-view", { tid: args.tid, msg }), 20);
    if (cmd === "op" && args.msg.op === "resume" && views[args.msg.id]) { setTimeout(() => T.view(args.tid, views[args.msg.id]), 10); return null; }
    if (cmd === "op" && args.msg.op === "model") { picked = args.msg; return T.invoke("op", { tid: args.tid, msg: { op: "models" } }); }
    if (cmd === "op" && args.msg.op === "models") { const c = (o) => { const r = { efforts: ["low", "medium", "high", "max"], effort: "high", bench: "", why_not: null, key_env: null, url: null, current: false, ...o };
      return picked ? { ...r, current: r.model === picked.model && r.url === (picked.url ?? null) && r.key_env === (picked.key_env ?? null) } : r; };
      const signed = (name) => PROVIDERS.find((x) => x.name === name).key;
      m({ type: "models", rebuild: null, choices: [
        c({ model: "claude-opus-5-5", label: "Opus 5.5", place: "Claude · subscription", provider: "Claude", price: "plan", current: true }),
        c({ model: "claude-sonnet-5-5", label: "Sonnet 5.5", place: "Claude · subscription", provider: "Claude", price: "plan" }),
        c({ model: "claude-fable-5-2", label: "Fable 5.2", place: "Claude · subscription", provider: "Claude", price: "plan" }),
        c({ model: "claude-haiku-5-5", label: "Haiku 5.5", place: "Claude · subscription", provider: "Claude", price: "plan" }),
        c({ model: "gpt-6.2-sol", label: "gpt-6.2-sol · subrouter", place: "subrouter.ai", provider: "SubRouter", price: "1.25 / 10", bench: "8.4 (12)", efforts: ["low", "medium", "high"] }),
        c({ model: "deepseek-v4.1-flash", label: "deepseek-v4.1-flash · subrouter", place: "subrouter.ai", provider: "SubRouter", price: "0.14 / 0.56", bench: "7.1 (16)", efforts: ["off", "high", "max"] }),
        c({ model: "codestral-2508", label: "codestral-2508 · mistral", place: "api.mistral.ai", provider: "Mistral", price: "0.3 / 0.9" }),
        c({ model: "mistral-large-3", label: "mistral-large-3 · mistral", place: "api.mistral.ai", provider: "Mistral", price: "2 / 6", bench: "7.8 (12)" }),
        // the same models on a Vibe CLI key (core 0.1.198): the Vibe plan pays
        c({ model: "codestral-2508", label: "codestral-2508 · mistral", place: "api.mistral.ai", provider: "Mistral Vibe", price: "plan", url: "https://api.mistral.ai/v1", key_env: "MISTRAL_VIBE_API_KEY" }),
        c({ model: "mistral-large-3", label: "mistral-large-3 · mistral", place: "api.mistral.ai", provider: "Mistral Vibe", price: "plan", url: "https://api.mistral.ai/v1", key_env: "MISTRAL_VIBE_API_KEY", bench: "7.8 (12)" }),
        c({ model: "agy/gemini-4-hydrogen", label: "gemini-4-hydrogen · agy", place: "OmniRoute · agy", provider: "OmniRoute", price: "plan" }),
        c({ model: "qwen3.8-max", label: "qwen3.8-max · qwen cloud", place: "token-plan.maas.qwencloudapi.com", provider: "Qwen Cloud Token Plan", price: "plan", url: "https://token-plan.maas.qwencloudapi.com/compatible-mode/v1", key_env: "QWENCLOUD_TOKEN_PLAN_API_KEY" }),
        // rows that cannot be picked come compact (core 0.1.196): no key yet, or a host not trusted
        { model: "or/qwen-4-coder", label: "qwen-4-coder · openrouter", provider: "OpenRouter", price: "0.2 / 0.8", why_not: "no key: /login, or set the environment variable OPENROUTER_API_KEY before starting tazuna", no_key: true },
        { model: "gpt-oss-120b", provider: "Cerebras", price: "0.35 / 0.75", bench: "6.9 (8)", why_not: "no key: /login, or set the environment variable CEREBRAS_API_KEY before starting tazuna", no_key: true },
        { model: "qwen-4-coder-480b", provider: "Cerebras", price: "2 / 2", why_not: "no key: /login, or set the environment variable CEREBRAS_API_KEY before starting tazuna", no_key: true },
        { model: "glm-5.3", provider: "Z.ai Coding Plan", price: "plan", why_not: "no key: /login, or set the environment variable ZAI_CODING_API_KEY before starting tazuna", no_key: true },
        { model: "zai-org/glm-5.3", provider: "Chutes", price: "0.2 / 0.8", why_not: "not trusted: the code would go to llm.chutes.ai; /login allows it, or add the line llm.chutes.ai to /Users/x/.config/tazuna/trusted-providers" },
        // sign-ins: ChatGPT's models once signed in (the plan pays), a greyed row before; Copilot not signed in
        ...(signed("ChatGPT")
          ? [["gpt-6.2-sol", "GPT-6.2 Sol"], ["gpt-6.1-sol", "GPT-6.1 Sol"], ["gpt-6-luna", "GPT-6 Luna"]].map(([model, label]) => c({ model, label, place: "ChatGPT", provider: "ChatGPT", price: "plan", url: "https://api.openai.com/v1", key_env: "TAZUNA_SIGNIN_CHATGPT", efforts: ["low", "medium", "high", "xhigh", "max"] }))
          : [{ model: "chatgpt-models", label: "ChatGPT's models", provider: "ChatGPT", price: "plan", why_not: "not signed in", no_key: true }]),
        { model: "copilot-models", label: "GitHub Copilot's models", provider: "GitHub Copilot", price: "plan", why_not: "not signed in", no_key: true },
        // Google's two (core 0.1.198): Gemini CLI's models from its quota once signed in, Antigravity's plan with Claude among them
        ...(signed("Gemini CLI")
          ? ["gemini-2.5-pro", "gemini-3.1-pro-preview"].map((model) => c({ model, label: model, place: "Gemini CLI", provider: "Gemini CLI", price: "plan", url: "https://cloudcode-pa.googleapis.com", key_env: "TAZUNA_SIGNIN_GEMINI", efforts: [], effort: null }))
          : [{ model: "gemini-models", label: "Gemini CLI's models", provider: "Gemini CLI", price: "plan", why_not: "not signed in", no_key: true }]),
        ...(signed("Antigravity")
          ? [["gemini-3.1-pro-high", "Gemini 3.1 Pro (High)"], ["gemini-3.1-pro-low", "Gemini 3.1 Pro (Low)"], ["claude-sonnet-4-6", "Claude Sonnet 4.6"], ["claude-opus-4-6-thinking", "Claude Opus 4.6 (Thinking)"]]
            .map(([model, label]) => c({ model, label, place: "Antigravity", provider: "Antigravity", price: "plan", url: "https://daily-cloudcode-pa.googleapis.com", key_env: "TAZUNA_SIGNIN_ANTIGRAVITY", efforts: [], effort: null }))
          : [{ model: "antigravity-models", label: "Antigravity's models", provider: "Antigravity", price: "plan", why_not: "not signed in", no_key: true }]),
      ], cautions: { "Gemini CLI": { note: PROVIDERS.find((x) => x.name === "Gemini CLI").note, warn: true }, Antigravity: { note: PROVIDERS.find((x) => x.name === "Antigravity").note, warn: true }, ChatGPT: { note: "OpenAI's own sign-in for open-source apps: requests use your ChatGPT plan (manage usage in ChatGPT's settings)", warn: false },
        "GitHub Copilot": { note: "tazuna signs in as OpenCode's GitHub app and calls Copilot as Copilot CLI. Whether GitHub's terms allow this, or GitHub acts on accounts for it, is unclear. Use at your own risk.", warn: true }, "Z.ai Coding Plan": { note: "Z.ai allows the Coding Plan only in its listed tools and tazuna is not listed: the account may be at risk", warn: true },
        Chutes: { note: "Chutes' terms: interactive and low-volume use, not large automated runs", warn: false } } }); return null; }
    if (cmd === "op" && args.msg.op === "sessions") { m({ type: "sessions", dirs: [
      { dir: "/Users/x", list: [{ id: "c1", first: "Names for a grey cat", last_at: now - 3600, messages: 2, model: "agy/gemini-4-argon" }, { id: "c4", first: "test", last_at: now - 7200, messages: 1, model: "agy/gemini-4-hydrogen" }, { id: "c5", first: "Question 1: what about edge case 1?", last_at: now - 2 * 86400, messages: 16, model: "claude-opus-5-5" }, { id: "c2", first: "Refund for a train ticket", last_at: now - 86400, messages: 6, model: "gpt-6.2-sol" }] },
      { dir: "/Users/x/api-gateway", list: [{ id: "p1", first: "Fix the flaky retry test", last_at: now - 20, messages: 4, model: "claude-opus-5-5" }, { id: "p2", first: "Bump the TLS library to the latest minor", last_at: now - 120, messages: 2, model: "deepseek-v4.1-flash" }, { id: "c3", first: "Why is the CI build so slow? Give me the top three fixes.", last_at: now - 3 * 3600, messages: 2, model: "claude-opus-5-5" }, { id: "p3", first: "Speed up the CI build", last_at: now - 5 * 86400, messages: 9, model: "mistral-large-3" }] }] }); return null; }
    return inv(cmd, args);
  };
})();

}

// desktop 0.1.87 (design review 8): what `/` and `@` complete to, an attached image's thumbnail, and an answer that names
// a picture it made ("Speed up the CI build", p3)
(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T);
  const chart = "data:image/svg+xml;base64," + btoa('<svg xmlns="http://www.w3.org/2000/svg" width="640" height="320" viewBox="0 0 640 320"><rect width="640" height="320" fill="#1b1a17"/>'
    + [[94, "npm ci"], [61, "type check"], [48, "tests"], [27, "lint"]].map(([v, l], i) => `<rect x="${70 + i * 140}" y="${260 - v * 2.2}" width="84" height="${v * 2.2}" rx="4" fill="${i ? "#3a3731" : "#8f9bf0"}"/><text x="${112 + i * 140}" y="286" fill="#afaa9e" font-family="sans-serif" font-size="15" text-anchor="middle">${l}</text><text x="${112 + i * 140}" y="${250 - v * 2.2}" fill="#ece8df" font-family="sans-serif" font-size="15" text-anchor="middle">${v} s</text>`).join("") + "</svg>");
  const p3 = { type: "view", from: 0, model: "mistral-large-3", effort: "high", cwd: "/Users/x/api-gateway", profile: "repo", mode: "auto", session_id: "p3", cost: 0.012, turns: 1, activity: "Done", context: 18000,
    usage: { input: 3000, cache_write: 0, cache_read: 14000, output: 700 },
    // a subagent still working after the reply ended (desktop 0.1.92): the row over the message box, and its own conversation
    tasks: [{ id: "t1", kind: "local_agent", agent: "general-purpose", description: "Review the chart script", status: "running", running: true, doing: "Reading tools/ci-chart.mjs", tokens: 48200, tools: 7, calls: ["ag1"], secs: null }],
    blocks: [{ kind: "prompt", text: "Chart where the CI time goes and save it as a picture." },
      { kind: "tool", id: "g1", name: "Bash", input: { command: "node tools/ci-chart.mjs" }, result: "wrote out/ci-time.svg", error: false, secs: 1.4, running: false },
      { kind: "tool", id: "ag1", name: "Agent", input: { description: "Review the chart script", subagent_type: "general-purpose", run_in_background: true }, result: "Async agent launched successfully.", error: false, secs: 0.9, running: false },
      { kind: "text", text: "Done. The chart is in `out/ci-time.svg`: `npm ci` is the tallest bar at 94 s, ahead of the type check and the tests. A reviewer is going through the script in the background." },
      { kind: "done", ok: true, turns: 1, stopped: false, cost: 0.012, secs: 964, usage: { input: 3000, cache_write: 0, cache_read: 14000, output: 700 } }] };
  T.invoke = async (cmd, args) => {
    const m = (msg) => setTimeout(() => T.emit("tazuna-view", { tid: args.tid, msg }), 20);
    if (cmd === "image_data") { if (/ci-time\.svg$|\.png$/.test(args.path)) return chart; throw "no such file"; }
    if (cmd === "pick_files") return ["/Users/x/Desktop/Screenshot 2026-11-19 at 21.04.png", "/Users/x/api-gateway/docs/retry-policy.md"];
    // a pasted image: its bytes come raw (args is the Uint8Array), saved as a file the message names
    if (cmd === "paste_image") return args instanceof Uint8Array && args.length ? "/tmp/tazuna-desktop-images/paste-1.png" : Promise.reject("no image in the paste");
    if (cmd === "paste_clipboard_image") return "/tmp/tazuna-desktop-images/paste-clipboard.png";
    if (cmd === "op" && args.msg.op === "resume" && args.msg.id === "p3") { m(p3); return null; }
    if (cmd === "op" && args.msg.op === "agent") { m({ type: "agent", call: args.msg.call, view: { ...p3, session_id: null, tasks: [], activity: "Tool", blocks: [
      { kind: "prompt", text: "Review tools/ci-chart.mjs: does it read the timings correctly, and is the SVG it writes valid? Report what you find; change nothing." },
      { kind: "tool", id: "s1", name: "Read", input: { file_path: "tools/ci-chart.mjs" }, result: "…", error: false, secs: 0.1, running: false },
      { kind: "tool", id: "s2", name: "Bash", input: { command: "node tools/ci-chart.mjs --check" }, result: "4 bars, 640x320", error: false, secs: 1.2, running: false },
      { kind: "text", text: "The script reads the four timings from `ci-timings.json` and sorts them, so the tallest bar is always first. Checking the SVG it writes next." },
      { kind: "tool", id: "s3", name: "Bash", input: { command: "xmllint --noout out/ci-time.svg" }, result: null, error: false, secs: 3.4, running: true }] } }); return null; }
    if (cmd === "op" && args.msg.op === "complete") { m({ type: "complete",
      commands: [["/agent", "Send to the coding agent, past the chat router"], ["/clear", "Start a new conversation in this folder"], ["/compact", "Summarize the conversation so far to free context"],
        ["/create", "Make a command, skill or tool for every model"], ["/init", "Write a CLAUDE.md for this repository"], ["/review", "Review a pull request"],
        ["/security-review", "Check the branch's changes for security problems"], ["/ticket", "Your skill: read a ticket and plan the change"]],
      files: ["src/retry.ts", "src/backoff.ts", "src/gateway.ts", "src/routes/health.ts", "tests/retry.test.ts", "tests/gateway.e2e.test.ts", "docs/retry-policy.md", "package.json", ".github/workflows/ci.yml", "README.md"] }); return null; }
    return inv(cmd, args);
  };
})();

// desktop 0.1.97 (design review 11, the look): a receipt's data, each turn saying how it was paid as the core sends it
// ("Why is the CI build so slow?": a turn on the Claude plan whose rows at list prices and its explorer subagent's two calls
// (in the total, not among the rows) do not add up to the CLI's exact total, then a turn on DeepSeek that moved from
// SubRouter to OpenRouter inside it (a 503), paid to both; "Speed up the CI build": paid to Mistral;
// "Fix the flaky retry test": an older turn without those fields, its total alone; design review 12: in "Why is the CI
// build so slow?" a turn on an API key (paid to Anthropic, not the plan), a stopped turn whose cost is tazuna's estimate
// ("about", its cut-off call's cost not known), and a SubRouter turn with a call the provider sent no usage for and two
// tries not in the total), today's runs for the start page's
// ledger (`window.__NO_TODAY__`: a day without one), and gh's error for a project (`window.__PR_ERROR__`).
(() => { const T = window.__TAURI_INTERNALS__; if (!window.__SENT__) return;
  const emit = T.emit.bind(T), now = Math.floor(Date.now() / 1000), midnight = Math.floor(new Date().setHours(0, 0, 0, 0) / 1000);
  const u = (i, c, o) => ({ input: i, cache_write: 0, cache_read: c, output: o });
  const call = (block, fresh, cached, out, cost, ms, model, more = {}) => ({ block, fresh, cached, out, cost, first_ms: 900, tps: 55, total_ms: ms, model, role: null, provider: null, ...more });
  const OPUS = { model: "claude-opus-5-5", plan: true, provider: null, price_m: [4, 0.4, 20] }, DEEP = { model: "deepseek-v4.1-flash", plan: false, provider: null, price_m: null, providers: ["SubRouter", "OpenRouter"] },
    MISTRAL = { model: "mistral-large-3", plan: false, provider: "Mistral", price_m: [2, 0.2, 6] },
    KEYED = { model: "claude-opus-5-5", plan: false, provider: "Anthropic API", price_m: [4, 0.4, 20] }, SUBR = { model: "glm-5.3", plan: false, provider: "SubRouter", price_m: [0.6, 0.11, 2.2] };
  const at = (i) => Math.max(midnight + 60, now - i * 300);
  const today = [...Array.from({ length: 9 }, (_, i) => [at(i + 1), "claude-opus-5-5", "0.1.176", true, 0.2 + i * 0.05, 2000, 500, 40000, 1200, null, null, null, "auto", true, [], []]),
    ...[0.12, 0.11, 0.1, 0.08].map((c, i) => [at(i + 10), "deepseek-v4.1-flash", "0.1.176", false, c, 3000, 0, 30000, 900, null, null, null, "auto", true, [], [], "SubRouter"])];
  T.emit = (event, payload) => {
    const m = payload?.msg;
    if (m?.type === "usage") m.rows = window.__NO_TODAY__ ? m.rows.filter((r) => r[0] < midnight) : [...m.rows.filter((r) => r[0] < midnight), ...today];
    if (m?.type === "pr" && window.__PR_ERROR__) payload.msg = { type: "pr", state: "", error: "no git remotes found" };
    if (m?.type === "view" && m.session_id === "c3" && !m.calls) {
      m.blocks = [...m.blocks.slice(0, 6), { kind: "done", ok: true, secs: 38, turns: 3, cost: 0.031, usage: u(1300, 8900, 560), run_cost: 0.031, run_usage: u(1300, 8900, 560), stopped: false, ...OPUS, sub_calls: 2 },
        { kind: "prompt", text: "What would the cache save us in a month?" },
        { kind: "tool", id: "t9", name: "bash", input: { command: "gh run list --limit 200 --json durationMs" }, result: "200 runs, median 6 min 12 s", error: false, running: false, secs: 2 },
        { kind: "notice", text: "route · now https://openrouter.ai/api/v1: HTTP 503", error: false },
        { kind: "text", text: "About 26 hours of runner time: 74 s saved on each of roughly 1,260 runs a month. At the plan's price per minute that is a little under $13.", live: false },
        { kind: "done", ok: true, secs: 14, turns: 2, cost: 0.0316, usage: u(2500, 20400, 920), run_cost: 0.0006, run_usage: u(1200, 11500, 360), stopped: false, ...DEEP },
        { kind: "prompt", text: "On the API key now: which step should we cache first?" },
        { kind: "text", text: "The npm store: it is 41% of the build and the cache step is three lines.", live: false },
        { kind: "done", ok: true, secs: 9, turns: 1, cost: 0.0736, usage: u(3700, 26400, 1040), run_cost: 0.042, run_usage: u(1200, 6000, 120), stopped: false, ...KEYED },
        { kind: "prompt", text: "And measure the type check on every branch" },
        { kind: "tool", id: "t10", name: "Bash", input: { command: "for b in $(git branch --format='%(refname:short)'); do …; done" }, result: null, error: false, running: false, stopped: true, secs: 31 },
        { kind: "notice", text: "stopped · send a message to continue", error: false },
        { kind: "done", ok: false, secs: 33, turns: 2, cost: 0.2536, usage: u(23700, 26400, 6040), run_cost: 0.18, run_usage: u(20000, 0, 5000), stopped: true, estimate: true, ...KEYED },
        { kind: "prompt", text: "Ask the cheap model for a one-line summary" },
        { kind: "text", text: "Cache the npm store first; run the type check beside the tests.", live: false },
        { kind: "done", ok: true, secs: 41, turns: 2, cost: 0.254, usage: u(25200, 26400, 6100), run_cost: 0.0004, run_usage: u(1500, 0, 60), stopped: false, uncounted: 2, ...SUBR }];
      // claude's rows are estimates at list prices (its exact total comes with the result); the own loop's are as charged
      m.calls = [call(1, 800, 2100, 90, 0.006, 4000, OPUS.model, { estimate: true }), call(3, 200, 2900, 110, 0.006, 6000, OPUS.model, { estimate: true }),
        call(5, 300, 3900, 360, 0.014, 21000, OPUS.model, { estimate: true }),
        call(8, 500, 5600, 210, 0.0002, 6000, "deepseek-v4.1-flash", { provider: "SubRouter" }), call(10, 700, 5900, 150, 0.0004, 5000, "deepseek-v4.1-flash", { provider: "OpenRouter" }),
        call(13, 1200, 6000, 120, 0.0072, 7000, KEYED.model, { estimate: true, provider: "Anthropic API" }),
        call(16, 20000, 0, 5000, 0.18, 30000, KEYED.model, { estimate: true, provider: "Anthropic API" }), call(17, 0, 0, 0, null, null, KEYED.model, { provider: "Anthropic API", first_ms: null, tps: null }),
        call(20, 0, 0, 0, null, 38000, SUBR.model, { provider: "SubRouter", first_ms: null, tps: null }), call(20, 1500, 0, 60, 0.0004, 3000, SUBR.model, { provider: "SubRouter" })];
      Object.assign(m, { cost: 0.254, turns: 10, usage: u(25200, 26400, 6100) });
    }
    if (m?.type === "view" && m.session_id === "p3" && !m.calls) {
      m.calls = [call(1, 1400, 6000, 210, 0.0053, 5200, MISTRAL.model, { provider: "Mistral" }), call(3, 1600, 8000, 490, 0.0077, 12400, MISTRAL.model, { provider: "Mistral" })];
      m.blocks = m.blocks.map((b) => (b.kind === "done" ? { ...b, ...MISTRAL, cost: 0.013 } : b));
      m.cost = 0.013;
    }
    return emit(event, payload);
  };
})();

// the chat's hand-off (2026-11-21), a plain chat on DeepSeek: a question about tomorrow's weather the chat could not know,
// answered by the agent after a fetch; a message with a link, which the chat handed over too; and an ordinary chat answer,
// whose "Send to agent" shows beside Copy on hover
(() => { const T = window.__TAURI_INTERNALS__; if (!window.__SENT__) return;
  const inv = T.invoke.bind(T), emit = T.emit.bind(T), now = Math.floor(Date.now() / 1000);
  const u = (i, c, o) => ({ input: i, cache_write: 0, cache_read: c, output: o });
  const DS = { model: "deepseek-v4.1-flash", plan: false, provider: "SubRouter", price_m: [0.15, 0.003, 0.6] };
  const done = (secs, turns, cost, usage, run, run_usage) => ({ kind: "done", ok: true, secs, turns, cost, usage, run_cost: run, run_usage, stopped: false, ...DS });
  const fetch = (id, url, secs) => ({ kind: "tool", id, name: "fetch", input: { url }, result: "…", error: false, secs, running: false });
  const c6 = { type: "view", from: 0, model: "deepseek-v4.1-flash", effort: "high", cwd: "/Users/x", profile: "chat", mode: "ask", session_id: "c6", cost: 0.0019, turns: 7, activity: "Done", context: 3400,
    usage: u(10400, 2000, 1150), blocks: [
      { kind: "prompt", text: "Will it rain in Berlin tomorrow?" },
      { kind: "notice", text: "answered by the agent: needs live data or this computer", error: false },
      fetch("h1", "https://wttr.in/Berlin?format=j1", 0.8),
      { kind: "text", text: "Most likely, yes. Tomorrow's forecast for Berlin has an 80% chance of rain from about noon, 9 to 12 °C, and a south-west wind around 20 km/h. Take an umbrella if you are out in the afternoon.", live: false },
      done(7, 3, 0.0008, u(4300, 0, 380), 0.0008, u(4300, 0, 380)),
      { kind: "prompt", text: "Summarise https://example.com/blog/static-sites in two lines" },
      { kind: "notice", text: "answered by the agent: needs live data or this computer", error: false },
      fetch("h2", "https://example.com/blog/static-sites", 1.1),
      { kind: "text", text: "The post argues that most small sites need no server: a static generator and a CDN give faster pages, fewer moving parts and almost no hosting cost.\n\nIt keeps a server only for what must change per visitor, such as logins and payments.", live: false },
      done(9, 2, 0.0015, u(8900, 1200, 900), 0.0007, u(4600, 1200, 520)),
      { kind: "prompt", text: "Name three classic science fiction novels" },
      { kind: "text", text: "*Dune* by Frank Herbert, *The Left Hand of Darkness* by Ursula K. Le Guin, and *Foundation* by Isaac Asimov.", live: false },
      done(3, 1, 0.0019, u(10400, 2000, 1150), 0.0004, u(1500, 800, 250))] };
  // the chat's own call (it replied [[agent]]) is in the first answer's receipt, beside the agent's two
  const call = (block, fresh, cached, out, cost, ms) => ({ block, fresh, cached, out, cost, first_ms: 600, tps: 60, total_ms: ms, model: DS.model, role: null, provider: "SubRouter" });
  c6.calls = [call(1, 900, 0, 5, 0.0001, 900), call(2, 1600, 0, 115, 0.0004, 2500), call(3, 1800, 0, 260, 0.0003, 4300),
    call(7, 2400, 1200, 120, 0.0003, 2400), call(8, 2200, 0, 400, 0.0004, 6200), call(11, 1500, 800, 250, 0.0004, 3000)];
  T.invoke = async (cmd, args) => {
    if (cmd === "op" && args.msg.op === "resume" && args.msg.id === "c6") { setTimeout(() => T.view(args.tid, c6), 10); return null; }
    return inv(cmd, args);
  };
  T.emit = (event, payload) => {
    const m = payload?.msg, home = m?.type === "sessions" && m.dirs.find((d) => d.dir === "/Users/x");
    if (home && !home.list.some((e) => e.id === "c6")) home.list.unshift({ id: "c6", first: "Will it rain in Berlin tomorrow?", last_at: now - 600, messages: 6, model: "deepseek-v4.1-flash" });
    return emit(event, payload);
  };
})();

// design review 12, the core's new shapes: Claude Code's sign-in in `setup` (`window.__CLAUDE_LOGIN__ = false`: signed out,
// its rows "not signed in" in the catalog), a local model with no context window (null), the signed-out error once
// ("what is a haiku?"), Retry that runs the failed turn again without a second message, a model switch that goes on in a
// new session under a divider, an edit asked for with its diff, and words before a step that go into the fold ("Rename the
// retry helper")
(() => { const T = window.__TAURI_INTERNALS__; if (!window.__SENT__) return;
  const inv = T.invoke.bind(T), emit = T.emit.bind(T), now = Math.floor(Date.now() / 1000);
  const u = (i, c, o) => ({ input: i, cache_write: 0, cache_read: c, output: o });
  const out = () => window.__CLAUDE_LOGIN__ === false;
  const last = {};
  const base = { type: "view", from: 0, effort: "high", mode: "ask", cost: 0, usage: u(0, 0, 0), turns: 0, context: 0, context_window: 200000, changes: [], edit_points: [], activity: "Done", limits: [36, 79], limits_reset: now + 8040, balance: 12.4 };
  const SIGNED_OUT = "Claude Code isn't signed in. Run `claude auth login` in a terminal (or Settings → Providers → Claude), then Retry.";
  const views = {
    c7: { ...base, session_id: "c7", model: "claude-opus-5-5", cwd: "/Users/x", profile: "chat", blocks: [{ kind: "prompt", text: "what is a haiku?" },
      { kind: "notice", text: SIGNED_OUT, error: true }, { kind: "done", ok: false, secs: 0.4, turns: 0, cost: 0, usage: u(0, 0, 0), stopped: false }] },
    p4: { ...base, session_id: "p4", model: "deepseek-v4.1-flash", cwd: "/Users/x/api-gateway", profile: "repo", activity: "Tool", cost: 0.002, turns: 2, context: 9000,
      blocks: [{ kind: "prompt", text: "Rename the retry helper to withRetry" },
        { kind: "text", text: "I'll read the helper first.", live: false },
        { kind: "tool", id: "e1", name: "read", input: { path: "src/retry.ts" }, result: "export async function retry(fn) { … }", error: false, secs: 0.1, running: false },
        { kind: "tool", id: "e2", name: "edit", input: { path: "src/retry.ts", old: "export async function retry<T>(fn: () => Promise<T>, tries = 3): Promise<T> {\n  for (let i = 0; ; i++) {", new: "export async function withRetry<T>(fn: () => Promise<T>, tries = 3): Promise<T> {\n  for (let attempt = 0; ; attempt++) {" }, error: false, secs: 0, running: true }],
      asking: { tool: "edit", what: "src/retry.ts", note: "this file, for the session", once: false, queued: 0 } },
    // two edits asked for at once: the core asks the oldest first, and its card shows that one's diff (review of review 12)
    p5: { ...base, session_id: "p5", model: "claude-opus-5-5", cwd: "/Users/x/api-gateway", profile: "repo", activity: "Tool", cost: 0.004, turns: 1, context: 12000,
      blocks: [{ kind: "prompt", text: "Rename limit to maxTries in both helpers" },
        { kind: "tool", id: "f1", name: "Edit", input: { file_path: "/Users/x/api-gateway/src/a.ts", old_string: "const limit = 3;", new_string: "const maxTries = 3;" }, error: false, secs: 0, running: true },
        { kind: "tool", id: "f2", name: "Edit", input: { file_path: "/Users/x/api-gateway/src/b.ts", old_string: "if (n > limit) throw err;", new_string: "if (n > maxTries) throw err;" }, error: false, secs: 0, running: true }],
      asking: { tool: "Edit", what: "/Users/x/api-gateway/src/a.ts", note: "", once: false, queued: 1 } },
  };
  const reply = "Sure. A **haiku** is a three-line poem with 5, 7 and 5 syllables.\n\nExample:\n\n> An old silent pond\n> a frog jumps into the pond\n> splash! silence again";
  T.invoke = async (cmd, args) => {
    const msg = args?.msg ?? {}, tid = args?.tid, m = (x) => setTimeout(() => T.emit("tazuna-view", { tid, msg: x }), 20);
    // `window.__NO_CLAUDE__`: Claude Code is not installed
    if (cmd === "op" && msg.op === "setup") { m(window.__NO_CLAUDE__ ? { type: "setup", claude: null, claude_login: null } : { type: "setup", claude: "2.1.330 (Claude Code)", claude_login: !out() }); return null; }
    if (cmd === "send" && window.__NO_CLAUDE__ && last[tid]) {
      const v = last[tid];
      if (args.draft != null) m({ type: "send_rejected", ...args.draft });
      m({ ...v, from: 0, blocks: [...v.blocks, { kind: "notice", text: "starting claude (is it installed and on PATH?): No such file or directory (os error 2)", error: true }] });
      return null;
    }
    if (cmd === "send" && window.__DRAFT_SEND_SUCCESS__ && last[tid]) {
      const v = last[tid];
      m({ ...v, from: 0, activity: "Thinking", blocks: [...v.blocks, { kind: "prompt", text: args.text }] });
      return null;
    }
    // Stop (and Send now, which stops first): the reply ends
    if (cmd === "interrupt" && window.__DRAFT_SEND_SUCCESS__ && last[tid]) {
      const v = last[tid];
      m({ ...v, from: 0, activity: "Done", blocks: [...v.blocks, { kind: "done", ok: false, secs: 1, turns: 0, cost: 0, usage: u(0, 0, 0), stopped: true }] });
      return null;
    }
    if (cmd === "op" && msg.op === "resume" && views[msg.id]) { setTimeout(() => T.view(tid, views[msg.id]), 10); return null; }
    // Retry: the failed turn's error makes way for the new reply; no second message of yours
    if (cmd === "op" && msg.op === "retry") {
      const v = last[tid]; if (!v) return null;
      const at = v.blocks.map((b) => b.kind).lastIndexOf("prompt"), kept = v.blocks.slice(0, at + 1);
      T.view(tid, { ...v, activity: "Thinking", blocks: kept });
      setTimeout(() => T.view(tid, { ...v, activity: "Done", context: 1800, blocks: [...kept, { kind: "text", text: reply, live: false },
        { kind: "done", ok: true, secs: 0.25, turns: 1, cost: 0, usage: u(1800, 0, 80), stopped: false }] }), 600);
      return null;
    }
    // a switch of model after messages: the conversation goes on in a new session, under a divider (core C5); a local
    // model's window is not known (C9)
    // as the core does it: no session until the new one's first turn (null), every earlier id in `earlier_sessions`
    if (cmd === "op" && msg.op === "model" && last[tid]?.blocks.some((b) => b.kind === "prompt")) {
      const v = last[tid], label = msg.model === "mock-coder-7b" ? "Mock Coder 7B" : msg.model, was = v.session_id;
      const next = { ...v, session_id: null, earlier_sessions: [...(v.earlier_sessions ?? []), ...(was ? [was] : [])], model: msg.model, context_window: msg.url?.includes("127.0.0.1") ? null : 200000,
        blocks: [...v.blocks, { kind: "notice", text: `── continued on ${label} ──`, error: false }] };
      setTimeout(() => T.view(tid, next), 30);
      setTimeout(() => T.view(tid, { ...next, session_id: `${was ?? "s"}-2` }), 600);
    }
    return inv(cmd, args);
  };
  T.emit = (event, payload) => {
    const msg = payload?.msg;
    if (msg?.type === "view") last[payload.tid] = msg;
    if (msg?.type === "models") {
      // signed out: Claude's rows as the sign-ins' are before a sign-in (C2); a model server on this machine
      if (out()) msg.choices = msg.choices.map((c) => (c.provider === "Claude" ? { model: c.model, label: c.label, provider: "Claude", price: "plan", why_not: "not signed in", no_key: true, current: c.current } : c));
      if (!msg.choices.some((c) => c.model === "mock-coder-7b"))
        msg.choices.push({ model: "mock-coder-7b", label: "mock-coder-7b · lm studio", place: "LM Studio · this machine", provider: "LM Studio", price: "0 / 0", url: "http://127.0.0.1:1234/v1", key_env: "LM_STUDIO_API_KEY", efforts: [], effort: null, bench: "", why_not: null, current: false });
    }
    const home = msg?.type === "sessions" && msg.dirs.find((d) => d.dir === "/Users/x"), proj = msg?.type === "sessions" && msg.dirs.find((d) => d.dir === "/Users/x/api-gateway");
    if (home && !home.list.some((e) => e.id === "c7")) home.list.unshift({ id: "c7", first: "what is a haiku?", last_at: now - 300, messages: 1, model: "claude-opus-5-5" });
    if (proj && !proj.list.some((e) => e.id === "p4")) proj.list.unshift({ id: "p4", first: "Rename the retry helper to withRetry", last_at: now - 60, messages: 1, model: "deepseek-v4.1-flash" });
    if (proj && !proj.list.some((e) => e.id === "p5")) proj.list.unshift({ id: "p5", first: "Rename limit to maxTries in both helpers", last_at: now - 30, messages: 1, model: "claude-opus-5-5" });
    return emit(event, payload);
  };
})();

// the receipt's numbers and the work panel's Stats (the user, 2026-11-25, mockups/v11.html): "Make the request logger…", a
// plan's own loop (DeepSeek V4.1 Flash 1017 on Mistral Vibe, at Mistral's reference price $0.30 / $0.007 / $1.20 per
// million: plan money, not billed), the user's eight calls from the screenshot and a second turn, each call with its time to
// first token, speed and time; `window.__PLAY__` plays a third turn call by call (start, next, end) for the live Stats.
// "Rename the config loader": a plan whose catalog lists no price (Z.ai Coding Plan): no reference, so no amount, never $0;
// then a paid turn (SubRouter), so its conversation's slip opens. The run log has Mistral Vibe's runs beside Claude's, for the
// usage page's plans by provider (`window.__REFLESS_ONLY__`: a no-price plan's runs alone). `window.__MOCK__.emit(id, f)`
// shows a conversation's view changed by f (a reply running, for the sweep).
(() => { const T = window.__TAURI_INTERNALS__; if (!window.__SENT__) return;
  const inv = T.invoke.bind(T), emit = T.emit.bind(T), now = Math.floor(Date.now() / 1000), midnight = Math.floor(new Date().setHours(0, 0, 0, 0) / 1000);
  const u = (i, c, o) => ({ input: i, cache_write: 0, cache_read: c, output: o });
  const VIBE = { model: "deepseek-v4.1-flash-1017", plan: true, provider: "Mistral Vibe", price_m: [0.3, 0.007, 1.2] };
  const price = (fresh, cached, out) => (fresh * 0.3 + cached * 0.007 + out * 1.2) / 1e6;
  // [in, cached, out, first token in s, t/s while writing]
  const call = (block, [inn, cached, out, first, tps]) => ({ block, fresh: inn - cached, cached, out, cost: price(inn - cached, cached, out), first_ms: Math.round(first * 1000), tps,
    total_ms: Math.round((first + out / tps + 0.15) * 1000), model: VIBE.model, role: null, provider: "Mistral Vibe", plan: true });
  const tool = (id, name, input) => ({ kind: "tool", id, name, input, result: "…", error: false, secs: name === "Bash" ? 2.4 : 0.1, running: false });
  const sum = (cs, f) => cs.reduce((a, c) => a + f(c), 0);
  const total = { i: 0, c: 0, o: 0, cost: 0 };
  const done = (cs) => { total.i += sum(cs, (c) => c.fresh); total.c += sum(cs, (c) => c.cached); total.o += sum(cs, (c) => c.out); total.cost += sum(cs, (c) => c.cost);
    return { kind: "done", ok: true, secs: Math.round(sum(cs, (c) => c.total_ms) / 1000 + 2), turns: cs.length, cost: total.cost, usage: u(total.i, total.c, total.o), stopped: false,
      run_cost: sum(cs, (c) => c.cost), run_usage: u(sum(cs, (c) => c.fresh), sum(cs, (c) => c.cached), sum(cs, (c) => c.out)), ...VIBE }; };
  const first = [[144.5e3, 0, 527, 1.9, 212], [145.6e3, 144.4e3, 234, 0.9, 198], [147.5e3, 145.5e3, 6.0e3, 0.8, 231], [153.1e3, 147.3e3, 312, 1.0, 205],
    [153.9e3, 153.1e3, 445, 1.1, 219], [154.5e3, 153.9e3, 293, 0.9, 224], [154.8e3, 154.5e3, 145, 0.7, 190], [155.2e3, 154.9e3, 701, 1.0, 216]].map((c, i) => call(i + 1, c));
  const second = [[156.0e3, 155.1e3, 210, 1.1, 208], [156.6e3, 156.0e3, 160, 0.8, 219], [157.3e3, 156.6e3, 540, 0.9, 226]].map((c, i) => call(12 + i, c));
  const f = (p) => ({ file_path: `src/${p}` });
  const blocks = [{ kind: "prompt", text: "Make the request logger skip health checks and log each request's latency" },
    tool("v1", "Read", f("logger.ts")), tool("v2", "Read", f("server.ts")), tool("v3", "Write", f("middleware/latency.ts")), tool("v4", "Read", f("routes/health.ts")),
    tool("v5", "Read", f("config.ts")), tool("v6", "Edit", f("logger.ts")), tool("v7", "Read", f("logger.test.ts")),
    { kind: "text", text: "The logger now skips `/health` and `/ready`, and every other request logs its latency in milliseconds from a small middleware (`src/middleware/latency.ts`).", live: false },
    done(first), { kind: "prompt", text: "Log the route's pattern, not the raw path" }, tool("v8", "Read", f("middleware/latency.ts")), tool("v9", "Edit", f("middleware/latency.ts")),
    { kind: "text", text: "It logs `req.route.path` now (`/users/:id`), falling back to the raw path where no route matched.", live: false }, done(second)];
  const p6 = { type: "view", from: 0, model: VIBE.model, effort: "high", cwd: "/Users/x/api-gateway", profile: "repo", mode: "auto", session_id: "p6", activity: "Done", turns: 11,
    cost: total.cost, usage: u(total.i, total.c, total.o), context: 157300, context_window: 256000, changes: [], edit_points: [], blocks, calls: [...first, ...second] };
  // the plan with no reference price: the core starts it at [0,0,0], so its calls come with their model and cost 0
  const zcall = (block, inn, cached, out, first, tps) => ({ block, fresh: inn - cached, cached, out, cost: 0, first_ms: first, tps, total_ms: Math.round(first + (out / tps) * 1000 + 150),
    model: "glm-5.3", role: null, provider: "Z.ai Coding Plan" });
  const scall = { block: 6, fresh: 1300, cached: 2100, out: 90, cost: 0.0004, first_ms: 700, tps: 140, total_ms: 1500, model: "deepseek-v4.1-flash", role: null, provider: "SubRouter" };
  const p7 = { type: "view", from: 0, model: "deepseek-v4.1-flash", effort: "high", cwd: "/Users/x/api-gateway", profile: "repo", mode: "auto", session_id: "p7", activity: "Done", turns: 3, cost: 0.0004,
    usage: u(5400, 20100, 700), context: 3400, context_window: 200000, changes: [], edit_points: [], calls: [zcall(1, 9800, 0, 140, 2300, 88), zcall(2, 12300, 9800, 470, 1200, 94), scall],
    blocks: [{ kind: "prompt", text: "Rename the config loader to loadSettings" }, tool("z1", "Edit", f("config.ts")),
      { kind: "text", text: "Renamed `loadConfig` to `loadSettings` in `src/config.ts` and its two callers.", live: false },
      { kind: "done", ok: true, secs: 9, turns: 2, cost: 0, usage: u(4100, 18000, 610), stopped: false, run_cost: 0, run_usage: u(4100, 18000, 610), model: "glm-5.3", plan: true, provider: "Z.ai Coding Plan", price_m: null },
      { kind: "notice", text: "── continued on DeepSeek V4.1 Flash ──", error: false }, { kind: "prompt", text: "Say in one line what changed" },
      { kind: "text", text: "`loadConfig` is `loadSettings` now, in the module and both callers.", live: false },
      { kind: "done", ok: true, secs: 2, turns: 1, cost: 0.0004, usage: u(5400, 20100, 700), stopped: false, run_cost: 0.0004, run_usage: u(1300, 2100, 90), model: "deepseek-v4.1-flash", plan: false, provider: "SubRouter", price_m: [0.15, 0.003, 0.6] }] };
  let tid6 = null, live = p6;
  const show = (v) => { live = v; T.view(tid6, v); }, tids = {}, views = { p6, p7 };
  window.__MOCK__ = { emit: (id, f) => T.view(tids[id], f(JSON.parse(JSON.stringify(views[id])))) };
  const third = [[158.0e3, 157.2e3, 260, 1.4, 201, "Read"], [158.9e3, 158.0e3, 1900, 0.8, 228, "Write"], [161.2e3, 158.9e3, 140, 0.7, 196, "Bash"], [161.5e3, 161.2e3, 380, 0.9, 223, "Read"]];
  let n = 0;
  window.__PLAY__ = {
    start() { n = 0; show({ ...p6, activity: "Thinking", blocks: [...p6.blocks, { kind: "prompt", text: "Add a test for the skipped health checks" }] }); },
    // a call ends: its step, its numbers in the view, the conversation's cost and context with it
    next() { const [i, c, o, fi, tp, name] = third[n++], at = live.blocks.length, k = call(at, [i, c, o, fi, tp]), calls = this.timer ? live.calls.slice(0, -1) : live.calls;
      clearInterval(this.timer); this.timer = 0;
      show({ ...live, activity: "Tool", cost: live.cost + k.cost, context: i, blocks: [...live.blocks, tool(`w${n}`, name, name === "Bash" ? { command: "npm test -- logger" } : f("logger.test.ts"))], calls: [...calls, k], live: null }); },
    // the next call streaming (the core's `live`, src/lib/live.ts): its row in `calls` at zeros until it ends, the estimate
    // (4% over the exact count, which it settles to) in a view every 50 ms: 0.8 s to the first token, then `tps`; next() ends it
    stream(tps = 200) { clearInterval(this.timer); const [i, c, o] = third[n], t0 = Date.now(), calls = live.calls;
      const z = { ...call(live.blocks.length, [i, c, o, 0.8, tps]), fresh: 0, cached: 0, out: 0, cost: 0, first_ms: null, tps: null, total_ms: null };
      const tick = () => { const ms = Date.now() - t0, out = ms < 800 ? 0 : Math.min(Math.round(o * 1.04), Math.round(((ms - 800) / 1000) * tps));
        show({ ...live, activity: ms < 800 ? "Thinking" : "Writing", calls: [...calls, z], live: { started: t0, first: ms < 800 ? null : t0 + 800, out, deltas: Math.max(0, Math.floor((ms - 800) / 50)),
          in: i, cached: c, in_cost: price(i - c, c, 0), out_m: 1.2, row: calls.length } }); };
      tick(); this.timer = setInterval(tick, 50); },
    end() { const cs = live.calls.slice(p6.calls.length); clearInterval(this.timer); this.timer = 0;
      show({ ...live, activity: "Done", live: null, blocks: [...live.blocks, { kind: "text", text: "Added `logger.test.ts`: health checks log nothing, other routes log their pattern and latency.", live: false }, done(cs)] }); },
  };
  const vibeRows = [1, 2, 3].map((d) => [midnight - d * 86400 + 3600, VIBE.model, "0.1.226", true, 0.06 + d * 0.01, 1500, 0, 150000, 900, "completed", "claimed", null, "live", true, [], [], "Mistral Vibe"]);
  T.invoke = async (cmd, args) => {
    if (cmd === "op" && args.msg.op === "resume" && args.msg.id === "p6") { tid6 = tids.p6 = args.tid; setTimeout(() => show(p6), 10); return null; }
    if (cmd === "op" && args.msg.op === "resume" && args.msg.id === "p7") { tids.p7 = args.tid; setTimeout(() => T.view(args.tid, p7), 10); return null; }
    return inv(cmd, args);
  };
  T.emit = (event, payload) => {
    const m = payload?.msg, proj = m?.type === "sessions" && m.dirs.find((d) => d.dir === "/Users/x/api-gateway");
    if (proj && !proj.list.some((e) => e.id === "p6")) proj.list.push({ id: "p6", first: "Make the request logger skip health checks and log each request's latency", last_at: now - 4 * 3600, messages: 4, model: VIBE.model },
      { id: "p7", first: "Rename the config loader to loadSettings", last_at: now - 6 * 3600, messages: 2, model: "glm-5.3" });
    if (m?.type === "usage") m.rows = window.__REFLESS_ONLY__ ? [1, 2].map((d) => [midnight - d * 86400 + 3600, "glm-5.3", "0.1.226", true, 0, 9800, 0, 9800, 600, "completed", "claimed", null, "live", true, [], [], "Z.ai Coding Plan"])
      : [...m.rows, ...vibeRows];
    return emit(event, payload);
  };
})();

// a step's insides (review 14, steps): "Order the summary lines by their detail" in the api-gateway project, a reply whose
// steps are each kind a step draws as code: a read of a .ts range with long lines (claude's numbered lines), `sed -n 1,40p` of
// a .ts file after a cd into a folder of the project, a `grep -n` over one .ts file, an edit, a multi-edit, a new file, a
// Python and a YAML read, and a command after a cd into the project itself. `window.__STEPS__.ask(kind)`: the same reply
// waiting on an "edit", "multi", "write", "far" (a long path outside the project) or "huge" (a 476 KB line) approval; and a read of a deeply nested file (its folder cut in the title) (`.done()` puts it back).
(() => { const T = window.__TAURI_INTERNALS__; if (!window.__SENT__) return;
  const inv = T.invoke.bind(T), emit = T.emit.bind(T), now = Math.floor(Date.now() / 1000);
  const P = "/Users/x/api-gateway", D = `${P}/src/routes/orders`;
  const num = (from, text) => text.split("\n").map((l, i) => `${String(from + i).padStart(6)}→${l}`).join("\n");
  const tool = (id, name, input, result, secs = 0.1) => ({ kind: "tool", id, name, input, result, error: false, secs, running: false });
  const read = num(88, [
    "  // A hold that expired while the cart was open is released here, before the totals are computed, so a customer never pays for stock that is no longer reserved for them; see the note on clearing holds.",
    "  for (const hold of cart.holds) {",
    "    if (hold.expiresAt <= now) await releaseHold(hold.id, { reason: \"expired\", notify: false, keepAudit: true, source: \"summary\" });",
    "  }",
    "  const lines = cart.items.map((item) => summaryLine(item, prices.get(item.sku) ?? fallbackPrice(item), { currency: cart.currency, rounding: 2 }));",
    "  return { lines, total: sum(lines.map((l) => l.amount)), currency: cart.currency, heldUntil: null };",
    "}", ""].join("\n"));
  const unit = ["import { describe, expect, it } from 'vitest';", "import { planSummary } from './summary';", "import { cartWith, priceMap } from '../../test/fixtures';", "",
    "describe('planSummary', () => {", "  it('lists every item once, with its price', () => {", "    const cart = cartWith([{ sku: 'TEE-01', qty: 2 }, { sku: 'CAP-07', qty: 1 }]);",
    "    const summary = planSummary(cart, priceMap({ 'TEE-01': 1999, 'CAP-07': 1250 }));", "    expect(summary.lines).toHaveLength(2);",
    "    expect(summary.total).toBe(5248); // 2 × 19.99 + 12.50, in cents", "  });", "", "  /* an empty cart has no lines and a zero total */",
    "  it('handles an empty cart', () => {", "    expect(planSummary(cartWith([]), priceMap({})).total).toBe(0);", "  });", "});"].join("\n");
  const grep = ["141:    const latestFetch = await fetchPrices(cart.items.map((item) => item.sku), { cache: \"no-store\", timeoutMs: 2500, retries: 2, signal: abort.signal });",
    "156:    if (!latestFetch.ok) return staleSummary(cart, latestFetch.error ?? new Error(\"prices unavailable\"), { keepHolds: true }); // stale, never empty"].join("\n");
  const rank = ["// where a line's detail goes in the summary: size first, then colour, then the rest by name", "export const ORDER = ['size', 'colour', 'material'] as const;", "",
    "export type Detail = (typeof ORDER)[number] | string;", "", "/** A detail's place: its index in ORDER, or after all of them. */", "export function rank(detail: Detail): number {",
    "  const at = ORDER.indexOf(detail as (typeof ORDER)[number]);", "  return at < 0 ? ORDER.length : at;", "}", ""].join("\n");
  const py = num(1, ["#!/usr/bin/env python3", "\"\"\"Seed the local database with a few orders for the summary tests.\"\"\"", "import json", "from pathlib import Path", "",
    "FIXTURES = Path(__file__).parent / \"fixtures\"  # next to this script", "", "def seed(db, count=12):", "    for i, order in enumerate(json.loads((FIXTURES / \"orders.json\").read_text())):",
    "        if i >= count:", "            break", "        db.insert(\"orders\", {**order, \"status\": 'open'})  # every one starts open"].join("\n"));
  const yml = num(1, ["# runs on every push and pull request", "name: ci", "on:", "  push:", "    branches: [main]", "  pull_request:", "jobs:", "  test:",
    "    runs-on: ubuntu-latest", "    timeout-minutes: 15", "    steps:", "      - uses: actions/checkout@v4", "      - run: npm ci", "      - run: npm test -- --reporter=dot # quiet on CI"].join("\n"));
  const multi = [
    { old_string: "export function planSummary(cart: Cart, prices: PriceMap): Summary {\n  const lines = cart.items.map((item) => summaryLine(item, prices));\n  return { lines, total: sum(lines.map((l) => l.amount)) };",
      new_string: "export function planSummary(cart: Cart, prices: PriceMap): Summary {\n  const lines = sortSummaryLines(cart.items.map((item) => summaryLine(item, prices)));\n  return { lines, total: sum(lines.map((l) => l.amount)) };" },
    { old_string: "import { summaryLine } from './line';", new_string: "import { summaryLine, type SummaryLine } from './line';\nimport { rank } from './rank';\n\n/** The lines in the order their details are read: by rank, then by name. */\nexport const sortSummaryLines = (lines: SummaryLine[]) => [...lines].sort((a, b) => rank(a.detail) - rank(b.detail) || a.name.localeCompare(b.name));" }];
  const edit = { file_path: `${D}/summary.unit.ts`, old_string: "import { planSummary } from './summary';", new_string: "import { planSummary, sortSummaryLines } from './summary';" };
  const blocks = [{ kind: "prompt", text: "Order the summary lines by their detail: size, then colour, then the rest" },
    tool("k1", "Read", { file_path: `${D}/summary.ts`, offset: 88, limit: 8 }, read),
    tool("k2", "Bash", { command: `cd "${D}" && sed -n 1,40p summary.unit.ts` }, unit, 0.1),
    tool("k3", "Bash", { command: "grep -n \"latestFetch\" src/routes/orders/summary.ts" }, grep, 0.1),
    tool("k4", "Edit", edit, `The file ${D}/summary.unit.ts has been updated successfully.`, 0.2),
    tool("k5", "MultiEdit", { file_path: `${D}/summary.ts`, edits: multi }, `Applied 2 edits to ${D}/summary.ts:\n1. Replaced "export function planSummary(…" with "export function planSummary(…"\n2. Replaced "import { summaryLine } from…" with "import { summaryLine, type Su…"`, 0.2),
    tool("k6", "Write", { file_path: `${D}/rank.ts`, content: rank }, `File created successfully at: ${D}/rank.ts`, 0.1),
    tool("k7", "Read", { file_path: `${P}/scripts/seed_orders.py`, limit: 12 }, py),
    tool("k8", "Read", { file_path: `${P}/.github/workflows/ci.yml`, limit: 14 }, yml),
    tool("k9", "Bash", { command: `cd "${P}" && npm test -- summary` }, " ✓ src/routes/orders/summary.unit.ts (3 tests) 41ms\n\n Test Files  1 passed (1)\n      Tests  3 passed (3)", 3.1),
    // a path deep enough that its folder is cut from the left in the step's title, its file name whole
    tool("k10", "Read", { file_path: `${P}/packages/storefront/src/features/checkout/components/summary/lines/SummaryLineDetailBadge.tsx`, limit: 3 },
      num(1, ["export function SummaryLineDetailBadge({ detail }: { detail: string }) {", "  return <span className=\"badge\">{detail}</span>;", "}"].join("\n"))),
    { kind: "text", text: "The summary's lines now go by their detail: size, then colour, then material, and anything else after them by name (`sortSummaryLines` in `summary.ts`, the order in the new `rank.ts`). The tests pass.", live: false },
    { kind: "done", ok: true, secs: 48, turns: 9, cost: 0.061, usage: { input: 3100, cache_write: 0, cache_read: 21800, output: 1900 }, stopped: false }];
  const p9 = { type: "view", from: 0, model: "claude-opus-5-5", effort: "high", cwd: P, profile: "repo", mode: "ask", session_id: "p9", activity: "Done", turns: 9, cost: 0.061,
    usage: { input: 3100, cache_write: 0, cache_read: 21800, output: 1900 }, context: 24900, context_window: 200000, edit_points: [], blocks,
    changes: [{ turn: 0, at_block: blocks.length - 1, undone: false, can_undo: true, files: [{ path: "src/routes/orders/summary.ts", added: 6, removed: 2, status: "M" },
      { path: "src/routes/orders/summary.unit.ts", added: 1, removed: 1, status: "M" }, { path: "src/routes/orders/rank.ts", added: 10, removed: 0, status: "A" }] }] };
  // the turn's diff as git gives it (the review's op): the edit's and the multi-edit's lines in it, so their steps are numbered
  const head = (f, made) => `diff --git a/src/routes/orders/${f} b/src/routes/orders/${f}\n${made ? "new file mode 100644\n--- /dev/null" : `--- a/src/routes/orders/${f}`}\n+++ b/src/routes/orders/${f}\n`;
  const files = [head("summary.ts"), "@@ -1,5 +1,9 @@\n import type { Cart, PriceMap, Summary } from './types';\n", ...multi[1].old_string.split("\n").map((l) => `-${l}\n`), ...multi[1].new_string.split("\n").map((l) => `+${l}\n`),
    " import { sum } from '../../lib/money';\n \n const fallbackPrice = (item: CartItem) => item.listPrice;\n@@ -172,7 +176,7 @@ export async function summarise(cart: Cart, now: number) {\n }\n \n",
    ` ${multi[0].old_string.split("\n")[0]}\n-${multi[0].old_string.split("\n")[1]}\n+${multi[0].new_string.split("\n")[1]}\n ${multi[0].old_string.split("\n")[2]}\n }\n \n`,
    head("summary.unit.ts"), `@@ -1,5 +1,5 @@\n ${unit.split("\n")[0]}\n-${edit.old_string}\n+${edit.new_string}\n ${unit.split("\n").slice(2, 5).join("\n ")}\n`,
    head("rank.ts", true), "@@ -0,0 +1,10 @@\n", ...rank.split("\n").slice(0, 10).map((l) => `+${l}\n`)].join("");
  // with `path` (a step's numbers) that file alone, as the core answers, with the project's folder in the repository ("")
  const fileOf = (path) => files.split(/(?=^diff --git )/m).find((f) => f.startsWith(`diff --git a/${path} `)) ?? "";
  let tid = null;
  const asks = { edit: ["Edit", edit], multi: ["MultiEdit", { file_path: `${D}/summary.ts`, edits: multi }], write: ["Write", { file_path: `${D}/rank.ts`, content: rank }],
    // a file outside the project behind a long path (the card wraps it, never cuts its "~/"), and one minified line of 476 KB
    far: ["Edit", { file_path: "/Users/x/other-app/src/features/settings/components/forms/advanced/sections/billing/config.ts", old_string: "export const retries = 3;", new_string: "export const retries = 5;" }],
    huge: ["Write", { file_path: `${D}/vendor.min.js`, content: "var a=1,b=\"x\";".repeat(34_000) }] };
  window.__STEPS__ = {
    ask(kind) { const [name, input] = asks[kind], b = blocks.slice(0, 4);
      T.view(tid, { ...p9, activity: "Tool", changes: [], blocks: [...b, { kind: "tool", id: "ask", name, input, error: false, secs: 0, running: true }],
        asking: { tool: name, what: input.file_path, note: "always = this tool", once: false, queued: 0 } }); },
    done() { T.view(tid, p9); },
  };
  T.invoke = async (cmd, args) => {
    if (cmd === "op" && args.msg.op === "resume" && args.msg.id === "p9") { tid = args.tid; setTimeout(() => T.view(args.tid, p9), 10); return null; }
    if (cmd === "op" && args.msg.op === "review" && args.tid === tid && args.msg.scope === "turn") { const path = args.msg.path ?? null;
      setTimeout(() => emit("tazuna-view", { tid, msg: { type: "review", scope: "turn", turn: args.msg.turn, path, prefix: "", base: null, text: path ? fileOf(path) : files } }), 20); return null; }
    return inv(cmd, args);
  };
  T.emit = (event, payload) => {
    const m = payload?.msg, proj = m?.type === "sessions" && m.dirs.find((d) => d.dir === P);
    if (proj && !proj.list.some((e) => e.id === "p9")) proj.list.push({ id: "p9", first: "Order the summary lines by their detail: size, then colour, then the rest", last_at: now - 8 * 3600, messages: 2, model: "claude-opus-5-5" });
    return emit(event, payload);
  };
})();
