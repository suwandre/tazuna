// Self-check: drive server.mjs over stdio and assert tool behavior.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import assert from "node:assert/strict";

const srv = spawn("node", [new URL("./server.mjs", import.meta.url).pathname]);
let buf = "";
const waiting = new Map();
srv.stdout.on("data", (d) => {
  buf += d;
  let i;
  while ((i = buf.indexOf("\n")) >= 0) {
    const msg = JSON.parse(buf.slice(0, i));
    buf = buf.slice(i + 1);
    waiting.get(msg.id)?.(msg);
  }
});
let next = 1;
const rpc = (method, params) =>
  new Promise((res) => {
    const id = next++;
    waiting.set(id, res);
    srv.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
const call = async (name, args) => (await rpc("tools/call", { name, arguments: args })).result;

const dir = mkdtempSync(join(tmpdir(), "lean-"));
const f = join(dir, "a/b.txt");

assert.equal((await rpc("initialize", { protocolVersion: "2025-06-18" })).result.serverInfo.name, "lean");
srv.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");
assert.deepEqual((await rpc("tools/list")).result.tools.map((t) => t.name), ["bash", "read", "write", "edit"]);
assert.equal((await call("write", { path: f, content: "x\ny\nx\n" })).content[0].text, "ok");
assert.equal((await call("read", { path: f, offset: 2, limit: 1 })).content[0].text, "y");
assert.ok((await call("edit", { path: f, old: "x", new: "z" })).isError, "ambiguous edit must fail");
assert.equal((await call("edit", { path: f, old: "y", new: "$&q" })).content[0].text, "ok");
assert.equal(readFileSync(f, "utf8"), "x\n$&q\nx\n");
assert.match((await call("bash", { command: "echo hi; exit 3" })).content[0].text, /hi\n\n\[exit 3\]/);
assert.equal((await rpc("nope")).error.code, -32601);
await call("bash", { command: `cd ${dir}/a` });
assert.equal((await call("bash", { command: "pwd" })).content[0].text.trim().replace(/^\/private/, ""), `${dir}/a`.replace(/^\/private/, ""));
srv.kill();
console.log("ok");
