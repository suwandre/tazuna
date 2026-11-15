import type { Message, Project, Session, Thread, Usage } from "./types";

export const chats: Thread[] = [
  { id: "c1", title: "Names for a grey cat" },
  { id: "c2", title: "Train ticket refund (DE)" },
];

export const projects: Project[] = [
  { name: "api-gateway", open: true, pending: 1, threads: [
    { id: "t1", title: "Retry test fails on CI", project: "api-gateway", running: true },
    { id: "t2", title: "Speed up the build", project: "api-gateway" },
    { id: "t3", title: "Review PR #1468", project: "api-gateway" },
  ] },
  { name: "billing-ui", open: false, threads: [] },
  { name: "infra", open: false, threads: [] },
];

export const messages: Message[] = [
  { role: "user", text: "Why does `retry.test.ts` fail on CI but pass locally?" },
  { role: "agent",
    tools: [
      { verb: "Read", target: "src/client/retry.ts", secs: 0.8, ok: true },
      { verb: "Searched", target: "“maxDelay”", detail: "4 files", secs: 0.3, ok: true },
      { verb: "Ran", target: "pnpm test retry", detail: "1 failed", secs: 6.1, ok: true },
    ],
    prose: [
      "The test uses real timers. CI runners are slower, so the third retry fires after the test's 5 s timeout: the delay doubles from 1 s (1, 2, 4 s) and the cap of 30 s never kicks in.",
      "I switched the test to fake timers and advanced them explicitly. The code itself is fine.",
    ],
    change: { file: "src/client/retry.test.ts", added: 12, removed: 3 },
    meta: { cost: 0.042, tokens: 18400, cachedPct: 94, secs: 31 } },
  { role: "user", text: "thanks! btw how long should I boil an egg for a runny yolk?" },
  { role: "agent", routedChat: true,
    prose: ["About 6 minutes from boiling water for a large egg, then straight into cold water. 6½ minutes gives a jammy yolk; 9 minutes a firm one."],
    meta: { cost: 0.003, tokens: 900, secs: 4 } },
];

export const usage: Usage = { contextPct: 34, contextUsed: "68k", contextMax: "200k", plan5h: 38, plan5hReset: "18:40", planWeek: 21,
  threadCost: 0.045, threadTurns: 2, cachedPct: 94, saved: 0.31, today: 1.82, todayTurns: 37, todaySeries: [6, 10, 8, 17, 14, 23, 18, 21, 15, 26, 22] };

export const session: Session = { model: "Opus 5.5", effort: "high", mode: "auto", branch: "main", changed: 2, lastCost: 0.003 };
