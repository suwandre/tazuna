#!/usr/bin/env python3
"""A fixed small benchmark per model and provider, run on a schedule: does a model on a route answer like its public
scores say, and does that change over time (a provider serving a weaker model or cutting its reasoning, a model drifting)?

The set (canary/set-v1.jsonl, fixed once): AIME 2025's 30 problems (integer answers) and 100 MMLU-Pro questions (seed 1:
ten blocks of ten at random places), graded exactly. One run: every item once, no tools, the answer in the last "Answer:" line;
results per item in .model-check/canary.jsonl (scoreboard.py reads them). 95% Wilson interval on each accuracy: 30 items
is about +-17 points, 100 about +-9, so it catches a weaker model or cut reasoning, not a point or two.

  canary.py fetch                                  build the set (once; public data from Hugging Face)
  canary.py run MODEL --key-env VAR [--effort E]   SubRouter's Chat route with the key in VAR (default effort max)
  canary.py show [MODEL]                           accuracy per model, route, set and date
"""
import json, math, os, random, re, sys, threading, time, urllib.parse, urllib.request
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
SET, OUT, lock = HERE / "canary" / "set-v1.jsonl", HERE / ".model-check" / "canary.jsonl", threading.Lock()
ROWS = "https://datasets-server.huggingface.co/rows?dataset={}&config=default&split={}&offset={}&length={}"


def rows(dataset, split, offset, length):
    q = urllib.request.Request(ROWS.format(urllib.parse.quote(dataset, safe=""), split, offset, length), headers={"User-Agent": "tazuna-bench/1"})
    for attempt in range(5):
        try:
            with urllib.request.urlopen(q, timeout=60) as r:
                return [x["row"] for x in json.load(r)["rows"]]
        except OSError:  # 429 when asked too fast (one row a request was, 2026-10-31)
            time.sleep(30 * (attempt + 1))
    raise RuntimeError(f"{dataset} rows at {offset}: no answer after 5 tries")


def fetch():
    items = [dict(id=f"aime25-{r['problem_idx']}", set="aime25", question=r["problem"], answer=str(int(r["answer"])))
             for r in rows("MathArena/aime_2025", "train", 0, 30)]
    rng = random.Random(1)
    blocks = [r for start in sorted(rng.sample(range(0, 12032 - 10), 10)) for r in rows("TIGER-Lab/MMLU-Pro", "test", start, 10)]
    for r in blocks:
        opts = r["options"] if isinstance(r["options"], list) else json.loads(r["options"].replace("'", '"'))
        letters = "ABCDEFGHIJ"[: len(opts)]
        q = r["question"] + "\n\n" + "\n".join(f"{l}. {o}" for l, o in zip(letters, opts))
        items.append(dict(id=f"mmlupro-{r['question_id']}", set="mmlupro", category=r["category"], question=q, answer=r["answer"]))
    SET.parent.mkdir(exist_ok=True)
    SET.write_text("".join(json.dumps(i) + "\n" for i in items))
    print(f"{SET}: {len(items)} items ({sum(i['set'] == 'aime25' for i in items)} AIME 2025, {sum(i['set'] == 'mmlupro' for i in items)} MMLU-Pro)")


def prompt(item):
    if item["set"] == "aime25":
        return item["question"] + "\n\nSolve it. End with one line: Answer: <the integer>"
    return item["question"] + "\n\nChoose one option. End with one line: Answer: <the letter>"


def grade(item, text):
    m = re.findall(r"Answer:\s*\**\s*([A-J]|-?\d+)", text or "")
    got = m[-1] if m else None
    if got and item["set"] == "aime25":
        return got.lstrip("0") or "0" == item["answer"], got
    return got == item["answer"], got


def ask(model, key, effort, text):
    body = {"model": model, "messages": [{"role": "user", "content": text}], "max_tokens": 32000, "stream": False}
    if effort:
        body["reasoning_effort"] = effort
    q = urllib.request.Request("https://subrouter.ai/v1/chat/completions", data=json.dumps(body).encode(),
                               headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "curl/8"})
    err = None
    for attempt in range(3):
        try:
            with urllib.request.urlopen(q, timeout=900) as r:
                d = json.load(r)
            return d["choices"][0]["message"].get("content") or "", d.get("usage") or {}
        except Exception as e:
            err = e
            time.sleep(5 * (attempt + 1))
    raise RuntimeError(f"no answer after 3 tries: {err}")


def wilson(k, n, z=1.96):
    if not n:
        return 0.0, 0.0
    p = k / n
    c = (p + z * z / (2 * n)) / (1 + z * z / n)
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / (1 + z * z / n)
    return c - h, c + h


def run(model, key_env, effort):
    items = [json.loads(l) for l in SET.open()]
    key, day = os.environ[key_env], time.strftime("%Y-%m-%d")

    def one(item):
        t = time.time()
        try:
            text, usage = ask(model, key, effort, prompt(item))
        except Exception as e:
            text, usage = f"[error: {e}]", {}
        ok, got = grade(item, text)
        row = dict(date=day, model=model, route=key_env, effort=effort, set=item["set"], id=item["id"], ok=ok, got=got,
                   answer=item["answer"], secs=round(time.time() - t, 1), out_tokens=usage.get("completion_tokens"),
                   error=text if text.startswith("[error") else None)
        with lock, OUT.open("a") as f:
            f.write(json.dumps(row) + "\n")
        return row

    OUT.parent.mkdir(exist_ok=True)
    with ThreadPoolExecutor(8) as ex:
        done = list(ex.map(one, items))
    summary(done)


def summary(rows):
    by = defaultdict(list)
    for r in rows:
        by[(r["date"], r["model"], r["route"], r.get("effort"), r["set"])].append(r)
    for (day, model, route, effort, s), rs in sorted(by.items()):
        k, n = sum(r["ok"] for r in rs), len(rs)
        lo, hi = wilson(k, n)
        errs = sum(bool(r.get("error")) for r in rs)
        print(f"{day} {model:28} {route:22} {effort or '-':6} {s:8} {k:3}/{n:<3} {100 * k / n:5.1f}%  (95%: {100 * lo:.0f}-{100 * hi:.0f}%)"
              + (f"  {errs} errors" if errs else ""))


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[:1] == ["fetch"]:
        fetch()
    elif a[:1] == ["run"] and len(a) >= 2 and "--key-env" in a:
        run(a[1], a[a.index("--key-env") + 1], a[a.index("--effort") + 1] if "--effort" in a else "max")
    elif a[:1] == ["show"]:
        rows_ = [json.loads(l) for l in OUT.open()] if OUT.exists() else []
        summary([r for r in rows_ if len(a) < 2 or a[1] in r["model"]])
    else:
        sys.exit(__doc__)
