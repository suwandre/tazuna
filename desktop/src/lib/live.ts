// The conversation's numbers while a reply streams (Settings › Appearance › Live stats; desktop/DESIGN.md, Motion): the
// core's estimate of the model call in flight (`live`), counted between views at the stream's pace, read every animation
// frame by the few small components that show it and drawn when a digit they show moves; never the transcript.
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { LIVE, useLook } from "./chrome";
import { short, stats } from "./fromView";
import type { View } from "./tazuna";
import { usd, writing } from "./types";

export type Live = NonNullable<View["live"]>;
const reduce = typeof window === "undefined" ? null : window.matchMedia("(prefers-reduced-motion: reduce)");
export const still = () => !!reduce?.matches;

/** A count between the core's figures: from where it stands to the newest over the time the last step took (the stream's
 * pace: ~50 ms while text streams, a second between Claude's thinking estimates), so it counts rather than jumps, never past
 * the newest figure and never back. The first figure it sees it takes as it is (a call already under way). */
export class Count {
  from = 0; to = 0; t0 = 0; dur = 1; last = -1;
  push(v: number, now: number) {
    if (this.last < 0) { this.from = this.to = v; this.t0 = this.last = now; return; }
    if (v <= this.to) return;
    this.from = this.at(now); this.to = v;
    this.dur = Math.min(1000, Math.max(50, now - this.last)); this.t0 = this.last = now;
  }
  at(now: number) { return this.from + (this.to - this.from) * Math.min(1, Math.max(0, (now - this.t0) / this.dur)); }
}

// one count per call (by its start), shared by the strip, the Stats and the Working row, so they say the same
const counts = new Map<number, Count>();
/** The call's output tokens now: counted (or, under reduced motion, the core's figure as it is). */
export function outNow(l: Live, now = performance.now()) {
  let c = counts.get(l.started);
  if (!c) { if (counts.size > 8) counts.clear(); counts.set(l.started, (c = new Count())); }
  c.push(l.out, now);
  return still() ? l.out : c.at(now);
}

/** The core's rule for timing a call's speed (session.rs `stream`): 20 tokens or more, 5 pieces, half a second of writing,
 * and no faster than 1,000 t/s (a burst handed over at once). A live speed by another rule came and went at the call's end. */
export const timed = (out: number, deltas: number, writingMs: number) => out >= 20 && deltas >= 5 && writingMs >= 500 && out <= writingMs;

/** The view with its call in flight drawn now: the call's row in `calls` (the core's zeros until it ends) given the live
 * estimate (its input; its output so far, `out`, counted; its cost, the input's and the counted output at the call's price;
 * its time to first token running until the token comes; its speed by the core's rule), the context grown by the output so
 * far. A view with no call in flight is itself. */
export function liveView(v: View, out: number, wall = Date.now()): View {
  const l = v.live, calls = v.calls ?? [], k = l?.row ?? calls.length - 1, c = calls[k];
  if (!l || !c) return v;
  const w = l.first == null ? 0 : wall - l.first, rows = [...calls];
  rows[k] = { ...c, fresh: l.in - l.cached, cached: l.cached, out, cost: l.in_cost == null ? c.cost : l.in_cost + (out * (l.out_m ?? 0)) / 1e6,
    first_ms: (l.first ?? wall) - l.started, tps: timed(out, l.deltas ?? 0, w) ? out / (w / 1000) : null, total_ms: wall - l.started };
  return { ...v, calls: rows, context: l.in ? l.in + out : v.context,
    usage: { ...v.usage, input: v.usage.input + l.in - l.cached, cache_read: v.usage.cache_read + l.cached } };
}

/** The context between calls: the last call's input and its output, which the next call carries (the core's `context` is
 * the input alone: the strip eased down by a call's output at its end and jumped back at the next start). */
export function settled(v: View): View {
  // the last finished call's (with Live stats off, one in flight is still zeros)
  const c = (v.calls ?? []).filter((_, i) => i !== v.live?.row).pop();
  return c && v.context ? { ...v, context: v.context + c.out } : v;
}

/** Where a number following the estimate stands `ms` after following began from `from`: eased onto `to` over 450 ms (a
 * call's start moves the context and the cost by its input), then `to`. */
export const blend = (from: number, to: number, ms: number) => (ms >= 450 ? to : from + (to - from) * (1 - (1 - Math.max(0, ms) / 450) ** 3));

/** The last finished reply's average writing speed (`writing`, the receipt's and the Stats' rule), or, where it had none to
 * time (an 8-token answer), the last reply's before it that had one (`last` false); null before any. */
export function lastSpeed(v: View): { tps: number; last: boolean } | null {
  const d = v.blocks.flatMap((b, i) => (b.kind === "done" ? [i] : [])), calls = v.calls ?? [];
  for (let j = d.length - 1; j >= 0; j--) {
    const t = writing(calls.filter((c) => c.block > (d[j - 1] ?? -1) && c.block <= d[j]));
    if (t) return { tps: t, last: j === d.length - 1 };
  }
  return null;
}

/** The call in flight's row of a view drawn live (`liveView`). */
export const liveRow = (v: View) => (v.live ? v.calls?.[v.live.row ?? v.calls.length - 1] : undefined);

// one loop for every caller, a store React draws from in the same frame (before its paint, as one render); at most 60 a
// second (a 120 Hz screen draws numbers no better at twice the work)
const subs = new Set<() => void>();
let raf = 0, last = 0, tick = 0;
const frame = (t: number) => {
  raf = subs.size ? requestAnimationFrame(frame) : 0;
  if (t - last < 12) return;
  last = t; tick++;
  for (const f of subs) f();
};
const each = (f: () => void) => { subs.add(f); if (!raf) raf = requestAnimationFrame(frame); return () => { subs.delete(f); }; };
// a hidden window has no frames; its reduced-motion ticks stop too
const slow = (f: () => void) => { const t = setInterval(() => { if (document.hidden) return; tick++; f(); }, 250); return () => clearInterval(t); };
const none = () => () => {};
/** While `on`, reads `shown` every animation frame (four times a second under reduced motion; once a frame) and draws the
 * caller again when what it says changes: a frame that moves no digit costs no render. Stops with `on`; a hidden window
 * gets no frames. */
export function useFrame(on: boolean, shown: () => string) {
  const at = useRef<{ t: number; s: string } | null>(null);
  return useSyncExternalStore(on ? (still() ? slow : each) : none, () => {
    if (!on) return "";
    if (at.current?.t !== tick) at.current = { t: tick, s: shown() };
    return at.current.s;
  });
}

/** The view as the numbers draw it now: `liveView` while its call streams and Live stats is on (`streaming`; the caller is
 * on screen), drawn again in the frames where `shown` (what the caller says of a view) changes; else the view itself, whose
 * numbers move as each call ends. `on`: Live stats is on. */
export function useLive(v: View, shown: (live: View) => string) {
  const on = useLook(LIVE) !== "off", streaming = on && !!v.live && !!v.calls?.length;
  const now = () => liveView(v, Math.round(outNow(v.live!)));
  useFrame(streaming, () => shown(now()));
  return { view: streaming ? now() : settled(v), streaming, on };
}

type Cache = NonNullable<View["cache"]>;
const warmIn = (c: Cache) => { const ms = c.warm_until - Date.now(); return ms > 0 ? Math.floor(ms / 60_000) : -1; };
/** What `said` makes of the whole minutes Claude's cache stays warm (0: under one; -1: cold); null with no cache to tell
 * (the own loop, before claude's first call). Looked at once a second, drawn again only when what it says changes. */
function useCacheSaid<T>(c: View["cache"], said: (minutes: number) => T): T | null {
  const [, seen] = useState(() => (c ? said(warmIn(c)) : null));
  useEffect(() => { if (!c) return; const t = setInterval(() => seen(said(warmIn(c))), 1000); return () => clearInterval(t); }, [c?.warm_until]);
  return c ? said(warmIn(c)) : null;
}
/** The whole minutes Claude's cache stays warm (0: under one; -1: cold), drawn again as they change; null: no cache. */
export const useWarm = (c: View["cache"]) => useCacheSaid(c, (m) => m);
/** Whether Claude's cache is cold, drawn again only when that changes (the box's placeholder). */
export const useCold = (c: View["cache"]) => useCacheSaid(c, (m) => m < 0) === true;
/** The cache in a sentence (a tooltip): how long it stays warm, and what the next message re-reads once it is cold, with
 * ≈ dollars where known, "of plan usage" on the plan (as the terminal says it, main.rs rebuild_text), alone where an API
 * key, Bedrock or a gateway bills it. */
export function warmSaid(c: Cache, m: number) {
  const life = c.ttl_ms >= 3_600_000 ? "1-hour" : "5-minute";
  const re = c.rebuild ? `the next message re-reads ${short(c.rebuild)} tokens${c.rebuild_usd != null ? ` (≈ ${usd(c.rebuild_usd)}${c.plan === false ? "" : " of plan usage"})` : ""}` : "";
  if (m < 0) return `Claude's ${life} cache is cold: ${re || "the next message writes it again"}`;
  return `Claude's ${life} cache stays warm for ${m ? `${m} more minute${m === 1 ? "" : "s"}` : "less than a minute"}${re ? `; once cold, ${re}` : ""}`;
}

/** A check of the count: `npx tsx -e 'import("./src/lib/live.ts").then(m => m.demoLive())'`. Monotone, never past the
 * newest figure, settling on it; a reduced figure is held, not followed back; the live row and its end. */
export function demoLive() {
  let seed = 7;
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  for (let run = 0; run < 200; run++) {
    const c = new Count();
    let t = 0, real = 0, max = 0, shown = 0;
    for (let i = 0; i < 60; i++) {
      // a view now and then (10 ms to 1.5 s apart), its figure up by 0 to 40, sometimes down (an estimate's rounding)
      const gap = 10 + rnd() * 1500;
      real = Math.max(0, real + Math.round(rnd() * 44) - 4);
      c.push(real, t);
      max = Math.max(max, real);
      // frames between this view and the next
      for (let f = 0; f < 5; f++) {
        const at = c.at(t + (f * gap) / 5);
        if (at < shown - 1e-9) throw new Error(`live: went back ${shown} → ${at}`);
        if (at > max + 1e-9) throw new Error(`live: ahead of the newest figure ${max}: ${at}`);
        shown = at;
      }
      t += gap;
    }
    if (c.at(t + 1000) !== max) throw new Error(`live: settled at ${c.at(t + 1000)}, not ${max}`);
  }
  // the first figure as it is; a step over the time the last one took
  const c = new Count();
  c.push(500, 0);
  if (c.at(0) !== 500) throw new Error("live: a call under way counts from its figure");
  c.push(600, 100);
  if (c.at(150) !== 550 || c.at(200) !== 600) throw new Error(`live: step ${c.at(150)} ${c.at(200)}`);
  // the row: the call's zeros given the estimate, its cost from the counted output; no call in flight, the view itself
  const v = { activity: "Writing", blocks: [{ kind: "prompt", text: "go" }], cost: 0, context: 9000, usage: { input: 10, cache_write: 0, cache_read: 900, output: 5 },
    calls: [{ block: 1, fresh: 0, cached: 0, out: 0, cost: 0, first_ms: null, tps: null, total_ms: null, model: "m" }],
    live: { started: 1000, first: 1800, out: 100, deltas: 30, in: 10000, cached: 9000, in_cost: 0.002, out_m: 10, row: 0 } } as unknown as View;
  const r = liveView(v, 120, 2800), row = r.calls![0];
  const want = { fresh: 1000, cached: 9000, out: 120, cost: 0.002 + 0.0012, first_ms: 800, tps: 120, total_ms: 1800 };
  for (const [k, x] of Object.entries(want)) if (Math.abs(Number((row as Record<string, unknown>)[k]) - x) > 1e-12) throw new Error(`live row ${k}: ${(row as Record<string, unknown>)[k]}`);
  if (r.context !== 10120 || r.usage.cache_read !== 9900) throw new Error(`live context ${r.context}, cache ${r.usage.cache_read}`);
  const wait = liveView({ ...v, live: { ...v.live!, first: null } }, 0, 1200).calls![0];
  if (wait.first_ms !== 200 || wait.tps !== null) throw new Error("live: the time to first token runs until it comes");
  const none = { ...v, live: null } as View;
  if (liveView(none, 120) !== none) throw new Error("live: none in flight");
  // the speed by the core's rule: too few tokens, pieces or ms, or a burst, has none (it came and went at the end)
  if (!timed(20, 5, 500) || timed(19, 5, 500) || timed(20, 4, 500) || timed(20, 5, 499) || timed(900, 40, 600)) throw new Error("live: the speed's rule");
  if (liveView({ ...v, live: { ...v.live!, deltas: 3 } }, 120, 2800).calls![0].tps !== null) throw new Error("live: 3 pieces timed");
  // the in-flight point is no point of the 3 a sparkline needs: the core's view has it at none
  if (stats(v, "all").speed.length !== 0 || stats(r, "all").speed.length !== 1) throw new Error("live: the in-flight point");
  // the context does not drop at the call's end: input and output, during and after (the core's exact 120 out)
  const end = settled({ ...v, live: null, calls: [{ ...v.calls![0], fresh: 1000, cached: 9000, out: 120 }], context: 10000 } as View);
  if (end.context !== r.context) throw new Error(`live: the context went ${r.context} → ${end.context}`);
  // with Live stats off, the call in flight's zero row is no last call: the context stays the finished one's input and output
  const off = settled({ ...v, context: 10000, calls: [{ ...v.calls![0], out: 120 }, { ...v.calls![0], block: 2 }], live: { ...v.live!, row: 1 } } as View);
  if (off.context !== 10120) throw new Error(`live: off ${off.context}`);
  // following begins from where the number stood, eased onto the estimate, never a jump
  if (blend(0.5, 0.76, 0) !== 0.5 || blend(0.5, 0.76, 450) !== 0.76 || !(blend(0.5, 0.76, 100) > 0.5 && blend(0.5, 0.76, 100) < 0.76)) throw new Error("live: the blend");
  // the last reply's speed, or the last timed one's before an untimed reply
  const two = { blocks: [{ kind: "prompt" }, { kind: "done" }, { kind: "prompt" }, { kind: "done" }], calls: [{ block: 0, out: 400, tps: 200 }, { block: 2, out: 8, tps: null }] } as unknown as View;
  if (lastSpeed(two)?.tps !== 200 || lastSpeed(two)?.last !== false || lastSpeed({ ...two, calls: [] } as View) !== null) throw new Error(`live: the last speed ${JSON.stringify(lastSpeed(two))}`);
  console.log("live ok");
}
