#!/usr/bin/env python3
"""The compaction A/B's judge (cmp-1127.sh): Opus 5.5 on the Claude plan, one call per task and run index, each with the
two arms' diffs of that run (blind, shuffled, against the reference fix: judge.py). Before each call the plan's 5-hour
meter is read from tazuna's app-server (`accounts`, "Current session"); at 90% or more it stops and says so. Counts the
calls. Run it after the 5-hour window has reset. guard.py is off for this round: the meter reading before each call is
written as the guard's state, which judge.py checks."""
import json, os, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
TAZUNA = Path.home() / "bench-homes/tz-v1235/tazuna"
TASKS = ["caddy-tls-automate-names", "prometheus-replay-mmap", "prometheus-wbl-checkpoint"]
ARMS = ["tazuna-ownmisv-v1235@deepseek-v4-1-flash-1017:high", "tazuna-ownmisvcmp-v1235@deepseek-v4-1-flash-1017:high"]
STOP_AT = 90.0


def session_used():
    """The Claude plan's "Current session" (5-hour) use in percent, as the window's account card reads it."""
    out = subprocess.run(f"(echo '{{\"op\":\"accounts\"}}'; sleep 8; echo '{{\"op\":\"quit\"}}') | perl -e 'alarm 30; exec @ARGV' {TAZUNA} app-server",
                         shell=True, capture_output=True, text=True,
                         # the user's own login reads the plan (with the bench token set the card carries no reading); the
                         # token is the same plan's, for the judge's claude -p
                         env={k: v for k, v in os.environ.items() if k != "CLAUDE_CODE_OAUTH_TOKEN"}).stdout
    found = re.findall(r'"Current session","used":([0-9.]+)', out)
    if not found:
        raise SystemExit("meter: no Current session reading; no judge call")
    return max(float(f) for f in found)


def main():
    calls = 0
    for i in range(3):
        out = HERE / f"judge.cmp1127.r{i}.jsonl"
        done = {json.loads(l)["task"] for l in out.read_text().splitlines()} if out.exists() else set()
        results = [json.loads(l) for l in (HERE / "results-tasks-hard.jsonl").read_text().splitlines() if l.strip()]
        for t in TASKS:
            if t in done:
                continue
            # both arms' run i finished (a run still going or run again: later)
            if not all(any(r["task"] == t and r["harness"] == a and r["run"] == i for r in results) for a in ARMS):
                print(f"waiting: {t} r{i} has not both arms' results yet", flush=True)
                continue
            used = session_used()
            print(f"meter before {t} r{i}: {used}%", flush=True)
            if used >= STOP_AT:
                print(f"STOP: Claude 5-hour use at {used}% (>= {STOP_AT}%) after {calls} judge calls", flush=True)
                return 1
            # judge.py asks for a fresh guard reading; guard.py stays off this round (its claude -p probes, and its PAUSE
            # at 70% would hold the Mistral-only runs), so this reading stands in for it, written just before the call
            (HERE.parent / "guard.state.json").write_text(json.dumps({"util": used, "five_hour": used, "extra": 0.0, "baseline": 0.0,
                "at": __import__("time").time(), "by": "cmp1127_judge (tazuna app-server accounts)"}))
            env = {**os.environ, "BENCH_SET": "tasks-hard", "JUDGE_TASKS": t, "JUDGE_OUT": str(out),
                   "JUDGE_ONLY": ",".join(f"{a}-r{i}" for a in ARMS)}
            r = subprocess.run([sys.executable, "-u", str(HERE / "judge.py"), "--model", "claude-opus-5-5", "--effort", "xhigh"],
                               cwd=HERE, env=env, capture_output=True, text=True)
            calls += 1
            print(f"judge call {calls}: {t} r{i}: {(r.stdout + r.stderr).strip().splitlines()[-1:] or ['(no output)']}", flush=True)
    print(f"done: {calls} judge calls", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
