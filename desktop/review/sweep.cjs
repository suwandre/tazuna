// The design sweep: the desktop app on test data (review/mock.js), every screen, menu and state worth a look, in dark and
// light and at three window sizes, as PNGs to go through before a release. Design review 4 (2026-11-19) found a menu cut
// off and a raw error that the main screens alone had not shown.
//
//   npm run dev                                   # the app on localhost:1420
//   node review/sweep.cjs [http://localhost:1420] [out dir]   # needs playwright: npx playwright install chromium
//   ENGINE=webkit node review/sweep.cjs …         # in WebKit (the Mac app's engine); ONLY=rows: the window's rows alone,
//                                                 # ONLY=steps: a step's insides (diffs, code, gutters) alone
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
  const ctxOf = async ({ theme = "dark", w = 1280, h = 800, setup = true, extra = "", mac = false, scale = 1, motion = "no-preference" } = {}) => {
    // a Linux agent: on a Mac one the app leaves its shortcuts to the native menus, which a browser does not have. `mac`:
    // a Mac's agent all the same, for the title bar's row (the overlay's traffic lights drawn where the real window has them).
    // `scale` 2: a Retina screen's device pixels; `motion` "reduce": reduced motion
    const ctx = await browser.newContext({ viewport: { width: w, height: h }, colorScheme: theme, userAgent: "Mozilla/5.0 (X11; Linux x86_64) Chrome/140", deviceScaleFactor: scale, reducedMotion: motion });
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
  // the conversation's numbers in the status bar (Settings › Appearance; they are the composer's strip by default): its
  // "this chat" opens the conversation's receipt
  const toBar = (p) => p.evaluate(() => { localStorage.setItem("tazuna.numbers", "bar"); window.dispatchEvent(new Event("tz-looks")); });
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
  // the receipt's numbers and the work panel's Stats (the user, 2026-11-25, mockups/v11.html): a plan run's receipt with its
  // time to first token, speed, the cache's share and a totals row (wide, then narrow: speed goes first, then TTFT, the
  // share stays), the plan footers (a plan's provider, Claude's, one with no reference price), the Stats over this turn and
  // the conversation in both themes, beside and over the conversation, a sparkline's tip by pointer and by keyboard, the data
  // marks in `live` under every accent, the numbers tweening while a reply runs (jumping under reduced motion), and the usage
  // page's plans by provider. ONLY=stats: these alone
  if (!process.env.ONLY || process.env.ONLY === "stats") {
    const must = (ok, what) => { if (!ok) throw new Error(what); };
    const VIBE = "Make the request logger";
    const slip = (p) => p.evaluate(() => { const s = [...document.querySelectorAll(".tz-slip")].pop(), shown = (e) => getComputedStyle(e).display !== "none", t = s.querySelector("table");
      return { cols: [...s.querySelectorAll("th")].filter(shown).map((e) => e.textContent).join(","), total: [...s.querySelectorAll("tr")].pop().innerText.replace(/\s+/g, " ").trim(),
        text: s.innerText, over: t ? t.scrollWidth - s.clientWidth + 32 : 0, width: Math.round(s.getBoundingClientRect().width) }; });
    const stats = (p) => p.evaluate(() => { const s = document.querySelector("section[aria-label=Stats]"); if (!s) return null;
      const tile = (l) => [...s.querySelectorAll("div")].find((d) => d.textContent === l && d.className.includes("text-faint"))?.nextElementSibling?.textContent;
      const probe = document.createElement("i"); probe.style.color = "var(--live)"; s.append(probe); const live = getComputedStyle(probe).color; probe.remove();
      const cut = [...s.querySelectorAll(".truncate")].filter((e) => e.scrollWidth > e.clientWidth + 1).map((e) => e.textContent);
      const sub = [...s.querySelectorAll("div")].find((d) => d.textContent === "Cost" && d.className.includes("text-faint"))?.nextElementSibling?.nextElementSibling?.textContent ?? null;
      return { first: s.parentElement.firstElementChild === s, in: tile("In"), out: tile("Out"), cost: tile("Cost"), costSub: sub, calls: tile("Calls"), text: s.innerText.replace(/\s+/g, " "),
        strokes: [...s.querySelectorAll("path")].map((e) => getComputedStyle(e).stroke), live, wide: s.scrollWidth - s.clientWidth, cut,
        tip: s.querySelector(".tz-float")?.textContent ?? null }; });
    for (const theme of ["dark", "light"]) {
      const [ctx, p] = await ctxOf({ theme, w: 1440, h: 900 }), T = `${theme[0]}-stats`;
      await open(p, VIBE);
      await step(p, `${T}-receipt-wide`, async () => { await p.getByTitle(/^The receipt/).first().click(); await p.waitForTimeout(500);
        const r = await slip(p); log.push(`${T} receipt: ${r.width} px, ${r.cols} | ${r.total}`);
        must(r.cols === "Call,In,Cached,Out,TTFT,Speed,Time,Price" && r.over <= 0, `wide receipt: ${JSON.stringify(r)}`);
        must(/^Total 1\.2M 1\.1M ?87\.1% 8\.7k 0\.9 s ?med 225 t\/s 49 s \$0\.064$/.test(r.total), `totals row: ${r.total}`);
        must(r.text.includes("Total at Mistral Vibe's API prices") && /On your Mistral Vibe plan\s+not billed/.test(r.text) && !r.text.includes("Paid"), `the plan's footer: ${r.text}`); });
      // the receipt's close (the user's recording, 2026-11-26): the fold shrinks and fades to nothing, never flashing back
      // at full height for a frame before it goes (the finished animation had let go of the body before React took it out)
      await step(p, `${T}-receipt-close`, async () => {
        const frames = await p.evaluate(async () => {
          const out = [];
          const t0 = performance.now();
          document.querySelector("button[title^='The receipt']").click();
          const sample = () => {
            const e = document.querySelector(".scroll-mb-10");
            out.push(e ? { h: e.getBoundingClientRect().height, op: Number(getComputedStyle(e).opacity) } : null);
            if (performance.now() - t0 < 400) requestAnimationFrame(sample);
          };
          requestAnimationFrame(sample);
          await new Promise((r) => setTimeout(r, 450));
          return out;
        });
        const gone = frames.findIndex((f) => f === null), before = frames.slice(0, gone).filter(Boolean);
        log.push(`${T} close: ${before.map((f) => `${Math.round(f.h)}@${f.op.toFixed(2)}`).join(" ")} then gone`);
        must(gone > 3, `the fold never went: ${JSON.stringify(frames)}`);
        // once small and faint it stays small and faint: no frame back at full height or opacity on the way out
        const small = before.findIndex((f) => f.h < 40);
        must(small >= 0 && before.slice(small).every((f) => f.h < 40 && f.op < 0.2), `the close flashed back: ${JSON.stringify(before)}`);
        await p.getByTitle(/^The receipt/).first().click(); await p.waitForTimeout(400); // open again for what follows
      });
      // the Stats beside the conversation (1440): this turn, then the conversation
      await step(p, `${T}-turn`, async () => { await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(700);
        const s = await stats(p); log.push(`${T} turn: ${JSON.stringify(s)}`);
        must(s && s.first && s.calls === "3" && s.in === "469.9k" && s.out === "910" && s.text.includes("at API prices · not billed") && s.text.includes("t/s") && !/tok\/s|\d \/s/.test(s.text), `this turn: ${JSON.stringify(s)}`);
        must(s.wide <= 0 && !s.cut.length && s.strokes.length === 3 && s.strokes.every((c) => c === s.live), `fit and colour: ${JSON.stringify(s)}`); });
      await step(p, `${T}-conversation`, async () => { await p.getByRole("radio", { name: "Conversation" }).click(); await p.waitForTimeout(300);
        const s = await stats(p); must(s.calls === "11" && s.in === "1.7M", `the conversation: ${JSON.stringify(s)}`); });
      // the pointer over the speed line: a crosshair and "call N · 219 t/s"; the keyboard on the first line: the arrows
      await step(p, `${T}-hover`, async () => { const g = p.locator("[role=group][aria-label^='Writing speed']"), b = await g.boundingBox();
        await p.mouse.move(b.x + b.width * 0.42, b.y + b.height / 2); await p.waitForTimeout(150);
        const s = await stats(p); log.push(`${T} tip: ${s.tip}`); must(/^call \d+ · \d+ t\/s$/.test(s.tip ?? ""), `the speed's tip: ${s.tip}`); });
      await step(p, `${T}-keys`, async () => { await p.mouse.move(700, 450); await p.locator("[role=group][aria-label^='Time to first token']").focus(); await p.keyboard.press("ArrowLeft");
        const label = await p.evaluate(() => document.activeElement.getAttribute("aria-label")), s = await stats(p);
        must(label === "Time to first token: call 10, 0.8 s" && s.tip === "call 10 · 0.8 s", `keys: ${label} | ${s.tip}`); });
      await p.keyboard.press("Tab");
      // every accent: the lines stay live, never the accent
      for (const accent of ["indigo", "jade", "amber", "rose", "graphite"]) await step(p, `${T}-accent-${accent}`, async () => {
        await p.evaluate((a) => { document.documentElement.dataset.accent = a; }, accent); await p.waitForTimeout(100);
        const s = await stats(p); must(s.strokes.every((c) => c === s.live), `${accent}: ${JSON.stringify([s.strokes, s.live])}`); });
      await p.evaluate(() => { delete document.documentElement.dataset.accent; });
      // live: a reply plays call by call; the numbers move over ~450 ms (sampled), the newest point grows in
      for (const motion of ["no-preference", "reduce"]) {
        const [c2, q] = motion === "reduce" ? await ctxOf({ theme, w: 1440, h: 900, motion }) : [null, p], M = motion === "reduce" ? `${T}-still` : `${T}-live`;
        if (c2) { await open(q, VIBE); await q.getByTitle(/^Open the work panel/).click(); await q.waitForTimeout(700); }
        else { await q.getByRole("radio", { name: "This turn" }).click(); }
        await q.evaluate(() => { window.__PLAY__.start(); }); await q.waitForTimeout(300);
        await q.evaluate(() => { window.__PLAY__.next(); }); await q.waitForTimeout(800);
        await step(q, M, async () => {
          const seen = await q.evaluate(async () => { const out = (() => { const s = document.querySelector("section[aria-label=Stats]"); return [...s.querySelectorAll("div")].find((d) => d.textContent === "Out" && d.className.includes("text-faint")).nextElementSibling; })();
            window.__PLAY__.next(); const vals = []; const t0 = performance.now();
            while (performance.now() - t0 < 700) { vals.push(out.textContent); await new Promise((r) => setTimeout(r, 30)); }
            return vals; });
          await q.screenshot({ path: `${OUT}/${M}-before-end.png` });
          const distinct = [...new Set(seen)];
          log.push(`${M}: Out ${distinct.join(" → ")}`);
          must(seen[seen.length - 1] === "2.2k", `${M}: ended at ${seen[seen.length - 1]}`);
          must(motion === "reduce" ? distinct.length <= 2 && seen[3] === "2.2k" : distinct.length >= 4, `${M}: ${distinct.join(" → ")}`);
        });
        // a frame mid-way through the next call's tween
        await q.evaluate(() => { window.__PLAY__.next(); }); await q.waitForTimeout(120); await q.screenshot({ path: `${OUT}/${M}-mid.png` });
        await q.waitForTimeout(600); await q.evaluate(() => { window.__PLAY__.end(); }); await q.waitForTimeout(400);
        await step(q, `${M}-ended`, async () => { const s = await stats(q); must(s.calls === "3" && !s.text.includes("live") && s.cost.startsWith("$"), `after the reply: ${JSON.stringify(s)}`); });
        if (c2) await c2.close();
      }
      // the review of 2026-11-25: a reply running after plan turns is "so far" in the conversation, never "not billed"; a
      // switch of scope during a reply jumps; running calls without their model, or on a plan with no reference price, have
      // no cost ("–", never $0)
      const running = (calls) => p.evaluate((cs) => window.__MOCK__.emit("p6", (v) => ({ ...v, activity: "Tool", blocks: [...v.blocks, { kind: "prompt", text: "And the README" }],
        calls: [...v.calls, ...cs.map((c) => ({ block: v.blocks.length + 1, fresh: 900, cached: 157000, out: 120, first_ms: 800, tps: 210, total_ms: 1500, role: null, ...c }))] })), calls);
      await step(p, `${T}-paid-after-plan`, async () => { await running([{ cost: 0.01, model: "deepseek-v4.1-flash", provider: "SubRouter" }]); await p.waitForTimeout(300);
        await p.getByRole("radio", { name: "Conversation" }).click(); await p.waitForTimeout(600);
        const s = await stats(p); log.push(`${T} a paid reply after plan turns: ${s.cost} · ${s.costSub}`); must(s.costSub === "so far" && s.cost === "$0.079", `paid after plan: ${JSON.stringify(s)}`); });
      await step(p, `${T}-scope-jump`, async () => {
        const seen = await p.evaluate(async () => { const s = document.querySelector("section[aria-label=Stats]"), out = () => [...s.querySelectorAll("div")].find((d) => d.textContent === "Out" && d.className.includes("text-faint")).nextElementSibling.textContent;
          [...s.querySelectorAll("[role=radio]")].find((b) => b.textContent === "This turn").click(); const vals = [];
          for (let i = 0; i < 12; i++) { await new Promise((r) => setTimeout(r, 30)); vals.push(out()); } return vals; });
        log.push(`${T} scope switched during a reply: Out ${[...new Set(seen)].join(" → ")}`); must(new Set(seen).size === 1 && seen[0] === "120", `the switch slid: ${seen.join(" → ")}`); });
      await step(p, `${T}-running-no-model`, async () => { await running([{ cost: 0.3, estimate: true }]); await p.waitForTimeout(500);
        const s = await stats(p); must(s.cost === "–" && !s.costSub && !/\$0(?!\.)/.test(s.text), `a call without its model: ${JSON.stringify(s)}`); });
      await p.evaluate(() => window.__MOCK__.emit("p6", (v) => v)); await p.waitForTimeout(300);
      // a plan whose catalog lists no price: no amount, never $0 (receipt line and footer); Claude's plan by name
      await step(p, `${T}-receipt-refless`, async () => { await open(p, "Rename the config loader"); await p.getByTitle(/^The receipt/).first().click(); await p.waitForTimeout(500);
        const r = await slip(p), line = await p.getByTitle(/^The receipt/).first().innerText();
        must(/On your Z\.ai Coding Plan\s+not billed/.test(r.text) && !r.text.includes("API prices") && !/\$0(?!\.)/.test(r.text) && line.startsWith("not billed"), `no reference: ${line} | ${r.text}`); });
      await step(p, `${T}-chat-receipt-no-ref`, async () => { await toBar(p); await p.getByTitle(/^This conversation's receipt/).click(); await p.waitForTimeout(400);
        const t = await p.getByRole("dialog", { name: "This conversation's receipt" }).innerText();
        must(/On your Z\.ai Coding Plan\s+not billed/.test(t) && /Rename the config loader[^\n]*\s+2\s+9 s\s+not billed/.test(t) && !/\$0(?!\.)/.test(t), `the conversation's slip, no reference: ${t}`); });
      await esc(p);
      await step(p, `${T}-running-no-ref`, async () => { await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(600);
        await p.evaluate(() => window.__MOCK__.emit("p7", (v) => ({ ...v, model: "glm-5.3", activity: "Tool", blocks: [...v.blocks, { kind: "prompt", text: "Rename it back" }],
          calls: [...v.calls, { block: v.blocks.length + 1, fresh: 900, cached: 12000, out: 60, cost: 0, first_ms: 900, tps: 90, total_ms: 1700, model: "glm-5.3", role: null, provider: "Z.ai Coding Plan" }] })));
        await p.waitForTimeout(500); const s = await stats(p);
        must(s.cost === "–" && !s.costSub && !/\$0(?!\.)/.test(s.text), `a running turn on a plan with no reference: ${JSON.stringify(s)}`); });
      await p.getByTitle(/^Close the work panel/).click().catch(() => {});
      await step(p, `${T}-receipt-claude`, async () => { await open(p, "Why is the CI build"); await p.getByTitle(/^The receipt/).first().click(); await p.waitForTimeout(500);
        const r = await slip(p); must(r.text.includes("Total at Claude's API prices") && /On your Claude plan\s+not billed/.test(r.text), `Claude's plan: ${r.text}`); });
      // the usage page: each plan by its provider
      await step(p, `${T}-usage-plans`, async () => { await p.keyboard.press("Control+Shift+U"); await p.waitForTimeout(900);
        await p.getByText("A plan's runs are counted at API prices").scrollIntoViewIfNeeded();
        const text = await p.getByRole("dialog", { name: "Usage" }).innerText();
        must(text.includes("Mistral Vibe plan") && /Plans\s/.test(text) && text.includes("Plans, at API prices") && !text.includes("Claude on your plan"), "the usage page's plans"); });
      await esc(p);
      log.push(`${T} page errors: ${JSON.stringify(p.errs.slice(0, 5))}`);
      await ctx.close();
      // over the conversation (1280), and the receipt narrow: speed goes first, then the time to first token
      const [c3, r] = await ctxOf({ theme, w: 1280, h: 800 });
      await open(r, VIBE);
      await step(r, `${T}-over`, async () => { await r.getByTitle(/^Open the work panel/).click(); await r.waitForTimeout(700);
        const s = await stats(r); must(await r.locator("div[data-panel] section[aria-label=Stats]").count() === 1 && s.wide <= 0 && !s.cut.length, `over: ${JSON.stringify(s)}`); });
      await c3.close();
      for (const [w, cols] of [[600, "Call,In,Cached,Out,TTFT,Time,Price"], [520, "Call,In,Cached,Out,Time,Price"]]) {
        const [c4, q] = await ctxOf({ theme, w, h: 800 });
        await step(q, `${T}-receipt-${w}`, async () => { await q.getByRole("button", { name: "Conversations", exact: true }).click(); await open(q, VIBE);
          await q.getByTitle(/^The receipt/).first().click(); await q.waitForTimeout(500);
          const s = await slip(q); log.push(`${T} receipt at ${w}: ${s.width} px, ${s.cols} | ${s.total}`);
          must(s.cols === cols && s.over <= 0 && /87\.1%/.test(s.total), `narrow ${w}: ${JSON.stringify(s)}`); });
        await c4.close();
      }
    }
    { const [c5, q] = await ctxOf({ extra: "window.__NO_TODAY__ = true; window.__REFLESS_ONLY__ = true;" });
      await step(q, "stats-usage-no-ref", async () => { await q.keyboard.press("Control+Shift+U"); await q.waitForTimeout(900);
        await q.getByText("A plan's runs are counted at API prices").scrollIntoViewIfNeeded();
        const text = await q.getByRole("dialog", { name: "Usage" }).innerText();
        must(text.includes("Z.ai Coding Plan") && /Z\.ai Coding Plan\s+not billed/.test(text) && !text.includes("No plan runs") && !/\$0(?!\.)/.test(text), `only plans with no price: ${text}`); });
      await c5.close(); }
    if (process.env.ONLY === "stats") { fs.writeFileSync(`${OUT}/log.txt`, log.join("\n")); console.log(log.filter((l) => !l.startsWith("ok")).join("\n")); await browser.close(); if (log.some((l) => l.startsWith("FAIL"))) process.exitCode = 1; return; }
  }
  // the work panel's width (desktop/DESIGN.md, Layout; ONLY=resize: these alone): its edge dragged to 420 and to the most at
  // 1440 and 1600, the reading column centred in what is left at each width (its text's two gaps and the box's within 1 px)
  // and following the pointer during the drag (no transition), the keyboard (arrows, Shift, Home, End, Enter), a double
  // click back to 300, the width kept over a reload, a smaller window holding it, the panel over the conversation at 1280
  // as wide, the approval and the way back down at 420, and at 260 nothing in the panel past its edge (long checks, branch
  // and paths put in); both themes
  if (!process.env.ONLY || process.env.ONLY === "resize") {
    const must = (c, what) => { if (!c) throw new Error(what); };
    const SEP = "[role=separator][aria-label='Width of the work panel']", RAIL = "aside[aria-label='The work']";
    const AGAIN = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { if (p?.msg?.type === "view") window.__lastView = p; return emit(e, p); };
      window.__again = (f) => { const { tid, msg } = window.__lastView; T.view(tid, f({ ...msg, from: 0 })); };
      window.__pr = (m) => T.emit("tazuna-view", { tid: window.__lastView.tid, msg: { type: "pr", ...m } }); })();`;
    // what the panel measures: its width, the separator's values, the text's gaps on the sheet and the box's
    const geo = (p) => p.evaluate(([SEP, RAIL]) => { const m = document.querySelector("main").getBoundingClientRect(), t = document.querySelector("[data-msg]")?.getBoundingClientRect(), c = document.querySelector("[data-composer]")?.getBoundingClientRect();
      const a = document.querySelector(RAIL), sep = document.querySelector(SEP), over = !!document.querySelector("div[data-panel] " + RAIL);
      return { panel: a ? Math.round((over ? a.parentElement : a).getBoundingClientRect().width) : 0, over, now: sep && +sep.getAttribute("aria-valuenow"), max: sep && +sep.getAttribute("aria-valuemax"), min: sep && +sep.getAttribute("aria-valuemin"),
        left: t && Math.round(t.left + 28 - m.left), right: t && Math.round(m.right - (t.right - 28)), text: t && Math.round(t.width - 56), boxLeft: c && Math.round(c.left - m.left), boxRight: c && Math.round(m.right - c.right),
        kept: localStorage.getItem("tazuna.panel-width") }; }, [SEP, RAIL]);
    const even = (g) => Math.abs(g.left - g.right) <= 1 && Math.abs(g.boxLeft - g.boxRight) <= 1;
    // the pointer on the edge, pressed, moved by dx in steps, let go
    const drag = async (p, dx, hold) => { const h = await p.locator(SEP).boundingBox(), x = h.x + h.width / 2, y = h.y + h.height / 2;
      await p.mouse.move(x, y); await p.mouse.down(); for (let i = 1; i <= 8; i++) await p.mouse.move(x + (dx * i) / 8, y);
      if (hold) return [x, y]; await p.mouse.up(); await p.waitForTimeout(300); return [x, y]; };
    // anything in the panel past its own edges, and what is cut with no tooltip saying it whole
    const spill = (p) => p.evaluate((RAIL) => { const a = document.querySelector(RAIL), r = a.getBoundingClientRect(), name = (e) => `${e.tagName.toLowerCase()}.${String(e.className).split(" ").slice(0, 3).join(".")} "${(e.textContent ?? "").trim().slice(0, 30)}"`;
      const els = [...a.querySelectorAll("*")].filter((e) => e.getBoundingClientRect().width > 0);
      // what shows of an element: its box inside every ancestor that clips (a truncated row's spans run on under its cut)
      const shown = (e) => { let b = e.getBoundingClientRect(), l = b.left, rt = b.right; for (let q = e.parentElement; q && q !== a; q = q.parentElement) if (getComputedStyle(q).overflowX !== "visible") { const c = q.getBoundingClientRect(); l = Math.max(l, c.left); rt = Math.min(rt, c.right); } return [l, rt]; };
      return { wide: a.scrollWidth - a.clientWidth, past: els.filter((e) => { const [l, rt] = shown(e); return rt > l && (rt > r.right + 0.5 || l < r.left - 0.5); }).map(name),
        cut: els.filter((e) => e.scrollWidth > e.clientWidth + 1 && getComputedStyle(e).overflowX !== "visible" && e !== a && !e.closest("[title]") && !e.closest("[role=group]")).map(name) }; }, RAIL);
    // long lines put in: a check's command, the branch, deep paths in Changes
    const long = (p) => p.evaluate(() => { window.__again((v) => ({ ...v,
      blocks: v.blocks.map((b) => (b.kind === "verify" ? { ...b, cmd: "cargo test --workspace --all-features -- --test-threads=1 --nocapture integration::retry::backoff_respects_the_jitter_window" } : b)),
      changes: [{ turn: 0, at_block: 8, undone: false, can_undo: true, files: [{ path: "packages/gateway/src/middleware/request-logging/latency-histogram-buckets.test.ts", added: 120, removed: 48, status: "M" },
        { path: "tests/retry.test.ts", added: 9, removed: 3, status: "M" }, { path: "docs/an-unusually-long-file-name-with-no-folder-at-all-for-the-panel.md", added: 2, removed: 0, status: "A" }] }] }));
      window.__pr({ none: true, branch: "feature/T-123-make-the-retry-backoff-respect-the-jitter-window-on-slow-runners" }); });
    // the conversation's row is named by its first message once it has been opened
    const openFix = async (p) => { await side(p).locator('a[title^="Fix the flaky retry test"], a[title^="The retry test fails"]').first().click({ timeout: 4000 }); await p.waitForTimeout(800); };
    // back to 300 by the keyboard, the focus given back
    const reset = async (p) => { await p.locator(SEP).focus(); await p.keyboard.press("Enter"); await p.locator(SEP).blur(); await p.waitForTimeout(300); };
    const openPanel = async (p) => { if (!(await p.locator(RAIL).count())) { await p.evaluate(() => document.querySelector("main > header button[data-panel]").click()); await p.waitForTimeout(450); } };
    for (const theme of ["dark", "light"]) {
      const T = `resize-${theme[0]}`, [ctx, p] = await ctxOf({ theme, w: 1440, h: 900, extra: AGAIN });
      await p.evaluate(() => localStorage.removeItem("tazuna.panel-width"));
      for (const w of [1440, 1600]) {
        await step(p, `${T}-${w}-300`, async () => { await p.setViewportSize({ width: w, height: 900 }); await p.waitForTimeout(250); await openFix(p); await long(p); await p.waitForTimeout(300); await openPanel(p);
          // one width for the window: 1440's last one stands at 1600 until Enter puts it back
          if (w === 1600) { log.push(`${T} 1600 with 1440's width kept: ${JSON.stringify(await geo(p))}`); await p.locator(SEP).focus(); await p.keyboard.press("Enter"); await p.locator(SEP).blur(); await p.waitForTimeout(300); }
          const g = await geo(p); log.push(`${T} ${w} at 300: ${JSON.stringify(g)}`); must(g.panel === 300 && g.now === 300 && even(g) && g.text === 700, `at 300: ${JSON.stringify(g)}`); });
        // the hairline on hover (line-strong), then the drag: during it the column is where the width says at once
        await step(p, `${T}-${w}-hover`, async () => { const h = await p.locator(SEP).boundingBox(); await p.mouse.move(h.x + h.width / 2, 450); await p.waitForTimeout(200);
          const c = await p.locator(SEP).evaluate((e) => [getComputedStyle(e, "::before").backgroundColor, getComputedStyle(e).cursor]), want = await p.evaluate(() => { const d = document.createElement("i"); d.style.color = "var(--line-strong)"; document.body.append(d); const c = getComputedStyle(d).color; d.remove(); return c; });
          log.push(`${T} ${w} hover: ${c} (${want})`); must(c[0] === want && c[1] === "col-resize", `hover: ${c}, wanted ${want}`);
          await p.screenshot({ path: `${OUT}/${T}-${w}-hover-edge.png`, clip: { x: h.x - 60, y: 0, width: 160, height: 220 } }); });
        await step(p, `${T}-${w}-dragging`, async () => { await drag(p, -60, true); await p.waitForTimeout(60);
          const mid = await geo(p), on = await p.evaluate(() => [document.documentElement.classList.contains("tz-sizing"), getComputedStyle(document.querySelector("[data-tauri-drag-region].grid")).transitionDuration]);
          await p.mouse.up(); await p.waitForTimeout(300);
          log.push(`${T} ${w} mid-drag: ${JSON.stringify(mid)} ${on}`); must(mid.panel === 360 && even(mid) && on[0] && on[1] === "0s", `mid-drag: ${JSON.stringify(mid)} ${on}`); });
        await step(p, `${T}-${w}-420`, async () => { await drag(p, -60); const g = await geo(p); log.push(`${T} ${w} at 420: ${JSON.stringify(g)}`);
          must(g.panel === 420 && g.now === 420 && g.kept === "420" && even(g), `at 420: ${JSON.stringify(g)}`); });
        await step(p, `${T}-${w}-most`, async () => { await drag(p, -600); const g = await geo(p), want = w === 1440 ? 548 : 708; log.push(`${T} ${w} at the most: ${JSON.stringify(g)}`);
          must(g.panel === want && g.max === want && even(g) && g.text === 580, `at the most (${want}): ${JSON.stringify(g)}`); });
      }
      // the keyboard: the separator focused (the page's own focus(): WebKit takes Tab to links and buttons only by its setting)
      await step(p, `${T}-keys`, async () => { await p.locator(SEP).focus(); const got = [];
        for (const k of ["Enter", "ArrowLeft", "Shift+ArrowLeft", "ArrowRight", "Shift+ArrowRight", "Home", "End", "Enter"]) { await p.keyboard.press(k); await p.waitForTimeout(260); const g = await geo(p); got.push(`${k} ${g.now}/${g.panel}`); }
        log.push(`${T} keys at 1600: ${got.join(", ")}`);
        must(got.join(",") === "Enter 300/300,ArrowLeft 316/316,Shift+ArrowLeft 380/380,ArrowRight 364/364,Shift+ArrowRight 300/300,Home 260/260,End 708/708,Enter 300/300", `keys: ${got}`);
        await p.keyboard.press("Shift+ArrowLeft"); await p.waitForTimeout(260);
        await p.screenshot({ path: `${OUT}/${T}-keys-focus.png` }); });
      await step(p, `${T}-dblclick`, async () => { await drag(p, -200); const before = (await geo(p)).panel; await p.locator(SEP).dblclick(); await p.waitForTimeout(350); const g = await geo(p);
        log.push(`${T} double click: ${before} -> ${JSON.stringify(g)}`); must(before === 564 && g.panel === 300 && g.kept === "300" && even(g), `double click: ${before} ${JSON.stringify(g)}`); });
      // 260: nothing past the panel's edge, the long lines wrapped or cut with a tooltip
      await step(p, `${T}-260`, async () => { await p.setViewportSize({ width: 1440, height: 900 }); await p.waitForTimeout(250); await p.locator(SEP).focus(); await p.keyboard.press("Home"); await p.waitForTimeout(300);
        const g = await geo(p), s = await spill(p); log.push(`${T} 260: ${JSON.stringify(g)}; spill ${JSON.stringify(s)}`);
        must(g.panel === 260 && even(g) && s.wide <= 0 && !s.past.length && !s.cut.length, `260: ${JSON.stringify([g, s])}`);
        await p.locator(RAIL).screenshot({ path: `${OUT}/${T}-260-panel.png` }); });
      await step(p, `${T}-420-panel`, async () => { await p.keyboard.press("End"); await p.waitForTimeout(300); await p.locator(RAIL).screenshot({ path: `${OUT}/${T}-548-panel.png` });
        await drag(p, 128); const g = await geo(p), s = await spill(p); log.push(`${T} 420 panel: ${JSON.stringify(g)}; spill ${JSON.stringify(s)}`); must(g.panel === 420 && !s.past.length, `420: ${JSON.stringify([g, s])}`); });
      // kept over a reload (one width for the window: another conversation has it too)
      await step(p, `${T}-reload`, async () => { await p.reload(); await p.waitForTimeout(1100); await open(p, "Why is the CI build"); await openPanel(p); const g = await geo(p);
        log.push(`${T} after a reload: ${JSON.stringify(g)}`); must(g.panel === 420 && g.kept === "420" && even(g), `after a reload: ${JSON.stringify(g)}`); });
      // the approval card and the way back down follow the column
      await step(p, `${T}-approval-420`, async () => { await open(p, "Bump the TLS library"); await openPanel(p); await p.getByText("Allow a shell command?").waitFor({ timeout: 4000 });
        const a = await p.evaluate(() => { const m = document.querySelector("main").getBoundingClientRect(), c = document.querySelector("[data-composer]").getBoundingClientRect(), card = [...document.querySelectorAll("main *")].find((e) => e.textContent?.startsWith("Allow a shell command?") && e.getBoundingClientRect().width > 300 && e.getBoundingClientRect().width <= c.width + 1)?.getBoundingClientRect();
          return { card: card && [Math.round(card.left - m.left), Math.round(m.right - card.right)], box: [Math.round(c.left - m.left), Math.round(m.right - c.right)] }; });
        log.push(`${T} approval at 420: ${JSON.stringify(a)}`); must(a.card && Math.abs(a.card[0] - a.card[1]) <= 1 && Math.abs(a.box[0] - a.box[1]) <= 1, `approval: ${JSON.stringify(a)}`); });
      await step(p, `${T}-down-420`, async () => { await p.setViewportSize({ width: 1440, height: 560 }); await open(p, "Why is the CI build"); await openPanel(p); await p.locator(".tz-scroller").first().evaluate((e) => { e.scrollTop = 0; e.dispatchEvent(new Event("scroll")); }); await p.waitForTimeout(400);
        const d = await p.evaluate(() => { const m = document.querySelector("main").getBoundingClientRect(), b = document.querySelector("button[aria-label='Jump to the latest']")?.getBoundingClientRect(), t = document.querySelector("[data-msg]").getBoundingClientRect();
          return b ? { button: Math.round(b.left + b.width / 2 - m.left), text: Math.round(t.left + t.width / 2 - m.left) } : null; });
        log.push(`${T} way back down at 420: ${JSON.stringify(d)}`); await p.screenshot({ path: `${OUT}/${T}-down-420-shot.png` }); await p.setViewportSize({ width: 1440, height: 900 });
        must(d && Math.abs(d.button - d.text) <= 1, `way back down: ${JSON.stringify(d)}`); });
      // shut mid-drag (Ctrl+\): the drag ends where it was, no resize cursor or frozen transitions left behind; a press that
      // does not move keeps the kept width
      await step(p, `${T}-shut-mid-drag`, async () => { await drag(p, -40, true); await p.waitForTimeout(60); await p.keyboard.press("Control+Backslash"); await p.waitForTimeout(100); await p.mouse.up(); await p.waitForTimeout(400);
        const left = await p.evaluate(() => [document.documentElement.classList.contains("tz-sizing"), localStorage.getItem("tazuna.panel-width"), !!document.querySelector("aside[aria-label='The work']")]);
        await openPanel(p); const h = await p.locator(SEP).boundingBox(); await p.mouse.click(h.x + h.width / 2, 450); await p.waitForTimeout(200); const g = await geo(p);
        log.push(`${T} shut mid-drag: ${JSON.stringify(left)}, open again ${JSON.stringify(g)}`); must(!left[0] && left[1] === "460" && !left[2] && g.panel === 460 && g.kept === "460", `shut mid-drag: ${JSON.stringify([left, g])}`); });
      // a drag leaves the focus where it was: typing goes on in the box, and Enter or the arrows do not resize
      await step(p, `${T}-focus-stays`, async () => { await reset(p); const box = p.getByRole("textbox", { name: "Message" }); await box.click(); await p.keyboard.type("hello");
        const w0 = (await geo(p)).panel; await drag(p, -40); await p.keyboard.type(" world"); await p.keyboard.press("ArrowLeft");
        const got = { value: await box.inputValue(), focus: await p.evaluate(() => document.activeElement?.getAttribute("aria-label")), panel: (await geo(p)).panel };
        log.push(`${T} focus after a drag: ${JSON.stringify(got)} (from ${w0})`); await box.fill("");
        must(got.value === "hello world" && got.focus === "Message" && got.panel === w0 + 40, `focus after a drag: ${JSON.stringify(got)}`); });
      // the window left mid-drag (blur) or a move with the button up: the drag ends where it was, the mouse no longer resizes
      await step(p, `${T}-lost-release`, async () => { await reset(p); const w0 = (await geo(p)).panel; let [x, y] = await drag(p, -40, true); await p.waitForTimeout(60);
        await p.evaluate(() => window.dispatchEvent(new Event("blur"))); await p.mouse.move(x - 120, y); await p.waitForTimeout(100);
        const a = [(await geo(p)).panel, await p.evaluate(() => document.documentElement.classList.contains("tz-sizing"))]; await p.mouse.up(); await p.waitForTimeout(250);
        [x, y] = await drag(p, -24, true); await p.waitForTimeout(60);
        await p.locator(SEP).evaluate((e, x) => e.dispatchEvent(new PointerEvent("pointermove", { bubbles: true, buttons: 0, pointerId: 1, clientX: x, clientY: 400 })), x - 200); await p.mouse.move(x - 150, y); await p.waitForTimeout(100);
        const b = [(await geo(p)).panel, await p.evaluate(() => document.documentElement.classList.contains("tz-sizing"))]; await p.mouse.up(); await p.waitForTimeout(250);
        log.push(`${T} lost release from ${w0}: blur ${a}, buttons up ${b}`); must(a[0] === w0 + 40 && !a[1] && b[0] === w0 + 64 && !b[1], `lost release from ${w0}: ${a} ${b}`); });
      // a second pointer while one drags starts nothing: the drag goes on from where it began
      await step(p, `${T}-second-pointer`, async () => { await reset(p); const w0 = (await geo(p)).panel, [x, y] = await drag(p, -20, true); await p.waitForTimeout(60);
        await p.locator(SEP).evaluate((e, x) => e.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, cancelable: true, button: 0, buttons: 1, pointerId: 7, clientX: x, clientY: 400 })), x + 300);
        await p.mouse.move(x - 40, y); await p.mouse.up(); await p.waitForTimeout(250); const g = await geo(p);
        log.push(`${T} second pointer from ${w0}: ${g.panel}`); must(g.panel === w0 - (-40), `second pointer from ${w0}: ${JSON.stringify(g)}`); });
      // at 260 the Stats switch stands where it stood idle once a reply runs (the live dot keeps its room)
      await step(p, `${T}-260-stats-live`, async () => { await open(p, "Make the request logger"); await openPanel(p); await p.locator(SEP).focus(); await p.keyboard.press("Home"); await p.locator(SEP).blur(); await p.waitForTimeout(300);
        const at = () => p.evaluate(() => { const s = document.querySelector("section[aria-label=Stats]"), h = s.firstElementChild, dot = h.querySelector(".text-live-ink");
          return [Math.round(s.querySelector("[role=radio]").getBoundingClientRect().top), Math.round(h.getBoundingClientRect().height), dot ? getComputedStyle(dot).visibility : "none"]; });
        const idle = await at(); await p.locator(RAIL).screenshot({ path: `${OUT}/${T}-260-stats-rail-idle.png` });
        await p.evaluate(() => window.__MOCK__.emit("p6", (v) => ({ ...v, activity: "Thinking", blocks: [...v.blocks, { kind: "prompt", text: "And the README" }] }))); await p.waitForTimeout(500);
        const live = await at(); await p.locator(RAIL).screenshot({ path: `${OUT}/${T}-260-stats-rail-live.png` });
        await p.evaluate(() => window.__MOCK__.emit("p6", (v) => v)); await p.waitForTimeout(300);
        log.push(`${T} 260 Stats switch idle ${idle}, live ${live}`); must(idle[0] === live[0] && idle[1] === live[1] && idle[2] === "hidden" && live[2] === "visible", `260 Stats: idle ${idle}, live ${live}`); });
      // a smaller window holds the width (the kept one stays, a wider window gives it back)
      await step(p, `${T}-1372-held`, async () => { await drag(p, -600); await p.setViewportSize({ width: 1372, height: 900 }); await p.waitForTimeout(400); const g = await geo(p);
        log.push(`${T} 1372 with ${g.kept} kept: ${JSON.stringify(g)}`); must(g.panel === 480 && g.max === 480 && g.kept === "548" && even(g), `1372: ${JSON.stringify(g)}`); });
      // over the conversation at 1280: as wide as kept, to 45% of the window; its own edge resizes it
      await step(p, `${T}-1280-over`, async () => { await p.setViewportSize({ width: 1280, height: 860 }); await p.waitForTimeout(400); await openFix(p); await long(p); await p.waitForTimeout(300); await openPanel(p);
        const g = await geo(p); log.push(`${T} 1280 over: ${JSON.stringify(g)}`); must(g.over && g.panel === 548 && g.max === 576, `1280 over: ${JSON.stringify(g)}`); });
      await step(p, `${T}-1280-over-drag`, async () => { await drag(p, 200); const g = await geo(p), s = await spill(p); log.push(`${T} 1280 over, dragged: ${JSON.stringify(g)}; spill ${JSON.stringify(s)}`);
        must(g.over && g.panel === 348 && g.kept === "348" && !s.past.length, `1280 over, dragged: ${JSON.stringify([g, s])}`); });
      await step(p, `${T}-1280-over-260`, async () => { await p.locator(SEP).focus(); await p.keyboard.press("Home"); await p.waitForTimeout(300); const g = await geo(p), s = await spill(p);
        log.push(`${T} 1280 over at 260: ${JSON.stringify(g)}; spill ${JSON.stringify(s)}`); must(g.over && g.panel === 260 && s.wide <= 0 && !s.past.length && !s.cut.length, `1280 over 260: ${JSON.stringify([g, s])}`); });
      log.push(`${T} page errors: ${JSON.stringify(p.errs.slice(0, 5))}`);
      await ctx.close();
    }
    // where scrollbars always show (15 px, as on Windows; set before the app measures, on the root as it comes: WebKit runs
    // init scripts before there is one): the widest docked panel still leaves the box 580 px
    { const [ctx, p] = await ctxOf({ theme: "dark", w: 1440, h: 900, extra: AGAIN + ';new MutationObserver((_, o) => { if (document.documentElement) { document.documentElement.style.setProperty("--tz-sbw", "15px"); o.disconnect(); } }).observe(document, { childList: true });' });
      await step(p, "resize-scrollbars", async () => { await openFix(p); await openPanel(p); await p.locator(SEP).focus(); await p.keyboard.press("End"); await p.waitForTimeout(300);
        const g = await geo(p), box = await p.evaluate(() => Math.round(document.querySelector("[data-composer]").getBoundingClientRect().width));
        log.push(`resize scrollbars of 15 px at the most: ${JSON.stringify(g)}, box ${box}`); must(g.max === 533 && g.panel === 533 && box === 580, `scrollbars: ${JSON.stringify(g)} box ${box}`); });
      await ctx.close(); }
    if (process.env.ONLY === "resize") { fs.writeFileSync(`${OUT}/log.txt`, log.join("\n")); console.log(log.filter((l) => !l.startsWith("ok")).join("\n")); await browser.close(); if (log.some((l) => l.startsWith("FAIL"))) process.exitCode = 1; return; }
  }
  // Live stats (desktop/DESIGN.md, Motion; ONLY=live: these alone): a call streaming in the mock (the core's `live` estimate
  // every 50 ms: 0.8 s to the first token, then 200 t/s), the composer's strip and the Stats (tiles, rows) caught waiting for
  // the first token, mid-stream, as the call ends and once settled, then the last reply's speed after the reply; both themes,
  // Live stats on and off. On: the Out count moves between views and never goes back, the strip's parts after a moving number
  // stand still, the estimates are `muted` and exact ones `ink`; off: nothing moves until the call ends.
  if (!process.env.ONLY || process.env.ONLY === "live") {
    const must = (ok, what) => { if (!ok) throw new Error(what); };
    const VIBE = "Make the request logger";
    const read = (p) => p.evaluate(() => { const g = document.querySelector("[aria-label=\"This conversation's numbers\"]"), s = document.querySelector("section[aria-label=Stats]");
      const tile = [...s.querySelectorAll("div")].find((d) => d.textContent === "Out" && d.className.includes("text-faint"))?.nextElementSibling;
      const out = tile ?? [...s.querySelectorAll("span")].find((d) => d.textContent === "Out")?.parentElement.lastElementChild;
      const colour = (v) => { const i = document.createElement("i"); i.style.color = `var(${v})`; s.append(i); const c = getComputedStyle(i).color; i.remove(); return c; };
      const parts = [...g.children], speed = parts[parts.length - 1];
      return { strip: g.innerText.replace(/\s+/g, " "), lefts: parts.map((e) => e.getBoundingClientRect().left), cut: g.scrollWidth - g.clientWidth, out: out.textContent,
        outColour: getComputedStyle(out).color, muted: colour("--muted"), ink: colour("--ink"), speed: speed.innerText.replace(/\s+/g, " "), speedTitle: speed.querySelector("[title]")?.getAttribute("title") ?? "",
        stats: s.innerText.replace(/\s+/g, " ") }; });
    const ctxOfStrip = (r) => num(r.strip.match(/^([\d.]+k)/)?.[1] ?? "0");
    const num = (t) => parseFloat(t) * (/k$/.test(t) ? 1000 : 1);
    // the setting: Settings › Appearance › Live stats, On by default, kept
    for (const theme of ["dark", "light"]) { const [ctx, p] = await ctxOf({ theme, w: 1280, h: 800 });
      await step(p, `${theme[0]}-live-setting`, async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(300); await p.getByRole("button", { name: "Appearance" }).click();
        const g = p.getByRole("radiogroup", { name: "Live stats" });
        must(await g.getByRole("radio", { name: "On" }).getAttribute("aria-checked") === "true", "Live stats is not on by default");
        await g.getByRole("radio", { name: "Off" }).click(); await p.waitForTimeout(150); const off = await p.evaluate(() => localStorage.getItem("tazuna.live-stats"));
        await g.getByRole("radio", { name: "On" }).click(); await p.waitForTimeout(150); const on = await p.evaluate(() => localStorage.getItem("tazuna.live-stats"));
        must(off === "off" && on === "on", `not kept: ${off} ${on}`); await g.scrollIntoViewIfNeeded(); });
      await ctx.close(); }
    for (const theme of ["dark", "light"]) for (const layout of ["tiles", "rows"]) for (const mode of ["on", "off"]) {
      const T = `${theme[0]}-live-${layout}-${mode}`;
      const [ctx, p] = await ctxOf({ theme, w: 1440, h: 900, extra: `localStorage.setItem("tazuna.live-stats", "${mode}"); localStorage.setItem("tazuna.stats", "${layout}");` });
      await open(p, VIBE); await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(700); await p.mouse.move(700, 450);
      await p.evaluate(() => { window.__PLAY__.start(); window.__PLAY__.next(); }); await p.waitForTimeout(800);
      // the call starts: the context moves by its input eased (158.3k, the last call's input and output, to 158.9k), never a jump
      await p.evaluate(() => { window.__PLAY__.stream(200); });
      const start = []; for (let i = 0; i < 12; i++) { start.push(ctxOfStrip(await read(p))); await p.waitForTimeout(30); }
      await step(p, `${T}-wait`, async () => { const r = await read(p); log.push(`${T} waiting: ${r.strip}; the context at the start ${[...new Set(start)].join(" → ")}`);
        if (mode === "on") must(new Set(start).size >= 3 && start.every((x, i) => !i || x >= start[i - 1]), `the start jumped: ${start.join(" ")}`);
        must(mode === "on" ? /^(<0\.1|\d\.\d) s$/.test(r.speed) && r.speedTitle.startsWith("Time to first token, counting") : /t\/s$/.test(r.speed), `the first token's wait: ${r.speed} | ${r.speedTitle}`); });
      await p.waitForTimeout(700);
      await step(p, `${T}-mid`, async () => {
        const seen = []; for (let i = 0; i < 50; i++) { seen.push(await read(p)); await p.waitForTimeout(25); }
        const outs = seen.map((r) => r.out), still = seen.map((r) => r.lefts.slice(1).join()), r = seen[seen.length - 1];
        log.push(`${T} mid: Out ${[...new Set(outs)].slice(0, 6).join(" → ")}… (${new Set(outs).size} values), strip ${r.strip}`);
        must(seen.every((x) => x.cut <= 0), `the strip spills: ${r.cut}`);
        // on the plan (each call says so): at API prices, not billed, while it runs too; a sparkline's tile does not come for the call in flight
        must(r.strip.includes("at API prices · not billed") && !r.strip.includes("so far"), `the plan's words: ${r.strip}`);
        must(layout !== "tiles" || !r.stats.includes("Writing speed"), `a speed tile for the call in flight: ${r.stats}`);
        if (mode === "on") {
          must(outs.every((o, i) => !i || num(o) >= num(outs[i - 1])) && new Set(outs).size >= 12, `the count: ${outs.join(" ")}`);
          must(new Set(still).size === 1, `the strip moved: ${[...new Set(still)].join(" | ")}`);
          must(r.outColour === r.muted && /t\/s$/.test(r.speed), `the estimate: ${r.outColour} (muted ${r.muted}), ${r.speed}`);
        } else must(new Set(outs).size === 1 && r.outColour === r.ink, `off moved: ${[...new Set(outs)].join(" ")}`); });
      // the call ends: its exact numbers (260 + 1,900 out this turn), eased there
      await p.evaluate(() => { window.__PLAY__.next(); }); await p.waitForTimeout(120); await p.screenshot({ path: `${OUT}/${T}-settling.png` });
      await p.waitForTimeout(600);
      await step(p, `${T}-settled`, async () => { const r = await read(p); log.push(`${T} settled: Out ${r.out}, ${r.strip}`);
        // the context is the call's input and output (158.9k + 1.9k), as it was while it ran: no drop at the end
        must(r.out === "2.2k" && r.outColour === r.ink && /^160\.8k of 256k/.test(r.strip), `settled: ${JSON.stringify(r)}`); });
      await p.evaluate(() => { window.__PLAY__.end(); }); await p.waitForTimeout(500);
      await step(p, `${T}-final`, async () => { const r = await read(p); log.push(`${T} after the reply: ${r.speed} (${r.speedTitle})`);
        must(/^\d+ t\/s$/.test(r.speed) && r.speedTitle.startsWith("Average writing speed of the last reply"), `the last reply's speed: ${r.speed} | ${r.speedTitle}`); });
      // an estimate over the exact count (270 for 260) settles without counting back down
      if (mode === "on" && layout === "tiles") await step(p, `${T}-overshoot`, async () => {
        await p.evaluate(() => { window.__PLAY__.start(); window.__PLAY__.stream(2000); }); await p.waitForTimeout(1400);
        const before = (await read(p)).out; await p.evaluate(() => { window.__PLAY__.next(); });
        const after = []; for (let i = 0; i < 20; i++) { after.push((await read(p)).out); await p.waitForTimeout(25); }
        log.push(`${T} over the exact count: ${before} → ${[...new Set(after)].join(" → ")}`);
        must(before === "270" && after[after.length - 1] === "260" && after.every((o) => o === "270" || o === "260"), `counted down: ${before} ${after.join(" ")}`); });
      await ctx.close();
    }
    if (process.env.ONLY === "live") { fs.writeFileSync(`${OUT}/log.txt`, log.join("\n")); console.log(log.filter((l) => !l.startsWith("ok")).join("\n")); await browser.close(); if (log.some((l) => l.startsWith("FAIL"))) process.exitCode = 1; return; }
  }
  // the usage page's ranges (ONLY=usage: these alone): the menu by the keyboard, every preset and All with its bars and
  // words, a range picked by hand over midnight, one with nothing in it, the last 15 minutes with nothing (today's runs
  // left out), the range kept on reopening; both themes, 1000 px wide. Nothing wider than the page, the axis' three labels apart
  if (!process.env.ONLY || process.env.ONLY === "usage") {
    const must = (ok, what) => { if (!ok) throw new Error(what); };
    const local = (d) => new Date(d - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
    for (const theme of ["dark", "light"]) {
      const T = `range-${theme[0]}`, [ctx, p] = await ctxOf({ theme, w: 1000, h: 900 });
      const dlg = p.getByRole("dialog", { name: "Usage" }), btn = dlg.locator("button[aria-haspopup=menu]");
      const fits = () => dlg.evaluate((d) => { const body = d.querySelector(".overflow-auto"), axis = [...d.querySelectorAll(".justify-between > span")].map((s) => s.getBoundingClientRect());
        return { wide: body.scrollWidth - body.clientWidth, apart: axis.every((r, i) => !i || r.left >= axis[i - 1].right + 4), bars: d.querySelectorAll("[role=group] > span").length }; });
      const pick = async (name) => { await btn.click(); await p.getByRole("menuitem", { name }).click(); await p.waitForTimeout(300); };
      // what the page asks the core (the start page's own asks have no `since`)
      await p.evaluate(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T); window.__asks = [];
        T.invoke = (cmd, args, o) => { if (cmd === "op" && args?.msg?.op === "usage" && args.msg.since != null) window.__asks.push({ ...args.msg, tid: args.tid }); return inv(cmd, args, o); }; });
      const asks = () => p.evaluate(() => window.__asks);
      await p.keyboard.press("Control+Shift+U"); await p.waitForTimeout(900);
      await step(p, `${T}-menu`, async () => { await btn.focus(); await p.keyboard.press("Enter"); await p.waitForTimeout(150); await p.keyboard.press("ArrowDown");
        must(await p.getByRole("menuitem", { name: "Last hour" }).evaluate((e) => e === document.activeElement), "the menu's arrows"); });
      await step(p, `${T}-menu-esc`, async () => { await p.keyboard.press("Escape"); await p.waitForTimeout(250);
        must(await dlg.isVisible() && await btn.evaluate((e) => e === document.activeElement), "Esc shuts the menu alone, the focus back on its button"); });
      const want = { "Last 15 minutes": [/The last 15 minutes from the run log/, /max \$\S+ a minute/, [15, 16]], "Last hour": [/The last hour/, /per 5 minutes/, [12, 13]], "Last 3 hours": [/The last 3 hours/, /per 15 minutes/, [12, 13]],
        "Last 24 hours": [/The last 24 hours/, /an hour/, [24, 25]], "Last 7 days": [/The last 7 days/, /a day/, [7]], "Last 30 days": [/The last 30 days/, /a day/, [30]], "Last 90 days": [/The last 90 days/, /a day/, [90]],
        "All time": [/Everything in the run log, since \w+ \d+ \(the first run\)/, /a day/, [30, 31]] };
      for (const [name, [note, max, bars]] of Object.entries(want)) await step(p, `${T}-${name.toLowerCase().replace(/\s+/g, "-")}`, async () => {
        await pick(name); const text = await dlg.innerText(), f = await fits();
        must(note.test(text) && max.test(text) && bars.includes(f.bars) && f.wide <= 0 && f.apart && (await btn.innerText()).trim() === name, `${name}: ${JSON.stringify(f)} ${text.slice(text.indexOf("Spending"), text.indexOf("Spending") + 260)}`);
        await dlg.locator("[role=group]").hover(); });
      // the lower half under the last hour: the table and How the runs went, open, on the same range
      // the signals (every recording read) only once the fold opens, for the page's range
      await step(p, `${T}-last-hour-below`, async () => { await pick("Last hour"); const before = await asks();
        await dlg.getByText("How the runs went").click(); await p.waitForTimeout(200); const after = await asks();
        must(before.length && before.every((a) => !a.signals) && after.length === before.length + 1 && after.at(-1).signals && after.at(-1).since === before[0].since, `the signals: ${JSON.stringify([before, after.at(-1)])}`);
        await dlg.getByText("The same range as Spending").scrollIntoViewIfNeeded(); must((await fits()).wide <= 0, "the lower half wider than the page"); });
      await dlg.getByText("How the runs went").click({ timeout: 3000 }).catch(() => {});
      // the start page's refresh after a turn (90 days from later, no signals): the page asks its own range again, and keeps its rows meanwhile
      await step(p, `${T}-refresh`, async () => { const before = await asks(), a = before.at(-1);
        await p.evaluate((a) => window.__TAURI_INTERNALS__.emit("tazuna-view", { tid: a.tid, msg: { type: "usage", rows: [], since: a.since + 3600, signals: false } }), a); await p.waitForTimeout(300);
        const after = await asks(), f = await fits(); must(after.length === before.length + 1 && after.at(-1).since === a.since && [12, 13].includes(f.bars), `the refresh: ${JSON.stringify([a, after.at(-1), f])}`); });
      // yesterday 9am to 2pm today: hours over midnight
      const now = new Date(), y9 = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1, 9), t14 = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 14);
      await step(p, `${T}-custom`, async () => { await pick("Custom range…");
        must(await dlg.getByLabel("From").evaluate((e) => e === document.activeElement), "the range's start takes the focus");
        await dlg.getByLabel("From").fill(local(y9)); await dlg.getByLabel("To").fill(local(t14)); await p.waitForTimeout(300);
        const text = await dlg.innerText(), f = await fits(), label = (await btn.innerText()).trim();
        must(/^\w+ \d+, 9am – \w+ \d+, 2pm$/.test(label) && text.includes(`${label.replace(" – ", " to ")} from the run log.`) && /an hour/.test(text) && f.bars === 30 && f.wide <= 0 && f.apart, `custom: ${label} ${JSON.stringify(f)}`); });
      // the end before the start takes the start along
      await step(p, `${T}-custom-swap`, async () => { await dlg.getByLabel("To").fill(local(new Date(y9.getTime() - 3600e3))); await p.waitForTimeout(250);
        must(await dlg.getByLabel("From").inputValue() === local(new Date(y9.getTime() - 3600e3)), "start after end"); });
      // 8pm to 2am: nothing ran then in the test data
      await step(p, `${T}-custom-empty`, async () => { await dlg.getByLabel("From").fill(local(new Date(y9.getTime() + 11 * 3600e3))); await dlg.getByLabel("To").fill(local(new Date(t14.getTime() - 12 * 3600e3))); await p.waitForTimeout(300);
        const text = await dlg.innerText(); must(/Nothing ran from \w+ \d+, 8pm to \w+ \d+, 2am\./.test(text) && !(await fits()).bars, `custom empty: ${text.slice(text.indexOf("Spending"), text.indexOf("Spending") + 200)}`); });
      // a year before 1970 (years 1 to 99 read as 1901 to 1999) is no start: the range stays, the page never waits on it
      await step(p, `${T}-custom-1970`, async () => { const label = (await btn.innerText()).trim();
        for (const v of ["1950-01-01T00:00", "0050-01-01T00:00"]) { await dlg.getByLabel("From").fill(v); await p.waitForTimeout(200); }
        const text = await dlg.innerText(); must((await btn.innerText()).trim() === label && !text.includes("Reading the run log") && !(await asks()).some((a) => a.since < 0), `before 1970: ${label} ${text.slice(text.indexOf("Spending"), text.indexOf("Spending") + 120)}`); });
      // the range picked by hand comes back as it was, from another range or picked again
      await step(p, `${T}-custom-kept`, async () => { const label = (await btn.innerText()).trim(); await pick("Last hour"); await pick("Custom range…"); const back = (await btn.innerText()).trim();
        await pick("Custom range…"); must(back === label && (await btn.innerText()).trim() === label, `the custom range kept: ${label} | ${back}`); });
      await pick("Last 15 minutes"); await esc(p);
      // today's runs gone: the last 15 minutes empty, the range kept on reopening
      await p.evaluate(() => { window.__NO_TODAY__ = true; });
      await step(p, `${T}-empty`, async () => { await p.keyboard.press("Control+Shift+U"); await p.waitForTimeout(900); const text = await dlg.innerText();
        must(text.includes("Nothing ran in the last 15 minutes.") && !(await fits()).bars && (await btn.innerText()).trim() === "Last 15 minutes", `empty: ${text.slice(text.indexOf("Spending"), text.indexOf("Spending") + 200)}`); });
      log.push(`${T} page errors: ${JSON.stringify(p.errs.slice(0, 5))}`);
      await ctx.close();
    }
    // a rolling range moves on while the page is open: two minutes on, the last 15 minutes end two minutes later
    { const [ctx, p] = await ctxOf({ w: 1000, h: 900, extra: `localStorage.setItem("tazuna.usage-range", '{"key":"15m"}')` });
      await p.clock.install(); await p.keyboard.press("Control+Shift+U"); await p.waitForTimeout(900);
      const dlg = p.getByRole("dialog", { name: "Usage" }), right = () => dlg.locator(".justify-between > span").last().innerText();
      await step(p, "range-tick", async () => { const was = await right(); await p.clock.runFor(120_000); await p.waitForTimeout(200); const is = await right();
        must(was !== is && /\d(am|pm)$/.test(is), `the last 15 minutes after two minutes: ${was} → ${is}`); });
      await ctx.close(); }
    if (process.env.ONLY === "usage") { fs.writeFileSync(`${OUT}/log.txt`, log.join("\n")); console.log(log.filter((l) => !l.startsWith("ok")).join("\n") || "every step ok"); await browser.close(); if (log.some((l) => l.startsWith("FAIL"))) process.exitCode = 1; return; }
  }
  // design review 14's picks from mockups/v12.html (ONLY=v12: these alone, with v13's below; ONLY=v13: v13's alone): the composer's pills with no fill (a hairline on
  // hover), sentence-case modes and the short placeholder, in a 520 px box too; the Stats as rows (Settings › Appearance ›
  // Stats, kept) and the tiles' empty states; Ai, the default accent, in both themes with the in-page contrast audit
  // (review/audit.cjs), and the window under every accent; the reading column centred beside the work panel, gliding there
  // and back (snapping under reduced motion); the copy pass's strings; the window at 1000 and 1440 px
  if (!process.env.ONLY || process.env.ONLY === "v12" || process.env.ONLY === "v13") {
    const ok = (c, what) => { if (!c) throw new Error(what); };
    const VIBE = "Make the request logger", AUDIT = require("./audit.cjs");
    // the last view the window got, sent again changed (as LAST, further down)
    const AGAIN = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { if (p?.msg?.type === "view") window.__lastView = p; return emit(e, p); };
      window.__again = (f) => { const { tid, msg } = window.__lastView; T.view(tid, f({ ...msg, from: 0 })); }; })();`;
    const rowsOf = (p) => p.evaluate(() => { const s = document.querySelector("section[aria-label=Stats]"); if (!s) return null;
      const rs = [...s.querySelectorAll(".grid.min-h-\\[29px\\]")];
      return { rows: rs.map((r) => r.innerText.replace(/\s+/g, " ").trim()), lines: s.querySelectorAll(".grid svg path").length, tiles: s.querySelectorAll(".rounded-\\[10px\\]").length,
        dot: /live/.test(s.querySelector("div").innerText), wide: s.scrollWidth - s.clientWidth, quiet: rs.filter((r) => r.lastElementChild.className.includes("text-faint")).map((r) => r.firstElementChild.textContent) }; });
    const gaps = (p) => p.evaluate(() => { const m = document.querySelector("main").getBoundingClientRect(), t = document.querySelector("[data-msg]").getBoundingClientRect(), c = document.querySelector("[data-composer]").getBoundingClientRect();
      return { left: Math.round(t.left + 28 - m.left), right: Math.round(m.right - (t.right - 28)), text: Math.round(t.width - 56), boxLeft: Math.round(c.left - m.left), boxRight: Math.round(m.right - c.right) }; });
    if (process.env.ONLY !== "v13") for (const theme of ["dark", "light"]) {
      const T = `v12-${theme[0]}`, [ctx, p] = await ctxOf({ theme, w: 1440, h: 900, extra: AGAIN });
      await step(p, `${T}-pills`, async () => {
        await open(p, "Why is the CI build");
        const box = p.locator("[data-composer]"), mode = box.locator("button[aria-haspopup=menu]").first();
        const look = () => mode.evaluate((b) => { const cs = getComputedStyle(b); return { bg: cs.backgroundColor, border: cs.borderTopColor, text: b.innerText.trim(), icon: !!b.querySelector("svg") }; });
        const lineStrong = await p.evaluate(() => { const d = document.createElement("i"); d.style.color = "var(--line-strong)"; document.body.append(d); const c = getComputedStyle(d).color; d.remove(); return c; });
        const rest = await look(); await mode.hover(); await p.waitForTimeout(200); const hover = await look();
        log.push(`${T} mode pill: rest ${JSON.stringify(rest)}, hover ${JSON.stringify(hover)}`);
        ok(/rgba\(0, 0, 0, 0\)|transparent/.test(rest.bg) && /rgba\(0, 0, 0, 0\)|transparent/.test(rest.border) && rest.icon && /^[A-Z]/.test(rest.text), `the pill at rest: ${JSON.stringify(rest)}`);
        ok(hover.border === lineStrong && /rgba\(0, 0, 0, 0\)|transparent/.test(hover.bg), `the pill on hover: ${JSON.stringify(hover)} (${lineStrong} wanted)`);
        const ph = await p.getByRole("textbox", { name: "Message" }).getAttribute("placeholder");
        ok(ph === "/ for commands, @ for files", `the placeholder: ${ph}`);
        await box.screenshot({ path: `${OUT}/${T}-pills-hover-box.png` });
        await mode.focus(); await p.keyboard.press("Tab"); await p.keyboard.press("Shift+Tab"); await p.waitForTimeout(150);
        await box.screenshot({ path: `${OUT}/${T}-pills-focus-box.png` });
        // the menu: sentence case, six words at most a line
        await mode.click(); await p.waitForTimeout(200);
        const items = await p.getByRole("menuitemradio").allInnerTexts();
        log.push(`${T} mode menu: ${JSON.stringify(items)}`);
        ok(items.length === 6 && items.every((t) => /^[A-Z]/.test(t) && t.split("\n").pop().split(/\s+/).length <= 6), `the mode menu: ${JSON.stringify(items)}`);
        await p.screenshot({ path: `${OUT}/${T}-mode-menu.png` });
        await esc(p);
      });
      // a 520 px box, full mode and the longest model label: one row, nothing past the box
      await step(p, `${T}-pills-520`, async () => {
        await p.evaluate(() => window.__again((v) => ({ ...v, mode: "full" }))); await p.waitForTimeout(200);
        await p.evaluate(() => { document.querySelector("[data-composer]").style.width = "520px"; }); await p.waitForTimeout(200);
        const fit = await p.evaluate(() => { const b = document.querySelector("[data-composer]").getBoundingClientRect(), row = document.querySelector(".tz-pills"), kids = [...row.querySelectorAll("button")].map((e) => e.getBoundingClientRect());
          return { tops: [...new Set(kids.map((k) => Math.round(k.top + k.height / 2)))].length, past: kids.filter((k) => k.right > b.right + 0.5).length, over: row.scrollWidth - row.clientWidth, text: row.innerText.replace(/\s+/g, " ") }; });
        log.push(`${T} 520 px box: ${JSON.stringify(fit)}`);
        await p.locator("[data-composer]").screenshot({ path: `${OUT}/${T}-pills-520-box.png` });
        await p.evaluate(() => { document.querySelector("[data-composer]").style.width = ""; });
        ok(fit.tops === 1 && !fit.past && fit.over <= 0, `the pills in 520 px: ${JSON.stringify(fit)}`);
      });
      // Ai by default: nothing kept is Ai; the selected row's fill; the window's text against WCAG AA (review/audit.cjs)
      await step(p, `${T}-ai-default`, async () => {
        const got = await p.evaluate(() => { const r = getComputedStyle(document.documentElement), row = document.querySelector("aside[aria-label=Conversations] .bg-selected");
          return { kept: localStorage.getItem("tazuna.accent"), attr: document.documentElement.dataset.accent ?? "-", accent: r.getPropertyValue("--accent").trim(), live: r.getPropertyValue("--live").trim(), selected: r.getPropertyValue("--selected").trim(), row: row ? getComputedStyle(row).backgroundColor : null }; });
        log.push(`${T} Ai: ${JSON.stringify(got)}`);
        ok(got.kept === null && got.attr === "-" && got.accent === (theme === "dark" ? "#7fb0d0" : "#1d5f86") && got.live === got.accent && got.selected === (theme === "dark" ? "#303638" : "#e0e7e9"), `Ai: ${JSON.stringify(got)}`);
        const a = await p.evaluate(AUDIT); log.push(`${T} Ai audit, conversation: ${JSON.stringify({ contrast: a.contrast, clipped: a.clipped, overflowX: a.overflowX })}`);
      });
      // the Stats: tiles by default; rows from Settings, kept over a reload, no live dot, lines from 8 calls
      await step(p, `${T}-stats-tiles`, async () => { await open(p, VIBE); await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(700);
        const r = await rowsOf(p); ok(r.tiles >= 4 && !r.rows.length, `tiles by default: ${JSON.stringify(r)}`); });
      await step(p, `${T}-settings-stats`, async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(300);
        await p.getByRole("button", { name: "Appearance" }).click(); await p.waitForTimeout(200);
        const text = await p.getByRole("dialog", { name: "Settings" }).innerText();
        ok(!/System follows|Buttons, focus|larger or smaller|The type an answer/.test(text) && /Stats\s*Tiles\s*Rows/.test(text), `Appearance: ${text}`);
        await p.getByRole("radiogroup", { name: "Stats" }).getByRole("radio", { name: "Rows" }).click(); await p.waitForTimeout(200);
        ok(await p.evaluate(() => localStorage.getItem("tazuna.stats")) === "rows", "Rows was not kept"); });
      await esc(p);
      await step(p, `${T}-stats-rows-turn`, async () => { const r = await rowsOf(p); log.push(`${T} rows, this turn: ${JSON.stringify(r)}`);
        ok(r.rows.length >= 7 && !r.tiles && !r.lines && r.wide <= 0 && r.rows[0].startsWith("In") && r.rows.some((x) => /^Calls 3$/.test(x)) && r.rows.some((x) => /^Time to first token median \d/.test(x)), `rows, this turn: ${JSON.stringify(r)}`); });
      await step(p, `${T}-stats-rows-conversation`, async () => { await p.getByRole("radio", { name: "Conversation" }).click(); await p.waitForTimeout(300);
        const r = await rowsOf(p); log.push(`${T} rows, the conversation: ${JSON.stringify(r)}`); ok(r.lines === 3 && r.rows.some((x) => /^Calls 11$/.test(x)) && r.wide <= 0, `rows, 11 calls: ${JSON.stringify(r)}`); });
      // a reply just started: no call yet, zeros quiet, no line, no live dot
      await step(p, `${T}-stats-rows-running`, async () => { await p.getByRole("radio", { name: "This turn" }).click();
        await p.evaluate(() => window.__MOCK__.emit("p6", (v) => ({ ...v, activity: "Thinking", blocks: [...v.blocks, { kind: "prompt", text: "And the README" }] }))); await p.waitForTimeout(500);
        const r = await rowsOf(p); log.push(`${T} rows, running with no call: ${JSON.stringify(r)}`); ok(!r.dot && r.quiet.includes("Calls") && !r.rows.some((x) => /–/.test(x)), `rows while running: ${JSON.stringify(r)}`); });
      await step(p, `${T}-stats-rows-kept`, async () => { await p.reload(); await p.waitForTimeout(1100); await open(p, VIBE); await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(700);
        const r = await rowsOf(p); ok(r.rows.length >= 7, `rows after a reload: ${JSON.stringify(r)}`); });
      // the tiles' empty states: back to tiles, a reply with one call so far: no sparkline box, quiet zeros, never an empty "–" box
      await step(p, `${T}-stats-tiles-empty`, async () => { await p.evaluate(() => { localStorage.setItem("tazuna.stats", "tiles"); window.dispatchEvent(new Event("tz-looks")); });
        await p.evaluate(() => window.__MOCK__.emit("p6", (v) => ({ ...v, activity: "Thinking", blocks: [...v.blocks, { kind: "prompt", text: "And the README" }] }))); await p.waitForTimeout(500);
        const t = await p.evaluate(() => { const s = document.querySelector("section[aria-label=Stats]"); return { text: s.innerText.replace(/\s+/g, " "), sparks: s.querySelectorAll("[role=group]").length, quiet: [...s.querySelectorAll(".text-faint.leading-\\[25px\\]")].map((e) => e.textContent) }; });
        log.push(`${T} tiles, no call yet: ${JSON.stringify(t)}`);
        await p.screenshot({ path: `${OUT}/${T}-stats-tiles-no-call.png` });
        ok(!t.sparks && !/Time to first token|Writing speed|Cache hit rate/.test(t.text) && t.quiet.includes("0"), `the tiles' empty state: ${JSON.stringify(t)}`);
        await p.evaluate(() => window.__MOCK__.emit("p6", (v) => v)); await p.waitForTimeout(300); });
      // the reading column beside the panel: centred (equal sheet each side within 1 px), at 1440 and 1600; the glide sampled
      // the header's panel button, pressed in the page (the pointer resting on it takes its title for the tooltip)
      const panelKey = () => p.evaluate(() => document.querySelector("main > header button[data-panel]").click());
      for (const w of [1440, 1600]) await step(p, `${T}-centred-${w}`, async () => {
        await p.setViewportSize({ width: w, height: 900 }); await p.waitForTimeout(300); await open(p, "Why is the CI build");
        if (await p.locator("aside[aria-label='The work']").evaluate((a) => a.getBoundingClientRect().width > 0).catch(() => false)) { await panelKey(); await p.waitForTimeout(400); }
        const shut = await gaps(p);
        const frames = await p.evaluate(async () => { const el = document.querySelector("[data-msg]"), out = []; const t0 = performance.now();
          document.querySelector("main > header button[data-panel]").click();
          while (performance.now() - t0 < 320) { await new Promise((r) => requestAnimationFrame(r)); out.push([Math.round(performance.now() - t0), Math.round(el.getBoundingClientRect().left * 10) / 10]); } return out; });
        const open_ = await gaps(p);
        const steps = frames.slice(1).map((f, i) => f[1] - frames[i][1]), most = Math.max(...steps.map(Math.abs)), travel = Math.abs(frames.at(-1)[1] - frames[0][1]);
        log.push(`${T} ${w}: shut ${JSON.stringify(shut)}; open ${JSON.stringify(open_)}; glide ${frames.map((f) => `${f[0]}ms:${f[1]}`).join(" ")} (largest step ${most} px of ${travel})`);
        await p.screenshot({ path: `${OUT}/${T}-centred-${w}-open.png` });
        ok(Math.abs(open_.left - open_.right) <= 1 && Math.abs(shut.left - shut.right) <= 1 && open_.text === 700 && Math.abs(open_.boxLeft - open_.boxRight) <= 1, `not centred at ${w}: ${JSON.stringify([shut, open_])}`);
        // a glide: frames between the two ends, one way only, never the whole way in one frame (a headless sample at ~30 fps
        // of the ease-out: a slow frame took 113 of 148 px, a jump would take it all)
        ok(frames.filter((f) => f[1] !== frames[0][1] && f[1] !== frames.at(-1)[1]).length >= 2 && (steps.every((d) => d <= 0) || steps.every((d) => d >= 0)) && most < travel * 0.9, `no glide at ${w}: largest step ${most} of ${travel}`);
        await panelKey(); await p.waitForTimeout(400);
        const back = await gaps(p); ok(JSON.stringify(back) === JSON.stringify(shut), `closing did not come back: ${JSON.stringify([shut, back])}`);
      });
      await p.setViewportSize({ width: 1440, height: 900 });
      // mid-glide frames, to look at
      await step(p, `${T}-glide-frames`, async () => { await open(p, "Why is the CI build");
        await panelKey();
        for (const ms of [40, 90]) { await p.waitForTimeout(ms === 40 ? 40 : 50); await p.screenshot({ path: `${OUT}/${T}-glide-${ms}ms.png` }); }
        await p.waitForTimeout(400); await panelKey(); await p.waitForTimeout(400); });
      // the window under every accent (rows and the panel open, a selected row, the composer)
      await p.evaluate(() => { localStorage.setItem("tazuna.stats", "rows"); window.dispatchEvent(new Event("tz-looks")); });
      await open(p, VIBE); await p.getByTitle(/^Open the work panel/).click(); await p.waitForTimeout(600);
      for (const accent of ["ai", "indigo", "jade", "amber", "rose", "graphite"]) await step(p, `${T}-window-${accent}`, async () => {
        await p.evaluate((a) => { if (a === "ai") delete document.documentElement.dataset.accent; else document.documentElement.dataset.accent = a; }, accent); await p.waitForTimeout(150);
        const a = await p.evaluate(AUDIT); log.push(`${T} ${accent} audit: ${JSON.stringify({ contrast: a.contrast, clipped: a.clipped })}`); });
      await p.evaluate(() => { delete document.documentElement.dataset.accent; localStorage.removeItem("tazuna.stats"); });
      log.push(`${T} page errors: ${JSON.stringify(p.errs.slice(0, 5))}`);
      await ctx.close();
      // reduced motion: the column snaps (its first frame is already the last)
      const [c3, q] = await ctxOf({ theme, w: 1440, h: 900, motion: "reduce" });
      await step(q, `${T}-centred-snap`, async () => { await open(q, "Why is the CI build");
        const frames = await q.evaluate(async () => { const el = document.querySelector("[data-msg]"), out = []; document.querySelector("main > header button[data-panel]").click();
          for (let i = 0; i < 8; i++) { await new Promise((r) => requestAnimationFrame(r)); out.push(Math.round(el.getBoundingClientRect().left)); } return out; });
        log.push(`${T} reduced motion: ${frames.join(" ")}`); ok(new Set(frames).size === 1, `it glided under reduced motion: ${frames}`); });
      await c3.close();
      // 1000 px (the panel over the conversation) and the copy pass's places: the start page, the setup dialog, Settings
      const [c4, r] = await ctxOf({ theme, w: 1000, h: 760 });
      await step(r, `${T}-1000-conversation`, async () => { await open(r, "Why is the CI build"); });
      await step(r, `${T}-1000-panel`, async () => { await r.getByTitle(/^Open the work panel/).click(); await r.waitForTimeout(500); });
      await esc(r);
      await step(r, `${T}-1000-start`, async () => { await r.keyboard.press("Control+n"); await r.waitForTimeout(500);
        const t = await r.locator("main").innerText(); ok(!t.includes("any project on this machine"), "the start page's subline is back"); });
      await step(r, `${T}-1000-providers`, async () => { await r.keyboard.press("Control+,"); await r.waitForTimeout(300); await r.getByRole("button", { name: "Providers" }).click(); await r.waitForTimeout(300);
        const t = await r.getByRole("dialog", { name: "Settings" }).innerText(); ok(t.includes("Claude runs on your subscription; other providers take a key or a sign-in."), "the providers' line"); });
      await step(r, `${T}-1000-mcp`, async () => { await r.getByRole("button", { name: "Tools from MCP servers" }).click(); await r.waitForTimeout(300);
        const t = await r.getByRole("dialog", { name: "Settings" }).innerText(); ok(!/terminal's|\/mcp\b(?!\.)/.test(t.replace(/https?:\S+/g, "")), `a terminal command in Settings: ${t}`); });
      await step(r, `${T}-1000-appearance`, async () => { await r.getByRole("button", { name: "Appearance" }).click(); await r.waitForTimeout(200); });
      await esc(r);
      await step(r, `${T}-1000-setup`, async () => { await r.keyboard.press("Control+k"); await r.keyboard.type("Set up tazuna"); await r.keyboard.press("Enter"); await r.waitForTimeout(500);
        const t = await r.getByRole("dialog").first().innerText(); ok(t.includes("Everything here can wait. Ctrl+K › Set up tazuna brings this back."), `the setup's first line: ${t.slice(0, 200)}`); });
      await esc(r);
      await step(r, `${T}-1000-approval`, async () => { await open(r, "Bump the TLS library"); await r.getByText("Allow a shell command?").waitFor({ timeout: 4000 }); });
      log.push(`${T} 1000 page errors: ${JSON.stringify(r.errs.slice(0, 5))}`);
      await c4.close();
    }
    // P7, the conversation numbers (mockups/v13.html, M-A): the composer's strip under the box by default, above it, in the
    // status bar, off; each number once; a click (or Enter) opens Stats over the conversation; plan, free and running; 1000
    // px; the start page has none; the route line hidden
    const FREE = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { const m = p?.msg;
      if (window.__free && m?.type === "models") m.choices = [...m.choices.map((c) => ({ ...c, current: false })), { model: "qwen3-coder:30b", label: "Qwen3 Coder 30B", provider: "Ollama", place: "this machine", price: "free", url: "http://localhost:11434/v1", key_env: null, why_not: null, efforts: [], effort: null, current: true }];
      return emit(e, p); }; })();`;
    const strip = (p) => p.evaluate(() => { const c = document.querySelector("[data-composer]"), s = c?.querySelector(".tz-nums"), bar = document.querySelector("footer[aria-label=Status]").innerText;
      return s ? { at: s === c.firstElementChild || s.previousElementSibling === c.querySelector("textarea[aria-hidden]") ? "top" : s === c.lastElementChild ? "under" : "middle", text: s.innerText.replace(/\s+/g, " ").trim(), drop: s.dataset.drop,
        h: Math.round(s.getBoundingClientRect().height), over: s.scrollWidth - s.clientWidth, first: Math.round(s.querySelector("button svg").getBoundingClientRect().left - c.querySelector("textarea:not([aria-hidden])").getBoundingClientRect().left), titles: [...s.querySelectorAll("button")].map((b) => b.title || b.getAttribute("aria-description")), bar }
        : { at: "none", bar }; });
    const setNumbers = (p, v) => p.evaluate((x) => { localStorage.setItem("tazuna.numbers", x); window.dispatchEvent(new Event("tz-looks")); }, v);
    for (const theme of ["dark", "light"]) {
      const T = `v13-${theme[0]}`, [ctx, p] = await ctxOf({ theme, w: 1440, h: 900, extra: AGAIN + FREE });
      const box = (name) => p.locator("[data-composer]").screenshot({ path: `${OUT}/${T}-${name}-box.png` });
      await step(p, `${T}-start-none`, async () => { const s = await strip(p); ok(s.at === "none", `a strip on the start page: ${JSON.stringify(s)}`); });
      await step(p, `${T}-under`, async () => { await open(p, "Why is the CI build"); await p.mouse.move(5, 400); await p.waitForTimeout(200); const s = await strip(p); log.push(`${T} under: ${JSON.stringify(s)}`);
        ok(s.at === "under" && s.h === 32 && Math.abs(s.first) <= 1 && !/context|cache \d|this chat \$/.test(s.bar), `the strip under the box, the status bar without the numbers: ${JSON.stringify(s)}`); await box("under"); });
      await step(p, `${T}-opens-stats`, async () => { await p.locator(".tz-nums button").first().click(); await p.waitForTimeout(700);
        const sc = await p.getByRole("radio", { name: "Conversation" }).getAttribute("aria-checked"); ok(sc === "true", `Stats not over the conversation: ${sc}`); });
      await p.evaluate(() => document.querySelector("main > header button[data-panel]")?.click()); await p.waitForTimeout(400);
      await step(p, `${T}-keys`, async () => { await p.getByRole("textbox", { name: "Message" }).focus(); await p.keyboard.press("ArrowLeft"); await p.locator(".tz-nums button").first().focus(); await box("focus");
        await p.keyboard.press("Enter"); await p.waitForTimeout(600); ok(await p.locator("section[aria-label=Stats]").count() === 1, "Enter on a number did not open Stats"); });
      await p.evaluate(() => document.querySelector("main > header button[data-panel]")?.click()); await p.waitForTimeout(400);
      // a plan at API prices (the Mistral Vibe plan), then one with no reference price (Z.ai Coding Plan)
      await step(p, `${T}-plan`, async () => { await open(p, VIBE); await p.mouse.move(5, 400); const s = await strip(p); log.push(`${T} plan: ${JSON.stringify(s)}`);
        ok(/\$[\d.]+ at API prices · not billed/.test(s.text) && s.titles.some((t) => /On your Mistral Vibe plan/.test(t)), `the plan's cost: ${JSON.stringify(s)}`); await box("plan"); });
      await step(p, `${T}-plan-no-ref`, async () => { await open(p, "Rename the config loader"); await p.mouse.move(5, 400);
        // every turn on the plan with no reference price (the mock's last one was paid)
        await p.evaluate(() => window.__again((v) => ({ ...v, blocks: v.blocks.map((b) => (b.kind === "done" ? { ...b, plan: true, provider: "Z.ai Coding Plan", price_m: undefined, model: b.model ?? "glm-5.3", cost: 0, run_cost: 0 } : b)) }))); await p.waitForTimeout(300);
        const s = await strip(p); log.push(`${T} no reference: ${JSON.stringify(s)}`);
        ok(/not billed \(Z\.ai Coding Plan\)/.test(s.text) && !/\$0(?!\.)/.test(s.text), `a plan with no reference price: ${JSON.stringify(s)}`); await box("plan-no-ref"); });
      // running: t/s comes last, nothing before it moves
      await step(p, `${T}-running`, async () => { await open(p, VIBE); const before = await strip(p);
        await p.evaluate(() => window.__MOCK__.emit("p6", (v) => ({ ...v, activity: "Tool", blocks: [...v.blocks, { kind: "prompt", text: "And the README" }],
          calls: [...v.calls, { block: v.blocks.length + 1, fresh: 900, cached: 157000, out: 120, first_ms: 800, tps: 210, total_ms: 1500, role: null, model: "mistral-medium-3.5", provider: "Mistral Vibe", cost: 0.004 }] }))); await p.waitForTimeout(500);
        const s = await strip(p); log.push(`${T} running: ${JSON.stringify(s)}`);
        ok(/\d+ t\/s$/.test(s.text) && s.first === before.first && s.over <= 0, `running: ${JSON.stringify([before, s])}`); await box("running"); await p.screenshot({ path: `${OUT}/${T}-running-window.png` }); });
      await p.evaluate(() => window.__MOCK__.emit("p6", (v) => v)); await p.waitForTimeout(300);
      // a local model: "free on this machine", no cache part
      await step(p, `${T}-free`, async () => { await open(p, "Why is the CI build");
        await p.evaluate(() => { window.__free = true; window.__TAURI_INTERNALS__.invoke("op", { tid: window.__lastView.tid, msg: { op: "models" } }); }); await p.waitForTimeout(300);
        await p.evaluate(() => window.__again((v) => ({ ...v, model: "qwen3-coder:30b", cost: 0, context_window: null, usage: { ...v.usage, cache_read: 0, cache_write: 0 },
          calls: (v.calls ?? []).map((c) => ({ ...c, fresh: c.fresh + c.cached, cached: 0, cost: 0 })), blocks: v.blocks.map((b) => (b.kind === "done" ? { ...b, cost: 0, run_cost: 0, plan: false, provider: undefined, estimate: false } : b)) }))); await p.waitForTimeout(400);
        const s = await strip(p); log.push(`${T} free: ${JSON.stringify(s)}`); ok(/free on this machine/.test(s.text) && !/cached|%/.test(s.text), `a local model: ${JSON.stringify(s)}`); await box("free");
        // its Cost tile: "$0" quiet, not a big number
        await p.locator(".tz-nums button").first().click(); await p.waitForTimeout(500);
        const q = await p.evaluate(() => [...document.querySelectorAll("section[aria-label=Stats] .leading-\\[25px\\]")].map((e) => e.textContent));
        await p.evaluate(() => document.querySelector("main > header button[data-panel]")?.click()); await p.waitForTimeout(300);
        ok(q.includes("$0"), `the free model's Cost tile: ${JSON.stringify(q)}`); });
      await p.evaluate(() => { window.__free = false; window.__TAURI_INTERNALS__.invoke("op", { tid: window.__lastView.tid, msg: { op: "models" } }); }); await p.waitForTimeout(300);
      // the route line: a paid model says nothing under the box now
      await step(p, `${T}-route-line-hidden`, async () => { await open(p, "Why is the CI build");
        await p.evaluate(() => window.__TAURI_INTERNALS__.invoke("op", { tid: window.__lastView.tid, msg: { op: "model", model: "deepseek-v4.1-flash", url: null, key_env: null } })); await p.waitForTimeout(500);
        const under = await p.evaluate(() => document.querySelector("[data-composer]").parentElement.innerText); ok(!/→ .* paid/.test(under), `the route line shows: ${under}`); });
      // the setting: above, the status bar, off, and back
      await step(p, `${T}-above`, async () => { await setNumbers(p, "above"); await p.waitForTimeout(200); const s = await strip(p); ok(s.at === "top" && !/this chat \$/.test(s.bar), `above: ${JSON.stringify(s)}`); await box("above"); });
      await step(p, `${T}-in-bar`, async () => { await setNumbers(p, "bar"); await p.waitForTimeout(200); const s = await strip(p); log.push(`${T} in the status bar: ${s.bar}`); ok(s.at === "none" && /context|this chat/.test(s.bar), `the status bar: ${JSON.stringify(s)}`);
        await p.screenshot({ path: `${OUT}/${T}-in-bar-window.png` }); });
      await step(p, `${T}-off`, async () => { await setNumbers(p, "off"); await p.waitForTimeout(200); const s = await strip(p); ok(s.at === "none" && !/context|this chat/.test(s.bar), `off: ${JSON.stringify(s)}`); });
      await step(p, `${T}-settings-numbers`, async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(300); await p.getByRole("button", { name: "Appearance" }).click();
        await p.getByRole("radiogroup", { name: "Conversation numbers" }).getByRole("radio", { name: "Under the box" }).click(); await p.waitForTimeout(200);
        ok(await p.evaluate(() => localStorage.getItem("tazuna.numbers")) === "under", "Under the box was not kept"); });
      await esc(p);
      // the review's fixes: the cost opens the conversation's slip with the strip on; Tab reaches the numbers, Shift+Tab
      // from the box with them above; the speed opens Stats at this turn; a running reply keeps its cost ("so far")
      await step(p, `${T}-cost-slip`, async () => { await setNumbers(p, "under"); await open(p, VIBE); await p.mouse.move(5, 400);
        await p.locator(".tz-nums button").nth(1).click(); await p.waitForTimeout(400);
        const t = await p.getByRole("dialog", { name: "This conversation's receipt" }).innerText(); ok(/Total|Mistral Vibe/.test(t), `the slip from the strip: ${t.slice(0, 120)}`);
        await p.screenshot({ path: `${OUT}/${T}-cost-slip.png` }); await esc(p); });
      await step(p, `${T}-tab-reach`, async () => { await p.getByRole("textbox", { name: "Message" }).focus(); await p.keyboard.press("Tab");
        const at = await p.evaluate(() => !!document.activeElement?.closest(".tz-nums")); ok(at, "Tab from the box does not reach the numbers"); });
      await step(p, `${T}-shift-tab-above`, async () => { await setNumbers(p, "above"); await p.waitForTimeout(200); const mode0 = await p.locator("[data-composer] button[aria-haspopup=menu]").first().innerText();
        await p.getByRole("textbox", { name: "Message" }).focus(); await p.keyboard.press("Shift+Tab"); await p.waitForTimeout(150);
        const at = await p.evaluate(() => !!document.activeElement?.closest(".tz-nums")), mode1 = await p.locator("[data-composer] button[aria-haspopup=menu]").first().innerText();
        ok(at && mode0 === mode1, `Shift+Tab with the numbers above: in the strip ${at}, mode ${mode0} → ${mode1}`); await box("shift-tab-above"); await setNumbers(p, "under"); });
      await step(p, `${T}-running-keeps-cost`, async () => {
        const read = () => p.evaluate(() => { const s = document.querySelector(".tz-nums"), b = [...s.querySelectorAll("button")], t = (e) => e.getAttribute("title") ?? e.getAttribute("aria-description") ?? e.getAttribute("aria-label") ?? "";
          return { text: s.innerText.replace(/\s+/g, " "), ctx: Math.round(b[0].getBoundingClientRect().left), n: b.length, titles: b.map(t) }; });
        await open(p, VIBE); const before = await read();
        // Claude's own models bring no model on a call: no running sum; the finished turns' stays, "so far"
        await p.evaluate(() => window.__MOCK__.emit("p6", (v) => ({ ...v, activity: "Tool", blocks: [...v.blocks, { kind: "prompt", text: "And the README" }],
          calls: [...v.calls, { block: v.blocks.length + 1, fresh: 900, cached: 157000, out: 120, first_ms: 800, tps: 210, total_ms: 1500, role: null }] }))); await p.waitForTimeout(400);
        const during = await read(); log.push(`${T} a reply with no price yet: ${JSON.stringify([before, during])}`);
        // the speed was there before too (the last reply's average, desktop 0.1.131): the reply's own now, nothing comes or goes
        ok(/\$0\.069 so far/.test(during.text) && during.n === before.n && during.ctx === before.ctx, `the cost during a reply: ${JSON.stringify([before, during])}`);
        ok(during.titles.some((x) => x.startsWith("This conversation's cost so far")) && during.titles.at(-1).startsWith("The reply's writing speed"), `the titles during a reply: ${JSON.stringify(during.titles)}`);
        await p.locator(".tz-nums button").last().click(); await p.waitForTimeout(600);
        ok(await p.getByRole("radio", { name: "This turn" }).getAttribute("aria-checked") === "true", "the speed did not open Stats at this turn");
        await p.evaluate(() => document.querySelector("main > header button[data-panel]")?.click()); await p.evaluate(() => window.__MOCK__.emit("p6", (v) => v)); await p.waitForTimeout(400); });
      // a side chat has the column: a number opens nothing, nor later by itself, and a Stats opened by hand is not flipped
      await step(p, `${T}-side-chat-owns`, async () => { await open(p, VIBE); await p.getByRole("button", { name: "Side chat", exact: true }).click(); await p.waitForTimeout(500);
        await p.locator(".tz-nums button").first().click(); await p.waitForTimeout(300);
        await p.getByRole("button", { name: "Side chat", exact: true }).click(); await p.waitForTimeout(500);
        const opened = await p.locator("section[aria-label=Stats]").count();
        await p.evaluate(() => document.querySelector("main > header button[data-panel]").click()); await p.waitForTimeout(600);
        const scope = await p.getByRole("radio", { name: "This turn" }).getAttribute("aria-checked");
        ok(!opened && scope === "true", `with a side chat: opened by itself ${opened}, This turn ${scope}`);
        await p.evaluate(() => document.querySelector("main > header button[data-panel]").click()); await p.waitForTimeout(400); });
      // the palette's efforts and the picker's effort keys in sentence case; the providers' line on one line
      await step(p, `${T}-providers-one-line`, async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(300); await p.getByRole("button", { name: "Providers" }).click(); await p.waitForTimeout(300);
        const h = await p.getByText("Claude runs on your subscription; other providers take a key or a sign-in.").evaluate((e) => Math.round(e.getBoundingClientRect().height)); ok(h <= 22, `the providers' line is ${h} px high`); });
      await esc(p);
      log.push(`${T} page errors: ${JSON.stringify(p.errs.slice(0, 5))}`);
      await ctx.close();
      // 1000 px: running, with the plan's long words; one row, what does not fit dropped in order. Larger text is the
      // webview's zoom: a 1000 px window at 1.25 is 800 CSS px wide on a 1.25 scale
      for (const [w, zoom] of [[1000, 1], [1000, 1.25]]) {
        const [c5, r] = await ctxOf({ theme, w: w / zoom, h: 760 / zoom, scale: zoom, extra: AGAIN }), N = `${T}-${w}${zoom > 1 ? "-zoom" : ""}`;
        await step(r, `${N}-running`, async () => { if (w / zoom < 1000) { await r.getByRole("button", { name: "Conversations", exact: true }).click(); await r.waitForTimeout(250); } await open(r, VIBE);
          await r.evaluate(() => window.__MOCK__.emit("p6", (v) => ({ ...v, activity: "Tool", blocks: [...v.blocks, { kind: "prompt", text: "And the README" }],
            calls: [...v.calls, { block: v.blocks.length + 1, fresh: 900, cached: 157000, out: 120, first_ms: 800, tps: 210, total_ms: 1500, role: null, model: "mistral-medium-3.5", provider: "Mistral Vibe", cost: 0.004 }] }))); await r.waitForTimeout(500);
          const s = await strip(r); log.push(`${N} running: ${JSON.stringify(s)}`); ok(s.at === "under" && s.h === 32 && s.over <= 0, `${N}: ${JSON.stringify(s)}`);
          await r.locator("[data-composer]").screenshot({ path: `${OUT}/${N}-running-box.png` });
          // a box narrower than any window gives it (a side chat's): parts drop in order, one row, nothing past it
          await r.evaluate(() => { document.querySelector("[data-composer]").style.width = "400px"; }); await r.waitForTimeout(300);
          const n = await strip(r); log.push(`${N} in 400 px: ${JSON.stringify(n)}`);
          await r.locator("[data-composer]").screenshot({ path: `${OUT}/${N}-400-box.png` });
          await r.evaluate(() => { document.querySelector("[data-composer]").style.width = ""; });
          ok(Number(n.drop) >= 2 && n.h === 32 && n.over <= 0 && !/cached|so far/.test(n.text), `400 px: ${JSON.stringify(n)}`); });
        await c5.close();
      }
      // 1000 px: the panel opens over the conversation; a press on a number keeps it open (no shut-and-open), Esc gives the
      // number its focus back; Settings at Larger text keeps the numbers' keys inside the card
      { const [c6, r] = await ctxOf({ theme, w: 1000, h: 760 }), [c7, z] = await ctxOf({ theme, w: 800, h: 608, scale: 1.25 });
        await open(r, VIBE);
        await step(r, `${T}-effort-labels`, async () => { await r.getByRole("textbox", { name: "Message" }).focus(); await r.keyboard.press("Control+k"); await r.keyboard.type("effort"); await r.waitForTimeout(300);
          await r.screenshot({ path: `${OUT}/${T}-effort-palette.png` });
          const t = await r.evaluate(() => [...document.querySelectorAll("[role=dialog]")].map((d) => d.innerText).join(" | ")); await esc(r); log.push(`${T} palette efforts: ${JSON.stringify(t)}`);
          ok(/Effort\n(Off|Low)\n.*High/s.test(t) && !/\n(high|low|max|off)\n/.test(t), `the palette's efforts: ${JSON.stringify(t)}`); });
        await step(r, `${T}-1000-floating`, async () => { await r.mouse.move(600, 300); const b = r.locator(".tz-nums button").first();
          await b.click(); await r.waitForTimeout(500); const one = await r.locator("div[data-panel].tz-pop").count();
          await b.click(); await r.waitForTimeout(500); const two = await r.locator("div[data-panel].tz-pop").count();
          await r.keyboard.press("Escape"); await r.waitForTimeout(300); const back = await r.evaluate(() => !!document.activeElement?.closest(".tz-nums"));
          ok(one === 1 && two === 1 && back, `the floating panel: open ${one}, after a second press ${two}, focus back ${back}`); });
        await step(z, `${T}-1000-larger-settings`, async () => { await z.keyboard.press("Control+,"); await z.waitForTimeout(300);
          await z.getByRole("button", { name: "Appearance" }).click(); await z.waitForTimeout(200);
          const fit = await z.evaluate(() => { const g = document.querySelector("[role=radiogroup][aria-label='Conversation numbers']").getBoundingClientRect(), card = document.querySelector("[role=radiogroup][aria-label='Conversation numbers']").closest(".rounded-lg").getBoundingClientRect(); return { past: Math.round(g.right - card.right) }; });
          log.push(`${T} Larger text, the numbers' keys: ${JSON.stringify(fit)}`); ok(fit.past <= 0, `past the card: ${JSON.stringify(fit)}`); });
        await c6.close(); await c7.close(); }
    }
    if (process.env.ONLY === "v12" || process.env.ONLY === "v13") { fs.writeFileSync(`${OUT}/log.txt`, log.join("\n")); console.log(log.filter((l) => !l.startsWith("ok")).join("\n") || "every step ok"); await browser.close(); if (log.some((l) => l.startsWith("FAIL"))) process.exitCode = 1; return; }
  }
  // review 14, usage sources (ONLY=sources: these alone): Settings › Providers' usage line for each way of reading (the API,
  // a key's permission, an admin key, the CLI's commands, the console's sign-in and what its first visit found, the allowance
  // form, none), the Track switch on and off, and an untracked provider gone from the status bar and the usage page
  if (!process.env.ONLY || process.env.ONLY === "sources") {
    const ok = (c, what) => { if (!c) throw new Error(what); };
    for (const theme of ["dark", "light"]) {
      const T = `src-${theme[0]}`, [ctx, p] = await ctxOf({ theme });
      const row = (name) => p.getByRole("switch", { name: `Track ${name}'s usage` }).locator("xpath=ancestor::div[contains(@class,'py-3')][1]");
      const shot = async (name, file) => { await row(name).scrollIntoViewIfNeeded(); await p.waitForTimeout(250); await row(name).screenshot({ path: `${OUT}/${T}-${file}.png` }); };
      const said = async (name) => (await row(name).innerText()).replace(/\s+/g, " ");
      const before = await p.getByText("$12.40", { exact: false }).count();
      await step(p, `${T}-providers`, async () => { await p.keyboard.press("Control+,"); await p.waitForTimeout(600); await p.getByRole("button", { name: "Providers", exact: true }).click(); await p.waitForTimeout(400);
        ok(await p.getByRole("switch", { name: /^Track / }).count() === 6, `usage lines: ${await p.getByRole("switch", { name: /^Track / }).count()}`); });
      await step(p, `${T}-providers-lower`, async () => { await row("Qwen Cloud Token Plan").scrollIntoViewIfNeeded(); });
      await step(p, `${T}-api`, async () => { await shot("SubRouter", "row-api"); ok((await said("SubRouter")).includes("read automatically"), await said("SubRouter")); });
      await step(p, `${T}-api-scope`, async () => { await shot("Cheaper Inference", "row-api-scope");
        ok(await row("Cheaper Inference").getByRole("button", { name: "Turn on account:read for this key" }).count() === 1 && (await said("Cheaper Inference")).includes("needs account:read"), await said("Cheaper Inference")); });
      await step(p, `${T}-console`, async () => { await shot("Qwen Cloud Token Plan", "row-console");
        ok((await said("Qwen Cloud Token Plan")).includes("found usage data on /usage (3 fields)") && (await said("Qwen Cloud Token Plan")).includes("Automated reading may break its terms"), await said("Qwen Cloud Token Plan")); });
      await step(p, `${T}-cli`, async () => { await row("Qwen Cloud Token Plan").getByRole("radio", { name: "CLI" }).click(); await shot("Qwen Cloud Token Plan", "row-cli");
        ok(await row("Qwen Cloud Token Plan").locator("code").count() === 2 && await row("Qwen Cloud Token Plan").getByRole("button", { name: "Check again" }).count() === 1, await said("Qwen Cloud Token Plan")); });
      await step(p, `${T}-local-form`, async () => { const r = row("Qwen Cloud Token Plan"); await r.getByRole("radio", { name: "Count here" }).click();
        await r.getByLabel("Qwen Cloud Token Plan's allowance").fill("2000000"); await r.getByLabel("Period").selectOption("week"); await r.getByLabel("Resets on").selectOption("3");
        await shot("Qwen Cloud Token Plan", "row-local-form"); });
      await step(p, `${T}-local-saved`, async () => { await row("Qwen Cloud Token Plan").getByRole("button", { name: "Save" }).click(); await p.waitForTimeout(300); await shot("Qwen Cloud Token Plan", "row-local-saved");
        ok((await said("Qwen Cloud Token Plan")).includes("counting tazuna's own use, 2,000,000 tokens a week"), await said("Qwen Cloud Token Plan"));
        const sent = await p.evaluate(() => window.__SENT__.filter(([c, a]) => c === "op" && a.msg.op === "usage_allowance").map(([, a]) => a.msg).pop());
        ok(JSON.stringify(sent) === JSON.stringify({ op: "usage_allowance", provider: "Qwen Cloud Token Plan", allowance: { amount: 2000000, unit: "tokens", period: "week", reset: 3 } }), JSON.stringify(sent)); });
      await step(p, `${T}-vibe-local`, async () => { await shot("Mistral Vibe", "row-local-vibe"); ok((await said("Mistral Vibe")).includes("$150 a month"), await said("Mistral Vibe")); });
      // an admin key: information beside the ways, never one of them
      await step(p, `${T}-admin-key`, async () => { await shot("Mistral Vibe", "row-admin-key");
        ok(await row("Mistral Vibe").getByRole("radio", { name: "Admin key" }).count() === 0 && (await said("Mistral Vibe")).includes("Admin key: an Admin key tells this month's spend only"), await said("Mistral Vibe")); });
      // a console without a key (More providers): its switch all the same
      await step(p, `${T}-console-no-key`, async () => { await p.getByRole("button", { name: /^More providers/ }).click(); await p.getByLabel("Search providers").fill("caffeine"); await p.waitForTimeout(300);
        await shot("Caffeine", "row-console-no-key"); ok(await row("Caffeine").getByRole("switch").getAttribute("aria-checked") === "false", await said("Caffeine")); });
      await step(p, `${T}-console-new`, async () => { await row("Caffeine").getByRole("switch").click(); await p.waitForTimeout(300); await shot("Caffeine", "row-console-new");
        ok((await said("Caffeine")).includes("sign in once to find its usage page") && await row("Caffeine").getByRole("button", { name: "Forget sign-in" }).count() === 0, await said("Caffeine")); });
      await step(p, `${T}-console-done`, async () => { await row("Caffeine").getByRole("button", { name: "Sign in…" }).click(); await p.waitForTimeout(600); await shot("Caffeine", "row-console-done");
        ok((await said("Caffeine")).includes("nothing usage-like yet"), await said("Caffeine"));
        ok(await p.evaluate(() => JSON.stringify(window.__SENT__.filter(([c]) => c === "console_open").pop()[1])) === JSON.stringify({ provider: "Caffeine", learn: true }), "console_open"); });
      await step(p, `${T}-refused`, async () => { await row("Caffeine").getByRole("radio", { name: "Count here" }).click(); await p.waitForTimeout(400); await shot("Caffeine", "row-refused");
        ok((await said("Caffeine")).includes("Not saved: ~/.config/tazuna/usage.json is read-only") && await row("Caffeine").getByRole("radio", { name: "Console" }).getAttribute("aria-checked") === "true", await said("Caffeine")); });
      await step(p, `${T}-none`, async () => { await row("Groq").getByRole("switch").click(); await p.waitForTimeout(300); await row("Groq").getByRole("radio", { name: "None" }).click(); await p.waitForTimeout(300);
        await shot("Groq", "row-none"); ok((await said("Groq")).includes("Groq's API tells no usage"), await said("Groq")); });
      await step(p, `${T}-track-off`, async () => { await row("SubRouter").getByRole("switch").click(); await p.waitForTimeout(300); await shot("SubRouter", "row-off");
        ok(!(await said("SubRouter")).includes("read automatically") && await row("SubRouter").getByRole("radio").count() === 0, await said("SubRouter")); });
      await esc(p);
      await step(p, `${T}-usage-untracked`, async () => { await p.keyboard.press("Control+Shift+U"); await p.waitForTimeout(900);
        const after = await p.getByText("$12.40", { exact: false }).count();
        log.push(`${theme} SubRouter's "$12.40" on screen: ${before} tracked, ${after} untracked`);
        ok(after === 0, `an untracked provider still shows: ${after}`); });
      if (p.errs.length) log.push(`FAIL ${T} page errors: ${p.errs.join(" | ")}`);
      await ctx.close();
    }
    if (process.env.ONLY === "sources") { fs.writeFileSync(`${OUT}/log.txt`, log.join("\n")); console.log(log.filter((l) => !l.startsWith("ok")).join("\n") || "every step ok"); await browser.close(); if (log.some((l) => l.startsWith("FAIL"))) process.exitCode = 1; return; }
  }

  // design review 14 (ONLY=r14: these alone): a row's menu (PopMenu) by pointer and keyboard, focus back after Esc; the
  // header's "Open in" (its editors under a rule); best-of's check rows by Space; a retry's countdown with Stop and Change
  // model, the live status above a message the core holds (`queued`) and kept in place when one comes; the picker's
  // Current row; an undone turn's card
  if (!process.env.ONLY || process.env.ONLY === "r14") {
    const ok = (c, what) => { if (!c) throw new Error(what); };
    const L14 = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T), inv = T.invoke.bind(T);
      T.emit = (e, p) => { if (p?.msg?.type === "view") window.__lastView = p; return emit(e, p); };
      window.__again = (f) => { const { tid, msg } = window.__lastView; T.view(tid, f({ ...msg, from: 0 })); };
      T.invoke = async (c, a) => c === "open_in" ? (a?.app ? (window.__opened = [a.path, a.app], []) : ["files", "terminal", "vscode", "zed"]) : inv(c, a); })();`;
    for (const theme of ["dark", "light"]) {
      const T = `r14-${theme[0]}`, [ctx, p] = await ctxOf({ theme, extra: L14 });
      await step(p, `${T}-row-menu`, async () => {
        // (by its place in the list: the tooltip takes a hovered row's title away)
        const row = side(p).locator("a[data-flip]").first();
        await row.click({ button: "right" }); await p.waitForTimeout(200);
        const menu = p.getByRole("menu");
        ok((await menu.innerText()).includes("Rename") && (await menu.innerText()).includes("Remove from the list"), `the row's menu: ${await menu.innerText()}`);
        ok(await p.evaluate(() => document.activeElement?.getAttribute("role")) === "menuitem", "focus is not on the menu's first item");
        await p.screenshot({ path: `${OUT}/${T}-row-menu-open.png` });
        await p.keyboard.press("Escape"); await p.waitForTimeout(150); ok(!(await menu.count()), "Esc left the menu open");
        // by the keyboard: Shift+F10 on the focused row, the arrows, Esc, and the focus back on the row
        await row.focus(); await p.keyboard.press("Shift+F10"); await p.waitForTimeout(200); ok(await menu.count(), "Shift+F10 opened no menu");
        await p.keyboard.press("ArrowDown"); ok(/Remove/.test(await p.evaluate(() => document.activeElement?.textContent ?? "")), "ArrowDown did not move");
        await p.keyboard.press("Escape"); await p.waitForTimeout(150);
        ok(await row.evaluate((r) => document.activeElement === r), "the focus did not come back to the row");
      });
      await step(p, `${T}-project-menu`, async () => {
        await side(p).locator("button[title^='/']").first().click({ button: "right" }); await p.waitForTimeout(200);
        const t = await p.getByRole("menu").innerText();
        ok(t.includes("New chat here") && t.includes("Copy path"), `the project's menu: ${t}`);
      });
      await esc(p);
      await step(p, `${T}-open-in`, async () => {
        await open(p, "Fix the flaky retry test");
        await p.locator("main > header button").filter({ hasText: "Open in" }).click(); await p.waitForTimeout(200);
        const items = await p.getByRole("menu").locator("[role=menuitem], [role=separator]").evaluateAll((es) => es.map((e) => e.getAttribute("role") === "separator" ? "|" : e.textContent));
        ok(items.join(",") === "File manager,Terminal,|,VS Code,Zed", `Open in: ${items}`);
        await p.screenshot({ path: `${OUT}/${T}-open-in-menu.png` });
        await p.getByRole("menuitem", { name: "Zed" }).click(); await p.waitForTimeout(100);
        ok(JSON.stringify(await p.evaluate(() => window.__opened)) === '["/Users/x/api-gateway","zed"]', `opened ${JSON.stringify(await p.evaluate(() => window.__opened))}`);
      });
      await step(p, `${T}-undone`, async () => {
        await p.evaluate(() => window.__again((v) => ({ ...v, changes: (v.changes ?? []).map((c) => ({ ...c, undone: true, can_undo: false })) })));
        await p.waitForTimeout(200);
        const card = p.locator("[data-msg] .rounded-lg.border.bg-card").filter({ hasText: "changed in this turn" }).first();
        await card.scrollIntoViewIfNeeded();
        ok((await card.innerText()).includes("Undone") && (await card.getByRole("button", { name: "Diff" }).evaluateAll((bs) => bs.every((b) => b.disabled))), `the undone card: ${await card.innerText()}`);
        await card.screenshot({ path: `${OUT}/${T}-undone-card.png` });
      });
      await step(p, `${T}-best-of-checks`, async () => {
        await p.keyboard.press("Control+k"); await p.keyboard.type("several"); await p.keyboard.press("Enter"); await p.waitForTimeout(500);
        const rows = p.getByRole("dialog", { name: "Try on several models" }).getByRole("checkbox");
        ok(await rows.first().getAttribute("tabindex") === "0", "a check row is no Tab stop");
        await rows.first().focus(); await p.keyboard.press("Space"); await rows.nth(1).focus(); await p.keyboard.press("Space");
        const on = await rows.evaluateAll((r) => r.map((x) => x.getAttribute("aria-checked")));
        ok(on[0] === "true" && on[1] === "true" && on.slice(2).every((x) => x === "false"), `the checks: ${on}`);
      });
      await esc(p);
      await step(p, `${T}-retry-countdown`, async () => {
        await open(p, "Names for a grey cat");
        await p.evaluate(() => window.__again((v) => ({ ...v, activity: "Waiting", blocks: [...v.blocks, { kind: "prompt", text: "fail please" },
          { kind: "notice", text: "the provider is rate limiting this model: 2 tries failed, trying again (HTTP 429: busy)", error: false, retry_at: Date.now() + 12_000, tries: 2 }] })));
        await p.waitForTimeout(300);
        const st = await p.getByRole("status").allInnerTexts(), all = st.join(" | ").replace(/\s+/g, " ");
        ok(/The provider is rate limiting this model.*2 tries failed.*next try in 1\d s.*Stop.*Change model/.test(all) && /waiting to retry.*next try in 1\d s/.test(all), `the retry: ${all}`);
        await p.screenshot({ path: `${OUT}/${T}-retry-countdown.png` });
      });
      await step(p, `${T}-held-below-status`, async () => {
        await p.getByRole("status").last().evaluate((e) => { e.dataset.seen = "1"; });
        await p.evaluate(() => window.__again((v) => ({ ...v, blocks: [...v.blocks, { kind: "prompt", text: "and then this", queued: true }] })));
        await p.waitForTimeout(300);
        const live = p.locator("[role=status][data-seen]"), held = p.locator("[data-msg]").filter({ hasText: "and then this" });
        ok(await live.count() === 1, "the live status was drawn again when a message was held (its mark started over)");
        ok((await held.innerText()).includes("Queued"), "the held message is not shown queued");
        ok((await live.boundingBox()).y < (await held.boundingBox()).y, "the live status sits under the held message");
        await p.screenshot({ path: `${OUT}/${T}-held-below-status.png` });
      });
      await ctx.close();
      const [c2, q] = await ctxOf({ theme });
      await step(q, `${T}-picker-current`, async () => {
        await q.locator("button[aria-haspopup=dialog]").filter({ hasText: "Opus" }).first().click(); await q.waitForTimeout(600);
        const heads = await q.locator("input[aria-label='Search models and providers']").locator("xpath=ancestor::div[contains(@class,'tz-pop')]").locator("div.font-medium").allInnerTexts();
        ok(/^Current/.test(heads[0] ?? ""), `the picker's first group: ${heads.slice(0, 3)}`);
      });
      await c2.close();
    }
    if (process.env.ONLY === "r14") { fs.writeFileSync(`${OUT}/log.txt`, log.join("\n")); console.log(log.filter((l) => !l.startsWith("ok")).join("\n") || "every step ok"); await browser.close(); if (log.some((l) => l.startsWith("FAIL"))) process.exitCode = 1; return; }
  }
  // review 14, a step's insides (ONLY=steps: these alone; review/mock.js "Order the summary lines"): every kind of step
  // unfolded, its code drawn as code: a read's numbers in a gutter (none in a copy) and its long lines hanging under their
  // indent in the code's column, a printed file (`sed -n 1,40p x.ts` after a cd into a folder of the project, said "in …")
  // and a numbered search coloured by the file's language, an edit and a multi-edit as diffs (rows tinted, changed words
  // marked, numbered from the turn's diff as the review pane numbers the same lines, the "updated successfully" hidden), a
  // new file with its numbers, a Python and a YAML read, a cd into the project
  // left out; in both themes, under every accent, in the approval card (edit, multi-edit, write), and the review pane on
  // the same rows (a click on a line still comments)
  if (!process.env.ONLY || process.env.ONLY === "steps") {
    const ok = (c, what) => { if (!c) throw new Error(what); };
    // a check whose picture is its own element's (no page shot beside it)
    const check = async (name, fn) => { try { await fn(); log.push(`ok ${name}`); } catch (e) { log.push(`FAIL ${name}: ${String(e).split("\n")[0]}`); } };
    const shot = async (p, el, name) => { await el.evaluate((e) => e.scrollIntoView({ block: "center" })); await p.mouse.move(2, 2); await p.waitForTimeout(150); await el.screenshot({ path: `${OUT}/${name}.png` }); };
    // what a copy of a box takes: its text as selected, the gutters' numbers and signs drawn by CSS left out
    const copied = (el) => el.evaluate((e) => { const s = getSelection(); s.selectAllChildren(e); const t = s.toString(); s.removeAllRanges(); return t; });
    const guts = (el) => el.locator(".tz-gut").evaluateAll((g) => g.map((e) => e.dataset.n).filter(Boolean));
    // each code row's numbers ("old/new"), its sign and its text: a step's and the review pane's alike (CodeRow)
    const rowNums = (el) => el.evaluate((e) => [...e.querySelectorAll("div:has(> .tz-gut)")].map((r) => { const g = [...r.querySelectorAll(":scope > .tz-gut")].map((x) => x.dataset.n ?? "");
      return [g.slice(0, -1).join("/"), g.at(-1), r.querySelector(":scope > span.flex-1")?.textContent ?? ""]; }));
    // the contrast of every coloured text in a diff's tinted rows (their tokens, signs, marked words) against what is under it:
    // the card, the row's tint, a mark's over it, composited in a canvas as the screen does; the lowest, and where
    const lowest = (box) => box.evaluate((root) => {
      const c = document.createElement("canvas").getContext("2d", { willReadFrequently: true }); c.canvas.width = c.canvas.height = 1;
      const px = (...fills) => { c.clearRect(0, 0, 1, 1); for (const f of fills) { c.fillStyle = "#000"; c.fillStyle = f; c.fillRect(0, 0, 1, 1); } return [...c.getImageData(0, 0, 1, 1).data].slice(0, 3); };
      const L = (p) => { const [r, g, b] = p.map((v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
      const ratio = (a, b) => { const [h, l] = [L(a), L(b)].sort((x, y) => y - x); return (h + 0.05) / (l + 0.05); };
      const card = getComputedStyle(root).backgroundColor;
      let low = { ratio: 99, what: "" };
      for (const row of root.querySelectorAll(".bg-diff-add, .bg-diff-del")) for (const el of [...row.querySelectorAll("span[class*=tok-], mark, .tz-gut")]) {
        if (el.classList.contains("tz-gut") && !el.dataset.n) continue;
        const under = [card, getComputedStyle(row).backgroundColor, ...(el.closest("mark") ? [getComputedStyle(el.closest("mark")).backgroundColor] : [])];
        const r = ratio(px(getComputedStyle(el).color), px(...under));
        if (r < low.ratio) low = { ratio: Math.round(r * 100) / 100, what: `${el.tagName.toLowerCase()}.${el.className} "${el.textContent || el.dataset.n}"` };
      }
      return { ...low, inMarks: root.querySelectorAll("mark span").length };
    });
    const KINDS = ["read-ts", "sed-ts", "grep-ts", "edit", "multi-edit", "write", "read-py", "read-yaml", "cd-project", "deep-path"];
    for (const theme of ["dark", "light"]) {
      const T = `steps-${theme[0]}`, [ctx, p] = await ctxOf({ theme, w: 1280, h: 900 });
      const rows = p.locator("[data-msg] button:has(svg.lucide-chevron-right)"), at = (k) => rows.nth(KINDS.indexOf(k) + 1).locator("xpath=../..");
      const stepRows = []; // the edit's and the multi-edit's numbered rows, for the review pane's
      await step(p, `${T}-unfolded`, async () => {
        await open(p, "Order the summary lines");
        await p.getByRole("button", { name: /^Worked/ }).first().click(); await p.waitForTimeout(300);
        for (let i = 1; i <= KINDS.length; i++) { await rows.nth(i).click(); await p.waitForTimeout(60); }
        await p.waitForTimeout(300);
      });
      for (const k of KINDS) await check(`${T}-${k}`, async () => {
        const el = at(k), text = await el.innerText();
        if (k === "read-ts") {
          const code = el.locator(".border-line.bg-card");
          ok(JSON.stringify(await guts(code)) === JSON.stringify(["88", "89", "90", "91", "92", "93", "94", "95"]), `the read's gutter: ${await guts(code)}`);
          const c = await copied(code);
          ok(c.startsWith("  // A hold") && !/\b88\b/.test(c), `a copy of the read takes its numbers: ${c.slice(0, 60)}`);
          // the long comment wraps under its indent, in the code's column: its second line starts right of its first and of the gutter
          const wrap = await code.locator(".tok-comment").first().evaluate((e) => { const r = document.createRange(); r.selectNodeContents(e); const rs = [...r.getClientRects()];
            const gut = e.closest(".flex").querySelector(".tz-gut").getBoundingClientRect(); return { lines: rs.length, first: rs[0].left, second: rs[1]?.left, gut: gut.right }; });
          ok(wrap.lines >= 2 && wrap.second > wrap.first && wrap.second > wrap.gut + 20, `the read's long line: ${JSON.stringify(wrap)}`);
        }
        if (k === "sed-ts") {
          const given = el.locator("[title]").filter({ hasText: "$ sed" }).first();
          ok((await given.innerText()).startsWith("in src/routes/orders $ sed -n 1,40p summary.unit.ts") && (await given.getAttribute("title")).startsWith('cd "/Users/x/api-gateway/src/routes/orders" && sed'), `the command: ${await given.innerText()}`);
          ok(await el.locator(".bg-card .tok-keyword").count() > 3 && await el.locator(".bg-card .tok-string").count() > 3 && !(await guts(el)).length, "the printed file is not coloured as TypeScript, or has a gutter");
        }
        if (k === "grep-ts") ok(JSON.stringify(await guts(el.locator(".bg-card"))) === '["141","156"]' && await el.locator(".bg-card .tok-keyword").count() > 2, `the search's gutter: ${await guts(el)}`);
        if (k === "edit" || k === "multi-edit") {
          ok(!/updated successfully|Applied 2 edits/.test(text), `${k}: its result that only says it worked shows`);
          const box = el.locator(".bg-card").first(), add = box.locator(".bg-diff-add").first();
          ok(await box.locator(".bg-diff-del").count() >= 1 && await add.count(), `${k}: no tinted rows`);
          const tint = await add.evaluate((e) => [getComputedStyle(e).backgroundColor, getComputedStyle(e.closest(".bg-card")).backgroundColor]);
          ok(tint[0] !== tint[1] && !/rgba\(0, 0, 0, 0\)|transparent/.test(tint[0]), `${k}: the row's tint ${tint}`);
          ok(!/^[+−-]/m.test(await copied(box)), `${k}: a copy takes the signs`);
          // every coloured text on a tinted row reads 4.5:1, a marked word in ink 7:1 (review of review 14: tokens in a mark fell to 3.0)
          const low = await lowest(box);
          ok(low.ratio >= 4.5 && !low.inMarks, `${k}: ${JSON.stringify(low)}`);
          const marked = await box.locator("mark").first().evaluate((m) => getComputedStyle(m).color === getComputedStyle(document.body).color);
          ok(marked, `${k}: a marked word is not in ink`);
          log.push(`${theme} ${k} lowest contrast on a tinted row ${low.ratio} (${low.what})`);
          if (k === "edit") ok((await box.locator("mark.bg-diff-add-word").innerText()).includes("sortSummaryLines"), "the edit's changed words are not marked");
          // numbered from the turn's diff (review/mock.js answers the review's op): old on removed and unchanged lines, new on added and unchanged ones
          const nums = await rowNums(box);
          const wantNums = k === "edit" ? ["2/", "/2"] : ["174/178", "175/", "/179", "176/180", "2/", "/2", "/3", "/4", "/5", "/6"];
          ok(JSON.stringify(nums.map((r) => r[0])) === JSON.stringify(wantNums), `${k}: its numbers ${nums.map((r) => r[0])}`);
          stepRows.push(...nums);
          if (k === "multi-edit") ok(await box.locator(":scope > div").count() === 2, "the multi-edit is not two pieces");
        }
        if (k === "write") {
          ok(text.includes("new file · 10 lines") && (await guts(el)).join(",") === "1,2,3,4,5,6,7,8,9,10" && !text.includes("created successfully"), `the write: ${text.slice(0, 120)}`);
          // its blank lines copy as nothing, not a space
          const c = await copied(el.locator(".bg-card"));
          ok(c.includes("as const;\n\nexport type") && !/\n[ \u200b]+\n/.test(c), `a copy of the write: ${JSON.stringify(c.slice(0, 160))}`);
        }
        if (k === "read-py") ok((await el.locator(".tok-comment").allInnerTexts()).some((t) => t.startsWith("# next to this script")), "the Python comment is not a comment");
        if (k === "read-yaml") ok((await el.locator(".tok-comment").allInnerTexts()).some((t) => t.startsWith("# runs on every push")), "the YAML comment is not a comment");
        if (k === "cd-project") ok(/^\$ npm test -- summary/.test(await el.locator("[title]").filter({ hasText: "$ npm" }).first().innerText()), `the cd into the project shows: ${text}`);
        if (k === "deep-path") {
          // the folder cut from its left, the file name whole, the whole path on hover
          const cut = await rows.nth(KINDS.indexOf(k) + 1).locator("[title$='SummaryLineDetailBadge.tsx']").evaluate((e) => {
            const [dir, name] = e.children; return { dir: dir.scrollWidth > dir.clientWidth + 1, name: name.scrollWidth <= name.clientWidth + 1, title: e.title, left: getComputedStyle(dir).direction }; });
          ok(cut.dir && cut.name && cut.left === "rtl" && cut.title.startsWith("/Users/x/api-gateway/packages/"), `the deep path: ${JSON.stringify(cut)}`);
        }
        await shot(p, el, `${T}-${k}-step`);
      });
      // the tints are the result colours', whatever the accent: an edit and a multi-edit under each
      await check(`${T}-accents`, async () => {
        for (const accent of ["ai", "indigo", "jade", "amber", "rose", "graphite"]) {
          await p.evaluate((a) => { document.documentElement.dataset.accent = a; }, accent);
          const a = await at("edit").boundingBox(), b = (await at("multi-edit").evaluate((e) => (e.scrollIntoView({ block: "center" }), 0)), await at("multi-edit").boundingBox());
          const top = (await at("edit").boundingBox()).y;
          await p.mouse.move(2, 2); await p.waitForTimeout(100);
          await p.screenshot({ path: `${OUT}/${T}-accent-${accent}.png`, clip: { x: b.x - 4, y: Math.max(0, top - 4), width: b.width + 8, height: b.y + b.height - top + 8 } });
          ok(a && b, "no edit steps");
        }
        await p.evaluate(() => { delete document.documentElement.dataset.accent; });
      });
      // the approval card: what would change, as the step draws it, under the file's path from the project
      for (const kind of ["edit", "multi", "write"]) await step(p, `${T}-approval-${kind}`, async () => {
        await p.evaluate((k) => window.__STEPS__.ask(k), kind); await p.waitForTimeout(400);
        const card = p.locator("[data-asking]");
        await card.getByText("Allow this edit?").waitFor({ timeout: 4000 });
        const t = await card.innerText();
        ok(t.includes("src/routes/orders/") && !t.includes("/Users/x/api-gateway"), `the card's path: ${t.slice(0, 120)}`);
        // before its result a write may be over a file: "whole file", never "new file" (review of review 14)
        ok(kind === "write" ? t.includes("whole file · 10 lines") && !t.includes("new file") : await card.locator(".bg-diff-add").count() > 0, `the card's change: ${t.slice(0, 200)}`);
        await card.screenshot({ path: `${OUT}/${T}-approval-${kind}-card.png` });
      });
      // a path outside the project, long: wrapped in the card, its "~/" kept (it was cut from the left and read as inside)
      await step(p, `${T}-approval-far`, async () => {
        await p.evaluate(() => window.__STEPS__.ask("far")); await p.waitForTimeout(400);
        const card = p.locator("[data-asking]"), path = card.locator("[title$='config.ts']").first();
        await card.getByText("Allow this edit?").waitFor({ timeout: 4000 });
        const t = await path.innerText(), cut = await path.evaluate((e) => e.scrollWidth > e.clientWidth + 1);
        ok(t.replace(/\s/g, "") === "~/other-app/src/features/settings/components/forms/advanced/sections/billing/config.ts" && !cut, `the far path: ${t} (cut ${cut})`);
        await card.screenshot({ path: `${OUT}/${T}-approval-far-card.png` });
      });
      // one minified line of 476 KB in the card: drawn plain, at once (it took 32 s)
      await step(p, `${T}-approval-huge`, async () => {
        const ms = await p.evaluate(async () => { const t0 = performance.now(); window.__STEPS__.ask("huge");
          for (let i = 0; i < 400 && !document.querySelector("[data-asking] [title$='vendor.min.js']"); i++) await new Promise((r) => setTimeout(r, 5));
          await new Promise((r) => requestAnimationFrame(() => setTimeout(r, 0))); return performance.now() - t0; });
        const card = p.locator("[data-asking]");
        ok(ms < 1500 && !(await card.locator("span[class*=tok-]").count()) && (await card.innerText()).includes("whole file · 1 line"), `the huge write took ${Math.round(ms)} ms`);
        log.push(`${theme} a 476 KB one-line write in the card: ${Math.round(ms)} ms`);
      });
      // the review pane on the shared rows: its numbers, tints and marks as before, and a click on a line still comments
      await step(p, `${T}-review-pane`, async () => {
        await p.evaluate(() => window.__STEPS__.done()); await p.waitForTimeout(400);
        await p.getByTitle(/Go through this turn/).first().click(); await p.waitForTimeout(600);
        const pane = p.locator("section[aria-label='Review changes']");
        ok(await pane.locator(".bg-diff-add").count() > 3 && await pane.locator("mark.bg-diff-add-word").count() > 0, "the review's rows lost their tints or marks");
        const first = pane.locator("[role=button][aria-label^='Comment on line']").first();
        ok(!/^\s*\d/.test(await copied(first)), "a copy of the review's line takes its number");
        // the steps' numbers are the pane's for the same lines
        const paneRows = await rowNums(pane);
        for (const [n, sign, text] of stepRows) ok(paneRows.some((r) => r[0] === n && r[1] === sign && r[2] === text), `the step's ${n} ${sign} ${text.slice(0, 40)} is not the review's`);
        ok(stepRows.length === 12, `the steps' numbered rows: ${stepRows.length}`);
        await first.click(); await p.waitForTimeout(200);
        ok(await pane.locator("textarea").count() === 1, "a click on the review's line wrote no comment");
        await p.screenshot({ path: `${OUT}/${T}-review-pane-comment.png` });
        await pane.locator("textarea").press("Escape"); await p.waitForTimeout(150);
      });
      await ctx.close();
    }
    if (process.env.ONLY === "steps") { fs.writeFileSync(`${OUT}/log.txt`, log.join("\n")); console.log(log.filter((l) => !l.startsWith("ok")).join("\n") || "every step ok"); await browser.close(); if (log.some((l) => l.startsWith("FAIL"))) process.exitCode = 1; return; }
  }
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
  if (process.env.ONLY === "rows") { fs.writeFileSync(`${OUT}/log.txt`, log.join("\n")); console.log(log.filter((l) => !l.startsWith("ok")).join("\n")); await browser.close(); if (log.some((l) => l.startsWith("FAIL"))) process.exitCode = 1; return; }
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
    await step(p, `${T}-receipt-chat`, async () => { await toBar(p); await p.getByTitle(/^This conversation's receipt/).click(); }); await esc(p);
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
    await step(p, `${T}-receipt-chat-running`, async () => { await toBar(p); await p.getByTitle(/^This conversation's receipt/).click(); }); await esc(p);
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
    await step(p, "w760-receipt-chat", async () => { await toBar(p); await p.getByTitle(/^This conversation's receipt/).click(); });
    await ctx.close(); }
  for (const [w, h] of [[873, 545], [768, 480]]) {
    const [ctx, p] = await ctxOf({ w, h });
    await step(p, `z${w}-start`, async () => {});
    await step(p, `z${w}-receipt-open`, async () => { await p.getByRole("button", { name: "Conversations", exact: true }).click(); await open(p, "Why is the CI build"); await p.getByTitle(/^The receipt/).first().click(); await p.waitForTimeout(500); });
    await step(p, `z${w}-receipt-chat`, async () => { await toBar(p); await p.getByTitle(/^This conversation's receipt/).click(); });
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
      // design review 14: the refusal in words, not the core's raw text
      await p.getByRole("alert").filter({ hasText: "Claude Code isn't installed." }).waitFor({ state: "visible", timeout: 4000 });
      if (await p.getByRole("alert").getByText(/starting claude/).count()) throw new Error("the refusal's raw text is on screen");
      await p.waitForFunction(() => document.querySelector('textarea[aria-label="Message"]')?.value === "  Hello\nWorld  ", null, { timeout: 4000 });
    });
    await step(p, "d-send-rejected-attachments", async () => {
      const box = p.getByRole("textbox", { name: "Message" });
      const remove = p.getByRole("button", { name: /Remove Screenshot/ });
      await box.fill("  Look at this\nplease  ");
      await p.getByTitle("Attach files or images (or drop them on the window)").click();
      await remove.waitFor({ state: "visible" });
      await p.getByTitle("Send (Enter)").click();
      await p.waitForFunction(() => document.querySelector('textarea[aria-label="Message"]')?.value === "  Look at this\nplease  ", null, { timeout: 4000 });
      await remove.waitFor({ state: "visible", timeout: 4000 });
      await p.getByRole("button", { name: "Remove retry-policy.md" }).waitFor({ state: "visible", timeout: 4000 });
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
      // its title names ⌘↵ too since desktop 0.1.115 ("Send after this reply (Enter; Ctrl+Enter sends now)")
      await p.getByTitle(/^Send after this reply \(Enter/).click();
      await p.getByText("Second message", { exact: true }).waitFor({ state: "visible", timeout: 4000 });
      if (await box.inputValue()) throw new Error("a queued send left its draft in the box");
    });
    // Send now on a queued message: the reply stops and that message goes at once, no longer queued. A held message is a
    // row of the transcript while it waits (desktop 0.1.12x): its words show at once, so what is waited for is the queued
    // line going (read at once, it was still there for the few ms the stop takes)
    const sentNow = (why) => p.getByText("Queued: goes after this reply").waitFor({ state: "detached", timeout: 2000 }).catch(() => { throw new Error(why); });
    await step(p, "d-send-now", async () => {
      await p.getByRole("button", { name: "Send now" }).first().click();
      await sentNow("the message sent now still reads as queued");
      await p.locator("[data-msg]").getByText("Second message", { exact: true }).waitFor({ state: "visible", timeout: 4000 });
    });
    // ⌘↩ (Ctrl+Enter here) during a reply: the same, from the box
    await step(p, "d-send-now-key", async () => {
      await box.fill("Third message");
      await box.press("Control+Enter");
      await sentNow("Ctrl+Enter left the message queued");
      await p.locator("[data-msg]").getByText("Third message", { exact: true }).waitFor({ state: "visible", timeout: 4000 });
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
  // the saved conversation clicked is its row from the start (no "New conversation" of its own while its messages come), and
  // removing the one on screen closes it (it stayed, and a message sent there brought it back)
  { const slow = "window.addEventListener('DOMContentLoaded', () => { const T = window.__TAURI_INTERNALS__; const v = T.view.bind(T); T.view = (tid, x) => setTimeout(() => v(tid, x), x && x.session_id ? 1500 : 0); });";
    const [ctx, p] = await ctxOf({ extra: slow });
    const side = p.locator('aside[aria-label="Conversations"]');
    await step(p, "d-open-saved-no-new-row", async () => {
      await side.locator("a").filter({ hasText: "Fix the flaky retry test" }).first().click();
      await p.waitForTimeout(2500);
      await side.locator("a").filter({ hasText: "What is a haiku?" }).first().click();
      for (let i = 0; i < 10; i++) {
        if (await side.getByText("New conversation", { exact: true }).count()) throw new Error("a \"New conversation\" row while the saved one opened");
        await p.waitForTimeout(100);
      }
    });
    await ctx.close(); }
  { const [ctx, p] = await ctxOf({});
    const side = p.locator('aside[aria-label="Conversations"]');
    await step(p, "d-remove-open-conversation", async () => {
      const row = side.locator("a").filter({ hasText: "What is a haiku?" }).first();
      await row.click();
      await p.waitForTimeout(800);
      await row.hover();
      await row.getByTitle("Remove from the list (its recording stays)").click();
      await side.getByRole("button", { name: "Remove", exact: true }).click();
      await p.getByText("What are we working on?").waitFor({ state: "visible", timeout: 4000 });
    });
    await ctx.close(); }
  // typing keeps the conversation still: the box measured itself by shrinking to one line, the conversation's scroll was
  // pulled back a line and pinned again on the next key, so the text above jumped with each key (2026-11-24)
  { const [ctx, p] = await ctxOf({});
    await step(p, "d-typing-keeps-scroll", async () => {
      await p.locator('aside[aria-label="Conversations"] a').filter({ hasText: "Fix the flaky retry test" }).first().click();
      await p.waitForTimeout(1500);
      const box = p.getByRole("textbox", { name: "Message" });
      await box.click();
      const tops = [];
      for (const ch of "so can you TLDR again what this PR does, what the issue was and what we did to solve it? maybe also help") {
        await p.keyboard.type(ch);
        tops.push(await p.evaluate(() => { const s = [...document.querySelectorAll("section")].find((e) => e.scrollHeight > e.clientHeight); return s ? Math.round(s.scrollTop) : -1; }));
      }
      for (let i = 2; i < tops.length; i++) if (tops[i] === tops[i - 2] && tops[i] !== tops[i - 1]) throw new Error(`the conversation jumped back and forth while typing: ${tops.join(",")}`);
    });
    await ctx.close(); }
  // a sent message's attachments: a thumbnail under it (not its path in the text), whole over the window on a click
  { const [ctx, p] = await ctxOf({ extra: "window.__DRAFT_SEND_SUCCESS__ = true;" });
    const box = p.getByRole("textbox", { name: "Message" });
    await step(p, "d-message-attachment", async () => {
      await box.fill("is this intended?");
      await p.evaluate(() => {
        const png = Uint8Array.from(atob("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg=="), (c) => c.charCodeAt(0));
        const dt = new DataTransfer(); dt.items.add(new File([png], "image.png", { type: "image/png" }));
        document.querySelector('textarea[aria-label="Message"]').dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true }));
      });
      await p.getByText("paste-1.png").waitFor({ state: "visible", timeout: 4000 });
      await p.getByTitle("Send (Enter)").click();
      await p.getByRole("button", { name: "paste-1.png: show it whole" }).click();
      await p.getByRole("dialog", { name: "paste-1.png" }).waitFor({ state: "visible", timeout: 4000 });
      if (await p.locator("[data-msg]").getByText("/tmp/tazuna-desktop-images/paste-1.png").count()) throw new Error("the path still shows in the message");
    });
    await step(p, "d-message-attachment-closed", async () => {
      await p.keyboard.press("Escape");
      await p.getByRole("dialog", { name: "paste-1.png" }).waitFor({ state: "detached", timeout: 4000 });
    });
    await ctx.close(); }
  // the model picker's search takes the typing as it opens (0.1.74 to 0.1.76 it did not)
  { const [ctx, p] = await ctxOf({});
    await p.getByTitle("Model and effort").click(); await p.waitForTimeout(400);
    log.push(`picker focus: ${await p.evaluate(() => document.activeElement?.getAttribute("placeholder") || document.activeElement?.tagName)}`);
    await ctx.close(); }
  { const [ctx, p] = await ctxOf({ w: 1024, h: 768 });
    await step(p, "d-project-model-picker-fits", async () => {
      await side(p).locator('button[title="/Users/x/api-gateway"]').click();
      await p.getByTitle("Model and effort").click();
      await p.getByLabel("Search models and providers").fill("opus");
      const price = p.getByText("$ per M, in / out", { exact: true });
      await price.waitFor({ state: "visible", timeout: 4000 });
      await p.waitForFunction(() => {
        const picker = document.querySelector('[aria-label="Search models and providers"]')?.closest(".tz-pop");
        if (!picker) return false;
        const frame = picker.closest("main").getBoundingClientRect(), pop = picker.getBoundingClientRect();
        return pop.left >= frame.left && pop.right <= frame.right && pop.width >= 459;
      }, null, { timeout: 4000 });
    });
    await ctx.close(); }
  // design review 13: the permission card, the work fold, the model picker's keys and a send the core refused; every check
  // a step's own failure. `must`: fail the step with what was seen
  const must = (ok, what) => { if (!ok) throw new Error(what); };
  const answersOf = (p) => p.evaluate(() => window.__SENT__.filter(([c, a]) => c === "op" && a?.msg?.op === "answer").map(([, a]) => `${a.msg.allow ? "allow" : "deny"}${a.msg.always ? "+always" : ""}`));
  // the last view emitted, and a way to emit another (a second question, a mode's notice)
  const LAST = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { if (p?.msg?.type === "view") window.__lastView = p; return emit(e, p); };
    window.__again = (f) => { const { tid, msg } = window.__lastView; T.view(tid, f({ ...msg, from: 0 })); }; })();`;
  // A force push auto asks about, its note as the core writes it (no `risky` field: the note says so): why under the title,
  // Allow as plain as Decline; no key typed in the box answers, not even a first one after a pause; Shift+Tab from the box
  // reaches the buttons; a key outside it after a pause does
  { const risky = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { const m = p?.msg; if (m?.type === "view" && m.session_id === "p2" && m.asking) { m.asking = { ...m.asking, what: "git push --force origin main", once: true, note: "once only · auto asks: git push --force: rewrites the remote's history" }; } return emit(e, p); }; })();`;
    const [ctx, p] = await ctxOf({ extra: risky + LAST });
    const box = p.getByRole("textbox", { name: "Message" });
    await step(p, "d-asks-risky", async () => {
      await open(p, "Bump the TLS library"); await p.getByText("rewrites the remote's history").waitFor({ timeout: 4000 }); await p.waitForTimeout(700);
      must(!(await p.getByRole("button", { name: /^Always/ }).count()), "a once-only question offers Always");
      must(!/bg-accent/.test(await p.getByRole("button", { name: /^Allow/ }).getAttribute("class")), "Allow is the filled accent button");
      must(!(await p.locator(".tz-mark-write, .tz-mark-pulse").count()), "the mark is still moving while the reply waits for permission");
    });
    await step(p, "d-asks-typing", async () => {
      await box.click(); await p.waitForTimeout(900); await box.pressSequentially("explain why", { delay: 60 });
      must((await box.inputValue()) === "explain why", `the box holds "${await box.inputValue()}"`);
      must(!(await answersOf(p)).length, `typing answered: ${await answersOf(p)}`);
    });
    await step(p, "d-asks-first-key", async () => {
      for (const k of ["n", "y"]) {
        await box.fill(""); await p.waitForTimeout(900); await box.press(k);
        must((await box.inputValue()) === k, `a first ${k} did not reach the box: "${await box.inputValue()}"`);
      }
      must(!(await answersOf(p)).length, `a first key in the empty box answered: ${await answersOf(p)}`);
    });
    await step(p, "d-asks-shift-tab", async () => {
      await box.fill(""); await box.focus(); await p.keyboard.press("Shift+Tab");
      const at = await p.evaluate(() => document.activeElement?.textContent?.trim());
      must(/^Decline/.test(at ?? ""), `Shift+Tab from the box went to "${at}"`);
      must(!(await p.getByText(/to the buttons/).count()), "the card's hint is back (design review 14 took it out)");
    });
    // Enter on Decline reached by Shift+Tab answers, and the focus goes back to the box, not the page
    await step(p, "d-asks-key-after-pause", async () => {
      await p.keyboard.press("Enter"); await p.waitForTimeout(200);
      must((await answersOf(p)).join() === "deny", `Enter on Decline: ${await answersOf(p)} (one deny wanted)`);
      must(await p.evaluate(() => document.activeElement?.matches("[data-composer] textarea:not([aria-hidden])")), `after the answer the focus is on ${await p.evaluate(() => document.activeElement?.tagName)}`);
    });
    // the same question once more (answered by a click, asked again): its 600 ms again, the buttons off at once, and a key
    // after a pause but within them answers nothing
    await step(p, "d-asks-again", async () => {
      await p.waitForTimeout(900); await p.getByRole("button", { name: /^Decline/ }).click();
      await p.evaluate(() => window.__again((v) => ({ ...v, asking: { ...v.asking } })));
      must(await p.getByRole("button", { name: /^Decline/ }).isDisabled(), "the buttons stay enabled after an answer");
      await p.waitForTimeout(250); await p.keyboard.press("n"); await p.waitForTimeout(600);
      must(!(await p.getByRole("button", { name: /^Decline/ }).isDisabled()), "the buttons never came back");
      must((await answersOf(p)).join() === "deny,deny", `a key in the second question's first 600 ms answered: ${await answersOf(p)} (deny,deny wanted)`);
    });
    await ctx.close(); }
  // keys something else takes: the palette (typing "auto" there), an input method composing, the window coming back
  { const [ctx, p] = await ctxOf({});
    await step(p, "d-asks-palette", async () => {
      await open(p, "Bump the TLS library"); await p.waitForTimeout(700);
      await p.locator("body").click({ position: { x: 640, y: 120 } }); await p.waitForTimeout(900);
      await p.keyboard.press("Control+k"); await p.waitForTimeout(900);
      for (const k of "auto") { await p.keyboard.press(k); await p.waitForTimeout(900); }
      must(!(await answersOf(p)).length, `typing in the palette answered: ${await answersOf(p)}`);
    });
    await esc(p); await p.locator("body").click({ position: { x: 640, y: 120 } });
    // a key on a dialog (the shortcuts) or on a menu's item (the permission mode's, a .tz-pop too) is theirs (the model
    // picker cannot open while a question waits: a reply runs)
    await step(p, "d-asks-dialog-button", async () => {
      await p.keyboard.press("Control+/"); await p.getByRole("dialog", { name: "Keyboard shortcuts" }).waitFor(); await p.waitForTimeout(900); await p.keyboard.press("n");
      await esc(p); await p.getByTitle(/^Permission mode/).click(); await p.waitForTimeout(900);
      must(await p.evaluate(() => !!document.activeElement?.closest("[role=menu]") && document.activeElement.tagName === "BUTTON"), "the menu's item has no focus");
      await p.keyboard.press("n"); await esc(p);
      must(!(await answersOf(p)).length, `a key in a dialog or a menu answered: ${await answersOf(p)}`);
    });
    await p.locator("body").click({ position: { x: 640, y: 120 } });
    await step(p, "d-asks-ime", async () => {
      await p.waitForTimeout(900);
      await p.evaluate(() => { document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "y", isComposing: true, bubbles: true, cancelable: true })); });
      await p.waitForTimeout(900);
      await p.evaluate(() => { const e = new KeyboardEvent("keydown", { key: "y", bubbles: true, cancelable: true }); Object.defineProperty(e, "keyCode", { get: () => 229 }); document.body.dispatchEvent(e); });
      must(!(await answersOf(p)).length, `a composing key answered: ${await answersOf(p)}`);
    });
    await step(p, "d-asks-focus-back", async () => {
      await p.waitForTimeout(900); await p.evaluate(() => window.dispatchEvent(new Event("focus"))); await p.keyboard.press("y");
      must(!(await answersOf(p)).length, `a key as the window came back answered: ${await answersOf(p)}`);
      await p.waitForTimeout(900); await p.keyboard.press("y"); await p.waitForTimeout(200);
      must((await answersOf(p)).join() === "allow", `y after a pause, outside the box: ${await answersOf(p)} (one allow wanted)`);
      must(await p.evaluate(() => document.activeElement?.matches("[data-composer] textarea:not([aria-hidden])")), "a key's answer left the focus on the page");
    });
    await ctx.close(); }
  // Shift+Tab onto the card, then n at once: a key on its own buttons needs no pause (it did nothing for 800 ms)
  { const [ctx, p] = await ctxOf({});
    await step(p, "d-asks-on-card", async () => {
      await open(p, "Bump the TLS library"); await p.waitForTimeout(700);
      await p.getByRole("textbox", { name: "Message" }).focus(); await p.keyboard.press("Shift+Tab"); await p.keyboard.press("n"); await p.waitForTimeout(200);
      must((await answersOf(p)).join() === "deny", `n on the card right after Shift+Tab: ${await answersOf(p)} (one deny wanted)`);
    });
    await ctx.close(); }
  // the narrow window's sidebar (a native dialog): Escape in it shuts it and answers nothing
  { const [ctx, p] = await ctxOf({ w: 900, h: 700 });
    await step(p, "d-asks-drawer", async () => {
      await open(p, "Bump the TLS library").catch(() => {});
      if (!(await p.getByText("Allow a shell command?").count())) { await p.getByRole("button", { name: "Conversations" }).first().click(); await p.locator("dialog[open]").locator('a[title^="Bump the TLS"]').first().click(); await p.waitForTimeout(900); }
      await p.waitForTimeout(700); await p.getByRole("button", { name: "Conversations" }).first().click(); await p.locator("dialog[open]").waitFor(); await p.waitForTimeout(900);
      await p.keyboard.press("Escape"); await p.waitForTimeout(300);
      must(!(await answersOf(p)).length, `Escape in the sidebar answered: ${await answersOf(p)}`);
      must(!(await p.locator("dialog[open]").count()), "the sidebar stayed open");
    });
    await ctx.close(); }
  // what makes a question destructive: the core's `risky` (with its `risky_why`), an older core's note with "always =",
  // risky with a note that is no reason (a chat's hand-off): never Always, `a` allows nothing, the reason or else the note
  for (const [name, asking, reason] of [
    ["d-asks-risky-field", { risky: true, risky_why: "git reset --hard: drops uncommitted changes", once: true, note: "once only" }, "git reset --hard: drops uncommitted changes"],
    ["d-asks-risky-old-note", { once: false, note: "auto asks: git push --force: rewrites the remote's history · always = this command" }, "git push --force: rewrites the remote's history"],
    ["d-asks-risky-handoff", { risky: true, once: true, note: "once only: the agent is answering a chat's question" }, "once only: the agent is answering a chat's question"],
  ]) {
    const [ctx, p] = await ctxOf({ extra: `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { const m = p?.msg; if (m?.type === "view" && m.session_id === "p2" && m.asking) m.asking = { ...m.asking, what: "git push --force origin main", ...${JSON.stringify(asking)} }; return emit(e, p); }; })();` });
    await step(p, name, async () => {
      await open(p, "Bump the TLS library"); await p.getByText("Allow a shell command?").waitFor(); await p.waitForTimeout(700);
      must(!!(await p.getByText(reason, { exact: true }).count()), `"${reason}" not on the card`);
      must(!(await p.getByRole("button", { name: /^Always/ }).count()), "a destructive question offers Always");
      must(!/bg-accent/.test(await p.getByRole("button", { name: /^Allow/ }).getAttribute("class")), "Allow is the filled accent button");
      await p.locator("body").click({ position: { x: 640, y: 120 } }); await p.waitForTimeout(900); await p.keyboard.press("a"); await p.waitForTimeout(200);
      must(!(await answersOf(p)).length, `a answered a destructive question: ${await answersOf(p)}`);
    });
    await ctx.close();
  }
  // a failed step said folded too, in the result colour ("1 failed"); a stopped, declined or running one is no failure
  { const fail = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { const m = p?.msg; if (m?.type === "view" && m.session_id === "p1") m.blocks = m.blocks.map((b) => b.id === "t3" ? { ...b, error: true, result: "cat: nope.txt: No such file or directory" } : b.id === "t2" ? { ...b, error: true, stopped: true } : b.id === "t4" ? { ...b, error: true, declined: true } : b.id === "t7" ? { ...b, error: true } : b); return emit(e, p); }; })();`;
    const [ctx, p] = await ctxOf({ extra: fail });
    await step(p, "d-workfold-failed", async () => {
      await open(p, "Fix the flaky retry test"); await p.getByText(/^Worked \d+ s/).first().waitFor({ timeout: 4000 });
      const folds = await p.locator("main button[aria-expanded]").filter({ hasText: /^(Worked|Working)/ }).allInnerTexts();
      must(/\b1 failed$/.test(folds[0]?.replace(/\n/g, " ") ?? ""), `the done fold: "${folds[0]}" (… 1 failed wanted)`);
      must(!/failed/.test(folds[1] ?? ""), `the running fold counts a failure: "${folds[1]}"`);
    });
    await ctx.close(); }
  // a name the user gave stands while its reply runs: the working row and the header showed the first message instead,
  // and the name only once the reply ended (the user, 2026-11-25)
  { const named = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { const m = p?.msg; if (m?.type === "sessions") m.dirs = m.dirs.map((d) => ({ ...d, list: d.list.map((r) => r.id === "p1" ? { ...r, first: "Office lunch app", named: true } : r) })); return emit(e, p); }; })();`;
    const [ctx, p] = await ctxOf({ extra: named });
    await step(p, "d-renamed-while-working", async () => {
      await open(p, "Office lunch app"); await p.waitForTimeout(500);
      const head = await p.locator("main > header h1").innerText(), rows = await side(p).innerText();
      must(head === "Office lunch app", `the header: "${head}" (the name wanted)`);
      must(!/Fix the flaky retry test/.test(rows), "a sidebar row still shows the first message");
    });
    await ctx.close(); }
  // the model picker from the keyboard: the arrows through the rows and back up, Home and End, never a disabled row, the
  // row reached never under the sticky heads; Enter takes the one match, a "no key" row opens Settings on its key, an
  // empty search only shuts it, an input method's Enter picks nothing (the mock keeps a pick to itself: it is caught)
  { const [ctx, p] = await ctxOf({ extra: `(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T); T.invoke = async (c, a) => { if (c === "op" && a?.msg?.op === "model") window.__picked = a.msg.model; return inv(c, a); }; })();` });
    const search = p.getByLabel("Search models and providers");
    const row = () => p.evaluate(() => [...document.querySelectorAll("button[data-row]:not(:disabled)")].indexOf(document.activeElement));
    // (the pill by its role: a title is the tooltip's once the pointer has been on it)
    const pick = async () => { await p.locator("[data-composer] button[aria-haspopup=dialog]").first().click(); await search.waitFor(); };
    await step(p, "d-picker-arrows", async () => {
      await pick(); await p.keyboard.press("ArrowDown"); await p.keyboard.press("ArrowDown");
      must((await row()) === 1, `down twice: row ${await row()} (1 wanted)`);
      await p.keyboard.press("End"); const n = await p.evaluate(() => document.querySelectorAll("button[data-row]:not(:disabled)").length);
      must((await row()) === n - 1, `End: row ${await row()} of ${n}`);
      await p.keyboard.press("Home"); must((await row()) === 0, `Home: row ${await row()}`);
      await p.keyboard.press("ArrowUp");
      must((await p.evaluate(() => document.activeElement?.getAttribute("aria-label"))) === "Search models and providers", "ArrowUp from the first row left the search");
    });
    // a long list down row by row, End, then up row by row: each row reached shows whole, below the sticky heads and
    // above the bottom fade (22 px)
    await step(p, "d-picker-scroll", async () => {
      const hidden = () => p.evaluate(() => { const r = document.activeElement.getBoundingClientRect(), list = document.activeElement.closest(".tz-scrollfade").getBoundingClientRect(), head = document.querySelector(".tz-pop .sticky")?.getBoundingClientRect();
        return [head ? Math.round(head.bottom - r.top) : 0, Math.round(r.bottom - (list.bottom - 22))]; });
      await search.fill("a"); await p.waitForTimeout(200); await search.focus();
      for (const [key, n] of [["ArrowDown", 16], ["End", 1], ["ArrowUp", 16]]) for (let i = 0; i < n; i++) {
        await p.keyboard.press(key); const [top, bottom] = await hidden();
        must(top <= 0 && bottom <= 0, `after ${key}: a row ${top > 0 ? `${top} px under the sticky heads` : `${bottom} px into the bottom fade`}`);
      }
      await search.focus();
    });
    await step(p, "d-picker-disabled", async () => {
      await search.fill("glm 5.3"); await p.waitForTimeout(200);
      const [all, on] = await p.evaluate(() => [document.querySelectorAll("button[data-row]").length, document.querySelectorAll("button[data-row]:not(:disabled)").length]);
      must(all > on, `no disabled row to skip (${all} rows, ${on} enabled)`);
      const seen = [];
      for (let i = 0; i < on + 1; i++) { await p.keyboard.press("ArrowDown"); seen.push(await p.evaluate(() => document.activeElement?.disabled ? "disabled" : document.activeElement?.hasAttribute("data-row") ? "row" : "other")); }
      must(seen.every((x) => x === "row"), `the arrows reached: ${seen}`);
    });
    await step(p, "d-picker-ime", async () => {
      await search.focus(); await search.fill("mock coder");
      await p.evaluate(() => document.querySelector('[aria-label="Search models and providers"]').dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", isComposing: true, bubbles: true, cancelable: true })));
      await p.waitForTimeout(300);
      must(!!(await search.count()) && !(await p.evaluate(() => window.__picked)), "an input method's Enter picked");
    });
    await step(p, "d-picker-enter", async () => {
      await p.keyboard.press("Enter"); await p.waitForTimeout(300);
      must(!(await search.count()), "Enter left the picker open");
      must((await p.evaluate(() => window.__picked)) === "mock-coder-7b", `Enter picked ${await p.evaluate(() => window.__picked)}`);
    });
    await step(p, "d-picker-enter-empty", async () => {
      await p.evaluate(() => { window.__picked = undefined; }); await pick(); await p.keyboard.press("Enter"); await p.waitForTimeout(300);
      must(!(await search.count()) && !(await p.evaluate(() => window.__picked)), `an empty search's Enter: open ${await search.count()}, picked ${await p.evaluate(() => window.__picked)}`);
    });
    await step(p, "d-picker-enter-no-key", async () => {
      await pick(); await search.fill("cerebras"); await p.keyboard.press("Enter"); await p.waitForTimeout(700);
      const at = await p.evaluate(() => document.activeElement?.getAttribute("aria-label"));
      must(at === "Cerebras key", `Enter on a no-key row: focus on ${at} (Cerebras key wanted)`);
    });
    await ctx.close(); }
  // a message claude cannot take, in a conversation: back in the box, the error beside it, no row; a mode's notice leaves
  // it; after a message that goes, nothing of it at the top either; an error of the core's that is no refused send (a
  // command's reply) stays a row, as before
  { const [ctx, p] = await ctxOf({ extra: "window.__NO_CLAUDE__ = true;" + LAST });
    const box = p.getByRole("textbox", { name: "Message" }), alert = () => p.getByRole("alert").filter({ hasText: "Claude Code isn't installed." });
    await step(p, "d-send-refused-conversation", async () => {
      await open(p, "Names for a grey cat"); await box.fill("write me a haiku about rain"); await p.getByTitle("Send (Enter)").click();
      await alert().waitFor({ state: "visible", timeout: 4000 });
      await p.waitForFunction(() => document.querySelector('textarea[aria-label="Message"]')?.value === "write me a haiku about rain", null, { timeout: 4000 });
      must(!(await p.locator("[data-msg]").getByText(/starting claude/).count()), "the error is a row of the conversation");
      await p.evaluate(() => window.__again((v) => ({ ...v, mode: "plan", blocks: [...v.blocks, { kind: "notice", text: "mode: plan", error: false }] })));
      await p.waitForTimeout(200); must(!!(await alert().count()), "a mode's notice took the error away");
      // ...and another model does (design review 14: it stood after another model was picked)
      await p.evaluate(() => window.__again((v) => ({ ...v, model: "mock-coder-7b" })));
      await p.waitForTimeout(200); must(!(await alert().count()), "another model left the refusal standing");
    });
    await step(p, "d-send-after-refused", async () => {
      await p.evaluate(() => { window.__NO_CLAUDE__ = false; window.__DRAFT_SEND_SUCCESS__ = true; });
      await p.getByTitle("Send (Enter)").click();
      await p.locator("[data-msg]").getByText("write me a haiku about rain", { exact: true }).waitFor({ state: "visible", timeout: 4000 });
      must(!(await p.locator("[data-msg]").getByText(/starting claude/).count()) && !(await alert().count()), "the refused send's error is still on screen");
    });
    await step(p, "d-command-error-row", async () => {
      await p.evaluate(() => window.__again((v) => ({ ...v, activity: "Done", blocks: [...v.blocks, { kind: "done", ok: true, secs: 1, turns: 1, cost: 0, usage: v.usage, stopped: false }, { kind: "notice", text: "github: removed", error: true }] })));
      await p.locator("[data-msg]").getByText("github: removed").waitFor({ state: "visible", timeout: 4000 });
      must(!(await p.getByRole("alert").count()), "a command's error stood over the box");
    });
    // a refusal that brought no notice (a model being restored, /create): an error more than a second later is not its own
    await step(p, "d-refused-without-notice", async () => {
      await p.evaluate(() => { const { tid } = window.__lastView; window.__TAURI_INTERNALS__.emit("tazuna-view", { tid, msg: { type: "send_rejected", text: "kept" } }); });
      await p.waitForTimeout(1300);
      await p.evaluate(() => window.__again((v) => ({ ...v, blocks: [...v.blocks, { kind: "notice", text: "linear: added, connecting…", error: true }] })));
      await p.locator("[data-msg]").getByText("linear: added, connecting…").waitFor({ state: "visible", timeout: 4000 });
      must(!(await p.getByRole("alert").count()), "a later error was taken for the refusal");
    });
    await ctx.close(); }
  // the core's word for it (`rejected` on the notice) where the message showed first (the router's check, then the
  // refusal): message and error leave the conversation, the words are back in the box once, the error over it
  { const routed = `(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T); T.invoke = async (c, a) => { if (c === "send" && window.__lastView) { const { tid, msg } = window.__lastView;
      T.view(tid, { ...msg, from: 0, blocks: [...msg.blocks, { kind: "prompt", text: a.text }, ...(window.__modeBetween ? [{ kind: "notice", text: "mode: plan", error: false }] : [])] });
      setTimeout(() => { T.emit("tazuna-view", { tid, msg: { type: "send_rejected", ...a.draft } }); T.view(tid, { ...window.__lastView.msg, from: 0, blocks: [...window.__lastView.msg.blocks, { kind: "notice", text: "starting claude (is it installed and on PATH?): No such file or directory (os error 2)", error: true, ended: true, rejected: true }] }); }, 60); return null; } return inv(c, a); }; })();`;
    const [ctx, p] = await ctxOf({ extra: LAST + routed });
    const box = p.getByRole("textbox", { name: "Message" });
    // a new chat: the start page stays, its title and the sidebar do not take the refused words
    await step(p, "d-send-refused-fresh", async () => {
      await box.fill("write me a haiku about rain"); await p.getByTitle("Send (Enter)").click();
      await p.getByRole("alert").filter({ hasText: "Claude Code isn't installed." }).waitFor({ state: "visible", timeout: 4000 }); await p.waitForTimeout(300);
      must((await box.inputValue()) === "write me a haiku about rain", `the box holds "${await box.inputValue()}"`);
      must(!!(await p.getByText("What are we working on?").count()), "the start page went");
      must((await p.locator("main > header").innerText()).includes("New chat"), `the header reads "${await p.locator("main > header").innerText()}"`);
      must(!(await side(p).getByText(/haiku about rain/i).count()), "the sidebar took the refused words");
    });
    // with attachments: its prompt holds the paths after the text (as the box sends it), and still leaves with its refusal
    await step(p, "d-send-refused-attachments", async () => {
      await box.fill("look at this"); await p.getByTitle("Attach files or images (or drop them on the window)").click();
      await p.getByRole("button", { name: /Remove Screenshot/ }).waitFor({ state: "visible" }); await p.getByTitle("Send (Enter)").click(); await p.waitForTimeout(600);
      must(await p.getByRole("button", { name: /Remove Screenshot/ }).isVisible(), "the attachments did not come back");
      must(!(await p.locator("[data-msg]").count()), `the refused message is a row: ${JSON.stringify(await p.locator("[data-msg]").allInnerTexts())}`);
      const chip = p.getByRole("button", { name: /^Remove (Screenshot|retry-policy)/ });
      await box.fill(""); while (await chip.count()) await chip.first().click();
    });
    // a mode picked during the router's check: its notice between the message and the refusal; the message still leaves
    await step(p, "d-send-refused-mode-between", async () => {
      await p.evaluate(() => { window.__modeBetween = true; });
      await box.fill("draw a cat"); await p.getByTitle("Send (Enter)").click(); await p.waitForTimeout(600);
      must((await box.inputValue()) === "draw a cat", `the box holds "${await box.inputValue()}"`);
      must(!(await p.locator("[data-msg]").getByText("draw a cat", { exact: true }).count()), "the refused message stayed a row past the mode's notice");
      await box.fill(""); await p.evaluate(() => { window.__modeBetween = false; });
    });
    await step(p, "d-send-refused-routed", async () => {
      await open(p, "Names for a grey cat"); await box.fill("write me a haiku about rain"); await p.getByTitle("Send (Enter)").click();
      await p.getByRole("alert").filter({ hasText: "Claude Code isn't installed." }).waitFor({ state: "visible", timeout: 4000 });
      await p.waitForTimeout(300);
      must((await box.inputValue()) === "write me a haiku about rain", `the box holds "${await box.inputValue()}"`);
      must(!(await p.locator("[data-msg]").getByText("write me a haiku about rain", { exact: true }).count()), "the refused message is still a row");
      must(!(await p.locator("[data-msg]").getByText(/starting claude/).count()), "the error is a row");
    });
    await ctx.close(); }
  // design review 13, D1-D4 of mockups/v10.html
  // D1: the mark writes on the running reply's Working row and pulses in the sidebar;
  // mark at work; with reduced motion all of it still. Its tokens are the output of the calls after the last finished turn,
  // from the view alone: a reply begun out of sight counts its calls at once, a message queued during it does not start the
  // count again, and a subagent's tokens (its whole context) are not added
  { const live = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { const m = p?.msg; if (m?.type === "view" && m.session_id === "p1" && !m.calls.some((c) => c.block === 10)) m.calls.push({ block: 10, fresh: 3000, cached: 9000, out: 1234, cost: 0.01, first_ms: 800, tps: 60, total_ms: 4000 }); return emit(e, p); }; })();`;
    const task = { id: "bg1", kind: "local_agent", agent: "explorer", description: "Find the flaky tests", status: "running", running: true, doing: "", tokens: 47000, tools: 2, calls: [], secs: null };
    for (const [motion, R] of [["no-preference", ""], ["reduce", "-still"]]) for (const theme of ["dark", "light"]) {
      const [ctx, p] = await ctxOf({ theme, extra: live + LAST, scale: 2, motion }), N = `${theme[0]}-d1-working${R}`;
      const fold = p.locator("main button[aria-expanded]").filter({ hasText: /^Working/ });
      const brand = side(p).locator(".tz-mark");
      const text = () => fold.evaluate((b) => b.innerText.replace(/\s+/g, " ").trim());
      await step(p, N, async () => {
        const ready = await p.locator("main .tz-mark").evaluate((mark) => [mark.querySelectorAll("path").length, mark.getAnimations({ subtree: true }).length]);
        must(ready.join() === "4,0", `new chat's mark: ${ready} (four still strokes wanted)`);
        await open(p, "Fix the flaky retry test"); await fold.waitFor({ timeout: 4000 }); await p.mouse.move(5, 400); await p.waitForTimeout(300);
        const seen = await fold.evaluate((b) => {
          const mark = b.querySelector(".tz-mark"), guide = mark.querySelector(".tz-mark-guide"), word = b.querySelector(".tz-shimmer");
          const paths = [...mark.querySelectorAll(".tz-mark-strokes > path")];
          const frames = (ms) => paths.map((path) => {
            const [anim] = path.getAnimations();
            if (anim) { anim.pause(); anim.currentTime = ms; }
            return getComputedStyle(path).clipPath;
          });
          const start = frames(0), drawing = frames(200), complete = frames(1100), holding = frames(2500);
          const anim = paths.map((path) => getComputedStyle(path).animationName);
          for (const path of paths) for (const a of path.getAnimations()) a.play();
          return { size: mark.getBoundingClientRect().width, anim, start, drawing, complete, holding, guide: getComputedStyle(guide).display,
            shimmer: word ? getComputedStyle(word).backgroundImage.slice(0, 15) : "", text: b.innerText.replace(/\s+/g, " ").trim() };
        });
        log.push(`${N}: ${JSON.stringify(seen)}`);
        // 16 px, a CSS pixel per cell of its grid (design review 14: at 10 px a grey smudge)
        must(seen.size === 16 && seen.anim.length === 4, `no 16 px, four-stroke mark at work on the row: ${JSON.stringify(seen)}`);
        if (R) {
          must(seen.anim.every((a) => a === "none") && seen.start.every((c) => c === "none") && seen.guide === "none" && seen.shimmer === "none", "reduced motion did not show the still mark and plain text");
        } else {
          must(seen.anim.join() === "tz-mark-crown,tz-mark-first,tz-mark-second,tz-mark-hook" && seen.guide !== "none" && seen.shimmer.startsWith("linear-gradient"), "the Working row did not write the mark");
          must(seen.start[0] !== seen.drawing[0] && seen.drawing[0] !== seen.complete[0] && seen.start.slice(1).join() === seen.drawing.slice(1).join(), "the crown did not draw smoothly ahead of the other strokes");
          must(seen.start.every((c, i) => c !== seen.complete[i]) && seen.complete.join() === seen.holding.join(), "the completed hand was not held");
        }
        const pulse = await brand.evaluate((mark) => {
          const svg = mark.querySelector("svg"), [anim] = svg.getAnimations();
          if (!anim) return { opacity: +getComputedStyle(svg).opacity };
          anim.pause(); anim.currentTime = 0;
          const dim = +getComputedStyle(svg).opacity;
          anim.currentTime = 1200;
          const bright = +getComputedStyle(svg).opacity;
          anim.play();
          return { dim, bright, duration: anim.effect.getTiming().duration, strokes: mark.querySelectorAll("path").length };
        });
        must(R ? pulse.opacity === 1 : pulse.dim === .67 && pulse.bright === 1 && pulse.duration === 2400 && pulse.strokes === 4, `sidebar mark: ${JSON.stringify(pulse)}`);
        await fold.screenshot({ path: `${OUT}/${N}-row.png` });
        await brand.screenshot({ path: `${OUT}/${N}-brand.png` });
      });
      if (!R && theme === "dark") {
        // the reply was running when the window first saw it: its call is counted at once (the call before it, a finished
        // turn's, is not)
        await step(p, "d-d1-tokens-off-screen", async () => { must(/^Working \S+ s\s*↓ 1\.2k tokens/.test(await text()), `the row reads "${await text()}" (↓ 1.2k tokens wanted)`); });
        await step(p, "d-d1-tokens-queued", async () => {
          await p.evaluate(() => window.__again((m) => ({ ...m, blocks: [...m.blocks, { kind: "prompt", text: "and update the docs" }], calls: [...m.calls, { block: m.blocks.length + 1, fresh: 900, cached: 9000, out: 800, cost: 0.004, first_ms: 600, tps: 60, total_ms: 3000 }] })));
          await p.waitForTimeout(300);
          must(/↓ 2\.0k tokens/.test(await p.locator("main").innerText()), `after a queued message and a call: "${await text().catch(() => "no row")}" (↓ 2.0k tokens wanted)`);
        });
        await step(p, "d-d1-tokens-subagent", async () => {
          await p.evaluate((t) => window.__again((m) => ({ ...m, tasks: [t] })), task); await p.waitForTimeout(300);
          const all = await p.locator("main").innerText();
          must(/↓ 2\.0k tokens/.test(all) && !/↓ 4\d\.\dk/.test(all), `a subagent's 47k tokens counted as output: "${await text().catch(() => "no row")}"`);
        });
      }
      // a conversation at rest beside it: the sidebar's mark still works, for the reply running in the other
      if (!R) await step(p, `${N}-other`, async () => {
        await open(p, "Why is the CI build");
        must(await brand.locator("svg").evaluate((svg) => getComputedStyle(svg).animationName) === "tz-mark-pulse", "the sidebar's mark stopped while p1 runs");
        await p.getByTitle(/^The receipt/).last().click();
        const receipt = await p.locator("main .tz-slip .tz-mark").evaluate((mark) => [mark.querySelectorAll("path").length, mark.getAnimations({ subtree: true }).length]);
        must(receipt.join() === "4,0", `receipt's mark: ${receipt} (four still strokes wanted)`);
      });
      await ctx.close();
    } }
  // a side chat drawn again (away to another conversation and back; its own is working, so it stays open) still counts its
  // running reply's tokens
  { const sideBusy = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { const m = p?.msg; if (m?.type === "view" && m.session_id === "f1") { m.activity = "Thinking"; if (!m.blocks.some((b) => b.text === "why the fake clock?")) m.blocks.push({ kind: "prompt", text: "why the fake clock?" }); m.calls = [{ block: m.blocks.length, fresh: 900, cached: 8000, out: 1234, cost: 0.002, first_ms: 500, tps: 60, total_ms: 2000 }]; } return emit(e, p); }; })();`;
    const [ctx, p] = await ctxOf({ extra: sideBusy }), st = p.locator('aside[aria-label="Side chat"] [role=status]').first();
    await step(p, "d1-side-chat-remount", async () => {
      await open(p, "Fix the flaky retry test"); await p.getByRole("button", { name: "Side chat", exact: true }).click(); await st.waitFor({ timeout: 4000 });
      must(/↓ 1\.2k tokens/.test(await st.innerText()), `the side chat: "${await st.innerText()}"`);
      must((await st.locator(".tz-mark-write .tz-mark-strokes > path").count()) === 4, "the standalone Working row does not write the mark");
      await open(p, "Why is the CI build"); must(!(await st.count()), "the side chat shows beside another conversation");
      // back by its working row, titled by its first message
      await open(p, "The retry test fails on CI"); await st.waitFor({ timeout: 4000 });
      must(/↓ 1\.2k tokens/.test(await st.innerText()), `the side chat drawn again: "${await st.innerText()}"`);
    });
    await ctx.close(); }
  // a conversation out of sight that finishes is notified by the name the user gave it, found through every session it has
  // been (here an earlier one, not the one it was opened by)
  { const named = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { const m = p?.msg;
      if (m?.type === "sessions") m.dirs = m.dirs.map((d) => d.list.some((r) => r.id === "p1") ? { ...d, list: [...d.list.filter((r) => r.id !== "p0"), { id: "p0", first: "Office lunch app", named: true, last_at: 1, messages: 2, model: "claude-opus-5-5" }] } : d);
      if (m?.type === "view" && m.session_id === "p1") { m.earlier_sessions = ["p0"]; window.__p1 = { tid: p.tid, msg: m }; }
      return emit(e, p); }; })();`;
    const [ctx, p] = await ctxOf({ extra: named });
    await step(p, "d-notify-name", async () => {
      await open(p, "Fix the flaky retry test"); await open(p, "Why is the CI build");
      await p.evaluate(() => { const { tid, msg } = window.__p1; window.__TAURI_INTERNALS__.view(tid, { ...msg, from: 0, activity: "Done" }); }); await p.waitForTimeout(400);
      const told = await p.evaluate(() => window.__SENT__.filter(([c]) => c === "notify").map(([, a]) => `${a.title}: ${a.body}`));
      must(told.includes("tazuna is done: Office lunch app"), `notified: ${JSON.stringify(told)} ("tazuna is done: Office lunch app" wanted)`);
    });
    await ctx.close(); }
  // D2: each accent, chosen in Settings, in both themes: its tokens set and kept, on-accent text, the selected row and the
  // accent-soft tag read 4.5:1 or more; the swatches by keyboard; the accents in the command palette
  { const ACC = { Ai: ["#1d5f86", "#7fb0d0"], Indigo: ["#3d4aa8", "#8f9bf0"], Jade: ["#2f7a5e", "#6cc3a0"], Amber: ["#9a5f12", "#e3b25f"], Rose: ["#a8425f", "#ec8fae"], Graphite: ["#45433e", "#c9c4b9"] };
    const contrast = (p) => p.evaluate(() => {
      const rgb = (v) => { const d = document.createElement("i"); d.style.color = v; document.body.append(d); const c = getComputedStyle(d).color; d.remove(); const n = c.replace(/^[^(]*\(/, "").match(/[\d.]+/g).map(Number); return c.startsWith("color(") ? n.slice(0, 3).map((x) => x * 255) : n.slice(0, 3); };
      const L = (c) => { const [r, g, b] = c.map((v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
      const cr = (a, b) => { const [x, y] = [L(rgb(a)), L(rgb(b))]; return +((Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05)).toFixed(2); };
      return { "on-accent": cr("var(--on-accent)", "var(--accent)"), "faint on selected": cr("var(--faint)", "var(--selected)"), "ink-2 on selected": cr("var(--ink-2)", "var(--selected)"),
        "muted on accent-soft": cr("var(--muted)", "var(--accent-soft)"), "accent-ink on accent-soft": cr("var(--accent-ink)", "var(--accent-soft)"), "accent-ink on field": cr("var(--accent-ink)", "var(--field)") };
    });
    for (const theme of ["dark", "light"]) {
      const [ctx, p] = await ctxOf({ theme }), T = theme[0];
      await open(p, "Why is the CI build"); await p.getByRole("textbox", { name: "Message" }).fill("Make the cache key include the lockfile");
      for (const [n, [l, d]] of Object.entries(ACC)) { await step(p, `${T}-d2-accent-${n.toLowerCase()}`, async () => {
        await p.keyboard.press("Control+,"); await p.getByRole("radio", { name: n }).click(); await p.waitForTimeout(200);
        const got = await p.evaluate(() => ({ accent: getComputedStyle(document.documentElement).getPropertyValue("--accent").trim(), kept: localStorage.getItem("tazuna.accent") }));
        must(got.accent === (theme === "dark" ? d : l) && got.kept === n, `${n}: ${JSON.stringify(got)}`);
        const c = await contrast(p);
        log.push(`${T}-d2 ${n}: ${JSON.stringify(c)}`);
        must(Object.values(c).every((x) => x >= 4.5), `${n} under 4.5:1: ${JSON.stringify(c)}`);
        await p.screenshot({ path: `${OUT}/${T}-d2-accent-${n.toLowerCase()}-settings.png` });
        await esc(p);
      });
      // the selected row is ringed in the accent, so it never reads as a hovered one (Graphite's fill is a hover's grey)
      await step(p, `${T}-d2-selected-${n.toLowerCase()}`, async () => {
        await side(p).locator("a:not(.bg-selected)").nth(2).hover(); await p.waitForTimeout(200);
        const sel = await p.evaluate(() => {
          const rgb = (v) => { const d = document.createElement("i"); d.style.color = v; document.body.append(d); const c = getComputedStyle(d).color; d.remove(); const n = c.replace(/^[^(]*\(/, "").match(/[\d.]+/g).map(Number); return c.startsWith("color(") ? n.slice(0, 3).map((x) => x * 255) : n.slice(0, 3); };
          const L = (c) => { const [r, g, b] = c.map((v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
          const cr = (a, b) => { const [x, y] = [L(rgb(a)), L(rgb(b))]; return +((Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05)).toFixed(2); };
          const row = document.querySelector("aside[aria-label=Conversations] .bg-selected");
          return { shadow: row ? getComputedStyle(row).boxShadow : "no selected row", ring: cr("color-mix(in srgb, var(--accent) 35%, var(--selected))", "var(--selected)"), fill: cr("var(--selected)", "var(--hover)") };
        });
        log.push(`${T}-d2 ${n} selected: ${JSON.stringify(sel)}`);
        must(/inset/.test(sel.shadow) && sel.ring >= 1.3, `${n}: the selected row is not told from a hovered one: ${JSON.stringify(sel)}`);
      }); }
      // kept: a new window comes up in it
      await step(p, `${T}-d2-accent-kept`, async () => { await p.reload(); await p.waitForTimeout(900); must(await p.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--accent").trim()) === (theme === "dark" ? ACC.Graphite[1] : ACC.Graphite[0]), "the accent did not last a reload"); });
      await step(p, `${T}-d2-swatch-keys`, async () => {
        await p.keyboard.press("Control+,"); await p.getByRole("radio", { name: "Graphite" }).focus(); await p.keyboard.press("ArrowRight");
        const at = await p.evaluate(() => `${document.activeElement?.getAttribute("aria-label")} ${document.activeElement?.getAttribute("aria-checked")} ${localStorage.getItem("tazuna.accent")} ${document.querySelectorAll('[role=radiogroup][aria-label=Accent] [tabindex="0"]').length}`);
        must(at === "Ai true Ai 1", `ArrowRight from Graphite: ${at} (Ai true Ai 1 wanted)`);
      });
      await p.keyboard.press("ArrowRight"); await p.keyboard.press("ArrowRight"); await esc(p);
      await step(p, `${T}-d2-accent-jade-window`, async () => { await p.getByRole("textbox", { name: "Message" }).focus(); });
      await step(p, `${T}-d2-palette`, async () => {
        await p.keyboard.press("Control+k"); await p.keyboard.type("accent"); await p.waitForTimeout(250);
        const rows = await p.locator("[role=dialog] button[data-i]").allInnerTexts();
        must(rows.length === 6 && rows.some((r) => /Accent: Jade\s*now/.test(r)), `the palette: ${JSON.stringify(rows)}`);
        await p.screenshot({ path: `${OUT}/${T}-d2-palette-rows.png` });
        await p.keyboard.press("ArrowDown"); await p.keyboard.press("ArrowDown"); await p.keyboard.press("ArrowDown"); await p.keyboard.press("Enter"); await p.waitForTimeout(200);
        must(await p.evaluate(() => localStorage.getItem("tazuna.accent")) === "Amber", "the palette's Accent: Amber did not take");
      });
      await p.evaluate(() => { localStorage.removeItem("tazuna.accent"); });
      log.push(`${theme} D2 page errors: ${JSON.stringify(p.errs.slice(0, 5))}`);
      await ctx.close();
    } }
  // the accent is an attribute, not the root's inline style: a plugin's theme.set (that style) outlives a flip of the theme
  // and a change in Settings, and wins over the chosen accent; Ai (the default) takes the attribute off
  { const [ctx, p] = await ctxOf({ theme: "dark" });
    const now = () => p.evaluate(() => `${getComputedStyle(document.documentElement).getPropertyValue("--accent").trim()} ${document.documentElement.dataset.accent ?? "-"}`);
    await step(p, "d2-plugin-accent-kept", async () => {
      await p.evaluate(() => document.documentElement.style.setProperty("--accent", "#b4473a"));
      await p.keyboard.press("Control+Shift+L"); await p.waitForTimeout(200);
      must((await now()) === "#b4473a -", `after a flip: ${await now()}`);
      await p.keyboard.press("Control+,"); await p.getByRole("radio", { name: "Jade" }).click(); await p.waitForTimeout(200);
      must((await now()) === "#b4473a jade", `after Jade in Settings: ${await now()}`);
      await esc(p); await p.keyboard.press("Control+Shift+L"); await p.waitForTimeout(200);
      must((await now()) === "#b4473a jade", `after a second flip: ${await now()}`);
      // the plugin off: the chosen accent, in the theme of now (dark again)
      await p.evaluate(() => document.documentElement.style.removeProperty("--accent")); await p.waitForTimeout(100);
      must((await now()) === "#6cc3a0 jade", `the plugin gone: ${await now()}`);
      await p.keyboard.press("Control+,"); await p.getByRole("radio", { name: "Indigo" }).click(); await p.waitForTimeout(200);
      must((await now()) === "#8f9bf0 indigo", `Indigo: ${await now()}`);
      await p.getByRole("radio", { name: "Ai" }).click(); await p.waitForTimeout(200);
      must((await now()) === "#7fb0d0 -", `Ai: ${await now()}`);
    });
    await esc(p); await ctx.close(); }
  // what means a state keeps the live indigo under any accent: the status bar's meters, the sidebar's working row, a
  // running step's spinner (an amber or rose accent read as a warning or an error)
  for (const theme of ["dark", "light"]) for (const accent of ["Amber", "Rose"]) {
    const [ctx, p] = await ctxOf({ theme, extra: `localStorage.setItem("tazuna.accent", "${accent}");` }), N = `${theme[0]}-d2-state-${accent.toLowerCase()}`;
    await step(p, N, async () => {
      await open(p, "Fix the flaky retry test"); await p.waitForTimeout(300);
      const seen = await p.evaluate(() => {
        const live = getComputedStyle(document.documentElement).getPropertyValue("--live").trim(), want = (h) => `rgb(${[1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)).join(", ")})`;
        const fills = [...document.querySelectorAll(".bg-live")].map((e) => getComputedStyle(e).backgroundColor), marks = [...document.querySelectorAll(".text-live")].map((e) => getComputedStyle(e).color);
        return { accent: getComputedStyle(document.documentElement).getPropertyValue("--accent").trim(), live, fills: fills.length, marks: marks.length, off: [...fills, ...marks].filter((c) => c !== want(live)) };
      });
      log.push(`${N}: ${JSON.stringify(seen)}`);
      must(seen.live === (theme === "dark" ? "#7fb0d0" : "#1d5f86") && seen.accent !== seen.live, `the tokens: ${JSON.stringify(seen)}`);
      must(seen.fills >= 2 && seen.marks >= 2 && !seen.off.length, `the state marks: ${JSON.stringify(seen)}`);
    });
    await ctx.close();
  }
  // D3: in a plain chat the last answer keeps its numbers and the older ones show theirs on hover and focus, their actions
  // too; an agent's run (and a chat's answers by the agent) keep every line
  { const [ctx, p] = await ctxOf({}), shown = () => p.evaluate(() => [...document.querySelectorAll("[data-msg] div.scroll-mb-3")].map((e) => getComputedStyle(e).opacity));
    const answers = p.locator("[data-msg]").filter({ has: p.locator("div.scroll-mb-3") });
    await step(p, "d3-chat-numbers", async () => {
      await open(p, "Question 1"); await p.mouse.move(5, 400); await p.waitForTimeout(300);
      const o = await shown();
      must(o.length > 2 && o.slice(0, -1).every((x) => x === "0") && o.at(-1) === "1", `the chat's lines: ${o} (the last alone wanted)`);
    });
    await step(p, "d3-chat-hover", async () => {
      await answers.nth(-2).hover(); await p.waitForTimeout(300);
      must((await answers.nth(-2).locator("div.scroll-mb-3").evaluate((e) => getComputedStyle(e).opacity)) === "1", "an older answer's line did not show on hover");
    });
    // by keyboard: its line shows on the receipt's focus, and its actions (the next Tab stops: WebKit's Tab skips buttons,
    // so focused here) while one of them has it
    await step(p, "d3-chat-focus", async () => {
      const older = answers.nth(-3), seen = () => older.evaluate((m) => [m.querySelector("div.scroll-mb-3"), m.querySelector("div.scroll-mb-3 span.inline-flex")].map((e) => getComputedStyle(e).opacity).join() + ` ${document.activeElement?.textContent}`);
      await p.mouse.move(5, 400); await older.locator("div.scroll-mb-3 button").first().focus(); await p.waitForTimeout(300);
      must((await seen()).startsWith("1,0"), `the receipt focused: ${await seen()} (its line alone wanted)`);
      await older.locator("div.scroll-mb-3 span.inline-flex button").last().focus(); await p.waitForTimeout(300);
      must(/^1,1 Copy/.test(await seen()), `Copy focused: ${await seen()} (its line and actions wanted)`);
    });
    await step(p, "d3-agent-numbers", async () => { await open(p, "Why is the CI build"); await p.mouse.move(5, 400); await p.waitForTimeout(300); const o = await shown(); must(o.length > 1 && o.every((x) => x === "1"), `an agent's run: ${o}`); });
    await step(p, "d3-handoff-numbers", async () => { await open(p, "Will it rain in Berlin"); await p.mouse.move(5, 400); await p.waitForTimeout(300); const o = await shown(); must(o.length === 3 && o.every((x) => x === "1"), `a chat's answers by the agent: ${o}`); });
    await ctx.close(); }
  // a chat whose last reply failed at once (no answer, nothing spent, so no receipt): the answer before it keeps its line
  { const failed = `(() => { const T = window.__TAURI_INTERNALS__, emit = T.emit.bind(T); T.emit = (e, p) => { const m = p?.msg; if (m?.type === "view" && m.session_id === "c5" && !m.blocks.some((b) => b.text === "And edge case 9?"))
      m.blocks.push({ kind: "prompt", text: "And edge case 9?" }, { kind: "notice", text: "[500]: the provider could not answer", error: true }, { kind: "done", ok: false, secs: 0, turns: 0, cost: 0, usage: { input: 0, cache_write: 0, cache_read: 0, output: 0 }, stopped: false });
      return emit(e, p); }; })();`;
    const [ctx, p] = await ctxOf({ extra: failed }), shown = () => p.evaluate(() => [...document.querySelectorAll("[data-msg] div.scroll-mb-3")].map((e) => getComputedStyle(e).opacity));
    await step(p, "d3-chat-failed-last", async () => {
      await open(p, "Question 1"); await p.getByText("the provider could not answer").first().waitFor({ timeout: 4000 }); await p.mouse.move(5, 400); await p.waitForTimeout(300);
      const o = await shown();
      must(o.length > 2 && o.slice(0, -1).every((x) => x === "0") && o.at(-1) === "1", `after a failed last reply: ${o} (the last answer's line alone wanted)`);
    });
    await ctx.close(); }
  // D4: the mode pill's tints: full warn-tinted with a filled shield and its words, plan in the accent, the rest grey; the
  // menu keeps its descriptions (the mock answers a mode with the view in it)
  { const modes = `(() => { const T = window.__TAURI_INTERNALS__, inv = T.invoke.bind(T); T.invoke = async (c, a) => { if (c === "op" && a?.msg?.op === "mode") { setTimeout(() => window.__again((m) => ({ ...m, mode: a.msg.mode })), 10); return null; } return inv(c, a); }; })();`;
    for (const [theme, w, h] of [["dark", 1280, 800], ["light", 1280, 800], ["dark", 760, 640]]) {
      const [ctx, p] = await ctxOf({ theme, w, h, extra: LAST + modes }), N = `${theme[0]}-d4${w === 1280 ? "" : `-w${w}`}`;
      const pill = p.locator("[data-composer] button[aria-haspopup=menu]").filter({ hasText: /^(Auto|Ask|Edits|Review|Plan|Full)/ });
      if (w < 1000) { await p.getByRole("button", { name: "Conversations", exact: true }).click(); await p.waitForTimeout(250); }
      await open(p, "Why is the CI build");
      for (const m of ["full", "plan", "ask"]) await step(p, `${N}-mode-${m}`, async () => {
        await pill.click(); await p.getByRole("menuitemradio").filter({ hasText: new RegExp(`^${m}`, "i") }).click(); await p.mouse.move(5, 400); await p.waitForTimeout(300);
        const s = await pill.evaluate((b) => ({ tint: (b.className.match(/tz-mode-\w+/) ?? [""])[0], text: b.innerText.trim(), fill: b.querySelector("svg").getAttribute("fill"), color: getComputedStyle(b).color }));
        log.push(`${N} ${m}: ${JSON.stringify(s)}`);
        const want = { full: ["tz-mode-full", "Full · never asks", "currentColor"], plan: ["tz-mode-plan", "Plan", "none"], ask: ["", "Ask", "none"] }[m];
        must(s.tint === want[0] && s.text === want[1] && s.fill === want[2], `${m}: ${JSON.stringify(s)}, wanted ${want}`);
        await p.locator("[data-composer]").screenshot({ path: `${OUT}/${N}-mode-${m}-box.png` });
      });
      await step(p, `${N}-mode-menu`, async () => { await pill.click(); must((await p.getByRole("menuitemradio").count()) === 6 && (await p.getByText("a second model approves commands").isVisible()), "the mode menu lost its descriptions"); });
      await esc(p);
      // plan keeps the live indigo under every accent, never the accent (under Amber it looked like full)
      if (w === 1280) for (const accent of ["ai", "indigo", "jade", "amber", "rose", "graphite"]) await step(p, `${N}-plan-vs-full-${accent}`, async () => {
        await p.evaluate((a) => { if (a === "ai") delete document.documentElement.dataset.accent; else document.documentElement.dataset.accent = a; }, accent);
        const read = async (m) => { await pill.click(); await p.getByRole("menuitemradio").filter({ hasText: new RegExp(`^${m}`, "i") }).click(); await p.mouse.move(5, 400); await p.waitForTimeout(250);
          return pill.evaluate((b) => { const cs = getComputedStyle(b), root = getComputedStyle(document.documentElement), hex = (v) => `rgb(${[1, 3, 5].map((i) => parseInt(root.getPropertyValue(v).trim().slice(i, i + 2), 16)).join(", ")})`;
            return { color: cs.color, border: cs.borderColor, liveInk: hex("--live-ink"), warn: hex("--warn") }; }); };
        const plan = await read("plan");
        if (accent === "amber") await p.locator("[data-composer]").screenshot({ path: `${OUT}/${N}-plan-amber-box.png` });
        const full = await read("full");
        log.push(`${N} ${accent}: plan ${plan.color} ${plan.border}, full ${full.color} ${full.border}`);
        must(plan.color === plan.liveInk && full.color === full.warn && plan.border !== full.border, `${accent}: plan ${JSON.stringify(plan)}, full ${JSON.stringify(full)}`);
      });
      await ctx.close();
    } }
  { const [ctx, p] = await ctxOf({ setup: false }); await step(p, "first-run-setup", async () => { await p.waitForTimeout(600); }); await ctx.close(); }
  fs.writeFileSync(`${OUT}/log.txt`, log.join("\n"));
  console.log(log.filter((l) => !l.startsWith("ok")).join("\n") || "every step ok", `· ${log.length} steps in ${OUT}`);
  await browser.close();
  // a failed step fails the run (design review 13: failures were only lines in the log)
  if (log.some((l) => l.startsWith("FAIL"))) process.exitCode = 1;
})();
