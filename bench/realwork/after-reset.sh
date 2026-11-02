#!/bin/bash
# 2026-11-02: what the 5-hour window stopped before 21:00 continues after its reset (counted beforehand): at 21:05 the
# lean runs ln.sh skipped (finished ones are skipped), then the rest of the Haiku effort judging (judge.py skips judged
# tasks; 12 calls in all), then the lean judging (one pool per task with the same day's repo-profile runs, two passes, 12
# calls). Judge calls stop when the guard says pause (85%). Started with nohup.
cd "$(dirname "$0")"; source ~/.bench-token
target=0  # run now: the guard saw the reset at 21:11, after the first try at 21:05 (all skipped)
while [ "$(date +%s)" -lt "$target" ]; do sleep 60; done
while pgrep -f 'go\.sh|ln\.sh' > /dev/null; do sleep 60; done
echo "$(date +%H:%M) after the reset: lean leftovers, then the judging" >> ln.log
./ln.sh
RT="$(for v in $(seq 1209 2 1231); do echo -n "tazuna-rtall-v$v@claude-opus-5-5:medium,"; done)"
BH="$RT$(for v in $(seq 1257 1292); do for a in bhmed bhhigh bhxhigh; do echo -n "tazuna-$a-v$v@claude-opus-5-5:medium,"; done; done)"
LN="$RT$(for v in $(seq 1293 1304); do echo -n "tazuna-leanopus-v$v@claude-opus-5-5:medium,"; done)"
BENCH_SET=tasks JUDGE_OUT=judge.bh.tasks.jsonl JUDGE_ONLY="$BH" python3 -I ./judge.py --passes 2 >> judge-bh.log 2>&1
echo "$(date +%H:%M) Haiku effort judged ($(wc -l < judge.bh.tasks.jsonl) rows)" >> ln.log
BENCH_SET=tasks JUDGE_OUT=judge.ln.tasks.jsonl JUDGE_ONLY="$LN" python3 -I ./judge.py --passes 2 >> judge-ln.log 2>&1
echo "$(date +%H:%M) JUDGED-ALL (lean $(wc -l < judge.ln.tasks.jsonl) rows)" >> ln.log
