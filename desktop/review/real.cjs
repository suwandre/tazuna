// The real-data review: the window on the real core (review/bridge.mjs running), your saved conversations, projects,
// models, accounts and settings, with the in-page checks on each screen. Read-only: the bridge refuses anything that
// sends or changes. Screens and report.json go to the out dir.
//   node review/bridge.mjs & npm run dev
//   BRIDGE_TOKEN=<the bridge's token> [BRIDGE=http://127.0.0.1:4721] node review/real.cjs http://localhost:1420 /tmp/tazuna-real "/path/to/project" ["/another"]
const { chromium } = require(process.env.PW || "playwright");
const fs = require("fs"), path = require("path");
const audit = require("./audit.cjs");
const [BASE = "http://localhost:1420", OUT = "/tmp/tazuna-real", ...PROJECTS] = process.argv.slice(2);
fs.mkdirSync(OUT, { recursive: true });
(async () => {
  const b = await chromium.launch(), report = {};
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: "dark" });
  if (process.env.BRIDGE) await ctx.addInitScript(`window.TAZUNA_BRIDGE = ${JSON.stringify(process.env.BRIDGE)};`);
  await ctx.addInitScript(`window.TAZUNA_BRIDGE_TOKEN = ${JSON.stringify(process.env.BRIDGE_TOKEN || "")};`);
  await ctx.addInitScript(fs.readFileSync(path.join(__dirname, "real.js"), "utf8"));
  await ctx.addInitScript(`localStorage.setItem("tazuna.projects", ${JSON.stringify(JSON.stringify(PROJECTS))});`);
  const p = await ctx.newPage(), errs = [];
  p.on("pageerror", (e) => errs.push(String(e)));
  await p.goto(BASE); await p.waitForTimeout(4000);
  const side = p.locator("aside[aria-label=Conversations]");
  const shot = async (name) => { await p.waitForTimeout(500); await p.screenshot({ path: `${OUT}/${name}.png` }); report[name] = await p.evaluate(audit); };
  await shot("start");
  const titles = await side.locator("a[title]").evaluateAll((as) => as.map((a) => a.getAttribute("title")));
  for (const [i, t] of titles.slice(0, 5).entries()) {
    await side.locator(`a[title="${t.replace(/"/g, '\\"')}"]`).first().click().catch(() => {}); await p.waitForTimeout(3500);
    await shot(`thread${i}-end`);
    await p.mouse.move(700, 400); await p.mouse.wheel(0, -50000); await p.waitForTimeout(400); await shot(`thread${i}-top`);
  }
  report.openAfterFive = await side.locator("a").evaluateAll((as) => as.length);
  if (PROJECTS[0]) { await side.locator(`button[title="${PROJECTS[0]}"]`).click(); await p.waitForTimeout(4000); await shot("project"); }
  await p.getByTitle("Model and effort").click(); await p.waitForTimeout(2500);
  report.pickerFocus = await p.evaluate(() => document.activeElement?.getAttribute("placeholder") || document.activeElement?.tagName);
  await p.keyboard.type("gpt"); await p.waitForTimeout(500); await shot("picker-search"); await p.keyboard.press("Escape");
  await p.keyboard.press("Control+Shift+U"); await p.waitForTimeout(12000); await shot("usage");
  for (let i = 1; i <= 3; i++) { await p.mouse.move(720, 450); await p.mouse.wheel(0, 650); await shot(`usage-${i}`); }
  await p.keyboard.press("Escape");
  await p.keyboard.press("Control+,"); await p.waitForTimeout(2500); await shot("settings"); await p.keyboard.press("Escape");
  report.errors = errs;
  fs.writeFileSync(`${OUT}/report.json`, JSON.stringify(report, null, 1));
  const flags = Object.entries(report).filter(([, v]) => v && typeof v === "object" && "contrast" in v)
    .flatMap(([k, v]) => [...v.clipped.map((c) => `${k}: cut ${c}`), ...Object.keys(v.contrast).map((c) => `${k}: contrast ${c}`), ...(v.overflowX ? [`${k}: wider than the window`] : [])]);
  console.log(flags.join("\n") || "no cut text, contrast or overflow", `\npicker focus: ${report.pickerFocus} · rows after opening five: ${report.openAfterFive} · page errors: ${errs.length}`);
  await b.close();
})();
