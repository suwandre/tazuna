// The core's view (Session::view) turned into what the window draws.
import type { View } from "./tazuna";
import type { Message, ToolStep, Usage } from "./types";

function target(input: Record<string, unknown>): string {
  for (const k of ["file_path", "command", "pattern", "path", "url", "query", "description"]) {
    const v = input[k];
    if (typeof v === "string" && v) return v.length > 80 ? `${v.slice(0, 80)}…` : v;
  }
  return "";
}

function verb(name: string): string {
  const n = name.replace(/^mcp__\w+__/, "");
  return ({ Read: "Read", read: "Read", Bash: "Ran", bash: "Ran", Grep: "Searched", Glob: "Listed", Edit: "Edited", edit: "Edited", Write: "Wrote", write: "Wrote",
    Task: "Asked a subagent", Agent: "Asked a subagent", fetch: "Fetched", WebFetch: "Fetched" } as Record<string, string>)[n] ?? n;
}

export function messages(v: View): Message[] {
  const out: Message[] = [];
  let agent: Extract<Message, { role: "agent" }> | null = null, ask = "";
  const current = () => (agent ??= (out.push({ role: "agent", prose: [], tools: [] }), out[out.length - 1] as Extract<Message, { role: "agent" }>));
  for (const b of v.blocks) {
    if (b.kind === "prompt") { out.push({ role: "user", text: b.text }); agent = null; ask = b.text; }
    // the router answered it as a chat (app server, 0.1.127's router): the reply carries the tag and the ask, for "send to agent"
    else if (b.kind === "notice" && b.text.startsWith("answered as a chat")) { Object.assign(current(), { routedChat: true, ask }); }
    else if (b.kind === "text") { if (b.text.trim()) current().prose.push(b.text); }
    else if (b.kind === "tool") {
      const t: ToolStep = { verb: verb(b.name), target: target(b.input ?? {}), secs: b.secs ?? 0, ok: !b.error, running: b.running };
      current().tools!.push(t);
    }
    else if (b.kind === "done") {
      const u = b.usage, total = u.input + u.cache_write + u.cache_read + u.output, read = u.input + u.cache_write + u.cache_read;
      current().meta = { cost: b.cost, tokens: total, cachedPct: read ? Math.round((100 * u.cache_read) / read) : undefined, secs: Math.round(b.secs) };
      agent = null;
    }
    else if (b.kind === "notice" && b.error) { current().prose.push(`⚠ ${b.text}`); }
  }
  return out;
}

export function usage(v: View): Usage {
  const win = v.context_window ?? 200000;
  const k = (n: number) => (n >= 1000 ? `${Math.round(n / 1000)}k` : `${n}`);
  const u = v.usage, read = u.input + u.cache_write + u.cache_read;
  return { contextPct: Math.min(100, Math.round((100 * v.context) / win)), contextUsed: k(v.context), contextMax: k(win),
    plan5h: v.limits ? Math.round(v.limits[0]) : null, plan5hReset: "", planWeek: v.limits ? Math.round(v.limits[1]) : null,
    threadCost: v.cost, threadTurns: v.turns, cachedPct: read ? Math.round((100 * u.cache_read) / read) : 0, saved: 0, today: v.cost, todayTurns: v.turns, todaySeries: [] };
}
