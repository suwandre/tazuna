// The console reader's script (src-tauri/src/console.js) outside a window: its sanitizer on made-up JSON full of values,
// then its fetch and XMLHttpRequest wrappers on fake responses, in learn and read mode, on and off the provider's sites;
// its text path: the masked lines of a made-up page, the parser text extractors call, the settle watcher on a fake clock,
// and a text extractor for a typical layout (MISTRAL_EXAMPLE: the shape of the one Mistral Vibe's CONSOLES entry gets).
//   node review/console.cjs
const fs = require("fs"), path = require("path"), assert = require("assert");
// eval: the script is a function expression (main.rs appends its call), loaded here as the webview loads it
const install = eval(fs.readFileSync(path.join(__dirname, "../src-tauri/src/console.js"), "utf8"));
const api = install(null, null, null, null);
const SECRETS = ["someone@example.com", "1234567", "sk-live-abc", "1500.25", "2026-12-01", "Pro plan", "550e8400"];
const body = { user: { email: "someone@example.com", id: 1234567, token: "sk-live-abc" }, quota: { remaining: 1500.25, total: 2000, resetAt: "2026-12-01", plan: "Pro plan" },
  items: [{ name: "a", used: 1 }, { name: "b", extra: 2 }], "someone@example.com": { x: 1 }, "550e8400e29b41d4a716446655440000": true, flags: [], nothing: null };
const SHAPE = { user: { email: "string", id: "number", token: "string" }, quota: { remaining: "number", total: "number", resetAt: "string", plan: "string" },
  items: [{ name: "string", used: "number" }], "*": { x: "number" }, flags: [], nothing: "null" };
const clean = (x) => { const t = JSON.stringify(x); for (const s of SECRETS) assert(!t.includes(s), `a value got through: ${s} in ${t}`); };

// the sanitizer: keys and type names, the first item of an array, a key that may be a value as "*"; paths with ids as ":id",
// queries as their keys
assert.deepStrictEqual(api.shape(body), SHAPE);
clean(api.shape(body));
assert.deepStrictEqual(api.usageKeys(SHAPE), ["quota", "remaining", "total", "plan", "used"]);
assert.strictEqual(api.cleanPath(new URL("https://home.qwencloud.com/api/users/1234567/usage/550e8400-e29b-41d4-a716-446655440000/e29b41d4a7164466554400?region=sg&token=abc&region=x"), true),
  "/api/users/:id/usage/:id/:id?region&token");
assert.strictEqual(api.cleanPath(new URL("https://chat.mistral.ai/code/v1/usage?x=1"), false), "/code/v1/usage");
// a learned page keeps its region from the list (Qwen Cloud's usage is per region), and nothing else of its query
assert.strictEqual(api.pageOf(new URL("https://home.qwencloud.com/analytics/token-plan?tab=x&region=ap-southeast-1&region=us-east-1&token=abc")), "https://home.qwencloud.com/analytics/token-plan?region=ap-southeast-1");
assert.strictEqual(api.pageOf(new URL("https://home.qwencloud.com/usage/1234567?region=someone")), "https://home.qwencloud.com/usage/:id");
assert(api.onSite(["home.qwencloud.com"], "home.qwencloud.com") && !api.onSite(["home.qwencloud.com"], "x.home.qwencloud.com") && !api.onSite(["home.qwencloud.com"], "qwencloud.com"));
// values the review found kept (security review of the console reader): ids, keys, names; real field names stay
const uuidL = "e29b41d4-a716-4466-8554-446655440000";
assert.strictEqual(api.cleanPath(new URL(`https://chat.mistral.ai/v1/customers/cus_NffrFeUfNV2Hib/keys/sk-AbCdEfGhIjKlMnOp/${uuidL}/api.json`), false), "/v1/customers/:id/keys/:id/:id/api.json");
for (const k of [uuidL, "acme-prod", "john.smith", "key_2aBcD9eFgH1jK", "cus_NffrFeUfNV2Hib", "AbCdEfGhIjKl", "user9f8e7d6c", "a@b"]) assert.strictEqual(api.key(k), "*", k);
for (const k of ["used_tokens", "totalQuota", "remaining", "plan_limit", "resetAt", "v1", "creditsRemaining"]) assert.strictEqual(api.key(k), k, k);
assert.deepStrictEqual(api.shape({ acme: { used: 1, limit: 2 }, globex: { used: 3, limit: 4 } }), { "*": { used: "number", limit: "number" } });
assert.deepStrictEqual(api.shape({ daily: 1, monthly: 2 }), { daily: "number", monthly: "number" }, "leaves are not a map");
const deep = Array.from({ length: 15 }).reduce((v) => ({ a: v }), 1);
assert(JSON.stringify(api.shape(deep)).split("{").length <= 12, "at most ten levels");

// the wrappers on a fake window
const tick = () => new Promise((r) => setTimeout(r, 20));
function fake(host, mode, read, at = "/usage/1234567?tab=x") {
  const calls = [], responses = {};
  class XHR {
    constructor() { this.on = []; this.responseType = ""; }
    open(method, url) { this.url = url; }
    addEventListener(e, f) { if (e === "load") this.on.push(f); }
    send() { const r = responses[this.url]; setTimeout(() => { Object.assign(this, { status: r.status, responseText: r.body, responseURL: new URL(this.url, W.location.href).href, type: r.type }); this.on.forEach((f) => f()); }, 1); }
    getResponseHeader() { return this.type; }
  }
  const origFetch = async (input) => {
    const url = typeof input === "string" ? input : input.url, r = responses[url];
    const res = new Response(r.body, { status: r.status, headers: { "content-type": r.type } });
    Object.defineProperty(res, "url", { value: new URL(url, W.location.href).href });
    return res;
  };
  const u = new URL(`https://${host}${at}`);
  // a page it must never draw on (a bar over it hid Mistral's own sign-in): any element it makes fails the check
  const document = { readyState: "complete", addEventListener() {}, createElement() { throw new Error("console.js drew on the page"); } };
  const W = { location: { protocol: "https:", hostname: host, href: u.href, pathname: u.pathname, origin: u.origin }, fetch: origFetch, XMLHttpRequest: XHR, document,
    __TAURI_INTERNALS__: { invoke: (cmd, args) => { calls.push([cmd, JSON.parse(JSON.stringify(args))]); return Promise.resolve(null); } } };
  install(W, { provider: "Qwen Cloud Token Plan", mode, sites: ["home.qwencloud.com"] }, read);
  return { W, calls, responses, origFetch };
}
const json = (b, status = 200) => ({ status, type: "application/json; charset=utf-8", body: JSON.stringify(b) });

// A text extractor for a typical layout (made-up labels until the page's own are known): the share used from "€12.40 of
// €150.00 used" (else "12% used"), the reset from the line that says it. Euros have no unit of the core's: a share.
const MISTRAL_EXAMPLE = String.raw`(t, p) => {
  var n = p.nums(p.line(t, /\bof\b.*\bused\b/i)), pct = p.pct(p.line(t, /%\s*used/i));
  var used = n.length >= 2 && n[1] > 0 ? 100 * n[0] / n[1] : pct;
  return used == null ? null : { left: null, total: null, unit: "percent", used_pct: used, resets: p.date(p.line(t, /\b(resets?|renews?)\b/i)) };
}`;

(async () => {
  // learn: one report per call and shape, never a value; HTML, another site and a bare number are not looked at
  const L = fake("home.qwencloud.com", "learn", null);
  Object.assign(L.responses, { "/api/usage?region=sg": json(body), "/page": { status: 200, type: "text/html", body: "<p>1500.25</p>" }, "https://evil.example/api": json(body),
    "/api/n": json(5), "https://home.qwencloud.com/api/me": json({ name: "someone@example.com" }), "/api/plan": json({ planName: "Pro plan" }) });
  for (const u of ["/api/usage?region=sg", "/api/usage?region=sg", "/page", "https://evil.example/api", "/api/n"]) await (await L.W.fetch(u)).text();
  await L.W.fetch(new Request("https://home.qwencloud.com/api/me", { method: "post" }));
  const x = new L.W.XMLHttpRequest();
  x.open("PATCH", "/api/plan");
  x.send();
  await tick();
  const learned = L.calls.map(([cmd, a]) => (assert.strictEqual(cmd, "plugin:console|report"), assert.strictEqual(a.provider, "Qwen Cloud Token Plan"), assert.strictEqual(a.kind, "learn"), a.payload));
  assert.deepStrictEqual(learned, [
    { path: "/api/usage?region", method: "GET", status: 200, shape: SHAPE, page: "https://home.qwencloud.com/usage/:id" },
    { path: "/api/me", method: "POST", status: 200, shape: { name: "string" } },
    { path: "/api/plan", method: "PATCH", status: 200, shape: { planName: "string" }, page: "https://home.qwencloud.com/usage/:id" },
  ]);
  clean(learned);
  // the page gets its own response, untouched
  assert.strictEqual(await (await L.W.fetch("/api/usage?region=sg")).text(), JSON.stringify(body));
  if (process.env.SHOW) console.log(JSON.stringify(learned[0]));

  // off the provider's sites (a sign-in on another host): nothing wrapped
  const off = fake("accounts.example.com", "learn", null);
  assert.strictEqual(off.W.fetch, off.origFetch);

  // read: the extractor's numbers on usage-like JSON alone, no shapes
  const R = fake("home.qwencloud.com", "read", (p, j) => (p === "/api/usage" ? { left: j.quota.remaining, total: j.quota.total, unit: "credits", used_pct: null, resets: null } : null));
  Object.assign(R.responses, { "/api/usage?region=sg": json(body), "/api/me": json({ name: "x" }) });
  await R.W.fetch("/api/me");
  await R.W.fetch("/api/usage?region=sg");
  await tick();
  assert.deepStrictEqual(R.calls.map(([, a]) => [a.kind, a.payload]), [["read", { numbers: { left: 1500.25, total: 2000, unit: "credits", used_pct: null, resets: null } }]]);
  // read with no extractor yet (Phase 1): nothing at all
  const none = fake("home.qwencloud.com", "read", null);
  none.responses["/api/usage"] = json(body);
  await none.W.fetch("/api/usage");
  await tick();
  assert.deepStrictEqual(none.calls, []);
  // read on a sign-in page: signed out at once
  const out = fake("home.qwencloud.com", "read", null, "/auth/login");
  assert.deepStrictEqual(out.calls.map(([, a]) => [a.kind, a.payload]), [["signed_out", null]]);
  text();
  console.log("console script ok");
})().catch((e) => { console.error(e); process.exitCode = 1; });


function text() {
  const P = api.parse;
  // the parser: money in en-US and en-GB/EU writing, € and $, shares, dates with and without a year (from Nov 27, 2026)
  const nums = { "€12.40": 12.4, "$1,234.56": 1234.56, "1.234,56 €": 1234.56, "€150": 150, "12,40 €": 12.4, "$0.125": 0.125, "1,234": 1234, "1.234": 1234,
    "1,234,567 tokens": 1234567, "€1.234.567,89": 1234567.89, "US$ 2,500.00": 2500, "nothing": null };
  for (const [s, n] of Object.entries(nums)) assert.strictEqual(P.num(s), n, s);
  assert.deepStrictEqual(P.nums("€12.40 of €150.00 used"), [12.4, 150]);
  assert.deepStrictEqual(P.nums("12,40 € von 150,00 € used"), [12.4, 150]);
  assert.deepStrictEqual(P.nums("$1,234.56 of $2,000 used"), [1234.56, 2000]);
  assert.deepStrictEqual([P.pct("12% used"), P.pct("12,5 % used"), P.pct("83.3% of your limit"), P.pct("no share")], [12, 12.5, 83.3, null]);
  const now = new Date(2026, 10, 27, 15, 30).getTime(), day = (y, m, d) => new Date(y, m, d).getTime() / 1000;
  const dates = { "Resets on Dec 1": day(2026, 11, 1), "Renews December 1, 2026": day(2026, 11, 1), "Renews on 1 December 2026": day(2026, 11, 1),
    "Resets 1st Dec.": day(2026, 11, 1), "Next billing date: 2027-01-15": day(2027, 0, 15), "Resets on Nov 27": day(2026, 10, 27), "Resets on Nov 20": day(2027, 10, 20),
    "Renews on 3 March": day(2027, 2, 3), "Renews on the 3rd of March 2027": day(2027, 2, 3), "Resets Sept 30": day(2027, 8, 30),
    "Resets in 3 days": Math.floor(now / 1000) + 3 * 86400, "Resets in 5 hours": Math.floor(now / 1000) + 5 * 3600,
    "Resets Feb 30": null, "decimal 5 used": null, "Marketing 12 credits": null, "no date": null };
  for (const [s, t] of Object.entries(dates)) assert.strictEqual(P.date(s, now), t, s);
  assert.strictEqual(P.line("Subscription\n  12% used  \nResets on Dec 1", /used/), "12% used");
  assert.strictEqual(P.line("Subscription", /used/), "");

  // learn: a made-up page's short usage-like lines, every digit "#"; none with an @ or a value (a key as written, a name
  // in capitals mixed); no prose, address or conversation (over 8 words, or no usage word: "of" is none); a long count
  // stays, masked; nothing past 160, 40 lines, once
  const PAGE = ["Subscription", "Le Chat Pro", "Included usage", "€12.40 of €150.00 used", "12% used", "Resets on Dec 1", "Renews December 1, 2026",
    "Billing email someone@example.com for 2 seats", "Your key sk-AbCdEfGhIjKl was used 3 times", "Key sk-proj-a1B2c3D4e5 used 3 times", "Invoice INV-2026-000123 paid €14.99",
    "Org AbCdEfGhIjKlMn uses 4 seats of 10", "Used 123456 tokens", "Member since 2024", "Need more? Upgrade your plan for €24.99/month", `${"Long text ".repeat(20)} 5 used`,
    "Jane Doe asked about the plan limits for 3 teams last week", "Jane Doe, 12 Main Street, Springfield", "Part 2 of the guide", "Thread 4 of 9",
    "€12.40   of\t€150.00 used", "١٢ credits left"].join("\n");
  const MASKED = ["€##.## of €###.## used", "##% used", "Resets on Dec #", "Renews December #, ####", "Invoice INV-####-###### paid €##.##", "Used ###### tokens",
    "Need more? Upgrade your plan for €##.##/month", "## credits left"];
  const lines = api.textLines(PAGE);
  assert.deepStrictEqual(lines, MASKED);
  for (const l of lines) assert(!/\p{N}/u.test(l) && !l.includes("@") && l.length <= 160 && l.split(" ").length <= 8, l);
  for (const s of ["someone", "sk-", "a#B", "AbCdEf", "Jane", "Main Street", "guide", "Thread", "2024", "12.40", "150"]) assert(!lines.join("|").includes(s), s);
  const many = Array.from({ length: 50 }, (_, i) => `${String.fromCharCode(65 + (i % 26), 65 + Math.floor(i / 26))} ${i} used`).join("\n");
  assert.strictEqual(api.textLines(many).length, 40);

  // the settle watcher on a fake clock: 1.5 s without a change, at 20 s at the latest, again after an address change
  function page(mode, readText, at = "/subscription") {
    let now = 0, id = 0;
    const timers = new Map(), calls = [], observers = [];
    const u = new URL(`https://admin.mistral.ai${at}`);
    const W = { location: { protocol: "https:", hostname: u.hostname, href: u.href, pathname: u.pathname, origin: u.origin },
      document: { body: { innerText: "Loading" }, createElement() { throw new Error("console.js drew on the page"); } },
      MutationObserver: class { constructor(f) { observers.push(f); } observe() {} },
      setTimeout: (f, ms) => (timers.set(++id, { f, at: now + ms }), id), clearTimeout: (i) => timers.delete(i),
      __TAURI_INTERNALS__: { invoke: (cmd, args) => { calls.push([cmd, JSON.parse(JSON.stringify(args))]); return Promise.resolve(null); } } };
    install(W, { provider: "Mistral Vibe", mode, sites: ["admin.mistral.ai"], start: "https://admin.mistral.ai/subscription" }, null, readText);
    const advance = (ms) => {
      for (const end = now + ms; ;) {
        const next = [...timers].filter(([, t]) => t.at <= end).sort((a, b) => a[1].at - b[1].at)[0];
        if (!next) { now = end; return; }
        timers.delete(next[0]);
        now = next[1].at;
        next[1].f();
      }
    };
    const change = (t) => { if (t != null) W.document.body.innerText = t; observers.forEach((f) => f([])); };
    const go = (to) => { const v = new URL(to, W.location.href); Object.assign(W.location, { href: v.href, pathname: v.pathname }); };
    return { W, calls, advance, change, go, sent: () => calls.map(([cmd, a]) => (assert.strictEqual(cmd, "plugin:console|report"), [a.kind, a.payload])) };
  }
  const L = page("learn", null);
  L.advance(1000);
  L.change(PAGE);
  L.advance(1400);
  assert.deepStrictEqual(L.sent(), [], "still settling");
  L.advance(200);
  const at = "https://admin.mistral.ai/subscription";
  assert.deepStrictEqual(L.sent(), [["text", { page: at, lines: MASKED }]]);
  // a page that keeps changing (a clock on it): its text at 20 s, then nothing more on that page; the same lines go once
  for (let t = 0; t < 30; t++) { L.change(t === 3 ? `${PAGE}\nCredits left: 3` : null); L.advance(1000); }
  assert.deepStrictEqual(L.sent().slice(1), [["text", { page: at, lines: [...MASKED, "Credits left: #"] }]]);
  L.change(`${PAGE}\nTokens used: 4`);
  L.advance(5000);
  assert.strictEqual(L.calls.length, 2, "past 20 s nothing more");
  // a single-page app's navigation: text on the start page alone (another page's lines are not learned), settled again on
  // coming back; its query is not part of the start page
  L.go("/usage?tab=x&email=a");
  L.change("Usage\n4 requests left");
  L.advance(30000);
  assert.strictEqual(L.calls.length, 2, "not the start page");
  L.go("/subscription?tab=plan");
  L.change("Subscription\n4 requests left");
  L.advance(1500);
  assert.deepStrictEqual(L.sent().slice(2), [["text", { page: at, lines: ["# requests left"] }]]);
  // a learn visit that opens on another page learns no text there
  const elsewhere = page("learn", null, "/usage");
  elsewhere.change(PAGE);
  elsewhere.advance(30000);
  assert.deepStrictEqual(elsewhere.calls, []);
  // read mode without a text extractor: no watcher, nothing
  const none = page("read", null);
  none.change(PAGE);
  none.advance(30000);
  assert.deepStrictEqual(none.calls, []);

  // read: the text extractor on the page's text once it settles, its numbers alone, once; one that throws says nothing
  const R = page("read", eval(MISTRAL_EXAMPLE));
  R.advance(1500);
  assert.deepStrictEqual(R.sent(), [], "a loading page has no numbers");
  R.change(PAGE);
  R.advance(1500);
  const resets = P.date("Resets on Dec 1");
  assert.deepStrictEqual(R.sent(), [["read", { numbers: { left: null, total: null, unit: "percent", used_pct: (100 * 12.4) / 150, resets } }]]);
  R.change(`${PAGE}\n`);
  R.advance(1500);
  assert.strictEqual(R.calls.length, 1, "one reading");
  // a reading keeps trying past 20 s (the window ends at 30 s): numbers that come at 25 s are read
  const late = page("read", eval(MISTRAL_EXAMPLE));
  for (let t = 0; t < 25; t++) { late.change(`Loading ${t}`); late.advance(1000); }
  assert.deepStrictEqual(late.calls, [], "loading");
  late.change(PAGE);
  late.advance(1500);
  assert.strictEqual(late.sent()[0][0], "read", "read at 26.5 s");
  // the share alone where the amounts are not there
  const S = page("read", eval(MISTRAL_EXAMPLE));
  S.change("Subscription\n12% used\nRenews December 1, 2030");
  S.advance(1500);
  assert.deepStrictEqual(S.sent(), [["read", { numbers: { left: null, total: null, unit: "percent", used_pct: 12, resets: new Date(2030, 11, 1).getTime() / 1000 } }]]);
  // the real readers (main.rs QWEN_TEXT, MISTRAL_VIBE_TEXT) on the pages' text as the users' screenshots show it (amounts
  // made up), each in both layouts innerText may give: a label and its value on one line or on two; and the variants the
  // review named: labels in capitals, a number grouped by a space, a credit pack's or a team plan's own "Remaining", euros
  const src = fs.readFileSync(path.join(__dirname, "../src-tauri/src/main.rs"), "utf8");
  const reader = (name) => eval(src.match(new RegExp(`const ${name}: &str = r#"([\\s\\S]*?)"#;`))[1]);
  const run = (fn, text) => fn(text, P);
  const QW = reader("QWEN_TEXT"), MV = reader("MISTRAL_VIBE_TEXT");
  const qwen1 = "Token Plan Usage\nIndividual Plan\nEssential\nLast updated 2026-11-27 23:52:29\nStatus\nActive\nRemaining days\n24 days\nExpiry date\n2026-12-22 17:00:00\nAuto-renew\nMonthly Quota\nReset time 2026-12-22 17:00:00\nRemaining 95.8%\nPlan quota 45,000\nCredit Pack\nNo Credit Packs";
  const qwen2 = qwen1.replace("Remaining 95.8%", "Remaining\n95.8%").replace("Plan quota 45,000", "Plan quota\n45,000").replace("Reset time 2026", "Reset time\n2026");
  const qAt = new Date(2026, 11, 22, 17, 0, 0).getTime() / 1000;
  for (const t of [qwen1, qwen2]) {
    const r = run(QW, t);
    assert.deepStrictEqual({ ...r, left: Math.round(r.left), used_pct: +r.used_pct.toFixed(1) }, { left: 43110, total: 45000, unit: "credits", used_pct: 4.2, resets: qAt }, "Qwen's quota");
  }
  assert.strictEqual(run(QW, "Token Plan Usage\nLoading"), null, "Qwen still loading");
  assert.deepStrictEqual(run(QW, "Individual Plan\nMonthly Quota\nRemaining 12%").unit, "percent", "Qwen without its plan quota: a share");
  const q = (t) => { const r = run(QW, t); return r && [Math.round(r.left), r.total, +r.used_pct.toFixed(1)]; };
  for (const sp of [" ", "\u00a0", "\u202f"]) assert.deepStrictEqual(q(qwen1.replace("45,000", `45${sp}000`)), [43110, 45000, 4.2], `45${sp}000`);
  assert.deepStrictEqual(q(qwen1.replace("Monthly Quota", "MONTHLY QUOTA").replace("Individual Plan", "INDIVIDUAL PLAN").replace("Plan quota", "PLAN QUOTA")), [43110, 45000, 4.2], "capitals");
  assert.deepStrictEqual(q(qwen1.replace("Remaining 95.8%\n", "").replace("No Credit Packs", "Starter pack\nRemaining 50%\nPlan quota 1,000")), null, "a credit pack's share is not the plan's");
  assert.deepStrictEqual(q(qwen1.replace("No Credit Packs", "Starter pack\nRemaining 50%\nPlan quota 1,000")), [43110, 45000, 4.2], "the plan's, not the pack's");
  const team = "Token Plan Usage\nTeam Plan\nMonthly Quota\nRemaining 10%\nPlan quota 900,000\n" + qwen1.slice(qwen1.indexOf("Individual Plan")) + "\nTeam Plan\nMonthly Quota\nRemaining 20%\nPlan quota 100,000";
  assert.deepStrictEqual(q(team), [43110, 45000, 4.2], "the Individual Plan between two teams'");
  assert.strictEqual(q("Token Plan Usage\nTeam Plan\nMonthly Quota\nRemaining 10%\nPlan quota 900,000"), null, "a Team tab alone: not the plan read");
  assert.deepStrictEqual(q(qwen1.replace("Plan quota 45,000", "Plan quota 45,000\n2026 total")), [43110, 45000, 4.2], "a number after the quota does not join it");
  const vibe1 = "Subscription\nCURRENT PLAN\nEducation\nCancelling\nYou will be downgraded on Dec 6, 2026.\nINCLUDED MONTHLY ALLOWANCE\nThese allowances are included with your plan.\nIncluded API usage\nIncluded allowance for API usage. Resets on the first day of each calendar month, independently of your subscription renewal.\n$2.99\n$15\nResets in 3 days\nIncluded Vibe & Studio usage\nIncluded allowance for Vibe & Studio. Resets on the first day of each calendar month, independently of your subscription renewal.\n$49.63\n$150\nResets in 3 days";
  const vibe2 = vibe1.replace("$49.63\n$150", "$49.63$150");
  // the reset: the block's "Resets in N days" from today's UTC date at 00:00 UTC; without it next month's first day, 00:00 UTC
  const d0 = new Date(), utc = (m, d) => Date.UTC(d0.getUTCFullYear(), d0.getUTCMonth() + m, d) / 1000;
  const in3 = utc(0, d0.getUTCDate() + 3), first = utc(1, 1);
  for (const t of [vibe1, vibe2, vibe1.replace("$49.63\n$150", "US$49.63\nUS$150"), vibe1.replace("$49.63\n$150", "US$ 49.63\n150 $")]) {
    const r = run(MV, t);
    assert.deepStrictEqual({ ...r, left: +r.left.toFixed(2), used_pct: +r.used_pct.toFixed(2) }, { left: 100.37, total: 150, unit: "usd", used_pct: 33.09, resets: in3 }, "Mistral Vibe's allowance, not the API's");
  }
  for (const t of [vibe1.replace("$49.63\n$150", "€49.63\n€150"), vibe1.replace("$49.63\n$150", "49,63 €\n150 €"), vibe1.replace("$49.63\n$150", "1.049,63 €\n1.500 €")]) {
    const r = run(MV, t);
    assert.deepStrictEqual({ ...r, used_pct: +r.used_pct.toFixed(2) }, { left: null, total: null, unit: "percent", used_pct: t.includes("1.049") ? 69.98 : 33.09, resets: in3 }, `euros: a share alone (${t.slice(-30)})`);
  }
  assert.strictEqual(run(MV, vibe1.replace(/Resets in 3 days$/, "Resets soon")).resets, first, "no day count: next month's first day");
  assert.strictEqual(run(MV, vibe1.slice(0, vibe1.lastIndexOf("\nResets"))).resets, first, "no reset line");
  assert.strictEqual(run(MV, "Subscription\nIncluded API usage\n$2.99\n$15"), null, "no Vibe allowance on the page");
  const T = page("read", () => { throw new Error("a page that changed"); });
  T.change(PAGE);
  T.advance(30000);
  assert.deepStrictEqual(T.calls, []);
}
