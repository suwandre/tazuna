// The desktop window in a browser on the real core, read-only (design review 5: the bugs test data hid showed on real
// conversations, accounts and models): `tazuna app-server` per conversation as the Tauri shell
// starts it; ops that only read pass (sessions, resume, models, usage, accounts, settings, diff, review…); anything that
// sends, changes or spends is refused here.
import http from "node:http";
import crypto from "node:crypto";
import { spawn } from "node:child_process";
import readline from "node:readline";
//   node review/bridge.mjs [port]        # TAZUNA_BIN=… for another build of the core; then review/real.cjs
const BIN = process.env.TAZUNA_BIN || `${process.env.HOME}/.local/bin/tazuna`, PORT = Number(process.argv[2] || 4720);
// it serves your conversations: only to the review pages (BRIDGE_ORIGINS, the dev server's by default), only with this
// run's token (BRIDGE_TOKEN, or a new one printed at start), only addressed to this machine (a DNS-rebound name is not)
const TOKEN = process.env.BRIDGE_TOKEN || crypto.randomBytes(32).toString("hex");
const ORIGINS = new Set((process.env.BRIDGE_ORIGINS || "http://localhost:1420,http://127.0.0.1:1420").split(","));
const HOSTS = new Set([`127.0.0.1:${PORT}`, `localhost:${PORT}`]);
const same = (a, b) => a.length === b.length && crypto.timingSafeEqual(Buffer.from(a), Buffer.from(b));
const READ = new Set(["models", "sessions", "resume", "diff", "settings", "usage", "accounts", "review", "pr", "worktrees", "gallery", "setup", "mcp"]);
const kids = new Map(), clients = new Set(), log = [];
const emit = (ev) => { const s = `data: ${JSON.stringify(ev)}\n\n`; for (const c of clients) c.write(s); };
const stop = (tid) => { const k = kids.get(tid); if (k) { k.kill(); kids.delete(tid); } };
async function invoke(cmd, a) {
  if (cmd === "start") {
    stop(a.tid);
    const dir = a.cwd || process.env.HOME;
    const args = ["app-server", "--allow-non-git", ...(a.profile ? ["--profile", a.profile] : [])];
    const k = spawn(BIN, args, { cwd: dir, stdio: ["pipe", "pipe", "ignore"], env: { ...process.env, TAZUNA_NO_UPDATE_CHECK: "1" } });
    kids.set(a.tid, k);
    readline.createInterface({ input: k.stdout }).on("line", (l) => { try { emit({ event: "tazuna-view", payload: { tid: a.tid, msg: JSON.parse(l) } }); } catch {} });
    k.on("exit", () => { if (kids.get(a.tid) === k) emit({ event: "tazuna-exit", payload: a.tid }); });
    return dir;
  }
  if (cmd === "op") {
    const op = a.msg?.op;
    // read ops only, and those without a value that would set something (mcp with fields adds, hook with a command sets)
    if (!READ.has(op) || (op === "mcp" && Object.keys(a.msg).length > 1)) { log.push(`refused op ${op}`); throw `refused in review: ${op}`; }
    kids.get(a.tid)?.stdin.write(JSON.stringify(a.msg) + "\n");
    return null;
  }
  if (cmd === "stop") { stop(a.tid); return null; }
  if (["send", "interrupt", "trust", "new_chat", "set_profile", "update_install", "plugins_enable", "widgets_new", "widgets_remove", "plugins_import"].includes(cmd)) { log.push(`refused ${cmd}`); throw `refused in review: ${cmd}`; }
  if (cmd === "plugin:app|version") return "review";
  if (cmd === "widgets_list") return [];
  if (cmd === "update_check") return null;
  return null;
}
http.createServer(async (req, res) => {
  const origin = req.headers.origin;
  if (!HOSTS.has(req.headers.host ?? "") || (origin && !ORIGINS.has(origin))) { res.writeHead(403); return res.end(); }
  if (origin) { res.setHeader("Access-Control-Allow-Origin", origin); res.setHeader("Vary", "Origin"); }
  res.setHeader("Access-Control-Allow-Headers", "content-type, x-bridge-token");
  if (req.method === "OPTIONS") return res.end();
  const given = req.headers["x-bridge-token"] ?? new URL(req.url, "http://x").searchParams.get("token") ?? "";
  if (!same(String(given), TOKEN)) { res.writeHead(401); return res.end(); }
  if (req.url.startsWith("/events")) { res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-cache" }); clients.add(res); req.on("close", () => clients.delete(res)); return; }
  if (req.url === "/log") return res.end(JSON.stringify({ log, kids: [...kids.keys()] }));
  let body = ""; for await (const c of req) body += c;
  try { const { cmd, args } = JSON.parse(body); res.end(JSON.stringify({ ok: await invoke(cmd, args || {}) ?? null })); }
  catch (e) { res.end(JSON.stringify({ err: String(e) })); }
}).listen(PORT, "127.0.0.1", () => console.log(`bridge on ${PORT} · token ${process.env.BRIDGE_TOKEN ? "(from BRIDGE_TOKEN)" : TOKEN} · pages ${[...ORIGINS].join(" ")}`));
const bye = () => { for (const k of kids.values()) k.kill(); process.exit(0); };
process.on("SIGTERM", bye); process.on("SIGINT", bye);
