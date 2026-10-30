#!/bin/bash
# 2026-10-30 overnight: Grok 4.7's reasoning. October's run (v161, effort high, ~1,600 output tokens a turn) against today's
# runs at max (~19 a turn, no visible reasoning, loops). After the fate86 run at max (v687) ends: one rclone run on fate86 at
# high (v688), then the rclone pool judged (v161, v687, v688, Kimi v665), two passes; then the Mac sleeps (user asked).
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> overnight-1030.log; }
caffeinate -ims -w $$ &
export SUB_EFFORT=high SUB_KEYENV=SUBROUTER_MODELS_KEY GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95
while pgrep -f "sub_run.sh grok-4.7 v687" >/dev/null; do sleep 30; done
log "v687 over: $(grep -h ' r0:' bench.v687.log | tail -1 | cut -c1-160) $(grep -h stopped mrt-v687.log | tail -1)"
U=$(curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])")
[ -n "$U" ] && ./sub_guard.sh "$U" 300 v688 >> sub-guard-overnight-1030.log 2>&1 &
ps -axo command= | grep -q "[g]uard.py" || (cd .. && nohup ./guard.py >> guard.log 2>&1 &)
SUBROUTER_X_KEYENV=SUBROUTER_VET4_KEY SUB_SET=tasks-oct SUB_HARNESS=tazuna-ownsubx HARNESS_BIN=$HOME/bench-homes/tz-v106/tazuna ./sub_run.sh grok-4.7 v688 0.012,0.003,0.035 1.0 rclone-archive-symlinks < /dev/null
log "v688 over: $(grep -h ' r0:' bench.v688.log | tail -1 | cut -c1-160) $(grep -h stopped mrt-v688.log | tail -1)"
python3 -I leakscan.py > leakscan-overnight-1030.out 2>&1
G=grok-4.7
BENCH_SET=tasks-oct JUDGE_TASKS=rclone-archive-symlinks JUDGE_OUT=judge.overnight-1030.tasks-oct.jsonl \
  JUDGE_ONLY="tazuna-ownsubgp-v161@$G:high,tazuna-ownsubx-v687@$G:max,tazuna-ownsubx-v688@$G:high,tazuna-ownsubx-v665@kimi-k3.1-pro:max" \
  python3 -I ./judge.py --passes 2 >> judge-overnight-1030.log 2>&1
log "judged rclone"
log "OVERNIGHT-DONE; sleeping"
pmset sleepnow
