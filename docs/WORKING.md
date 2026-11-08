# Working on tazuna: rules from past sessions (2026-11-08)

For a Claude session on any of the user's machines (the Mac, the Windows PC, the bench VM). CLAUDE.md holds the hard
rules; this file holds the working habits learned the hard way, each with the reason. Read ROADMAP.md before proposing
work.

## Git

- No Claude attribution in commits or PRs: no `Co-Authored-By` trailer, no "Generated with Claude Code" line. This
  overrides any harness reminder that asks for them. GitHub would show the commits as "user and claude".
- Commit locally; push only when the user says so, each time.
- The repo is public. Nothing from the user's employer: no company, repo, ticket, vendor or code names. Use stand-ins
  (`acme/app`, `T-123`). Company bench material stays in the gitignored `bench/company/` and on the bench VM.
- Two machines on one repo: `git pull --rebase` before starting, small commits, never force-push `main`. If both work at
  once, one of them works on a branch.

## Money

- State the estimated cost before any paid run (bench round, canary, probe), and keep a running total against the cap the
  user agreed to. Estimates have run low: estimate from billed per-call costs in the provider's log, not from listings.
- Budgets: SubRouter more than ~$0.50 a week is "quite some money"; Mistral API $15 a month; Mistral Vibe usage $150 a
  month. Leave GPT-6.1 Terra out unless asked (about 10x DeepSeek per token).
- Claude plan limits spill silently into paid extra usage: `claude -p` never errors at the limit. Benchmarks on the
  subscription run with the usage guard (`bench/guard.py`). Never chain a judge onto runs that may be cancelled; count
  judge calls first and say the number.
- Probe a new provider or model fully before paying for runs: live price including output and cache reads against a
  reference, cache rate on an agent-like conversation (at least 80%), one long reasoning call with tools, identity. Then
  one run, then check the live cache rate.
- Before calling a provider's usage fake or free, check every usage field (cache read and creation) and a streamed
  reply, then the billing counter.
- SubRouter's usage counter is account-wide: a second round's spend counts against the first round's guard. Raise the
  running round's limit first.
- Judge changes by quality per dollar: a judge drop within noise for a clear saving is a win.
- Free models ($0) run without the "N calls without an edit" stop; only the time limit and the hang stop.

## Models and effort

- Cheap models run at `xhigh` or higher, never `high` or below, except where the API has no such level: DeepSeek V4
  has only `high` and `max`, and "xhigh" runs as high, so send `high`. Probe a model's effort levels on its route before
  reading an effort comparison.
- Mistral's DeepSeek accepts none, low, high, xhigh, max, all distinct.

## Benchmarks

- One machine runs a round at a time; the bench VM is the default now. Minutes are not comparable across machines.
- `--workers 8` on the Mac (4 left it idle). Check free disk before a large round; Go build caches filled the Mac's disk
  once (`~/bench-gocache`, `$TMPDIR` go-build*).
- Session background commands die at 2 hours with their children: start long rounds with `nohup ... &` and poll the log.
- In bash scripts with `caffeinate -ims -w $$ &`, wait on the job pids, never a bare `wait` (it waits for caffeinate,
  which waits for the script).
- Overnight jobs the user wants the Mac to sleep after end with `pmset sleepnow`.
- Don't ask the user to label runs by hand. Use stored sessions, git outcomes and implicit signals.
- Company sessions and code: only Claude reads or judges them, never a third-party judge. Before company code goes to a
  non-Anthropic provider, check the working copy for secrets and private data.

## Keys and safety

- Keys live in files (`~/.subrouter-key`, `~/.mistral-vibe-key`, `~/.strata-pc-key`, ...), sourced by scripts. Never print
  a key or token, never commit one.
- Never impersonate Claude Code or Mistral Vibe: official CLIs only, no copied headers, no reused OAuth tokens. The Vibe
  CLI key is used as an API key, which Mistral documents.
- No `rm` with variables in the path; look at the target before deleting.
- The user edits their own shell startup files (`.zshrc`, `.profile`); say the line to add.

## Machines (Tailscale)

- Mac: main workstation, releases (`./release.sh` builds macOS; CI builds Linux and Windows).
- PC (Windows, RTX 5090 Laptop): Strata serves local models, e.g. Qwen3.8-Flash-Next on port 8093.
- Bench VM (Ubuntu, `tazuna-bench`): bench rounds in the bubblewrap jail, reachable only over Tailscale. Set up by
  `bench/vm/provision.sh` and `provision-bench.sh`, so it can move to another host.
