# tazuna desktop: plan (2026-11-15)

What the app is for, what each version contains, how it should look, and what we take from the other harnesses.

Built so far (0.1.0, `desktop/`): the window in design A (DESIGN.md); `tazuna app-server` runs the core behind
it (JSON lines: the window's commands in, Session::view snapshots out); New chat (a plain chat in the home folder) and
Open folder (a project, recent ones in the sidebar); inside a project the router answers general questions as a chat,
tagged, with "send to agent"; a folder whose settings run code runs lean until the user trusts it; tool steps, markdown
answers, cost per turn and thread, context and plan usage; the model picker (the /model catalog by provider, searchable, with
what a locked row needs) and the effort menu; saved conversations in the sidebar (chats and each project's, the CLI's
/resume list: either side opens the other's); the permission mode under the composer (auto, ask, edits, plan; shift-tab)
and approvals inline (allow, always, decline; y / a / n); each turn's changes as a card (files, +/-, each file's diff) with
undo of the last turn (git snapshots before and after, src/turns.rs; the next message tells the model); Cmd-K palette
(chats, folders, conversations, model, effort, mode, undo, theme) and the Cmd-/ shortcut sheet. 0.1.1 carries its own tazuna (built
from the same commit; TAZUNA_BIN still overrides it) and CI builds it for macOS, Linux and Windows (.github/workflows/
desktop.yml, tags desktop-vX.Y.Z). 0.1.2: steps open to show what
a call was given and what came back (edits as removed and added lines), thinking rows, messages typed during a reply
queue behind it, attachments (paperclip and drop; images go as images), a macOS menu bar, and Settings (provider keys,
the data policy, the router). 0.1.3: markdown tables, links (to the browser,
http and https only), quotes and coloured code; long conversations send only what changed and draw their latest 40
messages; conversations renamed or removed from the lists; the plan window's reset time and SubRouter's balance; CI opens
the app once on macOS, Linux and Windows. 0.1 is complete.

0.2 so far: 0.1.4 keeps each open conversation in a core of its own (`tazuna app-server` per thread): one works while
another is on screen; "Open now" in the sidebar shows each with a dot (working, waiting for an answer, idle, stopped); at
most four idle ones stay open (the oldest close; they stay resumable). 0.1.5: a conversation out of sight that finishes or asks for an
answer says so with a system notification. 0.1.6: "edit from here" on your messages (claude path): the message comes back
to the box, the rest fades, and sending goes back to before it, with the files tazuna changed since if you leave that on. 0.1.7: Review (header, change cards, Cmd-K): uncommitted
changes, the branch against its base, or one turn, file by file and piece by piece; each piece staged or reverted; a click
on a line leaves a comment, and the comments go to the agent in one message. CI opens the app on all three systems on each run; on Linux it needed a D-Bus
session under Xvfb (a desktop has one), not a change to the app. 0.1.8: the notification plugin's own permission check is
allowed (its script was refused, so notifications could stay silent), and tazuna asks for the permission once. 0.1.15:
each conversation keeps its own draft in the box, through a restart; messages queued behind a reply when the window closed
come back in that conversation's box; "Your messages" in the rail lists what you sent and scrolls to it; a shortcut from
anywhere (Option-Space on macOS, Ctrl-Alt-Space elsewhere; Settings changes or clears it) brings the window up with a new chat. 0.1.16: a side chat (header, Cmd-K) beside a
Claude conversation: a fork of it on the same model (so the cache is the conversation's), in plan mode, in place of the
rail; nothing in it goes back, its fork stays out of the saved lists, and its questions take clicks only (y / n stay the
conversation's). 0.1.17: the core puts a side chat in plan mode itself, and the side box sends nothing until it is.
0.1.18: Usage (Cmd-Shift-U, the View menu, Cmd-K): API spend (billed) and the Claude plan at API prices (not billed), the
share of input the cache served, the plan now; cost per day; by model; and per version and model what the run log knows of
trouble in your own runs (stopped, errors, failed checks). Not yet: the cache's saving in dollars (worked back from run
costs the prices were too noisy to show) and real use's corrections and rewinds (bench/realuse.py reads recordings).
0.1.19 (with tazuna 0.1.141): own-loop models get "edit from here", side chats (on the conversation's own provider),
resume from the sidebar (moving to the model it ran on), and plan mode. 0.1.20: the app updates itself. It looks at
start and twice a day (and on Check for Updates in the tazuna menu or Cmd-K); a new version shows above Settings, and
two clicks install it and restart (the conversations' cores stop and stay resumable). Releases: `desktop/release.sh`
tags desktop-vX.Y.Z; CI builds the three systems, signs the update packages and writes latest.json onto the
desktop-latest release, which the apps read. The signature is tazuna's own updater key (free, not Apple's or a Windows
certificate): the private half is ~/.tauri/tazuna-updater.key on the Mac and the GitHub secret TAURI_SIGNING_PRIVATE_KEY,
the public half in tauri.conf.json. Losing it means one manual reinstall with a new key. Linux updates the AppImage
(.deb and .rpm through the package manager). 0.1.21 (with tazuna 0.1.142), routing that explains itself: a line under the
message box says where the next message goes and what it costs (Claude on your plan with the 5-hour window's use; a free
model; a paid one with about what a message cost on it in your own use, else its price per million tokens). When the
plan's 5 hours pass 90%, it offers the own-loop model you used most in the last 30 days. The first message to a paid
model asks once; paid spend stops at a cap per 24 hours ($1 by default, Settings), enforced in the core. 0.1.22 (with
tazuna 0.1.143): a conversation in a worktree of its own (Cmd-K "New conversation in a worktree"): branch
`tazuna/wt-...`, the folder under tazuna's data folder, `.env*` files and `.worktreeinclude` paths copied in, the
project's `.tazuna/setup.sh` run if you say so, ten ports in TAZUNA_PORT; it shows in the sidebar as "project · wt-...",
and Cmd-K removes it (its branch stays). The PR panel (header chip, Cmd-K): the branch's pull request through gh, its
checks and reviews; a failing check or the review comments go to the agent in one click, no PR yet asks tazuna to
open one, a merged worktree offers its removal. CheaperInference joins the providers in Settings. 0.1.25 (with tazuna
0.1.147): the usage page's real-use score, the same rules as bench/realuse.py (now in the core): per lane (Claude on the
plan, the own loop) and per version and model, how often your own runs were a clear miss (stopped, rewound, an error, a
change thrown away, the next message correcting it or asking again) or met pushback ("wait, ...", "progress?"). On the
last 90 days: Claude 4% clear misses over 350 runs, the own loop 5% over 40.
0.1.26-0.1.27 (with tazuna 0.1.150-0.1.151): the core's command menu, end-to-end speed and Cheaper Inference fixes, and model providers' keys kept out of the agent's commands.
0.1.28 (with tazuna 0.1.152-0.1.153), creator mode for behaviour: /create asks "part of the window" (a panel or plugin)
or "something tazuna does": a command, a skill or a tool (an MCP server), made in a new conversation, checked by the
core, turned on with Enable after you read it. They work on every model: claude gets them as a plugin (--plugin-dir),
the own loop and the lean profile through tazuna (commands expanded, skills listed in the prompt).
0.1.29-0.1.32 (with tazuna 0.1.154-0.1.157): /create follows /data, and a creation holding a link or a private key fails its check.
0.1.33 (with tazuna 0.1.158), from 1.1: under each reply, "N calls" opens its model calls, one row each: what it called,
new and cached input, output, time to the first token, writing speed, the whole call's time and its cost.
0.1.34, the accessibility pass: axe-core (WCAG 2.2 AA and best practice) finds nothing on the main window, the palette,
usage and settings, light and dark (before: faint text at 2.3-2.9:1 in 31-45 places a screen, unlabelled landmarks, no
heading). Panels are dialogs: focus goes in, Tab stays inside, Escape closes them from anywhere in them (the palette
only closed from its search box) and focus goes back where it was; one focus ring everywhere; diff lines take a comment
from the keyboard; the reply's progress is announced. Reduced motion was honoured already.
0.1.35 (with tazuna 0.1.159): Settings lists the MCP servers for every model (add a URL or a command, check, log in,
remove, on or off for the conversation) and takes the folder's hook: a command after each turn that changed files, at
no token cost (the terminal's /mcp and /hook).
0.1.36, best-of-N (from 1.1): "Try on several models…" (Cmd-K, in a project) takes a task and two to four models; each
lane gets a worktree of its own (no setup script), its model, auto mode and the task. "Compare the lanes" shows each
one's state, cost (the cheapest finished one marked), time, files and lines changed and its checks; Open goes to it,
Keep removes the other lanes' worktrees after a yes (their branches stay). Lanes last while the window is open. A
dialog now also takes back focus that a conversation opening behind it took.
0.1.37 (with tazuna 0.1.160), the first run: "Set up tazuna" opens once and says what is set and what is left, each with
its own way there: Claude on the subscription (the claude CLI found, with its version, or how to install it), other
models' keys, who may see the code, the quick-chat key. "Choose later" closes it; Cmd-K "Set up tazuna" brings it back.
0.1.38 (with tazuna 0.1.161), from 1.1: review mode in the mode menu: a reviewer model answers command requests and
what it allows runs in a sandbox (tazuna 0.1.161's notes); the question card says while the reviewer checks it.
0.1.39 (with tazuna 0.1.162): sharing and the gallery (docs/PLUGINS.md): Share saves a panel or plugin as one file, Add
from a file and Browse the gallery bring one in. A plugin from either stays off until Read and enable, which now shows
what it would register and its code first (Settings' Enable turned a plugin on without showing its code before).
0.1.40 (with tazuna 0.1.163): the review sandbox tightened (tazuna 0.1.163's notes); a plugin from a shared file or the
gallery is marked as such and warns harder before Enable, and the trial run's list is shown as what it did there, not
a promise: the code is what counts.
0.1.41-0.1.42 (with tazuna 0.1.164-0.1.165): the review sandbox closes .git, the keychain and every system service but the
user-name lookup, and is macOS only for now.
0.1.51: the message box grows upwards with what is typed (it stayed one line and earlier lines went out of sight), up
to 40% of the window, then scrolls; its card's border shows the focus.
0.1.52: what is still going on moves, as in the terminal: a running step spins and its time ticks between the core's
views (a long command's time stood still), live thinking pulses and its words shimmer, and under the reply a spinner,
what it is doing with counting dots and how long it has run. Step times no longer wrap. Reduced motion stops all of it.
0.1.53: an opened step reads as words, not JSON: a read is its file and lines, other calls' inputs labelled lines; a
skill's, subagent's or page's answer is drawn as markdown, its JSON as labelled lines (a skill's findings came as raw
```json); commands' and files' output stays as it is. A failed step shows a cross, not a red check; a skill's name is
on its line.
0.1.54: every button answers the pointer: a hand, a hover background, a press (the hover colour was the pills' own
background in dark mode, the cursor stayed an arrow, half the buttons had no hover style); the accent buttons darken.
Hover colours keep every text level at 4.5:1 or more (light #e7e3db, dark #302e29).
0.1.55: a button with a border takes an accent-tinted one on hover (the lighter background alone barely showed in dark
mode); the composer's pills answer the pointer only when they can be used, and look dimmed when they cannot (during a
reply, or a model with one effort). The app looks for an update hourly, not every 12 hours.
0.1.56: Settings ends with the app's version and its core's ("tazuna desktop 0.1.56 · its core 0.1.173"); "tazuna 0.1.173"
alone read as the terminal's.
0.1.57: another conversation comes in with a short fade up, its title too, so a switch is seen; a project's chevron folds
and unfolds its conversations (it only ever opened: open meant "the active project"), its name opens it; the sidebar's
rows act as buttons for the keyboard (Tab, Enter) and the pointer.
0.1.58: Settings ends with Updates: the app's version and its core's, Check for updates (checking, up to date, or the
new version with Install and restart).
0.1.59: snackbars (lib/toast.ts): a check for updates, from Settings, Cmd-K or the menu, ends in one at the bottom: up to
date, the new version with Install and restart, or why it could not check; installing says so too.
0.1.60: the snackbar alone says it: the sidebar's update row and Settings' line are gone; a new version found by the
hourly check shows once, and stays until installed or closed.
0.1.61: the usage page opens with "Your accounts now": Claude's plan from claude's own /usage (no model call), the
providers' credits with a link to each page, and OmniRoute's accounts with their quota windows. A click on the rail's plan,
cost or balance card opens it.
0.1.62: the sidebar's Settings row hovers like the rows above it: the fill sits evenly around its line (it had no padding
below), with the divider above it outside the fill.
0.1.63: Settings in sections: each a heading, a line on what it is for, and a card of rows (a setting and what it does
on the left, its control on the right), with switches for on and off and a two-way choice for who may see the code. About
shows the version as a tag. Check for updates answers in the middle over a dimmed window (up to date, a new version with
Install and restart, or why it could not check): Esc, × or a click outside closes it and Settings stays; a snackbar at
the bottom had covered Settings' own text. A new version found by the hourly check still shows as a snackbar.
0.1.64: the main screen after the design review (2026-11-18, prototype mockups/v2.html): each number once (the model in the
composer, the plan and context in a bar beside send with its card on hover, the thread's cost in the header, an answer's
numbers on hover); no speaker labels; a reply's steps folded into one line; the right column is the work panel (changes,
checks, pull request, your messages), open in a project, ⌘\ to switch; the sidebar grouped by what each conversation
needs (Needs you, Working, Open), then Recent in two-line rows with project and age.
0.1.65: an update's restart no longer waits on a busy conversation: each core gets 3 seconds to quit, all at once, then is
ended (a core in a model call or a test run read the quit only when it finished, so "Installing… tazuna restarts by
itself" could stay for minutes). Quitting the app waits the same way.
0.1.66: an update shows its download at the bottom until the restart ("Downloading 0.1.66 · 3.0 of 6.0 MB", a bar, then
"Installing… tazuna restarts by itself"), with a 5-minute limit: GitHub's release storage served the 6 MB at 80-200 KB/s,
and "Installing…" alone read as stuck for over a minute. The sidebar's rows keep their height in a long list (it
scrolls; 0.1.64 squeezed them until they overlapped).
0.1.67: the second design review's fixes: below 1280 px the work panel starts closed and below 1000 px the sidebar opens
from the header; the composer's pills never wrap and stay readable during a reply; one live status per reply ("Working
12 s", or "Waiting for you" during an approval), its time kept across a switch; no empty "New conversation" rows; no
$0.000 in a new chat's header; numbers in tabular Geist, mono only for code; no "−0"; the side chat a header icon like the
panel's; "Ask tazuna to open one" for a branch without a pull request.
0.1.68: the two screens of the second review's prototype (mockups/v3.html). A new conversation opens on a start screen:
"What are we working on?", the composer in the middle with a pill for where it runs (a chat, or a project: its own new
conversation there), the last three conversations to pick up, the projects to start in. The usage page is in Settings'
cards: Right now (every account's meters and credits), Spending (three figures with quiet zeros, the daily bars with
plan and paid parts, the models), How your runs went.
0.1.69: the third review's polish: no square focus ring inside the palette's, the model picker's or the side chat's
rounded boxes (the ring is in the base layer now, so a field whose box shows the focus is not ringed inside it; bordered
fields turn their border accent); a change card's Diff opens the review view at that file (one diff view, with line
numbers, Stage and Revert); the side chat's box takes the main one's focus border and send button; the palette names the
permission modes with what each does.
0.1.70: from the comparison with the harnesses installed on the Mac (Claude, T3 Code, MonoCode, DeepSeek Harness, Hermes):
the model and its effort in one pill ("Opus 5.5 · high"), the effort levels at the top of its picker, which opens above
the whole box (it covered the box's text); a copy button on each answer, beside its numbers; each conversation in the
sidebar marked with its provider (a letter: C Claude, D DeepSeek, O OpenAI, G Gemini, M Mistral…; core 0.1.178 sends the
model).
0.1.71: a status bar along the window's bottom (prototype mockups/v4.html, variant B): the plan's 5 hours and week with
their bars and what is left, SubRouter's balance, this conversation's context, cache and cost, the app's version; a click
on the plan or the cost opens the usage page. It replaces the plan bar by the send button and the cost in the header
(each number once). A narrow window drops the cache and the version first.

0.1.72: the usage page's "What's contributing to your Claude usage" laid out, where it was claude's text as it came:
one period at a time (24 hours, 7 days), each cause with its share as a bar and its hint below, the command that helps
as a chip, the top skills, subagents and MCP servers as tags. The 7/30/90 days choice moved from the page's header to
Spending, the section it changes (with "How your runs went"): in the header it read as changing "Right now" too.

0.1.73: the model picker opens below the message box where there is no room above it (the start page, whose box sits
high, cut it off at the top), and its height is what the page has room for.

0.1.74 (design review 4, every screen and menu on test data; before and after compared side by side):
- One placement rule for every menu on the composer (usePlace): the permission and folder menus open above the whole box
  where they fit, else below it. The permission menu was cut off on the start page and covered the box's text elsewhere.
- A reply that failed shows an error card: the provider's message in words, Retry, its raw reply on request. The raw
  JSON was printed as the answer, after a "⚠", and a failed reply otherwise ended with nothing; a stopped one says so.
- Answers: # and ## are headings (all headings were bold paragraphs); a numbered list goes on from its number after a
  code block (it restarted at 1); lists nest; code blocks show their language and a copy button; table columns align as
  their |--:| line says; *italic*.
- The backdrop behind a dialog darkens in both themes (the ink at 15% turned the dark window grey); dialogs, menus and
  the palette come up with a short motion, as toasts and the work panel did. The usage page keeps one height.
- One format for every reset on the usage page ("resets Nov 25 at 12:03pm"; OmniRoute's read "in 5 d"); the status bar
  says "5 h · resets in 2 h 14 min" and shows no context and cache before a conversation; money as "$0.41".
- Models by name ("Opus 5.5") in the usage tables, the sidebar's second line (instead of a letter in a box) and the
  model button before the list arrives. The picker labels its columns and names the provider once.
- The text column is as wide as the message box; the start page's rows ring their focus inside the card; the palette
  marks the typed words and lists its keys; the review's diff keeps the code's colours; ⌘\ in the shortcut sheet;
  plainer Settings wording.
- `desktop/review/sweep.cjs` takes the screenshots of all of this on test data (`review/mock.js`): run before a release.

0.1.75: the sidebar fills its column whatever its rows (with one project and no conversations it shrank to them and left
a strip of the window's background beside it); the start page sits in the middle of the window, a little above it (12%
from the top left a tall window empty in its lower half); a lone "Start in a project" card spans the width. The sweep
has this new user's window now.

0.1.76: the conversation lists load once the conversation on screen has its view. A project opened before its core had
answered (right after launch) skipped the fetch, so the sidebar and the start page stayed empty until a switch to a chat.
In a project the start page offers that project's own recent conversations, and the open project's list is unfolded.

0.1.77 (design review 5, on real data: `desktop/review/bridge.mjs` runs the window on the real core, read-only, and
`review/real.cjs` goes through your conversations, projects, models, accounts and settings with the in-page checks of
`review/audit.cjs`: text cut without "…", contrast under WCAG AA, targets under 24 px, anything wider than the window):
- A core that does not start shows why and Try again; the demo's title, plan and cost no longer fill the window around it.
- The model picker's search takes the typing as it opens (0.1.74 to 0.1.76 it did not); a menu or picker that closes
  gives the focus back to its button; the permission and folder menus move with the arrow keys.
- The status bar's plan is the account's newest reading, not the open conversation's own (it changed with each
  conversation opened, up to days old).
- Conversation names say what a pasted link is: "app PR #1521", "Claude artifact", with the next line when the
  first was only a link (eight rows read "https://github.com/acme/a…").
- Opening another conversation closes the idle one on screen (it stays in the lists); the one on screen stays in Recent,
  marked; projects stay folded until unfolded (0.1.76 listed each project conversation twice).
- **`code`** inside bold or italic; money in cents from ten cents up ("$10.13", was "$10.127"), "< $0.01" and "$0" in the
  usage tables; "of 1M" (was "1000k"); "1 min" (was "1 min 0 s"); tools without a verb counted ("used 4 other tools", not
  "toolsearch ×3, guide"); the usage page's model table shows the eight most used, the rest on request; a provider's Save
  shows once a key is typed; the sidebar's row buttons are 24 px; a long first message folds to six lines.

0.1.78 (the user's four points with design review 6):
- The usage page's model table names the provider of each API run ("SubRouter", "Mistral", "OmniRoute"; recorded since
  CLI 0.1.182, inferred for older runs where only one provider serves the model, else "API") and every model by its name
  ("DeepSeek V4.1 Flash 1017 Fast", not deepseek-v4.1-flash-1017-fast). The same names in the model picker, its button,
  the header and the palette (one rule, `modelLabel`, with the terminal's `pretty_model` on the same cases).
- "Show all 32 models" and "Show fewer" are a full-width button with a chevron (it read as a line of text).
- No middle dots: a stat sits under or beside its value ("3% clear misses", then "3% pushback  468 runs"), and items of one
  line are apart by a hairline (`Sep`, `Parts`): sidebar rows, the line under an answer, the steps' line, chips, pills,
  the picker, Settings, the PR view. The lanes are "Claude plan" and "API providers" (were "Claude (plan)", "own loop (API)").
- The status bar's usage: three variants in desktop/mockups/v5.html, for the user to choose.

0.1.79: the status bar's usage is variant A of mockups/v5.html. On the left the account the conversation runs on (the
plan's 5 hours and week on Claude, the credit on SubRouter, Mortera or OpenRouter, the window on OmniRoute), its dot in
its standing's colour; beside it one badge when another account needs you: its name and reading when it is one
("OpenRouter $0.17 over"), a count when more. Hover: every account, the conversation's own marked. A standing per account
(lib/health.ts: fine, running low from 75% or under $2, stopped from 90%, at $0 or a lapsed login), not one score: 93% of
a week and $12.40 left add up to nothing anyone can act on. The readings come at the start, every 15 minutes and with
the usage page.

0.1.80 (design review 6):
- Under each answer its own cost and tokens (CLI 0.1.183's `run_cost` and `run_usage`): "$0.29, 673k tokens" where it
  showed the conversation's total so far ("$12.13, 4.8M tokens" under a 31-second reply).
- The steps say what a command runs, not the `cd "<project>" &&` before it; a short thought with nothing to read is no
  row of its own (thoughts from 1k tokens stay).
- "Your messages" keeps its rows' height and scrolls (a long list squeezed them and cut the letters' tails).
- The usage page names each OmniRoute account ("agy, someone") and says "OpenAI-compatible" for a generated provider id;
  plainer wording on the usage page and in Settings' About.
- The model picker's prices evenly: "0.021 / 0.064" for "0.0213 / 0.0639", "free" for "0 / 0".

0.1.81: when each window starts again, as Claude says it: "in 2 h 14 min" within a day, "Sat 3am" within the week,
"Nov 26" later, the exact time on hover (lib/health.ts `whenReset`). The status bar shows one: the window closest to
stopping you when one runs low (the week at 96%), else the soonest; it had none since 0.1.79. The card shows every
window's (the conversation's own too, where it said only "this conversation"), and the usage page writes Claude's and
OmniRoute's the same way (OmniRoute's had no reset in the card; the page wrote dates only). A window from 90% is red on
the usage page as in the status bar.

0.1.82: the usage page's accounts in groups under headings written out whole: "Claude plan", each OmniRoute account
("OmniRoute | agy | someone@example.com | Antigravity starter quota") with its windows by name below, "Credit with each
provider". Under each window the account and plan were cut to "…" in 200 px, with no way to read them (the user). The
review's in-page check (review/audit.cjs) now also flags text cut to "…" that nothing lets you read whole; on real data
it found only the start page's recent titles besides, which now have it on hover.

0.1.83 (the user): every window in the status bar with its own reset after a small ↻ ("5% 5 h ↻ 3 h 30 min ▬ 96% week ↻
Sat 2:59am"; the words and the exact time on hover), where one reset stood for the bar and the 5 hours' was missing. The
badge for another account has its own hover, what it means and what to do ("Your OpenRouter credit is $0.17 past zero.
Add credit, or runs on OpenRouter will fail."; a Claude window from 90%: "Once it is full Claude stops, or bills extra
usage if you turned that on. It resets on Sat 3am."); it opened the account card, the same as the plan beside it.

0.1.84 (design review 7, with an interaction crawl: every control on every screen hovered and clicked on real data):
- The status bar's account card in groups, as the usage page (Claude plan, credit, each OmniRoute account written out
  whole), every line wrapping; a flat list of 13 rows cut "OmniRoute OpenAI-compatibl…" and the resets.
- The spending chart is one stop for the keyboard, the arrows along the days; each of its 30 to 90 bars was a stop and a
  click on one did nothing.
- A link pasted without its scheme names itself too ("app PR #1476"), and a Jira issue as "Jira T-1214".

0.1.85 (the user): the badge is no button (the account beside it opens the usage page); several accounts that need you
name the most urgent and count the rest, "OpenRouter $0.17 over +2", the stopped ones first, each in the hover under "3
accounts need you" (it said only "3 accounts need you"); "Add credits".

0.1.86 (the user): the version at the status bar's end checks for updates on a click and answers in place ("checking",
"up to date" for a few seconds; an update found opens its dialog, as Settings' button); an update the hourly check found
shows there by itself, "update to 0.1.87" in the accent. It was a line of text that did nothing.

0.1.87 (with tazuna 0.1.185; design review 8, against the other harnesses' windows: Conductor, T3 Code, Cursor's agents
window; before and after of each change side by side, as review 4's):
- The message box completes. `/` at its start lists the conversation's commands with what each does (the core's
  `complete`: the terminal's menu), `@` anywhere the project's tracked files; arrows, Enter or Tab to take, Esc to shut.
  The placeholder says so. It was a plain text field: typing `/` showed nothing.
- Attachments are chips over the text (a name, a thumbnail for an image, a way to take it off), sent as paths after the
  message as before. They were paths typed into the message.
- An answer that names an image file (`out/chart.png` in code marks, or a markdown image) shows it under the answer, up
  to four, each only if it is there; a click shows it whole. The shell's `image_data` reads it (images only, up to 8 MB):
  the page's policy loads nothing from disk or the network.
- Settings is one part at a time with a list on its left, and has Appearance: theme (System, Light, Dark; a flip now
  stays), text size (the webview's zoom) and the type answers are set in. A provider's key field shows when asked for.
- What floats lifts off the page (DESIGN.md's `pop` shadow, a lighter fill in dark), and a popover's list fades at its
  lower edge. Every `title` shows in the window's own tooltip (118 system ones). Selected text is the accent. A closed
  menu or dialog fades out; the work panel's column slides. All of it in lib/chrome.ts and styles.css.
- Scrolled up, a button goes back to the latest; an answer's calls open into view; one time format per answer ("16 min
  4 s" in the fold and under it); the approval card has the warn colour on its edge; the work panel says why it is empty
  in a plain chat.
- Not done from the review: the model picker's rows that need a key (one repeated sentence), the status bar changing
  shape between screens, days and pins in the sidebar, shorter copy on the usage page, the best-of dialog's raw ids,
  math, pasting an image, the app icon.
- The sweep (review/sweep.cjs, now with the message box's menus, chips, a picture and Settings' parts) ran in Chromium
  and, after the release, in WebKit (Playwright's, `chromium` swapped for `webkit`): 84 steps each, no page error. Then
  in the real window: the shell in dev mode with a script in the page that drives it and reports through `ui_log`
  (the zoom at 1.25 measured on the window's width, a dropped image's chip and thumbnail through `image_data`, which
  refuses a file that is no image, the tooltip, the `/` menu on the real core, a closed menu's and dialog's fading
  copy, the panel's column mid-slide), with the window captured at each step. Not seen there: a picture under an
  answer (the same `image_data` and `<img>`), the `@` menu (seen on the real core in a browser).

0.1.88 (the user, with MonoCode's bar as the example): a refresh button at the
end of the status bar's account reading (two arrows; the single one marks a reset). It reads every account again
(claude's own /usage, a few seconds, no model call), turns while it does, and says on hover when the reading on screen
came ("read 4 min ago"). The reading came only at the start and every 15 minutes: a stored one from an hour before
said "week 99%" where the live one said 72%. Seen in the real window against the real accounts (5.7 s).

0.1.89 (the user: "it was stuck at 15%, then I refreshed it once and it became 21%"): the status bar's usage follows the work, not the clock.
- Between full readings the plan's 5 hours and week come from the conversations themselves (lib/health.ts, `withLive`):
  claude's rate-limit event during a reply, which the core already kept and the bar ignored once the first full reading
  had come. Free. In 101 stored recordings it comes about three times a turn, 55 s apart at the median while a reply
  runs. The week on one model is not in it and waits for a full reading.
- The full reading (`claude -p /usage`: the CLI for about six seconds, no model call) is asked for when a reply ends, when
  the window comes back after three minutes or more (other programs spend the same plan), every 15 minutes while a reply
  runs, from the usage page and from the bar's button; an automatic one at most once a minute. Idle, none: it was every
  15 minutes whatever happened.
- Checked with a clock the test moves (12 cases: one reading at the start, none 10 minutes into a reply, one at 16, the
  conversation's newer numbers in the bar, one when the reply ends, none through an idle hour, one when the window comes
  back, none when it comes back again at once, the button always); the real window does get its focus event.

0.1.90 (with tazuna 0.1.186; the user: the refresh "takes like a few seconds", MonoCode's feels instant): a click on
refresh shows the plan's numbers in under two seconds where it took 5.7 (the core reads the plan over the CLI's stream
and sends it first, tazuna 0.1.186's notes; the bar takes that line, `accounts_plan`, and keeps the other accounts as
they were until theirs come a moment later). Timed in the real window, stamped when the button's state changes.

0.1.91 (with tazuna 0.1.187; the user, three things from one afternoon):
- The status bar's usage in MonoCode's few characters: "Claude ▬ 36% 2h 14m | ▬ 79% 1d 2h", a window being its meter,
  how full it is and how long until it resets. It read "Claude plan ▬ 36% 5 h ↻ 2 h 14 min ▬ 79% week ↻ Sat 3am": each
  window's name and a ↻ before its time ("way too much text"). Which window and the exact time are on its hover and in
  the card. The meters stay (their amber and red are the warning); no logo, the word.
- A window whose reset has passed reads "0% new" (a new one starts with the next message): claude's usage page goes on
  saying what the finished window held, and the bar said "100% 5 h, resets now" through two refreshes. A reset further
  off than its window is long is one that has passed (a time with no date was read as tomorrow's).
- A step cut off by a stop is "■ Asked a subagent | stopped 1.4 s" and counts no further (tazuna 0.1.187's notes).
- review/mock.js: the plan's resets move with the clock (its fixed dates had passed).

0.1.92 (with tazuna 0.1.188; the user: "see the background chat (the subagent that the main agent spawned)", as the
terminal's ctrl-b):
- A subagent's own conversation opens beside the one that started it (components/AgentPanel.tsx, the side chat's
  column): what it was asked, each step, what it wrote, drawn as any conversation and read only; it follows the agent
  while it runs (asked again every 1.5 s) and has a Stop for it. From "Open" on any "Asked a subagent" step, also in a
  resumed conversation, and from the row below.
- While claude's work goes on after a reply, a row over the message box says so: "Subagent in the background", what it
  is doing, its tokens, "Watch" (a command in the background the same, without the conversation). The conversation
  looked idle while a reviewer ran and spent the plan.
- claude passes on a subagent's steps and messages, not its reasoning (none in the recordings): the panel shows what it
  did and said.
- Checked on a real session's recording through the core (three subagents' conversations of 40 to 112 steps; the one
  stopped before it began has none) and on test data (review/mock.js has one working; the sweep opens it). Not seen: the
  real window, and one followed live (that takes a real reply).

0.1.93 (the user: "every single time I reinstall an update and open a chat it keeps asking me this", and the new-chat
page flashing before the chat):
- The macOS app is signed with tazuna's own certificate (self-signed, free; .github/workflows/desktop.yml). macOS knows
  an app by its signature: ad hoc, that was each build's own hash, so every update was a new app and it asked for the
  Desktop folder again. Signed, it is "dev.tazuna.desktop and this certificate" in every build (two test binaries signed
  so had the same requirement; the app built here with it verifies and starts). One more question on the first signed
  update, then none: that last part is seen only after two signed updates. Not Apple's signing (still no $99): a
  download's first open warns as before. The certificate is beside the updater's key (~/.tauri/tazuna-macos-signing.*)
  and in the repository's secrets; without it macOS asks once more after the next update, no more than that.
- Opening a saved conversation no longer shows the start screen for a moment first: its core comes up empty and its
  messages follow, and the window drew "What are we working on?" in between (0.8 s with a slow one, in the test; long
  enough to see while macOS asked its question).

## Decided (2026-11-15)
- macOS, Windows and Linux together, as for the CLI (Linux renders through WebKitGTK: test it from week one).
- Tauri with a React frontend in this repo (`desktop/`); the Rust side links tazuna's core.
- Design: five directions as mockups in `desktop/mockups/` (index.html; `python3 build.py` rebuilds them). We each pick,
  then the first direction goes into DESIGN.md.
- User customization from the start (see below).
- No code signing for now (2026-11-16): the app is for the user's own use; Apple's Developer Program ($99 a year) and a
  Windows certificate (or SignPath's free open-source signing) wait until others use it. Updates are signed anyway, with
  the updater's own free key (0.1.20).

## What it is for

One app for both kinds of asks: work in a project (the coding agent) and everyday questions (a plain chat), picked by
itself, with the CLI's strengths kept: cheap, fast, small, cost visible. The CLI stays; both share one core and one
data folder, so a session started in one opens in the other.

Principles, from tazuna's two goals (ROADMAP.md):
- Small footprint is a feature. Installed today: Claude 865 MB, DeepSeek Harness 1.0 GB, T3 Code 319 MB (all
  Electron), Zed 177 MB (native), MonoCode 41 MB and Hermes 12 MB (Tauri). The Electron apps also draw the most memory
  complaints (Claude ~1.2 GB idle, Codex's server ~4.4 GB, T3 out of memory on long threads).
- Cost is shown, never hidden: dollars per message and per thread, plan headroom next to API spend. Cursor removed
  per-request cost and users noticed; nobody shows it live.
- tazuna decides what it can (chat or agent, later model and effort) and says what it decided, with one key to
  override. Every other app makes the user pick Chat, Cowork, Code or Work by hand.
- Calm and keyboard-first: a document-style transcript, few panes by default, a command palette.
- Local and private: no telemetry by default, keys in the keychain, the data policy (`/data`) shown.

## Architecture

- Tauri 2: Rust backend that links tazuna's core (live.rs drives the official `claude` CLI, engine.rs is the own
  loop, route.rs, models.rs, the session recordings), web frontend for the window. Target: under 40 MB installed,
  under 150 MB in use beside the `claude` CLI.
- The frontend renders the same event stream the terminal UI renders (session.rs), over Tauri events: one source of
  truth, and the CLI and the app cannot drift apart.
- Risk to test first: WebKit differs per OS (OpenCode left Tauri for Electron over slow, inconsistent WebKit rendering
  and a Windows sidecar that failed to start). Keep the frontend light, virtualize long transcripts, try macOS WebKit
  and Windows WebView2 in week one.
- Dependency to confirm before building on it: how Anthropic counts headless `claude -p` use on a subscription
  (reports from June to October 2026 disagree on separate credits for it).

## Versions

Semantic versions, as for the CLI (its own number, in desktop/src-tauri/Cargo.toml and tauri.conf.json): each release
bumps the patch (0.1.1, 0.1.2, ...), a stage below starts the next minor, and 1.0.0 is the first public release. The CLI
keeps its own numbers; the app says which tazuna it needs.

Work order from 0.1.33 (2026-11-17, ranked by worth for daily use and the goals against effort): the per-turn call view
(done), the accessibility pass (done), MCP servers and hooks in Settings (done), best-of-N across models (done), first run with "choose later" (done),
the router learning from real use (held back: too few runs on cheap models yet), the approval reviewer (done), plugin sharing and a gallery (done), SSH and remote sessions, the
mobile companion and team settings. Code signing stays off by choice.

### 0.1: the MVP (usable every day by one person)
- Window: sidebar with Chats and Projects, the transcript, the composer. Simple by default.
- New chat = plain chat; Open folder = a project with the agent; inside a project, general questions go to chat by
  themselves (0.1.127's router), marked, with one key to send to the agent instead.
- Both engines: the Claude path and the own loop; model and effort picker grouped by provider, searchable.
- Transcript: streaming markdown and code, tool calls collapsed to one line each, expandable.
- Cost: per message, per thread, Claude plan usage (5-hour and weekly), SubRouter balance; context use as a ring.
- Permission mode always visible under the composer (auto, ask, edits, plan) with its shortcut; approvals inline.
- Changes: a read-only diff of each turn, and undo of the last turn (tazuna's rewind).
- Sessions shared with the CLI (open either way); settings for providers and keys (`/login`) and the data policy.
- Light and dark from one set of design tokens; Cmd-K palette; Cmd-/ shortcut sheet.
- Out: worktrees, parallel agents, PR panels, plugins, mobile.

### 0.2: better than the CLI for daily work
- Diff review: per turn, branch against base, uncommitted; stage or revert by hunk; line comments batched and sent to
  the agent with one key (Claude, Codex, T3).
- Checkpoints as hidden git refs and "edit from here" (rewind files, conversation, or both; T3).
- Background threads: send to background and open a fresh one in one key; steer or queue mid-turn, the queue kept
  across restarts; pending approvals as sidebar badges (T3).
- Side chat: reads the thread, writes nothing back (Claude, T3).
- Usage page: cost per model and day, cache savings, plan headroom and API spend; the real-use score
  (bench/realuse.py: stops, rewinds, corrections, discarded changes) per version and model.
- Global quick-entry hotkey for a chat from anywhere (Hermes); draft recovery and a prompt timeline (Hermes).

### 0.3 to 1.0.0: public release
- Worktree per thread with a setup script, gitignored files copied in, a port range each; archived when the PR
  merges (Conductor, Claude).
- PR panel through `gh`: checks, review comments, fix a failing check.
- Routing that explains itself: "DeepSeek high, est. $0.004" or "Opus, plan headroom 62%"; plan first, a cheap API
  model when the plan runs low; a warning and a cap before any paid call.
- Signed builds for macOS, Windows and Linux, auto-update; first run lets you "choose later" (Hermes). Auto-update done in
  0.1.20, the first run in 0.1.37; code signing off by choice.
- Accessibility pass, reduced motion honoured. Done in 0.1.34.

### 1.1
- Best-of-N across models in worktrees, each lane with its cost and checks: price to performance visible on the spot
  (Cursor's idea, with the number Cursor hides). Done in 0.1.36.
- An approval reviewer inside a fixed sandbox (Codex Auto-review, Claude Auto). Done in 0.1.38.
- Trajectory view per turn: each call, its tokens, time and cost (DeepSeek Harness). Done in 0.1.33.
- MCP and hooks in settings; hooks that run shell commands at no token cost (Kiro): done in 0.1.35. SSH and remote
  sessions.

### 1.2
- The router learns from real use: model and effort per kind of ask, from what was kept, reverted or corrected
  (ROADMAP "where tazuna can lead" 1).
- Mobile companion for watching and approving; shared team settings; plugins.

## Design

### Direction (proposal: A, with B's density in the code view and C's mono touches)
- A, warm paper: cream canvas (~#f6f4ef), warm ink (~#23221e), warm near-black for dark (~#171613), one accent:
  indigo (ai-iro; tazuna means reins). Hairline borders instead of shadows, medium density, a serif for chat prose.
  Family: Cursor's and Anthropic's styles on Refero.
- B, midnight instrument: Linear and Raycast; dense and polished, but the most cloned look in developer tools.
- C, quiet terminal: mono-led, monochrome with one status colour, carries the TUI over; tiring for long chats.

### Layout (0.1)

```
┌───────────────┬──────────────────────────────────────────────┬──────────┐
│ ⌕ search      │  project: api-gateway · Opus 5.5 high        │ context  │
│ + New chat    │──────────────────────────────────────────────│ ◔ 34%    │
│ Chats         │  you  why does the retry test fail?          │ plan 5h  │
│  · cat names  │  tazuna  The retry delay doubles … (prose)   │ ▮▮▮▯ 62% │
│ Projects      │   ▸ read src/retry.ts  ▸ ran 2 tests  ✓      │ thread   │
│  ▾ api-gateway│   diff  +12 −3  src/retry.ts      [view]     │ $0.042   │
│    · retry    │                                              │          │
│    · ci speed │  ┌──────────────────────────────────────────┐│          │
│               │  │ ask anything…                             ││          │
│               │  │ auto · Opus 5.5 high · $0.004 last  ⌘K    ││          │
│               │  └──────────────────────────────────────────┘│          │
└───────────────┴──────────────────────────────────────────────┴──────────┘
```

### Stack for the look
- shadcn/ui (Radix) with Tailwind v4, tokens generated from DESIGN.md; ReUI for grids, trees and filters (run
  history); EvilCharts or plain shadcn charts for cost and usage (check its licence first).
- Bencho and React Bits micro as references only: context meter, thinking indicator, hold-to-confirm, status mark,
  rebuilt in our own tokens. Transitions.dev for timing of panels, tabs and toasts.
- One icon set: Phosphor (light/regular) or Lucide at 1.5 stroke, 16 px. Vitrum only for the app icon.
- Fonts, bundled: Geist or IBM Plex Sans for the UI; Geist Mono or JetBrains Mono for code and tool calls; Source
  Serif 4 for chat prose.
- Motion: CSS transitions of 120-200 ms ease-out; Motion only for layout changes; no WebGL, particles or animated
  backgrounds (battery and frames in WebKit); streaming tokens are not animated.

### Keeping it consistent while agents write the UI
- One DESIGN.md in Google's format, linted (`npx @google/design.md lint`), seeded from a Refero or awesome-design-md
  system and rewritten so it is ours; the Tailwind theme generated from it; a short list of the components we own.
- Impeccable: `init` writes PRODUCT.md; `npx impeccable detect` in CI (no model, no key); `critique` and `polish` for
  review passes. Not stacked with other design skills (conflicting rules).
- Screenshots of the key screens in both themes on each UI change.

### What makes an app look AI-built (avoid)
Inter everywhere; purple-blue gradients, glass and glow; cards in cards; an icon tile over every heading; stock shadcn
zinc with rounded-xl and default Lucide; many accents or status colours as decoration; sparkle icons for "AI";
shimmer on everything; two-sided chat bubbles; web habits in a desktop app (selectable chrome, pointer cursors
everywhere, no keyboard flow); dark mode as an afterthought; numbers that are not real.

## Customization: users shape their own window

DeepSeek Harness's creator mode (studied 2026-11-16: memory and the research behind it) builds the whole app from
plugins and lets its agent write more, in-process and unsandboxed, in ~60 spots that can also replace built-in parts.
tazuna takes the reach and keeps a safe default:

- **Spots everywhere** (desktop/src/lib/slots.tsx, docs/PLUGINS.md): rail, header.actions, composer.dock,
  composer.pills, sidebar.top, sidebar.footer, message.user, message.text, message.after, tool.view, overlay.
  tazuna's own parts register there the same way (src/builtins.tsx), so a plugin can add, replace by id, or hide, and a
  part that works well can move into the app as it is.
- **Two kinds**, one folder each in ~/.config/tazuna/desktop/plugins/<id>/: a **panel** (index.html) is sandboxed and
  read-only (no permission needed); a **plugin** (plugin.js) runs in the window with its reach (spots, styling, Cmd-K
  actions, sending messages) and runs only once the user enables that exact code (Settings; a change outside creator
  mode asks again). Safe mode starts without plugins. Behaviour beyond the window (tools, commands) goes through what
  tazuna already has (MCP servers, skills, commands), not a plugin system in the core.
- **Creator mode** (`/create ...`, Cmd-K): a conversation in a new folder with the specification and a live map of the
  window; writes inside that folder need no question (the core decides on the real, resolved path); when the turn ends
  tazuna checks what was written without running it in the window (a plugin runs once in a sandboxed frame against a
  stand-in that records what it would register) and sends a failure or "shows nowhere" back into the conversation (two
  rounds), where DeepSeek's creator reports done for plugins that never loaded. A plugin that passes waits for the user:
  what it would do, its code, and Enable; nothing agent-written runs in the window before that (security review).
- Sharing and a gallery: done in 0.1.39. Next: more spots as needs show (settings sections, the sidebar's lists).
  (Behaviour plugins came in 0.1.28: /create.)

## Open questions
1. Which design direction first (mockups A-E)?
2. Accent colour, name and icon once the direction is chosen.
