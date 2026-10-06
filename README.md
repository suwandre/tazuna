# tazuna

手綱. A lean terminal harness around coding models, built for token efficiency and output quality.

tazuna drives the official `claude` CLI headlessly (`stream-json` in and out) with a lean profile, so Claude runs on your own
subscription through first-party Claude Code: no token reuse, no impersonation. Other providers will get their own agent loop.

## Status

Early. What works today:

- **Live mode**: run `tazuna` in a git repo, type a task. One `claude` process per conversation (warm prompt cache),
  clean interrupts (`ctrl-c`), follow-ups, copy last reply (`ctrl-y`), mouse scrolling.
- **Profile "cc-mid"**: ~10-line system prompt, `Grep`/`Glob` plus four lean tools (bash/read/write/edit) from a bundled MCP
  server, and none of the user's settings, skills, hooks or MCP servers. Benchmarked as the best cost/quality profile.
- **Four UI designs** (Ratatui + tachyonfx): aurora, cockpit (default: live context/cache gauges, tokens per turn, cost,
  tools, files), zen, timeline. `ctrl-t` switches.
- **Recordings**: every session is saved to `~/.local/share/tazuna/sessions/` and replays like a video.
- **Headless mode** for benchmarks: `tazuna --run prompt.txt` prints claude's result JSON.

Parity check (6 real Go tickets × 2 runs, Opus 5.5 medium): same hidden-test pass rate and judge score as `claude -p` with
the same profile, identical per-turn overhead.

## Usage

```sh
cargo build --release
cd <some git repo>
/path/to/tazuna/target/release/tazuna                         # Opus 5.5, medium effort
tazuna --model claude-sonnet-5-5 --effort high
tazuna ~/.local/share/tazuna/sessions/<file>.jsonl           # replay a session (space pause, ←/→ speed, 1-4 design)
```

Requires the `claude` CLI (logged in) and Node (for the bundled lean tool server, until it is ported to Rust).

tazuna only runs inside a git repo by default: the agent edits without asking, so `git diff` is the review step.

## Layout

```
src/main.rs      modes (live, replay, snapshot, headless) and the event loop
src/live.rs      the claude process: spawn, stream-json, interrupt, recording
src/session.rs   session state built from stream-json events
src/replay.rs    recorded sessions on a virtual clock
src/ui/          shared transcript renderer + the four designs
tools/           the lean MCP tool server (bundled into the binary)
recordings/      sample recording and the recorder used to make it
```
