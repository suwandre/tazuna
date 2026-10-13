#!/bin/zsh
# Tonight (2026-10-13): wait for the two MiMo caddy-unix runs, then one Opus judge call for them and DeepSeek side by side.
cd "${0:A:h}/realwork" || exit 1
battery_low() { pmset -g batt | grep -q "Battery Power" && [ "$(pmset -g batt | grep -o '[0-9]*%' | head -1 | tr -d %)" -le 15 ]; }
while pgrep -f "[P]ython run.py --harness tazuna-own" >/dev/null; do
  if battery_low; then pkill -f "[P]ython run.py"; pkill -f "tazuna engine"; echo "stopped: battery low $(date)" >> ../night-mimo.log; exit; fi
  sleep 60
done
source ~/.bench-token
export BENCH_SET=tasks-oct JUDGE_TASKS=caddy-unix-socket-close JUDGE_OUT=judge.oct.mimo.jsonl
export JUDGE_ONLY="tazuna-ownel-v46@mimo-v2.6-pro:max-r0,tazuna-own-v46@mimo-v2.6-pro:max-r0,tazuna-own-v38@deepseek-v4.1-flash:max-r0"
plan=$(python3 judge.py --dry-run 2>&1 | tail -1)
echo "$(date) $plan" >> ../night-mimo.log
calls=$(print -r -- "$plan" | grep -o '= [0-9]* judge calls' | grep -o '[0-9]*')
if [ -z "$calls" ] || [ "$calls" -gt 2 ]; then echo "not judging: expected at most 2 calls ($plan)" >> ../night-mimo.log; exit; fi
python3 judge.py >> ../night-mimo.log 2>&1
grep -ah " r0: " bench.mimo.log bench.mimo-ci.log >> ../night-mimo.log
echo "done $(date)" >> ../night-mimo.log
