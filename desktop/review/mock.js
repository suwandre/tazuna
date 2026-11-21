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
  p2: { ...base, cwd: PROJ, profile: "repo", session_id: "p2", activity: "Tool", blocks: [{ kind: "prompt", text: "Bump the TLS library to the latest minor" }, tool("u1", "Bash", { command: "npm outdated" }, 1.2)],
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
    credits: [{ provider: "SubRouter", left: "$12.40 left", page: "https://subrouter.ai" }, { provider: "Mortera", left: null, page: "https://mortera.cloud" }],
    omniroute: [{ provider: "agy", name: "someone@example.com", plan: "Ultra", status: "valid", windows: [{ group: "Gemini Models", window: "weekly", used: 12, resets: "2026-11-25T11:03:10.000Z", unlimited: false }] }] };
  const day = (i) => Math.floor(Date.now() / 1000) - i * 86400;
  window.__ROWS__ = Array.from({ length: 60 }, (_, i) => [day(i % 20) , i % 4 ? "claude-opus-5-5" : "deepseek-v4.1-flash", 0.02 + (i % 7) * 0.01, i % 4 !== 0]);
  T.invoke = async (cmd, args) => {
    if (cmd === "op" && args.msg.op === "accounts") { setTimeout(() => T.emit("tazuna-view", { tid: args.tid, msg: { type: "accounts", ...window.__ACCOUNTS__ } }), 300); return null; }
    return inv(cmd, args);
  };
})();

(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T), now = Math.floor(Date.now() / 1000);
  const rows = [];
  for (let d = 0; d < 30; d++) for (let k = 0; k < (d * 7) % 5 + 1; k++) {
    const plan = (d + k) % 6 !== 0, model = plan ? (k % 3 ? "claude-opus-5-5" : "claude-sonnet-5-5") : "deepseek-v4.1-flash";
    rows.push([now - d * 86400 - k * 600, model, d < 10 ? "0.1.176" : "0.1.170", plan, plan ? 0.3 + ((d * k) % 7) * 0.12 : 0.02 + (k % 3) * 0.01, 2000, 500, 40000, 1200, null, null, null, "auto", true, (d + k) % 9 === 0 ? ["stopped"] : [], (d + k) % 7 === 0 ? ["why"] : []]);
  }
  T.invoke = async (cmd, args) => {
    if (cmd === "op" && args.msg.op === "usage") { setTimeout(() => T.emit("tazuna-view", { tid: args.tid, msg: { type: "usage", rows } }), 20); return null; }
    return inv(cmd, args);
  };
})();

(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T);
  const diff = "diff --git a/tests/retry.test.ts b/tests/retry.test.ts\n--- a/tests/retry.test.ts\n+++ b/tests/retry.test.ts\n@@ -12,9 +12,15 @@ describe(\"retry\", () => {\n   it(\"retries once after a failure\", async () => {\n-    const fn = vi.fn().mockRejectedValueOnce(new Error(\"boom\"));\n-    await retry(fn);\n-    await sleep(100);\n+    vi.useFakeTimers();\n+    const fn = vi.fn().mockRejectedValueOnce(new Error(\"boom\")).mockResolvedValue(\"ok\");\n+    const p = retry(fn);\n+    // the backoff is 80 ms with up to 25% jitter: past its longest\n+    await vi.advanceTimersByTimeAsync(100);\n+    await p;\n     expect(fn).toHaveBeenCalledTimes(2);\n+    vi.useRealTimers();\n   });\n";
  T.invoke = async (cmd, args) => {
    const m = (msg) => setTimeout(() => T.emit("tazuna-view", { tid: args.tid, msg }), 20);
    if (cmd === "op" && args.msg.op === "diff") { m({ type: "diff", turn: args.msg.turn, path: args.msg.path, text: diff }); return null; }
    if (cmd === "op" && args.msg.op === "review") { m({ type: "review", scope: args.msg.scope, turn: args.msg.turn ?? null, base: "main", text: diff }); return null; }
    if (cmd === "op" && args.msg.op === "settings") { m({ type: "settings", version: "0.1.176", data_any: false, route: true, spend_cap: 5, spent_24h: 0.41,
      providers: [{ name: "SubRouter", key: "sk-…9f2c", example: "sk-…", page: "https://subrouter.ai", place: "SubRouter" }, { name: "Mistral", key: "", example: "", page: "https://console.mistral.ai/api-keys", place: "Mistral" }] }); return null; }
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
  T.invoke = async (cmd, args) => {
    const m = (msg) => setTimeout(() => T.emit("tazuna-view", { tid: args.tid, msg }), 20);
    if (cmd === "op" && args.msg.op === "resume" && views[args.msg.id]) { setTimeout(() => T.view(args.tid, views[args.msg.id]), 10); return null; }
    if (cmd === "op" && args.msg.op === "models") { const c = (o) => ({ efforts: ["low", "medium", "high", "max"], effort: "high", bench: "", why_not: null, key_env: null, url: null, current: false, ...o });
      m({ type: "models", rebuild: null, choices: [
        c({ model: "claude-opus-5-5", label: "Opus 5.5", place: "Claude · subscription", provider: "Claude", price: "plan", current: true }),
        c({ model: "claude-sonnet-5-5", label: "Sonnet 5.5", place: "Claude · subscription", provider: "Claude", price: "plan" }),
        c({ model: "claude-fable-5-2", label: "Fable 5.2", place: "Claude · subscription", provider: "Claude", price: "plan" }),
        c({ model: "claude-haiku-5-5", label: "Haiku 5.5", place: "Claude · subscription", provider: "Claude", price: "plan" }),
        c({ model: "gpt-6.2-sol", label: "gpt-6.2-sol · subrouter", place: "subrouter.ai", provider: "SubRouter", price: "1.25 / 10", bench: "8.4 (12)", efforts: ["low", "medium", "high"] }),
        c({ model: "deepseek-v4.1-flash", label: "deepseek-v4.1-flash · subrouter", place: "subrouter.ai", provider: "SubRouter", price: "0.14 / 0.56", bench: "7.1 (16)", efforts: ["off", "high", "max"] }),
        c({ model: "codestral-2508", label: "codestral-2508 · mistral", place: "api.mistral.ai", provider: "Mistral", price: "0.3 / 0.9" }),
        c({ model: "mistral-large-3", label: "mistral-large-3 · mistral", place: "api.mistral.ai", provider: "Mistral", price: "2 / 6", bench: "7.8 (12)" }),
        c({ model: "agy/gemini-4-hydrogen", label: "gemini-4-hydrogen · agy", place: "OmniRoute · agy", provider: "OmniRoute", price: "plan" }),
        c({ model: "or/qwen-4-coder", label: "qwen-4-coder · openrouter", place: "openrouter.ai", provider: "OpenRouter", price: "0.2 / 0.8", why_not: "needs OPENROUTER_API_KEY" }),
      ] }); return null; }
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
