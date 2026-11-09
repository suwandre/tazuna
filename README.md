# tazuna

手綱 (*tazuna*, "reins"). A lean terminal coding agent for any model. Claude runs through the official `claude` CLI on
your own subscription; DeepSeek, GPT, Kimi, GLM, Mistral and others run on tazuna's own agent loop. Every change is
benchmarked before it stays.

[Install](#install) · [Results](#results) · [What it does](#what-it-does) · [Other models](#models-beyond-claude) ·
[Changelog](CHANGELOG.md) · [Roadmap](ROADMAP.md)

## Install

macOS and Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/suwandre/tazuna/main/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/suwandre/tazuna/main/install.ps1 | iex
```

The binary goes to `~/.local/bin` (on Windows also onto your PATH; `TAZUNA_INSTALL_DIR` picks another folder), and
`tazuna update` replaces it with the newest release later. Releases have builds for macOS (Apple silicon, Intel), Linux
(x86_64, ARM) and Windows (x86_64).

You also need git, and for Claude the [`claude` CLI](https://docs.claude.com/en/docs/claude-code), logged in. On Windows,
Git for Windows too: like Claude Code there, tazuna runs commands in Git Bash. From source: `cargo install --path .`.

## Start

```sh
cd your-repo
tazuna                                          # Claude; the router picks model and effort
tazuna --model claude-sonnet-5-5 --effort high
tazuna -c                                       # continue this repo's last session
tazuna --run task.txt                           # one task, headless, result as JSON
```

Type a task and press enter. `/model` switches to another model mid-session (Claude or the own loop) and asks its effort
after, `/effort` changes only the effort (the conversation goes on), `/login` adds a provider's key, `/mcp` brings your
tools, `/keys` lists every key and command.

## Results

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/img/harnesses-dark.svg">
  <img alt="Judge score against median cost per task for eight harnesses on Claude Sonnet 5.5: tazuna 6.67 at $0.41, OMP 6.54 at $0.51, Claude Code 6.38 at $0.43, Hermes 6.21 at $0.55, pi 6.08 at $0.28, tazuna lean 6.04 at $0.31, OMO 5.38 at $0.49, Empryo 5.12 at $0.50" src="docs/img/harnesses-light.svg">
</picture>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/img/same-model-dark.svg">
  <img alt="Minutes per task with the same model: DeepSeek V4.1 Flash, Mistral Vibe CLI 21 minutes against tazuna 7; Kimi K3.1 Pro, Kimi Code 20.5 minutes against tazuna 14.3" src="docs/img/same-model-light.svg">
</picture>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/img/cost-dark.svg">
  <img alt="Cost per task: on Opus 5.5, Claude Code $0.91, tazuna $0.66, tazuna boss mode $0.45 at about the same judge score; on 14 October PRs, tazuna on Opus $2.78 against tazuna on DeepSeek V4.1 $0.60, which solved 13 of 14 against 9" src="docs/img/cost-light.svg">
</picture>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/img/memory-dark.svg">
  <img alt="Memory: Claude Code's UI 190 MB against tazuna's 7.8 MB; a whole agent run on the claude CLI 262-282 MB against 21-22 MB on tazuna's own loop" src="docs/img/memory-light.svg">
</picture>

How to read these: tasks are real merged PRs (Go: caddy, hugo, k6, prometheus, rclone, terraform) with the PR's own tests
hidden from the agent, and a blind Opus judge scoring every candidate for a task together. One run varies by about
±1.2 judge points and ±$0.40, so judge differences under 0.75 and cost differences under 15% count as noise. The
same-model rounds are small (one run per task): read them as "faster and cheaper", not "better". Every measured change
is in the [changelog](CHANGELOG.md); the kit is in [bench/](bench/README.md); the charts come from
[docs/charts.py](docs/charts.py).

<details>
<summary>The numbers as tables</summary>

Other harnesses, Claude Sonnet 5.5 high, six Go PRs × 2 runs (2026-10-07, tazuna 0.0.11):

| Harness | Judge | All hidden tests pass | Median cost | Median time |
|---|---|---|---|---|
| **tazuna** | **6.67** | **8/12** | **$0.41** | **411 s** |
| OMP 18.8.3 | 6.54 | 8/12 | $0.51 | 401 s |
| Claude Code 2.1.292 | 6.38 | 8/12 | $0.43 | 420 s |
| Hermes 0.21.5 | 6.21 | 7/12 | $0.55 | 453 s |
| pi 1.0.4 | 6.08 | 8/12 | $0.28 | 458 s |
| tazuna lean | 6.04 | 8/12 | $0.31 | 381 s |
| OMO 5.1.23 | 5.38 | 7/12 | $0.49 | 307 s |
| Empryo 3.8.7-beta | 5.12 | 8/12 | $0.50 | 464 s |

Same model, four Go PRs, one run each:

| Model | Harness | Judge | Hidden tests | Minutes per task | Cost per task |
|---|---|---|---|---|---|
| DeepSeek V4.1 Flash (Mistral's API) | **tazuna 0.1.73** | **8.00** | **11/11** | **7** | ~$0.06-0.35 |
| | Mistral Vibe CLI 3.1, max | 7.50 | 10/11 | 21 | plan usage |
| Kimi K3.1 Pro (SubRouter) | **tazuna 0.1.4** | 6.88 | 11/13 | **14.3** | **$0.05** |
| | Kimi Code 2.4.0 | 7.25 | 11/13 | 20.5 | $0.11 |

Cost per task:

| Round | Setup | Judge | Cost per task |
|---|---|---|---|
| Opus 5.5 medium, six Go PRs | Claude Code | 7.58 | $0.91 |
| | tazuna | 7.46 | $0.66 |
| | tazuna, boss mode | 7.33 | $0.45 |
| 14 Go PRs merged in October 2026 | tazuna on Opus 5.5 medium | 6.79 (9/14 solved) | $2.78 |
| | tazuna on DeepSeek V4.1 Flash, max | 7.79 (13/14 solved) | $0.60 |

Footprint: binary 4.4 MB; UI 7.8 MB RAM, ~2% CPU while animating, first frame in 8 ms (tazuna 0.1.83). Claude Code's own
UI: 190 MB, 0.45 s to start (2026-10-08). A whole agent run: 21-22 MB on the own loop, 262-282 MB on the claude CLI
(tazuna 0.0.50).

</details>

## What it does

- **Two paths, one UI.** Claude runs through the official `claude` CLI, headless, on your subscription: no token reuse,
  no impersonation. Every other model runs on tazuna's own loop, on any OpenAI-compatible, Responses or Anthropic
  Messages API. A task the cheap model cannot finish can go to Opus (on by default with `--provider`; `--no-escalate`).
- **Verified, not claimed.** When the agent says it is done, tazuna runs the tests and type checks for what changed and
  sends failures back, up to two rounds. Type errors from an edit come back right after the edit.
- **Lean by default.** The router puts tiny repos on Sonnet and the rest on Opus at medium effort. Repos with agent
  config run Claude Code's own prompt with the repo's settings, hooks and skills; others run a ~10-line prompt with four
  tools. Long command output is cut to its start and end. Boss mode (`/boss`; the default for `--run` on Opus) lets Opus
  direct while Haiku workers read, edit and test.
- **MCP for every model.** `/mcp add atlassian https://mcp.atlassian.com/v1/mcp`, then `/mcp on`, and "fix ABC-123" is
  enough. Servers that need a login get one in the browser, as tazuna itself; on Claude your claude.ai connectors come
  along. The list is `~/.config/tazuna/mcp.json`, in Claude Code's format.
- **A terminal UI that stays small.** Four designs (`ctrl-t`; the default cockpit shows context, cache, cost, tools and
  files as they change), click and drag in the message box, `@` for files, images with `ctrl-v`, `!cmd` for the shell,
  `/diff`, `/rewind`, `/resume`, and permission modes on `shift-tab` (auto, ask, edits, plan).
- **Recordings.** Every session is saved to `~/.local/share/tazuna/sessions/` and replays like a video:
  `tazuna <file>.jsonl`.
- **And:** `/loop` builds the open items of a `TODO.md` one by one, `/jira ABC-123` puts a ticket in the message without
  MCP, `--review-model` adds an independent reviewer, `tazuna --log` shows the run log.

## Models beyond Claude

`/model` shows the providers on the left and the selected one's models on the right, with prices and benchmark
scores; typing searches every provider at once. A provider without a key is greyed out until you log in: `/login` lists
the providers, and picking one opens the page where its keys are made in your browser and asks for the key. tazuna keeps
it in `~/.config/tazuna/keys.json` (readable by you only) and puts the provider on `~/.config/tazuna/trusted-providers`,
since it will see your code. A key in the environment (`MORTERA_API_KEY` and so on) still works and wins.

- **SubRouter** (DeepSeek, GPT, Kimi, GLM, MiniMax ...): your own key, or `tazuna subrouter --manage` makes one with the
  providers tazuna checked (they differ a lot: some serve another model, some barely cache); `tazuna subrouter --apply`
  sets them on your own key.
- **Mistral**: an API key, or one made under Code › Vibe CLI.
- **Mortera** (Gemini, GPT, Kimi, Grok, Qwen, GLM, DeepSeek ... at reseller prices): once its key is in, `/model` lists
  every text model on its list, as not checked yet.
- **Anything else** OpenAI-compatible: `--provider URL --key-env VAR --model ID` (its host on the trusted list, or
  `--data any`).

## Safety

- tazuna expects a git repo: in auto mode the agent edits and runs commands without asking, and `git diff` is the
  review. Outside a repo it starts in ask mode.
- A repo's `CLAUDE.md` / `AGENTS.md` go into the prompt; a repo whose config runs code (hooks, skills) asks "Trust this
  repo?" once. In repos you did not write, add `--no-project-instructions`.
- On the own loop the provider key never reaches the agent's commands, and keys saved in tazuna (and the managed
  SubRouter key) are never put in tazuna's environment. Keys you export in your shell are visible to Claude's commands,
  as in any terminal. MCP logins sit
  in `~/.config/tazuna/mcp-auth/`, readable by you only.

## Development

`cargo build --release`, `cargo test`. [ROADMAP.md](ROADMAP.md) holds the goals, the order of work and what was tried
or ruled out; [CHANGELOG.md](CHANGELOG.md) every change with its numbers; [bench/README.md](bench/README.md) the
benchmark kit.
