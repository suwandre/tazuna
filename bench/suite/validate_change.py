#!/usr/bin/env python3
"""Keeps a change task (build_change.py) only if some hidden test fails on the base and passes with the PR's fix; writes
those tests to the task's expected.json. Runs on the bench VM (Node 22.23+, corepack): validate_change.py TASK_DIR..."""
import json, os, shlex, shutil, subprocess, sys, tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HOME = Path.home()


def results(w, hidden_cmd):
    cmd = hidden_cmd.split("nodetest.py", 1)[1].split()
    cmd = [c for i, c in enumerate(cmd) if c != "--only" and (i == 0 or cmd[i - 1] != "--only")]
    out = subprocess.run(["python3", str(HOME / "tazuna/bench/suite/nodetest.py"), *cmd], cwd=w, capture_output=True, text=True).stdout
    return {e["Test"]: e["Action"] for e in map(json.loads, out.splitlines())}


def check(task):
    task = Path(task).resolve()
    g = json.loads((task / "git.json").read_text())
    w = Path(tempfile.mkdtemp(prefix=f"val-{task.name}-"))
    try:
        subprocess.run(f"git -C {HOME}/bench-repos/astro archive {g['base']} | tar -x -C {w}", shell=True, check=True)
        r = subprocess.run(g["setup"], shell=True, cwd=w, capture_output=True, text=True)
        if r.returncode:
            return task.name, f"setup failed: {r.stderr[-300:]}"
        for f in (p for p in (task / "hidden").rglob("*") if p.is_file()):
            dst = w / f.relative_to(task / "hidden"); dst.parent.mkdir(parents=True, exist_ok=True); shutil.copy(f, dst)
        before = results(w, g["hidden_cmd"])
        subprocess.run(["git", "apply", str(task / "solution.patch")], cwd=w, check=True)
        after = results(w, g["hidden_cmd"])
        fp = sorted(t for t, a in after.items() if a == "pass" and before.get(t) == "fail")
        broke = sorted(t for t, a in after.items() if a == "fail")
        (task / "expected.json").write_text(json.dumps(dict(hidden_total=len(fp), tests=fp), indent=2) + "\n")
        return task.name, f"{len(fp)} fail->pass of {len(after)} tests; failing with the fix: {len(broke)}"
    finally:
        shutil.rmtree(w, ignore_errors=True)


if __name__ == "__main__":
    with ThreadPoolExecutor(4) as ex:
        for name, msg in ex.map(check, sys.argv[1:]):
            print(name, msg, flush=True)
