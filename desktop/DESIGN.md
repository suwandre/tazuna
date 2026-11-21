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
    scrim: "#1d1b1640"
    desk: "#efebe3"
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
    scrim: "#0000008c"
    desk: "#121110"
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
  sheet: { light: "0 0 0 1px #2321190a, 0 1px 2px #2321190f, 0 8px 28px #2321190d", dark: "0 0 0 1px #ffffff0d, 0 1px 2px #00000066, 0 10px 30px #00000059" }
  pop: { light: "0 2px 6px #2321190f, 0 14px 36px #23211924", dark: "0 0 0 1px #ffffff0a, 0 10px 32px #00000080", dark-fill: "#282622" }
layout: { sidebar: 248px, rail: 232px, reading: 700px, gutter: 28px }
motion: { fast: 120ms, base: 160ms, slow: 200ms, easing: "cubic-bezier(0.2, 0, 0, 1)" }
icons: { set: "Lucide", stroke: 1.5, size: 16px, small: 14px }
---

# tazuna desktop design

Warm paper: a calm, legible workspace for both coding and everyday questions. Chosen 2026-11-15 from five mockups
(`mockups/a.html`). Every UI change follows this file; agents writing UI code read it first.

## Character
- Paper and ink: a warm cream canvas, warm near-black text, one indigo accent (ai-iro; tazuna means "reins").
- Paper on the desk (design review 10, prototype `mockups/v7.html`): the window is the desk (`desk`); the conversation is one
  sheet on it (`main`, 12 px corners, the `sheet` shadow, 8 px from the window's edges), and so is a side chat or a subagent
  beside it. The sidebar, the work panel and the status bar lie on the desk with no border of their own; the work panel's
  parts are sections apart by a rule, not cards.
- One mark: 手 (te, the hand of 手綱) drawn in the icons' round stroke, its last stroke ending in a rein's loop
  (`components/Mark.tsx`). The app icon (`app-icon.svg`, paper on indigo on macOS's grid), the sidebar (22 px tile) and the
  terminal's welcome (two cells, `▐手▌`) use it; nothing else stands for tazuna.
- Quiet structure: hairline borders, not shadows, for everything that sits on the page. What floats over it (a menu, a
  popover, a dialog, a tooltip; a toast or a card that opens by itself: `tz-float`) has the `pop` shadow, and in dark a fill one step lighter than a card: a menu over a card
  of the same fill read as part of it (design review 8, desktop 0.1.87). The composer keeps its own soft shadow.
- Reading first: the assistant's prose is set in a serif; code, paths, tool calls, costs and numbers in mono.
- Real numbers only: cost, tokens, cache and plan usage come from the session, never placeholders.

## Colour use
- `accent` marks the one thing to act on or the live state: the send button, the running thread, progress fills, the
  context ring. Never decoration, never more than one accented control per area.
- `ok` and `bad` only for results (diff counts, passed or failed checks). `warn` only for limits and risk.
- Tables in answers: rules between rows, a quiet header (11.5 px, faint), numbers in tabular figures; no grid.
- TeX in answers is drawn as MathML by the system (`lib/math.tsx`): `$…$` only when it holds a backslash, ^ or _ (money
  stays money), `\(…\)`, `$$…$$` and `\[…\]`.
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
- The status bar names an account this conversation does not run on, when it needs you, quietly: a dot in its colour and faint
  words ("OpenRouter $0.17 over +2"), the advice on hover.
- Each number shows once. The model and effort: the composer's one pill. The plan (5 hours and week, what is left), the
  SubRouter balance, this conversation's context, cache and cost, and the app's version: the status bar along the window's
  bottom (prototype `mockups/v4.html`, variant B, 2026-11-19), a click on the plan or the cost opening the usage page. An
  answer's own numbers: under it, on hover. The line under the composer only says what is new: a paid model's cost per
  message, or the plan running low.
- Narrow windows: below 1280 px the work panel starts closed; below 1000 px the sidebar opens over the conversation from
  the header's button. The composer's pills never wrap.
- One live status per reply: its fold says "Working 12 s" (counted from when the reply began), or "Waiting for you" during
  an approval; the line under the transcript shows only before the first step.
- The message box completes: `/` at its start lists the conversation's commands with what each does, `@` anywhere lists
  the project's files, in one menu over the box (arrows, Enter or Tab to take, Esc to shut); its placeholder says so.
  Attachments are chips over the text, an image with its thumbnail. An answer that names an image file shows it under
  the answer (a click shows it whole); nothing loads from the network.
- Settings is one part at a time, chosen in a list on its left (Appearance first: theme, text size, the type answers are
  set in); a provider's key field shows when asked for.
- Scrolled up from the end of a conversation, a round button over the message box goes back down.
- A new conversation before its first message is the start screen (prototype `mockups/v3.html`): a serif question, the
  composer in the middle with a "where it runs" pill (a chat or a project), "Pick up where you left off" (the last three
  conversations) and "Start in a project", then the keys worth knowing. No $0.000 anywhere on it.
- The usage page speaks Settings' language: sections (Right now, Spending), each a card of rows; every account's windows
  as meters (amber from 75%); accounts with nothing used fold into one line ("4 windows at 0%"), and so do providers
  whose API tells no balance (their pages inside); a zero is a quiet line, not a big number; the daily bars stack the plan
  and paid parts. How the runs went (for tuning tazuna) is folded at the bottom, closed (design review 9, mockups/v6.html).
- The sidebar lifts out what needs you or works (Needs you, Working); every saved conversation sits under the day it last
  ran (Today, Yesterday, This week, Earlier), two lines each: its title; its project and when (today its age, yesterday its
  time, this week its day, then its date). One open with nothing to do stays under its day with an accent dot (× closes
  it); "Open" is only for one the lists do not hold. A list where every row has the same place drops it (one line, the
  time on the right). No model on the row. Projects last, folded. The list fades at both ends.
- A conversation's title is its first message made short by rules (no polite opening, its first sentence, about 60
  characters at most, a capital); a name the user gave stays as given. The header shows the title, then in faint what the
  title left out of the first message.
- The review is a pane over the conversation, header and box included (Esc or "Conversation" goes back): the scopes and
  Unified or Split at its top, the changed files on its left (A, M, D), every file's diff one under another; the words
  that changed inside a line are marked; Stage and Revert per piece and per file (a file's revert takes a second
  click); a click on a line writes a comment, and the comments go to the agent as one message.
- The transcript is a document, not two-sided bubbles: no speaker labels (your message is the field block), a reply's
  steps folded into one line ("Worked 41 s · read 2 files, ran 3 commands"; open while it runs, a click unfolds it), prose,
  then a change card (file, +/-, Diff, Review, Undo) and its metadata line (cost, tokens, cached, time) on hover.
- A message answered by the router as a chat carries an `answered as a chat · send to agent` tag.
- The work panel: Changes (the files this conversation changed, the branch, Review), Checks (tazuna's runs after a turn),
  Pull request (state, CI, Checks and reviews), your messages, then the user's own panels.

## Components we own
Mark (`components/Mark.tsx`); sidebar item (two lines, or one when its place repeats), open row, group heading, project group, thread dot (idle, running), badge; header crumb, cost and
panel button; message (user, agent, chat), work fold, tool step, change card, metadata line, routed tag; composer with
pills (where, mode, model with its effort; one style: filled, an icon, a chevron where a menu opens; no mode in a plain
chat) and send, their menus placed by one rule (usePlace: above the whole box where it
fits, else below it); completion menu and attachment chip (Composer), answer images, jump to latest, tooltip (`.tz-tip`: every `title` shows
in it, lib/chrome.ts); answer copy; error card (the provider's message in words, Retry, its raw reply on request); code
block with its language and copy; work panel section (Changes, Checks, Pull request), usage bar, status bar. Reuse these before adding new ones; add new ones here when they are made.

## Motion
- CSS transitions of 120-200 ms with the easing above; panels slide, toasts fade.
- Every dialog comes up over its backdrop (`tz-dialog`, the backdrop `tz-fade`); a menu or popover opens from its button
  (`tz-pop`). The backdrop is `scrim`: darker in both themes, never the ink colour (in dark mode that lightened the window).
- No animated backgrounds, particles or glow; streaming text is not animated per token. The one shimmer: the light sweep over
  a running step's words (`tz-live`).
- Rows move, they do not jump: a sidebar row that changes group slides there (200 ms, kept by the conversation's id); a
  reply's steps unfold and fold in height (180 ms and 150 ms).
- What closes fades out where it was (`tz-out`, 110 ms; lib/chrome.ts does it for every menu and dialog), and the work
  panel's column slides open and shut.
- Selected text is the accent at 30%, not the system's blue.
- Honour `prefers-reduced-motion`.

## Desktop habits
- UI chrome is not selectable; the default cursor everywhere but text fields and links.
- Keyboard first: Cmd-K palette, Cmd-N new chat, Cmd-/ shortcut sheet, focus states always visible: the accent ring on
  buttons and links; a text field whose box is the visible container (the composer, the palette, a bordered field) shows
  its focus on that box's border instead (`outline-none` and `focus:border-accent` or `focus-within:border-accent`).
- macOS overlay title bar; native menus.
- A shortcut is written the Mac's way in the code and shown through `keysOf` (lib/types.ts): "Ctrl+K" on Windows and Linux.
- One clock (`clock`, lib/health.ts): "9:39pm". A tooltip repeats nothing the control already shows whole.

## Do not
Inter or purple-blue gradients; glass, glow, neon; cards inside cards; an icon tile above every heading; stock shadcn
zinc with default radius; sparkle icons or emoji for "AI"; colour as decoration; two-sided chat bubbles; invented
numbers; a dark mode added at the end.
