#!/bin/bash
# 2026-10-30: the rival round's judge, started by itself once the reruns (v736-v741) are done and the 5-hour window has
# reset (the guard lifts PAUSE; GUARD_PAUSE_AT=85 so the weekly 76% does not hold it): one pool per task, two passes, 8 calls.
cd "$(dirname "$0")"; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> hx.log; }
caffeinate -ims -w $$ &
M=deepseek-v4.1-flash-1017
while ps -axo command= | grep -qE "^/bin/bash \./sub_run\.sh $M v7(3[6-9]|4[01]) "; do sleep 60; done
log "reruns over: $(for v in 736 737 738 739 740 741; do grep -h ' r0:' bench.v$v.log | tail -1 | awk '{print $3, $5}' | sed "s/@$M:max//"; done | tr '\n' ';')"
until [ ! -e ../PAUSE ] && [ ! -e ../ABORT ] && python3 -I -c "import json,time,sys;d=json.load(open('../guard.state.json'));sys.exit(not(time.time()-d['at']<600 and d['five_hour']<50))"; do sleep 60; done
log "window clear; judging"
python3 -I leakscan.py > leakscan-hx.out 2>&1
pool() { echo -n "omp-sub-v$1@$M:max,opencode-sub-v$2@$M:max,cmd-sub-v$3@$M:max,tazuna-ownsubx-v$4@$M:max,tazuna-ownsubx-v$5@$M:max"; }
for s in "tasks terraform-import-foreach $(pool 721 729 740 705 707)" "tasks caddy-quoted-braces $(pool 737 730 741 709 711)" \
         "tasks-oct caddy-retry-match-expr $(pool 725 739 735 713 715)" "tasks-oct rclone-archive-symlinks $(pool 738 732 736 717 719)"; do
  set -- $s
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.hx.$1.jsonl JUDGE_ONLY="$3" python3 -I ./judge.py --passes 2 >> judge-hx.log 2>&1
  log "judged $2"
done
log "HX-DONE"
