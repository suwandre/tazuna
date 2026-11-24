// The design sweep: the desktop app on test data (review/mock.js), every screen, menu and state worth a look, in dark and
// light and at three window sizes, as PNGs to go through before a release. Design review 4 (2026-11-19) found a menu cut
// off and a raw error that the main screens alone had not shown.
//
//   npm run dev                                   # the app on localhost:1420
//   node review/sweep.cjs [http://localhost:1420] [out dir]   # needs playwright: npx playwright install chromium
//   ENGINE=webkit node review/sweep.cjs …         # in WebKit (the Mac app's engine); ONLY=rows: the window's rows alone
//
// BASE may be a built copy too (vite build --base ./, with review/mock.js loaded before the app).
const chromium = require(process.env.PW || "playwright")[process.env.ENGINE || "chromium"];
const fs = require("fs"), path = require("path");
const BASE = process.argv[2] || "http://localhost:1420", OUT = process.argv[3] || "/tmp/tazuna-sweep";
const MOCK = fs.readFileSync(path.join(__dirname, "mock.js"), "utf8");
// Tauri's drag script (tauri 2.12.5, src/window/scripts/drag.js) as far as the page sees it: a press on a drag region stops at
// the document
const DRAG = `document.addEventListener("mousedown", (e) => { for (const el of e.composedPath()) { if (!(el instanceof HTMLElement)) continue; const a = el.getAttribute("data-tauri-drag-region");
  const click = /^(A|BUTTON|INPUT|SELECT|TEXTAREA|LABEL|SUMMARY)$/.test(el.tagName) || (el.hasAttribute("tabindex") && el.getAttribute("tabindex") !== "-1");
  if (click && a === null) return; if (a === null) continue; if (a === "false") return; if (a === "deep" || el === e.composedPath()[0]) { e.preventDefault(); e.stopImmediatePropagation(); } return; } });`;
// the traffic lights, drawn where tauri.conf.json's trafficLightPosition ({x: 20, y: 30}) puts them in the real window, as
// measured there on macOS 26 (design review 12): 13 px, 23 px apart, x 20..80, centred on y 28
const LIGHTS = `;addEventListener("DOMContentLoaded", () => { const d = document.createElement("div"); d.style.cssText = "position:fixed;left:20.5px;top:21.5px;z-index:99;display:flex;gap:10px;pointer-events:none";
  d.innerHTML = ["#ff5f57", "#febc2e", "#28c840"].map((c) => \`<i style="width:13px;height:13px;border-radius:50%;background:\${c};box-shadow:inset 0 0 0 .5px #0003"></i>\`).join(""); document.body.append(d); });`;
fs.mkdirSync(OUT, { recursive: true });
const log = [];
(async () => {
  const browser = await chromium.launch();
  const ctxOf = async ({ theme = "dark", w = 1280, h = 800, setup = true, extra = "", mac = false } = {}) => {
    // a Linux agent: on a Mac one the app leaves its shortcuts to the native menus, which a browser does not have. `mac`:
    // a Mac's agent all the same, for the title bar's row (the overlay's traffic lights drawn where the real window has them)
    const ctx = await browser.newContext({ viewport: { width: w, height: h }, colorScheme: theme, userAgent: "Mozilla/5.0 (X11; Linux x86_64) Chrome/140" });
    await ctx.addInitScript(mac ? MOCK.replace("X11; Linux x86_64) Chrome/140", "Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15") + (process.env.LIGHTS === "0" ? "" : LIGHTS) : MOCK);
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
  // design review 12 (A2 and B3 of mockups/v9.html): the window's one top row, and the work panel open beside the reading
  // column (it keeps its left edge) or, where that leaves under 580 px of text, over the conversation; at the window's sizes
  // (960 is its least; 760 for the browser), on a Mac's agent, and on Linux's where there are no traffic lights
  const at = (p) => p.evaluate(() => {
    const r = (e) => e && e.getBoundingClientRect(), main = r(document.querySelector("main")), msg = r(document.querySelector("[data-msg]")), box = r(document.querySelector("[data-composer]"));
    const head = r(document.querySelector("main > header")), brand = r(document.querySelector("aside[aria-label=Conversations] > div")), pop = r(document.querySelector("div[data-panel]"));
    const rail = r(document.querySelector("aside[aria-label='The work']"));
    return `header ${head ? `${Math.round(head.top)}..${Math.round(head.bottom)} (row ${(head.top + head.height / 2 - 0.5).toFixed(1)})` : "none"}, brand row ${brand ? (brand.top + 8 + (brand.height - 8) / 2).toFixed(1) : "none"}`
      + `, column ${msg ? `${Math.round(msg.left - main.left)} in, ${Math.round(msg.width - 56)} text` : "none"}, box ${box ? `${Math.round(box.left - main.left)} in, ${Math.round(box.width)}` : "none"}`
      + `, panel ${pop ? "over" : rail && rail.width > 0 && main.right < innerWidth - 100 ? "beside" : "shut"}`;
  });
  const rows = [];
  for (const [mac, theme, w, h] of [true, false].flatMap((mac) => ["dark", "light"].flatMap((theme) => (mac ? [[1440, 900], [1280, 800], [960, 600], [760, 640]] : [[1440, 900], [960, 600]]).map(([w, h]) => [mac, theme, w, h])))) {
    const [ctx, p] = await ctxOf({ theme, w, h, mac }), N = `rows-${mac ? "mac" : "linux"}-${theme[0]}-${w}`;
    const nav = async () => { if (w < 1000) { await p.getByRole("button", { name: "Conversations", exact: true }).click(); await p.waitForTimeout(250); } };
    await step(p, `${N}-start`, async () => {});
    await step(p, `${N}-shut`, async () => { await nav(); await open(p, "Why is the CI build"); });
    rows.push(`${N} shut: ${await at(p)}`);
    // where a press drags the window, by Tauri's own rule (its drag.js: an element marked "deep" covers what is in it but a
    // control; a bare mark only itself)
    rows.push(`${N} drags: ${await p.evaluate((w) => {
      const drags = (el) => { for (let e = el; e; e = e.parentElement) { const a = e.getAttribute("data-tauri-drag-region"), click = /^(A|BUTTON|INPUT|SELECT|TEXTAREA|LABEL|SUMMARY)$/.test(e.tagName) || (e.hasAttribute("tabindex") && e.getAttribute("tabindex") !== "-1");
        if (click && a === null) return false; if (a === null) continue; if (a === "false") return false; if (a === "deep") return true; return e === el; } return false; };
      const title = document.querySelector("main > header h1").getBoundingClientRect(), side = document.querySelector("main > header button[aria-label='Side chat'], main > header button[aria-pressed]").getBoundingClientRect();
      const pts = { title: [title.left + 10, 28], "header's gap": [title.right + 40, 28], "desk over the sheet": [w / 2, 3], "header's button": [side.left + side.width / 2, 28] };
      if (w >= 1000) Object.assign(pts, { brand: [125, 28], search: [100, 74] });
      return Object.entries(pts).map(([k, [x, y]]) => `${k} ${drags(document.elementFromPoint(x, y)) ? "yes" : "no"}`).join(", ");
    }, w)}`);
    await step(p, `${N}-open`, async () => { await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(600); });
    rows.push(`${N} open: ${await at(p)}`);
    // over the conversation: Esc shuts it; beside: its button
    if (await p.locator("div[data-panel]").count()) { await esc(p); rows.push(`${N} after Esc: ${await at(p)}`); }
    else await p.getByTitle(/^Close the work panel/).click();
    await p.waitForTimeout(300);
    await step(p, `${N}-long`, async () => { await nav(); await open(p, "Question 1"); await p.mouse.move(w / 2, h / 2); await p.mouse.wheel(0, -900); });
    await step(p, `${N}-long-open`, async () => { await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(600); });
    rows.push(`${N} long, open: ${await at(p)}`);
    if (await p.locator("div[data-panel]").count()) { await p.mouse.click(w / 3, h / 2); await p.waitForTimeout(300); rows.push(`${N} after a click outside: ${await at(p)}`); }
    else await p.getByTitle(/^Close the work panel/).click();
    await step(p, `${N}-review`, async () => { await nav(); await open(p, "Fix the flaky retry test"); await p.getByTitle(/Go through this turn/).first().click(); await p.waitForTimeout(500); });
    await esc(p);
    await step(p, `${N}-settings`, async () => { await nav(); await side(p).locator("button", { hasText: "Settings" }).last().click(); await p.waitForTimeout(600); });
    await esc(p);
    rows.push(`${N} page errors: ${JSON.stringify(p.errs.slice(0, 5))}`);
    await ctx.close();
  }
  // the review of design review 12: the reply still followed when the panel opens beside it, Esc's order (a menu, then the
  // panel, then Stop), focus in and back, a press on a drag region (Tauri's drag script stops it at the document: copied
  // here), the narrow sidebar under Settings, scrollbars that always show
  { const [ctx, p] = await ctxOf({ w: 1440, h: 900, mac: true, extra: DRAG });
    const gap = () => p.evaluate(() => { const s = document.querySelector("section.tz-scroller"); return Math.round(s.scrollHeight - s.scrollTop - s.clientHeight); });
    const jump = () => p.evaluate(() => { const b = document.querySelector("button[aria-label='Jump to the latest']")?.getBoundingClientRect(), c = document.querySelector("[data-composer]").getBoundingClientRect();
      return b ? `the way down ${Math.round(b.left + b.width / 2 - (c.left + c.width / 2))} px off the box's centre` : "no way down"; });
    await open(p, "Question 1");
    const g0 = await gap();
    await step(p, "fix-follow-open", async () => { await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(600); });
    rows.push(`follow: ${g0} px from the end, then the panel beside: ${await gap()} px, ${await jump()} (0, 0, no way down wanted)`);
    await p.getByTitle(/^Close the work panel/).click(); await p.waitForTimeout(400);
    await p.mouse.move(700, 400); await p.mouse.wheel(0, -700); await p.waitForTimeout(400);
    const g1 = await gap();
    await step(p, "fix-away-open", async () => { await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(600); });
    rows.push(`scrolled up ${g1} px, then the panel beside: ${await gap()} px, ${await jump()} (more, 0 off wanted)`);
    await ctx.close(); }
  {
    const [ctx, p] = await ctxOf({ w: 1280, h: 800, mac: true, extra: DRAG });
    const state = () => p.evaluate(() => { const a = document.activeElement;
      return `panel ${document.querySelector("div[data-panel].tz-pop") ? "open" : "shut"}, menu ${document.querySelector("#tz-complete") ? "open" : "shut"}, focus ${a?.closest("div[data-panel]") ? "in the panel" : a?.matches("textarea") ? "in the box" : a?.matches("main > header button[aria-pressed]") ? "on its button" : a?.tagName}, ${window.__SENT__.filter(([c]) => c === "interrupt").length} stops`; });
    await open(p, "Fix the flaky retry test");
    await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(400);
    rows.push(`esc: opened by its button: ${await state()} (open, in the panel)`);
    await esc(p); rows.push(`esc: Esc: ${await state()} (shut, on its button, 0 stops)`);
    await p.locator("textarea[aria-label=Message]").fill("/"); await p.waitForTimeout(300);
    await p.keyboard.press("Control+\\"); await p.waitForTimeout(400);
    await p.locator("textarea[aria-label=Message]").evaluate((t) => t.focus()); await p.waitForTimeout(200);
    await step(p, "fix-esc-menu", async () => {});
    rows.push(`esc: a menu over the open panel: ${await state()} (open, open, in the box)`);
    await esc(p); rows.push(`esc: Esc: ${await state()} (open, shut)`);
    await esc(p); rows.push(`esc: Esc: ${await state()} (shut, shut, in the box, 0 stops)`);
    await esc(p); rows.push(`esc: Esc: ${await state()} (1 stop: the reply's Stop)`);
    for (const [what, x, y] of [["the header's title", 420, 28], ["the brand", 125, 28], ["the desk over the sheet", 700, 3]]) {
      await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(300);
      await p.mouse.click(x, y); await p.waitForTimeout(300);
      rows.push(`a press on ${what} (a drag region): ${await state()} (shut)`);
    }
    await ctx.close(); }
  { const [ctx, p] = await ctxOf({ w: 960, h: 600, mac: true });
    await p.getByRole("button", { name: "Conversations", exact: true }).click(); await p.waitForTimeout(250);
    await step(p, "fix-narrow-settings", async () => { await side(p).locator("button", { hasText: "Settings" }).last().click(); await p.waitForTimeout(600); });
    rows.push(`narrow: Settings from the sidebar: the sidebar ${await p.evaluate(() => document.querySelector("aside[aria-label=Conversations]").getBoundingClientRect().width > 0 ? "OVER IT" : "shut")}, Settings ${await p.getByRole("dialog", { name: "Settings" }).count()}`);
    await ctx.close(); }
  for (const w of [1440, 1372]) {
    // a classic scrollbar, as a 15 px right border on the scroller (it takes the same place; this browser's are overlays)
    const bars = `addEventListener("DOMContentLoaded", () => { document.documentElement.style.setProperty("--tz-sbw", "15px"); const s = document.createElement("style"); s.textContent = "section.tz-scroller { border-right: 15px solid #8886 }"; document.head.append(s); });`;
    const [ctx, p] = await ctxOf({ w, h: 900, extra: bars });
    const widths = () => p.evaluate(() => { const m = document.querySelector("[data-msg]").getBoundingClientRect(), c = document.querySelector("[data-composer]").getBoundingClientRect();
      return `text ${Math.round(m.left + 28)}..${Math.round(m.right - 28)}, box ${Math.round(c.left)}..${Math.round(c.right)}`; });
    await open(p, "Why is the CI build");
    const shut = await widths();
    await step(p, `fix-scrollbars-${w}`, async () => { await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(600); });
    rows.push(`scrollbars of 15 px, ${w}: shut ${shut}; open ${await widths()} (the same edges wanted)`);
    rows.push(`fixes page errors: ${JSON.stringify(p.errs.slice(0, 5))}`);
    await ctx.close(); }
  log.push(...rows);
  if (process.env.ONLY === "rows") { fs.writeFileSync(`${OUT}/log.txt`, log.join("\n")); console.log(log.filter((l) => !l.startsWith("ok")).join("\n")); await browser.close(); return; }
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
    await step(p, `${T}-panel-open`, async () => { await p.getByTitle(/^The work panel: changes/).click(); await p.waitForTimeout(500); });
    const col = () => p.evaluate(() => `${document.querySelector('[style*="grid-template-columns"]').style.gridTemplateColumns.split(" ").pop()} column, ${document.querySelectorAll('button[aria-label^="The work panel"]').length} tab`);
    log.push(`${theme} panel opened by its tab: ${await col()} (0px, 1 tab wanted: at 1280 it opens over the conversation, design review 12)`);
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
    // a plan's model picked: the line under the message box says the plan pays (it read as the same model on a paid route)
    // the second GPT-6.2 Sol: ChatGPT's (the first is SubRouter's)
    await step(p, `${T}-composer-plan-line`, async () => { await p.getByText("GPT-6.2 Sol", { exact: true }).nth(1).click(); await p.waitForTimeout(500); await esc(p); });
    log.push(`${theme} plan line: ${JSON.stringify(await p.evaluate(() => [...document.querySelectorAll("span")].map((e) => e.textContent).find((t) => t?.startsWith("→ ")) ?? null))} (→ GPT-6.2 Sol on your ChatGPT plan wanted)`);
    await esc(p); await esc(p);
    // Google's two (core 0.1.198): their warning first, Google's sign-in in the browser, the account's project, signed in;
    // Antigravity's plan in the picker with Claude among its models
    await step(p, `${T}-google-warning`, async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(600); await p.getByRole("button", { name: "Providers", exact: true }).click();
      await p.getByRole("button", { name: /^More providers/ }).click(); await p.getByLabel("Search providers").fill("gemini cli"); await p.locator('[data-sign-in="Gemini CLI"]').click(); });
    await step(p, `${T}-google-signed-in`, async () => { await p.getByRole("button", { name: "I understand, sign in" }).click(); await p.waitForTimeout(1000); await p.getByLabel("Search providers").fill(""); });
    log.push(`${theme} Gemini CLI: ${await p.getByText("someone@example.com: 2 models in the model picker; they use your Gemini Code Assist quota").count()} outcome line, ${await p.getByText(/\/model|\/login/).count()} terminal commands (1, 0 wanted)`);
    await step(p, `${T}-antigravity-warning`, async () => { await p.getByLabel("Search providers").fill("antigravity"); await p.locator('[data-sign-in="Antigravity"]').click(); });
    await step(p, `${T}-antigravity-signed-in`, async () => { await p.getByRole("button", { name: "I understand, sign in" }).click(); await p.waitForTimeout(1000); });
    await esc(p); await esc(p);
    await step(p, `${T}-picker-antigravity`, async () => { await p.getByTitle("Model and effort").click(); await p.keyboard.type("antigravity"); });
    await esc(p); await esc(p);
    await step(p, `${T}-picker-mistral`, async () => { await p.getByTitle("Model and effort").click(); await p.keyboard.type("mistral"); });
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
  // a key saved for both Mistral and Mistral Vibe before the split (core 0.1.198): neither uses it, each row asks which it is;
  // "Yes" sends nothing typed, even after a key typed and cancelled; how it went says so on the row
  { const unsure = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T), inv = T.invoke.bind(T); let pair = true; T.invoke = async (c, a) => { if (c === "op" && a.msg.op === "login") { window.__sent = a.msg; pair = false; } return inv(c, a); }; T.emit = (e, p) => { if (p?.msg?.type === "settings" && pair) p.msg.providers = p.msg.providers.map((x) => x.name === "Mistral" ? { ...x, key: "same key as Mistral Vibe: is it an API key?", trusted: true, unsure: "pair" } : x.name === "Mistral Vibe" ? { ...x, key: "same key as Mistral: is it a Vibe key?", unsure: "pair" } : x); return emit(e, p); }; })();`;
    const [ctx, p] = await ctxOf({ extra: unsure });
    await step(p, "d-mistral-unsure", async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(600); await p.getByRole("button", { name: "Providers", exact: true }).click(); await p.getByText("Mistral Vibe", { exact: true }).first().scrollIntoViewIfNeeded(); });
    log.push(`mistral unsure: ${await p.getByRole("button", { name: "Yes", exact: true }).count()} yes buttons (2 wanted)`);
    await step(p, "d-mistral-yes", async () => { await p.getByRole("button", { name: "Replace key" }).nth(2).click(); await p.getByLabel("Mistral Vibe key").fill("abandoned"); await p.getByRole("button", { name: "Cancel" }).click();
      await p.getByRole("button", { name: "Yes", exact: true }).nth(1).click(); await p.waitForTimeout(500); });
    log.push(`mistral yes sent: ${JSON.stringify(await p.evaluate(() => window.__sent))} (key "" wanted)`);
    await ctx.close(); }
  // that key kept under Mistral Vibe after a new API key: both rows ask, Yes or Remove it; MISTRAL_API_KEY from the
  // environment may be a Vibe key: said on Mistral's row; a Workspace Gemini CLI account: its project on the row
  { const kept = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { if (p?.msg?.type === "settings") p.msg.providers = p.msg.providers.map((x) => x.name === "Mistral" ? { ...x, key: "key in the environment", trusted: true, unsure: "old_vibe", hint: "MISTRAL_API_KEY in your environment bills per token here; if it is a Vibe key, set MISTRAL_VIBE_API_KEY instead" } : x.name === "Mistral Vibe" ? { ...x, key: "kept from before: is it your Vibe key?", unsure: "old_vibe" } : x.name === "Gemini CLI" ? { ...x, needs_project: "", confirmed: true } : x); return emit(e, p); }; })();`;
    const [ctx, p] = await ctxOf({ extra: kept });
    await step(p, "d-mistral-kept", async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(600); await p.getByRole("button", { name: "Providers", exact: true }).click(); await p.getByText("Mistral Vibe", { exact: true }).first().scrollIntoViewIfNeeded(); });
    log.push(`mistral kept: ${await p.getByRole("button", { name: "Remove it" }).count()} remove buttons (2 wanted)`);
    await step(p, "d-gemini-project", async () => { await p.getByRole("button", { name: /^More providers/ }).click(); await p.getByLabel("Search providers").fill("gemini cli"); await p.getByLabel("Gemini CLI Google Cloud project").fill("my-project-123"); });
    await ctx.close(); }
  // a fresh install: no key yet, nothing running here
  { const none = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { if (p?.msg?.type === "settings") p.msg.providers = p.msg.providers.filter((x) => !x.local).map((x) => ({ ...x, key: "" })); return emit(e, p); }; })();`;
    const [ctx, p] = await ctxOf({ extra: `${none} window.__NO_CLAUDE__ = true;` });
    await step(p, "d-send-no-provider", async () => {
      await p.getByRole("textbox", { name: "Message" }).fill("  Hello\nWorld  ");
      await p.getByTitle("Send (Enter)").click();
      await p.getByRole("alert").filter({ hasText: "starting claude (is it installed and on PATH?)" }).waitFor({ state: "visible", timeout: 4000 });
      await p.waitForFunction(() => document.querySelector('textarea[aria-label="Message"]')?.value === "  Hello\nWorld  ", null, { timeout: 4000 });
    });
    await step(p, "d-providers-none", async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(600); await p.getByRole("button", { name: "Providers", exact: true }).click(); });
    await ctx.close(); }
  { const [ctx, p] = await ctxOf({ extra: "window.__DRAFT_SEND_SUCCESS__ = true;" });
    const box = p.getByRole("textbox", { name: "Message" });
    await step(p, "d-send-clears-draft", async () => {
      await box.fill("First message");
      await p.getByTitle("Send (Enter)").click();
      await p.locator("[data-msg]").getByText("First message", { exact: true }).waitFor({ state: "visible", timeout: 4000 });
      if (await box.inputValue()) throw new Error("an accepted send left its draft in the box");
    });
    await step(p, "d-queue-clears-draft", async () => {
      await box.fill("Second message");
      await p.getByTitle("Send after this reply (Enter)").click();
      await p.getByText("Second message", { exact: true }).waitFor({ state: "visible", timeout: 4000 });
      if (await box.inputValue()) throw new Error("a queued send left its draft in the box");
    });
    await ctx.close(); }
  // Cmd-V of an image (a screenshot, or an image file copied in Finder, which brings its name as text too): an attachment
  { const [ctx, p] = await ctxOf({});
    const box = p.getByRole("textbox", { name: "Message" });
    await step(p, "d-paste-image", async () => {
      await box.fill("what is wrong here?");
      await p.evaluate(() => {
        const png = Uint8Array.from(atob("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="), (c) => c.charCodeAt(0));
        const dt = new DataTransfer();
        dt.items.add(new File([png], "image.png", { type: "image/png" }));
        dt.setData("text/plain", "Screenshot 2026-11-24 at 10.00.00.png");
        document.querySelector('textarea[aria-label="Message"]').dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true }));
      });
      await p.getByText("paste-1.png").waitFor({ state: "visible", timeout: 4000 });
      if (await box.inputValue() !== "what is wrong here?") throw new Error("the pasted file's name went into the draft");
    });
    await step(p, "d-paste-linux-clipboard-image", async () => {
      await box.evaluate((el) => el.dispatchEvent(new ClipboardEvent("paste", { clipboardData: new DataTransfer(), bubbles: true, cancelable: true })));
      await p.getByText("paste-clipboard.png").waitFor({ state: "visible", timeout: 4000 });
      if (await box.inputValue() !== "what is wrong here?") throw new Error("the fallback changed the draft");
    });
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
