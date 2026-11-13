#!/usr/bin/env python3
"""Blind LLM judge for saved run diffs (runs/<task>/<harness>-r<i>.diff).

  ./judge.py                 # judge every task in runs/ (skips tasks already in judge.jsonl)
  ./judge.py --summary

Per task, all candidate diffs are anonymized, shuffled and scored in one call against the
reference fix. One pass by default (screening); --passes 2 (two orders, averaged against position bias) for results a
default decision rests on. --dry-run prints how many judge calls a round would make, without making any.
"""
import argparse, json, os, random, re, statistics, subprocess, sys, tempfile, time
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


# Tasks answered in words (bench/suite: review, question, investigate, plan): the answer is graded against the task's
# reference.md (known defects, the checked answer, the root cause), not a patch against a reference fix.
SYSTEM_ANSWER = """You are a strict senior engineer grading anonymous answers to one request about a codebase (a review, a question,
an investigation or a plan). The reference states what is known to be true: the defects a review should find, the
checked answer, the real root cause, or the maintainers' chosen approach. It may not list everything that is true.
Do not reward length or confident tone. A confident wrong claim is worse than saying it is unsure. For a review,
an invented blocker (a problem that is not real) counts against the answer; real issues beyond the reference count for it.
For a follow-up (the user's second message is given), grade the last reply: true, consistent with the first reply or
plainly correcting it, and doing what the second message asks.

Score each dimension 1-5:
- correctness: what it states is true of the code
- coverage: it finds or covers what the reference says matters
- precision: no invented problems, no wrong claims
- clarity: easy to act on (file:line where it matters, blockers apart from nits)
- fit: answers what was asked, at the depth asked
Then overall 1-10.

Reply with only a JSON object: {"<candidate id>": {"correctness": n, "coverage": n, "precision": n, "clarity": n, "fit": n, "overall": n, "note": "<one sentence>"}, ...}"""
DIMS_ANSWER = ["correctness", "coverage", "precision", "clarity", "fit"]


# the general set (tasks-general, 2026-11-13): math, science, law, writing, formulas, software architecture; no codebase
SYSTEM_GENERAL = """You are a strict expert grading anonymous answers to one request outside coding (math, science, law,
writing, spreadsheet formulas or software architecture). The reference gives the correct result or the points a good
answer covers, and any constraints the request set. Do not reward length or confident tone; a wrong final answer or a
broken constraint (length, form) costs heavily, and a confident wrong claim is worse than saying it is unsure.

Score each dimension 1-5:
- correctness: what it states is true; the final answer or result is right
- coverage: it covers what the reference says matters
- precision: no wrong claims, no padding
- clarity: easy to follow and use
- fit: answers what was asked, in the form and length asked
Then overall 1-10.

Reply with only a JSON object: {"<candidate id>": {"correctness": n, "coverage": n, "precision": n, "clarity": n, "fit": n, "overall": n, "note": "<one sentence>"}, ...}"""


def general_kind(task):
    m = task / "meta.json"
    return m.exists() and json.loads(m.read_text()).get("kind") == "general"


def answer_kind(task):
    m = task / "meta.json"
    # a follow-up whose second turn asks for a change has hidden tests or an end-state check: graded as code, not as an answer
    return m.exists() and json.loads(m.read_text()).get("kind") in ("review", "question", "investigate", "plan", "followup", "general") \
        and not json.loads((task / "git.json").read_text()).get("hidden_cmd")


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


def ask_judge(prompt, model, effort, system=None):
    if "@" in model:
        return verdict_json(ask_api(prompt, model, effort, system))
    guard_allows()
    with tempfile.TemporaryDirectory() as cfg, tempfile.TemporaryDirectory() as cwd:
        # the prompt on stdin: as an argument, a task with long diffs passed Linux's 128 KB limit for one argument
        # ("Argument list too long", hugo on the bench VM, 2026-11-09); macOS has no such limit per argument
        cmd = ["claude", "-p", "--model", model, "--effort", effort, "--output-format", "json",
               "--no-session-persistence", "--setting-sources", "", "--strict-mcp-config",
               "--disable-slash-commands", "--exclude-dynamic-system-prompt-sections", "--tools", "",
               "--system-prompt", system or (SYSTEM_NOREF if NOREF else SYSTEM)]
        out = subprocess.run(cmd, cwd=cwd, env={**os.environ, "CLAUDE_CONFIG_DIR": cfg},
                             input=prompt, capture_output=True, text=True, timeout=1800).stdout
    d = json.loads(out.strip().splitlines()[-1])
    if d.get("is_error"):
        raise RuntimeError(d.get("result"))
    return verdict_json(d["result"])


def verdict_json(text):
    """The first whole JSON object in the reply: Fable 5.2 wrote text after it, and "first { to last }" took that in
    ("Extra data", the call lost, 2026-11-11). A reply that still does not parse is kept in judge-raw.txt."""
    i = text.find("{")
    try:
        if i < 0:
            raise ValueError("no JSON object")
        return json.JSONDecoder().raw_decode(text[i:])[0]
    except ValueError as e:
        with open(HERE / "judge-raw.txt", "a") as f:
            f.write(f"--- {time.strftime('%Y-%m-%d %H:%M:%S')} ({e})\n{text}\n")
        raise RuntimeError(f"judge reply did not parse ({e}); kept in judge-raw.txt: {text[:200]}")


def ask_api(prompt, model, effort, system=None):
    """--model MODEL@URL: one streamed call on the provider's Responses route, key from the variable JUDGE_KEY_ENV names.
    A judge from another family than Opus, for results where Opus is also a contestant (2026-10-29: Opus as reviewer)."""
    import urllib.request
    m, url = model.split("@", 1)
    body = {"model": m, "instructions": system or (SYSTEM_NOREF if NOREF else SYSTEM), "input": prompt,
            "reasoning": {"effort": effort}, "store": False, "stream": True}
    q = urllib.request.Request(url.rstrip("/") + "/responses", data=json.dumps(body).encode(), headers={
        "Authorization": f"Bearer {os.environ[os.environ['JUDGE_KEY_ENV']]}", "Content-Type": "application/json",
        "Accept": "text/event-stream", "User-Agent": "tazuna-bench/1"})
    for attempt in range(3):  # a shared pool's stream sometimes ends without its last event
        done, text = None, []
        try:
            with urllib.request.urlopen(q, timeout=1800) as r:
                for line in r:
                    if line.startswith(b"data: {"):
                        e = json.loads(line[6:])
                        if e.get("type") == "response.output_text.delta":
                            text.append(e.get("delta", ""))
                        elif e.get("type") in ("response.completed", "response.failed", "response.incomplete"):
                            done = e["response"]
        except OSError as e:
            print(f"judge stream broke ({e}), attempt {attempt + 1}", file=sys.stderr, flush=True)
        if done and done.get("status") == "completed":
            return "".join(c.get("text", "") for o in done["output"] if o.get("type") == "message" for c in o.get("content", []))
        if not done and "".join(text).rstrip().endswith("}"):  # the answer streamed in full; only the closing event is missing
            return "".join(text)
        print(f"judge call ended {done and done.get('status')}: {done and done.get('error')}, attempt {attempt + 1}", file=sys.stderr, flush=True)
    raise RuntimeError("judge call failed three times")


def candidates(task_name):
    """The finished runs of a task that JUDGE_ONLY selects (a run without a result row was cancelled or killed)."""
    sys.path.insert(0, str(HERE))  # python3 -I leaves the script's directory off the path
    from leakscan import leaked
    bad = leaked()  # runs that saw the reference fix are not judged
    # leakscan names a harness and task, not the run: JUDGE_SKIP_RUNS (<harness>-rN, comma-separated) names the runs that
    # leaked, and that pair's other runs are judged after all (tazuna v1142's k6 r1 read the hidden tests, 2026-11-11)
    skip = set(filter(None, os.environ.get("JUDGE_SKIP_RUNS", "").split(",")))
    lifted = {x.rsplit("-r", 1)[0] for x in skip}
    results = {(r["harness"], r["run"]): r for r in map(json.loads, RESULTS.read_text().splitlines())
               if r["task"] == task_name and ((task_name, r["harness"]) not in bad or r["harness"] in lifted)
               and f'{r["harness"]}-r{r["run"]}' not in skip}
    cands = []
    only = set(filter(None, os.environ.get("JUDGE_ONLY", "").split(",")))
    for f in sorted((RUNS / task_name).glob("*.diff")):
        stem = f.stem.replace("~", ":")  # run.py fname: ':' is '~' in file names
        h, run = stem.rsplit("-r", 1)
        if only and h not in only and stem not in only:  # a harness, or one run of it (<harness>-rN)
            continue
        r = results.get((h, int(run)))
        if r:
            final = f.with_suffix(".final.txt")
            cands.append(dict(harness=h, run=int(run), diff=f.read_text(), r=r, final=final.read_text() if final.exists() else ""))
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
        if (task / "followup.txt").exists():  # a follow-up task: the user's second message, sent after the first reply
            parts.append(f"# The user's second message, after the first reply\n{(task / 'followup.txt').read_text()}")
        if answer_kind(task):
            parts.append(f"# Reference\n{(task / 'reference.md').read_text()}")
            for cid, i in ids.items():
                c = cands[i]
                ans = c["final"] if len(c["final"]) <= MAX_DIFF else c["final"][:MAX_DIFF] + "\n[answer truncated]"
                parts.append(f"# Candidate {cid}\n{ans or '(no answer)'}" + (f"\n\n(it also changed files:)\n```diff\n{c['diff'][:4000]}\n```" if c["diff"].strip() else ""))
            verdict = ask_judge("\n\n".join(parts), model, effort, SYSTEM_GENERAL if general_kind(task) else SYSTEM_ANSWER)
            for cid, i in ids.items():
                if cid in verdict:
                    scores[i].append(verdict[cid])
            continue
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
        avg = {k: statistics.mean(s[k] for s in scores[i]) for k in (DIMS_ANSWER if answer_kind(task) else DIMS) + ["overall"] if all(k in s for s in scores[i])}
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
                print(f"{g:<10} {h:<8} {len(rs):>3} " + " ".join(f"{statistics.mean(r.get(d, float('nan')) for r in rs):>8.2f}" for d in DIMS)
                      + f" {statistics.mean(r['overall'] for r in rs):>8.2f}")


def main():
    ap = argparse.ArgumentParser()
    # Fable 5.2 at high since 2026-11-11 (the user's choice); Opus 5.5 xhigh judged every earlier round. Scores are compared
    # within a round (each has its baseline arm), not across the change. Fable has a weekly cap of its own that the plan
    # meter (guard.py) does not show: a call that hits it fails, it does not spill into extra usage
    ap.add_argument("--model", default="claude-fable-5-2")
    ap.add_argument("--effort", default="high")
    ap.add_argument("--passes", type=int, default=1)
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--summary", action="store_true")
    ap.add_argument("--rejudge", action="store_true")
    a = ap.parse_args()
    if a.summary:
        return summarize()
    if "@" not in a.model and not os.environ.get("CLAUDE_CODE_OAUTH_TOKEN"):
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
