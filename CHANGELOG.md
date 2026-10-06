# Changelog

Every change that can affect the agent is measured before it stays. Each entry lists what moved and by how much.

**Tiers**
- **Tier 0** (every change, free): tests, `bench/tier0.sh` (binary size, replay render time, input tokens per turn).
- **Tier 1** (changes to prompt, tools, routing or stop logic): 6 real-work Go tasks × 2 runs vs a same-day baseline. Judge
  score, hidden tests, cost, turns, time, cache hit rate.
- **Tier 2** (releases): Tier 1 plus the cross-harness table in the README.

**Noise thresholds**: judge ±0.75, cost ±15% (mean), turns ±20% at 12 runs per arm. Smaller differences are reported as
"within noise". (The judge threshold was ±0.5 until 0.0.3, whose two arms behaved identically and still scored 0.67 apart.)

Also recorded per measurement: the `claude` CLI version. Claude Code updates itself, and its built-in prompt changes token
counts (2.1.289 → 2.1.291 added ~104 input tokens per turn).

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
