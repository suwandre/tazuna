// The console reader's script (src-tauri/src/console.js) outside a window: its sanitizer on made-up JSON full of values,
// then its fetch and XMLHttpRequest wrappers on fake responses, in learn and read mode, on and off the provider's sites.
//   node review/console.cjs
const fs = require("fs"), path = require("path"), assert = require("assert");
// eval: the script is a function expression (main.rs appends its call), loaded here as the webview loads it
const install = eval(fs.readFileSync(path.join(__dirname, "../src-tauri/src/console.js"), "utf8"));
const api = install(null, null, null);
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
  const W = { location: { protocol: "https:", hostname: host, href: u.href, pathname: u.pathname, origin: u.origin }, fetch: origFetch, XMLHttpRequest: XHR,
    __TAURI_INTERNALS__: { invoke: (cmd, args) => { calls.push([cmd, JSON.parse(JSON.stringify(args))]); return Promise.resolve(null); } } };
  install(W, { provider: "Qwen Cloud Token Plan", mode, sites: ["home.qwencloud.com"] }, read);
  return { W, calls, responses, origFetch };
}
const json = (b, status = 200) => ({ status, type: "application/json; charset=utf-8", body: JSON.stringify(b) });

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
  console.log("console script ok");
})().catch((e) => { console.error(e); process.exitCode = 1; });
