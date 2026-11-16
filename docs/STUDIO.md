# tazuna studio (sketch)

An agent-first IDE: the terminal harness's transcript, instruments and review workflow, with code one keypress away
(handed to `$EDITOR`). Not a reimplementation of VS Code — no extensions, no language servers of its own. It keeps what
tazuna measures and GUI harnesses usually hide: what the run costs, which route and model it took, and whether the result
was actually verified.

Runnable sketch: `examples/studio.rs` (mock data, ratatui, no new dependencies).

```
 ▣ TAZUNA STUDIO   api-queue                                      ● waiting on you  opus 5.5 · med   14 turns · $0.42
 TASK                    │╭ session · caddy #6421 ──────────────────────main ✓ clean ╮╭ instruments ───────────────────╮
 fix the cache gauge     ││ ▌ you  +12s                                              ││ CONTEXT                        │
 caddy #6421 · main      ││   src/ui/cockpit.rs takes the gauge.                     ││ ███████████░░░░░░░░░░░░░░░░░░  │
                         ││   make it cache hits / reads, keep the tooltip.          ││ used                    38%    │
 VIEWS                   ││                                                          ││ tokens           76k / 200k    │
 ▌ session               ││ ▌ tazuna  opus 5.5 · med · turn 3/14                     ││ cache               91% hit    │
   review changes    4   ││   Reading the panel first; the gauge is fed              ││                                │
   bench             6   ││   from usage.cache_hit() in session.rs.                  ││ THIS TURN                      │
                         ││                                                          ││ sent                  12.4k    │
 FILES                   ││   ▸ thinking · 1.4k tokens · 2.1s                        ││ out                    1.8k    │
   ui/cockpit.rs   +18 −4││                                                          ││ first token            4.1s    │
   engine.rs       +6  −2││   ▸ Edit   src/ui/cockpit.rs               +18 −4   0.4s ││ speed              48 tok/s    │
   route.rs        +11 −0││     ✓ applied · 3 hunks · typecheck clean                ││                                │
   session.rs      +2  −2││                                                          ││ COST                           │
                         ││   ▸ Bash   cargo test --quiet                    ✓ 2.4s  ││ total                 $0.42    │
 KEYS                    ││     ✓ 41 passed                                          ││ per turn             $0.058    │
   ctrl-k commands       ││                                                          ││ Claude plan 5h 68% · wk 41%    │
   ctrl-o expand         ││   ▸ Verify cargo test --quiet                      3.1s  ││ SubRouter $8.10                │
   esc    stop / quit    ││     ✓ tests 41/41 · typecheck clean                      ││                                │
                         ││                                                          ││ ROUTER                         │
                         ││   Done: the gauge now reads cache hits.                  ││ sonnet 5.5      tried $0.09    │
                         ││                                                          ││   ↳ opus 5.5       now $0.33   │
                         ││ ▌ you  +18s                                              ││                                │
                         ││   good — now run bench 3                                 ││ VERIFY                         │
                         ││                                                          ││ ✓ tests 41/41 · 3.1s           │
                         ││ ▌ tazuna  thinking · 1.4k                                ││ ✓ diagnostics clean            │
                         ││   ▍                                                      ││                                │
                         ││   esc stop  ctrl-k commands  ctrl-o expand               ││ BACKGROUND                     │
                         ││                                                          ││ ◐ npm run dev · 12m            │
                         ││                                                          ││ ✓ 3 tasks finished             │
                         │╰──────────────────────────────────────────────────────────╯╰────────────────────────────────╯
╭ ask ───────────────────────────────────────────────────────────────────────────────ask · auto · @ files · / commands ╮
│ › describe the next change…▏                                                                                         │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯
 ask   auto-approve edits  ·  ctrl-k commands                    1 session · 2 review · 3 bench   tazuna 0.1.64 · $8.10
```

## Why this shape

The transcript stays the primary surface; IDEs that bury the conversation in a tab lose the thread of what the agent
did. The right column is cockpit's instrument panel, made permanent and slightly wider: context, cache hit rate, cost,
the router's escalation, the verifier, background tasks. Those are tazuna's differentiators and should not be a mode
you toggle away from.

The rail is the project, not a file tree: task, views, changed files, keys. A file tree is a keypress away (`e` opens
`$EDITOR`; the `/diff` panel already opens files). Files the agent touched sort first, with +/− counts, because that is
what you review.

## Three views

| Key | View | What it is |
|---|---|---|
| `1` | session | transcript, composer, instrument panel. Cockpit's live numbers, now with room for plan windows and the router. |
| `2` | review | changed files, diff hunks, `a`/`x` to accept or reject, verify status on the same screen. |
| `3` | bench | the benchmark table (hidden tests, judge, cost) for this task; select a candidate to see its run log and re-run with another model/effort. |

`ctrl-k` opens the command palette: sessions, `/model`, `/diff`, `/rewind`, design switch. Everything that does not
deserve a permanent key lives there.

## What is real, what is mock

The sketch only draws. In the real build:

- **Session view**: `Session` + `View` from `src/ui/mod.rs` already hold the transcript, usage, cost, calls, changed
  files and panels. The right column is a re-layout of cockpit's numbers (`src/ui/cockpit.rs`), not new state.
- **Review view**: `Changed`, `DiffPanel` and the git diff in `src/ui/mod.rs`; accept/reject would apply hunks to the
  worktree, then rerun `verify.rs` and `diag.rs` exactly as a normal run does.
- **Bench view**: `runlog.rs` (`~/.local/share/tazuna/runs.jsonl`) already records stop reason, cost, cache per turn,
  tools and files per run; `bench/` holds the hidden tests and the judge. The row table is a join of those two.
- **Router**: `route.rs` and the run log's model/effort per turn; the `sonnet → opus` line is the escalation tazuna
  already performs on small repos.
- **Background tasks**: `View.running` and the ctrl-b tasks panel already exist; here they get a fixed slot instead of
  a footer.

Nothing here changes the harness: it is the same event stream and the same run log, drawn differently.

## Build order, if it becomes more than a sketch

1. Add the side column and the rail to the existing live view (no new state).
2. Make review's accept/reject real: apply a hunk, run verify, show the result inline.
3. Add the bench view over `runs.jsonl`; read-only first.
4. Only then consider an editor pane (`$EDITOR` inside a PTY), because that is where scope explodes.

## Run it

```sh
cargo run --example studio                      # interactive, mock state
cargo run --example studio -- --snapshot        # one frame, plain text
cargo run --example studio -- --snapshot --ansi | less -R
cargo run --example studio -- --snapshot --view review --size 100x30
cargo run --example studio -- --snapshot --palette
cargo test --example studio                     # key handling and render smoke tests
```
