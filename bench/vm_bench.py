#!/usr/bin/env python3
"""The bench's core on the InstaBox VM (Linux, no macOS jail), for runs that must not depend on the Mac being awake
(2026-11-01: GPT-6.1 Sol at xhigh while the user travelled). Runs on the VM from /data/home/bench:

  vm_bench.py run MODEL EFFORT PRICE JOB...   JOB = task:label; two at a time; tazuna's own loop on SubRouter with the
                                              key in .key (a capped key made for the VM); diffs, results and session
                                              files under out/
  vm_bench.py score                           hidden tests (uploaded only after every run is over, so no agent saw
                                              them) and the repo's suite, per finished run

Mirrors run.py: the task's prompt verbatim, the repo at its base commit as a fresh git repo, the task's setup command,
`tazuna --run` with the own loop's flags, `git diff` of everything the run changed, run.py's hidden_score.
"""
import json, os, shutil, subprocess, sys, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

B = Path("/data/home/bench")
TAZ = "/data/home/tazuna/target/release/tazuna"
OUT = B / "out"
ENV = {**os.environ, "GOMODCACHE": "/data/home/go/mod", "GOPATH": "/data/home/go", "HOME": str(B / "home"), "GOFLAGS": "-buildvcs=false"}


def sh(cmd, cwd, timeout=1800, env=None, shell=False):
    return subprocess.run(cmd, cwd=cwd, env=env or ENV, shell=shell, capture_output=True, text=True, timeout=timeout)


def one(model, effort, price, task, label):
    g = json.loads((B / "tasks" / task / "git.json").read_text())
    w = B / "w" / label
    shutil.rmtree(w, ignore_errors=True)
    w.mkdir(parents=True)
    sh(["tar", "xzf", str(B / "repos" / f"{task}.tgz"), "-C", str(w)], B)
    for c in (["git", "init", "-q"], ["git", "-c", "user.email=b@b", "-c", "user.name=bench", "add", "-A"],
              ["git", "-c", "user.email=b@b", "-c", "user.name=bench", "commit", "-qm", "base"]):
        sh(c, w)
    if g.get("setup"):
        sh(g["setup"], w, shell=True)
    t0 = time.time()
    env = {**ENV, "SUBROUTER_VM_KEY": (B / ".key").read_text().strip(), "TAZUNA_PRICE": price, "TAZUNA_MAX_USD": "0.5"}
    try:
        r = sh([TAZ, "--run", str(B / "tasks" / task / "prompt.txt"), "--data", "any", "--provider", "https://subrouter.ai/v1",
                "--key-env", "SUBROUTER_VM_KEY", "--no-escalate", "--model", model, "--effort", effort], w, timeout=3600, env=env)
        rc, stdout, stderr = r.returncode, r.stdout, r.stderr
    except subprocess.TimeoutExpired:
        rc, stdout, stderr = "timeout", "", ""
    secs = round(time.time() - t0, 1)
    sh(["git", "add", "-A"], w)
    (OUT / f"{label}.diff").write_text(sh(["git", "diff", "--cached", "HEAD"], w).stdout)
    res = next((json.loads(l) for l in reversed(stdout.splitlines()) if l.startswith("{")), {})
    row = dict(task=task, label=label, model=model, effort=effort, secs=secs, rc=rc, cost=res.get("total_cost_usd"),
               turns=res.get("num_turns"), usage=res.get("usage"), tail=stderr[-400:])
    (OUT / f"{label}.json").write_text(json.dumps(row))
    print(json.dumps({k: row[k] for k in ("label", "task", "secs", "rc", "cost", "turns")}), flush=True)


def score():
    for f in sorted(OUT.glob("*.json")):
        row = json.loads(f.read_text())
        if "passed" in row:
            continue
        task, w = row["task"], B / "w" / row["label"]
        g = json.loads((B / "tasks" / task / "git.json").read_text())
        hidden = B / "hidden" / task
        for pkg in {p.parent.relative_to(hidden) for p in hidden.rglob("*_test.go")}:  # agent test edits back to base
            for t in (w / pkg).glob("*_test.go"):
                rel = str(t.relative_to(w))
                if sh(["git", "cat-file", "-e", f"HEAD:{rel}"], w).returncode == 0:
                    sh(["git", "checkout", "HEAD", "--", rel], w)
                else:
                    t.unlink()
        for p in (x for x in hidden.rglob("*") if x.is_file()):
            dst = w / p.relative_to(hidden)
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(p, dst)
        r = sh(g["hidden_cmd"], w, shell=True, timeout=900)
        final = {}
        for line in r.stdout.splitlines():
            try:
                e = json.loads(line)
            except ValueError:
                continue
            if e.get("Test") and "/" not in e["Test"] and e.get("Action") in ("pass", "fail", "skip"):
                final[e["Test"]] = e["Action"]
        exp = B / "tasks" / task / "expected.json"
        total = max(len([a for a in final.values() if a != "skip"]), json.loads(exp.read_text())["hidden_total"] if exp.exists() else 1)
        suite = sh(g["suite_cmd"], w, shell=True, timeout=1200)
        row.update(passed=sum(a == "pass" for a in final.values()), total=total, suite_fail=int(suite.returncode != 0))
        f.write_text(json.dumps(row))
        print(f"{row['label']} {task}: {row['passed']}/{total} suite_fail={row['suite_fail']}", flush=True)


if __name__ == "__main__":
    a = sys.argv[1:]
    OUT.mkdir(parents=True, exist_ok=True)
    (B / "home").mkdir(exist_ok=True)
    if a[:1] == ["run"]:
        model, effort, price, jobs = a[1], a[2], a[3], [j.split(":") for j in a[4:]]
        with ThreadPoolExecutor(2) as ex:
            list(ex.map(lambda j: one(model, effort, price, j[0], j[1]), jobs))
        print("VM-RUNS-DONE", flush=True)
    elif a[:1] == ["score"]:
        score()
        print("VM-SCORE-DONE", flush=True)
    else:
        sys.exit(__doc__)
