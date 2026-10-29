#!/usr/bin/env python3
"""Each reviewer and review prompt on the same saved diff, the review only (`tazuna --review-only`), so what differs is
the reviewer, not the implementer's run. Diffs: the no-review arm of the 2026-10-29 reviewer runs (DeepSeek 1017, max).
Inside the bench jail, like the runs. Appends to replay-review.jsonl. The scoped arm needs tz-v102r (the
TAZUNA_REVIEW_SCOPED lever, dropped in 0.1.2).

  replay-review.py [--dry-run]
"""
import json, os, subprocess, sys, tempfile, threading, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE)); sys.path.insert(0, str(HERE.parent))
import run, judge, subrouter

TZ = os.path.expanduser("~/bench-homes/tz-v102r")
M = "deepseek-v4.1-flash-1017"
DIFFS = [("tasks", "caddy-quoted-braces", "v648"), ("tasks-oct", "caddy-retry-match-expr", "v654"),
         ("tasks", "terraform-import-foreach", "v651")]
ARMS = {"opus": ["claude-opus-5-5", "max"], "astra": ["gpt-6-astra@https://subrouter.ai/v1", "xhigh"]}
OUT, lock = HERE / "replay-review.jsonl", threading.Lock()


def one(job):
    (bset, task, v), arm, scoped = job
    diff = HERE / ("runs" if bset == "tasks" else f"runs-{bset}") / task / f"tazuna-ownsubx-{v}@{M}:max-r0.diff"
    w = Path(tempfile.mkdtemp(prefix="rw-replay-"))  # rw-: other jailed runs cannot read it
    side = Path(tempfile.mkdtemp(prefix="replay-side-"))  # the task file and claude's config, outside the work tree
    g = json.loads((HERE / bset / task / "git.json").read_text())  # setup_workdir without the module download (no build)
    subprocess.run(f"git -C '{g['repo']}' archive {g['base']} | tar -x -C '{w}'", shell=True, check=True)
    for c in (["init", "-q"], ["add", "-A"], [*run.GIT_ID, "commit", "-qm", "base"]):
        subprocess.run(["git", *c], cwd=w, check=True)
    subprocess.run(["git", "apply", "--whitespace=nowarn", str(diff)], cwd=w, check=True)
    (side / "task.txt").write_text((HERE / bset / task / "prompt.txt").read_text())
    model, effort = ARMS[arm]
    cmd = [TZ, "--review-only", "--run", str(side / "task.txt"), "--review-model", model, "--review-effort", effort]
    env = {**os.environ, **run.proxy_env(), "CLAUDE_CONFIG_DIR": str(side / "claude")}
    if "@" in model:
        cmd += ["--review-key-env", "SUBROUTER_VET1_KEY", "--data", "any"]
    else:
        judge.guard_allows()
    if scoped:
        env["TAZUNA_REVIEW_SCOPED"] = "1"
    t = time.time()
    p = run.sh(cmd, w, timeout=1000, env=env)
    try:
        r = json.loads(p.stdout.strip().splitlines()[-1])
    except (ValueError, IndexError):
        r = {"error": (p.stderr or p.stdout)[-300:]}
    row = dict(task=task, diff=v, arm=arm, scoped=scoped, wall=round(time.time() - t), **r)
    with lock, OUT.open("a") as f:
        f.write(json.dumps(row) + "\n")
    print(f"{task} {arm} {'scoped' if scoped else 'old'}: {len(r.get('issues') or [])} issues, {row['wall']}s"
          + (f", error {r['error'][:80]}" if r.get("error") else ""), flush=True)


if __name__ == "__main__":
    jobs = [(d, a, s) for d in DIFFS for a in ARMS for s in (False, True)]
    print(f"{len(jobs)} reviews: {sum(a == 'opus' for _, a, _ in jobs)} Opus max (subscription), "
          f"{sum(a == 'astra' for _, a, _ in jobs)} Astra xhigh (SubRouter)", flush=True)
    if "--dry-run" in sys.argv:
        sys.exit()
    vid = subrouter.key("tazuna-vet-1", "gpt-6-astra", ["shared-openai-pro-50", "shared-openai-pro-52"], "SUBROUTER_VET1_KEY")
    k = subrouter.api(f"/api/token/{vid}")
    if not k.get("include_shared_subscriptions"):
        subrouter.api("/api/token/", "PUT", {**k, "include_shared_subscriptions": True})
    with ThreadPoolExecutor(len(jobs)) as ex:
        list(ex.map(one, jobs))
