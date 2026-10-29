#!/usr/bin/env python3
"""Each reviewer and review prompt on the same saved diff, the review only (`tazuna --review-only`), so what differs is
the reviewer, not the implementer's run. Diffs: the no-review arm of the 2026-10-29 reviewer runs (DeepSeek 1017, max).
Inside the bench jail, like the runs. Appends to replay-review.jsonl. The scoped arm needs tz-v102r (the
TAZUNA_REVIEW_SCOPED lever, dropped in 0.1.2).

  replay-review.py [--dry-run] [--set recall] [--scoped] [ARM ...]   # ARM from ARMS (default opus, astra); --scoped adds
                  # the held-to-task arm (tz-v102r only); --set recall: the diffs with known gaps and good ones below
"""
import json, os, re, subprocess, sys, tempfile, threading, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE)); sys.path.insert(0, str(HERE.parent))
import run, judge, subrouter

TZ = os.path.expanduser(os.environ.get("REPLAY_TZ", "~/bench-homes/tz-v102r"))
M = "deepseek-v4.1-flash-1017"
DIFFS = [("tasks", "caddy-quoted-braces", f"tazuna-ownsubx-v648@{M}:max-r0"),
         ("tasks-oct", "caddy-retry-match-expr", f"tazuna-ownsubx-v654@{M}:max-r0"),
         ("tasks", "terraform-import-foreach", f"tazuna-ownsubx-v651@{M}:max-r0")]
# Recall and precision: DeepSeek diffs from earlier no-review runs (not leaked), one per task. Gap: hidden tests failed
# (the largest partial pass per task); good: all hidden tests and the suite passed, judge 8 or more.
RECALL = [("tasks-oct", "rclone-archive-symlinks", f"tazuna-ownsubx-v348@{M}:max-r0"),
          ("tasks", "terraform-import-foreach", f"tazuna-ownsubx-v473@{M}:max-r0"),
          ("tasks-oct", "prometheus-exemplar-compaction", f"tazuna-ownsubx-v411@{M}:max-r0"),
          ("tasks-oct", "telegraf-snmp-reconnect", "tazuna-ownplan-v38@deepseek-v4.1-flash:max-r0"),
          ("tasks-oct", "caddy-unix-socket-close", f"tazuna-ownsubm-v331@{M}:max-r0"),
          ("tasks-hard", "caddy-tls-automate-names", f"tazuna-ownsubx-v477@{M}:max-r0"),
          ("tasks", "prometheus-duration-parens", f"tazuna-ownsubx-v479@{M}:max-r0"),
          ("tasks-oct", "caddy-unix-socket-close", f"tazuna-ownsubx-v403@{M}:max-r0"),
          ("tasks", "caddy-quoted-braces", f"tazuna-ownsubx-v492@{M}:max-r0"),
          ("tasks-oct", "prometheus-exemplar-compaction", f"tazuna-ownsubx-v394@{M}:max-r0"),
          ("tasks", "terraform-import-foreach", f"tazuna-ownsubx-v498@{M}:max-r0"),
          ("tasks-oct", "gitea-actions-cancelling", "tazuna-own-v38@deepseek-v4.1-flash:max-r0"),
          ("tasks-oct", "lazygit-mnemonic-prefix", f"tazuna-ownsubx-v409@{M}:max-r0")]
ARMS = {"opus": ["claude-opus-5-5", "max"], "astra": ["gpt-6-astra@https://subrouter.ai/v1", "xhigh"],
        "opus-high": ["claude-opus-5-5", "high"], "opus-medium": ["claude-opus-5-5", "medium"]}
OUT, lock = HERE / "replay-review.jsonl", threading.Lock()


def one(job):
    (bset, task, v), arm, scoped, dset = job
    diff = HERE / ("runs" if bset == "tasks" else f"runs-{bset}") / task / f"{v}.diff"
    w = Path(tempfile.mkdtemp(prefix="rw-replay-"))  # rw-: other jailed runs cannot read it
    side = Path(tempfile.mkdtemp(prefix="replay-side-"))  # the task file and claude's config, outside the work tree
    g = json.loads((HERE / bset / task / "git.json").read_text())  # setup_workdir without the module download (no build)
    subprocess.run(f"git -C '{g['repo']}' archive {g['base']} | tar -x -C '{w}'", shell=True, check=True)
    for c in (["init", "-q"], ["add", "-A"], [*run.GIT_ID, "commit", "-qm", "base"]):
        subprocess.run(["git", *c], cwd=w, check=True)
    a = subprocess.run(["git", "apply", "--whitespace=nowarn", str(diff)], cwd=w, capture_output=True, text=True)
    if a.returncode:  # a binary test fixture saved without its full index (rclone's .sqfs): the review cannot read it anyway
        skip = re.findall(r"cannot apply binary patch to '([^']+)'", a.stderr)
        subprocess.run(["git", "apply", "--whitespace=nowarn", *[f"--exclude={f}" for f in skip], str(diff)], cwd=w, check=True)
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
    row = dict(set=dset, bset=bset, task=task, diff=v, arm=arm, scoped=scoped, wall=round(time.time() - t), **r)
    with lock, OUT.open("a") as f:
        f.write(json.dumps(row) + "\n")
    print(f"{task} {v.split("@")[0][-5:]} {arm} {"scoped" if scoped else "old"}: {len(r.get('issues') or [])} issues, {row['wall']}s"
          + (f", error {r['error'][:80]}" if r.get("error") else ""), flush=True)


if __name__ == "__main__":
    arms = [a for a in sys.argv[1:] if a in ARMS] or ["opus", "astra"]
    dset = "recall" if "recall" in sys.argv else "first"
    jobs = [(d, a, s, dset) for d in (RECALL if dset == "recall" else DIFFS) for a in arms
            if os.environ.get("REPLAY_ONLY", "") in d[2]
            for s in ((False, True) if "--scoped" in sys.argv and not a.startswith("opus-") else (False,))]
    print(f"{len(jobs)} reviews: " + ", ".join(f"{sum(j[1] == a for j in jobs)} {a} {ARMS[a][1]}" for a in arms), flush=True)
    if "--dry-run" in sys.argv:
        sys.exit()
    vid = "astra" in arms and subrouter.key("tazuna-vet-1", "gpt-6-astra", ["shared-openai-pro-50", "shared-openai-pro-52"], "SUBROUTER_VET1_KEY")
    k = vid and subrouter.api(f"/api/token/{vid}")
    if k and not k.get("include_shared_subscriptions"):
        subrouter.api("/api/token/", "PUT", {**k, "include_shared_subscriptions": True})
    def safe(job):  # a failed job must not cancel the queued ones (Executor.map does on the first exception)
        try:
            one(job)
        except Exception as e:
            print(f"{job[0][1]} {job[0][2][:30]} {job[1]}: ERROR {e}", flush=True)
    done = {(r.get("set"), r["diff"], r["arm"], r["scoped"]) for r in map(json.loads, OUT.open())} if OUT.exists() else set()
    with ThreadPoolExecutor(14) as ex:
        list(ex.map(safe, [j for j in jobs if (j[3], j[0][2], j[1], j[2]) not in done]))
