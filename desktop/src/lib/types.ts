// What the window shows. Today filled from demo data; next from tazuna's app server (the same events the CLI renders).
export type Thread = { id: string; title: string; project?: string; running?: boolean };
export type Project = { name: string; open: boolean; pending?: number; threads: Thread[] };
export type ToolStep = { verb: string; target: string; detail?: string; secs: number; ok: boolean };
export type Change = { file: string; added: number; removed: number };
export type Meta = { cost: number; tokens: number; cachedPct?: number; secs: number };
export type Message =
  | { role: "user"; text: string }
  | { role: "agent"; prose: string[]; tools?: ToolStep[]; change?: Change; meta?: Meta; routedChat?: boolean };
export type Usage = { contextPct: number; contextUsed: string; contextMax: string; plan5h: number; plan5hReset: string; planWeek: number;
  threadCost: number; threadTurns: number; cachedPct: number; saved: number; today: number; todayTurns: number; todaySeries: number[] };
export type Session = { model: string; effort: string; mode: string; branch: string; changed: number; lastCost: number };
