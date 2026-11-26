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
    selected: "#e0e7e9"
    ink: "#23221e"
    ink-2: "#3f3c35"
    muted: "#5b574f"
    faint: "#69655d"
    line: "#e3ded3"
    line-soft: "#ece8df"
    line-strong: "#d8d2c5"
    accent: "#1d5f86"
    accent-ink: "#174b6b"
    accent-soft: "#e7eced"
    on-accent: "#ffffff"
    live: "#1d5f86"
    live-ink: "#174b6b"
    live-soft: "#e7eced"
    ok: "#347045"
    bad: "#aa4236"
    warn: "#8c5811"
    scrim: "#1d1b1640"
    desk: "#efebe3"
  dark:
    canvas: "#171613"
    main: "#211f1b"
    sidebar: "#151411"
    card: "#282622"
    field: "#302e29"
    hover: "#37342e"
    selected: "#303638"
    ink: "#ece8df"
    ink-2: "#d2cdc2"
    muted: "#afaa9e"
    faint: "#a29d95"
    line: "#35322c"
    line-soft: "#2b2925"
    line-strong: "#423e37"
    accent: "#7fb0d0"
    accent-ink: "#a3c8e0"
    accent-soft: "#2a2e2d"
    on-accent: "#0e1820"
    live: "#7fb0d0"
    live-ink: "#a3c8e0"
    live-soft: "#2a2e2d"
    ok: "#7cc08f"
    bad: "#e08b7f"
    warn: "#e0b46a"
    scrim: "#0000008c"
    desk: "#121110"
typography:
  ui: { family: "-apple-system, BlinkMacSystemFont, \"Segoe UI\"", fallback: "system-ui, sans-serif", size: 13.5px, line-height: 1.5 }
  mono: { family: "Geist Mono", fallback: "ui-monospace, monospace", size: 12.5px }
  prose: { family: "Source Serif 4", fallback: "Georgia, serif", size: 15.5px, line-height: 1.62 }
  label: { family: ui, size: 11.5px, weight: 500 }
  number: { family: ui, size: 22px, weight: 600, tracking: -0.02em }
spacing: { unit: 4px, scale: [2, 4, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32] }
radius: { sm: 6px, md: 7px, lg: 10px, xl: 14px, pill: 999px }
elevation:
  none: none
  composer: "0 1px 2px #2321190d, 0 4px 14px #23211908"
  sheet: { light: "0 0 0 1px #2321190a, 0 1px 2px #2321190f, 0 8px 28px #2321190d", dark: "0 0 0 1px #ffffff14, 0 1px 2px #00000080, 0 12px 34px #00000066" }
  pop: { light: "0 2px 6px #2321190f, 0 14px 36px #23211924", dark: "0 0 0 1px #ffffff0a, 0 10px 32px #00000080", dark-fill: "#2e2c27" }
layout: { sidebar: 248px, rail: 232px, reading: 700px, gutter: 28px }
motion: { fast: 120ms, base: 160ms, slow: 200ms, easing: "cubic-bezier(0.2, 0, 0, 1)" }
icons: { set: "Lucide", stroke: 1.5, size: 16px, small: 14px }
---

# tazuna desktop design

Warm paper: a calm, legible workspace for both coding and everyday questions. Chosen 2026-11-15 from five mockups
(`mockups/a.html`). Every UI change follows this file; agents writing UI code read it first.

## Character
- Paper and ink: a warm cream canvas, warm near-black text, one accent: Ai, true ai-iro (藍色; tazuna means "reins"),
  unless the user picks another (design review 13; ai-iro, not indigo, since design review 14, S5 of `mockups/v12.html`).
- Paper on the desk (design review 10, prototype `mockups/v7.html`): the window is the desk (`desk`); the conversation is one
  sheet on it (`main`, 12 px corners, the `sheet` shadow, 8 px from the window's edges), and so is a side chat or a subagent
  beside it. The sidebar, the work panel and the status bar lie on the desk with no border of their own; the work panel's
  parts are sections apart by a rule, not cards.
- The sheet is lifted in dark too (design review 11, `mockups/v8.html`): `main` #211f1b on the desk #121110 is 1.15:1 (it
  was 1.08:1, and the sheet read only in light); card, field, hover and the lines each a step up with it, and `faint` too,
  so it still reads 4.6:1 on a hovered row.
- One mark: 手 (ta, the hand of 手綱). Stroke Order separates it into a crown, two bars and a hook, paper on warm ink, centred on its tile
  (`mockups/logo-directions.html`, `app-icon.svg` and `components/Mark.tsx`). A running reply writes and holds the mark on its
  Working row and work fold (16 px, a CSS pixel per cell of its 16-cell grid, centred on the line: at 10 px it read as a grey
  smudge, design review 14); the sidebar pulses the complete mark (20 px) while any open thread is working.
  New chats, receipts and reduced motion use the still mark (`mockups/logo-animations.html`). The terminal draws its own
  glyph: `▐手▌` in the welcome and a colour pulse on 手 while working.
- Quiet structure: hairline borders, not shadows, for everything that sits on the page. What floats over it (a menu, a
  popover, a dialog, a tooltip; a toast or a card that opens by itself: `tz-float`) has the `pop` shadow, and in dark a fill one step lighter than a card: a menu over a card
  of the same fill read as part of it (design review 8, desktop 0.1.87). The composer keeps its own soft shadow.
- Reading first: the assistant's prose is set in a serif; code, paths, tool calls, costs and numbers in mono.
- Real numbers only: cost, tokens, cache and plan usage come from the session, never placeholders.
- Numbers said short where room is short (the receipt line, the status bar, the composer's numbers, Stats, the start
  page's ledger, a slip's sums): $0 for nothing, "<$0.001" under a tenth of a cent, four decimals under a cent, three under a
  dollar, then cents, zeros past the cents dropped (`usd`, lib/types.ts; "$0.000019" read as noise, design review 14). A
  slip's own cells and tooltips keep the exact figure (`usdExact`). Times under a tenth of a second are "<0.1 s", never
  "0.0 s" (`tenths`).

## Colour use
- `accent` (the user's choice, below) marks the one thing to act on and what is chosen: the send button, a switch that is
  on, the focus ring, the selected row (ringed in it, so never taken for a hovered one), selected text. `live`, a fixed
  ai-iro (Ai's own pair), marks the live state: the running thread and its dots and spinners, progress fills and meters,
  the plan pill (design review 13). Never decoration, never more than one accented control per area.
- The accent is the user's (Settings › Appearance, design review 13, D2): Ai (the default), Indigo, Jade, Amber, Rose or
  Graphite, each drawn for both themes in styles.css (`[data-accent=…]` on the root, set by lib/chrome.ts and kept like the
  theme; never the root's inline style, which is a plugin's `theme.set` and wins). `selected` and `accent-soft` follow it,
  the accent on the sheet at 12% and 9% (10% in dark; Ai's `selected` 16% in dark, which read grey at 12%), where every
  text level still reads 4.5:1 (faint on Ai's dark `selected` 4.56:1; on-accent text 5.2:1 at the least, Jade's light).
  Ai keeps the tokens above; nothing kept, or a name no longer offered, is Ai, and an accent picked by hand stays. What means a state keeps `live` whatever the accent (an amber accent read
  as a warning): also the usage charts, the Working shimmer, an info icon, code keywords. The mark stays paper on ink.
- `ok` and `bad` only for results (diff counts, passed or failed checks). `warn` only for limits and risk.
- Tables in answers: rules between rows, a quiet header (11.5 px, faint), numbers in tabular figures; no grid. A short
  cell ("+2 −1") stays on one line; code in a cell may break anywhere, so a long path does not crush the other columns.
- TeX in answers is drawn as MathML by the system (`lib/math.tsx`): `$…$` by pandoc's rule (the opening $ has no space
  after it, the closing one none before it and no digit after it: `$O(k)$` and `$k$` are math, "$5 per month, or $12"
  stays money), `\(…\)`, `$$…$$` (in a line too, read before `$…$`) and `\[…\]` (design review 12).
- Answers read as chat apps have them: a single newline in a paragraph or a quote is a line break (a paragraph is read
  whole, so **bold** or a link may run over it); `---` alone is a rule, and under a paragraph's line a heading (`===`
  too); code spans are read before math; an unclosed `$$` stays text;
  `- [x]` and `- [ ]` are task items with a drawn box (not a form's checkbox); `~~struck~~` is struck and muted.
- Code in answers has its own four tokens (`code-keyword` = live-ink, `code-string` ochre, `code-number` plum,
  `code-comment` = faint, italic), so syntax never borrows the result colours.
- Text levels: `ink` for content, `ink-2` for secondary content, `muted` for supporting text, `faint` for labels and
  metadata. No grey text on coloured backgrounds. Every text level reads at 4.5:1 or more on every background in both
  themes (WCAG AA; faint was 2.3-2.9:1 until desktop 0.1.34). Check with axe-core or review/audit.cjs (the sweep's in-page WCAG check) before changing a colour.
- Light and dark are designed together from the same tokens; neither is derived by inverting the other.

## Typography
- UI: the system's own face (`-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui`: San Francisco on a Mac; design
  review 14, S6 of `mockups/v12.html`) 13.5 px. Labels: 11.5 px medium, sentence case (no tracked capitals): a button, a
  menu item, a pill and a heading start with a capital ("Auto", "Show all"); a line that runs on after a rule or a dot
  does not ("answered as a chat · send to agent"), nor the status bar's quiet words for its figures ("this chat $0.051").
- Assistant prose: Source Serif 4, 15.5 px, line height 1.62, reading width 700 px.
- User messages: the UI face on a `field` block; code inline in Geist Mono.
- Mono for code: commands, file paths, diffs, branch names, tool targets. Numbers in the UI (costs, timings, counts, ages,
  percentages) are the UI face with tabular figures (`tabular-nums`), not mono: mono everywhere read as noise (review,
  2026-11-18).
- Fonts are bundled with the app (Source Serif 4, Geist Mono; no network fonts); the UI face is the system's.

## Layout
- Three columns: sidebar 248 px, main (header, transcript, composer), the work panel 300 px. The work panel never opens
  or closes by itself: opening at a turn's end moved the reading column under the reader, and Undo moved it back (design
  review 12). In a project it stays folded, a small tab on the sheet's right edge that counts the files changed so far,
  live while a reply edits ("Changes 2"; "Changes" alone at none, never "Changes 0"); the tab, ⌘\ and the header's button open it, for the conversation they are
  pressed in (one choice for the whole window put the panel into plain chats), and closing it brings the tab back. A
  plain chat has no tab. Open, it lies on the desk beside the sheet and the reading column is centred in what is left,
  gliding there with the panel's slide (its place and the sheet's width move together, the grid's 180 ms; under reduced
  motion it snaps), and back when it shuts: kept at its left edge (design review 12, B3 of `mockups/v9.html`) the text sat
  skewed right, about 340 px of sheet on its left and 40 on its right (design review 14). At 1440 the text keeps its 700 px, 92 px
  of sheet each side of it. Where the sheet beside the panel would be under 816 px (a window under 1372 px with the
  sidebar, any without it) the panel opens over the conversation instead, on a sheet of its own under its tab (the `pop`
  elevation; Esc, a click outside, the tab, its button or ⌘\ shut it; focus goes into it and back to what opened it) and
  nothing under it moves (design review 12, B2 of `mockups/v9.html`). A reply being followed is still followed while the
  column moves. Esc shuts the innermost thing first: a menu or dialog, then the panel over the
  conversation, then it is Stop (`escFor`). Where scrollbars always show, the text and the box keep the same edges. The
  rules are one function with its check (`panelState`, `demoPanel`, lib/types.ts; the column's CSS is `.tz-col`). (Design
  reviews of 2026-11-18, prototype `mockups/v2.html`, 11, 12 and 14.)
- The status bar names an account this conversation does not run on, when it needs you, quietly: a dot in its colour and faint
  words ("OpenRouter $0.17 below zero and 2 more"), the advice on hover. A balance its API does not tell is no news: the
  account's name alone.
- Each number shows once. The model and effort: the composer's one pill. The plan (5 hours and week, what is left), the
  SubRouter balance and the app's version: the status bar along the window's bottom (prototype `mockups/v4.html`, variant
  B, 2026-11-19), a click on the plan opening the usage page. This conversation's numbers: the composer's own strip along
  its lower edge, inside its border (design review 14, M-A of `mockups/v13.html`): 32 px, the box's full width, a ground of
  `field` at 40% on the card, 12.5 px, 14 px Lucide icons; its context against the model's window ("20.3k of 128k", a bar
  and "16%" where the window is known), its cost (the compact rule: "$0.0053 this chat"; on a plan "$0.521 at API prices ·
  not billed", the plan named on hover; "not billed (Z.ai Coding Plan)" where the plan has no reference price; "free on
  this machine" on a local model, "free" on another free one; while a reply runs the Stats' "$0.54 so far", the finished
  turns' sum where its calls bring no price yet, so the part never goes and comes back), the cache's share ("95% cached", none where the provider reports
  none) and, while a reply runs, its speed ("28 t/s", last, so nothing moves; the reply's "↓ tokens" stay on its Working
  row). They start at the box's text edge; a short row drops the cache, then the cost's words, then "of 128k", and never
  wraps. Each is a button (a Tab stop; with the strip above the box, Shift+Tab from the box reaches it): the cost opens the
  conversation's slip from it, the speed the work panel at Stats over this turn, the rest Stats over the conversation
  (nothing while a side chat or a subagent has the column). The Stats' and the status bar's own
  sums, none of its own; none on the start page. Settings › Appearance › Conversation numbers: under the box (the
  default), above it (the same strip along its top), in the status bar (context, cache and "this chat" there, as before
  design review 14), or off; with the strip the status bar has none of them. The work panel's Stats are the one exception
  the user asked for (2026-11-25): their sums and each call's numbers, live. An answer's own numbers: under it, on hover.
  The line under the composer only says what needs you: the plan running low (with the model to switch to), that Claude
  is signed out while the conversation runs on it ("Claude isn't signed in | Settings → Providers"), a model not restored.
  The route line that said where a message goes and what it costs ("→ DeepSeek V4.1 Flash on DeepSeek, paid, about $0.005
  a message") is hidden for now (design review 14: the user is unsure; `localStorage["tazuna.route-line"] = "on"` shows
  it).
- A new conversation starts on the model last picked by hand for its kind (a chat, a project) when that is not Claude; a
  Claude pick leaves it to the router's choice per task again (design review 12: new chats went back to Opus 5.5).
- Narrow windows: below 1280 px the work panel starts closed; below 1000 px the sidebar opens over the conversation from
  the header's button. The composer's pills never wrap.
- One live status per reply: its fold says "Working 12 s" (counted from when the reply began, less the time a question
  waited for you: the count stands while one asks; a finished turn says the core's own time), or "Waiting for you" during
  an approval, or "Waiting to retry" while a call is tried again (with "next try in 12 s" where the core says when); the
  line under the transcript shows only before the first step, directly under its reply and above any message queued behind
  it (under them it read as theirs, design review 14). Both have the mark at work before them,
  their words under a slow shimmer, and the reply's output so far once there is some ("↓ 1.2k tokens": the output of its
  model calls since the last finished turn, as each ends; a subagent's tokens are its whole context, not output, and stay
  on its own row; design review 13, D1).
- A message of yours sent while a reply runs (from the box, review comments, the work panel, the pull request) waits in
  the window, shown queued under it (as is one the core holds, sent in the moment between a reply and its checks: its
  `queued` prompt is no turn and no title until it starts), kept on disk until it goes, and goes when the reply ends, one at a time (the next
  when the reply before it has ended); Stop puts all that wait back in the box (sent on at once, they went to
  the agent the moment the reply was stopped).
- A call tried again says so while the reply runs, in its place ("The provider is rate limiting this model | 2 tries
  failed | next try in 12 s", "trying again" where the core sends no time), with two quiet text actions, Stop and Change
  model (it stops the reply, then the model picker opens); it goes once the reply's text comes, or becomes the error card
  if the reply fails ("Stopped while retrying" when you stopped it then; design review 14).
- A send the core refuses is a notice over the box in words, never its raw text: "Claude Code isn't installed." with
  Install page and Choose another model (the box's model picker); any other refusal in its own words, readable, with
  Choose another model where the model is the cause. It goes as soon as a message goes or the model changes, and stays
  gone back on that model.
- A change of model and a Retry go on in a new session of the core: the window keeps it one conversation (one row, still
  selected, a click on its row shows it rather than a second copy; its draft, queue and panel choice stay).
- The message box completes: `/` at its start lists the conversation's commands with what each does, `@` anywhere lists
  the project's files, in one menu over the box (arrows, Enter or Tab to take, Esc to shut); its placeholder says only that
  ("/ for commands, @ for files"; while a reply runs, how a message waits).
  Attachments are chips over the text, an image with its thumbnail. An answer that names an image file shows it under
  the answer (a click shows it whole); nothing loads from the network.
- Settings is one part at a time, chosen in a list on its left (Appearance first: theme, accent, text size, the type answers
  are set in, the Stats as tiles or rows, where the conversation's numbers stand; its rows have no line under them, the
  labels and controls say it: design review 14); a provider's key field shows when asked for. Claude is the first of your providers: signed in, or how to sign
  in (`claude auth login` with Copy, and Check again), or its install page; the first-run dialog ticks it only when it is
  signed in.
- Scrolled up from the end of a conversation, a round button over the message box goes back down.
- A new conversation before its first message is the start screen (prototype `mockups/v3.html`): the mark (40 px, so its pixels land on whole screen pixels), a serif
  question, the day's ledger from the run log ("Today 14 runs | $0.41 paid | 94% from the cache"; a click opens the usage
  page; a part with nothing to say is left out, and a day without a run shows the line that says what the box is for), the
  composer in the middle with a "where it runs" pill (a chat or a project), "Pick up where you left off" (the last three
  conversations) and, in a chat, "Start in a project", then the keys worth knowing. No $0.000 anywhere on it. A short
  window (720 px or less) takes less room around it and leaves the keys out; what does not fit scrolls under the
  transcript's fades. A bar under it (the trust bar) is in the flow, never over its content.
- The receipt (design review 11, variant A of `mockups/v8.html`): under every answer one faint line, always there
  ("$0.031 | 12.4k in (85% cached) | 640 out | 38 s", "<1 s" under a second; in a plain chat only under the last answer,
  older ones show theirs on hover or focus, as their actions do: design review 13, D3; a hidden one takes no room, it lies
  in the gap under its answer, so the space over the next message is the same with or without one: design review 14; an agent's run, and a chat's
  answer by the agent, keep every line; Copy beside it on hover or focus; a turn
  that gave no answer and cost nothing has none); a click unfolds the slip: a row
  per model call (its model and role, or what it called; in, cached with its share beside it, out, time to first token,
  speed while writing ("219 t/s", never "/s" or "tok/s"), time, and its price where the core fixed it when the call
  ended; "–" where the core sends none), the tools' own time, a totals row (the cache's share overall, the median time to
  first token, the speed over all the writing time), then the sums (on a plan, Claude's or a provider's, "Total at
  Mistral Vibe's API prices" and "On your Mistral Vibe plan: not billed", the plan alone where it has no reference price,
  never $0; or who was paid; and "The same without the cache"; a bare "Total" that only repeats the totals row is left
  out). An answer's slip is as wide as the reading column, its right edge under the change card's. A narrow slip drops the speed first, then the time to
  first token, and keeps the share (the user, 2026-11-25, `mockups/v11.html`). A slip is paper with a torn lower edge (`.tz-slip`), not a card. How a
  turn was paid comes from that turn alone (the core's `model`, `plan`, `provider`, `price_m` on its end): never from the
  model the conversation runs on now, which said "not billed" under answers a provider had billed. A turn that does not
  say has its total alone; a number the core does not send is a row or a column left out; rows that do not add up to
  the total say why in one line. The composer's cost, or the status bar's "this chat $0.051" where the numbers are put there, opens the conversation's own slip: its turns, a row
  for what is not a finished turn yet, a total that is the sum of its rows, the plan's part and each provider's on a line
  each, and the way on to the usage page.
- The transcript fades under the header and over the message box (`.tz-doc-edges`: two gradients in the sheet's colour
  over the scroller, clear of its scrollbar, never a mask on the scroller); its padding is as tall as the fades, so
  nothing is faded at its top or end, and a message jumped to starts under the fade. The way back down stands over the lower fade, which is deeper while it shows.
- The usage page speaks Settings' language: sections (Right now, Spending), each a card of rows; every account's windows
  as meters (amber from 75%); accounts with nothing used fold into one line ("4 windows at 0%"), and so do providers
  whose API tells no balance (their pages inside); a zero is a quiet line, not a big number; the daily bars stack the plan
  and paid parts; each plan is named by its provider ("Mistral Vibe plan"; "Claude plan" where the run names none), one with no reference
  price counted by its runs and "not billed", never $0. How the runs went (for tuning tazuna) is folded at the bottom, closed (design review 9, mockups/v6.html).
- The sidebar lifts out what needs you or works (Needs you, Working); every saved conversation sits under the day it last
  ran (Today, Yesterday, This week, Earlier), two lines each: its title; its project and when (today its age, yesterday its
  time, this week its day, then its date). One open with nothing to do stays under its day with an accent dot (× closes
  it); "Open" is only for one the lists do not hold. A list where every row has the same place drops it (one line, the
  time on the right). No model on the row. Projects last, folded. The list fades at both ends. A right click (or the menu
  key, Shift+F10) on a row opens its menu (PopMenu): a conversation's Rename and Remove from the list (the × button's
  question), and a project's folder in the file manager and Copy path; a project's row has New chat here, the folder and
  Copy path (design review 14).
- A project conversation's header has "Open in": the file manager, a terminal and the editors installed (VS Code, Cursor,
  Zed, Windsurf, under a rule; the shell's `open_in` command: an existing folder, never a macOS package, and an app of its
  list only, each by its absolute path with a PATH of absolute entries, never a shell; only the terminal starts in the
  folder). Its model
  picker ends with "Try on several models…" and the where-it-runs menu has "New conversation in a worktree" under the
  project: the palette's own actions where they are looked for (design review 14).
- The model picker shows the conversation's model first ("Current"), even when it cannot run, with its reason; a row that
  cannot run says why under its name (a key to add, a sign-in), and where its price would be "on your plan" the right says
  the reason in a few words instead ("not installed"); a priced row keeps its price there. Above the box it opens as tall
  as the room there and keeps that height, so its search field never moves (it grew from its bottom and the field jumped);
  the list scrolls inside, the fade and the room under the last row only while it scrolls; its last row, "Try on several
  models…", is one of the rows the arrows reach.
- The command palette ranks a label's matches above those found only in a hint or keyword: a label that starts with the
  word, then one with a word that does, then one that holds it; each group where its best row falls ("set" listed Auto,
  whose hint says "reset", above Settings: design review 14).
- A conversation's title is its first message made short by rules (no polite opening, its first sentence, about 60
  characters at most, cut at a word, a capital; where what the cut leaves is short, 24 characters, or 4 words in 32, the
  whole first sentence, 84 characters at most, with no "…" and no "— rest": design review 14); a name the user gave stays as given. The header shows the title, then in faint what the
  title left out of the first message after its words ("— just an image."), nothing where that cannot be said (never
  the whole message again).
- The review is a pane over the conversation, header and box included (Esc or "Conversation" goes back): the scopes and
  Unified or Split at its top, the changed files on its left (A, M, D), every file's diff one under another; the words
  that changed inside a line are marked; Stage and Revert per piece and per file (a file's revert takes a second
  click); a click on a line writes a comment, and the comments go to the agent as one message.
- The transcript is a document, not two-sided bubbles: no speaker labels (your message is the field block), a reply's
  steps folded into one line ("Worked 41 s · read 2 files, ran 3 commands"; open while it runs, a click unfolds it), prose,
  then a change card (file, +/-, Diff, Review, Undo; once undone it says "Undone" and its Diff buttons are off) and its metadata line (cost, tokens, cached, time) on hover. A short
  word the model wrote before a later step ("I'll look at the folder first.": one paragraph of about 280 characters or
  less, no code block, table or list) is a quiet line in the fold, in its place; longer prose stays in the answer (and in
  Copy), so an answer followed by a to-do step is not folded away. A declined step says its plain verb ("Edit main.rs | declined"). A change of
  model that goes on in a new session is a quiet divider ("continued on Mock Coder 7B"); the conversation above it stays.
- An approval asks in its title ("Allow a shell command?", "Allow this edit?", else "Allow <tool>?"). One of an edit or a
  write shows what would change (the step's own diff rendering, in a scroll of its own under the file's name); a command
  shows as itself.
- A message answered by the router as a chat carries an `answered as a chat · send to agent` tag. One the agent answered in
  a chat's place (the chat said it needs live data or this computer, or you sent it there)
  carries `answered by the agent · why` the same way; a plain chat's answers have Send to agent beside Copy.
- The work panel: Stats (this turn or the conversation: In, Out, Cost and Calls as tiles, a sparkline for time to first
  token, speed and cache hit rate, the context against the model's window; data marks in `live`; the user, 2026-11-25,
  `mockups/v11.html`; a sparkline of fewer than 3 points is left out, never an empty box with "–", and a zero is quiet text,
  not a big number. Or, by Settings › Appearance › Stats, rows on the desk with no boxes (design review 14, S4 of
  `mockups/v12.html`): the label left, the value right in tabular figures, rules between; a row with nothing to say left
  out, a zero quiet, a 2 px line at a row's right only from 8 calls, no live dot), Changes (the files this conversation changed, the branch, Review), Checks (tazuna's runs after a
  turn), Pull request (state, CI, Checks and reviews), your messages, then the user's own panels.

## Components we own
Segmented (a raised key on a `field` track; the `selected` fill is only a list's selected row; a radio group: one Tab stop,
the arrows, Home and End choose), `quietButton` (every bordered button that is not the accent one: Cancel, Later, Diff,
Review, Retry), `accentButton` and `dialogShell` (420, 600, 780 or 860 px wide, never past the window less 5 rem, the
`pop` shadow; its body is `main`, its groups are `card` with a hairline; the command palette alone keeps its own
geometry: 560 px near the window's top, at most 60% of its height), Swatches (the accent's: round, a radio group with
Segmented's keys, the chosen one ringed in ink), CheckRow (several that can be on together: a drawn 14 px box, the accent
when on, the model picker's row hover, Space toggles; never the browser's checkbox), PopMenu and `menuItem` (a menu at a
point: a row's right click, a header button's menu; `tz-pop`, arrows, Esc), all in `components/Controls.tsx`; the receipt (`components/Receipt.tsx`: Slip, AnswerSlip, ChatSlip), the start page's ledger
line, the work panel's tab; Mark (`components/Mark.tsx`); sidebar item (two lines, or one when its place repeats), open row, group heading, project group, thread dot (idle, running), badge; header crumb, cost and
panel button; message (user, agent, chat), work fold, tool step, change card, metadata line, routed tag; the conversation's
numbers (components/Stats.tsx `ConvNumbers`: the composer's strip, each part a button to Stats); composer with
pills (where, mode, model with its effort; one style: no fill, an icon, sentence-case words, a chevron where a menu opens,
a hairline in `line-strong` on hover and focus, the first one's icon where the box's text starts: design review 14, S2;
the mode pill tinted by the freedom it gives: full warn, with a 55% warn border, a 12% warn ground, a filled shield and
"Full · never asks" where the row has 520 px, plan `live`, the others plain, design review 13, D4) and send, their menus placed by
one rule (usePlace: above the whole box where it
fits, else below it); completion menu and attachment chip (Composer), answer images, jump to latest, tooltip (`.tz-tip`: every `title` shows
in it, lib/chrome.ts, only after the pointer really moves onto its element: a row that slides under a still pointer gets
none); answer copy; error card (the provider's message in words, with a capital, its error codes left to the raw reply, said once even when the answer repeats it; Retry runs
the last failed turn again without a second message, on the last reply only; its raw reply on request); code
block with its language and copy; work panel section (Stats, Changes, Checks, Pull request), sparkline, stat tile and
stat row (components/Stats.tsx: a sparkline is one 2 px line, its last point marked, a tip "call N · value" on hover or the
arrows; a row's line is 56 × 14 px with no mark), usage bar, status bar. Reuse these before adding new ones; add new ones here when they are made.

## Motion
- CSS transitions of 120-200 ms with the easing above; panels slide, toasts fade.
- Every dialog comes up over its backdrop (`tz-dialog`, the backdrop `tz-fade`); a menu or popover opens from its button
  (`tz-pop`). The backdrop is `scrim`: darker in both themes, never the ink colour (in dark mode that lightened the window).
- No animated backgrounds, particles or glow; streaming text is not animated per token. Two shimmers: the light sweep over
  a running step's words (`tz-live`), and the reply's live status, slower and to `live-ink` (`tz-shimmer`, 1.8 s; design
  review 13). The working mark draws a pixel every ~70 ms (a band going round where the webview lacks `linear()`).
- The Stats' numbers move to a call's new values (450 ms, easing out) while a reply runs, the newest point growing in
  from the one before; otherwise, and under reduced motion, they jump.
- Rows move, they do not jump: a sidebar row that changes group slides there (200 ms, kept by the conversation's id); a
  reply's steps unfold and fold in height (180 ms and 150 ms).
- What closes fades out where it was (`tz-out`, 110 ms; lib/chrome.ts does it for every menu and dialog), and the work
  panel's column slides open and shut, the reading column gliding to the centre of what is left with it (design review
  14); over the conversation it opens as a popover does (`tz-pop`).
- Selected text is the accent at 30%, not the system's blue.
- Honour `prefers-reduced-motion`: the working mark stands drawn whole and the shimmers are plain words (a spinner
  elsewhere still turns, slower).

## Desktop habits
- UI chrome is not selectable; the default cursor everywhere but text fields and links.
- Keyboard first: Cmd-K palette, Cmd-N new chat, Cmd-/ shortcut sheet, focus states always visible: the accent ring on
  buttons and links; a text field whose box is the visible container (the composer, the palette, a bordered field) shows
  its focus on that box's border instead (`outline-none` and `focus:border-accent` or `focus-within:border-accent`).
- One row at the window's top (design review 12, A2 of `mockups/v9.html`; it was three heights, ~40 px): on macOS the
  overlay title bar's traffic lights sit on it (tauri.conf.json `trafficLightPosition` {x: 20, y: 30}: centred on y 28,
  x 20 to 80 on macOS 26), then the brand (the sidebar's top row, 48 px: 8 px over the 40 px row) 12 px after them; the
  sheet keeps its 8 px inset and rounded top and its 40 px header is centred on the same line, as are a side chat's, a
  subagent's and the review's, and the work panel's first heading. In a narrow window the lights sit on the sheet's header,
  which starts after them with the sidebar's button (a dialog opening shuts that sidebar). The row is measured in the
  screen's points, not CSS px (`--tz-pt`, styles.css): the text sizes zoom the page and the lights do not. Windows and Linux have no lights (the Mac is told by the user agent
  inside Tauri, `macOverlay` in Sidebar.tsx): the brand sits 18 px in, on the same line. The sidebar's top row, the sheets'
  headers (their titles too: `data-tauri-drag-region="deep"`, which leaves the controls in them clickable) and the desk
  between the parts move the window. Native menus.
- A shortcut is written the Mac's way in the code and shown through `keysOf` (lib/types.ts): "Ctrl+K" on Windows and Linux.
- One clock (`clock`, lib/health.ts): "9:39pm". A tooltip repeats nothing the control already shows whole; a cut title's
  tooltip is the whole title (the first message), never the cut one.

## Do not
Inter or purple-blue gradients; glass, glow, neon; cards inside cards; an icon tile above every heading; stock shadcn
zinc with default radius; sparkle icons or emoji for "AI"; colour as decoration; two-sided chat bubbles; invented
numbers; a dark mode added at the end.
