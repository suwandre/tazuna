# Changelog

Every change that can affect the agent is measured before it stays. Each entry lists what moved and by how much. Versions
that change no measurable metric (CPU, memory, binary size, tokens, cost, quality) get no metric table.

**Tiers**
- **Tier 0** (every change, free): tests, `bench/tier0.sh` (binary size, replay render time, input tokens per turn, and since
  0.0.15 startup time, UI CPU and memory while animating, tool server memory).
- **Tier 1** (changes to prompt, tools, routing or stop logic): 6 real-work Go tasks × 2 runs vs a same-day baseline. Judge
  score, hidden tests, cost, turns, time, cache hit rate.
- **Tier 2** (releases): Tier 1 plus the cross-harness table in the README.

**Noise thresholds**: judge ±0.75, cost ±15% (mean), turns ±20% at 12 runs per arm. Smaller differences are reported as
"within noise". (The judge threshold was ±0.5 until 0.0.3, whose two arms behaved identically and still scored 0.67 apart.)

Also recorded per measurement: the `claude` CLI version. Claude Code updates itself, and its built-in prompt changes token
counts (2.1.289 → 2.1.291 added ~104 input tokens per turn).

## 0.1.84 (unreleased): a SubRouter key tazuna manages, or your own

- On a SubRouter key with no provider list for a model, SubRouter picks the provider: a bench round on such a key sent
  every DeepSeek V4.1 Flash call to RelAI, 11% of the input cached, where the checked providers (bangai, kriora,
  lingai) cached 99% (2026-11-09). Two ways now, offered where a SubRouter model has no key yet:
  - `tazuna subrouter --manage`: tazuna makes the key "tazuna (managed)" in your account, or renews it, with exactly
    the checked provider list per model, and keeps its value in `~/.config/tazuna/subrouter-key` (readable by you only).
    It is used whenever `SUBROUTER_API_KEY` is not set; other models still work on it, where SubRouter picks. A notice
    says what it creates and that your own key always wins. Needs the account's access token (`~/.subrouter-account`).
  - your own key in `SUBROUTER_API_KEY`: its settings choose the providers. `/model` says so on every checked row and
    when you switch to one (`tazuna subrouter --apply` still puts the checked lists on it).
- Checked on the user's account: the key made with DeepSeek's list on the checked three and no limits, renewed in place
  on a second run (no second key), the file matching it, one call landing on LingAI; then deleted.
- The managed key stays out of tazuna's environment (an automated security review caught the first version putting it
  there, where claude and the agent's commands would inherit it): tazuna reads it from the file where it builds its own
  requests, and only the own loop's engine gets it, which removes it from its environment before any tool runs.

## 0.1.83 (2026-11-09): a selection in the message box no longer copies itself

- Releasing a drag in the message box copied the selection (0.1.81), so selecting a word and pasting over it pasted that
  word back. The selection now copies with ctrl+c (cmd+c where the terminal passes it on: Terminal.app and iTerm2 keep
  cmd+c for themselves); without a selection ctrl+c still stops or quits. The transcript still copies on release: text
  there is only selected to be copied.

## 0.1.82 (2026-11-09): a model that refuses its effort gets one it takes

- The own loop sent the session's effort to every model, and a model that refused it ended the call: Codestral (Mistral)
  takes no `reasoning_effort` at all, not even `none`, and Mistral Small takes only `none` and `high`. With the cascade
  on, each such task went to Opus untried. Now a refusal that names the effort is answered once per model: the
  highest level the error lists up to the asked one (else the lowest listed), no effort field when the model takes none,
  or one level lower when only the asked one is refused (the old `max` to `xhigh` rule, which tried `max` again on every
  call). The model's later calls go out with that level from the start.
- Checked against Mistral's API (one call per model and level): Codestral none; Small and Medium 3.5 `none` and `high`;
  Large 4 `low`, `medium` and `high`; GLM 5.3 and 5.3 Flash `low`, `high` and `max`; DeepSeek V4.1 Flash all.
  Headless runs that ended in an error before (Codestral at `off`, Small at `xhigh`) made their file at $0.0005 and
  $0.0002. No change for models that take their effort.

## 0.1.81 (2026-11-09): the mouse in the message box; bold links open right

- A click in the message box puts the cursor under it; a drag selects typed text, shown reversed. Typing (or a paste)
  replaces the selection, backspace and delete remove it, any other key lets it go. Release copies it, as a transcript
  selection does (on Windows ctrl+c copies). Before, the cursor moved only with the keys. The click lands on the
  character drawn there: drawing and clicking share one layout of the wrapped text, and the cursor cell is not drawn
  from the press on, so nothing shifts under a drag. Not in zen, whose message row shows only the end of the text.
- Notes such as "copied 5 chars" now show under typed text too (before only in an empty message box).
- A bare address in **bold** linked to itself plus the closing marks (`…/pull/1437**`): cmd-click, Copy Link and a
  click opened a 404. The marks come out before links are found, as they already did for drawing.

## 0.1.80 (2026-11-09): MCP servers for every model (`/mcp`)

- One list of MCP servers for every model: `/mcp add NAME URL` for a remote server, `/mcp add NAME COMMAND [ARGS]` for
  a local one, kept in `~/.config/tazuna/mcp.json` in Claude Code's format. `/mcp on` and `/mcp off` replace
  `/connectors` (0.1.79) and are remembered the same way; `--mcp` is `--connectors`' new name (the old one still works).
  `/mcp` alone lists the servers.
- Remote servers that ask for a login get one as tazuna itself: OAuth with dynamic client registration and PKCE, the
  browser for consent, a one-time listener on 127.0.0.1, tokens in `~/.config/tazuna/mcp-auth/NAME.json` (0600, written
  through a rename), refreshed when they run out. A token is tied to the server's URL: a name pointed at another address
  never sends it there, and `/mcp add` over a name drops its login. Logins go over https only (or http to this machine),
  and the authorization server's metadata must name itself. Atlassian's server (`https://mcp.atlassian.com/v1/mcp`)
  registers tazuna as a client. No claude.ai connector token is used: those stay with claude.ai and the `claude` CLI.
  `/mcp login NAME` logs in again.
- The own loop (DeepSeek, Mistral, GPT ...) had no MCP client; it has one now (stdio and streamable HTTP). It gets one
  `mcp` tool whose description names the servers' tools: a call with `tool` alone returns that tool's description and
  parameters, a call with `arguments` runs it. The tool list stays the same all session, so the provider's prompt cache
  holds. In the ask and edits modes every run asks first (a server's own read-only marks are not trusted), named as
  claude names it (`mcp__server__tool`), so "always" covers that one tool. Servers connect in parallel within 30 s each;
  a server that ended its session gets a new one. `/mcp on` in the middle of a conversation connects in place: the
  conversation goes on.
- On claude the user's servers come next to the claude.ai connectors, remote ones through `tazuna mcp-proxy NAME`
  (tazuna's login). `/mcp on` works in every profile now: the lean profiles take the connectors too and add ToolSearch,
  without which claude sends every MCP tool's full definition.
- A server of the user's that does not connect shows a notice (`MCP NAME: ...`); claude.ai connectors waiting for a login
  stay quiet. `${VAR}` and `${VAR:-default}` expand as in Claude Code. A mcp.json that does not parse is never written
  over: `/mcp` says why. On Windows a server starts directly (npx.cmd from PATH), with no shell reading its arguments.
- With MCP on, naming a ticket is enough: the model fetches it. `/jira` stays as the shortcut that needs no MCP.

| Measured (default off: no metric moves) | MCP off | MCP on |
|---|---|---|
| Lean profile, Haiku, input tokens per message (claude 2.1.319; DeepWiki, Atlassian, Slack, Claude Docs: 83 tools) | 3.2k | 8.8k |
| Own loop, the `mcp` tool for Atlassian's 41 tools (characters / 4) | 0 | ~400 |

- Checked by hand: DeepWiki's `read_wiki_structure` through the proxy on Haiku (lean profile) and on Opus (ToolSearch
  loaded it first), and through the `mcp` tool on the own loop (Mistral Small 2603, $0.0003), each with the right answer;
  Atlassian's discovery and client registration up to the browser step.
- An independent review before the release found 6 major and 14 minor issues (the ones above: "always" covering every
  server's tools, a token following a renamed server, parallel refreshes losing tokens, a broken mcp.json written over,
  no new session after a 404, minutes of waiting on a server that is down); all fixed but the last limit below.
- Limits: the tokens sit in files the agent's commands can read, as `/jira`'s API token does; the older SSE transport is
  not supported (a clear error); esc does not stop an MCP call in flight (it ends at 300 s); two sessions refreshing one
  server's rotating token at the same moment can force a new login.

## 0.1.79 (2026-11-09): `/connectors` inside a session

- `/connectors` turns the user's claude.ai connectors and MCP servers (Jira, Slack, Claude Docs) on or off without
  restarting tazuna, as `--connectors` does at start: claude restarts with them and resumes the conversation (one cache
  rebuild). The choice is remembered (`~/.local/share/tazuna/connectors`) for the next live sessions, so a plain `tazuna`
  keeps them; `--run` still takes only the flag, so benches don't change. Repo profiles only, as before: the lean profile
  and the own loop say so instead. Default unchanged (off); the cost when on is the ~4.6k tokens a call measured in 0.1.69.

## 0.1.78 (2026-11-09): a 1-hour prompt cache for boss mode

- Headless runs in boss mode (the default for `--run` on Opus) now write the prompt cache with a 1-hour lifetime; other
  headless runs keep 5 minutes (0.0.20), live sessions keep 1 hour. `--cache-ttl` still overrides.
- Why: two costs of the 5-minute cache that 0.0.20 could not see. (1) In boss mode the lead waits on its workers, often
  past 5 minutes: 106 of 316 recorded Opus medium runs lost the lead's cache at least once, 181 of 198 losses came after a
  gap of 300 s or more, and those re-writes were 48% of the lead's cache writes. (2) Claude Code's tools and system
  prompt are cached across sessions, but only as stock sends them: a 5-minute lifetime changes the ScheduleWakeup tool's
  text, so the first call wrote the whole ~19.7k prefix. Single Haiku calls (`bench/prefixprobe.py`), one flag at a time against stock `claude -p`:
  only `CLAUDE_CODE_PROMPT_CACHE_TTL=5m` dropped the first call's cache read to 0; tazuna's full flag set at 1 hour read
  13.4k.
- Re-pricing the 313 recorded runs (`bench/ttl_replay.py`, no API calls; 1-hour writes at 2x instead of 1.25x, the first call's 13.4k and the
  re-writes after waits of 5 to 60 minutes read at 0.1x; the workers' writes at 2x too):

| Set (Opus 5.5 medium) | Runs | 1h vs 5m | Runs cheaper with 1h |
|---|---|---|---|
| besc (boss, 0.1.45 baseline) | 12 | −3.2% | 10 |
| bhmed (boss, workers medium) | 20 | −2.6% | 14 |
| boss (0.0.49) | 12 | −20.9% | 12 |
| bpick (`--boss-pick`) | 20 | +5.4% | 13 |
| noboss | 8 | +3.1% | 2 |
| all 313 | 313 | −3.6% | |

- One live run on a one-line fix, boss mode, Opus 5.5 medium: the lead's first call read 13,316 tokens and wrote 6,290
  (before: 0 and 19,726); the run cost $0.084 against $0.142 on 0.1.76 (one run each, not a benchmark). No change to
  prompts, tools or routing.

## 0.1.77 (2026-11-08): copying on Windows and Linux, the plan line on a new install, `/model` for users without the bench, releases from any machine

- Copying (a drag over the transcript, `ctrl-y`) ran `pbcopy` everywhere: "copy failed: pbcopy: program not found" on
  Windows. Now the system's clipboard: pbcopy on macOS, the clipboard API on Windows (not clip.exe, which guesses the
  encoding and on Windows 11 got "Bush hid the facts", a lone "ä" and a byte-order mark wrong), wl-copy, xclip or xsel on
  Linux. Over SSH, or where none of them takes it, the terminal gets the text (OSC 52: "copied N chars (through the
  terminal)"). On Windows a drag only selects and ctrl+c copies, as everywhere else there; releasing a drag still copies
  on macOS and Linux (0.1.70).
- The Claude plan line (5h, 7d) came only from earlier sessions' readings, so a machine without one showed none (the
  Windows PC, a new install that had run only Mistral and SubRouter). A session that finds no reading takes one at the
  start (one short Haiku call through claude, as `/usage` does; not with ANTHROPIC_API_KEY set). Readings from `/usage`
  and the start now go into the recording, so the next session opens with them (they were shown, not kept).
- The built-in model list (`bench/routes.py ship`) holds SubRouter's whole picker list (29 more models) and Mistral's
  checked routes on Mistral's usual key variable, `MISTRAL_API_KEY`: a new install listed 4 SubRouter models and no
  Mistral, the bench's machines 34 rows. Routes on a private host (the PC's Strata) stay home, and so do the bench's
  scores. Built-in routes are never marked stale: `tazuna upgrade` renews them.
- `/model` greys out the models SubRouter does not let `SUBROUTER_API_KEY` use (its /v1/models, read at the start): a key
  limited to 18 models answered 403 for the rest. SubRouter's copies of the subscription's models ("claude-opus-5.5"
  beside "claude-opus-5-5") are gone: a pick of one ran Opus through SubRouter instead of the plan. A greyed row says what
  to do: the variable to set (it said `source ~/.subrouter-key`, a file only the bench's machines have), or the line to
  add to the trusted list and where that file is.
- Releases need no Mac: CI builds every platform when a version tag is pushed (the repo is public, so macOS runners cost
  nothing), and `./release.sh` only checks, tags and pushes, from any machine. New builds: macOS on Intel and Linux on
  ARM; Linux builds on Ubuntu 22.04, so they also run where glibc is older than 2.39. A pull request that touches the
  code runs the same builds and the Windows tests. One-line installers (`install.sh` for macOS and Linux, `install.ps1`
  for Windows, which also puts tazuna on the user PATH), and `tazuna update` as another name for `tazuna upgrade`.
  `tazuna upgrade` no longer says the repo is private when a release is missing.
- Binary size +42 KB (+0.7%: 6,127,616 to 6,170,112 bytes, Windows release builds of main and this change on one
  machine), 9 KB of it the longer built-in list. No change to prompts, tools or routing; tier0.sh not run (Windows).

## The DeepSeek Harness's speed was its provider, 2026-11-04 (bench only)

tazuna's own loop on DeepSeek's official API (the Harness's provider): deepseek-flash (V4.1 Flash 1017) at xhigh,
caddy-quoted-braces, two runs without and two with the model's reasoning sent back (`TAZUNA_SEND_REASONING`), 21:41
CET (off-peak price). tazuna 0.1.54 with the search-prompt fix. Spent ¥0.57 of the user's granted credit.

| Arm | Minutes | Calls | Uncached | Cached | Output | Cost | Hidden |
|---|---|---|---|---|---|---|---|
| DeepSeek Harness (3 runs, earlier) | 3.2-4.4 | 43-59 | 59-75k | 2.4-3.2M | 28-32k | $0.035-0.039 | 5/5 each |
| own loop, SubRouter route (0.1.51) | 5.5-8.8 | 56-73 | 168-484k | 1.8-2.9M | 25-41k | $0.005-0.006 | 5/5 each |
| own loop, official API | 3.0, 4.1 | 41, 29 | 72k, 79k | 1.1M, 0.8M | 14k, 17k | $0.023, $0.025 | 5/5 each |
| own loop, official API, reasoning back | 2.5, 3.4 | 22, 41 | 38k, 41k | 0.6M, 1.4M | 15k, 16k | $0.017, $0.020 | 5/5 each |

On the same provider the own loop is as fast as the Harness or faster, with fewer calls and half the tokens: the gap was
SubRouter's route (slower calls, 2-6x the uncached input, more calls), which in exchange costs a quarter. Reasoning sent
back halved the uncached input in two runs; four runs on one task are below the noise threshold, so it stays opt-in.
All runs at off-peak price (the Harness's 10:55-11:10 UTC): the own loop costs 35-55% less per run than the Harness.

## 0.1.76 (2026-11-08): releases, `tazuna upgrade`, `tazuna --version`

- Each version tag (`v0.1.76`) becomes a GitHub release: CI builds tazuna for macOS (Apple silicon), Linux (x86_64) and
  Windows (x86_64) and attaches the archives, with the version's CHANGELOG section as the notes
  (.github/workflows/release.yml). Until now tazuna came only as source to build.
- `tazuna upgrade` puts the newest release in place of the running binary: downloaded beside it, unpacked with tar, run
  with `--version` to check it says the new version, then renamed over the old one (on Windows the running binary moves
  aside first and goes on the next start). A source build under target/ is left alone unless `--force`. The repo is
  private, so it reaches GitHub with GH_TOKEN, GITHUB_TOKEN or the gh CLI's login.
- A live session says once a day when a newer release is out ("tazuna 0.1.77 is out (this is 0.1.76): `tazuna upgrade`
  installs it"), from a check cached in update-check.json, off the UI thread; `TAZUNA_NO_UPDATE_CHECK` turns it off.

## 0.1.75 (2026-11-08): a route can name its cached-input price; Mistral's real prices

- A route's price in routes.json may carry a third number, cached input per million; without it cached input stays a
  tenth of input. Mistral's own list (docs.mistral.ai/inference/pricing) has DeepSeek V4.1 Flash 1017 at $0.30 input,
  $0.007 cached (2.3%), $1.20 output: these reproduce the billed caddy run ($0.062 against $0.06), where a tenth would
  have overstated each cached token threefold. Our Mistral routes carry the list's prices (GLM 5.3 $1.40/$0.14/$4.40,
  GLM 5.3 Flash $0.15/$0.03/$0.50, Small 4 $0.15/$0.015/$0.60, Large 4 $0.70/$0.07/$2.10, Codestral $0.30/$0.03/$0.90;
  Medium 3.5 is not listed). GLM 5.2 left them: Mistral retired it on 2026-10-31.
- Same model on both Mistral keys: the canary on the Vibe-plan key and on the API key, DeepSeek 1017 at high: AIME 29/30
  and 28/30, MMLU-Pro 88 and 88; 49.0 tokens a line and the same 28-token hidden prompt on both.
- Bench: a second run of each Go task for tazuna on the Vibe-plan key, Sol judging five arms per task again (pool 5):

| Arm | Judge | Hidden | Minutes (median) | Calls (median) |
|---|---|---|---|---|
| Vibe CLI, max | 7.67 | 20/24 | 22 | 98 |
| Vibe CLI, high | 7.33 | 22/24 | 27 | 92 |
| tazuna, Mistral Vibe key (12 runs) | 6.67 | 38/48 | 11 | 82 |
| tazuna, SubRouter route | 6.33 | 18/24 | 23 | 74 |
| tazuna, Mistral API key (4 tasks, two cut at the cap) | 8.25 | 11/11 | 7 | 52 |

  With every task and two runs, Vibe CLI on the same hosting scores 0.7-1.0 above tazuna's loop, borderline at these run
  counts and coming from the hard tasks (k6 5/5 and 3/5, hugo-template 6/8 and 5/8 against Vibe's 7/8); tazuna stays
  2-3x faster on a third of the plan usage. 0.1.73's "Vibe's lead was the hosting" rested on the four easier tasks only.
- Bench tools: provsmoke.py reads Mistral's streamed content blocks (its four-call probe failed every call there).

## 0.1.74 (2026-11-07): DeepSeek on the user's Mistral Vibe plan

- A key made under Code > Vibe CLI on chat.mistral.ai bills the plan's Vibe Code usage, not the API credit, also when the
  client is not Vibe: tazuna's own loop on Mistral's API with such a key (one caddy run: 5/5, 3.7 min, 61 calls, 2.28M
  tokens, ~$0.12) moved the user's Vibe Code meter and left API usage unchanged. Billing follows the key (as in
  mistral-vibe issue #1055), and Mistral's docs say plans include "monthly usage that is shared across Studio, the API, and
  Vibe Code". tazuna sends its own client name, nothing of Vibe's.
- The model picker names a checked route on another host by that host (it said "SubRouter" for every route). Our setup
  has `deepseek-v4.1-flash-1017@mistral` on `MISTRAL_VIBE_API_KEY` (routes.json, api.mistral.ai on the trusted list);
  Mistral takes reasoning_effort none, low, high, xhigh and max (not medium), and each is its own level there (3 hard AIME
  problems x2: low 7.9k, high 12.4k, xhigh 14.0k, max 19.3k reasoning tokens, all 6/6; on SubRouter low to xhigh are one),
  so the route offers all five.
- Bench, the six Go tasks on the Vibe key (cap $1.00 a run), Sol judging five arms per task in one pool:

| Arm | Judge (6 tasks) | Judge (4 tasks all finished) | Hidden | Minutes (median) | Tokens a run | Usage, 6 runs |
|---|---|---|---|---|---|---|
| Vibe CLI, high | 6.67 | 6.75 | 22/24 | 27 | 7.6M | ~$3.3 of Vibe usage |
| Vibe CLI, max | 7.50 | 7.75 | 20/24 | 22 | 7.8M | ~$3.3 of Vibe usage |
| tazuna, SubRouter route | 7.00 | 8.25 | 18/24 | 23 | 5.0M | $0.09 |
| tazuna, Mistral API credit (4 runs) | - | 8.50 | 11/11 | 7 | 3.0M | API credit |
| tazuna, Mistral Vibe key | 6.67 | 7.25 | 20/24 | 9 | 4.4M | ~$1.2 of Vibe usage |

  On Mistral (either key) tazuna runs 2-3x faster than Vibe CLI or its own SubRouter route and uses about a third of the
  plan usage Vibe CLI does for the same tasks. Vibe key against API key on the four shared tasks, 7.25 against 8.50, comes
  almost all from terraform (1/3 against 3/3 hidden) at one run a task; the same runs judged in different pools moved up to
  1.2 (tazuna on SubRouter: 5.83, 6.33, 7.00), so differences under ~1 point are noise here.
- Bench tools: canary.py reads Mistral's streamed content blocks (a string-only join failed every answer and retried each
  three times: the first Mistral effort probe was lost).

## 0.1.73 (2026-11-07): the own loop reads Mistral's replies; tazuna on Mistral's DeepSeek beats Vibe on it

- Mistral's API returns a Chat Completions `content` as blocks (`[{"type": "thinking", ...}, {"type": "text", ...}]`);
  the own loop read only a string, so every reply was empty. Text and thinking now come from either form, streamed or
  not. Mistral's DeepSeek 1017 thinks only when asked (no `reasoning_effort`: 0 reasoning tokens).
- Bench, same evening as 0.1.72's round: tazuna 0.1.73's own loop on Mistral's API (DeepSeek V4.1 Flash 1017 at high,
  the hosting Vibe used), Sol judging all four arms together per task. Two Mistral runs (hugo-template-return, k6) stopped
  at the $0.60 cap a run, so the four tasks every arm finished:

| Arm (caddy, hugo-section, prometheus, terraform) | Judge | Hidden | Minutes (median) |
|---|---|---|---|
| tazuna own loop, Mistral API | 8.00 | 11/11 | 7 |
| Vibe CLI, max | 7.50 | 10/11 | 21 |
| tazuna own loop, SubRouter route | 7.25 | 9/11 | 18 |
| Vibe CLI, high | 7.00 | 10/11 | 27 |

  Vibe's lead over tazuna on SubRouter (0.1.72) was the hosting: on the same hosting tazuna's loop scores higher, passes
  every hidden test and runs 2-8x faster with about half the calls. One run a task, four tasks, the two largest missing.
  Mistral's API costs ~$0.055 per M tokens at this mix (caddy: $0.06 billed, 1.42M tokens), 6-18x SubRouter's route, so
  $0.06-0.35 a task.

## 0.1.72 (2026-11-07): built-in DeepSeek route bangai, lingai, kriora; Vibe CLI against tazuna (bench)

- The DeepSeek 1017 route (built in, and our route key) is bangai, lingai, kriora. Dropped: x-loop (AIME 20/30 that
  evening, read as another model at 16:57) and a6aa (its content filter refused two of six bench tasks at their first
  call, after refusing a question on 11-05; SubRouter does not pass a refusal on). The gauge found every provider
  serving another model at least once in two days (tokens a line 36.5 or 56 against 49; the same 56 and hidden prompt on
  ydjs-ai and x-loop at once); those rounds also ran long: 7-11k output tokens a solved AIME problem against 3-4k.
- Bench: Mistral's Vibe CLI 3.1 (stock, headless, the user's EDU plan, only its own four skills: it also loads
  ~/.agents/skills) with DeepSeek V4.1 Flash 1017 hosted by Mistral, against tazuna 0.1.71's own loop on the SubRouter
  route, the six Go tasks, one run each, Sol judging each task's pool (three tasks rejudged in a pool of their own after
  tazuna's reruns: a6aa refused two, k6 timed out at 60 minutes on a6aa's route).

| Arm | Judge | Hidden | Minutes (median) | Calls (median) | Uncached + cached a run | Cost a run |
|---|---|---|---|---|---|---|
| Vibe, high | 7.00 | 22/24 | 27 | 92 | 1.37M + 6.2M | ~$0.55 of plan usage (~$6 for 11 runs) |
| Vibe, max | 7.17 | 20/24 | 22 | 98 | 1.29M + 6.5M | similar |
| tazuna own loop, SubRouter, high | 6.33 | 18/24 | 23 | 74 | 0.27M + 4.7M | $0.015 |

  Vibe +0.67 judge (inside the 0.75 noise threshold, at half its run count) and 4 more hidden tests; max no better than
  high. Harness and provider differ at once: tazuna on a provider that serves the real model every round (OpenScale,
  1.7 s to first token, 195 tok/s, AIME 29/30; or Mistral's API) would tell which.
- Two flat-price providers checked the same evening, both the real model: Experiential Labs (free tier: AIME 30/30,
  MMLU 88, 2.5 s, 116 tok/s, 99% cached) and OpenScale (pay-as-you-go test credit: 29/30, 87, 1.7 s, 195 tok/s, 95%).
  At SubRouter's $0.003-0.0095 per M tokens, a $20-29 plan breaks even only past 2-10 billion tokens a month.

## 0.1.71 (2026-11-06): one SubRouter key for every model

- Other users bring one SubRouter key, not a key per model and provider. A key holds a provider list per model
  (`subrouter_model_providers`), and SubRouter keeps each model's calls on its list: a test key with DeepSeek 1017 only
  on lingai and Sol only on bangai sent 6/6 calls of each there, also on a key open to every model, where a third model
  without a list still answered (bench/onekey.py, under $0.01). The key's flat provider field, filled, limits every model
  to those providers (a third model got 503 on every call), so tazuna never fills it.
- The checked routes are built into tazuna (src/routes.json, from `bench/routes.py ship`): without our bench's
  routes.json, `/model` offers them on `SUBROUTER_API_KEY`, noting where to set their providers.
- `tazuna subrouter` prints each model's checked providers and where to set them on the key; `--apply` sets them itself
  with the account's access token (~/.subrouter-account), on the key in SUBROUTER_API_KEY: lists for other models stay,
  the flat field and model limits stay, and shared pools are allowed when a list names one. Checked on a throwaway key.
- Bench: `routes.py tiers` picks DeepSeek's providers from the gauge: up to 4 of the cheapest a call (list price weighed
  by cache share: lingai at 99% cached costs a quarter of stableai at 33% for a higher list price) among those that passed
  every check, plus up to 2 proven ones (every call answered, 60%+ cached, 100+ tokens a second) at up to 3x the lowest
  list price, for when the cheap ones fail. Today: kriora, ydjs-ai, lingai, then bangai, x-loop; not applied until the
  canary has measured kriora and ydjs-ai.

## 0.1.70 (2026-11-06): a drag copies on release

- Selected text was copied only on ⌘C or ctrl+c (0.1.33). ⌘C never reached tazuna: the terminal keeps it for its own
  copy, which has nothing selected while tazuna holds the mouse. ctrl+c worked but reads as quit on a Mac. Releasing a
  drag now copies what it covers ("copied N chars" in the composer for a moment, nothing for blank cells, nothing in the
  transcript), and the highlight stays until the next key or click; ctrl+c on it copies again instead of quitting.

## 0.1.69 (2026-11-06): `--connectors`; the cost after `/resume` adds up

- `--connectors` (repo profile) lets the user's claude.ai connectors and MCP servers in (Jira, Slack, Claude Docs), without
  the rest of `~/.claude` that `--user-settings` brings (plugins, hooks, memory). tazuna kept them out with
  `--strict-mcp-config` since 0.0.11, when they cost ~42k tokens a turn; Claude Code now sends only their tool names until
  one is used. Measured (Haiku, claude 2.1.319, a task of two calls): 22.2k tokens a call without, 26.8k with, the
  connectors arriving from the second call on (Atlassian 41 tools, Slack 27, Claude Docs 5, 8 more waiting for a login).
  Off by default: ~4.6k more tokens a call, cached after the first. `--settings` with `disabledMcpServers` did not narrow
  the set to one connector.
- `/resume` replays a conversation's earlier recordings without the notices between them, so each recording's claude
  process, whose reported cost starts again at $0, read as a stopped turn: its tokens were priced at list rates ($51.50
  shown where claude reported $46.22), and a slash command, which uses no tokens, then showed that process's own figure
  ($55.63, then $17.98 after `/mcp`). Each replayed part now closes its process's cost.

## 0.1.68 (2026-11-06): `/jira`'s marks carry a random tag

- A comment could write `[end of ABC-1]` and a "My request:" line after it, which read as the user's own words past
  the ticket's end. Both marks now carry a tag drawn at random for each fetch (8 hex digits), and the opening mark says
  the data runs up to the end mark with the same tag (security review).

## 0.1.67 (2026-11-06): `/jira` puts a ticket in the message

- `/jira ABC-123 [what to do]` (or a ticket's /browse/ link) fetches the ticket through Jira's REST API and sends its
  text as the message: summary, type, status, assignee, labels, parent, subtasks, links, description and comments, up
  to 20,000 characters, then the request. tazuna fetches it itself, so it works for every model (the own loop has no
  MCP client) and costs no tokens until used; Atlassian's MCP server would put ~40 tool names in every Claude turn
  and needs an admin to allow API tokens. The token sits in `~/.atlassian-token` (`url=`, `email=`, `token=`), not in
  the environment the agent's commands see; `./jira-login.sh` writes it and checks the login. Reads only.
  Anyone who can comment wrote part of the ticket, and the default mode runs commands without asking: the text goes
  between marks that call it data about the task, not instructions, and the request follows as "My request: ..."
  (security review). Checked on the company site: T-1181 in 0.4 s, a missing key named as such.

## 0.1.66 (2026-11-06): the running cost at the model's own prices; DeepSeek's real effort levels

- While a turn ran, the cost on screen was its tokens at Claude's list prices ($4/M input, $20/M output); only the
  turn's end put the provider's figure in. A Luna turn read ~$3 while it ran and $0.007 when it ended. The own loop now
  tells the session its model's prices when it starts, and the running cost uses them.
- The model picker offers a checked route only the effort levels that differ (it offered all five and named the
  distinct ones in a note). DeepSeek V4.1 Flash 1017, measured on znu and a6aa with three hard AIME problems three times
  each (`none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` first once each): off (sent as `none`: ~2.6k tokens,
  1/3 right), high (low, medium, high and xhigh alike: medians 6-13k, overlapping), max (~19k). Its levels are now off,
  high and max.

## 0.1.65 (2026-11-06): the diff view is a file list; long rows and notices unfold

- `/diff` and a click on the files panel open a list of the changed files: path, `+added −removed`, a bar of their
  size, "gone" for one no longer on disk; the row under the mouse lights up, a click or enter opens the file in the
  editor at its first change. The diff text beside it ran off the modal and could not be scrolled sideways.
- A tool's call list cuts a long row to the modal's width with "…"; a click unfolds its whole text under it (the full
  command, the URL), dimmed. Sideways scrolling also on shift+wheel (VS Code's terminal sends no sideways wheel).
- A notice longer than 160 characters (a provider's error) shows its start and opens in full on a click; a known
  provider refusal is said in plain words first ("the provider's content filter refused this message ...") instead of
  the raw Chinese error, which was cut at 100 characters with no way to read the rest.

## 0.1.64 (2026-11-05): fetched pages by title; lists scroll sideways and close like modals

- A fetch call shows its page's title (fetch now puts it in its result's first line), or the URL's host and path for
  an API, instead of its raw `{"url": ...}`; in a tool's call list the label is a link that opens the page.
- A call list scrolls sideways (←/→, h/l, home, or the wheel's sideways motion): long rows ran off the modal.
- A list closes with esc or a click outside it; a click inside stays (it closed on any click, so clicking a row
  dismissed what one was reading).

## 0.1.63 (2026-11-05): the own loop can fetch the web

A `fetch` tool for the own loop (Claude has its own WebFetch): an http(s) URL in, its text out: a page's readable text
(the `<main>` or `<article>` part when marked, scripts, styles and tags out), JSON and plain text as they are, up to
20,000 characters, marked as web content to use as data, not instructions. Luna had fetched crypto prices with curl in
bash earlier in a session, then in a later turn looked for a web tool, found none and said it had no web access. The
proxy from the environment applies (the bench's blocking proxy still stops runs from fetching a task's fix). From a
security review of the first cut: it asks wherever bash asks (ask and edits modes: a page's injected text could make
it send data out), refuses hosts that resolve to this machine or a private network (loopback, private ranges,
link-local incl. cloud metadata, CGNAT, unique-local) and checks every redirect target the same way, and reads HTML
with ASCII-only lowercasing (lowercasing some non-ASCII letters changes their length and made slicing panic). DNS
rebinding between the check and the request is not caught. Cost: its definition, ~70 tokens a call, read from cache
after the first.

## 0.1.62 (2026-11-05): the own loop names its model, falls back within its route, and says why a turn failed

- The own loop's prompt starts with the model's name ("You are deepseek-v4.1-flash-1017, run by tazuna through
  subrouter.ai...", `TAZUNA_NO_IDENTITY` leaves it out). Under the application repo's CLAUDE.md ("This file provides
  guidance to Claude Code") DeepSeek answered "I'm Claude, made by Anthropic"; with the line it names itself. Without
  it, three tries each with that CLAUDE.md as the system prompt: znu "I'm Claude" 3/3, lingai (`@fast`) "I'm ChatGPT"
  3/3. Both serve DeepSeek's tokenizer, but lingai's calls carry ~30 more hidden tokens than znu's: about the length of
  a relay's default "You are ChatGPT, a large language model trained by OpenAI." The six-task A/B for side effects was
  stopped at the user's call after 4 of 12 runs (caddy and hugo-section-slug: same hidden tests, similar calls).
- A live own-loop session on a checked route gets the route's other providers as fallbacks (`TAZUNA_ROUTES`, each with
  its pinned key): SubRouter keeps a route's calls on one provider and does not forward a refusal. a6aa's content filter
  refused "when were you developed" twice (HTTP 403 `content_policy_violation`, also with no tazuna prompt at all) while
  znu in the same route answered; the session just failed.
- A failed turn shows why in the transcript (the result carried the reason, only "Failed" showed); known refusals in
  plain words.

DeepSeek's route is znu with a6aa as fallback now (stableai out: SubRouter sent it all 742 calls of a test round and it
cached 30%, 0-33% in the gauge rounds, against 99% for both).

## Grok 4.7 and Astra providers by tokenizer, 2026-11-05 (bench only)

The tokenizer slope (tokens a line of distinct text; `slope-grok-1105.txt`, `slope-astra-1105.txt`), after a user's
Grok 4.7 on a6aa described itself as "running on the current Grok codebase in this environment" (self-descriptions
prove nothing). Grok 4.7: xAI's own subscription pools 45.0 a line (with a ~1,240-token hidden prompt), miletechx,
kriora and relai 47.0 (hidden ~1,350, ~200, ~200), a6aa mixed (one call at 297 like kriora, one at 1,819 like
miletechx: two backends, both Grok-like); laoliu, stableai, quqiai timed out. Nothing points at another model; the
4% between xAI's pools and the API providers is under the 10% line. GPT-6 Astra: humanai and relai 43.0 (the GPT
tokenizer, with the 4.4k hidden prompt seen on Sol), model-share mixed, fxpayback 58.0 again: the same other model it
sells as Sol. Five Astra providers did not answer (403, 503, 524).

## 0.1.61 (2026-11-05): tazuna opens outside a git repo, in ask mode

A live session outside git refused to start ("not inside a git repo ... pass --allow-non-git"), a rule from the first
version: tazuna's default mode edits without asking, and git was the review and the undo. It now starts in ask mode
there (every edit asks first, as Claude Code does) and says why; shift-tab changes the mode, `--allow-non-git` keeps the
chosen one. Headless `--run` still needs git or the flag: nobody is there to answer.

## Sol and Luna against their public and first-party numbers, 2026-11-05 (bench only)

**Sol on HLE** (`canary_hle.py`, 100 of the 400 questions, xhigh, Luna judging): xcgx 45/100 (95%: 36-55%), fxpayback
29/100 (21-39%, 11 unanswered or ungraded) while writing ~3.6x the output tokens a question (median 5.7k against 1.6k).
Artificial Analysis lists Sol xhigh at 52.6%; our setup measured Sol at 41.5% on 200 questions on 2026-11-01, so xcgx is
Sol as we measure it and fxpayback is not: most likely a weaker model sold as Sol, which its price ($0.005/M input,
a twelfth of the OpenAI pools) also suggests. The easier canary (AIME 26/30) did not show it. Sol providers need the
HLE check before use.

Not chance: on the same 100 questions xcgx was right where fxpayback was wrong 19 times, the other way round 3 times
(exact McNemar p = 0.0009; on the 88 both answered, 15 against 3, p = 0.0075). And not Sol's tokenizer
(`fingerprint.py`): the same text costs fxpayback 56.8 tokens a copy against 42.0 on the OpenAI Pro 50 pool (+35%,
growing with the text, so a different tokenizer, not added text), and 6218 Responses input tokens against 4441 on xcgx,
Pro 50 and Pro 52 alike. A model's tokenizer is fixed, so fxpayback serves another model under Sol's name. (xcgx's Chat
route bills ~4.4k hidden tokens a call: a wrapper's prompt.)

The tokenizer by its slope (`slope-sol-1105.txt`: 2 and 10 distinct lines, tokens a line, so hidden prompts cancel): 43.0
a line on the OpenAI Pro 50, Pro 52 and Plus 51 pools (no hidden prompt), on kriora, a6aa, tokenx, lingai and humanai
(each with a ~4,400-token hidden prompt a call, billed as input), and on xcgx and cheaprouter for small calls (their
calls land on backends with and without that prompt); 58.0 on fxpayback alone. Every provider but fxpayback serves
Sol's tokenizer.

Since then: `routes.py refresh` measures every passing provider's tokens a line and leaves out one more than 10% off
most providers' (it dropped fxpayback in a test, kept the others, and names a hidden prompt over 1,000 tokens). Sol's
route is now xcgx with the OpenAI Pro 50 and Pro 52 pools (it was kriora and the pools): xcgx is Sol by tokenizer, HLE
and canary and had the top caddy mark with the fewest calls. A slip of mine: the probe keys were named by provider
only, so each new model's probe re-pointed the DeepSeek gauge's keys (kriora and a6aa answered Sol requests 403); keys
are named by model and provider now.

**Luna as OpenAI serves it** (`canary.py codex`: the canary through the official Codex CLI on the user's ChatGPT plan,
its own bench login): AIME 28/30, MMLU-Pro 88, against 27-29/30 and 87-89 on SubRouter's kriora, Pro 52 and Plus 51:
SubRouter's Luna is the real one. Codex itself on caddy (`codex@gpt-6-luna:xhigh`): 5/5, 4.0 min, 44 actions, Sol mark 8
against 7-9 for Luna through tazuna (`judge.luna4.sol.jsonl`).

## 0.1.60 (2026-11-05): a file that is gone does not open as an empty one

A changed file whose folder no longer exists (an agent's temporary git worktree, removed after its commit) opened in the
editor as a new, empty file at the old path, which read as "the agent wrote nothing". It now stays closed with a note
that it is gone, and the diff view marks it "gone"; its diff (what the agent wrote) still shows.

## GPT-6.1 Sol on SubRouter's providers, smoke test, 2026-11-05 (bench only)

Eleven providers probed on the Responses route at xhigh (`sol-probe-1105.jsonl`; `provsmoke.py` learned
`PROVSMOKE_API=responses`): all answered, first token 2.2 s (fxpayback, the cheapest at $0.005/M input) to 15.5 s. One
caddy run each on eight (`realwork/sol2.sh`, tazuna 0.1.55; fxpayback again on 0.1.58 after its `status` refusal), judged
by Sol with the Luna and DeepSeek caddy runs in one pool (`judge.three.sol.jsonl`; Sol judging Sol: read the Sol marks
with that in mind), the canary on three. $0.177 in all.

| Sol via | first token | caddy min | calls | cost | Sol mark | AIME | MMLU-Pro |
|---|---|---|---|---|---|---|---|
| kriora | 5.7 s | 3.3 | 29 | $0.021 | 7 | | |
| OpenAI Pro 50 | 7.5 s | 5.0 | 28 | $0.013 | 10 | 46 of 55 answered | (75 of 130 calls 503) |
| a6aa | 12.1 s | 5.2 | 32 | $0.008 | 8 | | |
| xcgx | 9.5 s | 6.5 | 25 | $0.014 | 10 | 28/30 | 88 |
| fxpayback (0.1.58) | 2.2 s | 6.8 | 40 | $0.004 | 10 | 26/30 | 87 |
| tokenx | 12.9 s | 11.8 | 37 | $0.042 | 9 | | |
| OpenAI Pro 52 | 13.7 s | 16.8 | 37 | $0.014 | 9 | | |
| cheaprouter | 7.2 s | 24.9 | 42 | $0.039 | 9 | | |

In the same pool DeepSeek averaged 8.9 (17 runs) and Luna 9.0 (3): on caddy the three models are level. Per run, Sol on
fxpayback costs about what DeepSeek on znu or o2oai does ($0.0025-0.004); Sol's shared pools are fast only when up (Pro
50 refused most canary calls). The harder-task comparisons with an Opus judge still favour DeepSeek (Sol 6.50 vs 7.00,
2026-11-01; Luna 4.88 vs 6.25, 2026-10-31). Candidates for a six-task round with an Opus judge after the weekly reset:
Sol on fxpayback and xcgx, Luna on kriora, against DeepSeek on znu / o2oai.

## 0.1.59 (2026-11-05): selecting text works like in a terminal; modals and clickable panels look it

- A drag past the transcript's top or bottom edge scrolls it (a line every 40 ms) and the selection grows with it.
- The selection stays on its text when the transcript scrolls (wheel, a drag at the edge, new output): it holds
  transcript lines, not screen rows. Before, a scroll moved the highlight onto whatever text was in those rows.
- cmd-c / ctrl-c copies everything selected, on screen or not; before, only what was visible.
- Behind any modal (a tool's calls, the model calls, diffs, tasks, rewind, sessions, models) the screen fades to the
  border colour, so the modal reads on its own (a terminal cannot blur).
- The clickable side panels show it: under the mouse the tokens and files panels take the accent border, a tool row
  gets a background, and the pointer turns into a hand. The hover readout of a call is set like the line under it
  (numbers bright, words faint), and the model-call list drops its cache-write column (the same numbers as "new in").

## 0.1.58 (2026-11-05): the own loop sends Responses items back without `status`

A Sol run on fxpayback (SubRouter's cheapest Sol provider) stopped at its 40th history item: 400 "Unknown parameter:
'input[40].status'". The model's output items now go back without the `status` field the API reports on them; OpenAI's
own API takes them either way. No metric change on the routes measured before.

## GPT-6 Luna on SubRouter's providers, smoke test, 2026-11-05 (bench only)

Six providers probed at xhigh (`luna-probe-1105.jsonl`): OpenAI Pro 50 and foxrouter answered 503; xcgx serves Luna
only without tools (it turns a Responses call with tools and a reasoning effort into a Chat call and refuses it: 400);
kriora, Pro 52 and Plus 51 work. Each of those three: the canary (30 AIME + 100 MMLU-Pro, xhigh) and one caddy run in
the own loop (`realwork/luna.sh`, `luna2.sh`, tazuna 0.1.55), judged by Sol in one pool with the 17 DeepSeek caddy runs
of 2026-11-04 (`judge.luna.sol.jsonl`). $0.019 in all.

| Luna via | first token | AIME | MMLU-Pro | caddy min | calls | out | Sol |
|---|---|---|---|---|---|---|---|
| kriora | 4.3 s | 29/30 | 89 | 3.8 | 18 | 3k | 8.0 |
| OpenAI Pro 52 | 13.6 s | 27/30 | 87 | 15.2 | 35 | 13k | 9.0 |
| OpenAI Plus 51 | 14.4 s | 27/30 | 87 | 15.5 | 36 | 12k | 8.0 |

In the same pool DeepSeek scored 9.0 on its official API and on o2oai, stableai, znu, getapi and model-share, 6-8 on
the rest (the pool rescored some DeepSeek runs a point or three from the provider pool the night before: one pass of
Sol moves that much). On this task and these checks Luna equals DeepSeek: the same canary marks (DeepSeek's good providers
26-30 and 86-90), the same caddy marks, and on kriora the fewest calls of any run (18). The shared pools are 4x slower
(14 s to the first token). Not a switch: caddy is easy, Sol is Luna's own family (a GPT judge), and on four harder
tasks with Opus judging Luna lost to DeepSeek 4.88 to 6.25 (2026-10-31). Luna on kriora is worth the six tasks with an
Opus judge.

## 0.1.57 (2026-11-05): the cockpit's side panels say more and open up

- **files**: lists every file the conversation wrote. Before, a file committed during the conversation (or written
  outside the repo) only showed as "no changes in the repo · 5 files written outside it", cut off, with nothing to open.
  Those files now get the agent's own diff (an edit's old text removed, its new text added; a write's content added), so
  a click on the panel opens all of them, git's and these, in the diff view; enter or a click opens one in the editor.
- **tools**: each tool's total and average time a call (`Agent ×14 ✓ 30.7m ⌀2.2m`). A click on a row lists that tool's
  calls: number, how long each took, how it ended, what it did; the title has the total, the average and the longest.
- **tokens**: the bar chart of output per model call has a y axis (its tallest bar's value, and 0) and says which calls
  it shows (`#12–40`). The mouse over a bar names that call (output, time to first token, tokens a second) and colours
  it; a click lists every model call with its new and cached input, cache writes, output, first token and speed.

## 0.1.56 (2026-11-05): a short command runs; long prompts open

- `/res` and enter runs `/resume`, the first suggestion under the input, as in Claude Code. Before, tazuna stopped with
  "no command /res: did you mean /resume?" and sent nothing. A full command name is never swapped for a longer one.
- A prompt longer than eight lines ("… 17 more lines") opens in full on a click, like tool calls, and with ctrl+o for
  every block; the line now says "click to show".

## Search tools after the prompt fix, o2oai on six tasks: stopped, 2026-11-05 (bench only)

`realwork/ts2.sh` (route stableai + znu with and without `--search-tools`, o2oai pinned; six tasks x two runs, tazuna
0.1.55) started 10:25 CET and was stopped by hand at 10:46: the providers cached 8-14% of the input (stableai 2.2M of
15.4M prompt tokens, o2oai 0.9M of 10.5M) against 90% in the route check at 00:30 and ~95% on o2oai's caddy run at 23:05
the night before, so runs cost 5-8x the estimate (a list-price run of hugo-template-return: $0.070). $0.35 spent for five
finished runs; no result. Next try in the evening, behind a cache check (provsmoke.py on the arm's providers first).

## GPT-6.1 Sol as a stand-in for the Opus judge, 2026-11-05 (bench only)

Four pools Opus 5.5 xhigh had judged (DeepSeek providers, Kiro, Sonnet lead / Opus low, boss test scope: tasks/, 114
runs) judged again by GPT-6.1 Sol xhigh through SubRouter, same calls and shuffles (`realwork/solcal.sh`, compared with
`realwork/solcmp.py`). Then the caddy runs of the 13 SubRouter providers and DeepSeek's API, Sol only. $0.095 in all,
no Claude usage. One Sol call hung for 20 minutes without an answer (the first pool's k6 task, one candidate): restarted
without it.

| Pool | Runs | Opus mean | Sol mean | Pearson r | Decided pairs in the same order | Arm order |
|---|---|---|---|---|---|---|
| DeepSeek providers | 21 | 7.05 | 7.52 | 0.89 | 31 of 31 | same |
| Kiro vs tazuna | 34 | 6.28 | 7.15 | 0.76 | 44 of 56 | same (Kiro Opus 7.04/7.83, tazuna 6.83/7.33, Kiro Haiku 4.88/6.08) |
| Sonnet lead, Opus low | 35 | 6.20 | 6.63 | 0.87 | 57 of 58 | same (7.54/7.92, 5.79/6.42, 5.08/5.42) |
| boss test scope | 24 | 6.77 | 7.42 | 0.53 | 23 of 24 | Opus 6.92 vs 6.62 (noise), Sol 7.42 each |
| all | 114 | | +0.61 | 0.80 | 155 of 169 (92%) | |

Every decision these pools carried comes out the same under Sol: Sol can judge relative comparisons instead of Opus,
at about $0.02 a six-task pool and none of the weekly limit. Its scores run ~0.6 higher (more at the low end), so one
comparison keeps one judge.

The provider caddy runs (Sol): 9.0 for DeepSeek's API (three of four runs), o2oai, stableai, bangai, getapi, tokenx,
lingai, a6aa, kriora and model-share; 8.0 for znu, x-loop, tokenus-net, ydjs-ai and one API run. No provider writes worse
code on this task (every run passed the hidden tests); providers differ in time, calls and caching, not quality here.

## SubRouter's DeepSeek providers, smoke test, 2026-11-04 (bench only)

DeepSeek V4.1 Flash 1017 at xhigh through 17 of SubRouter's 28 providers (the cheapest by listed price, without
Cybersol), one pinned key each (`tazuna-route-smoke-<slug>`). First `bench/provsmoke.py`: four growing streamed calls
per provider (~15k tokens of context, a bash tool), all at once: time to first token, output tokens a second, cached
share of the warm calls, reasoning tokens. quqiai (404), manxueai (503) and kiyara (500) answered nothing. Then one
caddy-quoted-braces run per remaining provider (`realwork/provsmoke.sh`, tazuna 0.1.54 frozen, seven at a time). One run
each: a screen, not a ranking. Spent $0.15 in all.

```
provider      ttft  tok/s cache  reas |   min calls s/call uncached   out tests  billed
o2oai         2.56  198.8   66%  1380 |   5.0    29   10.4      58k   24k 5/5   $0.0032
stableai      3.58  172.1   66%   317 |   5.2    31   10.1     732k   24k 5/5   $0.0114
znu           2.76  159.7   99%   536 |   6.7    38   10.6      55k   27k 5/5   $0.0025
x-loop        3.04 196.15   66%  1110 |   7.4    34   13.0      62k   36k 5/5   $0.0049
bangai        2.63 158.45   97%   629 |   7.4    61    7.3     337k   23k 5/5   $0.0102
getapi        2.77 136.25   99%  1077 |   8.0    44   10.9     104k   32k 5/5   $0.0072
tokenx        2.51 180.65   66%   566 |   9.3    40   14.0      68k   33k 5/5   $0.0072
lingai        3.57 144.55   99%   739 |   9.8    52   11.3     221k   33k 5/5   $0.0100
a6aa          2.62 159.15   65%     0 |  11.6    74    9.4     739k   30k 5/5   $0.0108
tokenus-net   3.32 1063.6   20%  1082 |  11.7    53   13.3    1834k   69k 5/5   $0.0434
ydjs-ai       4.83  230.5    2%   165 |  22.9    82   16.8    1180k   27k 5/5   $0.0240
kriora         2.5  177.7   75%     0 |  23.5   111   12.7     881k   47k 5/5   $0.0126
model-share   4.21 141.35   99%   650 |  33.3    70   28.5      58k   33k 5/5   $0.0055
relai         4.23  133.9   99%  1792 | failed mid-run, billed $0.0010
billed total 0.154
```

(probe: ttft and tok/s are medians of four calls, cache the warm calls' cached share, reas the reasoning tokens of all
four; run: minutes, model calls, uncached input, output, hidden tests, billed by SubRouter for probe and run.)
DeepSeek's own API on the same task: 2.5-4.1 minutes, 22-41 calls, 38-79k uncached, about 5 s a call, $0.017-0.025.

- Every provider is about twice as slow per call as DeepSeek's own API (7-14 s, model-share 28 s), with similar first
  tokens (2.5-4.8 s) and generation speed in the probe (130-230 tok/s; tokenus-net's 1,064 is a buffered burst).
- Calls make the difference between providers. Kriora and a6aa returned no reasoning at all at xhigh and took the most
  calls (111, 74), the pattern bob-team showed on 2026-11-04 (twice the official API's calls).
- Poor caching makes a provider slow and dear: ydjs-ai (2% cached) and tokenus-net (20%) cost 10-17x znu.
- Best of this screen: o2oai (5.0 min, 29 calls, $0.003), znu (6.7 min, 38 calls, 99% cached, $0.0025), stableai
  (5.2 min, but 732k uncached), x-loop (7.4 min, $0.005). relai dropped out mid-run (503 "no available channel"), as
  bob-team did earlier the same day: the route's only provider is gone from the list.

## 0.1.55 (2026-11-04): the own loop sends DeepSeek its earlier reasoning on DeepSeek's own API

On `https://api.deepseek.com` the model's earlier reasoning now goes back with the history by default (elsewhere still
only with `TAZUNA_SEND_REASONING`: some providers keep and bill it, one run went past its context limit, 0.1.0). Measured
in the next section, four tasks with both arms:

| | Judge | Minutes | Calls | Uncached | Cost |
|---|---|---|---|---|---|
| reasoning dropped (0.1.54) | 7.25 | 6.4 | 42 | 102k | $0.033 |
| reasoning sent back (0.1.55) | 7.12 | 6.5 | 48 | 56k | $0.029 |

Less uncached input on all four tasks, lower cost on three; judge and time within noise. Better on one metric and worse
on none, so it is the default where it was measured.

## DeepSeek's official API against SubRouter's route, judged, 2026-11-04 (bench only)

Follow-up on the section below: the other five tasks of tasks/ on the official API, one run per arm, 22:14 CET
(off-peak). The balance guard stopped the round at ¥3.25 spent (¥0.33 of the granted credit left): k6-run-once in both
arms and hugo-template-return with reasoning back were killed in flight. Judge: Opus 5.5 xhigh, one call per task with
every arm's diffs side by side (6 calls, 2% of the 5h window); SubRouter's runs are the baseline arm of 2026-11-04
(0.1.51, own-loop code identical to 0.1.54). Averages of per-task means.

| Arm | Tasks | Judge | Minutes | Calls | Uncached | Cost |
|---|---|---|---|---|---|---|
| SubRouter route | 4 | 7.00 | 16.7 | 92 | 597k | $0.015 |
| official API | 4 | 7.25 | 6.4 | 42 | 102k | $0.033 |
| official API, reasoning back | 4 | 7.12 | 6.5 | 48 | 56k | $0.029 |
| SubRouter route | 5 | 6.60 | 20.1 | 105 | 844k | $0.020 |
| official API | 5 | 6.60 | 8.4 | 51 | 120k | $0.042 |

(4 tasks: caddy-quoted-braces, hugo-section-slug, prometheus-duration-parens, terraform-import-foreach; 5 adds
hugo-template-return, where both arms broke the package suite.) Same quality (differences far inside ±0.75), the
official API 2.5x faster with half the calls at twice the cost. Per call both take about 9-11 s: the time goes into
SubRouter's extra calls, with the same model name and the same prompt, so the route behaves like a different serving of
the model. Reasoning sent back: same score and time, 45% less uncached input (every task), 12% less cost: default on DeepSeek's
own API from 0.1.55.
On the caddy task the DeepSeek Harness scored 8.5, the own loop 8.0 and 8.5 (reasoning back).

## The DeepSeek Harness's levers in the own loop, 2026-11-04 (bench only)

DeepSeek V4.1 Flash 1017 (xhigh) through SubRouter's route, tasks/ (six Go tasks), 0.1.51, three arms run together from
13:42: baseline, `--search-tools` (grep and glob tools, bash told not to search or read), and both plus `--batch-own`. The
route's only provider (bob-team) dropped out at 16:02 ("no available channel"): six second runs failed and four were
stopped; 26 valid runs, every task in every arm at least once. Averages of per-task means; no judge (hidden tests only).

| Arm | Minutes | Calls | Uncached | Cached | Output | Cost | Hidden |
|---|---|---|---|---|---|---|---|
| baseline | 22.8 | 108 | 838k | 8.0M | 85k | $0.021 | 33/39 |
| search tools | 19.9 | 103 | 848k | 7.1M | 77k | $0.020 | 30/39 |
| search tools + few calls | 27.8 | 132 | 925k | 11.5M | 109k | $0.027 | 27/31 |

The tools save 13% of the time and 5% of the calls, inside run-to-run noise, with three fewer hidden tests; the
few-calls rule makes runs longer (+22% calls). Neither explains the Harness's two-times speed: both stay opt-in. SubRouter
spend today: $0.99 (company tickets $0.46, this $0.53).

One flaw in this test: the system prompt still said "Search with grep -rn in bash" while `--search-tools` was on, so
the tools arm got two contradicting instructions. Fixed after the test (the prompt now names the grep and glob tools when
they are on); the tools are not measured again yet.

## Kiro against tazuna on the same models, 2026-11-04 (bench only)

Kiro CLI 2.37.1 (AWS; stock settings, web search and fetch not trusted, `kiro-cli chat --no-interactive --output-format
stream-json`) on the user's Kiro Pro credits, tasks/ (six Go tasks) x two runs, 14:57-15:47, against tazuna's Opus default
of Nov 3 (test scope, v1333-v1344); one judge pool per task, two passes (12 calls).

| Harness + model | Judge | Hidden | All passing | Minutes | Cost / run |
|---|---|---|---|---|---|
| tazuna + Opus 5.5 medium | 6.83 | 43/48 | 9/12 | 5.7 | $0.58 at API prices (plan usage) |
| Kiro + Opus 5.5 medium | 7.04 | 44/48 | 9/12 | 6.6 | 18.8 credits (~$0.38 on Pro) |
| Kiro + Haiku 5.5 medium (n=10) | 4.70 | 27/39 | 3/10 | 4.7 | 1.1 credits (~$0.02) |

Same model, same quality: Kiro +0.21, inside noise; per task each wins some (Kiro k6 8/8 vs 6/7, tazuna hugo-template
6/8 vs 5/5). 237 credits used in all (one Haiku run lost to a Kiro login refresh, one without a result). Haiku stays
too weak as the main model, as on tazuna (its 1.1 credits a run are the draw).

## 0.1.54 (2026-11-04): links react to the mouse

- A link under the mouse is drawn reversed, and the pointer turns into a hand over it (OSC 22, in terminals that take
  it: kitty, Ghostty, foot, WezTerm, xterm; others ignore it). Before, a clickable link looked like plain text until
  clicked. The pointer goes back to normal on exit.

## 0.1.53 (2026-11-04): files open in the editor; no more guessed paths

- The diff view (click the files panel, or `/diff`): Enter, `o` or a click on a file opens it in the editor at its first
  change. The editor: TAZUNA_EDITOR (a template, e.g. `code -g {path}:{line}`), else the first of Cursor, VS Code, Zed,
  Windsurf, Sublime Text or a JetBrains IDE on PATH, else the system's default app.
- The files panel without repo changes said nothing useful: it listed every path in the agent's commands (grep patterns,
  `/tmp` scripts, `{f}.json`). It now says "no changes in the repo yet", or, when an edit tool wrote files elsewhere,
  how many and which. A run's summary counts the repo's changed files (git), else the files an edit tool wrote.

## 0.1.52 (2026-11-04): notices when something changed underneath

- When claude starts in a live session, tazuna says, once: Claude Code updated since the last session ("Claude Code
  updated: 2.1.314 → 2.1.318"; it updates itself and tazuna runs whichever `claude` is installed), a Claude Code older
  than tazuna needs (2.1.292: `claude update`), a model the picker does not list yet, and a tazuna binary rebuilt since
  the window opened ("restart it to use the new build": an open window keeps the old build).
- 0.1.51 (own loop, under measurement): `--search-tools` (grep and glob tools; bash told not to search or read) and
  `--batch-own` (independent tool calls in one response), from DeepSeek's own harness.

## DeepSeek on the company tickets; DeepSeek Harness on one Go task, 2026-11-04 (bench only)

tazuna's own loop with DeepSeek V4.1 Flash 1017 (xhigh, SubRouter route tazuna-route-deepseek-v4-1-flash-1017, no
reviewer, no hand-over, 0.1.50) on the ten company tickets, one run each (00:04-12:12; route check, runs: $0.46 of the
user's $1.25 stop). Judged in each batch's pool (Opus xhigh, two passes, 20 calls) and checked against the reviewers'
findings (10 calls); the week moved 74% to 76%.

| Setup | Batch 1 judge | Batch 2 judge | Findings repeated | Minutes | Cost / ticket |
|---|---|---|---|---|---|
| Opus default (Nov 3) | 6.45 (n=10) | 5.70 (n=10) | 73% | 15.8 | $2.24 |
| DeepSeek own loop | 5.40 (n=5) | 5.10 (n=5) | 71% (34 of 48) | 61.4 | $0.048 |
| the user's / the author's first version | 5.90 | 6.62 | - | - | - |

DeepSeek alone is about 0.8 points below Opus at 2% of the cost and four times the time; it repeats as many review
findings. Its two failures are the hardest tickets (t-973 Ledger corrections 3, t-1106 stuck runs 3 with three
new type errors and eight failing tests: the one run with regressions); it beats Opus on b-188 (7 vs 5.5). The
hand-over to Opus on failing tests (the default with a provider, off here) would have caught t-1106.

DeepSeek Harness (`dsh` 0.2.1-rc.5, `dsh headless --json`, official API, deepseek-flash = V4.1-Flash-1017, off-peak)
against tazuna's own loop on SubRouter's DeepSeek and the Opus default, caddy-quoted-braces, two runs each, one pool
(2 calls): Harness 8.0 / 8.5 in 3.3 minutes, 44 steps, ~$0.033 (¥0.24) a run; own loop 8.0 / 9.0 in 6.3 minutes, 66
calls, ~$0.006; Opus 7.5 / 7.5. All 5/5 hidden tests. The Harness sends a quarter of the own loop's uncached tokens (59-75k
vs 193-308k): at the same price list it would be the cheaper of the two. Its web tools fetched this task's fix PR on the
first try (run discarded); the bench profile now disables them (`~/bench-homes/dsh/.dsh/profiles/headless/cordis.patch.yml`).
One small task: a lead to study (how it saves calls and uncached tokens), not a result.

## 0.1.50 (2026-11-03): `--history`; the reviewer gets the lessons too (both opt-in, under measurement)

- `--history`: claude is told to read why a file is the way it is before changing it (`git log` of the file, whose commit
  messages often carry the pull request's description, and `git blame` of the lines it changes). Only where the repository
  has more than one commit. Aimed at the gap to the human authors on company tickets: what they knew from earlier work.
- With `--lessons` and `--review-model`, the reviewer checks against the repository's lessons as well.
- Bench: history copies of the ten company-ticket templates (`bench/company/mkhist.sh`: the real history up to each base,
  fetched by hash, nothing merged later), per-task lessons from PRs merged before each base day (`mklessons.sh`), arms
  `tazuna-lsn`, `-hist`, `-rev` (Opus medium five-lens reviewer), `-lhr` (all three). The test runs after the weekly reset.

## 0.1.49 (2026-11-03): review lessons kept out of the repository's reach

- From an automated security review of 0.1.48: a repository could ship its own `.tazuna/review-lessons.md`, and on a
  public repository anyone may comment on a pull request; either way text would reach the agent's instructions. The list
  now lives in tazuna's data folder (`~/.local/share/tazuna/lessons/OWNER__NAME.md`), never in the working tree; only
  comments from the repository's owner, members and collaborators are used; the agent gets it between `<review-lessons>`
  tags as a checklist, told to ignore anything in it that asks for something else. Writing goes through a new private file
  and a rename, never through a symlink. A benchmark points `TAZUNA_LESSONS_FILE` at a task's own list.

## 0.1.48 (2026-11-03): `tazuna lessons` and `--lessons` (opt-in, under measurement)

- `tazuna lessons [--prs N] [--before YYYY-MM-DD]`: reads the review comments of the repository's last N merged pull
  requests on GitHub (through `gh`; bots, nits and approvals left out), and one `claude -p` call (Sonnet medium) groups
  what reviewers raised in at least two pull requests into a checklist of up to 20 points (what goes wrong, how to check,
  which PRs), written to `.tazuna/review-lessons.md` (moved to tazuna's data folder in 0.1.49). `--before` keeps a benchmark task from learning from its own review.
  Nothing in it is about one repository: the company repo (551 comments, 150 PRs, 70 s) gave points on side effects
  after a commit, Ledger echoes, migrations' `down()`, retry budgets; caddy (145 comments) on bodyless responses,
  deadlines, TLS policies in the Caddyfile adapter. A repo without review history gets nothing.
- `--lessons`: the list goes into the agent's instructions. Measured after the weekly reset: with and without, scored by
  the reviewers' findings on both company batches and the judge, with lessons only from PRs merged before each task.

## Company tickets, batch 2, 2026-11-03 (bench only)

Five PRs by other authors (t-973 #1198 Ledger corrections, b-188 #1190 duplicate documents, t-1106 #1375 stuck
scheduled runs, t-986 #1167 signing for an unknown bank, t-1039 #1309 a visitor's plan on arrival), today's default
(0.1.47) x two runs (21:18-22:17), judged against the merged PR, two passes (10 calls; ~2% of the window).

| Ticket | Judge (two runs) | Minutes | Cost | Files (PR) |
|---|---|---|---|---|
| t-1039 | 7, 6 | 16.7 | $1.34 | 10 (37) |
| t-986 | 6, 6 | 7.2 | $0.49 | 2 (15) |
| t-1106 | 6, 5 | 18.7 | $1.78 | 7-10 (8) |
| b-188 | 5, 6 | 13.9 | $1.76 | 12-14 (31) |
| t-973 | 5, 5 | 22.4 | $3.19 | 10-12 (26) |
| **mean** | **5.70** | **15.8** | **$1.71** | |

No new type errors or unit-test failures in any run. Weakest dimension: edge cases (2-3 of 5), where the reviewers' round-1
findings sit too. Against the human authors (same evening, 8 + 4 judge calls): each PR's first reviewed commit (the author's own version
before review, as a diff, its tests not run; t-1039 left out: its branch sat on three stacked PRs) judged in the
pool with tazuna's two runs: authors 6.62 (n=4) against tazuna 5.19 (n=8), ahead on all four tickets (t-973 7.5 vs
4.25, t-1106 7.0 vs 5.5, t-986 6.5 vs 6.0, b-188 5.5 vs 5.0). The authors' versions share the merged PR's design,
which the judge compares against, so the gap is partly that. Reviewer findings (`fcheck.py`, Opus xhigh decides per
finding and patch: present, avoided, or not applicable; round 1 and later rounds, 48 findings x 2 runs): of the 66
that apply, tazuna's versions repeat 50 (76%) and avoid 16; Critical 2 of 4, Major 19 of 28, Minor 29 of 34 present.
tazuna's first version carries most of what reviewers found in the humans' first versions: the review step, not the
first draft, is where these defects are caught.

Preview of a review step (batch 1's 19 round-1 findings, `fcheck.py`, 5 calls): October's boss runs with the in-loop
five-lens review (0.0.51, one run a ticket) repeat 59% of the applicable findings, the same boss runs without it 53%,
today's default 68% (two runs a ticket). No sign that the review removed defects; but 3 of those 5 reviews ran before
boss mode's workers had finished (a bug fixed afterwards), so the preview cannot settle it. A clean test waits for the
weekly reset (Nov 7).

## Company tickets, batch 1, 2026-11-03 (bench only)

Five of the user's company PRs (acme/app, TypeScript: t-838, -841, -842, -845, -851) with a peer's
round-1 findings, today's default (0.1.47: repo profile, no boss, Opus 5.5 medium, Haiku explorer at medium, test
scope) x two runs (17:24-18:07), judged in one pool per ticket with October's tazuna (0.0.50, `--run`, so boss mode) and
the user's own first round (Claude Code, Opus high, interactive, with the user's guidance). Opus xhigh judge against the
merged PR, two passes, diffs up to 120k characters (10 calls; the window did not visibly move).

| Setup | Judge | Cost / ticket | Minutes | Calls | Cache read | Output |
|---|---|---|---|---|---|---|
| tazuna today (no boss) | 6.00 (n=10) | $2.77 | 15.8 | 52 | 5.47M | 45.6k |
| tazuna October (boss) | 5.80 (n=5) | $1.78 | 36.2 | 13 (lead) | 0.66M | 15.5k |
| the user's own round 1 | 6.20 (n=5) | n/a | n/a | n/a | n/a | n/a |

All three within noise. No run added type errors or unit-test failures (t-842 two fewer type errors). Per ticket the
order changes (the user leads on 838, 841, 842; tazuna on 845, 851). Today's default is 2.3x faster than October's boss
mode at 56% more cost: the same trade as on the Go tasks. Tazuna writes more tests (tests dimension 4.25 vs 2.9).

## Test scope on the hard tasks; Sonnet 5.5 and Opus 5.5 low as the lead, 2026-11-03 (bench only)

tasks-hard (four tasks x two runs per arm, arms interleaved, 12:57-15:11, 0.1.46), the default config (repo profile, no
boss, Opus 5.5 medium), with and without test scope. Opus xhigh judge, one pool per task, two passes (8 calls).

| Setup | Judge | Hidden | Cost / run | Minutes | Calls |
|---|---|---|---|---|---|
| test scope (default) | 6.25 | 12/14 | $1.20 | 22.9 | 23.1 |
| `--no-test-scope` | 6.31 | 12/14 | $1.49 | 45.8 | 24.6 |

Same score and hidden tests, 19% cheaper, twice as fast: test scope holds where package tests take longest (same hour,
so the time gain is clean). It stays the default.

tasks/ (six tasks x two runs per arm, 15:11-15:38, 0.1.47: the explorer at medium whatever the lead's effort), the lead
swapped, against this morning's default (Opus medium with test scope, v1333-v1344); one pool per task with all three,
two passes (12 calls). One Sonnet run (hugo-section-slug) searched the web and is left out (leakscan).

| Lead | Judge | Hidden | Cost / run | Minutes | Calls |
|---|---|---|---|---|---|
| Opus 5.5 medium (default) | 7.54 | 43/48 | $0.58 | 5.7 | 20.0 |
| Sonnet 5.5 medium (n=11) | 5.18 | 35/48 | $0.26 | 4.9 | 15.9 |
| Opus 5.5 low | 5.79 | 35/48 | $0.38 | 4.4 | 15.4 |

Both cheaper leads lose well beyond noise (-2.36 and -1.75, 8 fewer hidden tests each): they stop sooner (16 calls
against 20) and miss parts of the task. Opus medium stays the lead. (The default's 7.54 here against 6.92 in its first
pool: scores move with the pool; only comparisons inside a pool count.)

## 0.1.47 (2026-11-03): the default's explorer at medium effort

- Without boss mode, the Haiku explorer (and the opt-in tester) had no effort of their own and ran at the session's: at
  xhigh in a session set to xhigh. They now run at medium, as the boss's workers do since 0.1.36 (Haiku xhigh cost 2.3x
  medium there, no better judge). Benchmarks run Opus at medium, so their numbers do not move; live sessions above
  medium get a cheaper, faster explorer.

## 0.1.46 (2026-11-03): test scope only where tazuna can test the repo

- The test-scope rule tells claude that tazuna runs the changed packages' tests at the end. It is now added only when
  verify finds a way to test the repo (a Go module, a Cargo crate, Python tests, a package.json test script at the root
  or in a package; or a `--verify` command). Before, a repo without one (a JS app without a test script, Java, C#) was
  told tests would run, and none did.

## `--boss-escalate` on tasks/, 2026-11-03 (bench only, not judged)

Six tasks x two runs (0.1.45, v1357-v1368, 11:46-12:14) against this morning's boss runs with test scope (v1345-v1356).
The lead never called the Sonnet worker: second implementer attempts (4 runs) went to Haiku again, and failing tests sent
back by verify (the same tasks fail them in the baseline too) went to the tester, which found them unrelated. Cost $0.47
vs $0.45, 11.4 vs 10.8 minutes, hidden 42/48 vs 44/48: the same setup in effect, so not judged (12 calls saved). A rule
the lead may follow is not enough; next: the boss hook sends a second attempt at the same change to Sonnet.

## 0.1.45 (2026-11-03): `--boss-escalate` (opt-in, under measurement)

- Boss mode with a fourth worker, `implementer-strong` on Sonnet, that only redoes what the Haiku implementer got wrong
  (its checks still fail, the tester reports failures it caused, or verify sends back failing tests); never a first
  attempt. Workers at the boss's effort (medium). Redo runs on Haiku alone scored 5.83 against 7.04 for first-time
  successes; with Sonnet available, 6.75.

## 0.1.44 (2026-11-03): cache life from the request's start

- The cache's life runs from the start of the request that read or wrote it (Anthropic's docs), not from the reply's
  end: the countdown now counts from the latest call's start, also while it runs. Only 5 minutes and 1 hour exist.

## Test scope in boss mode, 2026-11-03 (bench only)

tasks/ (six Go tasks) x two runs, boss with Haiku workers at medium, Opus 5.5 medium leading; 0.1.42 (the workers' rule,
v1345-v1356, 11:01-11:26) against the 2026-11-02 boss runs without test scope (0.1.32, v1257-v1290 bhmed, 17:00). Opus xhigh
judge, one pool per task, two passes (12 calls). No run saw the fix.

| Setup | Judge | Hidden | Cost / run | Minutes | Implementer minutes | Whole-package test minutes | Lead cache misses |
|---|---|---|---|---|---|---|---|
| boss, no test scope | 6.92 | 45/48 | $0.45 | 15.2 | 10.1 | 5.2 | 0.50 |
| boss, test scope | 6.62 | 44/48 | $0.45 | 10.8 | 5.4 | 0.6 | 0.42 |

Judge inside noise (-0.30), same cost, 29% faster: the workers' whole-package test runs fell from 5.2 to 0.6 minutes, the
implementer's time halved. The lead still loses its cache about as often (waits stay over five minutes on some runs), so
cost is unchanged. Kept as the default (0.1.42). Boss is still 1.9x slower than the default and 22% cheaper on tasks/.

## 0.1.43 (2026-11-03): the last call's cache share; how long the cache stays warm

- Context panel: "cache read 2.28M 92% · last 98%": the session's share read from the cache and the latest call's (what
  Claude Code's cache meters show; one call after a short exchange reads ~99%, a session less, since each call writes
  what is new once). Same formula: read / (read + written + uncached).
- "cache life 54:09 of 1h": time until the cache expires (each call renews it; its life, 5 minutes or 1 hour, comes from
  what claude reports writing); "expired" in warning colour after that, when the next call writes the whole context
  again. Claude path only.

## 0.1.42 (2026-11-03): test scope for the workers too

- With test scope on (the default), the workers that run tests (implementer, implementer-strong, tester) are told to run
  only the tests covering the change; the explorer runs none. The lead already had the rule; in boss mode it runs no
  commands, and its implementer spent most of its 10 minutes a run in package-wide tests while the lead waited.

## Where cost and time sit, per role, 2026-11-03 (analysis of stored runs, no new runs)

Opus 5.5 medium lead; default = repo profile with test scope (v1333-v1344, tasks/); boss = Haiku medium workers (0.1.32,
before test scope: tasks/ v1257-v1290, tasks-hard v1317-v1331); own loop = `reviewed` preset (DeepSeek 1017 max on
SubRouter, Opus medium reviewer, v661-v679, 7 runs). Worker cost split by their reported tokens; time from each run's events.

| Config | Cost / run | Cost by role | Minutes / run | Time by role |
|---|---|---|---|---|
| default, tasks/ | $0.58 | Opus lead 98%, explorer 2% | 5.6 | lead's tools 2.4, lead's calls ~2, explorer 1.0, verify 0.5 |
| boss, tasks/ | $0.45 | lead 82%, implementer 9%, explorer 7% | 14.9 | implementer 10.1, tester 1.7, explorer 1.5, lead 1.0, verify 0.8 |
| boss, tasks-hard | $0.92 | lead 57%, implementer 26%, explorer 16% | 46.1 | implementer 35.9, verify 6.8, tester 2.9, explorer 1.6 |
| own loop, reviewed | $0.23-0.52 | Opus reviewer ~95%, DeepSeek ~5% | 19-50 | DeepSeek 19-45, review 1-1.5, fix round 3-16 |

- The expensive model is the cost and the cheap one the time, in every config.
- Worker reports are small: the lead re-reads them for 2-5% of its cache reads. Shorter reports are not a lever.
- The boss's lead loses its cache while it waits: workers run past the 5-minute cache life (gaps of 6-85 minutes), so
  0.5 (tasks/) to 1 (hard) of its calls a run rewrites the whole context: 30% / 48% of its cache writes, and writes are
  51-56% of the lead's cost (output 35-36%, reads 10-13%: Opus 5.5 reads cost 0.05x the input price, writes 1.25x for 5
  minutes and 2x for an hour). The 1-hour cache pays only when over 37.5% of the writes are such rewrites: a little dearer
  on tasks/, a little cheaper on the hard tasks. Faster workers (test scope for them) shrink the waits instead.
- Boss mode ran before test scope; the default with it is now 2.7x faster than boss at 29% more cost on tasks/.

## 0.1.41 (2026-11-03): no more "6684 tok/s"

- Claude sometimes hands a finished reply over in one burst (66 stream deltas in 57 ms after the text was written); its
  speed read 6684 tok/s. A Claude call faster than 1,000 tokens a second now counts as handed over at once: its first
  token's time is unknown and its speed runs from the call's start (a lower bound).

## 0.1.40 (2026-11-03): test scope on by default

- Claude runs only the tests that cover its change while it works; tazuna's verify runs the changed packages' tests when
  it finishes (measured below: same judge, -17% cost, 40-58% faster). `--no-test-scope` turns it off; left out when
  verify is off. Bench arms built on the default (`tazuna-rtall` and the like) include it from 0.1.40 on.

## `--test-scope` against the default, 2026-11-03 (bench only)

tasks/ (six Go tasks) x two runs, Opus 5.5 medium, the repo profile without boss mode; `--test-scope` (0.1.39, v1333-v1344,
09:58-10:15) against the 2026-11-02 all-tools runs (0.1.24, v1209-v1231 odd, 13:19). Opus xhigh judge, one pool per task,
two passes (12 calls). No run saw the fix.

| Setup | Judge | Hidden | Cost / run | Tokens sent | Calls | Minutes | Test-command minutes (whole package / scoped) |
|---|---|---|---|---|---|---|---|
| default | 6.58 | 42/48 | $0.70 | 0.94M | 21.9 | 13.6 | 4.3 / 2.7 (3.4 / 3.3 runs) |
| `--test-scope` | 6.92 | 43/48 | $0.58 | 0.83M | 20.0 | 5.7 | 0.2 / 1.5 (0.8 / 4.0 runs) |

Judge inside noise (+0.33), cost -17%, tokens -12%, time -58%. 5.3 of the 7.9 minutes saved are test commands measured
inside the runs; the rest is likely the hour (the baseline ran in the afternoon, when the API is slower) and machine load,
so the time gain is between ~40% and 58%. Dominating on these tasks; only Go tasks and a baseline from another day.

## 0.1.39 (2026-11-03): `--test-scope` (opt-in, under measurement)

- `--test-scope`: claude is told to run only the tests that cover its change while it works, not whole packages or the
  repository; tazuna's verify already runs the changed packages' tests when it finishes and sends back failures. On
  2026-11-02's no-boss runs (tasks/), package-wide test runs took 5.0 of 13.6 minutes a run; in boss-pick, Sonnet's
  whole-suite runs were most of its extra time. Left out when verify is off.

## Boss-pick against fixed medium workers, 2026-11-03 (bench only)

`--boss-pick` (0.1.37, frozen tz-v137) against boss mode with Haiku workers at medium (`--boss-effort medium`), Opus 5.5
medium leading, the repo profile. tasks/: six tasks x two runs, against the 2026-11-02 medium runs (0.1.32, v1257-v1290);
tasks-hard: four tasks x two runs per arm, both arms run together (v1317-v1332, 22:30-01:33). Opus xhigh judge, one pool
per task, two passes (12 + 8 calls; the 5-hour window moved ~5% for the judge). No run saw the fix (leakscan).

| Set | Setup | Judge | Hidden | Cost / run | Minutes | Opus calls |
|---|---|---|---|---|---|---|
| tasks/ | medium workers | 7.00 | 45/48 | $0.45 | 15.2 | 8.8 |
| tasks/ | boss-pick | 6.62 | 43/48 | $0.69 | 16.0 | 8.6 |
| tasks-hard | medium workers | 6.19 | 11/14 | $0.92 | 46.2 | 8.8 |
| tasks-hard | boss-pick | 7.12 | 12/14 | $1.89 | 61.7 | 9.9 |

What the lead chose (21 boss-pick runs): testers at low (18 of 20 calls), explorers at medium (20 of 25), never xhigh or
max; the main change went to `implementer-strong` (Sonnet) in 17 runs, to the Haiku implementer in 4. So the test
measures Sonnet doing the implementing more than the effort picks. On tasks/ that costs 53% more for a score inside
noise (-0.38). On tasks-hard +0.93, just over the noise line at n=8, at 2.1x the cost and 34% more time; the gain is
mostly one task (prometheus-replay-mmap 6.5 vs 3.0; caddy-tls 5.5 vs 6.0, hugo-eq 8.5 vs 8.5, prometheus-wbl 8.0 vs 7.25).
A trade, not a dominating change: `--boss-pick` stays opt-in.

Where the cost and time went (per run, from each run's per-model usage and subagent timings): the Opus lead cost the same
($0.47 vs $0.52 hard, $0.38 vs $0.37 tasks/): picking is free. The gap is the Sonnet worker: $1.40 a hard run against the
$0.37 of Haiku it replaced; it read a third of Haiku's tokens (1.4M vs 4.3M cached) at 3.6x the money. Time: the hard-task
implementer ran 52 minutes on Sonnet against 36 on Haiku, because Sonnet ran whole-package test suites (1.6 a run, 0 for
Haiku; prometheus tsdb takes 10-30 minutes with eight runs in parallel), once 113 minutes in one run with 30-90-minute
timeouts and polling loops. That thoroughness likely also bought the hard-task gain.

## 0.1.38 (2026-11-03): live speed line; exact thinking counts

- The tokens panel's speed line is live: while a call waits, "waiting for first token · 1.4s" counts up; while it writes,
  "≈87 tok/s · 1.9s to first token" (estimated output: claude's thinking estimate, text and tool input at ~4 characters a
  token); between calls the last call's exact figures. "last first token" is gone.
- Time to first token is timed from the call's input being ready (the prompt sent, the last tool's result in) to its first
  streamed token. Claude's "requesting" note, used before, comes when the answer already streams (15 ms before the first
  token after a 2 s pause), so most calls showed 0.0s.
- Thinking counts: live from claude's `thinking_tokens` estimate (finer than the stream's steps of 50), then the exact
  count claude reports at the call's end (`output_tokens_details.thinking_tokens`), shown without "~". The estimate is
  often off (525 estimated for 478, 195 for 44).

## 0.1.37 (2026-11-02): `--boss-pick` (opt-in, under measurement); each model's effort levels known

- `--boss-pick`: in boss mode the lead picks each worker call's effort (Claude Code's per-call `effort`): low for a lookup
  or a mechanical edit, medium for ordinary work, high for logic that is easy to get wrong, never xhigh or max; a change
  hard even at high goes to a fourth worker, `implementer-strong` on Sonnet. The levels in its rule come from a table of
  what each model takes (`models::claude_levels`: Opus 5.5, Sonnet 5.5, Haiku 5.5 and Fable 5.1 take all five; Opus and
  Haiku default to medium, Sonnet and Fable to high); the own loop's models keep theirs per route in routes.json.

## 0.1.36 (2026-11-02): boss mode's workers at medium effort by default

The measurement below: boss mode's Haiku workers ran at the session's effort, so a session at xhigh (the user's) ran them
at xhigh, the dearest and slowest setting for no better score. They now run at medium unless `--boss-effort` says
otherwise. Headless runs on Opus medium (boss on by default) are unchanged; live sessions with `/boss` or `--boss` at a
higher effort get the cheaper workers.

## Boss mode's Haiku effort, and the lean profile on Opus, 2026-11-02 (bench only)

tasks/ (six Go tasks) x two runs per arm, Opus 5.5 medium, the repo profile, against the same day's no-boss runs (rt.sh,
13:19); boss arms 17:00-19:40 (tazuna 0.1.32), lean 19:41-21:23 (0.1.32, `--profile lean --no-boss`). Opus xhigh judge, one
pool per task and test, two passes (12 calls each test). One xhigh run (k6) used web search and is left out (leakscan).

| Setup | Judge | Correctness | Hidden | Cost / run | Tokens sent (Opus) | Opus calls | Minutes |
|---|---|---|---|---|---|---|---|
| no boss (live default) | 6.42 | 4.08 | 42/48 | $0.70 | 0.93M | 21.9 | 13.6 |
| **boss, Haiku medium** | **6.79** | 4.21 | **45/48** | **$0.45** | 0.29M | 8.8 | 15.2 |
| boss, Haiku high | 6.58 | 3.96 | 43/48 | $0.67 | 0.37M | 10.5 | 17.4 |
| boss, Haiku xhigh (n=11) | 6.55 | 4.27 | 38/43 | $1.02 | 0.29M | 8.5 | 23.7 |

Judge inside noise across the four; cost not: Haiku at medium 36% below no boss and 56% below xhigh, at 1.6 minutes more
than no boss. Worker effort default: medium (above). Boss mode itself stays opt-in for live sessions: cheaper at the same
quality on these tasks, but slower, and on company tickets (one run each, 2026-10-08) it scored 0.30 lower.

| Setup (same pool) | Judge | Hidden | Cost / run | Tokens sent | Calls | Minutes |
|---|---|---|---|---|---|---|
| repo profile (default) | 6.75 | 42/48 | $0.70 | 0.93M | 21.9 | 13.6 |
| lean profile | 6.71 | 43/48 | $0.81 | 1.18M | 31.2 | 10.4 |

Lean: the same score, 24% faster, 16% dearer: its call overhead is ~17k tokens smaller, but Opus makes 42% more calls
with its four plain tools. A trade, not a saving: it stays opt-in.

## 0.1.35 (2026-11-02): time to first token and tokens a second; a compact run summary; cache read and write by name

- Each model call is timed: claude says when it sends a request ("requesting"), and its stream opens with the first
  tokens, so time to first token is known on claude; the own loop sends no such note, so it gets only its generation
  time. The cockpit's tokens panel shows the latest call ("last first token 0.9s · 127 tok/s"); a replayed live session
  of 27 calls averaged 1.2 s to first token and 109 tok/s on Opus 5.5.
- The run summary is one compact row: "↑ 1.25M · 18 calls · cache 94% (1.18M) · ↓ 23.6k · 9 files · first token 1.2s ·
  109 tok/s" (it was "session: sent 1.25M over 18 calls (94% from cache) · out 23.6k · 9 files touched").
- The context panel names the cache's two sides: "cache read 1.18M 94%" (read back at a tenth of the price; its share of
  all sent) and "cache write 74.1k" (new content, written once at a premium); the tokens panel reads "↑ 1.25M sent ↓ 23.6k
  out". Display only.

## 0.1.34 (2026-11-02): files as a total with `/diff`; side panels aligned; the Claude plan named

- The files panel listed every file a tool named (temp files too), as many as fit. It now shows what this conversation
  changed in git: "3 files +130 −7", then each file that fits with its counts, the rest counted. A click on it, or
  `/diff`, opens the list beside the selected file's diff (↑↓ file, pgup/pgdn scroll, esc); a new file shows as added.
  Outside git it falls back to the files the tools named.
- The tools panel's name column is as wide as its longest name ("ScheduleWakeup" ran into its count), cut only when the
  panel is too narrow; the context panel's labels share one column.
- "on plan" reads "on Claude plan", and the windows "Claude 5h 33% · 7d 35%": named for a second subscription later.
  In the cockpit "Claude" and "SubRouter" are in colours of their own. Display only.

## 0.1.33 (2026-11-02): copy with cmd-c / ctrl-c, not on release

A drag over the transcript copied on release and said so in the transcript, also "copied 0 chars" after a drag over
blank space. Now a drag only selects (highlighted, with a moment's note that ⌘C or ctrl+c copies it); cmd-c (where the
terminal passes it on) or ctrl-c copies, and a selection of blank cells copies nothing. ctrl-c without a selection still
stops or quits. "copied 42 chars" shows in the composer for a moment instead of a transcript line. Display only.

## `--batch-calls` against the default, 2026-11-02 (bench only)

The six tasks/ tasks x two runs, Opus 5.5 medium, repo profile without boss mode; `--batch-calls` (tazuna 0.1.30, 16:00)
against the same day's default runs (0.1.24, 13:19), one judge pool per task, Opus xhigh, two passes (12 calls):

| Setup | Judge | Hidden | Cost / run | Tokens sent / run | Cache written | Calls | Minutes |
|---|---|---|---|---|---|---|---|
| default | 6.79 | 42/48 | $0.70 | 0.93M | 49k | 21.9 | 13.6 |
| `--batch-calls` | 6.62 | 42/48 | $0.76 | 1.05M | 54k | 22.4 | 15.0 |

The rule did not cut calls (fewer on caddy and terraform, more on the hugo tasks and k6): Claude Code already makes
independent calls together where it can. Judge and cost inside noise; it stays opt-in.

## 0.1.32 (2026-11-02): `/boss` mid-session; `--boss-effort` for its workers

- `/boss` turns boss mode on (Haiku workers; `/boss MODEL` for others) or off for the rest of a live session: claude
  restarts with the subagents and rules of the new mode, and the conversation goes on. Repo profile on claude only.
- `--boss-effort LEVEL` sets the effort of boss mode's explorer, implementer and tester. Unset, they run at the session's
  (Claude Code's rule for a subagent without one), so a session at xhigh ran its Haiku workers at xhigh. Under
  measurement.

## 0.1.31 (2026-11-02): background tasks live under the transcript; thinking counts smoothly; clearer numbers

- While background tasks run, the transcript ends in a slowly pulsing "waiting on 2 background tasks" and one row per
  task: a subagent's type, what it was asked, its time, tokens and tool calls, and its current step (claude's
  task_progress); a command's time. The screen looked still while the agent waited on them.
- Thinking: claude reports it in steps of 50-200 tokens about once a second; the count now runs up to each new total over
  the second after it, eased.
- A tool call's counts ("· 1 line · 18.2s ▸") get a row of their own when the call does not fit on one; "1 line", not
  "1 lines".
- The context window shows only as the model reports it: none until claude's first result, and none after a switch to
  the own loop (its providers report none). It was Opus 5.5's 1M whatever ran.
- The cockpit's context panel says what its cache figure is ("cached 82% of sent": of everything sent, read back from
  cache) and shows what went into the cache ("written 45.2k"); the tokens panel's "out/call" is now a caption under its
  bars ("output per call"). Display only.

## 0.1.30 (2026-11-02): a `/new` hint for large conversations; `--batch-calls` (opt-in, under measurement)

- After a task, once the conversation passes 60k tokens (then at each doubling), tazuna says so and that `/new` starts
  fresh for an unrelated task: every message re-reads the whole conversation (a review session sent 1.16M tokens over 16
  calls at 94k). Only a hint; `/resume` comes back.
- `--batch-calls` appends one rule to the repo profile's prompt: make independent tool calls in the same turn. A run's
  cost is about a third each reading back the cache, writing to it, and output; fewer turns cut the first two. Tested so
  far only on DeepSeek in the own loop (mixed: rclone 39 against 77 turns, caddy-unix 71 against 60).

## `--repo-tools core` and `--rtk` against all tools, 2026-11-02 (bench only)

tasks/ (six Go tasks) x two runs per setup, claude-opus-5-5 medium on the subscription, the repo profile without boss mode
(as live sessions run), tazuna 0.1.24 (all, core; 13:19) and 0.1.25 (rtk; 11:45, same code otherwise), one judge pool
per task over all 36 runs, Opus 5.5 xhigh, two passes (12 calls, about 1% of the 5-hour window per four).

| Setup | Judge | Correctness | Hidden tests | Cost / run | Tokens sent / run | Calls | Minutes |
|---|---|---|---|---|---|---|---|
| all tools (default) | 6.88 | 4.12 | 42/48 | $0.70 | 0.93M | 21.9 | 13.6 |
| `--repo-tools core` | 6.38 | 3.88 | 42/48 | $0.76 | 0.81M | 20.6 | 16.2 |
| `--rtk` | 7.00 | 4.25 | 43/48 | $0.74 | 1.07M | 22.8 | 13.5 |

Judge per task (all / core / rtk): caddy 8.75 / 7.75 / 8.00, hugo-section 8.00 / 6.50 / 7.50, hugo-template 5.50 / 5.25 /
5.50, k6 7.00 / 6.75 / 6.75, prometheus 7.00 / 7.50 / 8.50, terraform 5.00 / 4.50 / 5.75. Every difference is inside the
noise thresholds (judge ±0.75, cost ±15%). Core tools sent 13% fewer tokens, as the one-call check predicted (-27% a
call), but was not cheaper and scored lower on four of six tasks. rtk cut what the model saw of command output by 53%
(its own log, with Claude Code's 30,000-character cap applied: `go test` -96%, grep -36%), yet the runs sent 15% more
tokens over more calls, the pattern of the 8,000-character Bash cap (2026-10-08). Both stay opt-in; the default keeps all
tools and no rtk. No run saw the reference fix (leakscan).

## 0.1.29 (2026-11-02): `/new` starts a clean screen; `/resume` opens a session picker

- `/new` left the old conversation on screen and its tokens, cost, tools, files and clock in the side panels. It now
  starts a clean screen; the session's settings and the machine's readings (model, folder, mode, plan windows, SubRouter
  balance) stay. The elapsed clock starts again.
- `/resume` printed a numbered list to type from. It now opens a picker of this repo's claude sessions, the latest used
  first ("8m ago · 2 messages · review PR 1370"); ↑↓ and enter open one: the screen starts over with that conversation
  replayed from its recordings (only its own part where one recording holds several), and the next message continues
  it on claude. `/resume ID` still opens one directly. Display and session handling only.

## 0.1.28 (2026-11-02): tables with borders; links as titles you can open

- Markdown tables in replies were their raw pipes, wrapped across the screen. They are now drawn with borders, columns
  fitted to the width: a short column stays whole, a long one keeps its longest word whole and wraps inside its cell
  (rows then get a rule between them); too narrow for any of that, rows read as "a · b · c".
- `[title](url)` showed as raw markdown, the address wrapped over several rows. A link now shows its title, underlined,
  and is a real terminal hyperlink (OSC 8: cmd-click where the terminal supports it); a click opens it in the browser.
  Bare http(s) addresses too. Only plain http(s) addresses (printable, no spaces or control characters) become links;
  copied text keeps the title without the escape sequence. Display only.

## 0.1.27 (2026-11-02): thinking counts up; copying no longer freezes the stream; long calls close to a line

- Thinking: claude sends the tokens thought since its last update (50, 100, 150 ...), and the counter showed each one
  instead of their sum. They now add up, as in Claude Code.
- Copying text mid-run added a "copied 12 chars" notice, which ended the live thinking or text block: the spinner froze
  and what the model streamed next went nowhere (text could be lost). tazuna's own notices now leave the stream alone.
- A tool call is one short line until opened: a subagent shows who and what for ("explorer: Inspect load-secrets.sh retry
  impact"; its 25-row prompt was on screen in full), long inputs are cut at 160 characters, and ▸ marks a call with more.
  A click opens or closes one call (its whole input and output; ▾), the call under the mouse is shaded, ctrl-o still
  opens all. Display only.

## 0.1.26 (2026-11-02): working time beside elapsed; token totals named for what they are

The session showed one clock, the time since it began (the user's reading and typing included). Every design now also
shows the agent's working time: message to last token, summed over runs, the run in progress counting up ("elapsed 26:51
working 06:08"; past an hour h:mm:ss). The token totals beside "ctx 27.4k" read as a contradiction (119.7k in): they are
every call's whole input added up, each call sending the conversation again, so they now say "sent 119.7k over 5 calls
(89% from cache)". Zen keeps the plan and balance on a row of their own (its 92-column line ran out). Display only.

## 0.1.25 (2026-11-02): `--rtk` (opt-in, under measurement); the plan line without a session share

`--rtk` installs rtk's own Claude Code hook (`rtk hook claude`, PreToolUse on Bash) in the repo profiles, so claude's
commands run through rtk, which shortens their output (rtk counted -49% of command output on this Mac, 21,837 commands).
Whether that lowers the whole task's cost is open: capping Bash output at 8,000 characters cost +30% (2026-10-08), the
agent re-running commands to see what was cut. The plan line dropped its "(+2%)": the windows are the whole account's
(Claude Code beside tazuna counts too), and the number read as tazuna's own share.

## 0.1.24 (2026-11-02): `--repo-tools core` (opt-in, under measurement)

The repo profile loads all of Claude Code's tools (32 live, 29 under `-p`), most of them scheduling, messaging, worktrees
and workflows a coding task does not use. `--repo-tools core` keeps 14 (Task, TaskStop, ToolSearch, AskUserQuestion,
Bash, Read, Edit, Write, NotebookEdit, plan mode, Skill, web). First call of a one-line task in a repo with a CLAUDE.md,
Haiku: 14.8k input tokens against 20.4k (-27%). Run logs record `repo_tools`. The default stays all until Tier 1 shows the
quality holds.

## 0.1.23 (2026-11-02): SubRouter's balance shown whole in the cockpit

The cockpit's cost panel put the plan's windows and the balance on one line, which its 38-column sidebar cut to
"SubRouter $8". They now have a line each; every design is tested to show "SubRouter $8.82 left" whole. Display only.

## 0.1.22 (2026-11-02): `/new` and `/resume` in a live session

`/new` starts a fresh conversation on the same model, effort and route (the old one stays resumable). `/resume` lists this
repo's recent claude sessions (first message, age, messages sent); `/resume N` or `/resume <id>` continues one on claude.
Until now only `tazuna -c` / `--resume ID` at start did that, and claude's `/clear` cleared without a way back.

## 0.1.21 (2026-11-02): plan windows and SubRouter balance on screen; stopped turns priced; short commands caught

- A stopped turn: claude reported $0 for 21.4k tokens (8k of them written to cache), so the session read "$0 on plan".
  Such a turn's tokens are now priced at list rates and the cost marked "≈".
- The plan's 5-hour and 7-day windows: claude reports them only now and then (1 of the last 8 sessions), so a session
  showed none. It now opens with the newest reading from earlier sessions, with its age ("5h 12% · 7d 14% (2h ago)"; a
  window past its reset counts from 0), and `/usage` reads them now (one short Haiku call through claude).
- SubRouter's balance ("SubRouter $8.82 left") beside them, read with an API key at start and after each own-loop task
  (the billing endpoints, CNY at 6.8 to the dollar: matched the account page to the cent). No account login.
- "/mo" went to Opus as a message. A command typed short is now caught before anything is sent ("did you mean /model?").

## 0.1.20 (2026-11-02): the own loop's thinking on screen

The own loop's reasoning before each message now shows as a "thinking" row with its estimated size, like claude's, so a
run on DeepSeek or GPT reads the same as one on Claude: thinking, tool calls (reads, searches, commands), replies. Display
only.

## 0.1.19 (2026-11-01): the own loop's replies and tool calls on screen; costs under a cent; key hints that fit

After `/model` switched a live session to DeepSeek, a message ran four turns (it ran `cargo test` and answered) and the
screen showed only "Done 0m 00s · 4 turns · $0.00": the transcript is drawn from claude's streamed content blocks, and the
own loop sends whole messages. Their text and tool calls are now drawn from the message when nothing was streamed. The own
loop's result now carries its duration, and costs under a cent show two significant digits ($0.00019, not $0.00). The key
hints under the composer were one line cut at the screen's edge ("tab comp"); now as many whole hints as fit, then
"/keys all", and `/keys` lists every key. Display only.

## 0.1.18 (2026-11-01): pin one checked provider in `/model` (tab); the picker as wide as its table

A route's key lets SubRouter pick among the providers that passed the route check, call by call. Now each of those
providers also has a key of its own (`bench/routes.py pin`, run by every refresh; no model calls, keys cost nothing:
tazuna-route-<route>-<provider>, 12 made for DeepSeek, Luna, Sol and Terra), and tab in the picker cycles a row through
"any checked provider" and each one pinned, with its own price ("gpt-6.1-sol · kriora"). Picking a pinned provider leaves
the route's key, which other sessions may use, as it is. Only checked providers are offered.

The picker was 9/10 of the screen wide with the table at its natural width, so a wide screen showed an empty band beside
the columns. It is now as wide as the table (or its title), the label column taking what is left, and as tall as its rows
and the details need. Display and choice only.

## 0.1.17 (2026-11-01): every effort level on every model in `/model`

Routes offered only the levels that measurably differ (DeepSeek V4.1 Flash: high and max), so a cheap model could not be
picked at low for a quick question. Every row now cycles low ... max; a level the model lacks acts like one it has (the
provider maps it; the own loop already lowers a rejected max to xhigh). The note names the levels that differ ("only high
and max differ"). Presets and boss mode are unchanged. Display and choice only.

## 0.1.16 (2026-11-01): `/model` fits any width; the selected row's details below the list

The picker's rows ran past the panel and cut every price off (seen at 95 and 70 columns). Now a table with a header:
model, effort, price ($ per M in / out, three significant digits), bench (runs), AA. The columns fit the panel: the label
gives way first (cut in the middle, so "· fast" stays), then the scores go; the price always shows. The selected row in
full (price, scores, providers, cache share and age, or why it cannot be picked) wraps below the list. Tested at 50, 78
and 120 columns. Display only.

## 0.1.15 (2026-11-01): runs on Windows (cmd, PowerShell)

What differed between Unix and Windows now lives in src/platform.rs: the home folder (HOME, else USERPROFILE), the shell
(`sh -c` on Unix; Git Bash on Windows, as Claude Code uses there, so agent commands, test checks and `!` read the same),
stopping a timed-out command with everything it started (process group and `kill`; `taskkill /T`), private files (0600/0700
on Unix; the user profile's permissions on Windows), the clipboard image (PowerShell on Windows), hook paths for Git Bash,
the own loop's `search`/`list` through Git's grep and find (Windows has its own `find`).
Windows type-checks locally (`cargo check --target x86_64-pc-windows-gnu` with stub C headers for ring); build and tests run
in a GitHub Actions workflow on windows-latest: 92 tests pass there (the 2 Unix-only ones skipped). Git Bash gets its
script through an environment variable: on the command line MSYS re-parsed quotes and backslashes and broke 6 tests. macOS and Linux behaviour unchanged: 94 tests pass on both (2 new; Linux on InstaBox), release binary 3,684,208
bytes (-16), clippy warnings unchanged. Not on Windows yet: notifications, reaping an agent's leftover background
processes.

## GPT-6.1 Sol against DeepSeek as the own loop's cheap model, 2026-11-01 (bench only)

Sol at xhigh on its checked route (Kriora and the shared Pro pools) on the four tasks x two runs DeepSeek ran on LingAI
and on znu; one judge pool per task with all six runs, Opus two passes:

| | judge | hidden | min a run | billed a run |
|---|---|---|---|---|
| DeepSeek 1017 high, LingAI (`--preset reviewed`) | **7.00** | **24/26** | **8.1** | ~$0.005 |
| GPT-6.1 Sol xhigh | 6.50 | 22/26 | 11.7 | ~$0.017 |
| DeepSeek 1017 high, znu | 6.38 | 22/26 | 13.6 | ~$0.003 |

Sol led caddy-quoted (9.0 vs 7.2) and trailed on rclone (4.5 vs 7.0). DeepSeek on LingAI stays the cheap model: the best
judge and hidden tests, the fastest, and a third of Sol's cost. Sol's lead in the eight-model round (two tasks, one run
each) did not hold over four tasks.

## 0.1.14 (2026-11-01): the picker lists every scored SubRouter model, with bench and AA scores; `--pick`

`/model` now also lists every other SubRouter model that has a public score (the list holds 504 models, image and video
ones among them), best Artificial Analysis index first, dimmed and marked "unchecked: SubRouter picks the provider"
(picking one says so; `bench/routes.py refresh MODEL` checks it). Every row shows our bench's judge over the last 30 days
with its number of runs, and AA's intelligence index ("bench 6.8 (308) · AA 32"); bench/routes.py catalog writes them
into routes.json (no model calls). `tazuna --pick` opens the picker before the first message. Display only.

## 0.1.13 (2026-11-01): `--preset reviewed` on a checked route; two DeepSeek routes in `/model`

DeepSeek on znu (the gated refresh's pick: long context 88% vs AA 83%), the same four tasks x two runs as LingAI on
2026-10-30/31, one judge pool per task: judge 6.56 vs 6.94 (level, inside noise; terraform 1/3 vs 2/3), hidden 22/26 vs
24/26, ~40% cheaper ($0.024 for the eight runs), 13.6 against 8.1 minutes a run (busier machine, another day, and likely a
slower provider). A trade, not a win: the route key for DeepSeek keeps znu + stableai (cheapest), and a second route
"deepseek-v4.1-flash-1017 · fast" on LingAI (tazuna-route-deepseek-fast) sits next to it in `/model`.

`--preset reviewed` used SUBROUTER_API_KEY, which lets SubRouter pick any DeepSeek provider: among them ones that cached
22% or billed four times their listing. It now uses the fast route's key (LingAI, where every DeepSeek measurement ran).
Same model, effort and provider as measured.

## 0.1.12 (2026-11-01): cost split into money and plan; plan limits on screen

The cost shows who pays: "$0.50 + $7.46 on plan" (own-loop providers in money; claude at API prices, covered by the
subscription), "$7.46 on plan" or "$0.50" when only one ran. Claude's 5-hour and 7-day plan windows come from the
`claude` CLI's own rate_limit_event (no login token involved): "5h 3% · 7d 1% (+2%)", the rise since the session's first
reading in brackets; warning colour at 70%, error at 85% (where the benchmark guard pauses). A `/model` switch now closes
the old process's cost (the new one reports from $0; before, the total dropped after a switch). Display only.

## Route refresh with a quality gate and billed prices, 2026-11-01 (bench only)

bench/routes.py now lets a provider into a route only after the run checks (cache 70%+, few errors, served where asked),
40 long-context questions within 15 points of Artificial Analysis' LCR score, and a billed cost per call (from the account
log, same conversations on every provider) within 1.5x the cheapest passing one. First pass:

| model | provider | run checks | long context (AA) | billed | in the route |
|---|---|---|---|---|---|
| DeepSeek 1017 | LingAI | 89% cached | 81% of 100 (83%) | the reference | yes |
| DeepSeek 1017 | cybersol | 90% cached | 85% of 40 | listed $0, billed ~4x LingAI a call | no |
| DeepSeek 1017 | RelAI, a6aa, manxueai | 31%, 22%, errors | | | no |
| GPT-6.1 Sol | Kriora | 88% cached | 85% of 40 (80%) | listed 35% under the pools | yes |
| GPT-6.1 Sol | shared Pro 50/52 | 92% / 81% | 82% of 100 | | yes |
| GPT-6.1 Sol | manxueai | 86% cached | **20% of 40** | | no |
| GPT-6.1 Sol | laoliu, quqiai | 52%, 69% cached | | | no |

Two of the cheapest-listed providers would have been wrong picks: one billed four times its listing, one answered long
documents at a quarter of the model's public score. Luna's and Terra's routes were set before the gate (run checks only).

## 0.1.11 (2026-11-01): `/model`, a model picker; routes kept fresh

`/model` in a session lists Claude's models (the subscription) and the own loop's routes that bench/routes.py checked
(~/.local/share/tazuna/routes.json): providers, price, cache share, when checked, and only the effort levels the model
really has (DeepSeek: high, max). Type to filter, ←→ for effort, enter to switch. Grey: the route's key is not set;
yellow: the provider is not on the trusted list (the code would go there) or the route was checked over a week ago.
Between Claude models the conversation goes on (resumed on the new model); onto the own loop it starts afresh.

bench/routes.py refresh: provider prices move by the day (GPT-6.1 Sol's cheapest went from the shared Pro pools to Kriora
within days), so each model's four cheapest providers are checked like a real run (four agent-like conversations: cache
share, errors, where the calls went) and the route key is set to every one that passes; SubRouter then takes the
cheapest of them per call. First refresh: Sol on Kriora, laoliu and manxueai (cached 80-88%; quqiai failed). These pass
the run checks, not a quality check; the long-context canary (canary_lcr.py) is the one for that.

## 0.1.10 (2026-11-01): `--preset reviewed` names DeepSeek's effort as high

DeepSeek V4 has high and max only; the preset's "xhigh" already ran as high (below). The preset now says so. Same
requests, no metric change.

## Why xhigh beat max: what each route does with the effort it is sent, 2026-11-01 (bench only)

bench/effortprobe.py: three hard AIME 2025 problems, twice per effort, plus a made-up level ("ultra"); median reasoning
tokens (right answers of 6):

| route | low | high | xhigh | max | "ultra" |
|---|---|---|---|---|---|
| DeepSeek 1017, LingAI (our route) | 5.1k (6) | 10.1k (6) | 8.9k (6) | **25.9k** (6) | 11.1k (6) |
| DeepSeek 1017, RelAI | 0.7k (3) | 5.2k (4) | 9.9k (5) | 10.4k (5) | 20.6k (3) |
| Kimi K3.1 Pro, a6aa | 14.3k | 14.9k | 10.6k | 9.2k | 8.7k |
| GPT-6.1 Sol, shared Pro pools | 1.1k (4) | 0.9k (6) | 3.6k (5) | 2.6k (4) | HTTP 400 |
| GPT-6 Luna, shared Pro pools | 1.7k (2) | 3.7k (5) | 4.8k (6) | 3.6k (5) | HTTP 400 |
| GPT-6.1 Terra, Pro 52 | 1.9k (3) | 3.1k (5) | 2.8k (4) | 3.6k (4) | HTTP 400 |

DeepSeek V4 has two levels, high (the default) and max; xhigh and unknown values run as high. On LingAI max is real
(2.6x high), so "DeepSeek xhigh" in every round was DeepSeek high, and it beat max in agent work because max thinks
longer per call (1,434 against 956 output tokens a call), makes more calls (90 against 70) and takes twice the time
(37 against 18 minutes a run), which hit the one-hour cap on prometheus, without better code. Other providers map
levels their own way (RelAI clamps max to ~xhigh and got fewer right). Kimi K3.1 Pro on a6aa ignores the effort: its
high/xhigh/max runs were three runs of the same setting. The GPT pools validate levels but max did not think longer than
xhigh on these problems, so their effort comparison is open. A route's effort levels are now probed before an effort
comparison is read.

## Eight models x three efforts, a Luna check, and provider checks against public scores, 2026-10-31/11-01 (bench only)

**Models x effort** (mx*.sh): tazuna 0.1.8, own loop on SubRouter (routes vetted first: output under $0.50 per M, 70%+
cached; GPT models on the shared OpenAI Pro pools, Terra on Pro 52 alone) or the Claude path on the subscription
(Sonnet, Haiku), high / xhigh / max, two tasks no recent round used (telegraf-snmp-reconnect, gitea-actions-cancelling),
one run per cell (Haiku two). Judge: one Opus xhigh pool per task and effort (eight runs), two passes.

| model | telegraf judge h/x/m | gitea judge h/x/m | hidden h/x/m (of 4) | min a run h/x/m | real $ (6 runs) |
|---|---|---|---|---|---|
| GPT-6.1 Terra | 7.0 / 7.0 / 7.0 | 7.75 / 7.5 / 5.5 | 4 / 4 / 4 | 26 / 28 / 44 | ~1.70 |
| GPT-6.1 Sol | 5.5 / 5.5 / 8.0 | 6.75 / 8.0 / 8.0 | 3 / 3 / 4 | 9 / 11 / 17 | ~0.10 |
| Sonnet 5.5 (sub) | 5.0 / 6.0 / 8.0 | 6.0 / 8.0 / 4.5 | 3 / 3 / 3 | 14 / 14 / 42 | plan |
| Kimi K3.1 Pro | 6.5 / 2.5 / 6.0 | 8.0 / 3.75 / 6.75 | 4 / 3 / 4 | 22 / 27 / 19 | 0.49 |
| GPT-6 Luna | 4.5 / 7.5 / 6.5 | 2.75 / 6.0 / 6.0 | 1 / 4 / 4 | 18 / 8 / 15 | ~0.01 |
| Haiku 5.5 (sub) | 4.8 / 5.2 / 4.5 | 6.9 / 6.9 / 6.0 | 6 / 6 / 5 (of 8) | 10 / 18 / 33 | plan |
| DeepSeek 1017 | 4.0 / 5.0 / 4.5 | 6.5 / 7.5 / 5.0 | 3 / 3 / 3 | 22 / 18 / 32 | ~0.10 |
| Grok 4.7 | no change in any run | | 0 / 0 / 0 | 12 / 11 / 13 | 0.09 |

One run per cell is noise-sized (a judge point either way). What holds: max is the slowest everywhere and the best only
for Sol (and Sonnet on telegraf); breaking one existing telegraf test is common to every model; Grok 4.7 in the own loop
wrote its fixes into its replies and never edited a file (0 of 12, both providers). Kimi costs ~5x DeepSeek for the same
level. Two slips of mine: a pause meant for Kimi froze Grok's block 66 minutes (rerun clean), and Haiku ran twice
(subscription only); the gitea pools were judged twice (a scheduling slip; the scores average four passes).

**Luna xhigh against DeepSeek** (lx.sh): Luna at xhigh led telegraf on one run, against its public scores (AA index 23 vs
DeepSeek's 32.5, Terminal-Bench 8% vs 34%). Four other tasks, two runs each, one judge pool per task: DeepSeek xhigh 6.25
judge and 23/30 hidden, DeepSeek max 5.25 and 16/30, Luna xhigh 4.88 and 15/36 (k6 3.5 vs 6.0, caddy-unix 4.5 vs 6.0,
lazygit level). DeepSeek timed out at the 1-hour cap on prometheus-exemplar in all four runs (Luna finished it, 3/3 and
0/3). The telegraf lead was a fluke; DeepSeek xhigh stays the cheap default, and max again lost to xhigh.

**Providers against public scores** (canary_lcr.py, canary_hle.py; Artificial Analysis now publishes LCR and HLE for these
models, not MMLU-Pro or AIME). AA-LCR v1.1 (100 long-document questions, AA's prompts and checker): DeepSeek 1017 max on
LingAI 81% (AA 82.7%), Sol xhigh on the shared Pro pools 82% (AA 79.7%). HLE (200 of a fixed 400 text-only questions,
HLE's judge prompt): DeepSeek 37.0% (AA 43.3%; 7 questions thought past 128k tokens and answered nothing; 38.3% of the
answered), Sol 41.5% (AA 52.6%), but 34 of its calls came back empty within a minute (the shared pool, not the model):
50.0% of the answered. No route shows a cut-down model; DeepSeek sits ~5 points under AA on HLE, inside the interval.

## 0.1.9 (2026-10-31): `/loop` and `tazuna --loop`; `--preset reviewed` works outside the bench

`/loop [PRESET|FLAGS]` in a session (or `tazuna --loop [FLAGS]` from a shell) builds TODO.md's unchecked items one at a
time, each in a fresh headless run with those flags, and commits each item its run ticked. Without TODO.md it first asks
the session for PRD.md and TODO.md. It stops when everything is ticked, when an item is still open after two runs, at
$25 (`TAZUNA_LOOP_USD`), or when Claude's 5-hour window passes 85% (read from the run's `rate_limit_event`: past the
limit the plan goes on as paid extra usage); a notification at the end. When a reply leaves TODO.md with three or more
open items, the session says that `/loop` can build them. First run: a three-item Python tool with `--preset reviewed`:
3/3 built and committed, 31 tests passing, $0.22, about 6 minutes. Not measured against anything yet.

`--preset reviewed` stopped every run at the start outside the bench: SubRouter's model list has no prices and the own
loop's dollar cap needs one (the bench always set `TAZUNA_PRICE`). New `--price IN,CACHED,OUT`; the preset carries
SubRouter's price for DeepSeek 1017.

## The Claude path without boss mode, 2026-10-31 (bench only)

tazuna 0.1.8 `--no-boss` (Opus medium works itself, the Haiku explorer stays; what interactive sessions always run, since
the boss is on only for `--run` on Opus) against cp.sh's runs, the same four tasks x two runs (np.sh, v807-v814), one judge
pool per task over all three arms, Opus two passes:

| arm | judge | hidden | $ a run | min a run |
|---|---|---|---|---|
| stock Claude Code 2.1.314 | 6.88 | 20/26 | 0.79 | 4.2 |
| tazuna, boss (headless default) | 7.19 | 20/26 | 0.44 | 7.6 |
| tazuna, no boss (interactive default) | 6.81 | 20/26 | 0.60 | 5.5 |

Judge level across all three (inside noise; last night's pool had the boss arm at 6.88). No arm dominates: the boss is the
cheapest, no boss buys 28% less time for 37% more cost, Claude Code is the fastest and the dearest. Defaults stay: the
boss for headless runs (nobody waits), no boss when someone does.

Where the time goes (cct.sh: stock Claude Code again at the same hour as the no-boss runs, v815-v822, with a timestamped
transcript, `BENCH_STAMP`; timesplit.py now reads the Claude path). Claude Code took 4.8 minutes a run in the afternoon
against 4.2 at midnight, so most of the gap was the hour: at the same hour tazuna without the boss is 1.15x (5.5 against
4.8). Per run: model 2.6 against 2.2 minutes, the agent's own tests 2.4 against 2.5, the Haiku explorer 0.3 (used in
3 of 8 runs, ~45 s each while Opus waits; Claude Code started no subagent), tazuna's final check 0.1. Same hidden tests
(20/26). Nothing tazuna adds costs much time; the explorer is the largest part and saves cost. No change.

## 0.1.8 (2026-10-31): a shell a subagent leaves running no longer holds a session open

The rclone run above: an explorer subagent's search command went to the background, the subagent finished, the search
kept running. tazuna counts background commands as work in flight (claude wakes up when one ends), so the headless run
waited 25 minutes after its final check had passed. Nobody wakes up for a shell whose subagent has ended: it now stops
counting when its subagent ends. Shells claude itself started still count (it wakes up for them). Time per run only;
the agent's work is unchanged.

## The Claude path against stock Claude Code 2.1.314; canaries, 2026-10-31 (bench only)

tazuna 0.1.6's default on Opus 5.5 medium (headless: the Haiku boss, the Haiku explorer) against stock Claude Code 2.1.314,
the subscription, four tasks x two runs (cp.sh), Opus judge two passes: tazuna 6.88, Claude Code 6.94 (level), hidden
20/26 each, task by task the same; cost at list $0.44 against $0.79 a run (−44%), but 7.6 minutes against 4.2. Claude Code
got much faster since October; tazuna's boss mode trades time for cost. One tazuna run (rclone) passed its final check
at 12 minutes and then never ended: a background shell left in Claude Code kept `claude` open; ended by hand at 25 minutes
and scored from its diff (2/3). Bug: a headless run should end once its final check passes.

Canary (canary.py, 130 fixed items), first runs: DeepSeek 1017 on LingAI at max AIME 2025 28/30 (2 cut by HTTP 524, since
streamed) and MMLU-Pro 91/100; at xhigh 30/30 and 90/100; GPT-6.1 Sol on the shared Pro pools at xhigh 30/30 and 87/100.
No public AIME or MMLU-Pro reference without an Artificial Analysis key (Epoch has none for these models).

## 0.1.7 (2026-10-31): `--preset reviewed` runs DeepSeek at xhigh

The own-loop round below: the same judge as max, 41% faster, 32% cheaper. `--effort max` on the command line still wins
over the preset.

## Own-loop time: DeepSeek at xhigh, and jevgrep, 2026-10-30/31 (bench only)

`timesplit.py`: 86% of a DeepSeek 1017 run's wall time is the model (91% in October), tests and builds 7.5%, other shell
5%; ~61 calls a run at ~1,000 output tokens each, mostly reasoning at max; half the calls and half the time come before
the first edit. Two levers on that, tazuna 0.1.6, SubRouter LingAI, four tasks x two runs, a key per arm, Opus judge two
passes (xh.sh, jg.sh; the max runs serve both):

| arm | judge | tests | hidden | min/run | calls/run | real cost (8 runs) |
|---|---|---|---|---|---|---|
| max | 7.19 | 4.31 | 22/26 | 13.8 | 59 | $0.148 |
| **xhigh** | 7.06 | 4.00 | 24/26 | **8.1** | 51 | **$0.101** |
| max + jevgrep (`jg` on PATH, one search at the start) | 7.19 | 4.00 | 23/26 | 12.4 | 58 | $0.140 + Jev cents |

xhigh: the same score inside noise (−0.13), 41% faster and 32% cheaper: on the frontier rule it dominates max. jevgrep:
the model used it once a run; 10% faster, the score level (+0.19 against the same max runs, 7.00 there), cost level; it
sends code excerpts to TypeSafe. Not adopted (small gain, data leaves for one more party); xhigh as the own loop's DeepSeek
effort is the candidate.

## A decision model in the own loop: what it could decide, and Jev on "does this diff need a review", 2026-10-30 (bench only)

`decide_data.py`: 336 own-loop runs with known outcomes, a checkpoint every 20 model calls. Rules for "this run will
fail" barely beat the base rate (34% of runs fail): 60+ calls since the last edit flags 54 runs, 25% of them failing;
120+ calls 74, 35%. 113 of the 114 failed runs ended on their own, sure they were done: the failures are silent (the
hidden tests check what the run did not see), so stuck detection mid-run has little to find.

`decide_jev.py`: TypeSafe's Jev (jev-1.14.0, one `noul` question with the task and the diff as state) on 213 DeepSeek
diffs, truth = all hidden tests passed. Ranked by Jev's probability, reviewing 30/50/70% of the diffs would catch 23/46/66%
of the failures (random 30/50/70%); within the same task 39/54/65%. Its probabilities sit at a mean 0.49 for passing and
failing diffs alike. Simple rules do no better (fewest test lines 27/46/70%). 1.44M input tokens, ~$0.06, 8 seconds for
all 213. A decision model does not see what an implementation misses; the Opus review, reading the code, does (5-6 of 7).
Decision models stay for decisions they can see: routing a task by its text, once labels from real use exist (the bench
has 11 tasks with both a DeepSeek and an Opus arm, too few).

## Tried and dropped: a "thorough" workflow prompt, 2026-10-30 (bench only)

Lever TAZUNA_THOROUGH (own loop), ~1,000 characters in the spirit of oh-my-pi's prompt: find every place the change
touches, reproduce a bug first, a regression test that fails without the change, tests on boundaries, invalid input and
precedence, every stated behaviour checked, callers migrated and replaced code removed, no widening. DeepSeek 1017 max,
four tasks, two fresh runs per arm, a key per arm, Opus judge two passes:

| arm | judge | tests | code quality | edge cases | hidden | min/run | test lines | real cost/run |
|---|---|---|---|---|---|---|---|---|
| current prompt | 7.62 | 4.62 | 4.00 | 4.00 | 24/26 | 11.7 | 180 | $0.016 |
| thorough | 7.06 | 4.44 | 3.88 | 3.88 | 23/26 | 11.7 | 198 | $0.017 |

Nothing it aimed at moved (tests −0.18, +10% test lines); the −0.56 overall is terraform (current 10 and 4.5, thorough 5
and 5), inside noise. Dropped, and not kept as a preset: a preset has to buy score. Three prompt levers today (PROMPT3,
the tests nudge, this) left DeepSeek's tests where they were; the current arm itself scored 6.57, 7.19, 7.25 and 7.62 in
four rounds. omp's lead in tests (+0.87 over two runs a task) is not in its wording alone: its runs read 2-4x the files
and run 2x the builds and tests, with more tools (LSP diagnostics, todo, subagents).

## omp, OpenCode and Command Code against tazuna with DeepSeek 1017, 2026-10-30 (bench only)

Stock oh-my-pi 18.14 (`omp-sub`: pi's catalog entry, "openai" thinking format, own sealed agent dir), OpenCode 2.2.3
(`opencode-sub`: inline config, its data in a sealed folder of the run's own) and Command Code 1.88 (`cmd-sub`: BYOK
local-only, the user's login in a sealed per-run HOME, the relay adding reasoning_effort max, which it does not send for a
BYOK model), DeepSeek 1017 max on SubRouter LingAI, a key per harness, one run per task; tazuna 0.1.6 with the two
current-prompt runs per task from the tests-nudge round the same day. One judge pool per task, Opus two passes:

| harness | judge | tests | code quality | edge cases | hidden | min/run | real cost per key, failed attempts included |
|---|---|---|---|---|---|---|---|
| omp (n=4) | 8.62 | 5.00 | 4.25 | 4.75 | 13/13 | 32 | $0.18 |
| tazuna (n=8) | 7.50 | 4.38 | 4.06 | 3.94 | 23/26 | 20 | $0.08 for eight |
| Command Code (n=4) | 6.88 | 4.00 | 3.50 | 3.50 | 12/13 | 37 | $0.19 |
| OpenCode (n=4) | 5.12 | 3.12 | 3.00 | 2.88 | 10/13 | 28 | $0.09 |

omp leads by 1.1, past the noise line, on one run a task: terraform 9 (tazuna 4 and 6), rclone 9, caddy-retry 8.5 (70
min), tests 5.00 on every task. tazuna is faster and ~4x cheaper per run than omp and Command Code. Provider errors
(SubRouter/LingAI 403/499/502/503) ended nine rival runs today and none of tazuna's sixteen: omp, OpenCode and Command Code
retry briefly; those runs ran again. OpenCode reaches for its own webfetch and websearch (all refused here; leakscan reads
OpenCode's refusals since this round).

Second omp run a task (v747-v750, terraform once more after a provider error), judged in one pool per task with both omp
runs and tazuna's two: omp 7.81, tazuna 7.25 (+0.56, inside noise); tests 4.81 against 3.94, edge cases 4.25 against
3.75, code quality level (4.00), scope tazuna 4.50 against 3.75; hidden 25/26 against 23/26; 28 against 20 min a run.
omp's lead is mostly its tests. Its system prompt (~10.5K characters against tazuna's ~600) asks for a failing-before /
passing-after regression test, tests on boundaries, invariants and precedence, every acceptance criterion, and a clean
cutover (callers migrated, obsolete code removed); its runs read 17-38 files and run 11-25 builds or tests against
tazuna's 2-10 and 3-13. Next: a "thorough" workflow prompt for tazuna in that spirit, measured on score, cost and time
per run (a default if it dominates, an opt-in preset if it trades cost for score).

## Tried and dropped: a nudge for tests, 2026-10-30 (bench only)

Lever TAZUNA_TEST_NUDGE: a headless own-loop run that ends with a code change and no test file is asked once to add a test
that fails without the change. DeepSeek 1017 max, terraform, caddy-quoted, caddy-retry, rclone, two runs per arm, a key per
arm, Opus judge two passes: with the nudge 7.69 (hidden 25/26), without 7.19 (23/26), tests 4.44 against 4.38, median test
lines 184 against 201, time +9%. The nudge fired in one run of eight (caddy-retry, scored like the arm without it): DeepSeek
wrote tests in all sixteen runs. The +0.50 came from terraform (9 and 7.5 against 4.5 and 4), where it never fired: run
spread, not the lever. Dropped. The tests gap against other harnesses is in how much the tests cover, not whether there are
any. Same day, same tasks, the current prompt scored 7.19 here and 6.57 in the PROMPT3 round below: one arm moves ~0.6
between rounds.

## Why tazuna trails other harnesses by a little, and a prompt lever that did not close it, 2026-10-30 (bench only)

Every judge pool since v500 where tazuna and another harness ran the same model on the same task (106 pairs: pi, Codex,
Claude Code, Kimi Code): tazuna −0.32 overall. By dimension: tests −0.26 (negative against every rival, Claude Code
included, which tazuna beats overall), code quality −0.17, correctness and edge cases −0.12 each, scope +0.07. tazuna adds
~18% fewer test lines (median 167 against 203), and the judge names replaced code left unused in 13% of its diffs (0% of
the rivals'), e.g. the old `FindImportStatements` kept beside its replacement.

Lever TAZUNA_PROMPT3 (own loop): "a focused change" instead of "the smallest change", "remove what your change leaves
unused unless something else still uses it", "tests for the new behavior and its edge cases", and the exported-API rule
limited to code outside the repository (internal packages are not an API). DeepSeek 1017 max, four tasks, two runs per
arm, a key per arm, Opus judge two passes:

| arm | judge | tests | code quality | edge cases | correctness | hidden | time | real cost |
|---|---|---|---|---|---|---|---|---|
| current prompt (n=7) | 6.57 | 4.07 | 3.57 | 3.50 | 3.86 | 14/19 | 185 min | $0.19 |
| PROMPT3 (n=7) | 6.50 | 4.14 | 3.64 | 4.00 | 4.14 | 14/19 | 219 min | $0.22 |

No change overall or in the two dimensions it aimed at; edge cases +0.50 is inside noise; +18% time and +17% cost
(the PROMPT3 cost includes a run killed at 95 minutes). Dropped. One current-prompt run tried to fetch the upstream
terraform source (curl, `go mod download`); the jail blocked it and the leak scan left the run out. The runs of one prompt
spread widely (terraform 1/3 and 3/3, judge 4 and 10), so a gap of 0.3 needs many more runs to pin down than to find.

## 0.1.5-0.1.6 (2026-10-30): read takes line ranges; a call repeated three times in a row is named

Found with Grok 4.7 on tazuna's own loop. **0.1.5:** `read` takes `offset`/`limit` as numbers, numeric strings or a range in
`offset` ("100,200", "100-200"); anything else is an error saying what to send. Grok sent "100,200", tazuna read it as line
1, every repeat came back "unchanged since read #2", and Grok asked 134 times without an edit (v667). **0.1.6:** from the
third identical call in a row the result starts with "[the same call N times in a row: use what it returned, or do
something different]". In earlier own-loop sessions a call came three times in a row in 5 of 10 Grok sessions, 9 of 275
DeepSeek 1017 sessions and none of Sol's, Luna's or Astra's. Not measured on DeepSeek (it touches ~3% of its sessions);
on Grok it did not help (below). One test each.

## Kimi K3.1 Pro: tazuna against Kimi Code; Grok 4.7 on today's routes, 2026-10-29/30 (bench only)

Kimi K3.1 Pro on SubRouter (znu: 32/32 answered, 90% cached, reasoning on, 30 s a call in the probe), tazuna 0.1.4 against
Moonshot's own Kimi Code 2.4.0 (`kimi-sub`: an OpenAI-type provider through the relay, a fresh sealed home, headless
stream-json), one run per task, a key per harness, blind Opus judge two passes:

| task | tazuna | Kimi Code |
|---|---|---|
| rclone | 7.5, 3/3, 21 min | 6.0, 3/3, 34 min |
| caddy-quoted | 8.0, 5/5, 15 min | 9.0, 5/5, 17 min |
| terraform | 4.0, 1/3, 11 min | 5.0, 1/3, 12 min |
| caddy-retry | 8.0, 2/2, 9 min | 9.0, 2/2, 19 min |
| mean | 6.88, 11/13, 57 min | 7.25, 11/13, 82 min |
| real cost (account log) | $0.18 (161 calls, 7.3M input) | $0.45 (196 calls, 20.3M input) |

Level on quality (Kimi Code +0.38, inside noise; same hidden tests), tazuna 30% faster at 2.5x less. In the same pools
DeepSeek 1017 scored 8-9 on the caddy tasks at a fraction of Kimi's price.

**Grok 4.7** (miletechx; one run on fate86): it loops in tazuna on today's routes. Four runs on miletechx stopped at 240
calls without an edit (0.1.4 to 0.1.6), one fate86 run alternated two reads; the repeat note (0.1.6) was ignored, up to
145 identical calls in a row. Not the request format: tool results reach the model (streamed and not, empty or null
assistant content, 43K tokens of history: answered from the result each time). Not the effort: streamed with tools it
reasons at none, high and max (~1,200-2,200 reasoning tokens, right answer). What changed is how much it reasons inside an
agent run: ~38 output tokens a call by the account log (981 calls), against ~1,600 a turn in October's run (v161, X Premium+
pool, 3/3, judge 6, rescored 5 now). That pool is gone (model_not_found). Grok is not usable as an implementer on these
routes; tazuna's two fixes above came out of it. Overnight on fate86 at max (v687) and at high, October's effort (v688): both
stopped at 240+ calls without an edit, so the effort is not the cause; the rclone pool rescored October's run 6, Kimi 8.

## `--preset reviewed` as shipped, 2026-10-29 (bench only)

DeepSeek 1017 max on SubRouter, Opus medium reviewing, hand-over to Opus when the repo's tests still fail (tazuna 0.1.4);
against the earlier no-review and Opus-max-review arms, Opus judge two passes:

| task | no review | Opus max review | reviewed |
|---|---|---|---|
| caddy-quoted | 8.5, 5/5 | 8.5, 5/5 | 8.5, 5/5 |
| terraform | 4.0, 2/3 | 9.0, 3/3 | 5.5, 2/3 |
| caddy-retry | 9.0, 2/2 | 7.0, 2/2 | 8.0, 2/2 |
| caddy-tls | 4.5, 0/1 | 5.0, 0/1 | 5.5, 0/1 |
| mean | 6.50, 9/11 | 7.38, 10/11 | 6.88, 9/11 |

Opus medium reviewed in 38-80 s for $0.21-0.50 at list; it raised a real gap on terraform and caddy-tls and nothing on
caddy-retry. The hand-over never fired: the repo's own tests passed in all four runs (the hidden tests are not among them),
so it cannot rescue caddy-tls. One run per cell: the three arms are within noise. Three of the four runs were first killed
by the account-wide spend stop when the Kimi round started beside them, and rerun (v677-v679).

## Sonnet 5.5 as reviewer (medium, xhigh, max) against Opus 5.5 medium, 2026-10-29 (bench only)

The recall set below again, Sonnet 5.5 at three efforts added (it leads Opus on vals.ai), then all five reviewers of a
diff graded together by GPT-6.1 Sol (`GRADE_OUT=grade-review-all.jsonl`; notes without double quotes, or the reply is
not JSON). The usage guard paused at 70% of the 5-hour window: four Sonnet reviews of passing diffs were skipped.

| reviewer | cause named (7 failed diffs) | findings: gap / beyond / wrong | median time | over 15 min | cost at list |
|---|---|---|---|---|---|
| Opus medium | 5/7 | 13 / 4 / 1 | 68 s | 0 of 13 | $5.88 |
| Sonnet medium | 2/7 | 5 / 2 / 1 | 22 s | 0 of 12 | $1.12 |
| Sonnet xhigh | 4/7 | 8 / 1 / 1 | 246 s | 0 of 12 | $10.44 |
| Sonnet max | 2/7 | 5 / 1 / 1 | 900 s | 6 of 11 | $9.88 |
| Astra xhigh | 3/7 | 7 / 3 / 1 | 771 s | 6 of 13 | ~$0.10 real |

Opus medium stays the reviewer: the most causes named at the lowest cost per catch. Sonnet xhigh comes closest at
nearly twice the cost and four times the time; at max half its reviews ran past the cap. Grader noise: graded twice
(with two and with five reviewers), Sol changed its call on 2 of 14 cause verdicts (caddy-unix for Opus and Astra,
terraform for Astra), so one diff either way is within noise.

## Reviewer recall on 13 earlier DeepSeek diffs: Opus medium, GPT-6 Astra xhigh, both, 2026-10-29 (bench only)

`replay-review.py --set recall`: 13 diffs from earlier no-review DeepSeek runs (not leaked), 7 that failed hidden tests
and 6 that passed them all with judge 8 or more, each reviewed by Opus 5.5 medium (claude) and GPT-6 Astra xhigh in the
jail. `grade-review.py`: GPT-6.1 Sol xhigh (not a contestant) sees the task, the reference fix, the diff, its test result
and both reviewers' findings (anonymous, shuffled), classes each finding as gap (real, asked for), beyond (real, not asked
for) or wrong, and says per failed diff whether a reviewer named the cause.

| reviewer | cause named (7 failed diffs) | findings on passing diffs | all findings: gap / beyond / wrong | time a review | cost |
|---|---|---|---|---|---|
| Opus medium | 6/7 | 5 (all gap) | 13 / 4 / 1 | 31-157 s | $5.88 at list for 13 |
| Astra xhigh | 3/7 | 3 (2 gap) | 7 / 4 / 0 | 358-771 s; 6 of 13 hit the 15-minute cap | $0.10 real for the 7 that answered |
| both | 7/7 | 8 | 20 / 8 / 1 | | |

- Opus medium is the reviewer: it named the cause on six of seven failed diffs and answered every time inside three
  minutes. The one it missed (telegraf, no findings) Astra caught.
- Astra at xhigh still runs past 15 minutes on larger diffs (6 of 13 here, from 12K input up), so it cannot be relied on
  alone; where it answers, its findings are mostly real.
- Passing all hidden tests is not the same as clean: on the six passing diffs both reviewers found real gaps the task asks
  for (graded against the reference), none wrong.
- Two harness bugs found on the way: Executor.map cancels queued jobs after one fails (a diff with a binary fixture),
  and judge.py's API path always sent the judge's own instructions (the grader got scores back); both fixed.

## 0.1.2 (2026-10-29): `--review-only`; the review's JSON read past a note after it; Opus medium is reviewer enough

`tazuna --review-only --run TASK-FILE --review-model M` reviews the uncommitted change in the current directory and
prints the verdict as JSON, without an agent run. The bench uses it to put each reviewer and prompt on the same diff
(`replay-review.py`, in the jail), so the reviewer is compared, not the implementer's run: the diffs of the no-review
arm below (DeepSeek 1017 max), Opus 5.5 max through claude and GPT-6 Astra xhigh, each with the current prompt and with
a lever holding findings to the task ("a gap the task does not ask about is not a defect here, even if real").

| diff | Opus | Opus held to task | Astra | Astra held to task |
|---|---|---|---|---|
| terraform (the gap, hidden 2/3) | 3, gap found | 2, gap found | 2, gap found | 2, gap found |
| caddy-quoted (judge 8.0) | 0 | 0 | 1 (import arguments) | 2 (import arguments; the quoted-address suffix check the reference fixes) |
| caddy-retry (judge 9.0) | 0 | 0 | 2 | 1 |

- Opus is already precise: nothing on the two good diffs with either prompt, both terraform gaps found. The −2.0 behind
  its caddy-retry review last round was that run's own diff, not Opus finding things on good ones.
- Astra finds more on good diffs, partly real (the suffix check, which the judge noted as missed) and partly beyond
  the task (import arguments, again); the lever did not stop the latter. Dropped: no change on Opus, mixed on Astra.
- Astra at xhigh: 190-887 s, all inside the 15-minute cap (at max, four of five ran past it). Six reviews $0.084 real
  (~$0.014 each); Opus $1.96-3.52 a review at list.
- Opus at high and medium (tazuna's default), current prompt: the same two terraform gaps and nothing on the good
  diffs, in 29-99 s and $0.09-0.52 a review at list, against 360-640 s and $1.96-3.52 at max. Medium stays the default;
  the bench's max reviewer arms paid ~15x for the same findings. (Three diffs, one with a real gap: none of the Opus
  efforts saw the suffix check Astra found.)
- Opus answered "ok" in prose without the JSON twice at max (a failed review: no feedback, as with an empty one), and
  once at high with the JSON followed by a note quoting braces, which the parser took as unreadable. Now the first JSON
  object with `issues` wins (one test).

## A reviewer behind DeepSeek: GPT-6 Astra and Opus 5.5 at max, 2026-10-29 (bench only)

DeepSeek V4.1 Flash 1017 at max implementing on tazuna 0.1.1, three arms on separate keys: no review
(`tazuna-ownsubx`), GPT-6 Astra max through the provider's API (`tazuna-ownsubxrev`), Opus 5.5 max through claude
(`tazuna-ownsubxopus`). One run per task and arm; blind Opus judge, two passes.

| task | no review | Astra review | Opus review |
|---|---|---|---|
| caddy-quoted | 8.0, 5/5, 7.6 min | 6.0, 5/5, 30.9 min (1 finding, outside the task) | 7.5, 5/5, 18.1 min (no finding) |
| caddy-retry | 9.0, 2/2, 7.3 min | 8.0, 2/2, 26.6 min (review timed out) | 7.0, 2/2, 19.0 min (1 finding) |
| terraform-import | 4.0, 2/3, 12.3 min | 7.5, 3/3, 26.3 min (2 findings) | 8.5, 3/3, 19.3 min (2 findings) |
| mean of the three | 7.00, 9/10, 27 min | 7.17, 10/10, 84 min | 7.67, 10/10, 56 min |
| caddy-tls (unsolved by all) | 4.5, 0/1, 13.8 min | run stalled in review, not rerun | 5.0, 0/1, 30.5 min (review timed out) |

Real cost (account log, one key per arm): no review $0.031 for four tasks; Astra arm $0.054 implementing (two killed
runs included) + $0.065 for the four reviews that answered (~$0.016 each; timed-out calls were not billed); Opus arm
$0.032 implementing + $6.89 at list for three Opus reviews ($1.43-$3.04 each; the fourth timed out).

- Where DeepSeek had a blind spot, both reviewers found it and the fix landed: on terraform both named the same two
  gaps (dynamic for_each import targets dropped, module instance keys missing from the dedup key); judge +3.5 and +4.5,
  hidden tests 2/3 to 3/3.
- Where DeepSeek was already right, a finding cost quality: Astra's (quoted braces through import arguments, real but not
  asked) became an unrequested feature, judge −2.0; Opus's (the CEL expression re-parsed per response) led to a
  redesign the judge liked less, −2.0 (different implementer runs, one each).
- Astra at max is too slow for a reviewer: one of five reviews answered inside 15 minutes (823 s); the others took 24
  minutes, stalled past 30 (two runs killed by the hang watch before the 15-minute cap) or hit the cap. Opus: 289-672 s,
  one cap. One Opus answer came as prose without the JSON ("no Critical or Major defects"): read as a failed review,
  harmless here.

Opus judging an arm Opus reviewed: the same diffs judged again by GPT-6.1 Sol at xhigh (`judge.py --model
gpt-6.1-sol@URL`, `rejudge-sol.sh`; 8 calls, $0.02; Sol at max hit the route's 900 s limit). Means on the three
tasks: none 7.67, Astra 8.17, Opus 8.33 (Opus judge 7.00, 7.17, 7.67). Same order, same +0.67 for the Opus arm over no
review; Sol scores everything ~0.7 higher. Opus rated its own arm above Sol only on terraform (8.5 against 7.5) and below
on caddy-quoted (7.5 against 9.5): no self-preference beyond noise. caddy-tls: Sol 7.0 none, 6.5 Opus arm.

Within noise (+0.17 and +0.67 on three tasks, one run each); stays opt-in. Per review Astra costs ~1/140 of Opus at the
same catch on terraform. Next: Astra at xhigh for latency, and findings held to the task's acceptance criteria (both
quality losses came from findings beyond them).

## 0.1.1 (2026-10-29): the independent reviewer on any provider's API (not measured yet)

`--review-model MODEL@URL --review-key-env VAR` runs the reviewer as one call to that provider (Responses for gpt-,
Messages for claude-, Chat otherwise) instead of `claude -p`: the same instructions, the task, the diff and the changed
files in full (it has no tools), the verdict back to the implementer once as before. Cost from TAZUNA_REVIEW_PRICE. The
reviewer's key is read once at start and removed from tazuna's environment before anything starts, so neither the
agent's commands nor the test runs on its code can see it; `MODEL@URL` without `--review-key-env` is refused, and the
reviewer's provider passes the same data check as the provider and the explorer. Only regular files inside the work tree
go into the review (a symlink to a key file elsewhere would otherwise be sent to the provider). Made for GPT-6 Astra as a
reviewer behind a cheap implementer: on Astra, Codex's lead over tazuna was code quality and scope (dead code left,
convoluted logic where a helper existed, thin tests; judge notes), the kind a diff review catches. Bench harnesses
`tazuna-ownsubxrev` (Astra reviewer) and `tazuna-ownsubxopus` (Opus 5.5 reviewer through claude). One test against a
local stub provider (request shape, key, verdict, cost).

## Correction: Codex reports a fraction of its usage; on GPT-6 Astra it costs ~4x tazuna, 2026-10-29 (bench only)

One key per harness on the same shared Pro pools (`perkey.sh`, gpt-6-astra, caddy-quoted and caddy-retry) lets the
account log split real usage by harness. pi's and tazuna's own counts match it to the token; Codex's do not:

| | model calls | prompt tokens (cached share) | output | real cost | self-reported input |
|---|---|---|---|---|---|
| Codex (1.5 runs) | 183 | 15.38M (95%) | 85K | $0.521 | 0.91M |
| tazuna 0.1.0 (2 runs) | 71 | 4.35M (95%) | 31K | $0.155 | 4.35M |
| pi (2 runs) | 47 | 2.24M (94%) | 25K | $0.095 | 2.24M |

Codex's `turn.completed` usage covers only part of its model calls (here about a fifteenth): every Codex token and cost
figure above taken from it is far too low, so "Codex leaner on Astra" is withdrawn. On real cost Codex is the costliest
of the three on Astra and very likely on Sol (its Sol runs shared a key with the others; not split). Its quality lead on
Astra (judge 7.50 against 6.75) stands. Codex's second run here ended on SubRouter's 402 "insufficient balance": the
account was empty ($0.03 left).

## Tried and dropped: reads cut at 12K characters, 2026-10-29 (bench only)

Why tazuna sends GPT-6 Astra twice Codex's input: its whole-file reads (13-30K characters) stay in a history every call
sends again, where Codex's command output reaches the model cut to ~10 KB. A/B with reads stopping at whole lines after
12,000 characters (`readcap.sh`, TAZUNA_READ_MAX, not kept): Astra input over the four tasks 9.25M against 9.17M (no
change; terraform -50%, caddy-quoted +73%), 13.9 against 17.1 min a run, and judge 6.00 against 6.75 (caddy-tls 3.5
against 5.0; Codex 7.50, pi 6.50 in the same pools). DeepSeek on LingAI: caddy-quoted 1.41M against 1.90M, caddy-retry
1.97M against 1.58M. Reads are not where the gap is; next: how many calls Codex makes and what each one carries, with one
key per harness so the account log splits them.

## 0.1.0 (2026-10-29): the model's reasoning no longer goes back with the history by default; MiniMax 70 to 9 min

0.0.85 sent each step's `reasoning_content` back on the Chat route. No SubRouter provider passes it to DeepSeek's model
(0.0.94-0.0.97 entry) and none bills it on a short history (DeepSeek, GLM-5.3 and MiniMax prompt tokens identical with
and without ~1K tokens of it), but MiniMax-M3.1-Flash reacted to its own long thinking by thinking longer: with it,
20K characters of reasoning per step and 3.7K output tokens a call, the prompt at 177K tokens by call 30; one run reached
910K tokens and failed on the context limit. Now it goes back only with TAZUNA_SEND_REASONING or when a provider demands
it (the existing fallback); 0.0.97's note of a long step's end stays. GPT on the Responses route is unaffected (its
encrypted reasoning items still go back).

| MiniMax-M3.1-Flash, caddy-quoted (foxrouter) | hidden | min | calls | input | output |
|---|---|---|---|---|---|
| 0.0.99 (v604) | 5/5 | 70.5 | 79 | 6.7M | 293K |
| 0.1.0 (v616) | 5/5 | 8.8 | 38 | 1.27M | 18K |

One run a side (MiniMax was then dropped as a target: too slow and costly for its result at max, and foxrouter cached it
23-81%). DeepSeek check on 0.1.0: caddy-quoted 5/5 in 9.7 min and 47 calls (0.0.97: 8.1, 52), caddy-retry 2/2 (27.5
min, through a foxrouter outage and a move to LingAI mid-run); no regression.

## The other models, 2026-10-29 (bench only)

Same four tasks, deepseek-style method: probe first (`bench/chatprobe.py` for the Chat and Anthropic routes,
`bench/respprobe.py` for Responses), one run per harness per task, judged blind per task with two passes.

GPT-6.1 Sol with tazuna 0.0.99 (`night-1029.sh`, two tazuna runs a task, judged with the earlier Codex and pi runs):

| | judge (mean of tasks) | hidden | min/run | $/run |
|---|---|---|---|---|
| tazuna 0.0.99 | 6.81 | 16/22 | 8.3 | 0.0126 |
| pi | 6.88 | 10/13 | 21.5 | 0.0130 |
| Codex | 6.50 | 10/13 | 20.4 | 0.0258 |

GLM-5.3 at max on a6aa (90% cached in the probe; kriora 20% and empty answers on the Anthropic route): judge tazuna 5.62
(4 runs), stock Claude Code 6.00 (3), pi 6.00 (3); hidden tied; tazuna 30 min a run against 33-36. tazuna best on
caddy-quoted (8.0 against 6.5/7.5), behind on caddy-retry (5.0 against Claude Code's 7.5): its own test check failed in
`caddytest/integration` and the fix round took 69 min and 29M input (a collision of parallel runs on the integration
tests' fixed ports is the guess; unconfirmed, the output kept was too short). Lost: Claude Code caddy-tls (provider
stream error after 112 min), pi caddy-retry (`find /` without a time limit for 2 h; pi's bash has none by default).

MiniMax-M3.1-Flash at max on foxrouter: not judged. Every harness slow (30-126 min a run, up to 845K output tokens for
Claude Code); passes where finished (caddy-quoted 5/5 tazuna and Claude Code, caddy-retry 2/2 all three, terraform 2/3
Claude Code); several runs lost to the context limit (tazuna, the cause of 0.1.0), pi's unretried `incomplete_stream`,
a provider gap, pi's 3 h limit and the 08:00 stop.

GPT-6 Astra at max on the shared OpenAI Pro pools (92% cached, ~$0.001 a call; laoliu billed 5x for 10x the
reasoning), tazuna 0.1.0 (`astra.sh`):

| | judge (mean of tasks) | hidden | min/run | calls | $/run |
|---|---|---|---|---|---|
| Codex | 6.88 | 8/11 | 9.7 | 70 tool actions | 0.047 |
| tazuna 0.1.0 | 6.38 | 8/11 | 17.1 | 32 | 0.090 |
| pi | 6.12 | 8/11 | 12.6 | 33 | 0.079 |

Codex leads on its own model: judge within noise, but half tazuna's input per run (0.7-1.4M against 1.3-4.6M) and
faster. tazuna won caddy-retry (9.0, fastest) and lost most on caddy-tls (32 min, $0.17, the same miss). Where tazuna
stands: ahead of Claude Code and level with pi on DeepSeek and Sol at a fraction of the time; level on GLM; behind Codex
on Astra on efficiency, not on hidden tests.

## 0.0.99 (2026-10-28): the exported-API rule narrowed to breaking changes; Sol's caddy-retry judge 5.30 to 7.75

0.0.90's line "Do not change exported APIs ... unless the task asks for it; changing unexported code is fine when you
update every caller" is now "Do not change or remove existing exported APIs ..., or add methods to exported interfaces,
unless the task asks for it; adding a new exported function, or a method on a concrete type, is fine."

Why: tazuna's Sol runs lost caddy-retry (below). A read of the diffs against the upstream tests (reproduced locally)
found one mechanism: every passing run (pi, Codex, upstream, all of tazuna's DeepSeek runs) gave `MatchExpression` a
new exported method; tazuna's Sol runs added nothing exported ("This keeps public APIs unchanged", v568), sorted the
matchers once in `Handler.Provision` instead, and edited the existing test helper to match, so their own tests passed
while the upstream tests, which build handlers "bypassing Provision" (the helper's own comment), never retried. Sol reads
the old line as "add nothing exported"; DeepSeek did not.

| gpt-6.1-sol, caddy-retry-match-expr | runs | judge (one pool, 2 passes) | hidden | suite broken | min | $/run |
|---|---|---|---|---|---|---|
| 0.0.98 (old line) | 5 | 5.30 | 8/10 | 2 | 11.8 | 0.0149 |
| narrowed line | 4 | 7.75 | 8/8 | 0 | 8.5 | 0.0113 |

+2.45 judge (pi 8.5, Codex 7.25 on the same task). A second lever tried beside it changed nothing (tests line "change
existing tests or test helpers only where the task changes what they check", "reuse existing code rather than copying
it", no plan rule for gpt- models): judge 5.83 against 5.83, hidden 4/6 against 5/6; dropped.

DeepSeek check (deepseek-v4.1-flash-1017 at max, foxrouter): prometheus, the task the line was written for, twice a side:
3/3 every run, judge 7.0 against 7.5 (inside the line), and neither narrowed run touched the `ExemplarStorage` interface
(both added an exported method on the concrete `CircularExemplarStorage`, which the line allows). The four panel tasks once
with the narrowed line: caddy-quoted 5/5 (4.9 min, 28 calls, against 0.0.97's 8.1 and 52), caddy-retry 2/2, caddy-tls
0/1, terraform 1/3: in 0.0.97's range.

## gpt-6.1-sol at max: stock Codex, pi and tazuna 0.0.97; pi leads, tazuna level with Codex, 2026-10-28 (bench only)

Step 3's GPT leg, with GPT's own harness: stock Codex CLI on an API key (`codexsub`: no ChatGPT login, an empty
CODEX_HOME per run, the key behind the relay), pi with its own catalog entry for gpt-6.1-sol (Responses API) and tazuna
0.0.97, all on SubRouter's shared OpenAI Pro pools (`bench/respprobe.py`: Pro 50 cached 88% over four conversations with
no errors at 1/3.7 of foxrouter's cost; LingAI and kriora showed no reasoning at max, three others billed a prompt other
than the one sent). One run per harness per task, caddy-retry twice; judged blind in one pool per task, two passes
(`judge.sol1028.tasks.jsonl`, `judge.sol1028b.*.jsonl`).

| task | Codex: hidden, judge, min | pi | tazuna |
|---|---|---|---|
| caddy-quoted-braces | 5/5, 8.5, 12.8 | 5/5, 9.0, 8.6 | 5/5, 8.5, 9.5 |
| caddy-retry-match-expr (2 runs) | 4/4, 7.25, 33.0 | 4/4, 8.5, 11.8 | 3/4, 4.75, 13.2 |
| caddy-tls-automate-names | 0/1, 4.5, 15.6 | 0/1, 4.5, 60.4 (outage) | 0/1, 5.5, 48.6 |
| terraform-import-foreach | 1/3, 5.0, 7.7 | 1/3, 4.5, 14.7 | 1/3, 4.0, 7.8 |
| judge (mean of tasks), hidden | 6.31, 10/13 | 6.62, 10/13 | 5.69, 9/13 |
| per run: calls, input, cost | 116 tool actions, 7.0M, $0.026 | 43, 2.6M, $0.013 | 42, 3.8M, $0.017 |

tazuna is level with Codex on quality (-0.6, inside the line) at ~0.7x its cost; it does not beat GPT's own harness, and
pi is ahead of both. The gap is caddy-retry, where tazuna on DeepSeek scored best: both of its Sol runs copied CEL's
placeholder-expansion code instead of reusing it, ran parsing on the hot path and edited existing tests (one changed a
test helper's signature, which broke the upstream tests' build). pi's two were the closest to the reference. Sol against
DeepSeek in the same harnesses: 40-70% fewer calls and 3-8x less output for the same hidden tests, ~2x slower calls, and
about the same cost per run at the shared pools' prices.

Bench fixes on the way: pi's and Codex's parsers threw away two finished runs over one call each had retried (a run fails
now only on its last message or a failed turn); a pool outage (15:38-15:50, `shared_route_recovering`) ended two runs
outright (0.0.98 above); leakscan.py counted a `git clone https://github.com/...` line in CONTRIBUTING.md, read by six
runs, as a fetch (a fetch now counts only in a command the agent ran).

## 0.0.98 (2026-10-28): a conversation whose shared ChatGPT pool goes down moves to another pool (no metric)

SubRouter's shared OpenAI Pro pools (gpt-6.1-sol at ~2% of OpenAI's price, the cheapest Sol route) go down for minutes
at a time: 503 `shared_route_recovering`, Pro 50 from 15:38 to after 16:00 during the Sol panel. A key listing several
pools sends a new conversation to one that is up (tested: Pro 50 alone 503, Pro 50 + 52 4/4). A running conversation stays
on the pool whose account wrote its encrypted reasoning, and while that pool is down every call fails 503
`responses_source_unavailable` ("original conversation supply is currently unavailable"): it ended a tazuna, a pi and a
Codex run. tazuna now takes that error at once (no ~16 minutes of retries), drops the encrypted reasoning items, which only
that account can read, takes a new cache key and sends the call again: the conversation goes on in another pool of the
key, its visible text and plan notes intact, the first call there uncached (`conversation_moved` in the transcript).
Route switching (0.0.79) already dropped reasoning items the same way. Two tests: the move itself, and one request
only against a stub that answers the error. Normal runs never reach this path, so nothing to measure; it shows on the
next pool outage.

## Earlier runs audited for reading other runs' work: no kept or dropped version changes, 2026-10-28 (bench only)

leakscan.py now has a `peer` channel: a tool call that reads another run's work tree (`rw-...-vN@`) or a harness's logs
(`peer-listed` when it only lists them; `peer-blocked` from v526, where the jail refuses the read; `test_leakscan.py`).
Over every transcript, 19 runs read; leaked() leaves them all out of summaries and the judge from now on. The scored ones
from v300 on, and what they fed:
- v349 (0.0.85 A/B, rclone r1, 3/3) read its pair's diff (v348). Without the pair, 0.0.85's judge gain is +1.1 on four
  pairs instead of +1.5 on five; and 0.0.94-0.0.97 found its mechanism reaches no model through SubRouter anyway.
- v405 (round11, caddy-retry) read v309's whole diff. 0.0.90 was kept on -21% turns with the judge within noise: holds.
- v439 (round12, 0.0.92 prometheus, 2/3) diffed and read the 0.0.90 run beside it (v430). That round was read as noise.
- v449 (single-session provider test) read v309's and v391's diffs: a speed test, no quality decision.
- v376 (0.0.85 prometheus) saw other runs' changed-file lists; v381 ran a test in another tree while chasing a Go
  setting; v382 looked for sessions in a path that does not exist. Left out too.
- Before v300: v53 and v117 (old rounds).

## tazuna 0.0.97, stock Claude Code and pi with DeepSeek: same quality, tazuna 2-3x cheaper and faster, 2026-10-28 (bench only)

pi 1.4.0 as the third harness (`pi-sub` in run.py): stock pi with its own catalog entry for DeepSeek V4.1 Flash (thinking
format, levels, compat), only SubRouter's URL and model id changed, a fresh agent dir per run. One change from stock: its
retry patience raised to tazuna's (11 retries, up to 120 s apart): with pi's 3 retries in ~14 s, SubRouter 503s (CPU
overload, upstream quota) ended three runs at 27-45 min, and one more ended after the 11. Its key goes through a localhost
relay (below). deepseek-v4.1-flash-1017 at max on foxrouter, the same four tasks, two pi runs each, today; tazuna 0.0.97 ran
beside it at the same time (two runs a task), next to last night's tazuna and Claude Code runs. All judged blind in one pool
per task, two passes (`judge.pi1028.*.jsonl`, `judge-pi.sh`). Valid runs only (see the correction below); runs from v526 on
ran in the closed jail.

| task (runs: tazuna 4, Claude Code 2, pi 2) | tazuna: hidden, judge, min/run | Claude Code | pi |
|---|---|---|---|
| caddy-quoted-braces | 20/20, 8.50, 8.1 | 10/10, 8.25, 12.9 | 10/10, 7.25, 14.7 |
| caddy-retry-match-expr | 8/8, 8.00, 11.1 | 4/4, 8.00, 17.0 | 4/4, 8.25, 25.9 |
| caddy-tls-automate-names (tazuna 3) | 0/3, 4.83, 12.0 | 0/2, 6.00, 56.6 | 0/2, 4.50, 15.4 |
| terraform-import-foreach | 6/12, 5.50, 6.4 | 3/6, 4.75, 24.6 | 2/6, 5.00, 24.1 |
| all | 34/43 (79%), 6.83 over 15 | 17/22 (77%), 6.75 over 8 | 16/22 (73%), 6.25 over 8 |
| per run: minutes, calls, input, output, cost | 9.2, 48, 2.0M, 39K, ~$0.008 | 27.8, 132, 7.6M, 149K, ~$0.018 | 20.1, 70, 5.2M, 84K, ~$0.023 |

(cost: tokens at the same SubRouter prices for all three; pi and tazuna share one key and request path, so the account log
cannot split them.) Quality is level: the judge spread (6.25-6.83) is inside the 0.75 line, and so are hidden tests. tazuna
takes a third of Claude Code's time and a half of pi's, and costs ~0.45x Claude Code and ~0.35x pi. Same time of day:
pi against tazuna today 16/22 vs 16/22, judge 6.25 vs 6.44, 20.1 vs 9.3 min; Claude Code against tazuna last night
17/22 vs 18/21, 6.75 vs 7.29. pi is the costliest of the three: one caddy-retry run took 44 min, 114 calls and 16M input
for the same 2/2 tazuna got in 8 min. pi's two terraform passes (3/3 each) were the ones that read the fix (below); in
the jail it scored 1/3 twice, as tazuna did today.

Bench hardening in the same round: each harness gets only its own credentials (models grep `env` for Go and proxy settings,
and the output goes to the provider: four SubRouter keys did, pi's own and three tazuna runs never used; tazuna already drops
its own key from its tools' environment). pi keeps its key where its tools can read it, so it holds a dummy and a relay in
run.py, outside the jail, adds the real one.

## Correction to the entry below: one tazuna run had read Claude Code's work; the bench jail now stops that, 2026-10-28 (bench only)

tazuna's one caddy-tls pass (v504, judge 9) is out. It ran `git diff` in the work tree of the Claude Code run beside it
(v505) and copied test files from the tree of a killed Claude Code run (v462) that was still in TMPDIR. The other 15 runs
of the panel read no other run's files. Without v504:

| | tazuna 0.0.97 | Claude Code |
|---|---|---|
| caddy-tls-automate-names | 0/1, judge 3.5, 13.1 min | 0/2, 5.5, 56.6 min |
| all | hidden 18/21 (86%), judge 7.14 over 7 runs | 17/22 (77%), 6.62 over 8 |
| per run | 9.0 min, ~$0.006 | 27.8 min, ~$0.019 |

tazuna is ahead on three tasks of four, not every task: Claude Code's judge is higher on caddy-tls. The judge gap overall
(+0.52) is under the 0.75 line. Per run tazuna is 3.1x faster and ~3.1x cheaper (cost: the account log's $0.054 less
v504's 22% token share). The main item's step 1 holds on hidden tests, cost and time, not on a judge margin.

Found while adding pi as the third harness, which showed three holes in the bench, all closed in `run.py` (tested with
tazuna, pi and stock Claude Code making real calls inside the jail):
- Network: the proxy refused GitHub, but `curl --noproxy '*'` went around it; pi's model fetched the fix commit that way
  (v514). The jail now refuses every outbound connection that is not to localhost (the proxy, local test servers).
- Other runs' work trees: readable in TMPDIR, both a pair's other half and trees of killed runs (34 were left). A run now
  reads only its own tree among `rw-*`; the old trees are deleted.
- Transcripts: the bench writes pi's and Claude Code's next to their HOME, and tazuna's sessions are in its HOME; pi v518
  found and mined v514's transcript. The jail lets a run list and write those folders, not read the files.
leakscan.py caught v514 (web) and v518 (web, through v514's transcript); it does not look for reads of other runs' trees,
which only a search of every panel transcript found. Runs before v492 were not checked for that.

## tazuna 0.0.97 against stock Claude Code with DeepSeek: better on every task, 2.8x cheaper, 2.3x faster, 2026-10-28 (bench only)

(Corrected above: without the tazuna run that read Claude Code's work, tazuna is ahead on three tasks of four.)

The roadmap's main item, step 1. `night-1028.sh`: deepseek-v4.1-flash-1017 at max, both harnesses on foxrouter
(tazuna-vet-4; LingAI does not serve SubRouter's Anthropic route), two pairs at a time, two runs a side on four tasks
(two of them hard ones whose hidden tests split runs). Claude Code is the official CLI with its own prompt and tools and an
empty config dir (`cc-sub`). Judge: one pool per task, two passes (`judge.night1028.*.jsonl`; terraform and caddy-tls
re-judged with Claude Code in the pool, `judge.night1028b.*.jsonl`, after the leak scan learned WebFetch's refusal).
Leak scan clean: four Claude Code runs tried GitHub (curl, git ls-remote, WebFetch), all refused.

| task (2 runs a side) | tazuna: hidden, judge, minutes, turns, input, output | Claude Code |
|---|---|---|
| caddy-quoted-braces | 10/10, 8.75, 7.6, 52, 1.92M, 35K | 10/10, 8.25, 12.9, 74, 3.62M, 66K |
| caddy-retry-match-expr | 4/4, 8.0, 10.9, 47, 2.26M, 53K | 4/4, 7.75, 17.0, 100, 4.66M, 66K |
| caddy-tls-automate-names | 1/2, 6.5, 23.6, 94, 6.13M, 62K | 0/2, 5.5, 56.6, 218, 16.0M, 335K |
| terraform-import-foreach | 4/6, 6.5, 6.6, 38, 1.21M, 31K | 3/6, 5.0, 24.6, 133, 5.90M, 129K |
| all | hidden 19/22, judge 7.44, 1.6 h, $0.054 | 17/22, 6.62, 3.7 h, $0.151 |

(minutes to output: means per run; cost: the account log, foxrouter key, split by request path.) tazuna is ahead on
every task: judge +0.82 (over the 0.75 line at 8 runs a side, short of the 12 the threshold is set for), hidden 19 against
17, 2.8x cheaper, 2.3x faster, 0.38x the input, 0.30x the output. Same model, same provider, same night. Next: pi, the
minimal open harness, as the third in the same panel; then a second model (GLM or MiniMax, which also recommend Claude
Code).

## 0.0.94-0.0.97 (2026-10-27/28): the model's reasoning, found missing and kept as a note; -63% cost, -61% time

The fast panel against stock Claude Code (deepseek-v4.1-flash-1017 at max on foxrouter, side by side) turned up four
things, one version each:
- 0.0.94: foxrouter streams DeepSeek's reasoning as `reasoning`, not `reasoning_content`; tazuna kept none of it. Now both.
- 0.0.95: the transcript records each step's reasoning (a `reasoning` event, never sent; leakscan.py skips it).
- No provider passes earlier reasoning to the model: a code word planted in sent-back reasoning was never recalled,
  through foxrouter and LingAI, Chat and Messages API, with a tool call in between or not. 0.0.85's reasoning-back
  reaches no model through SubRouter. Without its plan tazuna's model worked out the same design again at every new
  finding: six times on one caddy-quoted run, 13-26K characters each; it had 41 calls over 5K output tokens where
  Claude Code had 8 (72% of its output). Ruled out on the way: the API path (identical reasoning on identical history),
  a brevity line, tazuna's own test check.
- 0.0.96: a prompt line asking the model to write its plan in the reply: ignored (14 heavy steps against 17).
- 0.0.97: the end of any step that reasoned over 4000 characters (its last 2500) goes into that turn's reply in the
  history, once, so the cached history stays the same.

`keep1.sh`, 0.0.97 against 0.0.96 side by side (tasks set, foxrouter):

| | caddy-quoted 0.0.97 | caddy-quoted 0.0.96 | terraform 0.0.97 | terraform 0.0.96 |
|---|---|---|---|---|
| hidden | 5/5 | 5/5 | 1/3 | 1/3 |
| turns | 50 | 92 | 48 | 67 |
| time | 533 s | 1354 s | 533 s | 1376 s |
| input / output | 1.73M / 44K | 3.97M / 152K | 1.54M / 40K | 2.94M / 166K |
| heavy steps (reasoning over 12K chars) | 2 | 15 | 4 | 21 |
| est. cost (account log, split by tokens) | $0.006 | $0.016 | $0.006 | $0.016 |

Claude Code on the same tasks earlier: caddy-quoted 5/5 in 101-108 turns, 17-19 min, 4.4-5.6M input, 86-92K output;
terraform 1/3 in 123 turns, 28 min, 6.0M input, 117K output. Single runs; two a side and the judge next.

Fast panel before the fix (tazuna 0.0.93/0.0.94 against Claude Code, `ccpanel*.sh`): caddy-quoted both 5/5 (tazuna
slower: 26-30 against 15-17 min); terraform tazuna 2/3 against 1/3 (tazuna ~2x the cost); caddy-tls both 0/1 (tazuna 66
min, Claude Code 48); prometheus-duration tazuna 0/1 in 2.2 h with 1.06M output, Claude Code 1/1 in 2.5 h with 1.11M.
foxrouter holds about three conversations in the Chinese evening (52-71% cached at five to seven) and four at night.

## tazuna against Claude Code with DeepSeek, first task; single sessions per provider, 2026-10-27 (bench only)

The roadmap's main item, step 1: the same model in tazuna's own loop and in stock Claude Code (the official CLI, its own
prompt and tools, a fresh config dir: no user settings, plugins or hooks), on SubRouter's Anthropic route
(`run.py` harness `cc-sub`; a local stub showed only the SubRouter key goes out, no login token). LingAI does not serve
that route for 1017 (404 "API endpoint not found"); foxrouter, kriora, a6aa, relai and fate86 do, so both ran on
foxrouter (`ccsub2.sh`, caddy-retry, deepseek-v4.1-flash-1017 at max, side by side; `judge.oct.ccsub1.jsonl`).

| | tazuna 0.0.93 (v455) | Claude Code (v456) |
|---|---|---|
| hidden | 2/2 | 2/2 |
| judge | 7.5 | 7.5 |
| turns | 74 | 90 |
| time | 808 s | 1179 s |
| input (cached) | 3.82M (91%) | 8.74M (94%) |
| output | 61K | 81K |
| cost (account log) | $0.0138 | $0.0214 |

Same quality, -36% cost, -31% time, -56% input on one task; tazuna on LingAI at the same time: 2/2, judge 9, 50 turns.
Claude Code sends ~10-17K tokens of prompt and tools with every call against tazuna's ~2K. Next: more tasks, two runs at
a time (not fourteen) so neither provider throttles.

Single sessions (`single2.sh`, one conversation per provider, caddy-retry then gitea, 11:57-13:38, Chinese evening):
all 2/2. Per call from the account log: relai 94% cached, 24 s, $0.00006; LingAI 93%, 9 s, $0.00014; kriora 59%, 41 s,
$0.00024; fate86 81%, 13 s, $0.00026; a6aa 46%, 31 s, $0.00039. Judge (two tasks, `judge.oct.single2.jsonl`): a6aa 8,
relai 7.75, LingAI 7.25, kriora 6.75, fate86 5. fate86's caddy-retry run took 62 minutes where it took 5 an hour
earlier. Only relai is cheaper than LingAI, at 2.5x the time a call; no separate cheap mode for now.

## 0.0.93 (2026-10-27): SubRouter streams again; cheap providers only for single sessions

SubRouter's gateway cuts an unstreamed reply at ~125 s with a 524 (not billed): a counting question at max effort, four
calls per mode, failed unstreamed every time on LingAI and fate86 and streamed through (LingAI 140-163 s, fate86
286-348 s, both ~90K characters of reasoning). Every long reasoning call waited two minutes before 0.0.92 sent it again
streamed. The only reason to start unstreamed was a6aa, which turned out to be the slowest; streaming is back.

`cacheprobe.py` with CACHE_CONC=4 (four conversations at once per provider, replies up to 2000 tokens; $0.34):

| provider | cached | failed | median s/call | real $/M input |
|---|---|---|---|---|
| LingAI | 100% | 0 | 14 | 0.0017 |
| foxrouter | 96% | 0 | 15 | 0.0024 |
| kriora | 87% | 1 | 16 | 0.0019 |
| relai | 100% | 4 | 28 | 0.0003 |
| fate86 | 100% | 15 | 45 | 0.0004 |
| a6aa | 92% | 51 | 36 | 0.0011 |
| tokenx | 36% | 35 | 36 | 0.0118 |
| o2oai | 38% | 42 | 37 | 0.0114 |

(The probe's "tokens per second" divided short outputs by whole-call time and is not generation speed; the counting
question gives that: fate86 about half LingAI's.) One conversation (`single1.sh`, caddy-retry, 0.0.93 streamed, at the
same time): fate86 alone 35 turns, 293 s, 92% cached, $0.0025, 2/2, judge 7; LingAI 41 turns, 384 s, 95%, $0.0039, 2/2,
judge 7 (`judge.oct.single1.jsonl`). A cheap provider holds up for one session (-36% here) and not under parallel runs;
LingAI stays the route key's provider.

Endpoints (20 interleaved calls each): `subrouter.ai` and the Cloudflare CDN `ai.orbitlink.me` reach the API in the same
0.31 s (median, /models); a small chat call's first byte 2.79 s and 2.44 s; the EU West Railway relay adds ~0.13 s
(0.44 s). Under 2% of a 14-45 s call: no change.

## 0.0.91-0.0.92 (2026-10-26/27): SubRouter unstreamed; route cache judged from call 20, 524 retried streamed

- 0.0.91: calls to SubRouter start unstreamed, like UnoRouter's (streamed again after a 413 or an empty reply).
- 0.0.92: a route's cache is judged from its 20th call (0.0.79 judged the first 10, which start cold: LingAI itself
  cached 45-98% there and 84-100% over the next 10, and all seven a6aa runs of round12's first start left at call 10);
  an unstreamed call the gateway cuts with a 524 (SubRouter's Cloudflare, ~100 s) goes again streamed.

`round12.sh` / `night-1026b.sh` overnight: all seven tasks once each, 0.0.90 streamed on the LingAI route key (v420-v432,
even) against 0.0.92 unstreamed on a key limited to a6aa with the LingAI route key behind it (v434-v440). Leak scan
clean; judge `judge.oct.round12.jsonl`.

| | 0.0.90, LingAI streamed | 0.0.92, a6aa then LingAI, unstreamed |
|---|---|---|
| judge (7) | 6.21 | 6.93 |
| hidden | 13/22 | 21/22 |
| turns | 656 | 448 (-32%) |
| cached (runs) | 93% | 90% |
| est cost / hours | $0.124 / 11.1 h | $0.111 / 8.0 h |

- a6aa under seven runs at once: calls took ~100 s (unstreamed calls ran into the 524 and went again streamed) and
  cached 22%; every run left it for LingAI at calls 10-17, on speed this time, not on the cold start. With three runs
  at once the evening before it cached 87% at 1/4 of LingAI's cost per call. A cheap provider with little capacity:
  not a default for parallel runs.
- LingAI unstreamed against streamed over the night: 93% cached both, $0.00022 against $0.00026 a call. Unstreamed is no
  worse on LingAI and is what a6aa-like providers need; kept.
- The judge and hidden-test gap is between single runs on different routes and versions whose changes do not touch
  the model's work (streaming, route rules); read as noise, with 0.0.90's 6.21 against its 6.57 in round11.

## 0.0.90 (2026-10-26): the own loop keeps exported APIs unless the task asks; same judge, -21% turns

One prompt line: "Do not change exported APIs (public functions, methods, interfaces and types other packages use)
unless the task asks for it; changing unexported code is fine when you update every caller." In sigprobe.py's replay it
halved how often DeepSeek planned to add a method to prometheus's exported ExemplarStorage interface.

`round11.sh` (v400-v413): all seven tasks-oct tasks once each, 0.0.89 against 0.0.90, fourteen at once; judged in one
pool per task with the a6aa runs below (`judge.oct.round11.jsonl`). Leak scan clean (v402 and v409's GitHub tries were
refused; the scan now reads curl -f's 403 and a curl that printed nothing as refused). $0.43 on the account.

| task | 0.0.89: judge, hidden, turns | 0.0.90 |
|---|---|---|
| rclone | 8, 3/3, 159 | 4, 1/3, 68 |
| caddy-unix | 5, 0/7, 44 | 9, 7/7, 32 |
| caddy-retry | 9, 2/2, 55 | 7, 2/2, 49 |
| gitea | 5, 2/2, 35 | 7, 2/2, 48 |
| lazygit | 7, 3/3, 110 | 8, 3/3, 71 |
| prometheus | 6.5, 3/3, 76 | 5, 2/3, 79 |
| telegraf | 6, 2/2, 69 | 6, 2/2, 87 |
| all | judge 6.64, hidden 15/22, 548 turns | 6.57, 19/22, 434 (-21%) |

Judge within noise; none of 0.0.90's lower scores comes from the rule (rclone read --links per call instead of at
construction; caddy-retry hardcoded proxy placeholder names). Its prometheus run tracked exemplar times on the series
as the reference does, with no interface change. Kept.

**Provider and streaming (bench only).** `cacheprobe.py` replayed a real history through each vetted provider,
unstreamed: fate86 100% cached, a6aa 98%, tokenx 99%, o2oai 97%, relai 97%, kriora 95%, foxrouter 96%, LingAI 84%;
stableai down. Real runs then disagreed for a6aa streamed and agreed unstreamed: 0.0.89 on rclone, caddy-retry and
telegraf through a key limited to a6aa, streamed (`a6aa1.sh`, v414-v416) and with TAZUNA_STREAM=0 (`a6aa2.sh`,
v417-v419), next to round11's LingAI runs. From the account log over the whole runs:

| route | calls | cached | per call | per M input |
|---|---|---|---|---|
| LingAI, streamed (round11) | 987 | 86% | $0.00038 | $0.0041 |
| a6aa, streamed | 192 | 73% | $0.00018 | $0.0032 |
| a6aa, unstreamed | 229 | 87% | $0.00010 | $0.0018 |

a6aa caches streamed calls badly and unstreamed ones like LingAI, at ~1/4 of the cost per call. Judge on the same three
tasks: a6aa 7.17 streamed, 6.33 unstreamed (3 runs each) against LingAI's 0.0.89 runs 7.67; hidden 12/14 against 7/7.
Single runs, inside noise; no unstreamed call failed. Next: SubRouter unstreamed by default, a6aa as the first route
with LingAI behind it (0.0.79's route switching), measured on all seven tasks.

## 0.0.89 (2026-10-26): tazuna's own test check reports a run past its 600 s limit as no result, not a failure

Under four prometheus runs at once the whole tsdb package needs over 600 s; the kill went back to the model as a test
failure, and round9's runs spent their remaining hours fixing bugs that were not there. Now the run ends unverified
(`claimed` in the run log), with no fix round; the UI shows "timed out after Ns · not verified".

`round10.sh` (v396-v399): prometheus, 0.0.88 against 0.0.89 twice each, four at once; judged in one pool with
round8/round9's prometheus runs (`judge.oct.round10.jsonl`). Leak scan clean (v396's GitHub tries were refused; the scan
now reads an empty fetch result as refused). $0.23 on the account.

| run | check | turns | time | hidden | judge | est. cost |
|---|---|---|---|---|---|---|
| v397, 0.0.89 | killed at 653 s: ends unverified | 112 | 54 min | 3/3 | 6.5 | $0.045 |
| v399, 0.0.89 | killed at 621 s: ends unverified | 83 | 55 min | 3/3 | 7 | $0.051 |
| v396, 0.0.88 | killed at 606 s, "failed": a fix round, then passed in 328 s | 92 | 77 min | 3/3 | 6.5 | $0.065 |
| v398, 0.0.88 | passed in 527 s | 75 | 25 min | 0/3 (gc signature) | 3 | $0.030 |

Both 0.0.89 runs met the limit and stopped there with all hidden tests passing: nothing hidden by the early end. The
0.0.88 run in the same spot spent a fix round (8 turns, ~20 min) on code that already passed; in round9 that loop ran
to the 3 h limit. Same pool, all prometheus runs since the go.work fix: 0.0.87 6.1 (4), 0.0.88 4.5 (6), 0.0.89 6.75 (2);
the low scores are the gc-signature runs (hidden tests do not compile), not a version.

## 0.0.88 (2026-10-26): bash takes a timeout up to 1800 s; polling gone, prometheus -32% turns, judge mixed (open)

At a fixed 120 s the model ran prometheus's slow tests in the background and polled them (`sleep 115; tail`), one turn
per poll with the whole history sent again: 33-47 polls and 4-5 timeouts a run in round7. `bash` now takes `timeout`
(seconds, default 120, up to 1800), its description says to give one for long builds and test runs, and a timeout says
to give a larger one.

`round8.sh` (v381-v390): prometheus 0.0.87 (tz-v85's binary, same code) against 0.0.88 twice each, and 0.0.88 once on
the other six tasks, judged in one pool with round7's 0.0.85 runs (`judge.oct.round8.jsonl`). `round9.sh` (v392-v395):
prometheus twice each again, after the bench fix below (`judge.oct.round9.jsonl`, all eight prometheus runs in one
pool). deepseek-v4.1-flash-1017 at max on the vetted route key; leak scans clean. Cost at the fixed round price.

| | 0.0.87 | 0.0.88 |
|---|---|---|
| prometheus, 4 runs each | judge 6.0 (6, 6.5, 3.5, 8), hidden 3/4 runs, 937 turns, $0.608, polls 10-25 a run | judge 4.0 (3, 2.5, 3.5, 7), hidden 1/4, 634 turns (-32%), $0.382 (-37%), no polls |
| six other tasks, 1 run each | (0.0.85, round7) judge 6.58, hidden 11/19 | judge 7.33 (+0.75), hidden 19/19 |

- Every prometheus failure is the same mistake: a changed `stripeSeries.gc` signature that the hidden tests (and the
  base tests) call. 0.0.88 made it 3 times in 4, 0.0.87 once, Opus v34 once in 2: p = 0.24 for 3/4 against 1/4, and
  nothing in the change touches that design choice (it is made before any long test run). The six other tasks gained
  +0.75 on runs from different days. Pooled over ten runs a side: 6.0 against 6.35, within noise; turns and cost lower.
  Kept, with prometheus as the open question: the next step is a measured prompt line against changing the signatures
  of existing functions the code and tests call (5 of 12 prometheus runs, DeepSeek and Opus, failed on it).
- Decision probe (`sigprobe.py`, $0.08): v384's history up to its first edit, the model asked for its plan 10 times per
  arm. Extra argument to `stripeSeries.gc`/`gcSeries`: 0.0.87 9/10, 0.0.88 7/9, 0.0.88 with a rule against changing
  existing signatures 9/10. So the tool text does not drive it; it is the model's default design from that code (an
  unexported method, all callers updated, which compiles; it fails only the PR's white-box tests). The rule did halve
  the riskier change, a method added to the exported `ExemplarStorage` interface (avoided 3/10 without, 6/10 with).
- Three round9 runs (v392-v394) hit the 3 h limit while tazuna's own check reran the whole tsdb package: under four
  runs at once it took over 600 s, and tazuna's verify counts its own 600 s kill as a failure, so the model hunted bugs
  that were not there (v393 and v394 twice). Scored by hand from their partial diffs; v393 had stashed its changes to
  test the base when stopped, its diff rebuilt by replaying its 38 edits (matches its last `git diff --stat`). Next for
  tazuna: a verify run killed at its limit reports "no result", not a failure.
- Bench fix: since the 2026-10-24 module-cache isolation `GOFLAGS=-mod=mod` made go refuse to run in the four prometheus
  tasks (workspace mode) until the agent deleted go.work; runs that kept it scored 0 hidden tests (v383, corrected to
  3/3), runs that deleted it were marked down for "unrelated go.work changes". Workspace repos now run without it.
- LingAI's cache: about 15% of large calls missed and made ~71% of round9's cost; the same history came back counted
  as ~135K or ~180K tokens, two upstream backends with their own caches. Not ours; a longer per-provider cache check is
  the next routing step.
- UnoRouter's a7-supplier group for 1017 (v391, caddy-retry): ~8x cheaper a turn than SubRouter but ~5x slower; its
  price rose 1.6x mid-run and uno_run.sh stopped it (81 turns, $0.0041). A fallback, not a default.

## 0.0.87 (2026-10-26): 0.0.85's reasoning back again (all of it); 0.0.86's window measured worse; 0.0.85 -55% cost on 7 tasks

0.0.86 sent back only the last 8-24 turns' reasoning, to cut the input that 0.0.85's full reasoning seemed to add. One
round of all seven tasks-oct tasks, 0.0.84 (no reasoning back) against 0.0.85 (all) against 0.0.86 (window)
(`round7.sh`, `round7b.sh`, v360-v380), deepseek-v4.1-flash-1017 at max on the vetted route key without the watcher, so
each run kept its provider. Leak scan clean (only GitHub tries the proxy refused). Judge `judge.oct.round7.jsonl` and
`judge.oct.round7p.jsonl` (two passes; anchors Opus v34, Codex Luna). Cost at one fixed price ($0.02/M uncached input,
$0.0004/M cached, $0.08/M output) so the provider does not skew it; the round spent $0.96 on the account.

| task | 0.0.84: judge, hidden, turns | 0.0.85 | 0.0.86 |
|---|---|---|---|
| rclone | 8, 3/3, 247 | 8, 3/3, 110 | 6, 3/3, 123 |
| caddy-unix | 8.5, 7/7, 63 | 4.5, 0/7, 66 | 4.5, 0/7, 107 |
| caddy-retry | 7, 2/2, 66 | 7.5, 2/2, 63 | 8, 2/2, 74 |
| gitea | 7, 2/2, 65 | 8, 2/2, 38 | 7.5, 2/2, 37 |
| lazygit | 7.5, 3/3, 179 | 7, 3/3, 176 | 6.5, 3/3, 89 |
| telegraf | 5, 1/2, 150 | 4, 1/2, 75 | 6, 2/2, 121 |
| prometheus | stopped at 3 h, 352 turns | 5.5, 3/3, 200 | 4.5, 3/3, 235 |
| 6 tasks (not prometheus) | judge 7.17, 770 turns, output 1.54M, input 6.69M uncached + 65.8M cached (90.8%), $0.283 | 6.50, 528, 0.49M, 3.66M + 38.5M (91.3%), $0.128 | 6.42, 551, 0.69M, 4.70M + 35.4M (88.3%), $0.163 |

- 0.0.84's judge lead is caddy-unix alone: its 7/7 is one of two passes in the last 15 caddy-unix runs, and 0.0.84
  failed it 0/7 twice the day before (v350, v356). The failing runs rebuild the listener's internal types, which the
  hidden white-box tests compile against. On the other five tasks: 6.90, 6.90, 6.80.
- 0.0.85 against 0.0.84: judge -0.67 (within noise, all of it caddy-unix), turns -31%, output -68%, cost -55% (-54%
  without rclone's 247-turn 0.0.84 run). 0.0.84 did not finish prometheus in 3 h; 0.0.85 did in 200 turns.
- Correction to 0.0.85's "+25% cost": that A/B ran with `subrouter.py watch`, whose provider moves cold-started the
  caches. Without it 0.0.85 caches as much as 0.0.84 and its fewer turns make it much cheaper.
- 0.0.86's window: each step changed the history and lost cache (88.3%), and output rose 40% over 0.0.85; judge -0.08 on
  six tasks and -1 on prometheus, cost +27%. Reverted: 0.0.87 is 0.0.85's code.

Seen on prometheus: all three versions ran the slow tests in the background and polled them (`sleep 115; tail ...`), one
turn per poll with the whole history sent again. Next to measure.

## 0.0.85 (2026-10-25): DeepSeek's reasoning back in the history; judge +1.5, turns -30%, output -63%, cost ~+25%

Where tazuna's turns before green went on rclone (clean 0.0.83/0.0.84 runs against OpenCode's): without its earlier
reasoning DeepSeek re-planned on every turn, 15-34 turns a run of 8-26K output tokens followed by one small edit, and
rewrote its own code (one file 7 times, 20 "final rewrite" announcements in a run); OpenCode keeps the reasoning, plans
once and edits each place once (73-82 turns against 168-217).

- The own loop sends `reasoning_content` back with the history by default (Chat Completions); a provider that refuses
  the field gets the history without it for the rest of the route (the reverse fallback, for one that demands it, stays).
  `TAZUNA_DROP_REASONING` turns it off.
- A failed `edit` without `old` says so; no "nothing changed" note when one command writes, runs and deletes a probe test.

A/B against 0.0.84 (`reason1.sh`, v348-v359), deepseek-v4.1-flash-1017 at max on the vetted route key, 2 rounds side by
side. v353 (0.0.85, caddy-retry round 1) read the reference patch from a git worktree of this repo left in ~/bench-homes:
excluded with its pair, the worktree removed, the sandbox now refuses any `bench/realwork` path and any `solution.patch`.
Judge `judge.oct.reason1.jsonl` (two passes; anchors Opus v34 4.5-9, Codex Luna 7).

| task | 0.0.84: judge, hidden, turns, output | 0.0.85: judge, hidden, turns, output |
|---|---|---|
| rclone r1 | 5.5, 2/3, 207, 454K | 8.5, 3/3, 236, 211K |
| caddy-unix r1 | 5, 0/7, 118, 318K | 5, 0/7, 47, 93K |
| rclone r2 | 4, 2/3, 145, 506K | 8, 3/3, 121, 178K |
| caddy-unix r2 | 4, 0/7, 172, 314K | 5, 0/7, 39, 89K |
| caddy-retry r2 | 8, 2/2, 75, 130K | 7.5, 2/2, 57, 57K |
| 5 pairs | judge 5.3, turns 717, output 1.72M, input 15.6M uncached + 50.1M cached | judge 6.8, turns 500, output 0.63M, input 26.5M + 58.0M |

The judge gain (+1.5, over the ±0.75 threshold) is rclone's: both 0.0.84 runs there passed 2 of 3 hidden tests and both
0.0.85 runs all 3. Input grows with the reasoning in the history and is cached less (69% against 76%): at LingAI's
prices (cache read assumed ~10% of input) about +25% a run, fractions of a cent; worth it for the quality. Next to try:
keeping only the last turns' reasoning, for the same planning at less input.

## SubRouter routes through the account API: vetted provider keys, live drops, 2026-10-25 (bench only)

SubRouter picks the provider per call from a key's settings (subscribed providers, price, its own stability and
cache-rate weighting); a request cannot name one. Its account API (system access token, `~/.subrouter-account`) can:
`bench/subrouter.py` reads the per-call log (provider, cached tokens, cost: what the CSV exports showed), the providers of
a model with prices, and creates keys limited per model to providers.

- **Vetting** (`subrouter.py vet MODEL`): each provider up to 3x the cheapest input price, through a test key limited to it
  (four at once): an answer, a long reasoning call, identity, and the log for who really served it. Without the cache
  test: its 6 short calls were wrong both ways (FoxRouter 99% in the check, 40-73% in the run after it; LingAI 15% in
  the check, 78-88% in runs). deepseek-v4.1-flash-1017: 10 of 13 pass (QuqiAI and GETAPI answer the long question with
  a tool message, ModelShare over 15 minutes); ~$0.03 for all. A first night's vetting lost 9 providers to transport
  errors (connection resets, 400s that a morning rerun did not show): errors are now retried, not failed.
- **Route key** `tazuna-route-<model>`: limited to the providers that passed; `subrouter.py watch` reads the log during
  runs and drops a provider whose last 15 calls cached under 70%.

Route test 1 (`routes1.sh`, v341-v346, 0.0.84, 1017 at max): default key against the route key, side by side, $0.27.
SubRouter sent both keys to LingAI only (548 calls, 94-95% cached): same tasks results (rclone 3/3 at 217 vs 201
turns, caddy-unix 0/7 at 116 vs 71, caddy-retry 2/2 at 83 vs 89), nothing to drop. A forced switch (v347: a test key
with a6aa and LingAI, cache weighting off, so the cheaper a6aa should come first) sent all 47 calls to LingAI as well:
SubRouter's own ordering (stability first) already avoided a6aa, so the run was stopped. Today the key limits matter
for keeping out providers SubRouter does pick (PixStudio at 5x the price, a6aa at 13-22% cached, both on 2026-10-24);
the live drop has yet to fire on a real run.

## Opus gate on 0.0.83, clean: still no better than DeepSeek alone, 2026-10-25 (bench only)

The first gate test (2026-10-22, judged 6.6 against 8.2) rested partly on runs that saw the reference fix, so it was
redone behind the leak guards (`bench/realwork/gate2.sh`): tazuna 0.0.83, deepseek-v4.1-flash-1017 at max on SubRouter's
default key (FoxRouter; its cache fell from 98-100% to 40-73% from 23:10, so the round cost $0.62 instead of ~$0.35),
alone (`tazuna-ownsubm`) and with the Opus 5.5 high five-lens review on the subscription (`tazuna-ownsubgate`), side by
side, one round (v335-v340); leak scan clean. Judge `judge.oct.gate2.jsonl` (two passes, the earlier 0.0.83 runs
v330/v332/v334 in the same pools, anchors Opus v34 5-9 and Codex Luna 6.5); Claude usage ~2% of the 5-hour window.

| task | alone (v335/v337/v339) | gate (v336/v338/v340) | 0.0.83 alone, earlier (v330/v332/v334) |
|---|---|---|---|
| rclone | 8, 3/3, 177 turns | 7, 3/3, 175 | 8 |
| caddy-unix | 5.5, 0/7, 172 | 5, 0/7, 138 | 4.5 |
| caddy-retry | 8.5, 2/2, 65 | 7.5, 2/2, 42 | 9 |
| mean | 7.33 | 6.5 | 7.17 |

The gate scores lower on every task (-0.83, just over the ±0.75 threshold, one round) and costs Opus usage; the earlier
verdict stands. On caddy-unix, 0.0.83's after-green note came at turn 35-36 in both arms, when a repro test written
before the fix first passed: the runs then went on for 100+ turns, rightly, since the fix was not done. The note
should wait for a change in non-test code as well (next version).

## 0.0.80-0.0.83 (2026-10-24): finish after green, whole-file reads, edit errors with context; -47% turns, judge +2.2

From the clean OpenCode/tazuna comparison on rclone (where tazuna's extra turns went: ~53 a run after its tests first
passed, re-reading, rerunning unchanged tests, builds and repros; reads cut at the 8K bash cap; python-heredoc edits
failing up to 17 times a run). Own loop only.

- **0.0.80**: when a test run passes and the change includes test files, the result gets "check the diff once, then
  finish; rerun only if code changes"; a passing rerun with the tree unchanged gets "nothing changed since the last
  passing run". Appended to the tool output, DeepSeek ignored it: 79 (rclone) and 95 (caddy-unix) more turns after it
  (one PixStudio round, v305-v310, stopped at $0.90).
- **0.0.81**: the same note as its own user turn.
- **0.0.82**: `read` returns the whole file (2000 lines, was 250; cut at a whole line under the output cap, with the
  offset to go on); a failed `edit` shows where the old text's first line is and the lines there; tool descriptions
  steer reads to `read` and changes to `edit`; the own loop's prompt no longer names Grep/Glob (tools it lacks).
- **0.0.83**: a tool call without an id (or with a repeated one) gets its own; LingAI sent 16 empty ids in one run and
  the provider refused the history at call 161.

A/B, deepseek-v4.1-flash-1017 at max on SubRouter's default key (round 1 LingAI, round 2 FoxRouter, 96-99% cached),
both arms side by side, leak scan clean (every attempt at GitHub or the bench repos was refused). Round 1: 0.0.79
against 0.0.81 (v323-v328, $0.36); round 2: 0.0.79b (0.0.79 plus the id fix) against 0.0.83 (v329-v334, $0.34). Judge
`judge.oct.stopgreen.jsonl`, one pool per task, two passes, anchors tazuna on Opus (v34) 3.5-8.5 and Codex Luna 6.

| task | 0.0.79 / 0.0.79b: judge, hidden, turns | 0.0.81 / 0.0.83: judge, hidden, turns |
|---|---|---|
| rclone r1 | 7, 3/3, 216 | 8, 3/3, 123 |
| rclone r2 | 8, 3/3, 231 | 8.5, 3/3, 168 |
| caddy-unix r1 | 4.5, 0/7, 108 | 8.5, 7/7, 74 |
| caddy-unix r2 | 3, 0/7, 175 | 4, 0/7, 72 |
| caddy-retry r1 | died at call 161 (empty ids) | 8.5, 2/2, 74 |
| caddy-retry r2 | 4.5, 2/2, 252 | 9, 2/2, 79 |
| 5 pairs | judge 5.4, turns 982, input 113M, output 2.5M | judge 7.6, turns 516 (-47%), input 41M (-64%), output 1.5M (-40%) |

Over the noise thresholds on every measure, at 5 pairs (fewer than the 12 runs per arm a Tier 1 asks). The note works
as a user turn: 4 of 5 runs given it ended 2-19 turns later (rclone 28-43). Long runs cost quality as well as tokens:
the baseline's 252-turn caddy-retry passed its tests and judged 4.5 (churned code), the 79-turn one 9. The hints move
DeepSeek part of the way: rclone r2 31 `edit` calls and 11 script edits, against 0 and 47 in the baseline. caddy-unix's
7/7 in round 1 is one lucky run: both arms fail it otherwise. 0.0.83 is the own loop's new baseline.

## OpenCode's free DeepSeek, clean: judges like tazuna's DeepSeek in fewer turns, 2026-10-24 (bench only)

Rerun behind the leak guards below (proxy, sandbox; leak scan clean for all six; times on a slow LTE link, not
comparable). OpenCode CLI 2.0.29, `opencode/deepseek-v4.1-flash-free` at its default effort and at `#max`. Judge:
`judge.oct.opencode-clean.jsonl`, one pool per task, two passes (6 calls, ~4% of a 5-hour window); anchors tazuna on Opus
(v34) and Codex Luna.

| task | default (hidden, turns) | `#max` (hidden, turns) | tazuna DeepSeek, clean runs (judge, median turns) |
|---|---|---|---|
| rclone | 6 (3/3, 73) | 8 (3/3, 82) | 6.6 (111); 1017: 7.1 (192) |
| caddy-unix | 4.5 (0/7, 60) | 5 (0/7, 63) | 4.0 (80); 1017: 4.75 (150) |
| caddy-retry | 9 (2/2, 73) | 9 (2/2, 67) | 9 (57); 1017: 7 (74) |
| mean | 6.5 | 7.33 | |

Same quality as tazuna's clean DeepSeek runs (from other pools, so only roughly comparable), in fewer turns on rclone
(73-82 against 111-192) and caddy-unix (60-63 against 80-150); caddy-retry is close. caddy-unix fails for both harnesses
and for Luna (0/7 in 7 clean runs): a model limit, not a harness one. Every OpenCode run tried GitHub or local copies
for the fix and was refused. Next: where tazuna's extra turns go on rclone (clean transcripts only).

## Correction: 91 of 263 tasks-oct runs saw the reference fix; DeepSeek's lead was mostly that, 2026-10-24 (bench only)

Comparing OpenCode's transcripts with tazuna's turned up runs that copied the merged upstream fix instead of writing one.
The agents can read anything on this machine and fetch from the internet, and the fix of every tasks-oct task is public.
Three channels, found with the new `bench/realwork/leakscan.py` (scans every transcript under `~/bench-homes`; writes
`leaks.jsonl`, one line per task and harness label):

- **GitHub** (66 task/label pairs): `curl` of `raw.githubusercontent.com/<repo>/master/...`, the commit API by path, and
  `github.com/.../commit/<fix>.patch`, then a diff against the work tree. DeepSeek did this most, but also Luna (v286
  caddy-retry), MiMo, Haiku and Opus via the `repo` profile on rclone (v34).
- **Go module cache** (20): `~/go/pkg/mod` held caddy v2.11.7 (contains the caddy-unix fix) and a 2026-10-17 pseudo-version
  (contains both caddy fixes), pulled in by earlier runs and shared by every run. The anchor run tazuna-own-v38 DeepSeek
  (judged ≈9, used as an anchor in every pool since) diffed its caddy-unix work against v2.11.7's `listen_unix.go`.
  rclone v1.75.1/v1.75.2 in the cache do not contain the rclone fix and are not counted.
- **The bench repos** (2, plus OpenCode): `git log`/`git show <fix>` in `~/bench-repos/<repo>` (full history); cascade-v38
  rclone and cascade-v34 caddy-retry applied the fix commit verbatim. OpenCode v287/v288 ran with the bench directory as
  their working directory (the harness passed no `PWD`), read `solution.patch` and the hidden tests and edited
  `~/bench-repos` directly: all six OpenCode rows are invalid, and the OpenCode table in the next entry is withdrawn.

Judged score, leaked runs against clean runs of the same task and model (all tasks-oct judges):

| task | model | leaked | clean |
|---|---|---|---|
| caddy-unix | deepseek-v4.1-flash | 8.67 (9) | 4.0 (1) |
| caddy-unix | deepseek-v4.1-flash-1017 | 8.0 (4) | 4.75 (2) |
| rclone | deepseek-v4.1-flash | 8.38 (12) | 6.57 (14) |
| rclone | deepseek-v4.1-flash-1017 | 8.33 (6) | 7.12 (12) |
| caddy-retry | deepseek-v4.1-flash-1017 | 8.4 (5) | 7.0 (1) |
| rclone | gpt-6-luna | - | 5.93 (43) |
| caddy-unix | gpt-6-luna | 8.0 (1) | 5.27 (11) |

What changes: "DeepSeek V4.1 Flash on the own loop judges ≈9" and the routes-by-day DeepSeek results are inflated; clean
DeepSeek runs judge like Luna (rclone 6.6-7.1, caddy-unix ~4-5). The Luna route conclusions (Pro pool against Team pool,
max over xhigh) stand: those runs are nearly all clean. The DeepSeek anchor (v38) is out of the anchor set. An earlier
note here that tazuna's DeepSeek runs "almost always" fail caddy-unix was also wrong: v231/v237/v243 passed 7/7 (v237
leaked; v231 fetched upstream files too).

Fixed in the harness (run.py): every command gets `PWD` set to its own working directory; each task gets its own Go
module cache (`~/bench-homes/gomod/<task>`, filled by the task's setup) and runs with `GOPROXY=off`, so `go` cannot fetch a
newer release mid-run. Then closed, same day:

- **GitHub and Go proxies**: agents and harnesses reach the internet through a CONNECT proxy inside run.py that refuses
  GitHub (incl. raw, API, codeload), the Go module proxies, pkg.go.dev, sourcegraph, gitlab and codeberg; model APIs and
  logins tunnel through. A clean OpenCode run (v290 caddy-retry) tried `git ls-remote`, webfetch and `go list -m
  -versions` against them and got 403 each time.
- **Local copies**: OpenCode v289 then took the fix commit from `~/bench-repos/caddy` instead. Agents now run under
  `sandbox-exec` with reads denied for the task repos, `bench/`, `~/go/pkg/mod` and the transcript stores (Claude, Codex,
  OpenCode); OpenCode and the Claude CLI were checked working inside it. The module-cache copies that held fixes (caddy
  v2.11.7 and the 10-17 pseudo-version, hugo v0.167.0, prometheus v0.315.0) and agents' upstream files in `/tmp` are
  deleted.
- **Detection**: `leakscan.py` runs after every batch, reads OpenCode's sqlite sessions too, remembers fixed module
  copies in `leak-modcache.json` and reports proxy-refused fetches as `web-blocked` (not a leak); the judge and the
  summaries skip leaked rows.

Over all task sets: 114 task/label pairs leaked, 109 of them DeepSeek V4.1 Flash (any variant), 2 Luna, 2 Sol, 1 MiMo.
Rows leaked: tasks-oct 89 of 266 (85 tazuna), tasks 22 of 374 (all tazuna own-loop DeepSeek), tasks-hard 5 of 20. The
Claude CLI's 165 transcripts show none; pi, omp, omo, hermes, empryo and Codex keep no transcripts and cannot be checked
(their rows are Opus/Haiku/Luna).

## Best of two, retested: +0.16, not worth twice the calls; OpenCode's free DeepSeek scores 9 or 1, 2026-10-23/24 (bench only)

(Leak check, 2026-10-24: v286 caddy-retry fetched upstream files; the OpenCode table below is invalid, see above.)

**Best of two** (`tazuna-ownsubxbest2`, bench prototype in run.py): the same own-loop setup twice in two copies of the
work tree, Opus 5.5 high on the subscription (no tools, no reference) picks one with each attempt's own test result.
Retest at night (23:21-01:39 CEST, early morning in China): gpt-6-luna at max on SubRouter's 4-providers key (it served
OpenAI Pro all night, 15-22 s a call in every run), tazuna 0.0.78, both arms side by side, 2 rounds of rclone,
caddy-unix and caddy-retry (v275-v286); both attempts of each pair scored on their own. Judge: `judge.oct.best2retest.jsonl`,
one pool per task, two passes (6 calls); anchors DeepSeek 9, Codex Luna 8; tazuna on Opus (v34) 4.5-9.

| task | one run | best of two: result (picked attempt / other) |
|---|---|---|
| rclone | 8, 8 | 5.5 (5.5 / 7), 7 (7 / 6) |
| caddy-unix | 4, 6 | 5 (5 / 5), 8 (8, 7/7 hidden / 5) |
| caddy-retry | 7.5, 8 | 8 (8 / 7.5), 9 (9 / 7.5) |
| mean | 6.92 | 7.08 |

The pick took the better attempt in 4 of the 5 pairs that differed; the miss was rclone again, where an extra guard
("drops archive members beneath a symlink") won as in the offline check. Best of two helps when one attempt is clearly
bad (caddy-unix: the only tazuna run of the day passing 7/7) and costs twice the model calls and the slower attempt's time
(33-70 minutes against 15-48). +0.16 is within noise: not made a default. The day before, a first try on the 6 Luna key
was spoiled by its OpenAI Team pool (56-148 s a call, a best-of-two attempt at the 3-hour limit): judged 6.0 against 6.5.

**OpenCode with its free DeepSeek** (`opencode` harness: the official OpenCode CLI, `opencode run --standalone --auto
--format json -m opencode/deepseek-v4.1-flash-free[#max]`, its own clean HOME). OpenCode Zen's free models answer only
from within OpenCode (403 "FreeTierError" to any other client), so they are measured in their own harness; effort is a
model variant (this one has `max` and `high`). Same pools as above:

| task | default effort (v287) | `#max` (v288) |
|---|---|---|
| rclone | 9 (3/3) | 1 (1/3) |
| caddy-unix | 9 (7/7) | 9 (7/7) |
| caddy-retry | 1 (1/2) | 1 (1/2) |

Each run judged 9 or 1, in 20-29 turns and 2-7 minutes (tazuna with paid DeepSeek 1017: 60-340 turns, 12-67 minutes,
judged 6-9). caddy-unix: 9 twice where tazuna's runs scored 4-6 and failed the hidden tests (they rewrite the socket
bookkeeping the PR's tests read). A lead for tazuna's own loop: the same model family needs far fewer turns there.

## 0.0.79 (2026-10-23)

- **Routes: the own loop moves to another provider or key when one fails, slows down or stops caching** (design:
  docs/ROUTES.md). `TAZUNA_ROUTES="url key-env; url key-env"` lists more routes for the run's model after `--provider`;
  their keys are read at start and leave the environment, and with `--data trusted` every route's host must be trusted.
  A route is left (`route_switch` in the run log, with the reason) on a call that fails for good (a refusal, or 3 failed
  tries while another route waits: no ~15 minutes of backing off), on 3 empty replies, or, from its own last 10 calls,
  under 70% cached or slow (median over 60 s, or over 40 s and 3x its first 10). On a switch Claude thinking and Responses
  reasoning (bound to the old upstream) leave the history and the per-route flags start over; the new route's cache
  starts cold. No probe calls: the run's own calls are the check, so a run whose first route works costs nothing extra.
  Why: on SubRouter one key put a Luna run on a pool at 56-148 s a call and another on one at 8-16 s for its whole length
  (2026-10-23), a key's routing refused the cache key for an hour, and a key without a caching provider cached 0%.
  Tests: route_left_when_it_fails_slows_or_stops_caching (each rule; a refusal finishing the turn on the next route).
  Not measured on runs yet.

## 0.0.78 (2026-10-23)

- **The cache key goes when a provider refuses it.** GPT models on the Responses API carry `prompt_cache_key` (the
  session id) since 0.0.61: on BeatAPI it lifted the cached share from 18-47% to 96-97%. On 2026-10-23 SubRouter answered
  every gpt-6-luna request with it 503 "该分组下模型无可用渠道" (no channel for the model in this group, `model_not_found`)
  and logged nothing, while the same request without it went through, so a Luna run on that key would have stopped at
  its first call. On that refusal the call goes again without the key, which stays off for the run; `TAZUNA_NO_CACHE_KEY`
  turns it off from the start. The route check (`uno_route.py`) does the same and takes `ROUTE_NO_CACHE_KEY`. A first
  check on the key showed the refusal comes and goes: its first call was taken with the key, the next refused.

## 0.0.77 (2026-10-22)

- **DeepSeek's reasoning goes back when a provider needs it.** On Chat Completions the own loop kept only text and tool
  calls (some providers reject reasoning fields); a gate run on SubRouter's default key ended at call 29 with HTTP 400
  "This reasoning model requires previous reasoning_content in multi-turn requests", most likely after the gateway moved
  it to another provider. Each assistant turn now keeps its `reasoning_content` (from the stream too), still left out of
  requests; on that 400 the call is sent again with it, and for the rest of the run. Memory: the reasoning text of the
  run stays in the history (a few MB at most on a 300-call run).

## Two cheap attempts, Opus picks one: +0.2 to +0.8, the worst run never picked, 2026-10-22 (bench only)

Offline, from runs already judged: for each pair of runs of a task, Opus 5.5 high (`claude -p` on the subscription, no
tools, no reference) sees the task and both diffs in random order and picks the one it would merge (`bench/realwork/pick.py`,
`pick.oct.jsonl`); its pick is then compared with the judge's scores. Thirty picks used ~4% of the 5-hour window (~0.15%
a pick, a third of a five-lens review).

| pool | pairs | picked the better / worse / tie | picked mean vs pair mean | lowest picked |
|---|---|---|---|---|
| DeepSeek 1017 alone, rclone and both caddy tasks (judged 7-9) | 9 | 5 / 2 / 2 | 8.39 vs 8.22 | 8 (pair minimums 7-7.5) |
| Luna, rclone, three routes (judged 2-8.5) | 21 | 14 / 5 / 2 | 6.98 vs 6.21 | 5 |

The run judged 2 lost all six of its pairs; the best (8.5) won five. All five wrong picks in the Luna pool were v181
(judged 5, 2/3 hidden tests), which won every pair with an extra guard (archive members under a symlink dropped: "blocks
the hostile-archive escape") although the picker's prompt counts hardening beyond the task against a patch. In tazuna each
attempt's own verification result would be at hand for the picker, which may have caught it. As a route, best of two
costs one more cheap run (~$0.01-0.04) and ~0.15% of the window for the pick, and raises the floor where single runs
vary most. Next: build it (two attempts in separate work trees, the pick with each attempt's test results) and measure
it end to end.

## Five-lens gate on the own loop: worse, not better, 2026-10-22 (bench only)

The cheap route with a quality gate: DeepSeek V4.1 Flash 1017 at max on SubRouter's default key implements; Opus 5.5 high
reviews the diff on the subscription (`claude -p`, the five-lens prompt, `--review-model`); its Critical/Major findings go
back once to DeepSeek as a fix round; then tazuna's verification. No hand-over (`--no-escalate`) to isolate the review.
Harness `tazuna-ownsubgate` against the same setup without the review (`tazuna-ownsubm`), side by side per task, 3 runs each
on rclone, caddy-unix and caddy-retry (v229-v247, `gate1.sh`; one gate run lost to a provider error, replaced by v247).
Judge: one pool per task, two passes (`judge.oct.gate1.jsonl`, `judge.oct.gate1r.jsonl`, six calls; anchors DeepSeek 8,
Codex Luna 7), with tazuna on Opus 5.5 medium (v34) for reference.

| task | alone | with the gate | tazuna on Opus (v34) |
|---|---|---|---|
| rclone | 8, 7.5, 8 (7.8) | 7, 7.5, 5 (6.5) | 4.5, 5 |
| caddy-unix | 9, 9, 8 (8.7) | 6, 4.5, 5 (5.2) | 4, 5 |
| caddy-retry | 7, 8.5, 9 (8.2) | 6.5, 9, 9 (8.2) | 8.5, 8.5 |
| all | 8.2, hidden tests 9 of 9 runs | 6.6, 5 of 9 | |

Turns 121 against 177, 30 against 46 minutes a run; $0.73 real on SubRouter for all 19 runs (~$0.04 a run); each review
~0.5% of the 5-hour window, 61-137 s (about $0.45 a review at API prices). Reviews found something in 3 runs of 9 (v230:
every zip member scanned the whole directory; v232: a socket registry pointing at a closed socket; v242: three symlink
cases) and the fixed runs judged 7, 6 and 5: the fix widened the patch away from what the task asked (v232's rewrite also
broke the hidden tests' build). Six reviews found nothing; two of those runs (v238, v244) were weak already, from
DeepSeek's own redesign of the socket bookkeeping (the hidden tests read a `count` field it removed: they do not compile,
0/7). With 3 runs an arm part of the gap is chance, but the gate never came out ahead. DeepSeek 1017 alone beat tazuna on
Opus on these three tasks (8.2 against ~6) at about a cent each. Not adopted: a review with a fix round by the cheap model
does not raise its floor (as with Claude implementers, 2026-10-08 and 2026-10-13).

## 0.0.76 (2026-10-22)

- **No prompt on a command line.** The own loop's engine got its system prompt, the repo's CLAUDE.md and AGENTS.md
  included, as an argument; in a benchmark round of 18 parallel runs one agent cleaned up its test servers with
  `pkill -f "integration.test"`, which matched caddy's AGENTS.md text ("integration tests") in every engine's arguments
  and stopped ten runs at once (in real use the same command would stop tazuna itself; the prompt was also readable by
  anyone running `ps`). The engine now reads it from `TAZUNA_SYSTEM_PROMPT` and drops it from its environment, so the
  agent's commands do not inherit it; the claude path's lean profiles pass `--system-prompt-file` (a file named by the
  prompt's hash in a private `~/.cache/tazuna/prompts`, 0700: not the shared temp dir, where on Linux another user could
  plant it); the reviewer reads the task and diff on stdin. Checked: the same `pkill` against a run in a repo
  whose AGENTS.md says "integration tests" leaves the engine running and the task finishes; a Haiku review reads its
  input from stdin (verdict in 4.9 s).

## Routes by day: Luna on the OpenAI Pro pool judges 8 for $0.005; UnoRouter DeepSeek over SubRouter, 2026-10-22 (bench only)

rclone, China evening (12:24-15:20 CEST); max and xhigh side by side on one key and route, SubRouter's log split by the
effort on each call (every call went out at the effort asked). Real cost: SubRouter's usage log, UnoRouter's counter.
Judge calls: `judge.oct.day6.jsonl` (the Sol, LingAI-Luna and UnoRouter runs), `judge.oct.luna-routes.jsonl` (all Luna
runs of 2026-10-21/22 in one pool) and `judge.oct.ds-sites.jsonl` (DeepSeek on both sites, both days), two each; anchors
DeepSeek 8-9, Codex Luna 7-7.5. Runs judged twice moved 0.5-1.

| run | key, route | effort | judge | tests | turns | time | real cost |
|---|---|---|---|---|---|---|---|
| gpt-6-luna (v205 / v204) | Luna pools key, OpenAI Pro | max / xhigh | 8 / 6 | 3/3, 3/3 | 67 / 72 | 21 / 18 min | $0.0050 / $0.0029 |
| gpt-6-luna (v201 / v200) | 4-provider key, LingAI | max / xhigh | 8 / 8.5 | 3/3, 3/3 | 105 / 63 | 35 / 17 min | $0.034 / $0.015 |
| gpt-6.1-sol (v197 / v196) | default, 子木 ai | max / xhigh | 7.5 / 4.5 | 3/3, 2/3 | 75 / 111 | 21 / 22 min | $0.054 / $0.073 |
| DeepSeek 1017 (v199) | UnoRouter, zlkpro | max | 8-9 | 3/3 | 117 | 19 min | $0.009 |
| DeepSeek V4.1 Flash (v198) | UnoRouter, ih-cb | max | 6.5 | 3/3 | 336 | 52 min | $0.016 |
| DeepSeek 1017 (v203) | default, Kriora | max | 6 | 3/3 | 133 | 42 min | $0.015 |
| DeepSeek V4.1 Flash (v202) | default, a6aa | max | 5.5 | 3/3 | 213 | 56 min | $0.057 |

Luna by route, all seven runs in one pool: LingAI 8.5/8, OpenAI Pro pool 8/6, FoxRouter (2026-10-21, early morning in
China) 6/5/2. The route moves Luna more than the effort does; max is ahead on two routes of three (mean 7.3 against 5.4 at
xhigh). The Pro pool is the cheapest good route: 8 for half a cent, a seventh of LingAI. The default key sent Luna to a6aa,
which cached nothing (0% in three checks, 13:14-13:25); a key with FoxRouter and the pools sent the check to FoxRouter,
also at 0%; the pools alone cached 99%. Sol: max over xhigh this time (7.5 against 4.5), the other way on 2026-10-21 (7
against 8.5): one run each does not settle it. DeepSeek: UnoRouter 1017 judged 8-9 in 19 min for under a cent; SubRouter's
DeepSeek judged 5.5-6 in the China evening against 8-8.5 the night before, V4.1 Flash on a6aa at 3.6x 1017's cost on
Kriora. UnoRouter 1017 needed four tries to find a group: its cheapest group took over 15 minutes for the long call, the
next cached 9% or answered 502/503.

Shannon again: of its GPT groups only 0.12x serves Luna and Sol (0.02x "upstream access forbidden" as on 2026-10-20, 0.19x
and 0.37x without the models, 0.27x Sol only); Luna there failed the long call (over 15 minutes), Sol passed (73% cached,
616 s long call). Luna on 0.12x and Sol on 0.27x bill ~4,400 input tokens for a one-line question (a hidden system prompt);
Sol on 0.12x 44.

Bench tooling: the SubRouter run, its watcher and the spend guard live in the repo (`bench/realwork/sub_run.sh`,
`sub_guard.sh`; a restart of the Mac at 12:10 cleared their `/tmp` copies, the tazuna binary the runs used and three runs
in flight). The no-edit stop counts a change in the work tree as an edit (DeepSeek edits through python in bash: 321 calls
without an edit tool, 618 lines changed). `sub_summary.py` reads the effort from the run's start line and splits two runs
of one model side by side by it; `uno_run.sh` takes `HARNESS_BIN` and no longer prints an arithmetic error on a run without
a group switch.

Later the same day (`judge.oct.sol-sites.jsonl`, `judge.oct.ds-flash.jsonl`, two calls each; anchors DeepSeek 8-9, Codex
Luna 7): Sol at max on Shannon 0.12x (v206) 3/3 in 16.5 minutes, 86% cached, ~$0.52 of credit (~$0.07 at the 2026-10-20
rate), judge 7, beside SubRouter's Sol runs 7.5-8 (v185, v167, v197) and Shannon 0.27x on 2026-10-20 at 4. DeepSeek V4.1
Flash on the default key with a6aa disabled went to Kriora (v207: 339 calls, 93% cached, $0.052, 67 minutes) and judged
6.5, as v202 on a6aa: the same quality and cost, a third less per call but more calls; UnoRouter's V4.1 Flash 7.5-9 for
$0.002-0.016. Mortera: Opus 5.5 left every prompt uncached (four calls on a 26K prefix marked for caching, each billed in
full; its streamed replies report 0 input tokens), so a run would cost ~$1.80 there; Sol at max (v208) spent $0.20 in 28
minutes unfinished, then was stopped by our own watcher after a pause (below) and is left out.

The run watcher counted an hour's `kill -STOP` as an hour without a reply and stopped v208 as it resumed; a gap of over
five minutes in its own loop (a pause or a sleeping Mac) now restarts the count. `sub_one.sh` runs the route check, then
one watched run with its own spend guard; `sub_guard.sh` also reads Mortera's balance (`GUARD_SITE=mortera`, IDR).

## Effort and time of day on SubRouter: Luna max over xhigh, Sol xhigh over max; MiniMax free passes, 2026-10-21/22 (bench only)

rclone, one run each unless noted; real cost from SubRouter's usage log (`sub_summary.py`), UnoRouter at tazuna's estimate.
The night round ran staggered (two rounds of three plus MiniMax, $0.75 cap per round) at 21:50-01:33 CEST; all calls of a
run went to one route. Judge: `judge.oct.night2.jsonl`, two passes (two calls, 5h meter +1%); anchors DeepSeek 8, Codex
Luna 7.5.

| run | route | effort | judge | tests | turns | time | real cost |
|---|---|---|---|---|---|---|---|
| DeepSeek V4.1 Flash (v187) | UnoRouter | max | 9 | 3/3 | 228 | 29 min | ~$0.002 est |
| DeepSeek 1017 (v182 / v183) | SubRouter, Kriora | xhigh | 8.5 / 7 | 3/3, 3/3 | 287 / 199 | 66 / 52 min | $0.051 / $0.026 |
| DeepSeek 1017 (v188) | UnoRouter | max | 8 | 3/3 | 184 | 54 min | ~$0.018 est |
| gpt-6.1-sol (v185 / v184) | SubRouter, 数游AI | xhigh | 8 / 5 | 3/3, 2/3 | 53 / 59 | 14 / 15 min | $0.085 / $0.088 |
| gpt-6-luna (v189) | SubRouter, FoxRouter | max | 6 | 3/3 | 87 | 16 min | $0.016 |
| gpt-6-luna (v181 / v180) | SubRouter, FoxRouter | xhigh | 5.5 / 2 | 2/3, 1/3 | 73 / 28 | 14 / 8 min | $0.015 / $0.003 |
| MiniMax M3.1 Flash free (v186) | SubRouter, XCGX | xhigh | 6 | 3/3 | 372 | 3.5 h | $0 |

Effort. SubRouter's log shows the effort each call went out with: v189 forwarded max on all 87 calls, the xhigh runs xhigh.
Luna on SubRouter (tazuna runs since 2026-10-20) judges 6.3 at max (7, 6, 6; tests 3, 2, 3 of 3) and 4.8 at xhigh (8, 6,
5.5, 4, 3.5, 2; tests 3, 3, 2, 2, 2, 1): max ahead by 1.5, over the noise threshold but on few runs, other days and routes.
Max does not think longer per call on FoxRouter (499 output tokens per call against 649-1018 at xhigh; the new effort check
below found no more reasoning at max than at xhigh there), it works longer (87 calls against 28-73). gpt-6.1-sol goes the other way:
max judged 7 against xhigh's 8.5 in a blind pair with v167 (`judge.oct.sol-max.jsonl`), at 3x the cost ($0.082 against
$0.026). Sol at xhigh: 8, 8, 8, 5, 4.

Time of day. The same Sol model, effort and key judged 8 in the evening (v167) and 4 at Chinese daytime (v174, 139 min against
16; `judge.oct.drift.jsonl`, UnoRouter DeepSeek v173 6.5 in the same pair). Daytime Luna and DeepSeek 1017 (v175 6, v176 6.5,
v177 on the pool key 3.5) against evening (v129 8, v131 8; `judge.oct.daypair.jsonl`): 1.5-4 points lower, 3-8x slower.

Route price. gpt-6.1-sol on 数游AI costs $0.085 a run, 3x the same model on LingAI or ChorAPI ($0.019-0.026) and 4x tazuna's
estimate; the log has no provider prices for it. MiniMax M3.1 Flash free changed no file in its first ~90 calls (1.2 min and
~5K output tokens each) and still passed 3/3 at 6 for $0: free models now run without the no-edit stop (time limit only).

Bench tooling: `uno_route.py MODEL --effort` asks a long question twice at high, xhigh and max (streamed, in parallel) and
prints the highest level that reasons more than the one below (`EFFORT <level>`, None when a level is cut or unmeasured);
`sub_summary.py` merges new runs into `sub-summary.json` and flags runs that shared a window with another run of the same
model (v174, v175, v177: their spend is not theirs alone), which `site_summary.py` then leaves at tazuna's estimate;
`site_summary.py` groups by effort too; `run.py` has harnesses for a key chosen per run (`ownsubx`) and MiniMax free
(`ownsubmm`). The usage guard sends its own User-Agent (`tazuna-guard/1`) instead of Claude Code's: the meter answers the same.

## SubRouter, more models: Gemini, Qwen, Opus, Grok pool, Sol family; real cost from the usage log, 2026-10-20/21 (bench only)

rclone, all on SubRouter. Real cost is SubRouter's own usage log (the console's CSV export: 500,000 quota per USD, within
5% of the account counter), read per run by `bench/realwork/sub_summary.py`, which also writes `sub-summary.json` (tokens
billed, routes and their prices, run and generation tok/s, first-token time) for ranking in the app later. Judge calls:
`judge.oct.sub4.jsonl` and `judge.oct.sol2.jsonl`, one each; DeepSeek anchor 9 in both, Codex Luna 9 and 7.

| run | route | judge | tests | turns | time | real cost |
|---|---|---|---|---|---|---|
| gpt-6.1-sol (v167) | ChorAPI | 7 | 3/3 | 44 | 16 min | $0.047 together with v168 |
| gpt-6-sol (v165) | (between the two exports) | 7 | 3/3 | 113 | 55 min | under $0.236 (that with v166) |
| Grok 4.7, X Premium+ pool, high (v161) | pool | 6 | 3/3 | 40 | 21 min | $0.020 |
| gpt-6-sol-plus (v170) | OrangeAPI | 6 | 3/3 | 125 | 83 min | $0.179 |
| gpt-5.6-sol (v163) | ModelShare | 5 | 2/3 | 81 | 18 min | $0.040 |
| gpt-6.1-sol-plus (v169) | 星球中转站 | 4 | 3/3 | 103 | 64 min | $0.096 |
| gpt-6.1-sol-new (v168) | ChorAPI | 4 | 2/3 | 31 | 12 min | see v167 |
| Opus 5.5 (v156) | fate86, viapi | 3 | 2/3 | 129 | 20 min | $0.467 |
| Opus 4.8 (v145) | BangAi | 1 | 0/3 | 73 | 11 min | $0.072 |
| for reference: DeepSeek 1017 / Luna / Sol / Astra (v131/v129/v137/v140) | | 8 | 3/3 | 33-67 | 8-16 min | $0.009 / ~$0.005 / $0.019 / $0.120 |

Cut before finishing (most at a $0.45-0.80 cap): Opus 4.7 (老六 reports ~200K tokens per call for ~35K of context,
~$0.008-0.009 per call; $0.48 and $0.52), Opus 5 (曲奇ai $0.475 at 224 calls; 老六 and the Claude shared pool at about 4x that
per call, the pool at 8.5x its token prices), Opus 4.8 on Stable AI ($0.47 and $0.67), Qwen 3.8 Max ($1.14 in 203 calls, at a6aa's $0.279 per M
input). Qwen 3.8 Flash (twice, 242 and 503 calls) and Gemini 3.8 Flash (257 calls) never changed a file.

What decides a SubRouter run is the route, not the model name: the same Opus cost $0.001-0.016 per call by provider.
Routes that drop the effort (a6aa and xcgx on Opus: reasoning_effort None in the log), add hidden prompts (老六 ~20-40K,
one-api ~68K), stop caching (BangAi on Opus 5 and later on Opus 4.8: 0-1%; LingAI's cache stalls at a fixed prefix) or
bill the official price (Claude 共享池, ratio 1.0) make a run 4-10x dearer or not a real xhigh run. A smart-routing key
also falls back to shared subscription pools and official channels when its providers fail (曲奇ai answered 503 "no
available accounts" all evening): the key settings "Include shared subscriptions" (now off) and "Maximum official-channel
price ratio" (now 0.2) close that; gpt-5.6-sol-fast is only served above the 0.2 ratio and did not run. Reasoning effort:
Gemini 3.8 Flash thinks at none and high, not at xhigh or max, Qwen 3.8 and Grok 4.7 reason the same at any level.

On this task the cheap models stay on top: DeepSeek 1017, Luna and gpt-6.1-sol at 7-8 for $0.005-0.03; Grok on the pool
passes the hidden tests for $0.02 but judges 6; the Opus models judge 1-3 at 20-50x the cost. One run per model: ±1-2.

Bench tooling: the route check (`uno_route.py`) now streams Claude calls as tazuna does (a provider left the cache fields
out of unstreamed replies, which read as 13 input tokens for a 27K conversation), counts cache writes as input, re-asks
without tools when Claude answers with a tool call, reads SubRouter's untiered price formulas, and takes
`ROUTE_CACHE_BLIND` where the bill was measured to ignore the cache; `run.py` keeps the work of a run that ends in an
error as `<run>.diff.partial` (the judge reads `*.diff` only) and has a harness for a second SubRouter key (`ownsubgp`).

## Shannon by day: DeepSeek 0.14x and Sol 0.27x, 2026-10-20 (bench only)

rclone, one judge call (`judge.oct.shn2.jsonl`):

| run | judge | tests | turns | time | cost |
|---|---|---|---|---|---|
| Sol, SubRouter one key (v137) | 8 | 3/3 | 33 | 14 min | ~$0.016 |
| DeepSeek, Shannon 0.14x by day (v135) | 7 | 3/3 | 142 | 39 min | |
| DeepSeek, Shannon 0.14x at night (v127) | 6 | 3/3 | 75 | 11 min | ~$0.009 paid |
| Sol, Shannon 0.27x (v136) | 4 | 2/3 | 103 | 99 min | ~$1.2 credit, ~$0.17 paid |
| anchors: tazuna own-v38 / Codex Luna | 9 / 7 | | | | |

The same Sol scored 4 on Shannon and 8 on SubRouter, at ten times the cost and seven times the time; the judge called the
Shannon run over-engineered (an O(N*M) pass dropping zip members, per-call directory walks). Of Shannon's GPT groups only
0.27x served Sol (0.02x forbidden, 0.12x "overloaded", 0.19x not listed, 0.37x unavailable). Shannon's DeepSeek scores
6-7 by day or night.

## GPT-6 Astra on the one SubRouter key; more runs, 2026-10-20 (bench only)

Astra's cheapest providers on SubRouter bill about 1% of its list price (老六 ¥0.595 in / ¥2.975 out / ¥0.0595 cached
per M, with a 21% "reference evaluation" against 53-56% for the next two). Check: 98% cached, the long question answered
exactly (571) after 518 s. rclone, one judge call (`judge.oct.astra.jsonl`), all on the shared key:

| run | judge | tests | turns | time | billed |
|---|---|---|---|---|---|
| Astra (v140, xhigh) | 8 | 3/3 | 38 | 14 min | 0.85 yuan (~$0.125) with its check |
| Luna (v129) | 8 | 3/3 | 67 | 8 min | ~$0.005 |
| DeepSeek 1017 (v131) | 8 | 3/3 | 62 | 16 min | ~$0.0095 |
| Sol (v137) | 7 | 3/3 | 33 | 14 min | ~$0.016 |
| anchors: tazuna own-v38 / Codex Luna | 9 / 6 | | | | |

On this task Astra scored no higher than Luna or DeepSeek at 13-25x their cost; Sol and Astra need about half the turns.
Also today: Shannon DeepSeek 0.14x by day (v135) 3/3 in 39 minutes at 55-63% cached (95% at night); GLM 5.3 Flash and
Muse Spark 1.3 on Mortera again read for an hour without changing a file (Muse stopped after 30 minutes without a reply,
GLM recovered from a 28-minute call); UnoRouter 1017: of the two cheapest groups one cached 10%, the other returned an
empty long answer after 31 minutes (the check's 15-minute limit was swallowed as an OSError; fixed).

## Is SubRouter's gpt-6-luna the real model? Codex on SubRouter, 2026-10-20 (bench only)

The same harness on two providers: Codex CLI (`codexsub`, a model provider pointed at SubRouter's Responses API with the
shared key) against the Codex reference run (Codex CLI on the user's ChatGPT plan, official gpt-6-luna). rclone, one
judge call (`judge.oct.codexsub.jsonl`):

| run | judge | tests | commands | time | billed |
|---|---|---|---|---|---|
| Codex, official gpt-6-luna (reference) | 7 | | | | ChatGPT plan |
| Codex, SubRouter gpt-6-luna (v138) | 7 | 3/3 | 162 | 11 min | 0.030 yuan (~$0.0044) |
| tazuna, SubRouter gpt-6-luna (v129) | 8 | 3/3 | 67 | 8 min | ~$0.005 |
| tazuna, SubRouter gpt-6-luna, cheapest provider (v119) | 7 | 3/3 | 115 | 15 min | ~$0.004 |
| anchor: tazuna own-v38 (DeepSeek) | 9 | | | | |

Same harness, same score: nothing points to a weaker model behind SubRouter's gpt-6-luna. The judge's notes on both
Codex runs name the same habits (double lookups in squashfs, symlink-specific offset handling in zip). tazuna on the same
provider scored a point higher with fewer than half the commands.

## GPT-6.1 Sol on the one SubRouter key, 2026-10-20 (bench only)

Sol on the shared models key: the check cached 98% and answered the long question exactly (571, 279 s). It bills at about
3% of the official list ($2 in, $0.10 cached, $10 out per M): 20 calls cost 0.079 yuan against $0.39 at list. The first
run (v134) was restarted after 20 calls because tazuna counted the list price and was about to reach its cap; with the
real price (0.06, 0.003, 0.30 per M):

| run | judge | tests | turns | time | billed |
|---|---|---|---|---|---|
| Sol, one key (v137, xhigh) | 8 | 3/3 | **33** | 14 min | 0.109 yuan (~$0.016) |
| Luna, one key (v129) | 8 | 3/3 | 67 | 8 min | ~$0.005 |
| DeepSeek 1017, one key (v131) | 7 | 3/3 | 62 | 16 min | ~$0.0095 |
| anchors: tazuna own-v38 / Codex Luna | 9 / 6 | | | | |

(`judge.oct.sol.jsonl`; v131 judged 8 the night before.) Sol solved the task in half the turns of any other model, with
"very thorough" tests, for under 2 cents.

## One SubRouter key for DeepSeek and Luna, 2026-10-20 (bench only)

The models-only key with its providers restricted to the ones that cache for DeepSeek 1017 (kriora, lingai, a6aa and one
more) and routing by stability, then cache-adjusted price. The check cached 0% on the first two calls (other providers),
then 7,424, 6,400, 9,600 and 10,624 tokens: 80% over the test, long call not cut.

| run | judge | tests | turns | time | cached | billed |
|---|---|---|---|---|---|---|
| DeepSeek 1017, one key (v131, xhigh) | 8 | 3/3 | 62 | 16 min | 97-98% | 0.065 yuan (~$0.0095) with its check |
| DeepSeek 1017, three providers (v114) | 9 | 3/3 | 113 | 37 min | ~87% | 0.32 yuan |
| DeepSeek 1017, three providers (v118) | 8 | 3/3 | 69 | 19 min | ~92% | <= 0.18 yuan |
| DeepSeek 1017, lingai + a6aa (v117) | 8 | 3/3 | 92 | 23 min | 96% | |
| anchors: tazuna own-v38 / Codex Luna | 8 / 7 | | | | | |

(`judge.oct.onekey.jsonl`.) With Luna on the same key (v129: 8, ~$0.005), one key serves both models at 8 for about a
cent a run or less. A key per model and provider is not needed: the key lists the models, allows the providers that
work for each, and routes by stability, then cache-adjusted price.

## One SubRouter key for all models: routing order, 2026-10-19 (bench only)

The models-only key (DeepSeek 1017, Luna, Haiku 5.5; no provider pinned) with its routing reordered: stability, then
price adjusted for cache hits, then first-token speed, credit, evaluation (before: stability, evaluation, credit,
first-token speed, price).
- Luna (v129, xhigh): **judge 8** in one pass with v119 (7, cheapest provider pinned), v125 (4, same key in the old
  order) and Codex with Luna (7); 3/3, 67 turns, 7.6 minutes, 3.7M input, ~0.033 yuan (~$0.005) with two checks.
- DeepSeek 1017: the check cached 0% twice, so no run. In the old order the same key put it on an official-price
  provider (v124, ~5x the fixed key). A new provider, manxueai (满血AI), joined 1017's list during the evening; one
  call that set a cookie for api.a6api.com went to a6aa. For DeepSeek the key has to allow only providers that cache
  (kriora, lingai, a6aa: judged 9 three times at ~$0.03 a run).
- Correction: the DeepSeek checks that "cached 0%" counted replies without usage as uncached; `uno_route.py` now prints
  each call and fails on a reply without usage. Re-run: with price first, the first call timed out (524) twice; with
  stability first and no provider restriction, all six calls answered with real usage and none cached (8,450 tokens
  sent twice, 0 cached), at 0.01 yuan for the check. Unrestricted, the key routes DeepSeek 1017 to a provider that
  does not cache.
- So one key can serve every model: allowed providers = the good ones per model, routing = stability, then
  cache-adjusted price.

## Provider and key comparison, night of 2026-10-19 (bench only)

rclone, one judge call (`judge.oct.night.jsonl`):

| run | judge | tests | turns | time | cost |
|---|---|---|---|---|---|
| DeepSeek 1017, SubRouter three providers (v118) | 9 | 3/3 | 69 | 19 min | <= 0.18 yuan |
| Luna, SubRouter cheapest provider (v119) | **8** | 3/3 | 115 | 15 min | ~$0.004 at the key's prices |
| DeepSeek V4.1 Flash, Shannon 0.14x at night (v127, xhigh) | 7 | 3/3 | 75 | 11 min | ~$0.06 credit, ~$0.009 paid |
| Luna, SubRouter 4 providers (v120) | 6 | 2/3 | 152 | 95 min | |
| Luna, SubRouter models-only key, smart routing (v125, xhigh) | 5 | 2/3 | 93 | 11 min | |
| Haiku 5.5 "mh-kiro", SubRouter (v121, xhigh) | 2 | 2/3 | 127 | 11 min | <= 0.045 yuan |
| anchors: tazuna own-v38 / Codex Luna | 9 / 8 | | | | |

- Luna on SubRouter is best on its single cheapest provider ($0.003 in, $0.0003 cached, $0.015 out per M): judged at
  Codex's level for about $0.004 a run; four providers or smart routing scored 5-6.
- The Kiro Haiku reports fake usage (1 token in, 0 out), thinks ~300 characters and answered the counting question wrong
  (2,411 and 2,439 for 571); its run judged 2. SubRouter's default Haiku 5.5 fails its streams after ~50 s without data,
  since its providers send nothing while Haiku thinks ("upstream stream read failed ... 等待响应超时").
- GLM 5.3 (kriora) and GLM 5.3 Flash (foxrouter) on SubRouter cached 15% and 31% in the check and 23-25% in runs;
  stopped after ~20 minutes at 0.67 yuan for both.
- Shannon at night (04:00 in China): DeepSeek 0.14x cached 100% in the check and ~95% over the run, 11 minutes against
  80 by day; Luna still waited 65-190 s for the first token; the 0.02x and 0.05x groups still answered 502/429.
- DeepSeek 1017 on a models-only SubRouter key (smart routing, v124) was stopped after ~90 calls: over 56 calls it billed
  0.87 yuan (~$0.13) for 5.1M input (98% cached) and 138K output, about the official DeepSeek price and roughly 5x the
  fixed three-provider key (v118: <= 0.18 yuan for a whole run). Smart routing ranks stability and evaluation before
  price, so for DeepSeek the providers have to be fixed in the key.
- The cost cap needs the price in real dollars: SubRouter's listed rates are a generic price (its 1017 list tripled during
  the day while real charges did not move), so `TAZUNA_PRICE` takes the provider's own price from the console.

## SubRouter: DeepSeek 1017 three times at 9, Luna at 7, 2026-10-19 (bench only)

rclone, one judge call (`judge.oct.sub3.jsonl`):

| run | judge | tests | turns | time | cached |
|---|---|---|---|---|---|
| 1017, three providers (v114) | 9 | 3/3 | 113 | 37 min | ~87% |
| 1017, three providers, price in real dollars (v118) | 9 | 3/3 | 69 | 19 min | ~92% |
| 1017, lingai + a6aa (v117) | 9 | 3/3 | 92 | 23 min | 96% |
| Luna, cheapest provider (v119) | 7 | 3/3 | 115 | 15 min | 98% |
| Luna, Mortera xhigh (v107) | 5 | 3/3 | 99 | 18 min | ~90% |
| anchors: tazuna own-v38 / Codex Luna | 9 / 7 | | | | |

- DeepSeek V4.1 Flash 1017 on SubRouter: three runs, all 9 in one pass, 19-37 minutes, about $0.03-0.05 each (one shared
  usage counter across the keys). The most consistent setup measured so far.
- Luna on SubRouter's cheapest provider matched Codex with Luna (7) in 15 minutes; Mortera's Luna at xhigh scored 5.
  SubRouter's Luna upstream adds about 4,400 tokens of `instructions` of its own to every call (cached).
- Effort on SubRouter's DeepSeek 1017 (reasoning tokens, streamed counting question): low 29.1K and 7.9K, xhigh 28.5K and
  32K+ (capped), max 23.2K and 17.4K: xhigh thought most both times, max below it, as on Mortera.

## Repeat runs, 2026-10-19 evening (bench only)

rclone, one judge call (`judge.oct.eve.jsonl`):

| run | judge | tests | turns | time | cached |
|---|---|---|---|---|---|
| 1017, SubRouter three providers (v114, afternoon) | 9 | 3/3 | 113 | 37 min | ~87% |
| 1017, SubRouter lingai + a6aa (v117) | 8 | 3/3 | 92 | 23 min | 96% |
| 1017, UnoRouter (v111) | 9 | 3/3 | 59 | 10 min | 57-78% |
| Luna, Mortera xhigh (v107, afternoon) | 5 | 3/3 | 99 | 18 min | ~90% |
| Luna, Mortera xhigh (v116) | 4 | 2/3 | 170 | 59 min | 94% |
| anchors: tazuna own-v38 / Codex Luna | 8 / 7 | | | | |

- DeepSeek V4.1 Flash 1017 holds at 8-9 on SubRouter whichever providers serve it; fixed to lingai + a6aa it was the
  fastest SubRouter run and cached 96%. Both SubRouter keys share one usage counter: v115, v117 and a check together
  billed 0.53 yuan (~$0.078).
- A second three-provider run (v115) was stopped by its own $0.50 cap at ~$0.07 of real spend: tazuna was given
  SubRouter's listed rates as dollars, about 6x its real charge (v114: tazuna $0.27, billed 0.32 yuan). For SubRouter,
  `TAZUNA_PRICE` should be about 0.17x the list (0.035, 0.0007, 0.14 per M), so the cap means real money.
- Luna at xhigh on Mortera: 5 and 4 in this pass, 2/3 hidden tests once; it varies a lot from run to run and stays
  below DeepSeek.
- Shannon's GPT groups again (18:35): 0.12x and 0.27x Pro answered the reasoning question right (109) after 67-141 s to
  the first token and ~12 minutes in all; short calls took 27-360 s, half of them empty; 0.19x and 0.37x still do not
  serve gpt-6-luna.

## SubRouter with three providers, 2026-10-19 (bench only)

The same 1017 run with the SubRouter key routed over kriora, lingai and a6aa (the site's own ranking: stability, then
reference score, provider credit, first-token speed, then price adjusted for cache hits), instead of kriora alone.
Judged in one call with the earlier 1017 runs (`judge.oct.sub2.jsonl`):

| run | judge | tests | turns | time | cached | billed |
|---|---|---|---|---|---|---|
| SubRouter, three providers (v114) | **9** | 3/3 | 113 | 37 min | ~87% at the end | 0.32 yuan, ~$0.047 |
| SubRouter, kriora only (v112) | 7 | 3/3 | 112 | 61 min | ~62% | 0.47 yuan, ~$0.069 |
| UnoRouter, zlkpro then hly (v111) | 9 | 3/3 | 59 | 10 min | 57-78% | 2.65 cents |
| anchors: tazuna own-v38 / Codex Luna | 8 / 7 | | | | | |

The pre-run check cached 55% with three providers (83% with kriora alone), yet over the run the cache rate rose to 87%:
the router's "adjust for cache hit rate" seems to keep a conversation where it caches. Faster and a third cheaper than
kriora alone, with no visible kriora 524s slowing it.

## DeepSeek V4.1 Flash 1017 on UnoRouter and SubRouter; Shannon's Luna groups, 2026-10-19 (bench only)

rclone, one judge call (`judge.oct.1017.jsonl`):

| run | judge | tests | turns | time | cached | billed |
|---|---|---|---|---|---|---|
| 1017, UnoRouter zlkpro then hly (v111, 0.0.75) | **9** | 3/3 | 59 | 10 min | 57-78% | 2.65 cents |
| 1017, SubRouter kriora (v112) | 6 | 3/3 | 112 | 61 min | ~62% | 0.47 yuan, ~$0.069 |
| V4.1 Flash, Mortera, default effort (v98) | 8 | 3/3 | 74 | 9 min | 97% | ~$0.018 |
| anchors: tazuna own-v38 / Codex Luna | 8 / 7 | | | | | |

- The first group switch in a real run: v111 left zlkpro at call 21 ("57% cached over the last 10 calls") for hly and
  finished there. UnoRouter billed it at almost the uncached price (2.65 cents against tazuna's 0.1-cent estimate), six
  times the regular V4.1 Flash runs (~0.4 cents). An earlier 1017 run on one checked group ended at call 43 when that
  group vanished (0.92 cents); the check script crashed on a group answering `"choices": null` (fixed).
- SubRouter (new-api, shown in yuan at 6.8 per dollar): the check passed (83% cached, long call not cut); the run cached
  ~62%. Some calls failed with a 524 that SubRouter logs as "json: cannot unmarshal number 1792415975.375 into Go struct
  field ... of type int64" (kriora sends a fractional timestamp); unbilled, retried by tazuna. The judge marked it down
  for reading every zip link target when listing.
- Shannon's GPT groups for gpt-6-luna, speed test (time to the first token at max effort): 0.12x 14 s to 601 s (a 504
  from its gateway), 0.27x Pro 180-248 s, 0.19x and 0.37x Fast do not serve gpt-6-luna at the moment (404, though on
  their price lists). A Luna run on 0.12x reached the 3-hour limit after ~50 calls.

## 0.0.74-0.0.75 (2026-10-19)

- **0.0.74: on Shannon the first token is waited for 600 s** (180 s elsewhere). Its GPT groups queue calls before the
  first token (gpt-6-luna on 0.12x: median 69 s, up to 396 s in its own usage export; "Say OK." took 14 s, 179 s and
  then a 504 from its gateway at 601 s), and a call tazuna gave up on still ran and was billed: the first request of a
  run went three times.
- **0.0.75: the group list can be a file** (`TAZUNA_GROUPS_FILE`), read again at each switch. `uno_run.sh` keeps the
  file: when at most one listed group still serves the model, it checks the groups not tried yet in the background
  (`uno_route.py` with `UNO_SKIP`) and adds those that pass; the run stops only when no group is left and no check is
  running. A DeepSeek V4.1 Flash 1017 run (UnoRouter, one checked group, 0.92 cents) ended at call 43 when that group
  vanished from the price list.

## Effort on Mortera, and Shannon runs, 2026-10-19 (0.0.72-0.0.73)

0.0.73: a set `TAZUNA_PRICE` skips the provider's model list (Shannon lists a model only while an account serving it is
online; gpt-6-luna came and went within the hour). rclone, one judge call (`judge.oct.xhigh.jsonl`; this pass scored the
anchors 1-2 points lower than earlier ones, so compare within it):

| run | effort | judge | tests | turns | time | cost |
|---|---|---|---|---|---|---|
| DeepSeek, Mortera (v98) | default (max ignored) | 8 | 3/3 | 74 | 9 min | $0.018 |
| DeepSeek, Mortera (v106) | xhigh | 8 | 3/3 | 214 | 48 min | ~$0.10 |
| DeepSeek, Shannon 0.14x (v108) | max | 7 | 3/3 | 203 | 80 min | $1.03 credit, ~$0.14 paid |
| Luna, Mortera (v99) | default | 4 | 2/3 | 150 | 37 min | $0.033 |
| Luna, Mortera (v107) | xhigh | 6 | 3/3 | 99 | 18 min | $0.012 |
| anchors: tazuna own-v38 / Codex Luna | | 8 / 6 | | | | |

xhigh lifted Luna two points at a third of the cost; DeepSeek scored the same at 5.7x the cost, so DeepSeek on Mortera
is best at its default effort (quality per dollar). Shannon's DeepSeek cached 67-77% over the run.

## 0.0.72 (2026-10-19)

On Mortera Cloud:
- **`max` effort is sent as `xhigh`.** Its sources take OpenRouter's effort levels, which end at `xhigh`; Mortera forwards
  the value untouched and accepts any string (a made-up "banana" passed on all four models). DeepSeek V4.1 Flash on a
  counting question, streamed, output tokens: `low` 12.2K and 10.6K, `xhigh` 21.0K and 25.0K, `max` 16.2K and 13.8K. The
  Mortera runs before this (DeepSeek 9, Haiku 7, Luna 6) ran at the model's default effort, not at its highest.
- **A `cache_affinity_key` per conversation** (a hash of its first messages): Mortera keeps the conversation on the same
  upstream source for up to an hour, for warm caches and no switch to an account that rejects earlier thinking blocks.
Haiku's effort is likely lost either way: Mortera carries thinking settings only on a native Anthropic source, Haiku's
usage comes back in OpenAI's shape, and through Chat Completions any effort field is refused (400 or 422).

Shannon (Sub2API 0.2.17, docs on Feishu: base URL `https://api.shannonapi.xyz/v1`, tools set to Chat Completions): the
DeepSeek 0.14x group (cache read $0.0028/M) cached 0-100% over ten six-call tests, about 48% on average, with or without
`session_id`/`conversation_id` headers, `prompt_cache_key` or `user` (one combination hit 100% once, then 53% and 0%):
about $0.065 a run in real money, 3.6x Mortera. Luna on the ChatGPT 0.12x group answered once (Responses, streamed,
`high`, no output cap: 311 s, 568) and otherwise came back empty or `502`. The two 0.02x/0.05x groups never answered.
Parked.

## 0.0.71 (2026-10-19)

- **A 400 on a conversation with thinking blocks drops them and goes again** (Messages API). A thinking block's
  signature holds only on the upstream account that wrote it; Mortera Cloud's failover moves a conversation to another
  account and then answers `400 "rejected the request: it is invalid"` (reproduced with a forged signature; the same
  request without thinking blocks goes through). Haiku 5.5 died at call 39 on 0.0.70; on 0.0.71 the drop fired 2 and 3
  times in two runs and each went on.
- **Mortera's `422 model_unavailable` is tried again like a 5xx**, though it says not to retry: it passed within a minute
  for Muse Spark and ended a Haiku run at call 38.

## 0.0.70 (2026-10-19)

**Mortera Cloud** (`https://mortera.cloud/v1`, rupiah pay-as-you-go, cache at cache rates): no top-level
`cache_control` there, and GPT unstreamed there.
- Mortera sets its own cache breakpoints and writes none when a request brings a top-level one: Haiku 5.5 0% cached over
  25 calls on 0.0.69, 92-96% on 0.0.70.
- Its streamed Responses carry no usage: GPT-6 Luna looked 0% cached and the cost cap counted nothing; unstreamed, Luna
  ran at 96% cached.

## Three more gateways, 2026-10-18/19 (bench only)

`uno_route.py` now checks any gateway (`ROUTE_API`, `ROUTE_KEY_ENV`; Apizio's time-of-day price formula, Mortera's
rupiah list, Shannon at the official price times the key's group rate) and streams its test calls off UnoRouter.

| gateway | model | check | run (rclone) |
|---|---|---|---|
| Apizio (new-api, $5 free) | DeepSeek V4.1 Flash (`n/`) | no reply in 100 s, 10 tries over 2 hours | - |
| Apizio | GLM 5.3 | cache passed; long call cut twice (549 s) | - |
| Mortera | GLM 5.3 Flash | cache 99%, long call 60 s, answer 566 | stopped: one call open 27 min after 27 calls |
| Mortera | DeepSeek V4.1 Flash | cache 99%, long call not cut | **judge 9**, 3/3, 74 turns, 9 min, 97% cached, ~$0.018 |
| Mortera | GPT-6 Luna | cache 79%, long call 102 s | judge 6, 2/3, 150 turns, 37 min, 96% cached, ~$0.033 |
| Mortera | Haiku 5.5 | cache 94%, long call 5 s | 0.0.69 uncached; 0.0.70 died on thinking (39 calls); 0.0.71 on a 422 (38), on Cloudflare 524 after an hour-long upstream outage (44); then, alone, **judge 7**, 3/3, 33 turns, 19 min, 144K output (its stream no longer reported input, so cache and cost only from the balance) |
| Mortera | Muse Spark 1.3 (free) | 422 in the cache test twice; streamed with an effort, "model unavailable" | - |
| Shannon (sub2api, pooled accounts) | DeepSeek (0.05x group), GPT-6 Astra (0.02x; Luna not in the group) | "upstream temporarily unavailable" / "access forbidden", 11 tries over 2 hours | - |

Judged in one pool with the anchors (`judge.oct.mrt.jsonl`; the second pool, `judge.oct.mrt2.jsonl`, with Haiku gave every other run the same score): Mortera DeepSeek 9 ("matches the reference fix"), tazuna
own-v38 9, Codex Luna 8, UnoRouter DeepSeek v91 7, Mortera Luna 6 ("an unrequested zip pre-scan ... fails that hidden
test"). Mortera spent Rp 2,074 (about $0.13) on 5 checks and 8 runs or attempts; Apizio 3.4 cents of free credit; Shannon
nothing. Five runs at once on one Mortera key: DeepSeek ran at 79 tokens/s (83.7 alone on UnoRouter); GLM's long calls
fell from about 50 to 12-24 tokens/s, unclear whether from the load or the provider.

## `--self-review` on the own loop, DeepSeek on UnoRouter, 2026-10-18 (no code change)

Weak DeepSeek runs are scope failures in the judge's notes: "over-engineered", "an unrelated DetachAfterHijack method",
"rewrites the internal types ... the hidden tests do not compile". `--self-review` (0.0.14: finish rules in the prompt,
one turn reviewing the diff when the model first stops, "revert changes the task did not ask for") already reaches the
own loop; measured only on Opus before. Three runs (harness `tazuna-ownunorev`, groups ih-cb then b2capi, no switch),
judged in one pool per task with the uno-2 runs and the anchors (`judge.oct.rev.jsonl`):

| task | uno-2 (no review) | with review | review turn |
|---|---|---|---|
| caddy-retry | 9, 2/2, 57 turns, 0.37 cents | 9, 2/2, 46 turns, 0.28 cents | 9 calls, 3 small edits |
| rclone | 6, 3/3, 61 turns, ~0.43 cents | 8, 3/3, 80 turns, 0.96 cents | 11 calls |
| caddy-unix | 9, 7/7, 57 turns, ~0.43 cents | 4, 0/7 (suite does not compile), 80 turns, 0.70 cents | 17 calls, no edit |

Mean 7.0 vs 8.0 at one run per task (judge noise 1.5 per run), billed +58% (1.94 vs 1.23 cents; the three runs shared
one counter, split by input tokens). The caddy-unix failure came before the review: the same redesign as v76 (a new
shared socket type, the map type and the `count` field changed). The review read the diff, called it all needed and
changed nothing: "revert what the task did not ask for" catches extra changes, not a fix larger than it needs to be.
Decision: stays opt-in; no evidence it lifts DeepSeek, and it does not touch the failure that costs most.

## UnoRouter: what 29 hours of prices say, 2026-10-18 (bench only)

The public price list read every 10 minutes from Saturday 14:25 to Sunday 19:18 (170 readings,
`bench/realwork/uno-prices.jsonl`, logger `uno_price_log.sh`). Per model, per hour:

| model | groups seen / listed >95% | groups appear / vanish | cheapest group changes | cheapest input, min-median-max $/M |
|---|---|---|---|---|
| DeepSeek V4.1 Flash | 50 / 26 | 3.1 / 3.1 | 2.4 | 0.0057-0.0084-0.0130 |
| GPT-6.1 Sol | 29 / 16 | 0.6 / 0.6 | 2.2 | 0.266-0.280-0.300 |
| Grok 4.7 | 15 / 8 | 0.5 / 0.5 | 3.6 | 0.0086-0.100-0.100 |
| MiMo V2.6 Flash | 17 / 6 | 0.7 / 0.6 | 1.4 | 0.024-0.044-0.121 |
| GLM 5.3 | 24 / 5 | 1.5 / 1.5 | 0.5 | 0.011-0.011-0.098 |
| GPT-6 Luna | 23 / 10 | 0.4 / 0.5 | 0.07 | 0.0125-0.021-0.021 |
| Haiku 5.5 | 7 / 4 | 0.1 / 0.1 | 0.14 | 0.043-0.043-0.050 |

Picking the cheapest group and keeping it: for DeepSeek it was gone within 30 minutes in 39% of cases, within an hour in
49%, within 12 hours in 90%; Luna's was gone within 12 hours in 11%. A group that stays rarely gets dearer by much within
an hour (at most 2x for DeepSeek, 1.2x for Sol). The one large move was model-wide: DeepSeek's output multiplier stood at
120x input for about four hours (Saturday 15:38-19:29; 3.8-4.9x otherwise), with the input price unchanged.

Groups that have been listed for a while stay: of DeepSeek's cheapest groups 43% vanished within the hour, of the cheapest
listed for 1, 3 and 6 hours 20%, 10% and 6%, at 1.10x, 1.18x and 1.25x the price (MiMo: 13% to 4% at 1.12x; Sol 8% to 2%
at 1.03x). At 0.4 cents a run the 18% costs nothing that matters, while three vanishing groups end the run. Changes:
- `uno_route.py` tries groups listed in every reading of the last 3 hours first (when the logger runs), cheapest first
  within each half;
- `uno_run.sh` stops a run when a typical run at the current price (input, cache and output) costs over 1.5x the
  estimate at the start; it compared only the input price and would have missed the 120x output.

How often to read prices: before every run (free, one request), and every few minutes during it only as a stop guard. A
daily list is stale (90% of DeepSeek's cheapest picks gone after 12 hours); a read before every call buys nothing (a group
that stays moves little within a run); switching groups mid-run for a lower price saves fractions of a cent and costs a
cache miss, so it is not built.

## UnoRouter uno-2: a first default, 2026-10-18 (no code change)

DeepSeek V4.1 Flash on UnoRouter with group switching (0.0.69; three groups passed the check: sinnode, zlkpro, zangin):

| task | tests | judge | turns | time | cached | billed |
|---|---|---|---|---|---|---|
| caddy-retry | 2/2 | 9 | 57 | 9 min | 98% | 0.37 cents |
| caddy-unix | 7/7 | 9 | 57 | 10 min | 97% | ~0.43 cents |
| rclone | 3/3 | 7 | 61 | 9 min | 98% | ~0.43 cents |

`leaderboard.py uno-2`: judge 8.4 +- 1.1, worst 7, floor 7.5, no failure: **DeepSeek V4.1 Flash is the first model to meet
the default rule** (3+ runs, floor 7 or more, at most one failure in five). For comparison, DeepSeek on CheaperInference
scored 8-9 at 2.4-5.2 cents a run. No group switch was needed in these three runs, so switching is so far tested only in
unit tests. The two runs of one launch share the billing counter; each is about half of the 0.86 cents logged.

## 0.0.69 (2026-10-18)

**A 400 that UnoRouter passes on from an upstream ("Provider returned error ... Baidu", `bad_response_status_code`) moves
to the next group** like a dropout, instead of ending the task. The first two runs with group switching (DeepSeek, rclone
and caddy-retry over ih-cb and burngate) both ended on such a 400: caddy-retry after the task was solved (its summary was
written; the 400 came in tazuna's follow-up, so the solution was not saved), rclone after 185 calls. **Group switches are
written to the run log** (`system` / `group_switch`); they went only to stderr, which no one kept.

## 0.0.68 (2026-10-18)

**UnoRouter group switching in the own loop.** `TAZUNA_GROUPS=a,b,c` (cheapest first, the groups that passed
`bench/uno_route.py`, which now prints up to three): calls go to the current group with an `X-Group` header; when it drops
out (`get_channel_failed`, "No permission to access this group", "no available provider") the same call goes at once to
the next, and when the last 10 calls on it cached under 70% the next call does. No listed group left: the task ends with
the error, as before (no fall back to automatic routing, which picked dear groups). Why: in one day four groups vanished
between check and run or mid-run; two of them ended working DeepSeek runs (99 and 146 calls, 20 and 15 edits) that a
switch would have kept going. Prices are not re-read during a run yet (`uno_run.sh` watches them). Also 0.0.67: Experiential
Labs' hourly free limit (`insufficient_quota` with code `free_limit_reached`) is waited out instead of read as an empty
balance; a flaky test server that answered before reading the whole request is fixed.

## 0.0.66 (2026-10-18)

**A rate limit is waited out for up to 70 more minutes** (the last try repeated every 5 minutes) instead of ending the
task after ~15 minutes of tries: Experiential Labs' free DeepSeek V4.1 Flash 1017 allows 166,500 output tokens an hour,
reset on the hour, and two runs hit it at once; BeatAPI's 15 requests a minute stalled runs the same way. A run can be
interrupted while it waits. Bench: `uno_run.sh` logs why it did not start (a DeepSeek group that passed the check was gone
by the start, the second group to vanish between check and run), and the launcher checks again in that case.

## UnoRouter: Grok 4.7 and GLM 5.3 through the check, 2026-10-18 (bench only)

- **Grok 4.7:** a7-bin-704 passed (100% cached, long call 527 s, 573) and was closed to the key a minute later (403 "No
  permission to access this group"); a7-bin-709 passed too (100%, 434 s, 570), but its price rose from $0.024 to $0.10
  per M input during the run (4.1 cents for 19 calls instead of ~1): stopped. Every Grok group costs $0.10 or more now
  (~20 cents a run). Since then `uno_route.py --price` reads the price again at the start and `uno_run.sh` refuses or
  stops a run on a price rise, a vanished group or a failing cache.
- **GLM 5.3 Flash:** four groups cache (80-100%; ~2-7 cents a run), but none finished a long reasoning call: empty after
  467-486 s, an error after 180-189 s, or held open for over 40 minutes. Not usable for hard steps on UnoRouter today.
- **GLM 5.3:** cache reads cost the full input price there; the groups that cache come to $0.28-0.46 a run, the
  cheapest group cached 20-32%.
- The check takes at least 50% cached now (the price at the measured rate decides above that), asks again without tools
  when a model calls one (GLM ignores tool_choice none), and gives a group at most 15 minutes.

## UnoRouter: the full check before a run, 2026-10-17 (bench only)

`bench/uno_route.py MODEL` now runs, for the cheapest groups by the price of a whole run, until one passes: the live
price (input, cache, output) against OpenRouter's public list; caching over a conversation grown like an agent's (at
least 80%); a long reasoning call with tools that must come back; an English answer near the right number. It prints
`PIN <group>`. DeepSeek V4.1 Flash: a7-sinnode-5969 (cheapest) came back empty after 296 s, a7-ipeenk-5188 cached 34%,
a7-burngate-5609 passed (0.46 cents for the check). OpenRouter's reference for a typical run: $0.12; UnoRouter's groups
$0.007-0.011. The run pinned to a7-burngate-5609: rclone 3/3, 111 turns, 27 minutes, 12.9M input, 91% cached,
**1.9 cents billed, judge 8**, the same as CheaperInference (8 and 8 in the same pool, $0.024-0.052); the references
scored as before (Codex 7; UnoRouter's default key 9 and 6).

## UnoRouter: Haiku and Sol, pinned groups, 2026-10-17 (no code change)

Group check first (long reasoning with tools): GLM 5.3 reasoned 0-156 tokens on its cheap groups and si answered 429
after 180 s, so it was left out; all four cheapest Haiku groups returned thinking blocks; one of five Sol groups
(a7-gpt-4848) finished, four cut the call at 180 s. Sol's price there had doubled ($0.30 / $1.50 per M).

| rclone, 3 runs each | tests | judge | turns | time | cost |
|---|---|---|---|---|---|
| Haiku 5.5 high, **tazuna's Claude path** (subscription) | 3/3, 3/3, 3/3 | **7, 8, 8** | 36-54 | 8-10 min | 5-hour window 7% -> 9% for all three |
| Haiku 5.5 high, UnoRouter pinned to a7-hiyo-codex-5544 | 0/3, 0/3, 1/3 | **1, 2, 3** | 43-52 | 7 min | ~6 cents together |
| Sol max, UnoRouter pinned to a7-gpt-4848 | none finished | | 25-28 calls | | ~$1.40 together |

The UnoRouter "Haiku" wrote its summaries in Chinese to an English task and scored 1-3: the group most likely serves
another model or adds its own prompt. The Sol runs cached 54% and cost ~$0.45 each by call 28 (BeatAPI's whole run:
$0.39); they emptied the $5 balance, then the group went offline (`get_channel_failed`). References in the same pool
scored as before (Codex 7, DeepSeek CheaperInference 9, Haiku BeatAPI 8, Haiku Claude Code at max 4).

## 0.0.65 (2026-10-17)

**`TAZUNA_HEADER="Name: value"` adds one header to every model call**, e.g. `X-Group: <group>` to pin an UnoRouter group
for a run without a key per group.

Why Luna scored 2-4 through UnoRouter: a long reasoning call with tools on the Responses API never returns on some groups
(CX-Sale-duck 605 s, ih-cx 992 s, a7-ai-router 1048 s: no output), so the cheap groups cut the hard steps; Chat
Completions is no way out (with tools Luna does not reason there: 0 reasoning tokens, wrong answer). Groups that finished
a long call: a7-misy-3451, a7-jisol-5564, a7-misy-5255. DeepSeek reasons fully through UnoRouter (92,801 reasoning tokens
against 99,911 on CheaperInference for the same question).

Judged with the same reference runs as the pass below (they scored within a point of it):

| rclone etc. | tests | judge | turns | time | cost |
|---|---|---|---|---|---|
| Luna, UnoRouter pinned to a7-jisol-5564 | 3/3 | 6 | 210 | 159 min | ~$0.20 counted |
| Luna, BeatAPI / Codex (reference) | 3/3 | 7 / 7 | | 19 / 7.5 min | $0.11 / plan |
| DeepSeek, UnoRouter default key, rclone x2 | 3/3, 3/3 | 9, 7 | 134-149 | 34-37 min | |
| DeepSeek, UnoRouter default key, caddy-unix x2 | **0/7**, 7/7 | **4**, 9 | 86-143 | | |
| DeepSeek, UnoRouter default key, caddy-retry x2 | 2/2, 2/2 | 9, 9 | 65-126 | | |
| DeepSeek, CheaperInference (control) | 3/3, 7/7, 2/2 | 8, 9, 8 | 60-77 | 12-15 min | $0.04-0.05 |
| DeepSeek, UnoRouter (earlier run) / CheaperInference, 0.0.64, rclone | 3/3, 3/3 | 8, 8 | 81, 77 | 17, 15 min | ~$0.01, $0.05 |

Luna on a vetted UnoRouter group works but is slower, dearer and no better than BeatAPI. DeepSeek on UnoRouter's default
key matched CheaperInference on 5 of 6 runs; one broke the package's tests. With automatic routing its runs cached less
(57-83% against 96% earlier) and took about twice the turns, and UnoRouter billed ~$1.27 for these 7 runs against ~$0.55
estimated at the cheapest group's prices: routing also used dearer groups. CheaperInference's balance ran out (402).

## Judge pass, 2026-10-17 (no code change)

One judge pool per task (Opus xhigh with the reference fix, 1 pass, so a score can swing by about 2), 3 calls: 5-hour
window 4% -> 6%, week 0% -> 1%, no extra usage.

rclone, latest run per model and provider (tests, judge, billed):

| model | provider | tests | judge | cost per run |
|---|---|---|---|---|
| DeepSeek V4.1 Flash | CheaperInference (0.0.38, anchor) | 3/3 | **9** | $0.024 |
| DeepSeek V4.1 Flash HC | BeatAPI, 0.0.57 | 3/3 | 8 | $0.18 |
| DeepSeek V4.1 Flash | UnoRouter (ih-cb) | 3/3 | 5 | ~$0.03 |
| GPT-6.1 Sol | BeatAPI | 3/3 | 8 | $0.39 |
| GPT-6.1 Sol | UnoRouter | 3/3 | 8 | $0.11 |
| GPT-6 Luna | BeatAPI, 2 runs | 3/3, 3/3 | 7, 7 | $0.08-0.11 |
| GPT-6 Luna | Codex CLI (ChatGPT plan) | 3/3 | 7 (8 in two earlier pools) | plan |
| GPT-6 Luna | UnoRouter si, xhigh / max | 3/3, 2/3 | **4, 2** | ~$0.02 |
| MiMo V2.6 Flash | UnoRouter | 3/3 | 7 | ~$0.03 |
| Claude Haiku 5.5 | BeatAPI, old route, 2 runs | 3/3, 2/3 | 8, 4 | ~$0.05-0.09 |
| Claude Haiku 5.5 | UnoRouter | 2/3 | 4 | $0.15 |
| Claude Haiku 5.5 | Claude Code (subscription) | 2/3 | 3 | plan |

The same model scores lower through UnoRouter than elsewhere (Luna 2-4 vs 7, DeepSeek 5 vs 8-9; Sol is the exception at
8), and UnoRouter reports a third of the output tokens: its upstreams likely give the model less reasoning, so part of its
low price is a weaker model. The cheap UnoRouter Luna runs are not a win.

DeepSeek HC on BeatAPI by tazuna version (rclone, caddy-unix, caddy-retry): 0.0.53 (two rounds) 6/8, 4/9, 9/5 = 6.83;
0.0.55 8, 7, 1 (no change on caddy-retry) = 5.33; **0.0.57 8, 9, 8 = 8.33**, the best of the three, with fewer turns and
less input (see 0.0.61).

## 0.0.64 (2026-10-17)

**An empty 200 to an unstreamed call is sent again as a stream** (UnoRouter): some of its upstreams cut a slow
unstreamed call at ~90 s with an empty 200 (0 tokens in and out) instead of the 413 handled in 0.0.63. (Written by
another assistant while this one was at its usage limit; reviewed, tested.)

GPT-6 Luna max on UnoRouter with automatic group routing (cheapest group of the hour, not si), 2026-10-16 evening: 1/3,
44 turns, 69 minutes, 3.6M input, 89% cached, ~2 cents, and no edit at all (four empty replies in a row above 105K, before
this fallback existed); a second run with the fallback edited and tested, then was stopped by its watch at 49% cached
after 40 calls (4.6 cents). On the si group earlier that day: 3/3 and 2/3 in 7-9 minutes. The group decides: pin a caching group
(`uno_route.py`) rather than trust automatic routing.

## 0.0.63 (2026-10-16)

**UnoRouter calls are no longer streamed** (a 413 for a long call switches that call to a stream): UnoRouter caches only
calls that are not streamed. Same conversation on its si group: streamed 0 / 0 / 0 tokens cached on calls 2-4, not
streamed everything before the new text; the billing log never showed a cache read for a streamed call. This explains
every UnoRouter GPT run so far (0-35% cached). BeatAPI caches streamed calls (97%), so streaming stays the default
elsewhere; `TAZUNA_STREAM=0/1` decides for any provider. **A call rejected for `max` effort goes again at `xhigh`**
instead of ending the task (one of si's upstreams accepts none ... xhigh only; the 400 is free and takes 1 s).

GPT-6 Luna, rclone, UnoRouter's si group (today's cheapest, $0.021 / $0.0021 cached / $0.105 per M):

| | tests | turns | time | input | cached | billed |
|---|---|---|---|---|---|---|
| 0.0.61, streamed, max | stopped by the 400 after 10 calls | | | | 0% | |
| 0.0.61, `TAZUNA_STREAM=0`, xhigh | 3/3 | 122 | 404 s | 3.7M | 92% | 1.6 cents |
| 0.0.63 defaults, max | 2/3 | 130 | 515 s | 5.9M | 96% | 1.9 cents |
| BeatAPI, cache key, max (0.0.60, 2 runs) | 3/3, 3/3 | 70, 102 | 17-19 min | 4.5-7.1M | 97% | 8-11 cents |

The fastest and cheapest Luna runs so far (not judged yet). UnoRouter reports few output tokens (12-16K per run against
42-50K on BeatAPI), though the model reasons. Group prices change by the hour (si was the dearest group the night
before): **`bench/uno_route.py MODEL`** lists a model's groups by today's price (UnoRouter's public `/api/pricing`) and
tests caching on the cheapest few with a growing conversation, through the `X-Group` header (no key per group).

Other models through UnoRouter's cheapest group (probe, then one rclone run at max):
- **MiMo V2.6 Flash:** UnoRouter 3/3, 211 turns, 105 minutes, 20.5M input, 98.5% cached, ~2.8 cents billed. BeatAPI
  ($0.098 / $0.00196 / $0.196 per M, caches streamed calls): 147 calls in 99 minutes without changing a file (searching
  and printing code only), stopped at ~15 cents. (The probe's 0% for MiMo, DeepSeek and GLM was a bug in the probe:
  UnoRouter's Chat Completions usage has `input_tokens: 0` next to `prompt_tokens`, and the probe read the first.)
- **DeepSeek V4.1 Flash:** UnoRouter 3/3, 174 turns, 30.3 minutes (1821 s), 18.7M input, 97.4% cached, ~3.2 cents billed ($0.021 counted). Both `ih-cb` and the `a7-*` groups cached ~100% in the probe once `uno_route.py` read `prompt_tokens` when `input_tokens` is `0` (`a7-*` returns `"input_tokens": 0` alongside `prompt_tokens`). **GLM 5.3** (si, $0.011 per M): 37%.

## 0.0.62 (2026-10-16)

**The server-side clearing of old tool results (0.0.60) is removed.** Haiku 5.5 lost code it had read, read it again and
never got to the change: at high effort on BeatAPI 163 turns, 0 edits, $0.45, stopped (60% cached: each clearing
rewrites the cache); at max 146 turns without finishing (~$0.8). Keeping the last 5 tool results is too few, and a
higher number would save little. Claude's context is left to grow; effort is the lever for its size.

**GPT-6 Luna through UnoRouter's a7-5053 group** (the best of five group keys in a 4-call probe: every repeat cached):
the real run cached 58% early and nothing after 10:20, calls took a median 77 s, and at 116 minutes and 37 turns the
group went offline (`get_channel_failed`), ending the run; 9.1 cents billed. Its log shows why: a miss is billed as a
cache write at 1.25x input (~10x a hit), and the input count fell five times between consecutive calls (100,236 ->
93,400) although the history only grows: the group's upstreams add hidden prompts of different sizes, so a switch is a
miss. A short probe cannot show this. GPT runs stay on BeatAPI with the cache key (97% cached, 17-19 minutes, $0.08-0.11).

## 0.0.61 (2026-10-16)

Four own-loop changes, each measured on rclone (October set) with the build named; not judged yet (weekly limit).

**0.0.58: Claude models (`claude-`) use Anthropic's Messages API.** Thinking goes back with the history (signature and
all, as Anthropic requires during tool use), `thinking: adaptive` with `output_config.effort` (so max works), a top-level
`cache_control` breakpoint that moves with the conversation, and all tool results of a turn in one user message. Through
a gateway's Chat Completions the thinking was dropped and effort rejected: one of three Haiku runs ended in nothing but
empty replies. Haiku 5.5 max on BeatAPI, 3 runs: no empty reply and no nudge in 139 calls, but kept thinking grew the
context to 143-334K, past BeatAPI's 100K line where Haiku costs 5x; 16 calls missed the cache entirely (after calls held
by BeatAPI's limit of 15 requests per minute per key, and some 12-22 s after a hit). About $3 billed for the three;
stopped at 43-48 turns.

**0.0.59: GPT models send `prompt_cache_key`** (the session id) on the Responses API. Probe on BeatAPI, same 10K-token
prompt 4 times: without it 31% cached and ~3,800 more input tokens per call; with it 99.97% from the second call.

| rclone, own loop, BeatAPI | tests | turns | input | cached | billed |
|---|---|---|---|---|---|
| GPT-6.1 Sol max, 0.0.58 | stopped | 29 | ~1.7M | 18% | $1.35 |
| GPT-6.1 Sol max, 0.0.59 | 3/3 | 42 | 3.1M | 96.5% | $0.39 |
| GPT-6 Luna max, 0.0.55, 2 runs | 3/3, 2/3 | | | 47% | ~$0.19 each |
| GPT-6 Luna max, 0.0.60, 2 runs | 3/3, 3/3 | 70, 102 | 4.5M, 7.1M | 97% | $0.08, $0.11 counted |

**0.0.60: Claude models clear old tool results on the server** (context editing, `clear_tool_uses_20250919`): above 80K
input tokens the oldest go, the last 5 stay, at least 20K at a time. The context fell from 80K back to ~22K each time,
but BeatAPI served 3 of the first 62 calls uncleared (106-198K): its upstreams differ. Haiku max still thought a lot
(1.23M output tokens in 146 turns) and hit the 3-hour limit without finishing: ~$0.8 billed, no result. Max effort is
not worth it for Haiku on this route.

**0.0.61: the 180 s call limit is on the reply's start only;** a started stream may run 15 minutes. Cut at 180 s,
UnoRouter's slow Luna calls (100-500 s) were billed anyway, with the input counted twice, and then sent again. Luna max
on UnoRouter's default group with the fix, 2 runs: both hit the 3-hour limit without finishing (~100 turns in the first 2
hours, 0% cached, calls of 100-500 s), $1.00 billed. BeatAPI did the same task in 17-19 minutes.

Also measured:
- **DeepSeek V4.1 Flash HC, 0.0.53 vs 0.0.57** (BeatAPI, same time, 3 tasks): tests 12/12 both, turns 528 vs 332,
  input 48.4M vs 21.8M, $0.68 vs $0.30. With the round before (0.0.53 vs 0.0.55): tests 17/24 vs 23/24, input -32%, cost
  -42%. Times not comparable (rate limit).
- **GPT-6.1 Sol on UnoRouter** (0.0.58): 3/3, 70 turns, 4.3M input, 95% cached, $0.11 billed.
- **UnoRouter** routes each model through groups of upstream accounts, set per API key. Luna probe, same 9K prompt 4
  times per group key: a7-5053 and ih-cx hit the cache on every repeat (~$0.010 and ~$0.012 per M input effective);
  CX-Sale-duck (the default) 1 in 3 (~$0.017); si (list price) 2 in 3 (~$0.068); a7-hlgbao1908-3844 never, and adds
  ~2,500 prompt tokens per call. Its Luna reasons (77-322 output tokens for a two-character answer) but reports no
  reasoning tokens; its Haiku returns no thinking at all.
- **BeatAPI:** 15 requests per minute per key across all models; more than 2-3 runs at once only add waiting. It bills
  cache writes at the 1-hour rate and settles minutes late.

## 0.0.57 (2026-10-15)

**Claude models on a gateway's Chat Completions route:** no `reasoning_effort` for `claude-` models (BeatAPI rejects
Haiku 5.5 with one: "requires adaptive thinking"; the model then runs at the gateway's default effort). **Streamed tool
calls are keyed by their `index`:** BeatAPI counts indexes across content blocks (text 0, tools 1, 2, ...), which left an
empty call at index 0 and failed the next request ("custom tool name is invalid").

Haiku 5.5 on rclone (October set; hidden tests, not judged yet):

| | tests | turns | time | input (cached) | output | cost |
|---|---|---|---|---|---|---|
| Claude Code through tazuna (subscription, max) | 2/3 | 118 | 3697 s | 47.6M (98%) | 441K | plan ($5.88 reported by the CLI) |
| own loop, UnoRouter, max | 2/3 | 91 | 2023 s | 6.4M (76%) | 174K | $0.15 billed |
| own loop, BeatAPI, default effort, 3 runs | 3/3, 1/3, 2/3 | 45, 19, 48 | 873-1353 s | 0.6-2.7M (73-95%) | 50-97K | $0.03-0.06 counted, billed ~1.5x |

On average the own loop matches Claude Code with Haiku at about 1/40 of the cost, but BeatAPI's runs vary. The 1/3 run
changed nothing: after 17 turns every reply came back empty, through 3 retries and both nudges. Likely cause: the
OpenAI-format translation drops Claude's thinking blocks, so a turn that ends in thinking alone arrives empty, and asking
again repeats it. Next: the own loop speaks Anthropic's Messages API for Claude models (thinking kept, effort set, cache
control).

## 0.0.56 (2026-10-15)

**The own loop streams its model calls** (`stream: true`; `TAZUNA_STREAM=0` turns it off) and puts the pieces back into a
whole reply. The answer is the same; gateways cap replies that are not streamed (UnoRouter: complete within 90 s, which a
130K-token prompt cannot meet: HTTP 413, and the task ended). Checked live on BeatAPI, Experiential Labs and UnoRouter.
**An empty reply (no text, no call) is asked again** up to 3 times (5, 15, 30 s) before the nudge; an error inside a
stream is retried like a 5xx. **Usage in either API's field names** (UnoRouter answers the Responses API with Chat
Completions usage).

GPT-6 Luna max on rclone, one judge pool (Opus xhigh with reference, 1 pass):

| | tests | judge | cost per run |
|---|---|---|---|
| Codex CLI (ChatGPT plan) | 3/3 | **8** (8 in the earlier pool too) | plan |
| tazuna own loop, BeatAPI, 0.0.55, 2 runs | 3/3, 2/3 | 8, 5 | $0.19 |
| tazuna own loop, BeatAPI, 0.0.54 | 3/3 | 6 (8 in the earlier pool) | |
| tazuna own loop, UnoRouter, 0.0.55, 2 runs | 1/3 (three empty replies), 2/3 | 1, 4 | $0.05 |

Codex is consistently at 8; tazuna with Luna reaches 8 but varies (5-8), and the same patch scored 6 and 8 in two pools,
so one judge pass swings by about 2 points. UnoRouter's Luna route returned empty replies and failed with "All accounts
failed. Tried 3 accounts": it pools accounts, so its GPT models are of the same unclear origin as apinex's. Not used for
GPT or Claude models.

## 0.0.55 (2026-10-15)

**A headless own-loop task no longer ends the moment the model answers without a tool call while nothing has changed.**
It gets up to two nudges ("you stopped without changing any file; continue, or reply DONE and say why"); an empty reply
is nudged in chats too. Before, any reply without a tool call counted as done: two rclone runs (BeatAPI's regular
DeepSeek route, UnoRouter) stopped after a plan or an empty reply, changed nothing, and were reported as successes; the
repo's own tests could not catch it, since they still pass with no change. Nudges show in the transcript.

**Luna vs Codex, again** (rclone, one judge pool, Opus xhigh with reference): Codex 8; tazuna 0.0.54 8 (input 6.3M vs
4.7M); 0.0.53 8; 0.0.52 (BeatAPI) 5; 0.0.38 (CheaperInference) 7. Quality equal now; input 1.3x Codex's.

**UnoRouter** (named German operator, OpenAI-compatible, `https://api.unorouter.com/v1`), DeepSeek V4.1 Flash max,
unpinned (`owned_by: openrouter`), rclone: before the fix 1/3 (stopped on an empty reply after 35 turns, no change,
$0.037); with it 3/3, 83 turns, 1138 s, 4.3M input, $0.16. No cached tokens reported; input $0.01/M, output $1.20/M:
6-8x CheaperInference (~$0.02) and BeatAPI HC ($0.027) for this task. Not used.

## 0.0.54 (2026-10-15)

**`read` returns at most 250 lines unless the model gives a limit,** with a note on the rest ("lines 1-250 of 900; read
with offset=251 for more"). **On the own loop, a repeat of an unchanged read points back to the earlier result** instead of
sending it again (nothing leaves the conversation, unlike the elision tried in 0.0.38). **Background processes no longer
outlive a task:** what a command leaves running (`server &`) is recorded (pid and start time) and killed when the task
ends, or by the next engine if this one was killed; a background process holding the output pipe no longer blocks the
tool call (2 s, then a hint to redirect). The benchmark runner kills the whole process tree at its time limit. Found
because timed-out runs had left a Caddy server, two test backends and an engine running for a day and a half.

A/B on BeatAPI, old (0.0.53) vs new, same time, one run each:

| | old: tests | old: turns / input | new: tests | new: turns / input |
|---|---|---|---|---|
| GPT-6 Luna max, rclone | 3/3 | 127 / 10.1M | 3/3 | **100 / 6.3M (-37%)** |
| DeepSeek V4.1 Flash HC max, caddy-retry | 2/2 | 90 / 5.3M | 1/2 | 31 / 0.8M |
| DeepSeek HC, caddy-unix | 0/7 | 139 / 8.4M | **7/7** | 133 / 7.9M |
| DeepSeek HC, rclone | 3/3 | 195 / 19.0M | 3/3 | 240 / 25.0M |

Luna: a third less input at the same result (Codex used 4.7M on this task). DeepSeek: tests 11/12 vs 5/12, tokens about
equal; BeatAPI was slow and erratic that day (rclone took 195-240 turns, 58 the night before), so the DeepSeek pairs are
noisy. In the failed caddy-retry run every read had an explicit range: the window was not the cause. No process was
left behind after these runs.

## 0.0.53 (2026-10-15)

**GPT models use the Responses API on any provider** (by model id `gpt-`; BeatAPI lists every model as "openai").
**Benchmark kit: Codex CLI as a harness** (`codex@MODEL:EFFORT`, the user's own ChatGPT plan through the official
client, a clean `CODEX_HOME` with only the login, a minimal environment).

First "own loop vs the model maker's harness" data point, rclone (October set), one run each, one judge pool (Opus xhigh,
with reference):

| | tests | judge | time | turns | input | cache | cost |
|---|---|---|---|---|---|---|---|
| GPT-6 Luna max, **Codex CLI** | 3/3 | **8** | **453 s** | 106 actions | **4.7M** | 96% | plan |
| GPT-6 Luna max, tazuna own loop, CheaperInference (0.0.38) | 3/3 | 7 | 1451 s | 119 | 14.6M | 99% | $0.133 |
| GPT-6 Luna max, tazuna own loop, BeatAPI | 2/3 | 5 | 728 s | 72 | 4.4M | 47% | $0.111 |
| DeepSeek V4.1 Flash max, own loop, CheaperInference | 3/3 | **9** | 1575 s | 77 | 4.4M | 98% | $0.024 |
| DeepSeek V4.1 Flash **HC** max, own loop, BeatAPI | 3/3 | **9** | 641 s | 58 | 3.2M | 97% | $0.027 |
| DeepSeek V4.1 Flash max, own loop, BeatAPI (regular) | 1/3 | 1 (no change) | 965 s | 39 | 1.8M | 39% | $0.102 |

With Luna, Codex is a point better at a third of the time and input tokens: the own loop has a real gap with GPT
models (next: where the 3x tokens go). With DeepSeek the own loop is the best result in the pool. BeatAPI's high-cache
route (`deepseek-v4.1-flash-hc`, $0.105/$0.0021/$0.42 per M, doubled on weekdays 09-12 and 14-18 Beijing time) matches
CheaperInference at about the same cost and faster; its regular route caches poorly and its run changed nothing.

## 0.0.52 (2026-10-14)

**`TAZUNA_PRICE=input,cached,output`** (USD per million tokens) for providers whose model list has no prices: the own
loop's dollar cap needs one.

First use, apinex (DeepSeek V4.1 Flash max, own loop, rclone from the October set; the provider reports no cached
tokens and bills two identical requests the same):

| | result | time | turns | cost |
|---|---|---|---|---|
| CheaperInference (earlier runs) | 3/3 | 352-1575 s | 39-89 | $0.010-0.026 |
| Experiential Labs | 3/3 | 593-1119 s | 60-91 | $0.086-0.204 |
| apinex free tier | stopped: 10M free tokens used up | 2354 s | 84 | $0 |
| apinex paid (5 requests/minute) | stopped by hand, unfinished | 2120 s | 115 | $0.465 |
| apinex `claude-opus-5.5` medium (own loop) | 2/3 | 391 s | 25 | $0.270 |
| BeatAPI DeepSeek (cache hits 39%) | 1/3 | 965 s | 39 | $0.102 |

DeepSeek there: about $0.06 per million input tokens, but every re-sent token is billed in full: agent loops cost 20x and
more. Its Opus 5.5: a flat ~$0.50 per million tokens (about a tenth of Anthropic's list price, so of unclear origin), fast
and few turns, but 2/3 tests at 10-25x DeepSeek's cost on CheaperInference. BeatAPI passes caching through, but only 39%
of input came from cache (97-99% elsewhere) and the run solved 1 of 3 (CheaperInference: 3 of 3 in every run). None is
used; CheaperInference stays the cascade's provider.

## 0.0.51 (2026-10-14)

**`--review-model` uses the five-lens review prompt** (bench/review-lens.txt: acceptance criteria with a counterexample,
every other reader and writer of the touched data, partial failure and retries, lifecycle of new state, actors and
variants). Only Critical and Major findings go back, each with a failure example, and the implementer is told to fix
with the smallest change. **Fixed:** with boss mode the review started at the first result, while subagents were
still working in the background (3 of 5 runs below), so it saw half a change; it now waits until no background task
runs.

First measurement (5 of the user's company tickets, boss + Opus 5.5 high reviewer, 1 run; 3 of the 5 reviews hit the
bug above): the peer's round-1 defects that apply, 9 of 16 with the review as without it; cost $14.68 vs $8.90 (+65%).
The reviews raised real issues the peer only found in later rounds (a gate a product swap never closes, mismatched
`anchorStepKey`), which this check does not count. Stays opt-in; a clean rerun of the 3 affected tickets and a judge
pass decide.

## 0.0.50 (2026-10-13)

**rustls instead of the system's TLS, and a size-tuned release build (strip, LTO, one codegen unit).** Linking the macOS
Security framework for the own loop (0.0.28) cost every tazuna process ~4.4 MB of memory at start, used or not: the UI
went from 7.2 to 11.6 MB and the tool server from 2.1 to 6.4 MB (bisected over 71 commits; 0.0.41's "UI 7 MB" was Tier 0
at 0.0.20). rustls is plain Rust code that stays out of memory until a provider is called. It made the binary 1.3 MB
larger; strip and LTO took that back and more.

| Tier 0 | 0.0.49 | 0.0.50 |
|---|---|---|
| binary | 4.04 MB | 3.35 MB (−17%) |
| UI memory (animating) | 11.6 MB | 7.3 MB (−37%) |
| tool server memory | 6.3 MB | 2.1 MB (−67%) |
| startup | 9.1 ms | 7.2 ms |
| UI CPU | 2.0% | 1.2-1.7% |
| replay render | 10 ms | 8 ms |

Own loop, one small fix (`bench/agent_footprint.py`): fixed on CheaperInference (22.2 MB peak, engine 19.3 MB) and on
Experiential Labs (20.8 MB), against 27 MB at 0.0.41. Root certificates now come from the bundled Mozilla list
(webpki-roots), not the system keychain: a provider behind a private CA would need its certificate added there. A clean
release build takes ~45 s.

## 0.0.49 (2026-10-13)

**Boss mode is the default for `--run` on Opus on the claude path.** Opus only directs; Haiku subagents read, edit and
run the tests (`--boss haiku`, 0.0.32). `--no-boss` turns it off; `--boss MODEL` picks another worker model. Interactive
sessions, the tiny-repo Sonnet lane and the own loop are unchanged (not measured with boss). Fixed on the way: `--boss`
switched an untrusted repo from the lean profile to the repo profile, so the repo's hooks and settings ran without trust.
Boss now turns itself off in an untrusted repo (with a warning when it was asked for).

Company tickets (5 company tickets × 2 runs, Opus 5.5 medium; one judge pool, Opus xhigh with reference; 698 and 870 from
the 0.0.37 pool with 2 passes, 872, 921 and 946 judged again with 1 pass to include the 3 new boss runs):

| | judge | cost per ticket | turns | time | new type errors / failing unit tests |
|---|---|---|---|---|---|
| stock Claude Code (cc) | 6.15 | $2.67 | 49 | 12 min | 0 / 0 |
| tazuna default until now (repo profile) | 5.90 | $2.18 | 33 | 31 min | 0 / 0 |
| tazuna `--boss haiku` | 5.25 | $1.38 (−37%) | 14 | 23 min | 0 / 0 |

Judge −0.65 against the old default, within the ±0.75 noise threshold; cost −37%, past it. On the Go tasks boss was −0.13
at −32% (0.0.48 table). By the quality-per-dollar rule this makes boss the default. The drop is close to the threshold
and concentrated in the 3 new runs (4, 5 and 2 on 872, 921 and 946, each the lowest on its ticket; the judge named missed
edge cases, e.g. a removed guard that re-opens T-619). Edge cases: 2.90 vs 3.45. If later tickets confirm a real
drop, the next lever is a reviewer pass on the boss's diff, not dropping boss. The 3 new boss runs cost $0.94-0.99 each
and finished in 15-18 minutes.

Benchmark note: `tazuna` harnesses in `bench/` now run with boss unless they pass `--no-boss`.

Follow-up, same day: **boss + an Opus 5.5 high reviewer** (`--review-model claude-opus-5-5 --review-effort high`, 1 run
per ticket) to buy the edge cases back. Fresh judge pool over all 5 tickets (1 pass, with reference, 7 candidates each):
stock 5.90 at $2.67, tazuna without boss 5.90 at $2.18, boss 5.20 at $1.38, boss + reviewer 5.20 at $2.15. The reviewer
found problems in 4 of 5 patches (review $0.11-1.12, mean $0.63); edge cases rose (3.2 vs 2.8) but scope fell (2.8 vs
3.2): the fixes it asked for widened the patches. Same score at the old default's price: not adopted. The boss gap held
in the second pool (−0.70), so on tickets boss is consistently ~0.7 lower; it stays the default for `--run` by the
quality-per-dollar rule, and `--no-boss` is the choice for a ticket where quality matters more than cost.

## 0.0.48 (2026-10-13)

**`--explorer MODEL@URL` (opt-in): Opus hands code questions to a cheap model on tazuna's own loop.** `tazuna
explore-mcp` serves one MCP tool, `explore`, answered by MODEL with read-only tools of its own (`read`, `search` = grep,
`list` = find; no shell, paths inside the repo only, so no key files or other projects reach the provider). The code
leaves the machine, so the host must be trusted or `--data any`. The explorer's cost lands in the result
(`explorer_cost_usd`, and in the total). A security review caught the first version, which gave the explorer bash behind a
word list (`rg --pre`, `git -c core.pager`, `sed 'w file'` and `sort -o` got through) and an unrestricted `read`.

Measured on the 6 Go tasks (Opus 5.5 medium, DeepSeek V4.1 Flash max as explorer, 1 run; judge 1 pass with reference):

| | hidden tests | judge (with ref) | cost (6 tasks) |
|---|---|---|---|
| tazuna default (Haiku explorer, 2 runs) | same | 7.13 | $3.97 |
| Opus + DeepSeek explorer | same (terraform 1/3, hugo-template 7/8, rest full) | 7.33 | $4.31 (+9%) |
| `--boss haiku` (2 runs) | same | 7.00 | $2.70 |

No saving: Opus asked the explorer 2-3 questions per task and read and searched on its own the rest of the time (25 own
calls against 2 questions on hugo-template-return); each DeepSeek answer took ~4 minutes on CheaperInference, so runs got
up to 3x longer. Stays opt-in. Opus only stops paying for reading when it cannot read at all, which is what `--boss` does.

## 0.0.47 (2026-10-13)

**A model call that times out gets twice the time on the next try** (180 s, then 360, 720, at most 15 minutes). At the
official speed (~30 tok/s), MiMo V2.6 Pro's reasoning answers take over 3 minutes; every try was cut off at 180 s, and a run
made 26 calls in 2 hours.

**Gauge run for the model check's first flag** (MiMo V2.6 Pro for the cascade's cheap step: +0.08 over DeepSeek V4.1 Flash
on public benchmarks). caddy-unix (Opus fails it, DeepSeek solves it), own loop at max, one Opus judge call:

| | hidden tests | judge | cost | time | output tokens |
|---|---|---|---|---|---|
| DeepSeek V4.1 Flash, CheaperInference (0.0.38) | 7/7 | 9 | $0.02 | 44 min | 37k |
| MiMo V2.6 Pro, Experiential Labs (156 tok/s) | 7/7 | 9 | $0.67 | 59 min | 555k |

Same result at ~30x the cost: MiMo writes 15x the output. DeepSeek stays the cheap step. Experiential Labs serves MiMo at
5x the speed Artificial Analysis measures for the official API, with no quality loss on this task. The other four tasks
were stopped when the Experiential Labs balance ran low; the CheaperInference run (official speed) hit the timeout above.

## 0.0.46 (2026-10-12)

**`ctrl-x` stashes the message:** it goes aside and the composer clears; after the next message is sent it comes back by
itself, and `ctrl-x` on an empty composer brings it back at once (with both, they swap). **`/vim`: modal editing in the
composer**, remembered across sessions: esc for normal mode (`h l w b e 0 ^ $ x X D C dd cc dw cw j k`), `i a I A o` to type
again; enter still sends, ctrl keys work as always, and esc in normal mode with nothing typed stops the run (it never clears
the text). The composer shows `-- NORMAL --` / `-- INSERT --` while typing. Checked in a pseudo-terminal: stash and unstash,
`/vim` on and off, and "fix the parser bug" + esc `0 w dw` became "fix parser bug" in normal mode.

## 0.0.45 (2026-10-12)

**`/rewind`: undo a message and everything after it.** It lists the messages sent so far; enter rewinds files and
conversation, `f` files only, `c` the conversation only. Files: claude's own file checkpoints
(`CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING`, live sessions only) and its `rewind_files` request, with a dry run first that
names the files. Conversation: claude restarts at the last reply before that message (`--resume-session-at`, as a fork of
the session), and the message comes back into the composer to edit and send again. Each typed message carries an id
tazuna gives it, which is what both take. Checked in a pseudo-terminal against Haiku: after "create a.txt with one" and
"append two", `/rewind` + enter reported "restoring 1 file(s): a.txt (+1 −2)", a.txt held only "one" again, the second
message was back in the composer, and asked about its previous message claude named only the first. Limits: claude
checkpoints only its own edit tools, so in the lean profile there is nothing to restore (tazuna says so); not on the own
loop yet.

**Text from outside cannot drive the terminal.** Every transcript row now passes the same filter as permission questions
and the task view: escape sequences are dropped whole and other control or invisible characters show as `�`; the Cockpit's
file list and tool names too. Before, model replies, tool output and notices (a file name in the rewind notice, an error
text) reached the screen unfiltered, so a crafted file name or command output could have set the window title, written the
clipboard or cleared the screen (found by a security review of the rewind notice). Replay render time unchanged (9 ms).

## 0.0.44 (2026-10-12)

**`ctrl-b`: the background tasks of a session** (commands claude runs with `run_in_background`, Monitors, background
subagents): status, age and the latest output of the selected one, refreshed every second; `x` stops the selected task, the
run goes on. While one runs, the composer shows `[N background · ctrl-b]`. Built on claude's own stream: `task_started`,
`task_updated` and `task_notification` events, and the `get_task_output` and `stop_task` control requests (the same data as
Claude Code's `/tasks` view). Checked in a pseudo-terminal against Haiku: a 60-second loop showed as running with its
output ticking, and `x` stopped it. Only with Claude Code's own tools (repo profile); the lean profile's bash and the own
loop run every command to its end. Task output and descriptions pass through the same filter as permission questions: every
escape sequence is dropped whole (CSI, OSC up to BEL or ST, DCS/APC/PM/SOS) and any other control or invisible character
shows as `�`, so a command's output cannot set the window title, write the clipboard or move the cursor (a security
review caught the first version, which only removed colour codes).

## 0.0.43 (2026-10-12)

**`ctrl-g` opens the message in your editor** (`$VISUAL`, else `$EDITOR`, else `vi`; a command like `code --wait` works).
tazuna steps out of the screen while the editor runs; what you save comes back into the composer, and a failing editor
leaves the message as it was. Checked in a pseudo-terminal: the edited text is back in the composer and the screen redraws.
The temporary file is created exclusively with mode 0600 under an unguessable name (a security review flagged the first
version's predictable name: in a shared /tmp a planted symlink could have redirected the write).

## 0.0.42 (2026-10-12)

**The own loop asks before commands and edits.** With `--provider`, the ask mode now asks before every `bash`, `write` and
`edit` (reading stays free) and the edits mode before `bash`; shift-tab switches mid-run. The engine sends claude's own
`can_use_tool` request, so tazuna's question UI, "always" answers and an interrupt while a question is open work as on
claude. Checked against DeepSeek over the engine's protocol: it read the file, asked before its edit, the answer "no" left the
file unchanged, and the model explained the fix instead. Plan mode stays claude-only (it needs claude's plan approval);
shift-tab skips it with `--provider`. `--run` now says that `--mode` does not apply to headless runs instead of ignoring it.

## 0.0.41 (2026-10-12)

**The cascade is the own loop's default.** With `--provider`, a task the own loop cannot finish (tests still failing, an
error, no change, the budget spent) goes to Opus 5.5 medium on claude without `--escalate`; `--no-escalate` keeps the own
loop's result. The `cascade` preset no longer needs the flag. **Model calls time out after 180 s** (was 300): of 15,505
recorded DeepSeek and Luna calls, the median took 15 s, 99% under 83 s and 0.1% over 180 s, while a provider that accepts a
call and never answers stalled a run for an hour across the retries.

## 0.0.40 (2026-10-12)

**Own loop on Experiential Labs** (`--provider https://api.experientiallabs.ai/v1`): its model list gives prices in nano-USD
and names the upstream provider in `data_policy`; both are read now, so GPT-6 Luna there goes through the Responses API.
**Retries span ~15 minutes** (a free model "at capacity" ended all seven runs of a benchmark at 4 minutes), but a 429 that
waiting cannot fix (`insufficient_quota`, a model locked until a purchase) ends the run at once.

**Cheap models next to DeepSeek V4.1 Flash** (own loop, max effort, October tasks; no judge yet):

| | provider | runs | cache hits | speed | cost vs DeepSeek | result |
|---|---|---|---|---|---|---|
| Qwen3.8 Flash | CheaperInference | rclone, gitea (stopped at ~70 min, balance) | 82-95% | 25-40 tok/s | 2-3x | none finished; DeepSeek: 12-26 min |
| GLM-5.3 Flash | CheaperInference | same | 16-58% | 8-28 tok/s | 4-10x | none finished |
| GLM-5.3 Flash | Experiential Labs | rclone (call hung 20+ min), gitea (stopped at $0.20, 84 min) | 79-96% | 30-50 tok/s | 5-6x | none finished |
| Qwen3.8 Flash Next (free) | Experiential Labs | 7 tasks | - | - | free | "at capacity" for free accounts; unusable |
| Qwen3.8 Flash | Experiential Labs | - | - | - | - | locked until credits are bought |
| DeepSeek V4.1 Flash | Experiential Labs | rclone | 95% | 76 tok/s | 8-14x (list price 4x, more output) | 3/3, judge 9.5 / 7.5 |

None beats DeepSeek on CheaperInference on price or speed; DeepSeek stays the cascade's first step.

**"Few turns" prompt for the own loop, measured and kept off** (`TAZUNA_BATCH=1`: bundle independent tool calls, read whole
files, run targeted tests until the end; DeepSeek made 1.14 tool calls per response):

| task | turns | total tokens | cost | hidden tests |
|---|---|---|---|---|
| rclone | 39 vs 77 | 1.6M vs 4.5M | $0.010 vs $0.024 | 3/3 both |
| caddy-unix | 71 vs 60 | 3.7M vs 3.3M | $0.036 vs $0.020 | 7/7 both |
| telegraf | 100 vs 129 | 5.7M vs 9.9M | $0.051 vs $0.047 | 2/2 both; package suite failed with the prompt |
| gitea | stopped at 59 vs 99 | 5.0M vs 8.3M | $0.112 (stopped) vs $0.035 | - |

Turns -21% and tokens -38% on the three finished tasks, but cost rose: with fewer turns DeepSeek writes 2-4x more output,
and its output costs 200x a cache read. Worth retrying only with a model whose input is expensive relative to its output.

## 0.0.39 (2026-10-12)

**Own loop: no time limit by default, $1 per task.** A cheap model's extra hour costs cents, so the own loop now runs until
it is done or has spent $1 on the task (was 60 minutes and $0.50: on lazygit and prometheus, DeepSeek reached $0.28-0.49
after about 2 hours, still working). `--max-minutes N` brings a time limit back for whoever wants it fast (with
`--escalate`, the hand-over comes after N minutes); `TAZUNA_MAX_USD` still overrides the dollar cap. The turn backstop rises
to 2000 (200 had cut DeepSeek off after ~75 minutes). The provider's model list at startup is retried like model calls:
one run died when that request hung.

**Benchmark: cascade on the 7 October tasks** (DeepSeek V4.1 Flash max first, Opus 5.5 medium on failure; 2 runs; Opus
5.5 xhigh judge with / without reference):

| | fully solved | judge without ref | with ref | cost (14 runs) | hours (sum / longest run) | Opus hand-overs |
|---|---|---|---|---|---|---|
| tazuna on Opus 5.5 medium (0.0.34) | 9/14 | 6.79 | 6.93 | $38.90 | 16.8 / 8.6 | - |
| cascade 0.0.34 (60 min, reset by each follow-up) | 12/14 | 7.47 | 7.97 | $5.04 | 12.3 / 2.1 | 6 |
| cascade 0.0.38 (no time limit, $0.50) | 13/14 | 7.79 | 8.32 | $8.36 | 14.1 / 2.7 | 5 |
| cascade 0.0.39 ($1), lazygit and prometheus only | 3/4 | 5.50 / 8.00 | 6.25 / 8.25 | $0.85 (4 runs) | 4.1 / 2.0 | 0 |
| GPT-6 Luna max, own loop, no fallback (1 run) | 5/7 | 6.79 | 6.14 | $0.85 (7 runs) | 3.7 / 1.4 | - |

Without the time limit, quality and solved runs move within noise (+0.3 judge, +1 solved) and the sum of hours by +15%;
cost rose 65%, nearly all from one prometheus hand-over ($3.79). The $1 cap is untested so far: none of its four runs got
near $0.50. DeepSeek solved lazygit in 10-17 minutes in both runs here and needed 2.7 hours in both runs at 0.0.38, so a
single task's time swings far more than any setting does. One prometheus run claimed success with the visible tests green
and failed the hidden ones, a miss the cascade cannot catch: it hands over only on visible failures. Luna is quick (5 tasks
under 30 minutes) but judged ~1 point below the cascade, which has Opus behind it.

## 0.0.38 (2026-10-11)

**Own loop: the time and dollar budget count per task.** tazuna's follow-ups (test failures, review) used to start a fresh
hour and a fresh $0.50 each; one DeepSeek run passed the benchmark's 90-minute limit that way, and a cascade waited that
much longer before handing over to Opus. **Model calls retry for about 4 minutes** (was 40 s): a short network outage
ended a 38-minute run.

**Two own-loop ideas from arXiv 2609.20804, measured and dropped** (DeepSeek V4.1 Flash max, own loop, 7 October tasks):
- *Eliding old tool outputs* (stubs once outputs pass ~30k tokens). Prompts shrank to ~60k tokens per call from 100-118k,
  but calls were not faster (23-29 s vs 15-27 s: the provider's wait dominates, not prompt size), and with 99% cache hits
  it saves almost nothing. Eliding every old output made the model re-read one file 16 times; eliding only stale reads
  and old bash output fixed that, still without a speed gain.
- *A `plan` tool* (checklist the model writes and updates, plus one system-prompt line; without that line it was never
  called). 1 run each:

| task | own loop | own loop + plan |
|---|---|---|
| caddy-retry-match-expr | over 90 min (budget bug above) | 2/2 |
| caddy-unix-socket-close | 7/7 | stopped at 60 min |
| lazygit-mnemonic-prefix | stopped at 60 min | 3/3 |
| gitea-actions-cancelling | 2/2 | 2/2 |
| rclone-archive-symlinks | 3/3 | 3/3 |
| telegraf-snmp-reconnect | 2/2 | 1/2 |
| prometheus-exemplar-compaction | stopped at 60 min | stopped at 60 min |

4 of 7 fully solved either way, cost $0.02-0.09 per run either way: no effect, so the code is gone. Most useful side
result: DeepSeek alone runs out of its hour on 2-3 of these 7 tasks per run, which is the cascade's hand-over to Opus doing
its job.

## 0.0.37 (2026-10-11)

**`--advisor MODEL` (opt-in until benchmarked):** sets Claude Code's `advisorModel`, which gives the main model an
`advisor` tool to consult MODEL when it is stuck (checked: with Sonnet 5.5 as main model and `opus` as advisor the tool
appears). Idea from a thread suggesting Opus as advisor with cheaper models doing the work.

**Benchmark: popular plugins and the advisor** (6 Go tasks, 1 run each, Opus 5.5 xhigh judge, each run alone):

| | judge without ref | with ref | cost (6 tasks) |
|---|---|---|---|
| stock Claude Code (Opus 5.5 medium, 2 runs, earlier) | 7.58 / 7.58 | 6.83 / 7.08 | $5.46 avg |
| + ECC | 7.67 | 7.08 | $5.56 |
| + superpowers | 7.50 | 6.83 | $5.18 |
| + caveman | 7.75 | 7.17 | $6.35 |
| + graphify alone | 7.58 | 6.92 | $5.24 |
| + GStack (skills; browser binary not built) | 7.08 | 6.75 | $5.32 |
| + all five together | 7.42 | 7.17 | $5.36 |
| tazuna default (Opus 5.5 medium + Haiku explorer, earlier) | 7.42 / 7.50 | 7.17 / 7.08 | $3.97 avg |
| tazuna, Sonnet 5.5 medium + Haiku explorer | 6.33 | 6.17 | **$1.35** |
| **tazuna, Sonnet 5.5 + `--advisor claude-opus-5-5`** | **7.33** | 6.75 | **$2.17** |

No plugin, alone or all together, moves quality past noise or lowers cost (−5% to +16% vs stock). The advisor lifts Sonnet
by +1.0 judge to about Opus's level (−0.1 vs tazuna's Opus default) at −45% cost: about `--boss haiku`'s quality and cost
(7.33, $2.70). One run each; a second run decides.

**Second run** (Go tasks again; company tickets 5 × 2 runs, judged in one pool with stock and tazuna on Opus):

| | Go judge without ref | with ref | Go cost (6 tasks) | tickets judge | ticket cost per run |
|---|---|---|---|---|---|
| stock Claude Code, Opus 5.5 medium | 7.58 | 6.96 | $5.46 | 6.15 | $2.67 |
| tazuna default, Opus 5.5 medium | 7.46 | 7.13 | $3.97 | 5.70 | $2.18 |
| tazuna `--boss haiku` | 7.33 | | $2.70 | 5.79 (7 runs) | $1.56 |
| tazuna Sonnet 5.5 + Opus advisor (2 runs) | 7.33 / 6.83 | 6.75 / 6.33 | $2.17 / $2.29 | 5.10 | $0.61 |

On Go the advisor holds up (7.08 mean without reference, −0.4 vs the Opus default, within noise) at −44% cost. On the
company tickets it falls 0.6 below the Opus default and 1.05 below stock, at −72% cost; no new type errors or unit failures,
PR-test counts identical in both runs. It stays opt-in: cheap, but the tickets show a drop larger than noise relative to
stock. `--boss haiku` keeps Opus-level quality on both sets.

## 0.0.36 (2026-10-11)

**`--preset NAME`: a model and effort per role, bundled.** Built in: `default` (Opus 5.5 medium + Haiku explorer), `boss`
(`--boss haiku`), `max` (Opus 5.5 at max effort, opt-in: terraform 3/3 at about 5× the cost of medium), `cascade`
(DeepSeek V4.1 Flash max on CheaperInference with `--data any`, Opus 5.5 medium on failure). `~/.config/tazuna/presets`
adds or overrides presets (`name: --flag value ...`); flags on the command line win over the preset's.

## 0.0.35 (2026-10-11)

**`--data trusted|any` (default `trusted`).** With `trusted`, `--provider` only accepts hosts listed in
`~/.config/tazuna/trusted-providers` (one per line); Anthropic through the claude CLI is always allowed. Providers that may
store or use the code (for example DeepSeek routes) need `--data any` or an entry in the list. A strict zero-retention mode
was considered and left out: none of CheaperInference's 97 models offers a ZDR route today, and the Claude subscription
follows the account's own retention terms.

## 0.0.34 (2026-10-10)

**Own loop: one hour per run.** A run still going after 60 minutes ends with an error `result`; with `--escalate` the
task then moves to the stronger model on claude. DeepSeek on the prometheus hard tasks ran into the benchmark's 3-hour
limit at 15-24 s of provider wait per call; the cascade could not take over.

Second cascade run (0.0.34): the same 8 tasks all pass every hidden test again, none breaks a passing test, $0.11 in total,
no escalation. Judge (Opus 5.5 xhigh, each run alone): Go 10 / 10 / 10 / 9 / 10 / 9, **mean 9.67** (run 1: 9.42); hard
caddy-tls 10, hugo-eq 9. Prometheus hard tasks: the hour ran out (replay-mmap) or the tests failed (wbl-checkpoint), and
both moved to Opus as designed.

**The judge anchors on the reference fix.** Same runs judged again without the merged PR's code (`JUDGE_NOREF=1`, task,
diff and test facts only). Go task means, with → without reference:

| | with | without |
|---|---|---|
| stock Claude Code r0 / r1 | 6.83 / 7.08 | 7.58 / 7.58 |
| tazuna default r0 / r1 | 7.17 / 7.08 | 7.42 / 7.50 |
| `--boss haiku` r0 / r1 | 7.08 / 6.92 | 7.33 / 7.33 |
| DeepSeek, own loop, no effort r0 / r1 | 8.33 / 8.20 | 7.67 / 7.70 |
| **cascade (DeepSeek max) r1 / r2** | **9.42 / 9.67** | **8.17 / 8.17** |

DeepSeek's solutions resemble the merged fixes (reference: up, no reference: −0.5 to −1.5); the Opus setups' solutions
differ in design (no reference: +0.3 to +0.8). The cascade's lead on the Go tasks shrinks from ~2.4 to ~0.7. Hard tasks
(caddy-tls, hugo-eq; merged after DeepSeek's release), no reference: cascade 9.0 / 7.5, tazuna Opus 7.25 / 7.0. Also: Opus
5.5 medium on the own loop (one agent, four tools, like DeepSeek) got terraform 1/3, like every Opus setup on claude: the
loop is not what makes the difference.
Effort is: tazuna default with Opus 5.5 at **max** solved terraform 3/3 (judge 9 with and without reference; 79 turns,
50 min, $4.71), where Opus at medium gets 1/3 in every harness (judge 4-5). DeepSeek at max: 3/3 for $0.01. Opus max stays
an opt-in profile (about 5× the cost of medium on this task).

**Clean benchmark: 7 tasks merged 1-10 October 2026** (after DeepSeek's release; caddy ×2, gitea, lazygit, prometheus,
rclone, telegraf), 2 runs each, judge Opus 5.5 xhigh with and without the reference fix:

| | judge without ref r0 / r1 | judge with ref r0 / r1 | cost r0 / r1 (7 tasks) |
|---|---|---|---|
| **cascade** (DeepSeek V4.1 Flash max, Opus 5.5 medium on failure) | **6.86 / 8.07** | 7.29 / 8.64 | **$2.25 / $2.79** |
| tazuna default (Opus 5.5 medium + Haiku explorer) | 7.21 / 6.36 | 7.21 / 6.64 | $20.31 / $18.59 |

On average the cascade scores +0.7 (no reference) at about 1/8 of the cost; without the prometheus task (Opus there:
$10-12.7 per run, 5-8.6 h, part of it while the Mac slept) the default costs $7.6 / $8.6 and the cascade $0.82 / $0.91.
The cascade escalated on rclone, telegraf, gitea (once) and prometheus (twice). Opus failed caddy-unix-socket-close in both
runs (0/7 hidden tests, a passing test broken; judge 4) where DeepSeek solved it alone (7/7, judge 9-10). Prometheus split
both ways: cascade 2.5 / 8, default 8 / 3. Telegraf breaks a passing test in every run of both setups.

Benchmark fixes found on the way: the diff was taken against HEAD, so an agent that commits left an empty diff (Opus on
lazygit, two planner runs, one tester run were judged as empty); it is now taken against the base commit. The usage gate
skipped jobs on a stale meter reading after a guard restart; a stale reading now waits.

## 0.0.33 (2026-10-10)

**Cascade: `--escalate MODEL[:EFFORT]` with `--provider`.** The cheap model works on tazuna's own loop first; when its
tests still fail, or it stops with an error or changes nothing, the task moves to MODEL on the claude CLI in a fresh session
with the task, a note that a cheaper model's changes are in the working tree, and the test failures. Smoke: Gemma 3 4B
changed nothing, Haiku 5.5 took over, fixed and verified.

Benchmark (DeepSeek V4.1 Flash max first, Opus 5.5 medium on failure; 1 run): all 8 tasks pass every hidden test, none
breaks a passing test, $0.17 in total; no task needed the escalation (DeepSeek's own tests passed each time). Go: k6 5/5,
prometheus 1/1, terraform **3/3** (every Opus run 1/3), caddy 5/5, hugo-section 2/2, hugo-template-return 8/8. Hard
(merged after DeepSeek's release): caddy-tls 1/1 (every Opus setup 0/1), hugo-eq 4/4. Prometheus hard tasks left out
(DeepSeek runs out of time there; the cascade has no time budget yet).
Judge (Opus 5.5 xhigh, each run alone): Go 10 / 10 / 9 / 10 / 9 / 8.5, **mean 9.42** (Opus default 7.12, stock 6.96,
DeepSeek without effort 8.33 / 8.20); hard caddy-tls 10, hugo-eq 9.

Also (0.0.32 follow-up, T-698 and T-870, all setups judged in one pool): `--boss haiku` 6.5 / 6.0 vs the default
6.75 / 5.75 and stock 7.0 / 7.0, at $2.26 / $1.75 vs the default's $2.26 / $2.29 (two runs each; boss mode's cost varies
run to run, the earlier −45% was partly one lucky run). `--boss sonnet`: 7 / 6 at $3.54 / $1.98, no gain: dropped.

## 0.0.32 (2026-10-10)

**`--boss MODEL` (opt-in until benchmarked): the main model only directs.** Explorer, implementer and tester subagents on
MODEL do all reading, editing and test runs; a PreToolUse hook (`tazuna boss-hook`) denies the main model every tool but
delegation (subagent calls carry an `agent_id`, the main model's do not; `--tools`/`--disallowedTools` would block the
subagents too). Implies the repo profile. Why: in the default, Opus still did ~27% of the reading, all test runs and every
edit itself (T-872: Opus 19 reads, 11 runs; Haiku explorer 52 reads). Smoke (Python fixture, Opus medium + Haiku):
Opus 2 delegations, Haiku 3 Bash, 1 Read, 2 Edit; verified, $0.13.

**Benchmark (2026-10-09/10, Opus 5.5 medium everywhere, judge Opus 5.5 xhigh, 2 passes).**

Go tasks, 2 runs each, every run judged alone:

| | judge r0 / r1 | cost r0 / r1 (6 tasks) |
|---|---|---|
| stock Claude Code | 6.83 / 7.08 | $5.50 / $5.42 |
| tazuna default (Haiku explorer) | 7.17 / 7.08 | $4.26 / $3.68 |
| **tazuna `--boss haiku`** | **7.08 / 6.92** | **$2.36 / $3.04** |
| Opus plan → Haiku executes → Opus checks | 5.92 | $6.53 |
| DeepSeek V4.1 Flash, own loop (CheaperInference) | 8.33 / 8.20 | $0.53 / $0.40 |

Company tickets, one judge pool per ticket (all setups side by side):

| | judge | cost per run | runs |
|---|---|---|---|
| stock Claude Code | 5.75 | $2.67 | 10 |
| tazuna default | 5.50 | $2.18 | 10 |
| tazuna without the explorer | 6.00 | $5.34 | 4 (1 hit 90 min) |
| **tazuna `--boss haiku`** | **5.20** | **$1.19** | 5 (5 more hit 90 min under load) |

Boss mode: same quality on the Go tasks at −32% vs the default and −51% vs stock; on the company tickets −45% vs the
default and −55% vs stock for −0.30 / −0.55 judge (one run per ticket; within noise but lower on 3 of 5). Opus needs 4-17
turns instead of 12-40. The five first-batch ticket runs hit the 90-minute limit while 11 runs and their test suites shared
the machine; the second batch, alone, took 19-26 min. Stays opt-in until a second ticket run.

Also measured: without the explorer the tickets score +0.5 at 2.4× the cost (the explorer stays); Opus plan → cheap
executor costs more than Opus alone (planning at high effort alone cost $0.47-1.21) and a wrong plan steers the executor
wrong (caddy-tls: DeepSeek alone 1/1, DeepSeek following Opus's plan 0/1); hard tasks (merged after DeepSeek's release):
DeepSeek solved caddy-tls (judge 10, $0.017) where every Opus setup failed, but timed out on the prometheus tasks
(CheaperInference adds 15-24 s per call; the agents rerun a 10-minute suite).


## 0.0.31 (2026-10-09)

**Own loop: a model call that hangs is retried.** Two of 18 benchmark runs hung on a call that never answered (the engine
waited 600 s, then gave up; the benchmark's 90-minute limit ended the run). Calls now time out after 300 s, and timeouts and
dropped connections get the same three retries as rate limits and server errors (2, 8, 30 s apart).

## 0.0.30 (2026-10-09)

**Six Go tasks, every run judged alone (Opus 5.5 xhigh, 2 passes).** Opus 5.5 medium is tazuna's default on claude; the
others run the own loop on CheaperInference. A run that hit the benchmark's 90-minute limit counts 0.

| Task | Opus r0 | Opus r1 | DeepSeek V4.1 Flash r0 | DeepSeek r1 | DeepSeek max | GPT-6 Luna max (Responses) |
|---|---|---|---|---|---|---|
| caddy-quoted-braces | 9 | 8.5 | 10 | 9 | 7 | 9 |
| hugo-section-slug | 9 | 8 | 9 | 9 | timeout | 7 |
| hugo-template-return | 4.5 | 5 | 9 | 9 | 9.5 | 3 |
| k6-run-once | 7 | 7 | 7.5 | timeout | 10 | 7 |
| prometheus-duration-parens | 9 | 9 | 9.5 | 9 | 9.5 | 3.5 |
| terraform-import-foreach | 4.5 | 5 | 5 | 5 | 4 | 5 |
| **Judge (finished runs)** | **7.17** | **7.08** | **8.33** | **8.20** | **8.00** | **5.75** |
| Hidden tests | 87% | 87% | 94% | 93% | 87% | 66% |
| Cost (6 tasks) | $4.26 | $3.68 | $0.53 | $0.40 (5) | $0.39 (5) | $0.37 |

Opus is consistent across its two runs. DeepSeek scores at or above Opus on every finished task at ~1/10 the cost; most of
the gap is hugo-template-return, which no harness in this pool has fully solved before (cc, OMP, Hermes, pi: at most 88%
of hidden tests) and DeepSeek solved in all three runs. Possible causes beyond the model: 4-8× the turns, the own loop,
training-data overlap on public repos (unchecked). Max effort shows no clear gain over the default. Luna stays behind and
breaks passing tests on two tasks. Two of 18 DeepSeek runs hung on a model call that never answered (the engine waits
600 s and does not retry a timeout): fix before the next benchmark.

**OpenAI models go through the Responses API.** Through Chat Completions with tools, GPT-6 Luna does not reason at all
(0 reasoning tokens at `xhigh` and `max`, with or without `max_tokens`); through Responses it does (54-98 on the same
prompt) and keeps `max` (Chat Completions on CheaperInference turns it into `xhigh`). Stateless (`store: false`): reasoning
comes back encrypted and goes out again with the history. Chosen by the model's owner in `/v1/models`. Every 0.0.28 and
0.0.29 Luna result is void. Smoke: GPT-6 Luna max fixed the Python fixture in 7 turns, $0.0005.

## 0.0.29 (2026-10-09)

**The own loop sends `--effort` as `reasoning_effort`.** 0.0.28 sent none, so every own-loop run used the provider's
default. On a hard prompt DeepSeek V4.1 Flash reasoned 152 tokens without it, 130 at `low` and 2,439 at `max`: the 0.0.28
DeepSeek results are near its lowest effort. GPT-6 Luna barely changes (109 / 53 / 93).

## 0.0.28 (2026-10-09)

**Own agent loop, first slice (`--provider URL [--key-env VAR]`).** tazuna runs its own loop on an OpenAI-compatible API
instead of `claude`: `tazuna engine` speaks claude's stream-json, so the run log, verification and the reviewer work
unchanged. Chat Completions without streaming, the four lean tools in-process, prices from the provider's `/v1/models`,
a $0.50 cap per run (and 200 turns as a backstop). The key is removed from the environment before any tool runs. Design and scope:
docs/OWN_LOOP.md.

| Metric | 0.0.27 | 0.0.28 |
|---|---|---|
| Binary | 2.77 MB | 3.80 MB (ureq with the system's TLS; rustls would be 5.17 MB) |

Smoke (DeepSeek V4.1 Flash on CheaperInference, two-bug Python fixture): fixed and verified in 6 turns, $0.0007, 75% of
input tokens from cache.
Go task smoke (hugo-section-slug, 1 run): hidden tests 2/2, $0.043, 57 turns, 28 min. tazuna on Opus 5.5 medium (0.0.15):
2/2, $0.52-0.60, 16-22 turns. Not judged yet. CheaperInference adds 15-24 s before the first byte of every call (a 7-token
prompt with a 2-token answer took 18 s), so its times say nothing about the model.

Six Go tasks × 1 run, each run judged alone (Opus 5.5 xhigh, 2 passes, same day):

| Task | Opus 5.5 medium (claude, 0.0.28 default): hidden, cost, turns, judge | DeepSeek V4.1 Flash (own loop): hidden, cost, turns, judge |
|---|---|---|
| caddy-quoted-braces | 5/5, $0.43, 14, 9.0 | 5/5, $0.10, 108, 10.0 |
| hugo-section-slug | 2/2, $0.53, 13, 9.0 | 2/2, $0.04, 57, 9.0 |
| hugo-template-return | 7/8, $1.36, 40, 4.5 | 8/8, $0.08, 60, 9.0 |
| k6-run-once | 5/5, $0.79, 23, 7.0 | 5/5, $0.12, 116, 7.5 |
| prometheus-duration-parens | 1/1, $0.71, 19, 9.0 | 1/1, $0.08, 105, 9.5 |
| terraform-import-foreach | 1/3, $0.44, 28, 4.5 | 2/3, $0.10, 124, 5.0 |
| **Total** | 87% hidden, **$4.26**, judge **7.17**, 64 min | 94% hidden, **$0.53** (−88%), judge **8.33**, 287 min |

One run per side, so within noise per task, but DeepSeek is equal or higher on all six. It takes 4-5× the turns; the time
is mostly CheaperInference's 15-24 s wait per call. The comparison mixes model and loop (claude CLI with the Haiku explorer
vs the own loop with four tools).

Also: the own loop refuses the ask, edits and plan modes for now (it never asks before a tool runs; found by a security
review); cost is the provider's billed `usage.cost` when reported.

## 0.0.27 (2026-10-09)

**`--review-model` reviews once per task, against the real task.** A run that claude starts on its own after a background
command (the Haiku explorer, so nearly every run) counted as a new run: the reviewer ran again for each wake-up, with the
placeholder "(claude continued on its own…)" as its task instead of the user's message. The reviewer now gets the user's
last message and runs once per message. First Fable 5.1 xhigh reviewer gauge (0.0.26, with this bug) is void.

Fable 5.1 xhigh reviewer gauge (T-872, 1 run, same judge pool as the default's 2 runs): one review per run confirmed in
the run log. Judge 5.00 vs 5.00, cost $3.56 vs $1.04 (3.4×; the review alone $2.15, 209 s), PR tests 58/86 both. The review
made Opus revert its backend change, which the judge called the root cause. Full run skipped; `--review-model` stays opt-in.

## 0.0.26 (2026-10-08)

**Headless runs no longer end while claude is about to wake up.** One run ended after 8 turns with no change ("Still
waiting on the explorer…"): its last background explorer finished during a turn, the 5-second settle counted from that
moment and had passed when the turn's result came, but claude started another turn 70 ms later. The settle now restarts at
each result while a finished background task is pending.

**`--tester <model>`** (opt-in): a second subagent runs tests, type checks, builds and linters and reports only the
failures, so their output stays out of the main model's context. All 5 company tickets, 2 runs each, all 10 at once, Haiku
explorer and Haiku tester against 0.0.24's Haiku explorer alone, one judge pool per ticket:

| Ticket | Explorer: cost, judge | Explorer + tester: cost, judge |
|---|---|---|
| T-921 | $1.24, 6.75 | $1.09, 6.25 |
| T-870 | $1.80, 6.50 | $1.61, 6.75 |
| T-946 | $1.87, 4.00 | $1.41, 3.00 |
| T-698 | $2.09, 7.50 | $1.86, 6.75 |
| T-872 | $1.04, 5.00 | $0.97, 5.00 |
| **Mean per ticket** | **$1.61, 5.95** | **$1.39 (−14%), 5.55 (−0.40)** |

The run that ended early (T-946, judge 1.0) is left out. Cheaper on every ticket, but the saving sits at the noise line
and the judge drop is larger than the explorer's (lower on 3 of 5 tickets): inconclusive (judge up on T-870, equal on T-872).
Stays opt-in until a retest with a second judge model. Usage: 10 runs + judge ≈ 19% of the 5-hour window.

## 0.0.25 (2026-10-08)

**Haiku explorer on by default** (repo profile; `--no-explorer` turns it off, `--explorer <model>` picks another model). The
user's measure is price to performance, not quality alone: 0.0.24's −28% cost for a judge −0.25 (within noise) is a better
ratio, and a strong reviewer can buy quality back where it matters. The default's numbers are now 0.0.24's Haiku column
(mean per ticket $1.61, 26 turns, judge 5.70).

## 0.0.24 (2026-10-08)

**Headless runs answer permission questions themselves.** A benchmark run hung for 25 minutes: Opus called
`AskUserQuestion`, and with nobody to answer, tazuna waited forever. With `--run`, every `can_use_tool` request is now
denied at once with "No user is present in this run … decide with your best judgment and continue." Live mode is unchanged.

**Haiku 5.5 explorer** (`--explorer haiku`, opt-in): all 5 company tickets, Opus 5.5 medium, 5-minute cache, one judge pool
per ticket. Default runs: 0.0.21 on T-921/870, 0.0.23 on the other three (the 0.0.12 runs there predate the 5-minute
cache). 2 runs per arm and ticket; one Haiku run on T-946 hung on the bug above and is excluded.

| Ticket | Default: cost, turns, judge | Haiku explorer: cost, turns, judge |
|---|---|---|
| T-921 | $1.38, 29, 5.75 | $1.24, 21, 6.50 |
| T-870 | $2.20, 42, 6.75 | $1.80, 33, 6.25 |
| T-946 | $3.54, 78, 4.25 | $1.87, 30, 3.50 |
| T-698 | $2.75, 47, 7.25 | $2.09, 31, 7.50 |
| T-872 | $1.33, 28, 5.75 | $1.04, 17, 4.75 |
| **Mean per ticket** | **$2.24, 45, 5.95** | **$1.61 (−28%), 26 (−41%), 5.70** |

Cost down well past noise on every ticket; judge −0.25, within noise, though lower on 3 of 5 tickets. No run added type
errors or unit failures, and each pair passes the same PR tests (one test fewer in one T-872 Haiku run). Haiku answers
cost a fraction of Opus reading the same files, unlike the Sonnet explorer (+12%). Made the default in 0.0.25.
Usage for this batch: 12 runs + judge ≈ 28% of the 5-hour window.

## 0.0.23 (2026-10-08)

**`--explorer` rule made firm.** With the first wording Opus called the explorer once in 4 runs and kept exploring with
`cat`/`sed`/`grep`. Now: no exploring yourself, every code question goes to the explorer, read a file only right before
editing it. Opus then delegated (its own shell commands dropped to 0-31 per segment; the explorers run in the background,
so each return shows as a separate run in the run log).

T-921 + T-870, 2 runs each, Opus 5.5 medium, Sonnet 5.5 explorer, 5-minute cache, one pool per ticket:

| Arm | 921 | 870 | Judge avg | Cost (4 runs) | Turns |
|---|---|---|---|---|---|
| default (0.0.21) | 6.0, 6.0 | 7.0, 7.0 | 6.50 | $7.15 | 24-50 |
| explorer, first wording (0.0.22) | 5.5, 6.5 | 7.0, 6.0 | 6.25 | $7.66 (+7%) | 29-45 |
| **explorer, firm rule (0.0.23)** | 6.0, 7.0 | 6.0, 6.0 | **6.25** | **$8.03 (+12%)** | **19-31** |

Fewer turns on the main model, but more money: the explorer reads the same code on Sonnet (only ~40% cheaper per token
than Opus) and its answers come back into Opus's context. Quality within noise. Stays opt-in; a Haiku 5.5 explorer is the
untested cheaper variant.

## 0.0.22 (2026-10-08)

**`--explorer <model>`** (opt-in until benchmarked, repo profile): a custom subagent (`claude --agents`) on a cheaper model
reads and searches the code and answers in short (paths, line ranges, the lines that matter); one appended rule tells the
main model to ask it and to read a whole file itself only right before editing. Reading is most of a run's cost and the
repo map did not reduce it; this moves the reading off the expensive model and out of its context.

## 0.0.21 (2026-10-08)

**`--repo-map`** (opt-in until benchmarked): the session's first message starts with a map of the repo, the folders with
file counts and, for the source files whose path or definitions match the task's words, the names they define (top-level
definitions by plain line matching; tests ranked below the code they test; capped at 12,000 chars). `tazuna map <task-file>`
prints it. On the company tickets it lists the files the real fixes changed (T-921: 1st and 7th of 25), in 0.7 s.

A/B on T-921 + T-870 (2 runs each, Opus 5.5 medium, 5-minute cache in both arms, judged with the afternoon's runs):

| Arm | 921 | 870 | Judge avg | Cost (4 runs) | Turns | Code read (median) | Searches (median) |
|---|---|---|---|---|---|---|---|
| default | 6.0, 6.0 | 6.5, 7.0 | 6.38 | $7.15 | 24-50 | 150k chars | 23 |
| `--repo-map` | 6.5, 6.0 | 7.0, 6.0 | 6.38 | $7.38 (+3%) | 23-59 | 153k chars | 19 |

No gain: the agent searched a little less but read as much, because it reads the files it changes in full; knowing where
to look does not shrink what it reads. Stays opt-in (`tazuna map` is still handy for a look at a task's files).

## 0.0.20 (2026-10-08)

**Prompt cache lifetime by mode.** claude writes its prompt cache with a 1-hour lifetime, billed at 2× input; a 5-minute
cache is billed at 1.25×. Cache writes are ~30% of a run's cost, and within a run turns come seconds apart, so headless
runs (`--run`) now use 5 minutes; live sessions keep 1 hour, where a pause over 5 minutes would re-write the whole context.
`--cache-ttl 5m|1h` overrides; a `CLAUDE_CODE_PROMPT_CACHE_TTL` the user set always wins.

A/B on T-921 + T-870 (2 runs each, Opus 5.5 medium, repo profile, same hour, side pool judged together):

| Arm | 921 | 870 | Judge avg | Cost (4 runs) | Turns |
|---|---|---|---|---|---|
| default (1-hour cache) | 6.5, 7.0 | 6.5, 6.5 | 6.62 | $8.16 | 30-46 |
| **5-minute cache** | 7.0, 7.0 | 6.5, 6.5 | **6.75** | **$7.35 (−10%)** | 32-44 |
| Bash output capped at 8,000 chars (`BASH_MAX_OUTPUT_LENGTH`) | 8.0, 6.5 | 6.0, 7.0 | 6.88 | $10.59 (+30%) | 40-61 |
| stock Claude Code (reference, earlier) | 6.0, 7.0 | 6.5, 6.0 | 6.38 | | |

Every 5-minute run was cheaper than every default run, as predicted from the cost breakdown (−11%). Capping Bash output
backfired: the agent ran more commands to see what was cut (54-61 turns on T-870 vs 42-46); ruled out. Benchmarks of
tazuna from this version on run with the 5-minute cache.

## 0.0.19 (2026-10-08)

Daily-driver features, part 2 (headless runs unchanged).

- **Slash commands** reach claude, which runs them in stream mode: built-ins (`/compact`, `/cost`, `/clear`, `/model`,
  `/context`, `/init`) and the repo's own commands and skills. tazuna shows output that has no model turn (`/cost`),
  marks `/clear` and `/compact` in the transcript, and completes commands with `tab` from the list claude reports.
- **`@` file completion** from the repo's tracked files (`tab` cycles).
- **Claude's to-do list** (TodoWrite) shows as a checklist (☐ ◐ ☑).
- **Images:** a message the user types that names an image file (`@shot.png`, a dragged-in path) sends it as an image;
  `ctrl-v` saves the clipboard's image to a private temp folder and names it in the message. Only typed messages attach:
  after a security review, text tazuna sends on its own (test failures, reviewer findings, `!cmd` output, `--run`
  prompts) stays text, because claude's or a command's output could name any file and bypass the permission mode.
- **`!cmd`** runs a command in the repo, shows the output and hands it to claude.

## 0.0.18 (2026-10-08)

Daily-driver features (no benchmark change: headless runs keep the "auto" mode and behave as before).

- **Permission modes** (`--mode`, `shift-tab` cycles live): **auto** (never asks, the default), **ask** (before edits and
  commands), **edits** (edits go ahead, commands ask), **plan** (read-only; the plan appears in the transcript and
  approving it switches to edits). claude routes its questions to tazuna (`--permission-prompt-tool stdio`); the
  composer shows "allow Write: b.txt  (always = this tool)   y yes · a always · n no". A stop answers open questions with no.
  Hardened after a security review of the first version: the question owns the keyboard and ignores keys for 600 ms after
  it appears, so typing cannot approve by accident, and Enter no longer means yes; the whole action is shown (every line
  of a command, control characters made visible, cuts marked) so a first line cannot hide the rest; "always" covers one
  exact command for shells and the tool otherwise, and the question says which; questions from a process that died are
  dropped instead of being answered later. A second review pass: the question on screen is now drawn from the same queue
  the answer applies to (a stop or a dead process can no longer leave a stale question showing), bidirectional overrides
  and zero-width characters are made visible too (in the plan text as well), and ctrl-c/ctrl-d still stop or quit while a
  question is open (the stop answers it with no).
- **Input editing:** cursor movement, `ctrl-a/e/u/k/w`, `alt-backspace`, and message history on `↑↓`, kept across
  sessions. `esc` stops a run when nothing is typed. Expand moved from `ctrl-e` to `ctrl-o` (Claude Code's detail key).
- **Resume:** `tazuna -c` continues this repo's latest session, `--resume <id>` a given one, with the earlier transcript
  on screen.
- **Subagents:** their messages stay under the call that started them ("12 steps · Read src/a.ts") instead of mixing into
  the main transcript.

## 0.0.17 (2026-10-08)

**repo-lean reads before it relies on a name.** Recorded runs showed why repo-lean scored lowest: with Claude Code's own
tools and the short prompt it read ~25% less code than Claude Code's own prompt (150k vs ~197k chars, median over 10
company-ticket runs) and ran the type check less (2 vs 3); on T-921 it read 102k vs 118-167k and the judge flagged a
guessed database column. Two lines now go into repo-lean's prompt (~60 tokens): read the code you change, its callers and
the definition of every name you rely on; run the type check and the relevant tests before finishing.

T-921 + T-870, 2 runs each, side pool of 12 per ticket judged together, Opus 5.5 medium:

| Arm | 921 | 870 | Judge avg | Cost (2 tickets, medians) |
|---|---|---|---|---|
| tazuna repo profile + diagnostics (0.0.12, the default since 0.0.13) | 7.0, 7.0 | 6.5, 8.0 | **7.12** | $4.53 |
| tazuna repo profile, no diagnostics (0.0.12) | 6.5, 7.0 | 7.5, 7.0 | 7.00 | $5.42 |
| stock Claude Code | 6.0, 7.0 | 7.5, 6.0 | 6.62 | $4.63 |
| **tazuna repo-lean 0.0.17** | 6.5, 5.5 | 7.0, 7.0 | **6.50** | $4.62 |
| tazuna lean | 5.5, 6.0 | 6.5, 7.0 | 6.25 | $3.82 |
| tazuna repo-lean 0.0.11 | 6.0, 6.0 | 6.0, 5.5 | 5.88 | $3.55 |

The two lines lifted repo-lean by 0.62 to stock level, by reading more, and its cost rose to stock's. It no longer has an
edge: the default (repo profile with diagnostics) scored higher at about the same cost. repo-lean stays opt-in.

## 0.0.16 (2026-10-08)

**Background commands.** When claude starts a command in the background and ends its turn, it wakes up again once the
command finishes. tazuna now tracks claude's running commands: the session is not idle (and `--run` does not exit) until
they are done and claude has had 5 s to start the turn it wakes up for. That turn is logged as a run of its own (prompt
"(claude continued on its own …)") with its cost, and `--run` output sums every run of the session (`tazuna.runs`).
Before, `--run` exited at the first result, cutting claude off where `claude -p` would have continued; none of the 25
tazuna runs in the cross-harness table used a background command, so those numbers stand.

**UI.** Multi-line input (the composer grows to 6 rows; `ctrl-j` everywhere, shift-enter where the terminal reports it);
`ctrl-s` hands the mouse to the terminal to select text; cache misses next to the Cockpit's cache gauge; Claude Code's own
Edit/Write show as a diff like the lean tools (they showed raw JSON); the session panel shows the real profile.

**Tier 0** now also measures startup to first frame (6.4 ms), UI CPU and memory while animating (1.6%, 7.1 MB) and the
tool server's memory (2.1 MB); stock Claude Code's UI: 190 MB, 2.2% idle, 0.45 s to first output.

## 0.0.15 (2026-10-08)

**`--review-model M [--review-effort E]`: an independent reviewer** (opt-in until benchmarked). When claude first says it is
done and the run changed files, a separate `claude -p` on model M reads the task and the diff (new files in full) in a fresh
context, with read-only tools, and answers `ok` or a list of concrete problems. Problems go back to the implementer once;
then verification runs. The reviewer's cost is added to the run (`total_cost_usd` in `--run` output is now tazuna's own
total for the run, every process included). Meant for a cheap implementer with a strong reviewer, e.g. Haiku 5.5 implements
and Opus 5.5 reviews. Smoke test (Haiku 5.5 low + Opus 5.5 low, a two-function fix): review in 2-5 s for $0.006-0.027.

Gauge on one company ticket and one Go task (2 runs each, all arms the same hour, judged in side pools):

| Arm | T-921 judge | T-921 cost | hugo-section-slug judge | hugo cost | Turns |
|---|---|---|---|---|---|
| Opus 5.5 medium (default) | 5.75-6.75 (earlier same-day runs) | $1.4-2.6 | **8.00** | $0.56 | 16-45 |
| Haiku 5.5 high + Opus 5.5 medium reviewer | 5.00 | $1.06 | 6.75 | $0.71 | 60-71 |
| Haiku 5.5 high alone | 4.50 | $0.45 | 7.00 | $0.26 | 44-67 |

The reviewer flagged a problem in 1 of 4 runs and passed implementations the judge called "diverges most from the spec";
each review cost $0.25-0.48 (Opus reading code), more than a Haiku run. Haiku 5.5 needs 2-3× the turns of Opus and
~25 min on the company ticket. Verdict: Haiku + Opus review is not competitive with Opus medium alone (lower score on both
tasks; on hugo also more expensive). `--review-model` stays opt-in. Usage: 10 runs and both judges, 37% → 46% of the 5-hour
window.

## 0.0.14 (2026-10-08)

**`--self-review`** (opt-in until benchmarked): three sentences of finish rules in the system prompt (finish only when every
requirement is met and checked; do not stop to ask; read a name before relying on it), and one review turn when claude first
says it is done and the run changed files: read the whole diff, map every acceptance criterion to code, revert what the task
did not ask for, check unread names, re-run tests. The review comes before verification, so its fixes are tested too.

A/B on T-921 and T-870 (the two company tickets whose cause the ticket names), repo profile with diagnostics, Opus 5.5
medium, 2 runs per arm per ticket, both arms the same morning, each ticket's whole pool re-judged:

| Arm | 921 | 870 | Judge avg | Median cost (sum of 2 tickets) | Turns (median) |
|---|---|---|---|---|---|
| tazuna 0.0.14 `--self-review` | 7.5, 7.0 | 7.0, 6.5 | **7.00** | $5.36 (**+31%**) | 45 |
| tazuna 0.0.14 without | 6.0, 6.0 | 7.5, 7.0 | 6.62 | $4.10 | 38 |

+0.38 judge (within the 0.75 noise threshold) for +31% cost. On T-921 the review lifted correctness (4.5 vs 3.5-4: the
judge called both reviewed runs "close to the reference"); on T-870 it scored 0.5 lower, one run extending into frontend
code the ticket scoped out. Earlier arms in the same re-judged pools moved by up to 0.5. Decision: stays opt-in. The 8 runs
and judging used 25 points of the 5-hour window.

## 0.0.13 (2026-10-08)

**Diagnostics after each edit are on by default** (`--no-diagnostics` turns them off), after this A/B on the five company
tickets (acme/app, Opus 5.5 medium, repo profile, 2 runs per arm per ticket, both arms the same evening, every
ticket's runs judged as one pool):

| Arm | 872 | 921 | 946 | 870 | 698 | Judge avg | Total cost (median per ticket) | New type errors |
|---|---|---|---|---|---|---|---|---|
| **tazuna 0.0.12 `--diagnostics`** | 5.00 | 6.25 | 3.50 | 7.00 | 7.00 | **5.75** | **$12.66 (−11%)** | 0 / 10 |
| tazuna 0.0.12 without | 5.00 | 6.25 | 3.50 | 7.00 | 7.00 | 5.75 | $14.18 | 0 / 10 |
| stock Claude Code (previous afternoon) | 5.00 | 7.00 | 4.50 | 6.50 | 7.00 | 6.00 | $13.19 | 0 / 10 |

Identical judge scores on every ticket; 11% cheaper and ~12% fewer turns, below the 15% cost threshold, so suggestive rather
than proven, at no quality cost. No run broke the type check, so the gain is errors caught right after the edit instead of
by the agent's own later type-check runs. The 20 runs and judging took the 5-hour window from 45% to 100%.

Tier 0: binary 2.52 MB (+84 KB); input tokens per turn unchanged (2,983; the hook adds nothing to the prompt).

## 0.0.12 (2026-10-07)

**`--diagnostics`: compiler errors right after an edit** (opt-in until benchmarked). tazuna adds a `PostToolUse` hook to
claude (`--settings`, so it runs beside the repo's own hooks, in every profile). After any tool call that changed files,
including shell commands (agents make most edits with `python3`/`sed` through Bash), it runs the repo's `typecheck` script
or `go vet` on the changed Go packages and tells claude about errors the change added, once, while the edit is fresh.
Errors the repo already had when the session started are left out (same comparison as verification).

## 0.0.11 (2026-10-07)

**`--profile repo-lean`.** The lean prompt with Claude Code's own tools (`Bash`, `Read`, `Edit`, `Write`, `Grep`, `Glob`,
`Skill`) and the repo's settings, hooks, skills, plugins and `CLAUDE.md`: the repo's rules at close to lean cost. It uses
Claude's tools, not the lean MCP tools, because repo hooks match Claude's tool names (`Write|Edit`) and would never fire on
`mcp__lean__edit`; the bash output cap does not apply. `AGENTS.md`, which Claude Code does not read, goes into the prompt
unless `CLAUDE.md` imports it. Not the default yet: it needs the real-work benchmark first.

**Fix: the repo profile loaded the user's claude.ai connectors.** They come with the account, not a settings file, so
`--setting-sources project,local` did not keep them out: in a personal setup with Atlassian, Slack and Claude Docs
connected, every turn carried 76 extra tool schemas (~42k tokens), and the first tool call broke the prompt cache when they
finished connecting. Both repo profiles now pass `--strict-mcp-config` and load only the repo's `.mcp.json`
(`--user-settings` keeps everything). Benchmarks ran on an empty config and were not affected.

Input tokens per turn, probe repo with CLAUDE.md, one skill, one hook and a `.mcp.json` server (Haiku 4.5, claude 2.1.292):

| Profile | Tokens per turn | Hook fired | Skill used | Repo MCP server |
|---|---|---|---|---|
| repo, 0.0.10, personal setup | ~51,900 | yes | yes | — |
| repo, 0.0.11 | 21,763 | yes | yes | yes |
| **repo-lean, 0.0.11** | **9,724** | yes | yes | yes |
| lean | 2,977 | no | no | no |

Tier 0: binary 2.43 MB (+3 KB), lean 2,977 input tokens per turn.

**Real work, repo-lean vs a same-day stock baseline** (five acme/app tickets, 2 runs per arm per ticket, Opus
5.5 medium, all runs of a ticket judged as one pool; repo-lean and stock ran 15:00-16:10 the same day):

| Arm | 872 | 921 | 946 | 870 | 698 | Judge avg | Total cost (median per ticket) |
|---|---|---|---|---|---|---|---|
| stock Claude Code, same day | 5.50 | 6.75 | 5.00 | 6.75 | 7.75 | **6.35** | $13.20 |
| stock Claude Code, earlier | 5.00 | 6.25 | 4.50 | 6.75 | 7.50 | 6.00 | $13.38 |
| tazuna lean (0.0.9) | 4.25 | 6.25 | 5.00 | 7.00 | 7.50 | 6.00 | $10.88 |
| tazuna repo (0.0.9) | 5.00 | 6.25 | 4.50 | 6.00 | 7.00 | 5.75 | $12.66 |
| cc-mid via `claude -p` | 4.75 | 6.25 | 3.50 | 7.25 | 6.00 | 5.55 | $10.98 |
| **tazuna repo-lean (0.0.11)** | 5.50 | 5.00 | 4.50 | 6.25 | 6.00 | **5.45** | **$11.62 (−12% vs same day)** |

repo-lean is 12% cheaper than same-day stock (cheaper on 4 of 5 tickets) but scores 0.90 lower, beyond the 0.75 noise
threshold, and lowest of the tazuna arms. The judge's notes point at unchecked assumptions (a guessed database column in
T-921, swallowed lookup errors in T-946), not at repo rules. Lean, with the same prompt, scored 6.00, so the drop comes
from the combination of Claude's tools, its CLAUDE.md loading and the repo's hooks with the short prompt; not isolated yet.
Decision: repo-lean stays opt-in; the repo profile stays the default. Stock Claude Code itself moved 0.35 judge and up to
±20% cost per ticket between two batches on the same and the previous day. The 16 runs and judging used 26 points of the
5-hour window and 5 points of the weekly limit.

## 0.0.10 (2026-10-07)

**Verification checks types.** When the repo has a `typecheck` (or `type-check`) script, verification runs it beside the
tests and fails only on type errors the run added: the check runs once when the run starts and again at the end, and errors
are compared without line and column, so known errors that moved are not new. Test runners like `tsx` strip types, so
before this a run that broke the type-checked build still passed as "verified". Monorepos without a root test script now
run the tests of the packages whose files changed (`npm --prefix <pkg> test`) instead of skipping verification.

**Fixes.** Parallel runs garbled `runs.jsonl` (each line was written in pieces); every line is now one write, also in
`labels.jsonl`. Sessions started in the same second in same-named directories wrote one recording; the file name now ends in
the process id.

Real work, five acme/app tickets (T-872, 921, 946, 870, 698), 2 runs per arm per ticket, Opus 5.5 medium,
each ticket judged as one pool. Arms ran 0.0.9:

| Arm | Judge avg | Total cost (median per ticket) | New type errors |
|---|---|---|---|
| tazuna lean | 5.65 | $10.88 (−19%) | 0 / 10 runs |
| stock Claude Code | 5.55 | $13.38 | 0 / 10 |
| cc-mid via `claude -p` | 5.50 | $10.98 (−18%) | 7 in 1 / 10 |
| tazuna repo profile | 5.35 | $12.66 (−5%) | 0 / 10 |

Quality is tied (all within 0.3). The lean profiles are consistently ~18% cheaper; cc-mid was cheaper than stock on 5 of 5
tickets. On two tickets (T-872, T-946) no arm found the real root cause.

0.0.10 vs 0.0.9 on T-698 (all 12 runs re-judged together):

| Arm | Judge | Median cost | Verification time | New type errors |
|---|---|---|---|---|
| tazuna lean 0.0.9 | 6.75 | $2.79 | 97-101 s | 0 |
| **tazuna lean 0.0.10** | **7.00** | **$3.10** | **65-73 s** | **0** |
| tazuna repo 0.0.9 | 5.75 | $3.17 | 96-103 s | 0 |
| **tazuna repo 0.0.10** | **6.50** | **$3.08** | **72-75 s** | **0** |

All differences are within noise; the type check adds no time (it runs beside the tests; the lower times came from 4
parallel runs instead of 8). No 0.0.10 run broke the build, so the check had nothing to catch here. Replayed on cc-mid's
broken T-698 diff, it reports exactly the 7 new errors against the repo's 361 known ones. Re-judging moved the 0.0.9
arms by up to 0.75, the noise threshold.

Tier 0: binary 2.43 MB (+20 KB), 2,983 input tokens per turn (unchanged), claude 2.1.292.

## 0.0.9 (2026-10-07)

**Repo config is respected by default.** In repos with agent config (`CLAUDE.md`, `AGENTS.md`, `.claude/`, `.mcp.json`),
tazuna runs the **repo profile**: Claude Code's own prompt and tools with the repo's settings, hooks and skills, and
Claude Code's own loading of `CLAUDE.md` (nested files and `@imports` included). The user's personal `~/.claude` setup stays
out unless `--user-settings`. Repos without config, tiny repos and `--profile lean` run the lean profile (cc-mid, ~30%
cheaper; the bash output cap only exists there).

**Trust.** When a repo's config can run code (any settings file, skills, commands, subagents, MCP servers), the first live
start asks "Trust this repo? [y/N]" and remembers the answer (`~/.local/share/tazuna/trusted.json`); headless runs need
`--trust`. Without trust the run is lean: nothing from the repo runs, and its `CLAUDE.md` still reaches the prompt. The check
fails closed: any settings file counts, not only ones with hooks (`statusLine`, `apiKeyHelper`, `env` and plugins can run
code too).

Real-work test on T-872 / PR #1068 (same task as 0.0.8), all 15 runs judged together, Opus 5.5 medium:

| Arm | Runs | Judge | Mean cost | New type errors | Verification |
|---|---|---|---|---|---|
| Stock Claude Code, repo hooks off | 3 | 5.67 | $1.91 | 0 | — |
| Stock Claude Code, repo hooks on | 2 | 5.50 | $2.02 | 0 | — |
| **tazuna 0.0.9 (repo profile)** | 3 | **5.17** (5.5, 5.0, 5.0) | **$1.79** | **0** | 3/3 verified |
| cc-mid via `claude -p` | 2 | 5.00 | $1.46 | 0 | — |
| tazuna 0.0.7 (lean) | 2 | 5.00 | $1.31 | 1 | 2/2 verified |
| tazuna 0.0.8 (lean + CLAUDE.md) | 3 | 4.83 | $1.37 | 1 | 3/3 verified |

The repo profile is within noise of stock Claude Code (−0.33 to −0.5) at 11% lower cost, with no new type errors and the
most consistent scores. Against lean it is +0.3 (within noise) at ~30% higher cost. Turning the repo's hooks off did not
lower stock Claude Code's score, so its earlier lead came from its prompt and tools, not the hooks. Across all 15 runs the
spread between arms (4.8-5.7) is small next to the spread within arms; only 2 runs found the backend root cause. One task:
an anecdote, not a benchmark.

Tier 0 (lean probe in a repo without config): 2,983 input tokens per turn (unchanged), binary
2.41 MB.

## 0.0.8 (2026-10-06)

**Project instructions.** The repo's own `CLAUDE.md`, `.claude/CLAUDE.md` and `AGENTS.md` are added to tazuna's system prompt
(duplicates skipped, cut at 80k chars); nested instruction files are listed so the agent reads them before working in that
directory. The text is the same every turn, so it is cached. Hooks and skills from the repo are still not loaded.
`--no-project-instructions` turns it off; the run log records which files were included.

First test on real company work: acme/app, T-872 / PR #1068 (TypeScript monorepo, 3k files; the agent gets
the Jira ticket plus the one log line, the PR's non-test diff is the judge's reference). All arms Opus 5.5 medium, judged
together:

| Arm | Runs | Judge | Mean cost | Mean time | New failing unit tests (of 17,812) | New type errors |
|---|---|---|---|---|---|---|
| Stock Claude Code (repo CLAUDE.md, hooks, skills) | 2 | 6.00 | $2.02 | 271 s | 0 | 0 |
| cc-mid via `claude -p` | 2 | 5.25 | $1.46 | 298 s | 0 | 0 |
| **tazuna 0.0.8 (with CLAUDE.md)** | 3 | 4.83 | $1.37 | 261 s | 0 | 0, 1, 0 |
| tazuna 0.0.7 | 2 | 4.50 | $1.31 | 376 s | 0 | 0, 1 |

Project instructions: +0.33 judge (within noise), +5% cost (CLAUDE.md is ~10k tokens, re-read from cache at 95-97% hit).
No arm found the backend root cause the PR fixed; all built a frontend workaround (correctness 3/5 for every run). Stock
Claude Code led in both judge passes; the repo's hooks (a comment linter after every Write/Edit) and Claude Code's full
prompt and tools are the likely reasons and are not yet separated. One task and 2-3 runs per arm is an anecdote, not a
benchmark.

Security: instruction files are only read when they are regular files whose real path is inside the repo (a symlinked
`CLAUDE.md` or `.claude` directory could otherwise put any file on the machine into the prompt). Repo instructions are trusted input; the README says to use `--no-project-instructions`
in repos you did not write.

Tier 0: 2,980 input tokens per turn in a repo without instruction files (unchanged), binary
2.35 MB.

## 0.0.7 (2026-10-06)

**Labels for runs**, the training data for routing by task type later. Kept in `~/.local/share/tazuna/labels.jsonl`
(append-only, next to the run log):
- `/good` or `/bad` (with an optional note) in the live UI rates the last finished run; it is never sent to claude.
- `tazuna --verdict good|bad [--note "..."] [--id RUN]` does the same from the shell (default: the latest run).
- `tazuna --label-git` labels older runs from git: `kept` when a run's output was later committed unchanged, `discarded`
  when a day has passed and it is in no commit and gone from the working tree. Runs in the current repo are labelled this
  way automatically when tazuna starts. A user verdict always wins over a git label.
- Run log lines now have an `id` and the content hash of every file the run changed (what the git check looks for).
  `tazuna --log` shows a verdict column.

No change to what claude sees or does. Binary 2.35 MB (0.0.6: 2.31 MB); other Tier 0 metrics
unchanged (2,980 input tokens per turn, 7 ms replay render).

## 0.0.6 (2026-10-06)

**Rules router v1.** Without `--model`/`--effort`, tazuna picks a lane per session from the number of tracked files:
tiny repos (≤ 50 files) start on Sonnet 5.5 medium, everything else on Opus 5.5 medium (unchanged default). If
verification fails on the cheap lane, tazuna switches the session to Opus 5.5 high (resuming it; one cache rebuild) and
runs the fix rounds there. The lane and reason show at the top of the transcript and go into the run log, with whether the
run escalated. `--model`/`--effort` turn routing off.

Why only repo size: in the cc-lean sweeps Sonnet medium matched Opus medium on easy and medium tasks in tiny repos at about a
third of the cost, but no cheap text signal separated hard tasks, and on real tickets in large repos Opus medium was the best
value. Routing by task type needs labelled outcomes from the run log (the Jev step).

Also: Python verification finds `test_*.py` files, uses pytest only when it is installed (else `uv run pytest`, else
`unittest`), and treats "no tests ran" (exit 5) as nothing to verify instead of a failure.

Tier 1 (4 tiny-repo tasks, 3 easy and 1 medium, × 3 runs; router vs always Opus 5.5 medium, judged together):

| Metric | Always Opus medium (= 0.0.5) | Router | Change |
|---|---|---|---|
| All hidden tests pass | 12/12 | 12/12 | same |
| Judge | 8.62 | 8.08 | −0.54, within noise (lower on 2 of 4 tasks) |
| Mean cost | $0.086 | $0.029 | **−67%** |
| Median time | 19 s | 11 s | −42% |
| Escalations | — | 0 of 12 | verification passed or found no tests |

Large repos route to Opus medium as before, so the real-work numbers are unchanged. Escalation is exercised by a live test
(Sonnet medium → failed verification → Opus high → verified), not yet by a benchmark task.

Tier 0:

| Metric | 0.0.5 | 0.0.6 | Change |
|---|---|---|---|
| Input tokens per turn before any work | 2,977 | 2,980 | within noise |
| Binary size | 2.27 MB | 2.31 MB | |
| Replay render | 7 ms | 7 ms | |

## 0.0.5 (2026-10-06)

**Bash output cap.** Bash output over 8,000 characters (~2k tokens) is shrunk before claude sees it: passing-test chatter
(`=== RUN`, `--- PASS`, `ok`) goes first, then the start (~2.5k chars) and the end (~5.5k chars) are kept with a note saying
how many lines were omitted and where the full output was saved (a private temp directory outside the repo; claude can read
any part of it). `--bash-cap N` sets the limit, `0` turns it off. `read` and short output are unchanged.

Tier 1 (6 real-work Go tasks × 2 runs, **Sonnet 5.5 high** to stay inside the 5h usage window, same binary with and without
`--bash-cap 0`, interleaved, judged together):

| Metric | Cap off (= 0.0.4) | Cap on | Change |
|---|---|---|---|
| Judge | 6.79 | 6.58 | within noise |
| All hidden tests pass | 8/12 | 8/12 | same |
| Hidden tests passed | 84% | 84% | same |
| Mean cost | $0.45 | $0.40 | −12%, within noise |
| Mean input tokens | 833k | 676k | −19% (mostly fewer turns: 17 vs 20) |
| Tool output chars per run | 61.5k | 58.8k | −4% |
| Outputs shrunk | — | 7 in 12 runs | |

Long bash output is rare on these tasks, so the direct saving is small (−4% of tool output). The cap costs no quality and
guards against blowups that would otherwise be re-sent every turn (live check: `seq 1 20000`, ~27k tokens, became ~2k, and
claude read the one line it needed from the saved file). Kept on by default.

Also: saved outputs go to a private directory (0700, unpredictable name) as new 0600 files, so on a shared /tmp nobody else
can read them or plant a symlink.

Tier 0:

| Metric | 0.0.4 | 0.0.5 | Change |
|---|---|---|---|
| Input tokens per turn before any work | 2,983 | 2,977 | within noise |
| Binary size | 2.27 MB | 2.27 MB | |
| Replay render | 8 ms | 7 ms | within noise |

## 0.0.4 (2026-10-06)

**Lean tool server in Rust.** The bash/read/write/edit tools are now `tazuna mcp`, a mode of the same binary, instead of a
bundled Node script. tazuna no longer needs Node. Tool names, descriptions and schemas are byte-identical, and replies match
the Node server's byte for byte on a captured request set (now a unit test). One behavior fix: a command that hits the
120 s timeout is killed with everything it started (process group); the Node server killed only `bash`, which left
children running (seen with graphify's 30-minute orphans in the addon benchmark).

| Metric | 0.0.3 | 0.0.4 | Change |
|---|---|---|---|
| Input tokens per turn before any work | 2,980 | 2,983 | within noise (same tools) |
| Tool server memory | 53 MB (Node) | 1.8 MB | −97% |
| Binary size | 2.20 MB | 2.27 MB | +3.2% |
| Replay render | 6 ms | 8 ms | within noise |
| Runtime dependencies | claude, node | claude | Node dropped |

Tier 1 not run: what claude sees (tool definitions and replies) is identical, so a Tier 1 run would measure only noise.
Remaining differences are error wording for failed file operations and the timeout fix.

## 0.0.3 (2026-10-06)

**Verified, not claimed.** When claude says a run is done, tazuna runs the tests for what the run changed (Go: `go test`
on the changed packages; Rust, Python and JS when set up; `--verify "<cmd>"` to override, `--no-verify` to disable).
Failures go back to claude as the next message, up to 2 extra rounds in the same process. The run log records
`verified`, `verify_failed`, `claimed` (nothing to test), `skipped` or `off`.

Tier 1 (6 real-work Go tasks × 2 runs, Opus 5.5 medium, same binary with and without `--no-verify`, interleaved, judged
together):

| Metric | Verification off (= 0.0.2) | Verification on | Change |
|---|---|---|---|
| Judge | 6.71 | 7.38 | +0.67, within noise (see below) |
| All hidden tests pass | 8/12 | 8/12 | same |
| Hidden tests passed | 87% | 90% | within noise |
| Mean cost | $0.67 | $0.71 | +5%, within noise |
| Median turns | 22 | 23.5 | within noise |
| Time added by verification | — | median 27 s (10-112 s) | zero tokens |

Verification passed on the first try in all 12 runs: claude already tests its own work on these tasks, so it never had to
send a failure back. Both arms saw identical inputs, which makes the +0.67 judge difference pure noise and is why the
judge threshold is now ±0.75. Verification stays on by default: it costs no tokens when tests pass, it is a safety net for
runs that skip or break tests, and `verified` vs `claimed` is useful data for routing. Possible follow-up: skip it when
claude's own last test run already covered the same packages after its last edit.

Also:
- Auto-detected test commands only include plain path characters, so a file name can't inject shell syntax.
- The run log and Tier 0 now record the `claude` CLI version.

Tier 0:

| Metric | 0.0.2 | 0.0.3 | Change |
|---|---|---|---|
| Input tokens per turn before any work | 2,873 (claude 2.1.289) | 2,980 (claude 2.1.291) | +104 from Claude Code's update, not tazuna |
| Binary size | 2.13 MB | 2.20 MB | +3.2% |
| Replay render | 5 ms | 6 ms | within noise |

## 0.0.2 (2026-10-06)

**Run log.** Every run (one message until claude's result) appends a line to `~/.local/share/tazuna/runs.jsonl`: why it
stopped (`completed`, `stopped`, `max_turns`, `budget`, `error`, `crashed`), cost, time, turns, per-turn tokens, cache hit
rate and the turns where the prompt cache missed, tool calls and failures, the files this run changed, and an empty
`verdict` to label later. Live and headless runs are both logged. `tazuna --log [N]` prints the last runs as a table.

Tier 0 only (no change to what claude sees or does):

| Metric | 0.0.1 | 0.0.2 | Change |
|---|---|---|---|
| Input tokens per turn before any work | 2,876 | 2,873 | within noise |
| Binary size | 2.02 MB | 2.13 MB | +5.4% |
| Replay render (median of 7) | 6 ms | 5 ms | within noise |

Measurement fix: replay render time is now the median of 7 warm runs. The first launch of a new binary includes a ~200 ms
macOS scan, which made the 0.0.1 figure (231 ms) wrong; re-measured at 6 ms.

## 0.0.1 (2026-10-06)

First version: live mode driving `claude` over stream-json with the cc-mid profile, replay, four UI designs, headless mode.

Baseline (no previous version to compare against):

| Metric | Value |
|---|---|
| Real work, Sonnet 5.5 high (12 runs) | judge 6.29 · all hidden tests 8/12 · median $0.31 · 240 s · 30 turns |
| Real work, Opus 5.5 medium (12 runs) | judge 7.25 · all hidden tests 8/12 · median $0.70 · 254 s · 29 turns · cache 94% |
| Parity vs `claude -p`, same profile | judge +0.55 (noise) · mean cost +3% (noise, same day) · per-turn overhead +58 tokens |
| Input tokens per turn before any work | 2,876 |
| Binary / replay render | 2.0 MB / 6 ms |

Fixes found while measuring:
- Exact cost was wrong after a resumed process (claude reports cost per process); now summed.
- `ctrl-c` used to kill and resume, which left a "No response requested." turn in history; now uses the stream-json interrupt.
- The auth token was visible to commands the agent runs; the tool server now removes it from its environment.
