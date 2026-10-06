#!/usr/bin/env node
// Minimal stdio MCP server: bash/read/write/edit with terse schemas.
// Lets `claude -p --tools ""` run with a pi-sized tool prompt.
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { createInterface } from "node:readline";

const MAX = Number(process.env.LEAN_MAX_OUTPUT) || 30000; // ponytail: hard output cap, add paging if agents hit it often
const cap = (s) => (s.length > MAX ? s.slice(0, MAX) + `\n[truncated ${s.length - MAX} chars]` : s);
const obj = (props, required) => ({ type: "object", properties: props, required });
const str = { type: "string" };
const int = { type: "integer" };

// ponytail: only cwd persists between bash calls, not env vars; use a long-lived shell if that matters
let cwd = process.cwd();
const CWD_MARK = "\n__LEAN_CWD__";

const tools = {
  bash: {
    description: "Run shell command. Working directory persists between calls.",
    inputSchema: obj({ command: str }, ["command"]),
    run: ({ command }) => {
      const script = `${command}\n__rc=$?; printf '${CWD_MARK.replace("\n", "\\n")}%s' "$PWD" >&2; exit $__rc`;
      const r = spawnSync("bash", ["-lc", script], { cwd, encoding: "utf8", timeout: 120000, maxBuffer: 1 << 26 });
      let stderr = r.stderr ?? "";
      const i = stderr.lastIndexOf(CWD_MARK);
      if (i >= 0) {
        cwd = stderr.slice(i + CWD_MARK.length);
        stderr = stderr.slice(0, i);
      }
      return cap(`${r.stdout}${stderr}${r.status ? `\n[exit ${r.status}]` : ""}${r.error ? `\n[${r.error.message}]` : ""}`);
    },
  },
  read: {
    description: "Read file. Optional 1-based line offset/limit.",
    inputSchema: obj({ path: str, offset: int, limit: int }, ["path"]),
    run: ({ path, offset = 1, limit }) => {
      const lines = readFileSync(path, "utf8").split("\n");
      return cap(lines.slice(offset - 1, limit ? offset - 1 + limit : undefined).join("\n"));
    },
  },
  write: {
    description: "Write file, creating dirs.",
    inputSchema: obj({ path: str, content: str }, ["path", "content"]),
    run: ({ path, content }) => {
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, content);
      return "ok";
    },
  },
  edit: {
    description: "Replace exact unique text in file.",
    inputSchema: obj({ path: str, old: str, new: str }, ["path", "old", "new"]),
    run: ({ path, old, new: replacement }) => {
      const text = readFileSync(path, "utf8");
      const n = text.split(old).length - 1;
      if (n !== 1) throw new Error(`old text found ${n} times, need exactly 1`);
      writeFileSync(path, text.replace(old, () => replacement));
      return "ok";
    },
  },
};

function handle({ id, method, params }) {
  switch (method) {
    case "initialize":
      return { protocolVersion: params?.protocolVersion ?? "2025-06-18", capabilities: { tools: {} }, serverInfo: { name: "lean", version: "0.1.0" } };
    case "ping":
      return {};
    case "tools/list":
      return { tools: Object.entries(tools).map(([name, t]) => ({ name, description: t.description, inputSchema: t.inputSchema })) };
    case "tools/call": {
      const tool = tools[params?.name];
      const args = params?.arguments ?? {};
      if (typeof args.path === "string") args.path = resolve(cwd, args.path); // relative to persisted cwd
      if (!tool) throw new Error(`unknown tool ${params?.name}`);
      try {
        return { content: [{ type: "text", text: String(tool.run(args)) }] };
      } catch (e) {
        return { content: [{ type: "text", text: e.message }], isError: true };
      }
    }
    default:
      if (id === undefined) return undefined; // notification
      throw Object.assign(new Error(`method not found: ${method}`), { code: -32601 });
  }
}

createInterface({ input: process.stdin }).on("line", (line) => {
  if (!line.trim()) return;
  const msg = JSON.parse(line);
  let reply;
  try {
    const result = handle(msg);
    if (msg.id === undefined) return;
    reply = { jsonrpc: "2.0", id: msg.id, result };
  } catch (e) {
    if (msg.id === undefined) return;
    reply = { jsonrpc: "2.0", id: msg.id, error: { code: e.code ?? -32000, message: e.message } };
  }
  process.stdout.write(JSON.stringify(reply) + "\n");
});
