---
name: tazuna desktop
version: 0.1
direction: warm-paper
colors:
  light:
    canvas: "#f6f4ef"
    main: "#fbfaf7"
    sidebar: "#f3f0e9"
    card: "#fffefb"
    field: "#efebe3"
    hover: "#e7e3db"
    selected: "#e6e1f2"
    ink: "#23221e"
    ink-2: "#3f3c35"
    muted: "#5b574f"
    faint: "#69655d"
    line: "#e3ded3"
    line-soft: "#ece8df"
    line-strong: "#d8d2c5"
    accent: "#3d4aa8"
    accent-ink: "#2f3a8f"
    accent-soft: "#e7e8f6"
    on-accent: "#ffffff"
    ok: "#3b7d4f"
    bad: "#b4473a"
    warn: "#a76a14"
  dark:
    canvas: "#171613"
    main: "#1b1a17"
    sidebar: "#151411"
    card: "#211f1b"
    field: "#2a2823"
    hover: "#302e29"
    selected: "#2b2c45"
    ink: "#ece8df"
    ink-2: "#d2cdc2"
    muted: "#afaa9e"
    faint: "#9a958d"
    line: "#2f2c27"
    line-soft: "#25231f"
    line-strong: "#3a3731"
    accent: "#8f9bf0"
    accent-ink: "#aab3f5"
    accent-soft: "#272a44"
    on-accent: "#14152a"
    ok: "#7cc08f"
    bad: "#e08b7f"
    warn: "#e0b46a"
typography:
  ui: { family: "Geist", fallback: "system-ui, sans-serif", size: 13.5px, line-height: 1.5 }
  mono: { family: "Geist Mono", fallback: "ui-monospace, monospace", size: 12.5px }
  prose: { family: "Source Serif 4", fallback: "Georgia, serif", size: 15.5px, line-height: 1.62 }
  label: { family: "Geist", size: 11.5px, weight: 500 }
  number: { family: "Geist", size: 22px, weight: 600, tracking: -0.02em }
spacing: { unit: 4px, scale: [2, 4, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32] }
radius: { sm: 6px, md: 7px, lg: 10px, xl: 14px, pill: 999px }
elevation:
  none: none
  composer: "0 1px 2px #2321190d, 0 4px 14px #23211908"
layout: { sidebar: 248px, rail: 232px, reading: 700px, gutter: 28px }
motion: { fast: 120ms, base: 160ms, slow: 200ms, easing: "cubic-bezier(0.2, 0, 0, 1)" }
icons: { set: "Lucide", stroke: 1.5, size: 16px, small: 14px }
---

# tazuna desktop design

Warm paper: a calm, legible workspace for both coding and everyday questions. Chosen 2026-11-15 from five mockups
(`mockups/a.html`). Every UI change follows this file; agents writing UI code read it first.

## Character
- Paper and ink: a warm cream canvas, warm near-black text, one indigo accent (ai-iro; tazuna means "reins").
- Quiet structure: hairline borders, not shadows. The only shadow is the composer's.
- Reading first: the assistant's prose is set in a serif; code, paths, tool calls, costs and numbers in mono.
- Real numbers only: cost, tokens, cache and plan usage come from the session, never placeholders.

## Colour use
- `accent` marks the one thing to act on or the live state: the send button, the running thread, progress fills, the
  context ring. Never decoration, never more than one accented control per area.
- `ok` and `bad` only for results (diff counts, passed or failed checks). `warn` only for limits and risk.
- Code in answers has its own four tokens (`code-keyword` = accent-ink, `code-string` ochre, `code-number` plum,
  `code-comment` = faint, italic), so syntax never borrows the result colours.
- Text levels: `ink` for content, `ink-2` for secondary content, `muted` for supporting text, `faint` for labels and
  metadata. No grey text on coloured backgrounds. Every text level reads at 4.5:1 or more on every background in both
  themes (WCAG AA; faint was 2.3-2.9:1 until desktop 0.1.34). Check with axe-core before changing a colour.
- Light and dark are designed together from the same tokens; neither is derived by inverting the other.

## Typography
- UI: Geist 13.5 px. Labels: Geist 11.5 px medium, sentence case (no tracked capitals).
- Assistant prose: Source Serif 4, 15.5 px, line height 1.62, reading width 700 px.
- User messages: Geist on a `field` block; code inline in Geist Mono.
- Mono for code: commands, file paths, diffs, branch names, tool targets. Numbers in the UI (costs, timings, counts, ages,
  percentages) are Geist with tabular figures (`tabular-nums`), not mono: mono everywhere read as noise (review,
  2026-11-18).
- Fonts are bundled with the app (no network fonts).

## Layout
- Three columns: sidebar 248 px, main (header, transcript, composer), the work panel 300 px. The work panel is open in a
  project and closed in a plain chat; ⌘\ and the header's button switch it. (Design review, 2026-11-18, prototype
  `mockups/v2.html`.)
- Each number shows once. The model and effort: the composer's pills. The plan and context: the bar beside the send
  button, its card on hover (plan windows, context, cost, cache, balance), the usage page on a click. The thread's cost:
  the header. An answer's own numbers: under it, on hover. The line under the composer only says what is new: a paid
  model's cost per message, or the plan running low.
- Narrow windows: below 1280 px the work panel starts closed; below 1000 px the sidebar opens over the conversation from
  the header's button. The composer's pills never wrap.
- One live status per reply: its fold says "Working 12 s" (counted from when the reply began), or "Waiting for you" during
  an approval; the line under the transcript shows only before the first step.
- A new conversation before its first message is the start screen (prototype `mockups/v3.html`): a serif question, the
  composer in the middle with a "where it runs" pill (a chat or a project), "Pick up where you left off" (the last three
  conversations) and "Start in a project", then the keys worth knowing. No $0.000 anywhere on it.
- The usage page speaks Settings' language: sections (Right now, Spending, How your runs went), each a card of rows; every
  account's windows as meters (amber from 75%), credits with "Open its page" where no API tells the balance; a zero is a
  quiet line, not a big number; the daily bars stack the plan and paid parts.
- The sidebar groups the conversations open now by what they need (Needs you, Working, Open), then Recent: every saved
  conversation by when it last ran, two lines each (its name; its project and age). Projects last, folded.
- The transcript is a document, not two-sided bubbles: no speaker labels (your message is the field block), a reply's
  steps folded into one line ("Worked 41 s · read 2 files, ran 3 commands"; open while it runs, a click unfolds it), prose,
  then a change card (file, +/-, Diff, Review, Undo) and its metadata line (cost, tokens, cached, time) on hover.
- A message answered by the router as a chat carries an `answered as a chat · send to agent` tag.
- The work panel: Changes (the files this conversation changed, the branch, Review), Checks (tazuna's runs after a turn),
  Pull request (state, CI, Checks and reviews), your messages, then the user's own panels.

## Components we own
Sidebar item (two lines), open row, group heading, project group, thread dot (idle, running), badge; header crumb, cost and
panel button; message (user, agent, chat), work fold, tool step, change card, metadata line, routed tag; composer with
pills, plan bar and card, and send; work panel section (Changes, Checks, Pull request), usage bar. Reuse these before adding new ones; add new ones here when they are made.

## Motion
- CSS transitions of 120-200 ms with the easing above; panels slide, toasts fade.
- No animated backgrounds, particles, glow or shimmer; streaming text is not animated per token.
- Honour `prefers-reduced-motion`.

## Desktop habits
- UI chrome is not selectable; the default cursor everywhere but text fields and links.
- Keyboard first: Cmd-K palette, Cmd-N new chat, Cmd-/ shortcut sheet, focus states always visible.
- macOS overlay title bar; native menus.

## Do not
Inter or purple-blue gradients; glass, glow, neon; cards inside cards; an icon tile above every heading; stock shadcn
zinc with default radius; sparkle icons or emoji for "AI"; colour as decoration; two-sided chat bubbles; invented
numbers; a dark mode added at the end.
