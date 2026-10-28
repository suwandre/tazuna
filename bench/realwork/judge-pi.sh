#!/bin/bash
# 2026-10-28: the three-way panel (tazuna 0.0.97, stock Claude Code, pi; deepseek-v4.1-flash-1017 at max on foxrouter),
# judged blind once v530 ends: one pool per task with every valid run (night-1028 and today's), two passes, 8 calls.
# Excluded: v504 (read Claude Code's tree), v514/v518 (fix from the web / an earlier transcript); v510/v512/v527 have no row.
cd "$(dirname "$0")"; source ~/.bench-token
caffeinate -ims -w $$ &
until grep -q " end:" mrt-v530.log; do sleep 30; done
python3 -I leakscan.py > leakscan-judgepi.out 2>&1
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
T="492 494 496 498 500 502 506 509 511 513 517 519 521 529 531"; C="493 495 497 499 501 503 505 507"; PI="508 515 516 520 526 528 530 532"
ONLY="$(for v in $T; do echo -n "tazuna-ownsubx-v$v@deepseek-v4.1-flash-1017:max,"; done)$(for v in $C; do echo -n "cc-sub-v$v@deepseek-v4.1-flash-1017:max,"; done)$(for v in $PI; do echo -n "pi-sub-v$v@deepseek-v4.1-flash-1017:max,"; done)tazuna-repo-v34@claude-opus-5-5:medium"
judge() {  # set tasks expected-calls
  n=$(BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.pi1028.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 --dry-run 2>&1 | grep -oE "= [0-9]+ judge calls" | grep -oE "[0-9]+")
  if [ "${n:-0}" = "$3" ]; then
    BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.pi1028.$1.jsonl JUDGE_ONLY="$ONLY" python3 -I ./judge.py --passes 2 >> judge-pi1028.log 2>&1
    echo "$(date +%H:%M) judged $1: $(grep -c . judge.pi1028.$1.jsonl 2>/dev/null) rows" >> judge-pi1028.log
  else
    echo "$(date +%H:%M) judge $1 not run: dry run said ${n:-?} (expected $3)" >> judge-pi1028.log
  fi
}
judge tasks caddy-quoted-braces,terraform-import-foreach 4
judge tasks-hard caddy-tls-automate-names 2
judge tasks-oct caddy-retry-match-expr 2
echo "$(date +%H:%M) JUDGE-PI-DONE" >> judge-pi1028.log
