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
    await p.evaluate(([t, s]) => { if (s) localStorage.setItem("tazuna.setup", "1"); else { localStorage.removeItem("tazuna.setup"); sessionStorage.setItem("tz.firstRun", "1"); } localStorage.setItem("tazuna.theme", t); }, [theme, setup]);
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
    await step(p, `${T}-start-model-picker`, async () => { await p.getByTitle("Model and effort").click(); });
    await step(p, `${T}-start-model-search`, async () => { await p.keyboard.type("mis"); }); await esc(p);
    await step(p, `${T}-start-focus`, async () => { await p.locator("body").click({ position: { x: 700, y: 700 } }); for (let i = 0; i < 6; i++) await p.keyboard.press("Tab"); });
    await step(p, `${T}-usage-refresh`, async () => { await p.getByRole("button", { name: "Refresh usage" }).click(); await p.waitForTimeout(100); await p.getByRole("button", { name: "Refresh usage" }).hover(); await p.waitForTimeout(800); });
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
    // the chat's hand-off (2026-11-21): in a plain chat, answers the agent gave in the chat's place (live data, a page) under
    // their line, and an ordinary chat answer with "Send to agent" beside Copy on hover
    await step(p, `${T}-handoff`, async () => { await open(p, "Will it rain in Berlin"); await p.mouse.move(640, 300); await p.mouse.wheel(0, -900); });
    await step(p, `${T}-handoff-send`, async () => { await p.mouse.wheel(0, 900); await p.getByText("Dune", { exact: true }).hover(); });
    log.push(`${theme} hand-off: ${await p.evaluate(() => `${[...document.querySelectorAll("span")].filter((e) => e.textContent.trim().startsWith("answered by the agent")).length} agent tags, ${[...document.querySelectorAll("button")].filter((b) => b.textContent === "Send to agent").length} send-to-agent`)} (2 tags, 1 send wanted)`);
    // the message box: the `/` and `@` menus, attachments as chips, and an answer that names a picture (desktop 0.1.87)
    const msg = p.locator("textarea[aria-label=Message]");
    await step(p, `${T}-slash`, async () => { await msg.fill("/"); await p.waitForTimeout(300); });
    await step(p, `${T}-at`, async () => { await msg.fill("Why does @ret"); await p.waitForTimeout(300); });
    await step(p, `${T}-attach`, async () => { await msg.fill("What is wrong in this screenshot?"); await p.getByTitle(/Attach files/).click(); });
    await step(p, `${T}-image`, async () => { await open(p, "Speed up the CI build"); await p.mouse.move(640, 300); });
    // a subagent working in the background: its row over the message box, then its own conversation beside this one (0.1.92)
    await step(p, `${T}-subagent`, async () => { await p.getByRole("button", { name: /Subagent in the background/ }).click(); await p.waitForTimeout(600); });
    await p.getByRole("button", { name: "Close", exact: true }).click().catch(() => {});
    await open(p, "Why is the CI build");
    await step(p, `${T}-mode-menu-thread`, async () => { await p.getByTitle("Permission mode (shift-tab)").click(); }); await esc(p);
    await step(p, `${T}-palette`, async () => { await p.keyboard.press("Control+k"); await p.keyboard.type("fix"); }); await esc(p);
    await step(p, `${T}-shortcuts`, async () => { await p.keyboard.press("Control+/"); }); await esc(p);
    await step(p, `${T}-settings`, async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(600); });
    for (const t of ["Providers", "Tools from MCP servers", "Panels and plugins"]) await step(p, `${T}-settings-${t.split(" ")[0].toLowerCase()}`, async () => { await p.getByRole("button", { name: t, exact: true }).click(); });
    await step(p, `${T}-settings-key`, async () => { await p.getByRole("button", { name: "Providers", exact: true }).click(); await p.getByRole("button", { name: /Replace key|Add key/ }).first().click(); });
    await p.getByRole("button", { name: "Appearance", exact: true }).click();
    await esc(p);
    await step(p, `${T}-usage`, async () => { await p.keyboard.press("Control+Shift+U"); await p.waitForTimeout(800); });
    await step(p, `${T}-usage-contributing`, async () => { await p.getByText("What's contributing to your Claude usage").click(); });
    for (let i = 1; i <= 3; i++) await step(p, `${T}-usage-${i}`, async () => { await p.mouse.move(640, 450); await p.mouse.wheel(0, 600); });
    await esc(p);
    log.push(`${theme} page errors: ${JSON.stringify(p.errs.slice(0, 5))}`);
    await ctx.close();
  }
  // desktop 0.1.97 (design review 11, the look): the start page's ledger and a day without a run, an answer's receipt (on
  // the plan and paid), the conversation's from the status bar, the empty work panel's tab and the panel opened from it,
  // the transcript's faded edges, and the dialogs the first loop does not open
  for (const theme of ["dark", "light"]) {
    const [ctx, p] = await ctxOf({ theme }), T = theme[0];
    await open(p, "Why is the CI build");
    await step(p, `${T}-receipt-line`, async () => { await p.mouse.move(640, 400); await p.mouse.wheel(0, 9000); });
    await step(p, `${T}-receipt-open`, async () => { await p.getByTitle(/^The receipt/).first().click(); await p.waitForTimeout(500); });
    // the same conversation's second turn ran on another model, paid to a provider: its own slip says so, and the
    // conversation's slip has the plan's part and the provider's on a line each
    await step(p, `${T}-receipt-mixed`, async () => { await p.getByTitle(/^The receipt/).nth(1).click(); await p.waitForTimeout(500); await p.mouse.move(640, 400); await p.mouse.wheel(0, 9000); });
    await step(p, `${T}-receipt-chat`, async () => { await p.getByTitle(/^This conversation's receipt/).click(); }); await esc(p);
    await step(p, `${T}-panel-tab`, async () => { await p.mouse.move(640, 400); await p.mouse.wheel(0, -9000); });
    await step(p, `${T}-panel-open`, async () => { await p.getByTitle(/^The work panel: nothing/).click(); await p.waitForTimeout(500); });
    const col = () => p.evaluate(() => `${document.querySelector('[style*="grid-template-columns"]').style.gridTemplateColumns.split(" ").pop()} column, ${document.querySelectorAll('button[aria-label^="The work panel"]').length} tab`);
    log.push(`${theme} panel opened by its tab: ${await col()} (300px, 0 tab wanted)`);
    await step(p, `${T}-receipt-paid`, async () => { await open(p, "Speed up the CI build"); await p.waitForTimeout(300); await p.getByTitle(/^The receipt/).first().click(); await p.waitForTimeout(500); });
    log.push(`${theme} then another empty project conversation: ${await col()} (0px, 1 tab wanted)`);
    await step(p, `${T}-edges`, async () => { await open(p, "Question 1"); await p.mouse.move(640, 400); await p.mouse.wheel(0, -700); });
    log.push(`${theme} then a plain chat: ${await col()} (0px, 0 tab wanted)`);
    // a turn saved before the core said how it was paid: its total alone, no price per call; and the conversation's slip
    // while a reply runs, with a row for what is not a finished turn yet
    await step(p, `${T}-receipt-old`, async () => { await open(p, "Fix the flaky retry test"); await p.getByTitle(/^The receipt/).first().click(); await p.waitForTimeout(500); });
    await step(p, `${T}-receipt-chat-running`, async () => { await p.getByTitle(/^This conversation's receipt/).click(); }); await esc(p);
    // the segmented control by keyboard: Tab stops on the chosen key, the arrows move and choose
    await step(p, `${T}-segmented-keys`, async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(600); await p.getByRole("radio", { name: "Serif" }).focus(); await p.keyboard.press("ArrowRight"); });
    log.push(`${theme} segmented by keyboard: focus on ${await p.evaluate(() => `${document.activeElement?.textContent} (checked ${document.activeElement?.getAttribute("aria-checked")}), ${document.querySelectorAll('[role=radiogroup][aria-label=Answers] [tabindex="0"]').length} tab stop`)} (Sans, true, 1 wanted)`);
    await p.keyboard.press("ArrowLeft"); await esc(p);
    await open(p, "Why is the CI build");
    for (const [n, q] of [["bestof", "Try on several"], ["pr", "Pull request"]]) {
      await step(p, `${T}-dialog-${n}`, async () => { await p.keyboard.press("Control+k"); await p.waitForTimeout(250); await p.keyboard.type(q); await p.waitForTimeout(250); await p.keyboard.press("Enter"); await p.waitForTimeout(700); }); await esc(p); await esc(p); }
    log.push(`${theme} look page errors: ${JSON.stringify(p.errs.slice(0, 5))}`);
    await ctx.close();
    const [ctx2, q] = await ctxOf({ theme, extra: "window.__NO_TODAY__ = true;" });
    await step(q, `${T}-start-no-runs-today`, async () => {});
    await ctx2.close();
    // gh answers with an error (not installed, no remote): nothing to show, so the tab, not an empty panel
    const [ctx3, r] = await ctxOf({ theme, extra: "window.__PR_ERROR__ = true;" });
    await step(r, `${T}-pr-error-tab`, async () => { await open(r, "Why is the CI build"); });
    log.push(`${theme} gh's error: ${await r.evaluate(() => `${document.querySelector('[style*="grid-template-columns"]').style.gridTemplateColumns.split(" ").pop()} column, ${document.querySelectorAll('button[aria-label^="The work panel"]').length} tab`)} (0px, 1 tab wanted)`);
    await ctx3.close();
  }
  { const [ctx, p] = await ctxOf({ w: 760, h: 640 });
    await step(p, "w760-receipt-open", async () => { await p.getByRole("button", { name: "Conversations", exact: true }).click(); await open(p, "Why is the CI build"); await p.getByTitle(/^The receipt/).first().click(); await p.waitForTimeout(500); });
    await step(p, "w760-receipt-chat", async () => { await p.getByTitle(/^This conversation's receipt/).click(); });
    await ctx.close(); }
  for (const [w, h] of [[873, 545], [768, 480]]) {
    const [ctx, p] = await ctxOf({ w, h });
    await step(p, `z${w}-start`, async () => {});
    await step(p, `z${w}-receipt-open`, async () => { await p.getByRole("button", { name: "Conversations", exact: true }).click(); await open(p, "Why is the CI build"); await p.getByTitle(/^The receipt/).first().click(); await p.waitForTimeout(500); });
    await step(p, `z${w}-receipt-chat`, async () => { await p.getByTitle(/^This conversation's receipt/).click(); });
    log.push(`z${w} the conversation's slip: ${await p.evaluate(() => { const r = document.querySelector('[role=dialog][aria-label="This conversation\'s receipt"]').getBoundingClientRect(); return `y ${Math.round(r.top)}..${Math.round(r.bottom)} of ${innerHeight}, x ${Math.round(r.left)}..${Math.round(r.right)} of ${innerWidth}${r.top < 0 || r.left < 0 || r.right > innerWidth ? " CUT" : ""}`; })}`);
    await ctx.close(); }
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
  // a core that answers late, and a project opened before it did: the lists were never fetched (desktop 0.1.76)
  { const slow = `(() => { const go = () => { const T = window.__TAURI_INTERNALS__; if (!T) return setTimeout(go, 0); const v = T.view.bind(T); T.view = (tid, m) => setTimeout(() => v(tid, m), 700); }; go(); })();`;
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, colorScheme: "dark", userAgent: "Mozilla/5.0 (X11; Linux x86_64) Chrome/140" });
    await ctx.addInitScript(MOCK); await ctx.addInitScript(slow);
    const p = await ctx.newPage();
    await p.goto(BASE); await p.evaluate(() => localStorage.setItem("tazuna.setup", "1")); await p.reload(); await p.waitForTimeout(150);
    await step(p, "project-opened-early", async () => { await side(p).locator('button[title="/Users/x/api-gateway"]').click(); await p.waitForTimeout(2500); });
    log.push(`project opened early: ${await side(p).locator("a").count()} conversations in the sidebar (some wanted)`);
    await ctx.close(); }
  // the core does not start: its reason and Try again, no demo numbers around it (desktop 0.1.77)
  { const fail = `(() => { const go = () => { const T = window.__TAURI_INTERNALS__; if (!T) return setTimeout(go, 0); const inv = T.invoke.bind(T); T.invoke = async (c, a) => { if (c === "start") throw "could not start tazuna app-server: No such file or directory (os error 2)"; return inv(c, a); }; }; go(); })();`;
    const [ctx, p] = await ctxOf({ extra: fail });
    await step(p, "core-fails", async () => { await p.waitForTimeout(600); });
    log.push(`core fails: ${await p.getByText("tazuna could not start").count() ? "failure screen" : "NO failure screen"}, ${await p.getByText("Retry test fails on CI").count() ? "DEMO TITLE SHOWN" : "no demo title"}`);
    await ctx.close(); }
  // some seventy providers (core 0.1.196): yours first, the rest folded under More providers and searched, their terms quiet
  // or in the warning colour; a model whose provider has no key opens Settings on that provider, its key field open
  for (const theme of ["dark", "light"]) {
    const [ctx, p] = await ctxOf({ theme }), T = theme[0];
    await step(p, `${T}-providers`, async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(600); await p.getByRole("button", { name: "Providers", exact: true }).click(); });
    await step(p, `${T}-providers-more`, async () => { await p.getByRole("button", { name: /^More providers/ }).click(); });
    await step(p, `${T}-providers-search`, async () => { await p.getByLabel("Search providers").fill("coding plan"); });
    await step(p, `${T}-providers-search-quiet`, async () => { await p.getByLabel("Search providers").fill("opencode"); });
    await esc(p);
    await step(p, `${T}-picker-no-key`, async () => { await p.getByTitle("Model and effort").click(); await p.keyboard.type("cerebras"); });
    await step(p, `${T}-picker-no-key-settings`, async () => { await p.getByText("No key yet · add one in Settings").first().click(); await p.waitForTimeout(600); });
    log.push(`${theme} no key to Settings: focus on ${await p.evaluate(() => document.activeElement?.getAttribute("aria-label"))} (Cerebras key wanted)`);
    await esc(p); await esc(p);
    await step(p, `${T}-picker-caution`, async () => { await p.getByTitle("Model and effort").click(); await p.keyboard.type("glm"); });
    await esc(p); await esc(p);
    // sign-ins (core oauth.rs, 2026-11-22): a not-signed-in plan in the picker opens Settings on it; a sign-in that sends
    // another app's identity asks once, with its warning; its code to type; ChatGPT's own flow, its welcome once, then
    // signed in under Yours and its models in the picker
    await step(p, `${T}-picker-sign-in`, async () => { await p.getByTitle("Model and effort").click(); await p.keyboard.type("copilot"); });
    await step(p, `${T}-sign-in-from-picker`, async () => { await p.getByText("Not signed in · sign in in Settings").first().click(); await p.waitForTimeout(600); });
    log.push(`${theme} not signed in to Settings: focus on ${await p.evaluate(() => document.activeElement?.dataset.signIn)} (GitHub Copilot wanted)`);
    await step(p, `${T}-sign-in-warning`, async () => { await p.locator('[data-sign-in="GitHub Copilot"]').click(); });
    await step(p, `${T}-sign-in-code`, async () => { await p.getByRole("button", { name: "I understand, sign in" }).click(); await p.waitForTimeout(300); });
    await step(p, `${T}-sign-in-chatgpt`, async () => { await p.getByLabel("Search providers").fill("chatgpt"); });
    await step(p, `${T}-sign-in-welcome`, async () => { await p.getByRole("button", { name: "Continue with ChatGPT" }).click(); await p.waitForTimeout(1000); });
    await step(p, `${T}-signed-in`, async () => { await p.getByRole("button", { name: "Got it" }).click(); await p.waitForTimeout(300); await p.getByRole("button", { name: /^More providers/ }).click(); });
    log.push(`${theme} signed in: ${await p.getByRole("button", { name: "Sign out" }).count()} sign-out, ${await p.getByRole("button", { name: "Other account" }).count()} other-account button (1, 1 wanted)`);
    await esc(p);
    await step(p, `${T}-picker-signed-in`, async () => { await p.getByTitle("Model and effort").click(); await p.keyboard.type("gpt-6"); });
    await esc(p); await esc(p);
    log.push(`${theme} providers page errors: ${JSON.stringify(p.errs.slice(0, 5))}`);
    await ctx.close();
  }
  // ChatGPT signed in with plan use declined at its consent page: said in the warning colour, "Allow plan use" beside Sign out
  { const off = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { if (p?.msg?.type === "settings") p.msg.providers = p.msg.providers.map((x) => x.name === "ChatGPT" ? { ...x, key: "signed in, plan use not allowed", plan_off: true } : x); return emit(e, p); }; })();`;
    const [ctx, p] = await ctxOf({ extra: off });
    await step(p, "d-sign-in-plan-off", async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(600); await p.getByRole("button", { name: "Providers", exact: true }).click(); });
    log.push(`plan off: ${await p.getByRole("button", { name: "Allow plan use" }).count()} allow button (1 wanted)`);
    await ctx.close(); }
  // a fresh install: no key yet, nothing running here
  { const none = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { if (p?.msg?.type === "settings") p.msg.providers = p.msg.providers.filter((x) => !x.local).map((x) => ({ ...x, key: "" })); return emit(e, p); }; })();`;
    const [ctx, p] = await ctxOf({ extra: none });
    await step(p, "d-providers-none", async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(600); await p.getByRole("button", { name: "Providers", exact: true }).click(); });
    await ctx.close(); }
  // the model picker's search takes the typing as it opens (0.1.74 to 0.1.76 it did not)
  { const [ctx, p] = await ctxOf({});
    await p.getByTitle("Model and effort").click(); await p.waitForTimeout(400);
    log.push(`picker focus: ${await p.evaluate(() => document.activeElement?.getAttribute("placeholder") || document.activeElement?.tagName)}`);
    await ctx.close(); }
  { const [ctx, p] = await ctxOf({ setup: false }); await step(p, "first-run-setup", async () => { await p.waitForTimeout(600); }); await ctx.close(); }
  fs.writeFileSync(`${OUT}/log.txt`, log.join("\n"));
  console.log(log.filter((l) => !l.startsWith("ok")).join("\n") || "every step ok", `· ${log.length} steps in ${OUT}`);
  await browser.close();
})();
