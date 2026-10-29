#!/bin/bash
# The reviewer runs (v648-v660) judged again by GPT-6.1 Sol (EFF=xhigh|max): Opus judged arms where Opus was the reviewer.
cd "$(dirname "$0")"
source ~/.subrouter-key
M=deepseek-v4.1-flash-1017
ONLY="$(for v in $(seq 648 660); do for h in tazuna-ownsubx tazuna-ownsubxrev tazuna-ownsubxopus; do echo -n "$h-v$v@$M:max,"; done; done)"
one() {
  BENCH_SET=$1 JUDGE_TASKS=$2 JUDGE_OUT=judge.review-sol-$EFF.$1.jsonl JUDGE_ONLY="$ONLY" JUDGE_KEY_ENV=SUBROUTER_VET1_KEY \
    python3 -I ./judge.py --model gpt-6.1-sol@https://subrouter.ai/v1 --effort $EFF --passes 2 $DRY
}
one tasks caddy-quoted-braces,terraform-import-foreach &
one tasks-hard caddy-tls-automate-names &
one tasks-oct caddy-retry-match-expr &
wait
echo SOL-DONE
