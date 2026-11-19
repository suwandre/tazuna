// What the window shows. Today filled from demo data; next from tazuna's app server (the same events the CLI renders).
export type Thread = { id: string; title: string; project?: string; running?: boolean;
  /** where it ran (a project's name, or "chat") and when it last did (seconds since 1970): the row's second line */
  place?: string; at?: number;
  /** the model it ran on: its provider's mark beside the name */
  model?: string };
export type Project = { name: string; path?: string; open: boolean; pending?: number; threads: Thread[] };
/** One step before the answer: a tool call (with what it was given and what came back) or a stretch of thinking. */
export type ToolStep = {
  verb: string; target: string; detail?: string; secs: number; ok: boolean; running?: boolean;
  kind?: "tool" | "thinking"; name?: string; input?: Record<string, unknown> | null; result?: string; text?: string;
};
export type ChangedFile = { path: string; added: number | null; removed: number | null; status: string };
/** What one turn changed (the core's git snapshots before and after it). */
export type Change = { turn: number; files: ChangedFile[]; undone: boolean; canUndo: boolean };
/** One model call of a reply: what it called (tool verbs, or empty for the answer alone), its tokens, timing and cost. */
export type Call = { tools: string; fresh: number; cached: number; out: number; cost: number; firstMs: number | null; tps: number | null; totalMs: number | null };
export type Meta = { cost: number; tokens: number; cachedPct?: number; secs: number; calls?: Call[] };
export type Message =
  | { role: "user"; text: string; queued?: boolean; edit?: { cp: number; block: number } }
  | { role: "agent"; prose: string[]; tools?: ToolStep[]; change?: Change; meta?: Meta; routedChat?: boolean; ask?: string };
export type Usage = { contextPct: number; contextUsed: string; contextMax: string; plan5h: number | null; plan5hReset: string; planWeek: number | null;
  threadCost: number; threadTurns: number; cachedPct: number; saved: number; today: number; todayTurns: number; todaySeries: number[];
  /** SubRouter's balance in dollars, where a key is set */
  balance?: number | null };
export type Session = { model: string; effort: string; mode: string; branch: string; changed: number; lastCost: number };

/** Dollars as tazuna shows them: three decimals, or two significant digits below a tenth of a cent ($0.00012), so a cheap
 * turn does not read as free. */
export const usd = (n: number) => (n > 0 && n < 0.001 ? `$${n.toPrecision(2)}` : `$${n.toFixed(3)}`);
