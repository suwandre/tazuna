// What the window shows. Today filled from demo data; next from tazuna's app server (the same events the CLI renders).
export type Thread = { id: string; title: string; project?: string; running?: boolean };
export type Project = { name: string; path?: string; open: boolean; pending?: number; threads: Thread[] };
export type ToolStep = { verb: string; target: string; detail?: string; secs: number; ok: boolean; running?: boolean };
export type Change = { file: string; added: number; removed: number };
export type Meta = { cost: number; tokens: number; cachedPct?: number; secs: number };
export type Message =
  | { role: "user"; text: string }
  | { role: "agent"; prose: string[]; tools?: ToolStep[]; change?: Change; meta?: Meta; routedChat?: boolean; ask?: string };
export type Usage = { contextPct: number; contextUsed: string; contextMax: string; plan5h: number | null; plan5hReset: string; planWeek: number | null;
  threadCost: number; threadTurns: number; cachedPct: number; saved: number; today: number; todayTurns: number; todaySeries: number[] };
export type Session = { model: string; effort: string; mode: string; branch: string; changed: number; lastCost: number };

/** Dollars as tazuna shows them: three decimals, or two significant digits below a tenth of a cent ($0.00012), so a cheap
 * turn does not read as free. */
export const usd = (n: number) => (n > 0 && n < 0.001 ? `$${n.toPrecision(2)}` : `$${n.toFixed(3)}`);
