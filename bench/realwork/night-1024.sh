#!/bin/bash
# 2026-10-24 night: keep the Mac awake until the Luna retest's round 2 (v281-v286) is done, then one judge round (the
# retest, both OpenCode runs and the anchors; one pool per task, two passes: 6 calls, run only if the dry run says 6),
# a summary (best2retest-summary.txt), a commit, and sleep. The usage guard is restarted if it dies. Stops waiting at 15%
# battery or 04:30 (the runs are stopped then and the judge runs on what finished). Logs to night-1024.log.
cd "$(dirname "$0")"; source ~/.bench-token
log() { echo "$(date +%H:%M) $*" >> night-1024.log; }
caffeinate -i -s -w $$ &
log "caffeinate on"
late() { b=$(pmset -g batt | grep -oE '[0-9]+%' | tr -d %); { [ -n "$b" ] && [ "$b" -le 15 ] && ! pmset -g batt | grep -q "AC Power"; } || { [ "$(date +%H%M)" -ge 0430 ] && [ "$(date +%H%M)" -lt 1200 ]; }; }
guard() { ps -axo command= | grep -q "[g]uard.py" || { (cd .. && nohup ./guard.py >> guard.log 2>&1 &); log "usage guard restarted"; }; }
tree() { local c; for c in $(ps -axo pid=,ppid= | awk -v p="$1" '$2==p{print $1}'); do tree "$c"; done; echo "$1"; }
while ps -axo command= | grep run.py | grep -qE "v28[1-6]@"; do
  guard
  if late; then
    log "battery or 04:30: stopping round 2"
    for p in $(ps -axo pid=,command= | awk '/run.py/ && /v28[1-6]@/ && !/awk/ {print $1}'); do kill $(tree $p) 2>/dev/null; done
    break
  fi
  sleep 60
done
log "round 2 over: $(tail -1 best2retest.log)"
guard; sleep 160  # a fresh meter reading for the judge
J="BENCH_SET=tasks-oct JUDGE_TASKS=rclone-archive-symlinks,caddy-unix-socket-close,caddy-retry-match-expr JUDGE_OUT=judge.oct.best2retest.jsonl"
n=$(env $J JUDGE_ONLY="$(cat judge-night1024-only.txt)" python3 -I ./judge.py --passes 2 --dry-run 2>&1 | grep -oE "= [0-9]+ judge calls" | grep -oE "[0-9]+")
if [ "${n:-0}" = 6 ] && ! late; then
  env $J JUDGE_ONLY="$(cat judge-night1024-only.txt)" python3 -I ./judge.py --passes 2 > judge-best2retest.log 2>&1
  log "judged: $(grep -c . judge.oct.best2retest.jsonl 2>/dev/null) rows"
else
  log "judge not run: dry run said ${n:-?} calls (expected 6), or too late"
fi
python3 -I - > best2retest-summary.txt 2>&1 <<'PY'
import json, collections, statistics
res = [json.loads(l) for l in open("results-tasks-oct.jsonl")]
jud = {}
for l in open("judge.oct.best2retest.jsonl") if __import__("os").path.exists("judge.oct.best2retest.jsonl") else []:
    j = json.loads(l); jud[(j["task"], j["harness"])] = j["overall"]
arm = lambda h: next((a for a in ("best2", "atta", "attb") if f"ownsubx{a}-" in h), "opencode-default" if h.startswith("opencode-v287") else "opencode-max" if h.startswith("opencode-v288") else "single" if "ownsubx-" in h else None)
rows = [r for r in res if r["task"] in ("rclone-archive-symlinks", "caddy-unix-socket-close", "caddy-retry-match-expr")
        and any(f"-v{v}@" in r["harness"] for v in list(range(275, 287)) + [287, 288])]
print("task | arm | run | hidden | judge | turns | min | picked")
for r in sorted(rows, key=lambda r: (r["task"], arm(r["harness"]) or "", r["harness"])):
    print(r["task"][:12], "|", arm(r["harness"]), "|", r["harness"].split("@")[0], "|", f"{r['passed']}/{r['total']}", "|",
          jud.get((r["task"], r["harness"])), "|", r["turns"], "|", round(r["secs"] / 60, 1), "|", "picked" if r.get("picked") else "")
print()
by = collections.defaultdict(list)
for r in rows:
    s = jud.get((r["task"], r["harness"]))
    if s is not None:
        by[arm(r["harness"])].append(s)
for a, v in sorted(by.items()):
    print(f"{a}: judge mean {statistics.mean(v):.2f} over {len(v)}")
pairs = collections.defaultdict(dict)
for r in rows:
    a = arm(r["harness"])
    if a in ("atta", "attb"):
        pairs[(r["task"], r["harness"].split("-v")[1].split("@")[0])][a] = (jud.get((r["task"], r["harness"])), r.get("picked"))
right = wrong = tie = 0
for k, p in pairs.items():
    if len(p) == 2 and None not in (p["atta"][0], p["attb"][0]):
        picked, other = (p["atta"], p["attb"]) if p["atta"][1] else (p["attb"], p["atta"])
        right += picked[0] > other[0]; wrong += picked[0] < other[0]; tie += picked[0] == other[0]
print(f"picks: better {right}, worse {wrong}, tie {tie}")
PY
log "summary written"
cd ../..
git add bench/realwork/{run.py,results-tasks-oct.jsonl,best2-picks.jsonl,best2retest.sh,best2retest.log,best2retest-summary.txt,night-1024.sh,judge-night1024-only.txt} bench/realwork/runs-tasks-oct/ 2>/dev/null
[ -f bench/realwork/judge.oct.best2retest.jsonl ] && git add bench/realwork/judge.oct.best2retest.jsonl
if git diff --cached | grep -qE "sk-[A-Za-z0-9]{20,}|Bearer [A-Za-z0-9]{20,}"; then
  log "commit skipped: something like a key in the staged changes"
else
  git commit -q -m "Bench: best-of-two retest (Luna, 4-providers key) and OpenCode free DeepSeek; judged, summary in best2retest-summary.txt

Both attempts of each best-of-two run scored on their own; the opencode harness runs the official OpenCode CLI
(OpenCode Zen's free models answer only there)." && log "committed $(git log -1 --format=%h)"
fi
cd bench/realwork
log "sleeping"; sleep 30; pmset sleepnow
