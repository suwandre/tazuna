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
open one, a merged worktree offers its removal. CheaperInference joins the providers in Settings.

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
- Signed builds for macOS, Windows and Linux, auto-update; first run lets you "choose later" (Hermes).
- Accessibility pass, reduced motion honoured.

### 1.1
- Best-of-N across models in worktrees, each lane with its cost and checks: price to performance visible on the spot
  (Cursor's idea, with the number Cursor hides).
- An approval reviewer inside a fixed sandbox (Codex Auto-review, Claude Auto).
- Trajectory view per turn: each call, its tokens, time and cost (DeepSeek Harness).
- MCP and hooks in settings; hooks that run shell commands at no token cost (Kiro); SSH and remote sessions.

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
- Next: sharing and a gallery by 1.0; more spots as needs show (settings sections, the sidebar's lists); behaviour
  plugins in 1.1.

## Open questions
1. Which design direction first (mockups A-E)?
2. Accent colour, name and icon once the direction is chosen.
