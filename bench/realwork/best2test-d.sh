#!/bin/bash
# Round 2's last task (caddy-retry on LingAI: one run v267, best of two v268) after v265, then one judge round over both
# rounds (rclone, caddy-unix, caddy-retry pools: both arms, tazuna on Opus v34 and the two anchors), two passes: 6 calls,
# run only if the dry run says 6. Round 3 dropped. Logs to best2test.log.
cd "$(dirname "$0")"; source ~/.subrouter-key; source ~/.bench-token
export SUBROUTER_X_KEYENV=SUBROUTER_LUNA4_KEY SUB_KEYENV=SUBROUTER_LUNA4_KEY HARNESS_BIN=$HOME/bench-homes/tz-v78/tazuna GUARD_PAUSE_AT=85 GUARD_WEEK_AT=95 SUB_EFFORT=max
log() { echo "$(date +%H:%M) $*" >> best2test.log; }
used() { curl -s -A curl/8 --max-time 15 https://subrouter.ai/v1/dashboard/billing/usage -H "Authorization: Bearer $SUBROUTER_MODELS_KEY" | python3 -I -c "import json,sys;print(json.load(sys.stdin)['total_usage'])"; }
while ps -axo command= | grep run.py | grep -q "v265@"; do sleep 30; done
U=$(used); [ -n "$U" ] || U=$(used); [ -n "$U" ] || { log "no SubRouter counter reading: not starting"; exit 1; }
./sub_guard.sh "$U" 700 v267 v268 >> sub-guard-best2test.log 2>&1 &
log "round 2 caddy-retry-match-expr on LingAI: one run v267, best of two v268"
SUB_HARNESS=tazuna-ownsubx ./sub_run.sh gpt-6-luna v267 0.0082,0.0008,0.041 0.5 caddy-retry-match-expr &
SUB_HARNESS=tazuna-ownsubxbest2 ./sub_run.sh gpt-6-luna v268 0.0082,0.0008,0.041 0.5 caddy-retry-match-expr &
wait $(jobs -p | tail -2); log "round 2 done"
J="BENCH_SET=tasks-oct JUDGE_TASKS=rclone-archive-symlinks,caddy-unix-socket-close,caddy-retry-match-expr JUDGE_OUT=judge.oct.best2.jsonl"
n=$(env $J JUDGE_ONLY="$(cat judge-best2-only.txt)" python3 -I ./judge.py --passes 2 --dry-run 2>&1 | grep -oE "= [0-9]+ judge calls" | grep -oE "[0-9]+")
[ "${n:-0}" = 6 ] || { log "judge not run: dry run said ${n:-?} calls (expected 6)"; exit 1; }
env $J JUDGE_ONLY="$(cat judge-best2-only.txt)" python3 -I ./judge.py --passes 2 > judge-best2.log 2>&1
log "judged (6 calls); BEST2TEST-DONE"
