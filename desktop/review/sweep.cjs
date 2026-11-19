// The design sweep: the desktop app on test data (review/mock.js), every screen, menu and state worth a look, in dark and
// light and at three window sizes, as PNGs to go through before a release. Design review 4 (2026-11-19) found a menu cut
// off and a raw error that the main screens alone had not shown.
//
//   npm run dev                                   # the app on localhost:1420
//   node review/sweep.cjs [http://localhost:1420] [out dir]   # needs playwright: npx playwright install chromium
//
// BASE may be a built copy too (vite build --base ./, with review/mock.js loaded before the app).
const { chromium } = require(process.env.PW || "playwright");
const fs = require("fs"), path = require("path");
const BASE = process.argv[2] || "http://localhost:1420", OUT = process.argv[3] || "/tmp/tazuna-sweep";
const MOCK = fs.readFileSync(path.join(__dirname, "mock.js"), "utf8");
fs.mkdirSync(OUT, { recursive: true });
const log = [];
(async () => {
  const browser = await chromium.launch();
  const ctxOf = async ({ theme = "dark", w = 1280, h = 800, setup = true, extra = "" } = {}) => {
    // a Linux agent: on a Mac one the app leaves its shortcuts to the native menus, which a browser does not have
    const ctx = await browser.newContext({ viewport: { width: w, height: h }, colorScheme: theme, userAgent: "Mozilla/5.0 (X11; Linux x86_64) Chrome/140" });
    await ctx.addInitScript(MOCK);
    if (extra) await ctx.addInitScript(extra);
    const p = await ctx.newPage(); p.errs = [];
    p.on("pageerror", (e) => p.errs.push(String(e)));
    await p.goto(BASE);
    await p.evaluate(([t, s]) => { if (s) localStorage.setItem("tazuna.setup", "1"); else localStorage.removeItem("tazuna.setup"); localStorage.setItem("tazuna.theme", t); }, [theme, setup]);
    await p.reload(); await p.waitForTimeout(1100);
    return [ctx, p];
  };
  const step = async (p, name, fn) => {
    try { await fn(); await p.waitForTimeout(350); await p.screenshot({ path: `${OUT}/${name}.png` }); log.push(`ok ${name}`); }
    catch (e) { log.push(`FAIL ${name}: ${String(e).split("\n")[0]}`); }
  };
  const side = (p) => p.locator("aside[aria-label=Conversations]");
  const open = async (p, title) => { await side(p).locator(`a[title^="${title}"]`).first().click({ timeout: 4000 }); await p.waitForTimeout(800); };
  const esc = async (p) => { await p.keyboard.press("Escape"); await p.waitForTimeout(250); };
  for (const theme of ["dark", "light"]) {
    const [ctx, p] = await ctxOf({ theme }), T = theme[0];
    await step(p, `${T}-start`, async () => {});
    await step(p, `${T}-start-where-menu`, async () => { await p.getByTitle(/Where it runs/).click(); }); await esc(p);
    await step(p, `${T}-start-mode-menu`, async () => { await p.getByTitle("Permission mode (shift-tab)").click(); }); await esc(p);
    await step(p, `${T}-start-model-picker`, async () => { await p.getByTitle("Model and effort").click(); });
    await step(p, `${T}-start-model-search`, async () => { await p.keyboard.type("mis"); }); await esc(p);
    await step(p, `${T}-start-focus`, async () => { await p.locator("body").click({ position: { x: 700, y: 700 } }); for (let i = 0; i < 6; i++) await p.keyboard.press("Tab"); });
    await step(p, `${T}-sidebar-hover`, async () => { await side(p).locator('a[title="Names for a grey cat"]').hover(); });
    await step(p, `${T}-thread-busy`, async () => { await open(p, "Fix the flaky retry test"); });
    await step(p, `${T}-workfold-open`, async () => { await p.getByText(/^Worked \d+ s/).first().click(); });
    await step(p, `${T}-answer-hover`, async () => { await p.getByText("The test waits a fixed 100 ms").hover(); });
    await step(p, `${T}-review`, async () => { await p.getByTitle(/Go through this turn/).click(); await p.waitForTimeout(500); }); await esc(p);
    await step(p, `${T}-side-chat`, async () => { await p.getByRole("button", { name: "Side chat", exact: true }).click(); await p.waitForTimeout(500); });
    await p.getByRole("button", { name: "Side chat", exact: true }).click().catch(() => {});
    await step(p, `${T}-asks`, async () => { await open(p, "Bump the TLS library"); });
    await step(p, `${T}-failed`, async () => { await side(p).locator('a[title="test"]').first().click(); await p.waitForTimeout(800); });
    await step(p, `${T}-long`, async () => { await open(p, "Question 1"); await p.mouse.move(640, 400); await p.mouse.wheel(0, -900); });
    await step(p, `${T}-markdown`, async () => { await open(p, "Why is the CI build"); });
    await step(p, `${T}-mode-menu-thread`, async () => { await p.getByTitle("Permission mode (shift-tab)").click(); }); await esc(p);
    await step(p, `${T}-palette`, async () => { await p.keyboard.press("Control+k"); await p.keyboard.type("fix"); }); await esc(p);
    await step(p, `${T}-shortcuts`, async () => { await p.keyboard.press("Control+/"); }); await esc(p);
    await step(p, `${T}-settings`, async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(600); });
    for (let i = 1; i <= 3; i++) await step(p, `${T}-settings-${i}`, async () => { await p.mouse.move(640, 450); await p.mouse.wheel(0, 600); });
    await esc(p);
    await step(p, `${T}-usage`, async () => { await p.keyboard.press("Control+Shift+U"); await p.waitForTimeout(800); });
    await step(p, `${T}-usage-contributing`, async () => { await p.getByText("What's contributing to your Claude usage").click(); });
    for (let i = 1; i <= 3; i++) await step(p, `${T}-usage-${i}`, async () => { await p.mouse.move(640, 450); await p.mouse.wheel(0, 600); });
    await esc(p);
    log.push(`${theme} page errors: ${JSON.stringify(p.errs.slice(0, 5))}`);
    await ctx.close();
  }
  for (const [w, h] of [[980, 720], [760, 640], [1680, 1050]]) {
    const [ctx, p] = await ctxOf({ w, h });
    await step(p, `w${w}-start`, async () => {});
    if (w < 1000) await step(p, `w${w}-nav`, async () => { await p.getByRole("button", { name: "Conversations", exact: true }).click(); });
    await step(p, `w${w}-markdown`, async () => { await open(p, "Why is the CI build"); });
    await ctx.close();
  }
  // a new user's window: tall, one project, no saved conversation (the sidebar shrank to its rows and the start page sat
  // high, desktop 0.1.75)
  { const none = `(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T); T.invoke = async (c, a) => { if (c === "op" && a.msg.op === "sessions") { setTimeout(() => T.emit("tazuna-view", { tid: a.tid, msg: { type: "sessions", dirs: [] } }), 20); return null; } return inv(c, a); }; })();`;
    const [ctx, p] = await ctxOf({ w: 1700, h: 1300, extra: none });
    await step(p, "new-user-tall", async () => {});
    log.push(`new-user sidebar ${await p.evaluate(() => Math.round(document.querySelector("aside[aria-label=Conversations]").getBoundingClientRect().width))} px (248 wanted)`);
    await ctx.close(); }
  { const [ctx, p] = await ctxOf({ setup: false }); await step(p, "first-run-setup", async () => { await p.waitForTimeout(600); }); await ctx.close(); }
  fs.writeFileSync(`${OUT}/log.txt`, log.join("\n"));
  console.log(log.filter((l) => !l.startsWith("ok")).join("\n") || "every step ok", `· ${log.length} steps in ${OUT}`);
  await browser.close();
})();
