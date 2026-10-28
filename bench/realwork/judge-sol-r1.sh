#!/bin/bash
# 2026-10-28: the Sol panel's round 1, judged blind before Codex's caddy-tls rerun (v561) is done: caddy-quoted and
# terraform (one pool per task) and caddy-retry, two passes each, side by side (6 calls). caddy-tls later, with v561.
cd "$(dirname "$0")"; source ~/.bench-token
caffeinate -ims -w $$ &
M=gpt-6.1-sol
ONLY="$(for v in 533 534 535 536 538 557 558 559 560; do for h in codexsub pi-sub tazuna-ownsubx; do echo -n "$h-v$v@$M:max,"; done; done)"
BENCH_SET=tasks JUDGE_TASKS=caddy-quoted-braces,terraform-import-foreach JUDGE_OUT=judge.sol1028.tasks.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-sol1028.log 2>&1 &
BENCH_SET=tasks-oct JUDGE_TASKS=caddy-retry-match-expr JUDGE_OUT=judge.sol1028.tasks-oct.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-sol1028.log 2>&1 &
wait
echo "$(date +%H:%M) JUDGE-SOL-R1-DONE" >> judge-sol1028.log
