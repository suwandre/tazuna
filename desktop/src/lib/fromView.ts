// The core's view (Session::view) turned into what the window draws.
import type { View } from "./tazuna";
import type { Call, Message, ToolStep, Usage } from "./types";

/** A provider's error as its message alone: "provider · provider error in stream: {\"message\":\"[404]: … not found.\", …}"
 * (cut at 300 characters, so not always JSON) is "[404]: … not found.". The terminal reads it the same way (src/ui/mod.rs). */
export function readableError(text: string): string {
  const m = text.match(/"message"\s*:\s*"((?:[^"\\]|\\.)*)"/);
  const msg = m ? m[1].replace(/\\n/g, " ").replace(/\\(.)/g, "$1") : text.replace(/^[\w ]+ · /, "").replace(/ · trying again in \d+\s*s$/, "");
  return msg.trim();
}

function target(input: Record<string, unknown>): string {
  for (const k of ["file_path", "command", "pattern", "path", "url", "query", "skill", "description"]) {
    // a command that first goes to the project's folder says only what it runs there: every step read
    // `cd "/Users/…/application" && gh pr view …` (design review 6; the whole command is in the step's details)
    const v = typeof input[k] === "string" && k === "command" ? (input[k] as string).replace(/^cd\s+("[^"]+"|'[^']+'|\S+)\s*&&\s*/, "") : input[k];
    if (typeof v === "string" && v) return v.length > 80 ? `${v.slice(0, 80)}…` : v;
  }
  return "";
}

function verb(name: string): string {
  const n = name.replace(/^mcp__\w+__/, "");
  return ({ Read: "Read", read: "Read", Bash: "Ran", bash: "Ran", Grep: "Searched", Glob: "Listed", Edit: "Edited", edit: "Edited", Write: "Wrote", write: "Wrote",
    Task: "Asked a subagent", Agent: "Asked a subagent", fetch: "Fetched", WebFetch: "Fetched", Skill: "Used skill", WebSearch: "Searched the web" } as Record<string, string>)[n] ?? n;
}

/** Each call with what it called: the tool blocks from its start to the next call's (or the end). */
function calls(v: View): (Call & { block: number })[] {
  const list = v.calls ?? [];
  return list.map((c, i) => {
    const end = list[i + 1]?.block ?? v.blocks.length, counts = new Map<string, number>();
    for (const b of v.blocks.slice(c.block, end)) if (b.kind === "tool") { const w = verb(b.name); counts.set(w, (counts.get(w) ?? 0) + 1); }
    const tools = [...counts].map(([w, n]) => (n > 1 ? `${w} ×${n}` : w)).join(", ");
    return { block: c.block, tools, fresh: c.fresh, cached: c.cached, out: c.out, cost: c.cost, firstMs: c.first_ms, tps: c.tps, totalMs: c.total_ms };
  });
}

export function messages(v: View): Message[] {
  const out: Message[] = [];
  // the calls not yet given to a reply: each done block takes those that began before it
  const pending = calls(v);
  let agent: Extract<Message, { role: "agent" }> | null = null, ask = "";
  // the last error a reply met (an error notice, or a provider's retry): what a failed or stopped reply shows
  let lastError = "";
  // a message sent during a reply waits in the core's queue: shown after that reply ends, not above it
  let open = false;
  const queue: string[] = [];
  const edits = new Map((v.edit_points ?? []).map((e) => [e.block, e]));
  let at = 0;
  const prompt = (text: string, block = at) => { out.push({ role: "user", text, edit: edits.get(block) }); agent = null; ask = text; open = true; lastError = ""; };
  const current = () => (agent ??= (out.push({ role: "agent", prose: [], tools: [] }), out[out.length - 1] as Extract<Message, { role: "agent" }>));
  const changeAt = new Map((v.changes ?? []).map((c) => [c.at_block, c]));
  for (const [k, b] of v.blocks.entries()) {
    at = k;
    if (b.kind === "prompt") { if (open) queue.push(b.text); else prompt(b.text); }
    // the router answered it as a chat (app server, 0.1.127's router): the reply carries the tag and the ask, for "send to agent"
    else if (b.kind === "notice" && b.text.startsWith("answered as a chat")) { Object.assign(current(), { routedChat: true, ask }); }
    else if (b.kind === "text") { if (b.text.trim()) current().prose.push(b.text); }
    else if (b.kind === "tool") {
      const t: ToolStep = { kind: "tool", name: b.name, verb: verb(b.name), target: target(b.input ?? {}), secs: b.secs ?? 0, ok: !b.error,
        running: b.running, stopped: b.stopped, input: b.input, result: b.result };
      current().tools!.push(t);
    }
    else if (b.kind === "thinking") {
      const tok = b.tokens >= 1000 ? `${(b.tokens / 1000).toFixed(1)}k` : `${b.tokens}`;
      current().tools!.push({ kind: "thinking", verb: b.live ? "Thinking" : "Thought", target: "", detail: b.tokens ? `~${tok} tokens` : undefined,
        secs: 0, ok: true, running: b.live, text: b.text });
    }
    else if (b.kind === "done") {
      // the reply's own cost and tokens (the totals so far were shown under every answer until desktop 0.1.80)
      const u = b.run_usage ?? b.usage, total = u.input + u.cache_write + u.cache_read + u.output, read = u.input + u.cache_write + u.cache_read;
      let n = 0;
      while (n < pending.length && pending[n].block < k) n++;
      const mine = pending.splice(0, n).map(({ block: _, ...c }) => c);
      current().meta = { cost: b.run_cost ?? b.cost, tokens: total, cachedPct: read ? Math.round((100 * u.cache_read) / read) : undefined, secs: Math.round(b.secs),
        calls: mine.length ? mine : undefined };
      // a reply that ended without its answer says why, where a provider's error before it said (until desktop 0.1.74 the
      // error was its raw JSON, as the answer's text, and a failed reply ended with nothing)
      if (!b.ok || b.stopped) current().failure = { text: lastError ? readableError(lastError) : b.stopped ? "" : "The reply ended without an answer.", raw: lastError, stopped: b.stopped, ask };
      const c = changeAt.get(k);
      if (c) current().change = { turn: c.turn, files: c.files, undone: c.undone, canUndo: c.can_undo };
      agent = null;
      open = false;
      const next = queue.shift();
      if (next !== undefined) prompt(next);
    }
    else if (b.kind === "notice" && (b.error || /provider error|HTTP \d{3}/.test(b.text))) {
      lastError = b.text;
      // an error the reply went on past (a tool server that failed to start) stays in sight, in words; a provider's retry
      // shows only if the reply then fails
      if (b.error) (current().notes ??= []).push(b.text);
    }
  }
  for (const text of queue) out.push({ role: "user", text, queued: true });
  return out;
}

/** The messages waiting behind the reply now running (sent during it; the core holds them until it ends). */
export const queued = (v: View) => messages(v).flatMap((m) => (m.role === "user" && m.queued ? [m.text] : []));

export function usage(v: View): Usage {
  const win = v.context_window ?? 200000;
  const k = (n: number) => (n >= 1e6 ? `${+(n / 1e6).toFixed(1)}M` : n >= 1000 ? `${Math.round(n / 1000)}k` : `${n}`);
  const u = v.usage, read = u.input + u.cache_write + u.cache_read;
  return { contextPct: Math.min(100, Math.round((100 * v.context) / win)), contextUsed: k(v.context), contextMax: k(win),
    plan5h: v.limits ? Math.round(v.limits[0]) : null, plan5hReset: resetIn(v.limits_reset), planWeek: v.limits ? Math.round(v.limits[1]) : null,
    threadCost: v.cost, threadTurns: v.turns, cachedPct: read ? Math.round((100 * u.cache_read) / read) : 0, saved: 0, today: v.cost, todayTurns: v.turns, todaySeries: [], balance: v.balance };
}

/** "in 2 h 14 min" until the plan's 5-hour window resets; empty when unknown or past. */
export function resetIn(at?: number | null, now = Date.now() / 1000): string {
  if (!at || at <= now) return "";
  const m = Math.round((at - now) / 60), h = Math.floor(m / 60);
  return h ? `in ${h} h ${m % 60} min` : `in ${m} min`;
}
