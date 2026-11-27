// The core's view (Session::view) turned into what the window draws.
import { isFree } from "./route";
import { rewritten, stripCd } from "./steps";
import type { ModelChoice, View } from "./tazuna";
import { planName } from "./usage";
import { median, paidTo, pct1, usd, writing, type Call, type Message, type ToolStep, type Usage } from "./types";

/** Words before a later step that are an aside, not an answer: one short paragraph, no code block, table or list. */
export const interim = (text: string) => { const t = text.trim(); return t.length <= 280 && !/\n\s*\n/.test(t) && !/^\s*(```|\||[-*+]\s|\d+[.)]\s|#)/m.test(t); };

/** A provider's error as its message alone: "provider · provider error in stream: {\"message\":\"[404]: … not found.\", …}"
 * (cut at 300 characters, so not always JSON) is "[404]: … not found.". The terminal reads it the same way (src/ui/mod.rs). */
export function readableError(text: string): string {
  const m = text.match(/"message"\s*:\s*"((?:[^"\\]|\\.)*)"/);
  const msg = m ? m[1].replace(/\\n/g, " ").replace(/\\(.)/g, "$1") : text.replace(/^[\w ]+ · /, "").replace(/ · trying again in \d+\s*s$/, "");
  // "[404]: Antigravity upstream error (404): …" says its code twice: the first goes (design review 9; src/ui/mod.rs too);
  // the provider's error codes ("(rate_limit_exceeded, rate_limit_error)") are for the raw reply, and it starts with a
  // capital (design review 14)
  const said = msg.trim().replace(/^\[(\d{3})\]:\s*(?=.*\b\1\b)/, "").replace(/\s*\((?:[a-z]+_[a-z_]+(?:,\s*)?)+\)(?=\)*$)/, "");
  // only an ordinary word ("the provider is…"), never a name or a code ("ssh: …", "gpt-5 is…")
  return /^[a-z]+ /.test(said) ? said.charAt(0).toUpperCase() + said.slice(1) : said;
}

function target(input: Record<string, unknown>, cwd?: string): string {
  for (const k of ["file_path", "command", "pattern", "path", "url", "query", "skill", "description"]) {
    // a command that first goes to the project's folder (or a folder in it) says only what it runs there: every step read
    // `cd "/Users/…/application" && gh pr view …` (design review 6; the whole command is in the step's details). A cd
    // elsewhere stays: "cd /tmp && rm -rf build" is not "rm -rf build" (review of review 14)
    const v = typeof input[k] === "string" && k === "command" ? stripCd(input[k] as string, cwd).cmd : input[k];
    if (typeof v === "string" && v) return v.length > 80 ? `${v.slice(0, 80)}…` : v;
  }
  return "";
}

function verb(name: string): string {
  const n = name.replace(/^mcp__\w+__/, "");
  return ({ Read: "Read", read: "Read", Bash: "Ran", bash: "Ran", Grep: "Searched", Glob: "Listed", Edit: "Edited", edit: "Edited", MultiEdit: "Edited", multi_edit: "Edited", Write: "Wrote", write: "Wrote",
    Task: "Asked a subagent", Agent: "Asked a subagent", fetch: "Fetched", WebFetch: "Fetched", Skill: "Used skill", WebSearch: "Searched the web", search: "Searched the web" } as Record<string, string>)[n] ?? n;
}

/** Each call with what it called: the tool blocks from its start to the next call's (or the end). */
function calls(v: View): (Call & { block: number })[] {
  const list = v.calls ?? [];
  return list.map((c, i) => {
    const end = list[i + 1]?.block ?? v.blocks.length, counts = new Map<string, number>();
    for (const b of v.blocks.slice(c.block, end)) if (b.kind === "tool") { const w = verb(b.name); counts.set(w, (counts.get(w) ?? 0) + 1); }
    const tools = [...counts].map(([w, n]) => (n > 1 ? `${w} ×${n}` : w)).join(", ");
    return { block: c.block, tools, fresh: c.fresh, cached: c.cached, out: c.out, cost: c.cost, firstMs: c.first_ms, tps: c.tps, totalMs: c.total_ms, model: c.model, role: c.role, estimate: c.estimate };
  });
}

/** The running reply's output tokens so far (design review 13, D1: "↓ 1.2k tokens" on its Working row): the output of the
 * model calls that began after the last finished turn, read from the view alone, so a reply begun out of sight or a side
 * chat drawn again counts the same (not after the last message: one queued during the reply adds a prompt). A subagent's
 * tokens are its whole context, not output: they stay on its own row. */
export const replyOut = (v: View | null | undefined) => {
  const done = (v?.blocks ?? []).reduce((n, b, i) => (b.kind === "done" ? i : n), -1);
  return (v?.calls ?? []).filter((c) => c.block > done).reduce((n, c) => n + c.out, 0);
};

type Done = Extract<View["blocks"][number], { kind: "done" }>;
/** A call's number in its scope and a value of it. */
export type Point = [number, number];

/** The work panel's Stats (the user, 2026-11-25, mockups/v11.html): the calls of this turn or of the whole conversation and
 * what they add up to, from the view alone. This turn is replyOut's: the calls after the last finished turn while a reply
 * runs (or that no finished turn took), else the last finished turn's. Real numbers only: a call without a time to first
 * token, a speed or any input is left out of that series, never 0; a cost or a time not known is null. */
export function stats(v: View, scope: "turn" | "all") {
  const busy = !["Ready", "Done"].includes(v.activity), all = v.calls ?? [];
  const dones = v.blocks.flatMap((b, i) => (b.kind === "done" ? [{ i, b }] : []));
  const last = dones[dones.length - 1]?.i ?? -1, prev = dones[dones.length - 2]?.i ?? -1, after = all.filter((c) => c.block > last);
  const open = scope === "turn" && (busy || after.length > 0);
  const calls = scope === "all" ? all : open ? after : all.filter((c) => c.block > prev && c.block <= last);
  const turns: Done[] = scope === "all" ? dones.map((d) => d.b) : open ? [] : dones.slice(-1).map((d) => d.b);
  const sum = (f: (c: (typeof all)[number]) => number) => calls.reduce((a, c) => a + f(c), 0);
  const input = sum((c) => c.fresh + c.cached), cached = sum((c) => c.cached);
  const series = (f: (c: (typeof all)[number]) => number | null): Point[] => calls.flatMap((c, i) => { const x = f(c); return x == null ? [] : [[i + 1, x] as Point]; });
  const run = (b: Done) => (b.run_cost === undefined ? b.cost : b.run_cost);
  // a plan's own loop without a reference price: the plan, no money, never $0 at API prices (core, the user, 2026-11-25)
  const noRef = (b: Done) => b.plan === true && !!b.provider && !b.price_m;
  // the turn running now, by the receipt's rule: a sum only where every call brings its model (claude's models but Opus
  // carry none, their figure at Opus's prices) and it comes to something; "about" where one is at list prices
  const running = scope === "all" || open ? after : [], runSum = running.reduce((a, c) => a + (c.cost ?? 0), 0);
  const runCost = running.length && running.every((c) => c.model) && runSum > 0 ? runSum : null;
  // how it was paid, from the finished turns themselves (as the receipt has it); one still running says so when it ends, so
  // its money is "so far" (a paid reply after plan turns read "not billed": the review of 2026-11-25)
  const said = turns.length > 0 && turns.every((b) => b.model !== undefined && b.plan != null), plan = said ? turns.filter((b) => b.plan) : [];
  const planned = plan.reduce((a, b) => a + (run(b) ?? 0), 0), paid = turns.reduce((a, b) => a + (b.plan ? 0 : run(b) ?? 0), 0);
  const noMoney = said && turns.every(noRef) && runCost == null;
  // the turn running on a plan, each of its calls (core 0.1.230's `plan`), after finished turns all on a plan: at API prices
  // and not billed as they are, not "so far" (the review of 2026-11-26: "about $0.759 so far" counted up on Claude's plan)
  const planNow = running.length > 0 && running.every((c) => c.plan) && (!turns.length || (said && plan.length === turns.length));
  // this turn: what runs now, or its own when it has ended; the conversation: the core's total, or while a reply runs the
  // finished turns' and that reply's by the same rule
  const done = turns.reduce((a, b) => a + (run(b) ?? 0), 0);
  const cost = noMoney ? null : scope === "turn" ? (open ? runCost : run(turns[0])) ?? null
    : running.length ? (runCost == null ? null : done + runCost) : v.cost;
  const about = turns.some((b) => b.estimate) || running.some((c) => c.estimate);
  // a payee nobody named (a model on this machine, a gateway it cannot name): no word, as the receipt has none
  const payees = [...new Set(turns.map((b) => paidTo({ plan: b.plan, provider: b.provider ?? undefined, providers: b.providers })))];
  const how = noMoney ? "not billed" : cost == null || !said && !running.length ? undefined : planNow ? "at API prices, not\u00a0billed" : running.length ? "so far"
    : plan.length === turns.length ? "at API prices, not\u00a0billed" : !plan.length ? (payees.every(Boolean) ? (payees.length === 1 ? `paid to ${payees[0]}` : "paid") : undefined)
    : `${planned > 0 ? `${usd(planned)} ` : ""}not\u00a0billed + ${usd(paid)} paid`;
  // the finished turns' own time (the receipt line's), and the running one's calls so far
  const secs = turns.reduce((a, b) => a + b.secs, 0) + (scope === "all" || open ? after : []).reduce((a, c) => a + (c.total_ms ?? 0) / 1000, 0);
  // on a plan alone (every finished turn says so), and the plans' providers (none: the claude CLI's, "Claude plan")
  const planOnly = said && plan.length === turns.length, plans = [...new Set([...plan.map((b) => b.provider ?? null), ...(planNow ? running.map((c) => c.provider ?? null) : [])])];
  return { busy, open, calls: calls.length, input, cached, hit: input ? (100 * cached) / input : null, out: sum((c) => c.out), tps: writing(calls), cost, about, how, secs, noMoney, planOnly, planNow, plans, done,
    ttft: series((c) => (c.first_ms == null ? null : c.first_ms / 1000)), speed: series((c) => c.tps || null),
    hits: series((c) => (c.fresh + c.cached ? (100 * c.cached) / (c.fresh + c.cached) : null)) };
}

/** A message that went: a prompt the core has started, not one it still holds (`queued`, core 0.1.228: held while a reply
 * or its checks run, at the end of the view until it starts). Only these open a turn, name a conversation or count. */
export const went = (b: View["blocks"][number]): b is Extract<View["blocks"][number], { kind: "prompt" }> => b.kind === "prompt" && !b.queued;
/** How many messages went in a view. */
export const promptsIn = (v: View | null | undefined) => (v?.blocks ?? []).filter(went).length;

/** A running reply's clock: when it began and how long questions held it (the time a question waited for you is no work:
 * "Worked 14 s" for a 2 s turn after an approval, design review 14). Kept from every view of its conversation, on screen
 * or not; a new message or the reply's end (a Retry is a new reply) starts it again. The live count is now - start - paused. */
export type Clock = { turn: number; start: number; paused: number; at?: number };
export function clockOf(c: Clock | undefined, v: View, now: number): Clock | undefined {
  if (["Ready", "Done"].includes(v.activity)) return undefined;
  const turn = promptsIn(v), k: Clock = c?.turn === turn ? { ...c } : { turn, start: now, paused: 0 };
  if (v.asking && k.at == null) k.at = now;
  else if (!v.asking && k.at != null) { k.paused += now - k.at; k.at = undefined; }
  return k;
}

/** The prompts shown before the core refused them (the router's check, then the refusal) whose words are what went back
 * into the box: left out with their refusal, so a send again does not show it twice (`refused` as messages() has it). The
 * prompt is the nearest before the refusal past notices that are no error (a mode picked during the router's check). */
export function orphans(v: View | null | undefined, refused: ReadonlyMap<number, string>): Set<number> {
  const out = new Set<number>(), blocks = v?.blocks ?? [];
  for (const [k, back] of refused) {
    let i = k - 1;
    while (i >= 0 && ((blocks[i].kind === "notice" && !(blocks[i] as { error: boolean }).error) || (blocks[i].kind === "prompt" && (blocks[i] as { queued?: boolean }).queued))) i--;
    const p = blocks[i];
    if (back.trim() && p && went(p) && p.text.trim() === back.trim()) out.add(i);
  }
  return out;
}

/** The prompts messages() shows: the conversation's title, whether it is saved or empty, read from these. */
export const asked = (v: View | null | undefined, refused: ReadonlyMap<number, string> = new Map()) => {
  const gone = orphans(v, refused);
  return (v?.blocks ?? []).flatMap((b, k) => (went(b) && !gone.has(k) ? [b.text] : []));
};

/** `refused` (App's alone): the error notices of sends the core refused whose words went back into the box, each with
 * those words as the box sent them ("" not known); a side chat or a subagent keeps every error a row */
export function messages(v: View, refused: ReadonlyMap<number, string> = new Map()): Message[] {
  const out: Message[] = [];
  // the calls not yet given to a reply: each done block takes those that began before it
  const pending = calls(v);
  let agent: Extract<Message, { role: "agent" }> | null = null, ask = "";
  // the last error a reply met (an error notice, or a provider's retry): what a failed or stopped reply shows
  let lastError = "";
  // a message sent during a reply waits in the core's queue: shown after that reply ends, not above it
  let open = false;
  const queue: string[] = [];
  // the messages the core holds (`queued` prompts): never a turn of their own until it starts them, so no done takes them
  const held: string[] = [];
  const edits = new Map((v.edit_points ?? []).map((e) => [e.block, e]));
  let at = 0;
  const gone = orphans(v, refused);
  const prompt = (text: string, block = at) => { out.push({ role: "user", text, edit: edits.get(block) }); agent = null; ask = text; open = true; lastError = ""; };
  const current = () => (agent ??= (out.push({ role: "agent", prose: [], tools: [] }), out[out.length - 1] as Extract<Message, { role: "agent" }>));
  // a short word the model wrote before a later step is no answer: it goes into the fold, in its place among the steps, as a
  // quiet line ("I'll look at the folder first." showed under the finished fold as if said at the end, design review 12).
  // Longer prose (more than a paragraph of ~280 characters, or with a code block, table or list) stays in the answer: a
  // long answer and then a to-do step folded the whole answer away, and Copy took only the "Done." after it
  const step = (t: ToolStep) => {
    const a = current();
    delete a.retrying; delete a.retryAt;
    a.prose = a.prose.filter((text) => { const short = interim(text); if (short) a.tools!.push({ kind: "said", verb: "", target: "", secs: 0, ok: true, text }); return !short; });
    a.tools!.push(t);
  };
  const changeAt = new Map((v.changes ?? []).map((c) => [c.at_block, c]));
  for (const [k, b] of v.blocks.entries()) {
    at = k;
    if (b.kind === "prompt" && b.queued) held.push(b.text);
    else if (b.kind === "prompt") { if (gone.has(k)) { /* refused: its words are back in the box */ } else if (open) queue.push(b.text); else prompt(b.text); }
    // the router answered it as a chat (app server, 0.1.127's router): the reply carries the tag and the ask, for "send to agent"
    else if (b.kind === "notice" && b.text.startsWith("answered as a chat")) { Object.assign(current(), { routedChat: true, ask }); }
    // the agent answered in a chat's place (the chat replied [[agent]], or you sent it there)
    else if (b.kind === "notice" && b.text.startsWith("answered by the agent")) { current().handedOff = b.text.replace(/^answered by the agent:?\s*/, ""); }
    // a saved conversation shown but not continued (its model cannot run here now): said under it, in words (design
    // review 9: it opened empty under a red "/model to the model it used first")
    else if (b.kind === "notice" && !b.error && b.text.startsWith("This conversation ran on ")) { (current().info ??= []).push(b.text); }
    // a model switch that went on in a new session (core C5): a quiet divider, the conversation above it stays
    else if (b.kind === "notice" && !b.error && /^─+\s*continued on /.test(b.text)) out.push({ role: "notice", text: b.text.replace(/^[─\s]+|[─\s]+$/g, "") });
    else if (b.kind === "text") { if (b.text.trim()) { current().prose.push(b.text); delete current().retrying; delete current().retryAt; } }
    // ToolSearch is Claude Code loading a deferred tool before using it ("select:mcp__lean__bash"): plumbing, not a step
    // (it showed in every hand-off and added "used 1 other tool" to the summary; real window, 2026-11-22)
    else if (b.kind === "tool" && b.name === "ToolSearch") { /* nothing */ }
    else if (b.kind === "tool") {
      const t: ToolStep = { kind: "tool", id: b.id, name: b.name, verb: verb(b.name), target: target(b.input ?? {}, v.cwd), secs: b.secs ?? 0, ok: !b.error,
        running: b.running, stopped: b.stopped, declined: b.declined, input: b.input, result: b.declined ? undefined : b.result,
        waiting: !!(b.running && v.asking), cwd: v.cwd };
      step(t);
    }
    else if (b.kind === "thinking") {
      const tok = b.tokens >= 1000 ? `${(b.tokens / 1000).toFixed(1)}k` : `${b.tokens}`;
      step({ kind: "thinking", verb: b.live ? "Thinking" : "Thought", target: "", detail: b.tokens ? `~${tok} tokens` : undefined,
        secs: 0, ok: true, running: b.live, text: b.text });
    }
    else if (b.kind === "done") {
      // the reply's own cost and tokens (the totals so far were shown under every answer until desktop 0.1.80); a cost the
      // core says it cannot tell (null) stays unknown, only a core before 0.1.183 sends the totals alone
      const u = b.run_usage ?? b.usage, total = u.input + u.cache_write + u.cache_read + u.output, read = u.input + u.cache_write + u.cache_read;
      // a call belongs to the first done block from where it began (`block`: the blocks there were then): a call that
      // added no block began at the done's own place, and went under the next reply
      let n = 0;
      while (n < pending.length && pending[n].block <= k) n++;
      const mine = pending.splice(0, n).map(({ block: _, ...c }) => c), unknown = mine.filter((c) => c.cost == null).length;
      current().meta = { cost: b.run_cost === undefined ? b.cost : b.run_cost, tokens: total, in: read, cached: u.cache_read, out: u.output, cachedPct: read ? pct1(u.cache_read, read) : undefined, secs: b.secs,
        calls: mine.length ? mine : undefined,
        // how the turn was paid comes from the turn itself or not at all (the conversation's model of now said "not billed"
        // under answers a provider had billed, design review 11); a `plan` not sent is not known, not false
        billing: b.model === undefined ? undefined : { plan: b.plan ?? undefined, provider: b.provider ?? undefined, perM: b.price_m ?? undefined, providers: b.providers },
        subCalls: b.sub_calls, estimate: b.estimate, uncounted: b.uncounted, unknownCalls: unknown || undefined };
      // a reply that ended without its answer says why, where a provider's error before it said (until desktop 0.1.74 the
      // error was its raw JSON, as the answer's text, and a failed reply ended with nothing)
      if (!b.ok || b.stopped) {
        const f = current().failure = { text: lastError ? readableError(lastError) : b.stopped ? "" : "The reply ended without an answer.", raw: lastError, stopped: b.stopped, ask,
          retrying: b.stopped && !!current().retrying };
        // an answer that only repeats its error is said once, in the error card ("Not logged in · Please run /login" twice)
        current().prose = current().prose.filter((p) => !(f.text && [f.text, f.raw].some((e) => e.trim() === p.trim())));
      }
      // a plain chat's own answer can go to the agent too
      else if (v.profile === "chat" && current().handedOff == null) Object.assign(current(), { chat: true, ask });
      // a reply that answered and then took one more step (a last to-do list) has its last words as its answer, not none
      const a = current(), said = a.tools!.map((t) => t.kind).lastIndexOf("said");
      if (b.ok && !b.stopped && !a.prose.length && said >= 0) a.prose.push(a.tools!.splice(said, 1)[0].text!);
      delete current().retrying; delete current().retryAt; delete current().tries;
      const c = changeAt.get(k);
      if (c) {
        const change = current().change = { turn: c.turn, files: c.files, undone: c.undone, canUndo: c.can_undo, key: `${c.at_block}:${c.turn}:${JSON.stringify(c.files)}` };
        // its done steps know it too: an edit's lines are numbered from the turn's diff (components/Step.tsx), not a piece a
        // later step may have rewritten (one failed or declined changed nothing; a stopped one may have)
        const tools = current().tools ?? [], changed = (t: ToolStep) => t.kind === "tool" && !t.declined && (t.ok || !!t.stopped);
        tools.forEach((t, i) => {
          if (t.kind !== "tool" || !t.ok || t.declined || t.stopped) return;
          t.change = change;
          t.unsure = rewritten({ name: t.name, input: t.input }, tools.slice(i + 1).filter(changed).map((l) => ({ name: l.name, input: l.input })), v.cwd);
        });
      }
      agent = null;
      open = false;
      const next = queue.shift();
      if (next !== undefined) prompt(next);
    }
    // a call tried again (the core's retry notice): said while the reply runs, in its place; once the reply ends it goes
    // (past it, or into the error card). In a plain chat it showed only once the reply had failed, 55 s later
    else if (b.kind === "notice" && !b.error && /: \d+ (?:try|tries) failed/.test(b.text)) { lastError = b.text; Object.assign(current(), { retrying: b.text, retryAt: b.retry_at, tries: b.tries }); }
    else if (b.kind === "notice" && (b.error || /provider error|HTTP \d{3}/.test(b.text))) {
      // a send the core refused, its words back in the box (`refused`): said beside the box until a message goes
      // (`standing`), not a row; they piled up at the top with nothing of what was sent (design review 13). Its message,
      // shown before the refusal, is left out too (`orphans`). Every other error stays a row (a run that could not start
      // and gave nothing back, a command's reply)
      if (!refused.has(k)) {
        lastError = b.text;
        // an error the reply went on past (a tool server that failed to start) stays in sight, in words; a provider's retry
        // shows only if the reply then fails
        if (b.error) (current().notes ??= []).push(b.text);
      }
      // the message could not start (claude not found): its turn is over, so the next is not queued behind it
      if (b.ended) { open = false; const next = queue.shift(); if (next !== undefined) prompt(next); }
    }
  }
  for (const text of [...queue, ...held]) out.push({ role: "user", text, queued: true });
  return out;
}

/** The error shown beside the box: the last refused send's (`refused`, as messages() has it), until a message goes; a
 * change of mode or of model, or any other notice, leaves it. */
export function standing(v: View, refused: ReadonlyMap<number, string> = new Map()): string | undefined {
  const k = standingAt(v, refused);
  return k < 0 ? undefined : (v.blocks[k] as { text: string }).text;
}
/** Where that error is in the view (-1: none): App keeps the model it was refused on, and a change of model leaves it. */
export function standingAt(v: View, refused: ReadonlyMap<number, string> = new Map()): number {
  let open = false, waiting = 0, error = -1;
  for (const [k, b] of v.blocks.entries()) {
    if (went(b)) { if (open) waiting++; else { open = true; error = -1; } }
    else if (b.kind === "notice" && refused.has(k)) error = k;
    // the turns as messages() has them: a message waiting in the core's queue goes when one ends
    if (b.kind === "done" || (b.kind === "notice" && b.error && b.ended)) { if (waiting) { waiting--; error = -1; } else open = false; }
  }
  return error;
}

/** A check of the receipt's data: `npx tsx -e 'import("./src/lib/fromView.ts").then(m => m.demoView())'`. A field the core
 * does not send, or sends as null, is not known: never false, never $0. */
export function demoView() {
  const u = { input: 1000, cache_write: 0, cache_read: 0, output: 10 }, call = (block: number, cost: number | null) => ({ block, fresh: 1000, cached: 0, out: 10, cost, first_ms: null, tps: null, total_ms: null, model: "claude-sonnet-5-5" });
  const v = { blocks: [{ kind: "prompt", text: "a" }, { kind: "done", ok: false, secs: 3, turns: 1, cost: 0, usage: u, stopped: true, run_cost: null, run_usage: u, estimate: true, model: "claude-sonnet-5-5", provider: null, price_m: null },
    { kind: "prompt", text: "b" }, { kind: "done", ok: true, secs: 2, turns: 1, cost: 0.1, usage: u, stopped: false, run_cost: 0.1, run_usage: u, model: "glm-5.3", plan: false, provider: "SubRouter", price_m: [1, 0.1, 2], uncounted: 2 },
    { kind: "prompt", text: "c" }, { kind: "done", ok: true, secs: 1, turns: 1, cost: 0.2, usage: u, stopped: false }],
    calls: [call(1, null), call(3, 0.1)], model: "", effort: "", cwd: "", profile: "", mode: "", activity: "Done", cost: 0.1, usage: u, turns: 2, context: 0 } as View;
  const m = messages(v).flatMap((x) => (x.role === "agent" && x.meta ? [x.meta] : []));
  const ok = m.length === 3 && m[0].cost === null && m[0].estimate === true && m[0].billing?.plan === undefined && m[0].unknownCalls === 1 && m[0].calls?.length === 1
    && m[1].cost === 0.1 && m[1].billing?.plan === false && m[1].uncounted === 2 && m[1].calls?.[0].cost === 0.1 && m[1].cached === 0
    && m[2].cost === 0.2 && m[2].billing === undefined;
  if (!ok) throw new Error(`view: ${JSON.stringify(m)}`);
  // the cache's share to a tenth (the user, 2026-11-26): the receipt's line, the bar and the strip read the same figure
  const cu = { input: 2, cache_write: 0, cache_read: 998, output: 10 };
  const cdone = { kind: "done", ok: true, secs: 1, turns: 1, cost: 0, usage: cu, run_usage: cu, stopped: false };
  const cached = messages({ ...v, usage: cu, calls: [], blocks: [{ kind: "prompt", text: "a" }, cdone] } as View)
    .flatMap((x) => (x.role === "agent" && x.meta ? [x.meta.cachedPct] : []));
  if (cached.join() !== "99.8" || usage({ ...v, usage: cu } as View).cachedPct !== 99.8) throw new Error(`cachedPct: ${JSON.stringify(cached)}`);
  // a plain chat: the agent's answer in the chat's place says why, the chat's own offers "Send to agent"
  const done = { kind: "done", ok: true, secs: 1, turns: 1, cost: 0, usage: u, stopped: false } as const;
  const chat = messages({ ...v, profile: "chat", calls: [], blocks: [{ kind: "prompt", text: "rain tomorrow?" }, { kind: "notice", text: "answered by the agent: needs live data or this computer", error: false },
    { kind: "text", text: "Yes.", live: false }, done, { kind: "prompt", text: "a sci-fi novel?" }, { kind: "text", text: "Dune.", live: false }, done] } as View)
    .flatMap((x) => (x.role === "agent" ? [[x.handedOff, x.chat, x.ask]] : []));
  if (JSON.stringify(chat) !== JSON.stringify([["needs live data or this computer", undefined, undefined], [undefined, true, "a sci-fi novel?"]])) throw new Error(`chat: ${JSON.stringify(chat)}`);
  // design review 12: prose before a later step is in the fold, in order; an answer that repeats its error once; a switch's divider
  const bash = { kind: "tool", id: "1", name: "bash", input: { command: "ls" }, result: "", error: false, secs: 1, running: false } as const;
  const shaped = messages({ ...v, calls: [], blocks: [{ kind: "prompt", text: "a" }, { kind: "text", text: "I'll look first.", live: false }, bash, { kind: "text", text: "Done.", live: false }, done,
    { kind: "prompt", text: "c" }, { kind: "text", text: "All set.", live: false }, bash, done,
    { kind: "notice", text: "── continued on Mock Coder 7B ──", error: false }, { kind: "prompt", text: "b" }, { kind: "text", text: "Not logged in · Please run /login", live: false },
    { kind: "notice", text: "Not logged in · Please run /login", error: true }, { ...done, ok: false }] } as View)
    .map((x) => (x.role === "user" ? `user:${x.text}` : x.role === "notice" ? `notice:${x.text}` : `agent:${x.tools!.map((t) => t.kind).join("+")}|${x.prose.join("+")}|${x.failure?.text ?? ""}`));
  if (shaped.join(" / ") !== "user:a / agent:said+tool|Done.| / user:c / agent:tool|All set.| / notice:continued on Mock Coder 7B / user:b / agent:||Please run /login") throw new Error(`shapes: ${shaped.join(" / ")}`);
  // a step's title drops a cd into the project or a folder in it, and keeps one elsewhere (review of review 14)
  const cmd = (id: string, command: string) => ({ ...bash, id, input: { command } });
  const titles = messages({ ...v, cwd: "/p", calls: [], blocks: [{ kind: "prompt", text: "a" }, cmd("1", 'cd "/p" && ls'), cmd("2", "cd /p/src && make"), cmd("3", "cd /tmp && rm -rf build"), done] } as View)
    .flatMap((x) => (x.role === "agent" ? x.tools!.map((t) => t.target) : []));
  if (titles.join(" | ") !== "ls | make | cd /tmp && rm -rf build") throw new Error(`cd titles: ${titles.join(" | ")}`);
  const retry = { kind: "notice", text: "the provider is rate limiting this model: 2 tries failed, trying again (HTTP 429: busy)", error: false } as const;
  const live = messages({ ...v, calls: [], blocks: [{ kind: "prompt", text: "fail" }, retry] } as View)[1], ended = messages({ ...v, calls: [], blocks: [{ kind: "prompt", text: "fail" }, retry, { ...done, ok: false }] } as View)[1];
  if (live.role !== "agent" || !live.retrying || ended.role !== "agent" || ended.retrying || !ended.failure?.raw.includes("2 tries")) throw new Error("retry notice");
  // the core's countdown where it sends one; stopped during the tries: said so, the codes left for the raw reply
  const timed = messages({ ...v, calls: [], blocks: [{ kind: "prompt", text: "fail" }, { ...retry, retry_at: 5000, tries: 2 }] } as View)[1];
  const halted = messages({ ...v, calls: [], blocks: [{ kind: "prompt", text: "fail" }, { ...retry, text: `${retry.text.slice(0, -1)} (rate_limit_exceeded, rate_limit_error))` }, { ...done, ok: false, stopped: true }] } as View)[1];
  if (readableError("ssh: connect to host x port 22") !== "ssh: connect to host x port 22" || readableError("gpt-5 is not available") !== "gpt-5 is not available")
    throw new Error("readableError capitalised a name");
  if (timed.role !== "agent" || timed.retryAt !== 5000 || timed.tries !== 2 || halted.role !== "agent" || !halted.failure?.retrying
    || halted.failure.text !== "The provider is rate limiting this model: 2 tries failed, trying again (HTTP 429: busy)") throw new Error(`retry countdown, stop: ${JSON.stringify([timed, halted])}`);
  const answering = messages({ ...v, calls: [], blocks: [{ kind: "prompt", text: "x" }, retry, { kind: "text", text: "Here", live: true }] } as View)[1];
  if (answering.role !== "agent" || answering.retrying) throw new Error("retry notice once the text comes");
  const refused = { kind: "notice", text: "starting claude: program not found", error: true, ended: true } as const;
  const after = messages({ ...v, calls: [], blocks: [{ kind: "prompt", text: "a" }, refused, { kind: "prompt", text: "b" }, refused] } as View);
  if (after.some((m) => m.role === "user" && m.queued) || after.filter((m) => m.role === "user").length !== 2) throw new Error(`a refused message holds up the next: ${JSON.stringify(after)}`);
  // a send the core refused (claude not found; send_rejected named block 2): beside the box, no row, gone once a message
  // goes, not when a mode changes; without App's word (a side chat), a row. Shown before the refusal (the router's check),
  // its message goes too when it is what came back. A run that could not start and gave nothing back stays a row
  const lost = { kind: "notice", text: "starting claude: program not found", error: true } as const, was = [{ kind: "prompt", text: "a" }, done, lost];
  const mode = { kind: "notice", text: "mode: plan", error: false } as const, back = new Map([[2, "write a haiku"]]);
  const see = (blocks: unknown[], r?: Map<number, string>) => [messages({ ...v, calls: [], blocks } as View, r).map((m) => m.role[0]).join(""), standing({ ...v, blocks } as View, r) ?? ""].join();
  const between = [{ kind: "prompt", text: "a" }, done, { kind: "prompt", text: "write a haiku" }, mode, { ...lost, ended: true }];
  if (see(between, new Map([[4, "write a haiku"]])) !== `ua,${lost.text}`) throw new Error(`a refused message after a mode's notice: ${see(between, new Map([[4, "write a haiku"]]))}`);
  const seen = [see(was, new Map([[2, ""]])), see([...was, mode], new Map([[2, ""]])), see([...was, { kind: "prompt", text: "b" }], new Map([[2, ""]])), see(was),
    see([{ kind: "prompt", text: "a" }, done, { kind: "prompt", text: "write a haiku" }, { ...lost, ended: true }], new Map([[3, "write a haiku"]])),
    see([{ kind: "prompt", text: "a" }, { ...lost, ended: true }]), see([{ kind: "prompt", text: "a" }, done, lost], back)];
  const want = [`ua,${lost.text}`, `ua,${lost.text}`, "uau,", "uaa,", `ua,${lost.text}`, "ua,", `ua,${lost.text}`];
  if (seen.join(" / ") !== want.join(" / ")) throw new Error(`a refused message's error: ${seen.join(" / ")}`);
  // a message the core holds while the checks run (core 0.1.228, `queued`): drawn queued at the end, no title, no turn of its
  // own, and a refusal standing before it stays (it hid one and the checks' status sat under it)
  const heldB = { kind: "prompt", text: "b", queued: true } as const, ver = { kind: "verify", cmd: "npm test", secs: 0 } as const;
  const holding = { ...v, calls: [], activity: "Verifying", blocks: [{ kind: "prompt", text: "a" }, { kind: "text", text: "Done.", live: false }, done, heldB, ver] } as View;
  const drawn = messages(holding).map((m) => (m.role === "user" ? `${m.text}${m.queued ? "(queued)" : ""}` : m.role[0])).join(",");
  const refusedThen = { ...v, calls: [], blocks: [{ kind: "prompt", text: "a" }, done, lost, heldB] } as View;
  if (drawn !== "a,a,b(queued)" || asked(holding).join() !== "a" || promptsIn(holding) !== 1 || standingAt(refusedThen, new Map([[2, ""]])) !== 2
    || heldTurn({ ...holding, activity: "Done" }, { prompts: 1, at: 0 }, 500) !== 1500) throw new Error(`held prompts: ${drawn} | ${asked(holding)} | ${standingAt(refusedThen, new Map([[2, ""]]))}`);
  // the clock: a question at 10 s answered at 65 s, the conversation off screen meanwhile (views still come), counts 10 s
  const tool = { ...v, activity: "Tool", blocks: [{ kind: "prompt", text: "a" }] } as View, asking = { ...tool, asking: { tool: "Bash", what: "ls", note: "", once: false, queued: 0 } } as View;
  let clock = clockOf(undefined, tool, 0);
  clock = clockOf(clock, asking, 10_000); clock = clockOf(clock, asking, 30_000); clock = clockOf(clock, tool, 65_000);
  if (!clock || 70_000 - clock.start - clock.paused !== 15_000 || clockOf(clock, { ...tool, activity: "Done" }, 70_000) !== undefined
    || clockOf(clock, { ...tool, blocks: [...tool.blocks, { kind: "prompt", text: "b" }] } as View, 71_000)?.start !== 71_000) throw new Error(`clock: ${JSON.stringify(clock)}`);
  // a long answer, then a to-do step, then "Done.": the answer stays the answer
  const long = `${"The cache is keyed on the lock file. ".repeat(4)}\n\n- one\n- two`;
  const kept = messages({ ...v, calls: [], blocks: [{ kind: "prompt", text: "a" }, { kind: "text", text: long, live: false }, { ...bash, name: "TodoWrite" }, { kind: "text", text: "Done.", live: false }, done] } as View)[1];
  if (kept.role !== "agent" || kept.prose.join("|") !== `${long}|Done.` || kept.tools!.some((t) => t.kind === "said")) throw new Error(`long prose: ${JSON.stringify(kept)}`);
  // one conversation across sessions; the window's queue waits for the last message sent to be taken
  const sw = { ...v, session_id: null, earlier_sessions: ["a", "b"], blocks: [{ kind: "prompt", text: "x" }] } as View;
  if (sessionIds(sw, ["b", "c"]).join() !== "b,a,c" || sessionIds({ ...sw, session_id: "d" }).join() !== "d,b,a" || !unsaved(sw) || unsaved({ ...sw, session_id: "d" }) || unsaved({ ...v, session_id: null, blocks: [] }))
    throw new Error("session ids");
  const idle = { ...sw, activity: "Done" } as View, turns = [heldTurn(idle, undefined, 0), heldTurn(idle, { prompts: 1, at: 0 }, 500), heldTurn(idle, { prompts: 1, at: 0 }, 2500),
    heldTurn(idle, { prompts: 0, at: 0 }, 500), heldTurn({ ...idle, activity: "Tool" }, undefined, 0)];
  if (turns.join() !== "0,1500,0,0,-1") throw new Error(`held turns: ${turns}`);
  if (usage({ ...v, context: 5000, context_window: null }).contextPct !== 0 || usage({ ...v, context: 5000 }).contextPct !== 3) throw new Error("context window");
  // an approval shows the asked call's change only: the exact path, else the one near match, else none (never a guess)
  const ed = (id: string, p: string) => ({ kind: "tool", id, name: "Edit", input: { file_path: p }, error: false, secs: 0, running: true });
  const ask = (blocks: unknown[], what: string) => askedInput({ ...v, blocks, asking: { tool: "Edit", what, note: "", once: false, queued: 0 } } as unknown as View);
  if (ask([ed("1", "/p/a.ts"), ed("2", "/p/b.ts")], "/p/a.ts")?.file_path !== "/p/a.ts" || ask([ed("1", "/p/src/a.ts")], "src/a.ts")?.file_path !== "/p/src/a.ts"
    || ask([ed("1", "/p/x/a.ts"), ed("2", "/p/y/a.ts")], "a.ts") !== null || ask([ed("1", "/p/b.ts")], "/p/a.ts") !== null) throw new Error("view: askedInput");
  // a reply's tokens: the calls after the last finished turn, a message queued during it not starting again
  const running = { ...v, blocks: [...v.blocks, { kind: "prompt", text: "d" }], calls: [...v.calls!, call(7, null)] } as View;
  if (replyOut(v) !== 0 || replyOut(running) !== 10 || replyOut({ ...running, blocks: [...running.blocks, { kind: "prompt", text: "e" }], calls: [...running.calls!, call(9, null)] } as View) !== 20 || replyOut(null) !== 0) throw new Error("view: replyOut");
  // the Stats: this turn by replyOut's rule (the running one, else the last finished), a call's number kept where a series
  // leaves a call out, the writing speed over the calls' writing time, a plan's turn at API prices and not billed; the review
  // of 2026-11-25: a running turn's cost only where every call has its model and it comes to something (else not known,
  // never $0), "about" passed on, "so far" while a reply runs (a paid one after plan turns), no payee no word
  const sc = (block: number, fresh: number, cached: number, out: number, first_ms: number | null, tps: number | null, total_ms: number, cost: number, more: object = {}) =>
    ({ block, fresh, cached, out, cost, first_ms, tps, total_ms, model: "deepseek-v4.1-flash", ...more });
  const three = [sc(0, 1000, 0, 100, 2000, 50, 4000, 0.01), sc(1, 100, 900, 300, null, 100, 5000, 0.04), sc(2, 0, 2000, 200, 1000, 200, 3000, 0.002, { provider: "SubRouter" })];
  const vibe = { kind: "done", ok: true, secs: 4, turns: 1, cost: 0.05, usage: u, stopped: false, run_cost: 0.05, model: "deepseek-v4.1-flash", plan: true, provider: "Mistral Vibe", price_m: [0.3, 0.007, 1.2] };
  const sv = (activity: string, calls: unknown[], ...ds: object[]) => ({ ...v, activity, cost: 0.1, blocks: [...(ds.length ? ds : [vibe]).flatMap((d) => [{ kind: "prompt", text: "a" }, d]), { kind: "prompt", text: "b" }], calls } as View);
  const said = (s: ReturnType<typeof stats>) => JSON.stringify([s.calls, s.input, s.cached, s.hit, s.out, s.tps, s.cost, s.about, s.how, s.secs, s.ttft, s.speed.length, s.hits.length]);
  const two = three.slice(0, 2), free = { ...vibe, provider: "Z.ai Coding Plan", price_m: null, run_cost: 0, cost: 0 };
  const cases: [string, ReturnType<typeof stats>, unknown[]][] = [
    ["running", stats(sv("Tool", three), "turn"), [1, 2000, 2000, 100, 200, 200, 0.002, false, "so far", 3, [[1, 1]], 1, 1]],
    ["finished plan", stats(sv("Done", two), "turn"), [2, 2000, 900, 45, 400, 80, 0.05, false, "at API prices, not\u00a0billed", 4, [[1, 2]], 2, 2]],
    ["a paid reply running after a plan turn", stats(sv("Tool", three), "all"), [3, 4000, 2900, 72.5, 600, 100, 0.05 + 0.002, false, "so far", 7, [[1, 2], [3, 1]], 3, 3]],
    ["no reference price", stats(sv("Done", two, { ...vibe, price_m: null }), "turn"), [2, 2000, 900, 45, 400, 80, null, false, "not billed", 4, [[1, 2]], 2, 2]],
    ["paid", stats(sv("Done", two, { ...vibe, plan: false, provider: "SubRouter" }), "turn"), [2, 2000, 900, 45, 400, 80, 0.05, false, "paid to SubRouter", 4, [[1, 2]], 2, 2]],
    ["running, no reference price", stats(sv("Tool", [...two, sc(2, 0, 2000, 200, 1000, 200, 3000, 0)], free), "turn"), [1, 2000, 2000, 100, 200, 200, null, false, null, 3, [[1, 1]], 1, 1]],
    ["running, a call without its model", stats(sv("Tool", [...two, sc(2, 0, 2000, 200, 1000, 200, 3000, 0.3, { model: undefined, estimate: true })]), "turn"), [1, 2000, 2000, 100, 200, 200, null, true, null, 3, [[1, 1]], 1, 1]],
    ["running at list prices", stats(sv("Tool", [...two, sc(2, 0, 2000, 200, 1000, 200, 3000, 0.002, { estimate: true })]), "turn"), [1, 2000, 2000, 100, 200, 200, 0.002, true, "so far", 3, [[1, 1]], 1, 1]],
    ["paid to nobody named", stats(sv("Done", two, { ...vibe, plan: false, provider: null }), "turn"), [2, 2000, 900, 45, 400, 80, 0.05, false, null, 4, [[1, 2]], 2, 2]],
    ["a plan turn and a paid one", stats(sv("Done", [sc(0, 10, 0, 1, null, null, 100, 0.05), sc(2, 10, 0, 1, null, null, 100, 0.03)], vibe, { ...vibe, plan: false, provider: "SubRouter", run_cost: 0.03 }), "all"),
      [2, 20, 0, 0, 2, null, 0.1, false, "$0.05 not\u00a0billed + $0.03 paid", 8, [], 0, 2]],
  ];
  const off = cases.filter(([, s, w]) => said(s) !== JSON.stringify(w));
  if (off.length) throw new Error(`view: stats\n${off.map(([n, s, w]) => `${n}: ${said(s)}, wanted ${JSON.stringify(w)}`).join("\n")}`);
  if (median([3, 1, 2]) !== 2 || median([4, 1, 2, 3]) !== 2.5 || median([]) !== null || writing([{ out: 10, tps: null }]) !== null) throw new Error("view: median, writing");
  // the turn's card on its done edits alone (not failed, declined, stopped), and a piece a later step rewrote marked unsure
  const edit = (id: string, old_string: string, new_string: string, more = {}) => ({ kind: "tool", id, name: "Edit", input: { file_path: "/p/a.ts", old_string, new_string }, result: "ok", error: false, running: false, ...more });
  const ev = { ...v, cwd: "/p", calls: [], blocks: [{ kind: "prompt", text: "x" }, edit("a", "foo", "bar"), edit("b", "q", "r", { error: true }), edit("c", "q", "r", { declined: true }),
    edit("d", "q", "r", { stopped: true }), edit("e", "bar", "baz"), { kind: "done", ok: true, secs: 1, turns: 1, cost: 0, usage: u, stopped: false }],
    changes: [{ turn: 0, at_block: 6, undone: false, can_undo: true, files: [{ path: "a.ts", added: 1, removed: 1, status: "M" }] }] } as View;
  const marked = messages(ev).flatMap((x) => (x.role === "agent" ? x.tools ?? [] : [])).map((t) => [t.id, !!t.change, t.unsure ?? null]);
  if (JSON.stringify(marked) !== JSON.stringify([["a", true, [true]], ["b", false, null], ["c", false, null], ["d", false, null], ["e", true, [false]]])) throw new Error(`view: edits' turn ${JSON.stringify(marked)}`);
  console.log("view ok");
}

/** What the call being asked about was given, for the approval's diff: among the open steps of that tool, the one whose
 * file is the question's, else the oldest (the core asks the oldest open question first; two edits waiting showed the
 * second one's diff under the first one's file). */
export function askedInput(v: View | null): Record<string, unknown> | null | undefined {
  const name = (n: string) => n.replace(/^mcp__\w+?__/, ""), a = v?.asking;
  if (!a) return undefined;
  const open = v.blocks.flatMap((b) => (b.kind === "tool" && b.running && name(b.name) === name(a.tool) ? [b.input] : []));
  const file = (i: Record<string, unknown> | null) => [i?.file_path, i?.path, i?.notebook_path].find((p): p is string => typeof p === "string");
  // the change shown before Allow must be the asked call's own: the one open call whose path is the question's, never a
  // guess (an automated security review, 0.1.104: the newest, then the first open call, could show another file's diff)
  const same = open.filter((i) => file(i) === a.what);
  const near = open.filter((i) => { const f = file(i); return !!f && (a.what.endsWith(`/${f}`) || f.endsWith(`/${a.what}`)); });
  return same.length === 1 ? same[0] : same.length === 0 && near.length === 1 ? near[0] : null;
}

/** The files this conversation has changed so far, counted live (the work panel's tab), by the panel's own rule: the
 * finished turns' (`done`, from git when each ended) and those the reply now running has edited or written inside the
 * project; none of the running reply's where the folder is not a git repository (`git` false: the panel never lists them,
 * and "Changes 2" fell to 0 at the reply's end; so did a plan written to ~/.claude). */
export function changedCount(v: View | null, folder: string | null, done: string[], git = true): number {
  const files = new Set(done);
  const last = v ? v.blocks.map((b) => b.kind).lastIndexOf("done") : -1;
  for (const b of git && folder ? v?.blocks.slice(last + 1) ?? [] : []) {
    if (b.kind !== "tool" || b.running || b.error || b.declined || !/^(Edit|edit|Write|write|MultiEdit|multi_edit|NotebookEdit)$/.test(b.name.replace(/^mcp__\w+?__/, ""))) continue;
    const path = [b.input?.file_path, b.input?.path, b.input?.notebook_path].find((p): p is string => typeof p === "string");
    const rel = !path ? null : path.startsWith(`${folder}/`) ? path.slice(folder!.length + 1) : /^(\/|~|[A-Za-z]:[\\/]|\.\.\/)/.test(path) ? null : path.replace(/^\.\//, "");
    if (rel) files.add(rel);
  }
  return files.size;
}

/** Every id a conversation goes by, newest first: its session's, the core's earlier ones (a change of model and a Retry go
 * on in a new session, core C5), and those the window saw it by before (`seen`, newest first). */
export const sessionIds = (v: View | null, seen: string[] = []) =>
  [...new Set([v?.session_id, ...[...(v?.earlier_sessions ?? [])].reverse(), ...seen].filter((x): x is string => !!x))];

/** A conversation whose change of model lives only in its core: messages, and no session yet (its first turn on the new
 * model has not run). */
export const unsaved = (v: View | null, refused?: ReadonlyMap<number, string>) => !!v && !v.session_id && asked(v, refused).length > 0;

/** When the next message waiting in the window may go: -1 not now (no view, a reply running, a question open), else the ms
 * still to wait for the last one sent to be taken (one more message of yours in the view; a command that adds none counts
 * after 2 s), 0 now. */
export function heldTurn(v: View | null, last: { prompts: number; at: number } | undefined, now: number): number {
  if (!v || !["Ready", "Done"].includes(v.activity) || v.asking) return -1;
  const prompts = promptsIn(v), wait = last ? 2000 - (now - last.at) : 0;
  return last && prompts <= last.prompts && wait > 0 ? wait : 0;
}

/** The messages waiting behind the reply now running (sent during it; the core holds them until it ends). */
export const queued = (v: View) => messages(v).flatMap((m) => (m.role === "user" && m.queued ? [m.text] : []));

/** Tokens said short: "514k", "1.2M". */
export const short = (n: number) => (n >= 1e6 ? `${+(n / 1e6).toFixed(1)}M` : n >= 1000 ? `${Math.round(n / 1000)}k` : `${n}`);
/** The share of the model's window from which the context is drawn in the warning colour (the bar and the percent: the
 * composer's strip, Stats, the status bar), and Claude's Compact button shows. */
export const CONTEXT_WARN = 85;
/** Claude's Compact button shows (the strip's and the status bar's): claude takes /compact here, the context is from
 * CONTEXT_WARN of a known window, and no reply runs. */
export const compactable = (cache: View["cache"], u: Pick<Usage, "contextPct" | "contextMax">, busy: boolean) => !!cache?.compact && !!u.contextMax && u.contextPct >= CONTEXT_WARN && !busy;

export function usage(v: View): Usage {
  // a window the core does not know (null, core C9: a local model) has no share: "context 1% of 200k" was made up
  const win = v.context_window === null ? 0 : v.context_window ?? 200000;
  const k = short;
  const u = v.usage, read = u.input + u.cache_write + u.cache_read;
  return { contextPct: win ? Math.min(100, Math.round((100 * v.context) / win)) : 0, contextUsed: k(v.context), contextMax: win ? k(win) : "",
    plan5h: v.limits ? Math.round(v.limits[0]) : null, plan5hReset: resetIn(v.limits_reset), planWeek: v.limits ? Math.round(v.limits[1]) : null,
    threadCost: v.cost, threadTurns: v.turns, cachedPct: read ? pct1(u.cache_read, read) : 0, saved: 0, today: v.cost, todayTurns: v.turns, todaySeries: [], balance: v.balance };
}

/** "in 2 h 14 min" until the plan's 5-hour window resets; empty when unknown or past. */
export function resetIn(at?: number | null, now = Date.now() / 1000): string {
  if (!at || at <= now) return "";
  const m = Math.round((at - now) / 60), h = Math.floor(m / 60);
  return h ? `in ${h} h ${m % 60} min` : `in ${m} min`;
}

/** What the composer's numbers say the conversation cost: [the value, its words, the tooltip], or nothing to say. A plan's
 * turns at API prices and not billed (the plan named on hover), or "not billed" where it has no reference price; a free
 * model's nothing; while a reply runs the Stats' "so far", the finished turns' sum where its calls bring no price, so the
 * part never goes and comes back (the review of design review 14: a paid reply after plan turns read "not billed"). */
export function costSaid(s: ReturnType<typeof stats>, choice: ModelChoice | undefined, busy: boolean): [string, string, string] | null {
  const plan = s.plans.map(planName).join(", ") || planName(null), money = (n: number) => `${s.about ? "about " : ""}${usd(n)}`;
  if (s.noMoney) return ["not billed", `(${plan})`, `On your ${plan}: it has no reference price, not billed`];
  if (choice && isFree(choice) && !s.cost) return ["free", /^https?:\/\/(localhost|127\.|\[::1\])/.test(choice.url ?? "") ? "on this machine" : "", "Nothing paid"];
  if (busy) { const n = s.cost ?? s.done;
    return n && s.planNow ? [money(n), "at API prices, not billed", `On your ${plan}: what it would cost at API prices so far, not billed`] : [n ? money(n) : "–", "so far", "This conversation's cost so far"]; }
  if (s.cost && s.planOnly) return [money(s.cost), "at API prices, not billed", `On your ${plan}: what it would cost at API prices, not billed`];
  return s.cost ? [money(s.cost), "this chat", `This conversation's cost${s.how ? `, ${s.how}` : ""}`] : null;
}

/** A check of the composer's numbers' cost: `npx tsx -e 'import("./src/lib/fromView.ts").then(m => m.demoStrip())'`. */
export function demoStrip() {
  const done = (cost: number, o: object = {}) => ({ kind: "done", cost, run_cost: cost, secs: 5, ok: true, model: "claude-sonnet-5-5", plan: true, ...o });
  const call = (block: number, o: object = {}) => ({ block, fresh: 100, cached: 900, out: 50, first_ms: 800, tps: 40, total_ms: 2000, role: null, ...o });
  const view = (blocks: object[], calls: object[], activity = "Done", cost = 0) => ({ activity, blocks, calls, cost, context: 1000, usage: { input: 0, cache_write: 0, cache_read: 0, output: 0 } }) as unknown as View;
  const said = (v: View, busy: boolean, choice?: ModelChoice) => costSaid(stats(v, "all"), choice, busy)?.slice(0, 2).join(" ") ?? "none";
  const plan3 = [done(0.2), done(0.2), done(0.13)], p = { kind: "prompt", text: "go" };
  const cases: [string, string, string][] = [
    ["three Claude plan turns", said(view(plan3, [call(0), call(1), call(2)], "Done", 0.53), false), "$0.53 at API prices, not billed"],
    // the 2026-11-25 bug class: a paid reply after plan turns is "so far", never "not billed"
    ["then a paid reply running", said(view([...plan3, p], [call(0), call(1), call(2), call(4, { model: "deepseek-v4.1-flash", cost: 0.01 })], "Tool", 0.53), true), "$0.54 so far"],
    // calls with no model (Claude's Sonnet): no running sum, the finished turns' stays
    ["a reply running with no price yet", said(view([...plan3, p], [call(0), call(1), call(2), call(4)], "Tool", 0.53), true), "$0.53 so far"],
    ["the first reply running with no price", said(view([p], [call(1)], "Tool"), true), "– so far"],
    // the review of 2026-11-26: a reply running on the plan after plan turns is at API prices, not billed, as they are; after a
    // paid turn, or with a call not on the plan, "so far"
    ["a plan reply running after plan turns", said(view([...plan3, p], [call(0), call(1), call(2), call(4, { model: "claude-opus-5-5", cost: 0.01, plan: true })], "Writing", 0.53), true), "$0.54 at API prices, not billed"],
    ["the first plan reply running", said(view([p], [call(1, { model: "claude-opus-5-5", cost: 0.01, plan: true })], "Writing"), true), "$0.01 at API prices, not billed"],
    ["a plan reply running after a paid turn", said(view([done(0.02, { plan: false, provider: "DeepSeek" }), p], [call(0), call(2, { model: "claude-opus-5-5", cost: 0.01, plan: true })], "Writing", 0.02), true), "$0.03 so far"],
    ["a plan reply with a paid call", said(view([...plan3, p], [call(0), call(1), call(2), call(4, { model: "claude-opus-5-5", cost: 0.01, plan: true }), call(5, { model: "deepseek-v4.1-flash", cost: 0.01 })], "Tool", 0.53), true), "$0.55 so far"],
    ["a plan with no reference price", said(view([done(0, { provider: "Z.ai Coding Plan", model: "glm-5.3" })], [call(0)]), false), "not billed (Z.ai Coding Plan)"],
    ["a local model", said(view([done(0, { plan: false, provider: "Ollama" })], [call(0)]), false, { model: "q", provider: "Ollama", price: "free", why_not: null, url: "http://localhost:11434/v1" }), "free on this machine"],
    ["paid", said(view([done(0.02, { plan: false, provider: "DeepSeek" })], [call(0)], "Done", 0.02), false), "$0.02 this chat"],
  ];
  const bad = cases.filter(([, got, want]) => got !== want).map(([what, got, want]) => `${what}: ${got} (wanted ${want})`);
  if (bad.length) throw new Error(bad.join("\n"));
  console.log("strip ok");
}
