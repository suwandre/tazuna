#!/usr/bin/env python3
"""Blind LLM judge for saved run diffs (runs/<task>/<harness>-r<i>.diff).

  ./judge.py                 # judge every task in runs/ (skips tasks already in judge.jsonl)
  ./judge.py --summary

Per task, all candidate diffs are anonymized, shuffled and scored in one call against the
reference fix. One pass by default (screening); --passes 2 (two orders, averaged against position bias) for results a
default decision rests on. --dry-run prints how many judge calls a round would make, without making any.
"""
import argparse, json, os, random, re, statistics, subprocess, sys, tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SET = os.environ.get("BENCH_SET", "tasks")
RUNS, TASKS = HERE / ("runs" if SET == "tasks" else f"runs-{SET}"), HERE / SET
RESULTS = HERE / ("results.jsonl" if SET == "tasks" else f"results-{SET}.jsonl")
JUDGE = Path(os.environ.get("JUDGE_OUT") or HERE / "judge.jsonl")  # a side pool (JUDGE_ONLY) gets its own file
DIMS = ["correctness", "edge_cases", "code_quality", "scope", "tests"]
MAX_DIFF = 30000

SYSTEM = """You are a strict senior code reviewer grading anonymous candidate patches for one task.
Grade each candidate independently against the task (a real GitHub issue) and the reference fix (the merged PR's non-test changes). The reference is one valid solution, not the only one.
Ignore formatting nits. Do not reward length. Penalize unrelated changes, debugging leftovers, and edits to files the task did not need.
Hidden test results are given as facts; still judge the code yourself (a passing patch can be fragile or hacky, a failing one can be close).

Score each dimension 1-5:
- correctness: fixes the actual root cause / implements the spec correctly
- edge_cases: handles the edge cases the task implies
- code_quality: readable, idiomatic, fits the existing code
- scope: minimal and focused; no unrelated changes
- tests: quality of tests the candidate added or updated (if it added none, give 1-2; if tests were clearly unnecessary, 3)
Then overall 1-10.

Reply with only a JSON object: {"<candidate id>": {"correctness": n, "edge_cases": n, "code_quality": n, "scope": n, "tests": n, "overall": n, "note": "<one sentence>"}, ...}"""


NOREF = bool(os.environ.get("JUDGE_NOREF"))
SYSTEM_NOREF = SYSTEM.replace(" and the reference fix (the merged PR's non-test changes). The reference is one valid solution, not the only one.", ". There is no reference solution: judge from the task, the code and the test facts.")
assert SYSTEM_NOREF != SYSTEM


def reference(task):
    if (task / "solution.patch").exists():
        return (task / "solution.patch").read_text()
    return subprocess.run(["diff", "-ruN", "repo", "solution"], cwd=task, capture_output=True, text=True).stdout


def guard_allows():
    """No judge call unless the usage guard is running (fresh state) and says go: an unguarded judge round once
    spent extra usage after the guard had stopped (2026-10-12)."""
    import time
    g = Path(__file__).resolve().parent.parent
    try:
        fresh = time.time() - json.loads((g / "guard.state.json").read_text())["at"] < 600
    except (OSError, ValueError, KeyError):
        fresh = False
    if not fresh or (g / "ABORT").exists() or (g / "PAUSE").exists():
        raise RuntimeError("usage guard not running or says stop (ABORT/PAUSE): no judge call")


def ask_judge(prompt, model, effort):
    guard_allows()
    with tempfile.TemporaryDirectory() as cfg, tempfile.TemporaryDirectory() as cwd:
        cmd = ["claude", "-p", prompt, "--model", model, "--effort", effort, "--output-format", "json",
               "--no-session-persistence", "--setting-sources", "", "--strict-mcp-config",
               "--disable-slash-commands", "--exclude-dynamic-system-prompt-sections", "--tools", "",
               "--system-prompt", SYSTEM_NOREF if NOREF else SYSTEM]
        out = subprocess.run(cmd, cwd=cwd, env={**os.environ, "CLAUDE_CONFIG_DIR": cfg},
                             capture_output=True, text=True, timeout=1800).stdout
    d = json.loads(out.strip().splitlines()[-1])
    if d.get("is_error"):
        raise RuntimeError(d.get("result"))
    m = re.search(r"\{.*\}", d["result"], re.S)
    if not m:
        raise RuntimeError("judge returned no JSON: " + d["result"][:300])
    return json.loads(m.group(0))


def candidates(task_name):
    """The finished runs of a task that JUDGE_ONLY selects (a run without a result row was cancelled or killed)."""
    sys.path.insert(0, str(HERE))  # python3 -I leaves the script's directory off the path
    from leakscan import leaked
    bad = leaked()  # runs that saw the reference fix are not judged
    results = {(r["harness"], r["run"]): r for r in map(json.loads, RESULTS.read_text().splitlines())
               if r["task"] == task_name and (task_name, r["harness"]) not in bad}
    cands = []
    only = set(filter(None, os.environ.get("JUDGE_ONLY", "").split(",")))
    for f in sorted((RUNS / task_name).glob("*.diff")):
        h, run = f.stem.rsplit("-r", 1)
        if only and h not in only and f.stem not in only:  # a harness, or one run of it (<harness>-rN)
            continue
        r = results.get((h, int(run)))
        if r:
            cands.append(dict(harness=h, run=int(run), diff=f.read_text(), r=r))
    return cands


def judge_task(task_name, model, effort, passes):
    task = TASKS / task_name
    cands = candidates(task_name)
    if not cands:
        print(f"{task_name}: no candidates, skipping (no judge call)", flush=True)
        return []
    scores = {i: [] for i in range(len(cands))}
    for p in range(passes):
        order = list(range(len(cands)))
        random.Random(f"{task_name}-{p}").shuffle(order)
        ids = {f"C{k + 1}": i for k, i in enumerate(order)}
        parts = [f"# Task\n{(task / 'prompt.txt').read_text()}"]
        if not NOREF:  # JUDGE_NOREF=1: no reference fix, to measure how much the judge anchors on it
            parts.append(f"# Reference fix\n```diff\n{reference(task)[:MAX_DIFF]}\n```")
        for cid, i in ids.items():
            c = cands[i]
            diff = c["diff"] if len(c["diff"]) <= MAX_DIFF else c["diff"][:MAX_DIFF] + "\n[diff truncated]"
            reg = c["r"].get("regress")
            facts = f"hidden PR tests {c['r']['passed']}/{c['r']['total']}" + ("" if reg is None else (", package suite FAILED" if reg else ", package suite passed"))
            parts.append(f"# Candidate {cid} ({facts})\n```diff\n{diff or '(no changes)'}\n```")
        verdict = ask_judge("\n\n".join(parts), model, effort)
        for cid, i in ids.items():
            if cid in verdict:
                scores[i].append(verdict[cid])
    rows = []
    for i, c in enumerate(cands):
        if not scores[i]:
            continue
        avg = {k: statistics.mean(s[k] for s in scores[i]) for k in DIMS + ["overall"]}
        rows.append(dict(task=task_name, harness=c["harness"], run=c["run"], judge=model, effort=effort,
                         passes=len(scores[i]), notes=[s.get("note", "") for s in scores[i]], **avg))
    return rows


def summarize():
    rows = [json.loads(l) for l in JUDGE.read_text().splitlines()]
    tier = lambda r: r["task"].split("-")[0].upper()
    print(f"{'group':<10} {'harness':<8} {'n':>3} " + " ".join(f"{d[:8]:>8}" for d in DIMS) + f" {'overall':>8}")
    for g in sorted({tier(r) for r in rows}) + ["ALL"]:
        for h in sorted({r["harness"] for r in rows}):
            rs = [r for r in rows if r["harness"] == h and (g == "ALL" or tier(r) == g)]
            if rs:
                print(f"{g:<10} {h:<8} {len(rs):>3} " + " ".join(f"{statistics.mean(r[d] for r in rs):>8.2f}" for d in DIMS)
                      + f" {statistics.mean(r['overall'] for r in rs):>8.2f}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default="claude-opus-5-5")
    ap.add_argument("--effort", default="xhigh")
    ap.add_argument("--passes", type=int, default=1)
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--summary", action="store_true")
    ap.add_argument("--rejudge", action="store_true")
    a = ap.parse_args()
    if a.summary:
        return summarize()
    if not os.environ.get("CLAUDE_CODE_OAUTH_TOKEN"):
        sys.exit("CLAUDE_CODE_OAUTH_TOKEN not set")
    done = set() if a.rejudge or not JUDGE.exists() else {json.loads(l)["task"] for l in JUDGE.read_text().splitlines()}
    import threading
    from concurrent.futures import ThreadPoolExecutor
    lock = threading.Lock()

    def one(t):
        try:
            rows = judge_task(t, a.model, a.effort, a.passes)
        except Exception as e:
            print(f"{t}: ERROR {e}", file=sys.stderr, flush=True)
            return
        with lock, JUDGE.open("a") as f:
            f.writelines(json.dumps(r) + "\n" for r in rows)
        print(f"{t}: judged {len(rows)} candidates", flush=True)

    tasks = set(filter(None, os.environ.get("JUDGE_TASKS", "").split(",")))
    todo = [t for t in sorted(p.name for p in RUNS.iterdir() if p.is_dir()) if t not in done and (not tasks or t in tasks)]
    todo = [t for t in todo if candidates(t)]
    print(f"{len(todo)} tasks x {a.passes} passes = {len(todo) * a.passes} judge calls ({a.model} {a.effort})", flush=True)
    if a.dry_run:
        return
    with ThreadPoolExecutor(6) as ex:
        list(ex.map(one, todo))
    summarize()


if __name__ == "__main__":
    main()
