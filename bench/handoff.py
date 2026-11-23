#!/usr/bin/env python3
"""Does a chat model hand off what it cannot know (TAZUNA_CHAT_HANDOFF, 2026-11-21)? Two parts, through tazuna headless
(`--run --profile chat`), so each model gets exactly what the chat path sends it:

  A  every message of suite/handoff.jsonl (27 that need live data or this computer, 15 that look like it and do not) and
     all 36 general-set tasks (none needs either), with the hand-off line: did it say [[agent]] when, and only when, it
     should?
  B  the 12 tasks of PART_B without the line: the other half of a side-by-side judge (handoff_judge.py) of whether the
     line makes ordinary answers worse.

  handoff.py A --models ds,ernie --runs 2       # results in handoff/results.jsonl; done items are skipped
  handoff.py summary

Opus runs on the Claude plan: start bench/guard.py first (its PAUSE file stops new calls here). Paid models stop at
--cap-usd of their own estimated cost (the balance is read before and after by hand).
"""
import argparse, collections, json, os, subprocess, tempfile, threading, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
OUT = HERE / "handoff" / "results.jsonl"
BIN = Path(os.environ.get("HANDOFF_BIN") or HERE.parent / "target" / "release" / "tazuna")  # HANDOFF_BIN: another build (the wording before)
GENERAL = HERE / "realwork" / "tasks-general"
PART_B = ["info-canberra", "info-photosynthesis", "guide-running-plan", "guide-budget", "write-email-decline", "write-summary",
          "math-loan", "math-dice", "law-gdpr-cvs", "arch-shortener", "science-molarity", "formula-weighted"]
SUB = ["--provider", "https://subrouter.ai/v1", "--key-env", "SUBROUTER_API_KEY", "--data", "any", "--no-escalate"]
# prices: SubRouter's list (model_ratio x 0.2267 per million, as tazuna's picker shows DeepSeek 1017 at $0.017 / $0.068)
MODELS = {
    "opus": ["--model", "claude-opus-5-5", "--effort", "medium"],
    "ds": ["--model", "deepseek-v4.1-flash-1017", "--effort", "high", *SUB, "--price", "0.017,0.00034,0.068"],
    "sol": ["--model", "gpt-6.2-sol", "--effort", "xhigh", *SUB, "--price", "0.227,0.0113,1.133"],
    # on Mortera (the user, 2026-11-21: "run gpt 6.2 sol and ernie at mortera"); Mortera lists no gpt-6.2-sol, so 6.1 Sol;
    # prices from its list in rupiah at tazuna's 16,300 to the dollar
    "sol61m": ["--model", "gpt-6.1-sol", "--effort", "xhigh", "--provider", "https://mortera.cloud/v1", "--key-env",
               "MORTERA_API_KEY", "--data", "any", "--no-escalate", "--price", "0.121,0.0121,0.604"],
    "erniem": ["--model", "ernie-5-5-flash-preview", "--provider", "https://mortera.cloud/v1", "--key-env", "MORTERA_API_KEY",
               "--data", "any", "--no-escalate", "--price", "0,0,0"],
    "ernie": ["--model", "baidu/ernie-5.5-flash-preview:free", "--provider", "https://openrouter.ai/api/v1", "--key-env",
              "OPENROUTER_API_KEY", "--data", "any", "--no-escalate", "--price", "0,0,0"],
}
lock = threading.Lock()


def items(part):
    if part == "B":
        return [{"id": f"general:{t}", "text": (GENERAL / t / "prompt.txt").read_text(), "want": "chat", "kind": "general"} for t in PART_B]
    out = [json.loads(l) for l in (HERE / "suite" / "handoff.jsonl").read_text().splitlines() if l.strip()]
    out += [{"id": f"general:{d.name}", "text": (d / "prompt.txt").read_text(), "want": "chat", "kind": "general"}
            for d in sorted(GENERAL.iterdir()) if (d / "prompt.txt").exists()]
    return out


def done():
    if not OUT.exists():
        return set()
    return {(r["model"], r["arm"], r["id"], r["run"]) for r in map(json.loads, OUT.read_text().splitlines()) if r.get("got") != "error"}


def subrouter_key():
    for l in (Path.home() / ".subrouter-key").read_text().splitlines():
        if l.startswith("export SUBROUTER_API_KEY="):
            return l.split("=", 1)[1].strip().strip('"')


def one(model, arm, it, run):
    if model == "opus":
        while (HERE / "PAUSE").exists():
            time.sleep(30)
        if (HERE / "ABORT").exists():
            return None
    env = {k: v for k, v in os.environ.items() if k != "TAZUNA_CHAT_HANDOFF"}
    # since the line is on by default, the arm without it switches it off; "line" and "line2" were the two wordings
    # measured before the second became the only one (the switch's 1 and 2 now both mean on)
    env["TAZUNA_CHAT_HANDOFF"] = "1" if arm in ("line", "line2", "line3") else "0"
    if model in ("ds", "sol"):
        env["SUBROUTER_API_KEY"] = subrouter_key()
    with tempfile.TemporaryDirectory() as cwd:
        pf = Path(cwd) / "prompt.txt"
        pf.write_text(it["text"])
        t = time.time()
        p = subprocess.run([str(BIN), "--run", str(pf), "--allow-non-git", "--profile", "chat", *MODELS[model]], cwd=cwd, env=env,
                           capture_output=True, text=True, timeout=900)
    try:
        d = json.loads(p.stdout.strip().splitlines()[-1])
    except (IndexError, ValueError):
        d = {"is_error": True, "result": (p.stderr or p.stdout)[-300:]}
    text = d.get("result") or ""
    got = "error" if d.get("is_error") or p.returncode else ("agent" if "[[agent]]" in text[:200] else "chat")
    r = {"model": model, "arm": arm, "id": it["id"], "run": run, "want": it["want"], "kind": it["kind"], "got": got,
         "clean": text.strip() == "[[agent]]", "cost": d.get("total_cost_usd") or 0, "secs": round(time.time() - t, 1),
         "out_tokens": (d.get("usage") or {}).get("output_tokens"), "result": text}
    with lock:
        with OUT.open("a") as f:
            f.write(json.dumps(r) + "\n")
    return r


def run(part, models, runs, workers, cap):
    OUT.parent.mkdir(exist_ok=True)
    arm = {"A": "line", "A2": "line2", "A3": "line3"}.get(part, "none")
    have = done()
    for m in models:
        todo = [(it, i) for i in range(runs) for it in items(part) if (m, arm, it["id"], i) not in have]
        spent, n = 0.0, 0
        print(f"{m} {arm}: {len(todo)} calls", flush=True)
        with ThreadPoolExecutor(workers.get(m, 4)) as ex:
            futs = []
            for it, i in todo:
                futs.append(ex.submit(one, m, arm, it, i))
            for f in futs:
                r = f.result()
                if not r:
                    continue
                n += 1
                spent += r["cost"]
                if n % 20 == 0:
                    print(f"  {m}: {n}/{len(todo)}, ${spent:.4f}", flush=True)
                if spent > cap:
                    print(f"  {m}: cap ${cap} passed, stopping", flush=True)
                    for g in futs:
                        g.cancel()
                    break
        print(f"{m} {arm}: done {n}, est. ${spent:.4f}", flush=True)


def summary():
    rows = [json.loads(l) for l in OUT.read_text().splitlines()]
    by = collections.defaultdict(list)
    for r in rows:
        if r["arm"] in ("line", "line2", "line3"):
            by[f'{r["model"]} {r["arm"]}'].append(r)
    for m, rs in by.items():
        ok = [r for r in rs if r["got"] != "error"]
        live = [r for r in ok if r["want"] == "agent"]
        stay = [r for r in ok if r["want"] == "chat"]
        miss = [r for r in live if r["got"] != "agent"]
        wrong = [r for r in stay if r["got"] == "agent"]
        tricky = [r for r in stay if r["kind"] == "tricky"]
        print(f"{m}: {len(ok)} answered ({len(rs) - len(ok)} errors); live: {len(live) - len(miss)}/{len(live)} handed off; "
              f"should stay: {len(wrong)}/{len(stay)} handed off wrongly ({sum(r['got'] == 'agent' for r in tricky)}/{len(tricky)} "
              f"of the look-alikes); clean marker {sum(r['clean'] for r in ok if r['got'] == 'agent')}/{sum(r['got'] == 'agent' for r in ok)}; "
              f"est. ${sum(r['cost'] for r in rs):.4f}")
        for r in miss:
            print(f"   missed {r['id']} r{r['run']}: {r['result'][:90]!r}")
        for r in wrong:
            print(f"   wrong  {r['id']} r{r['run']}")


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("part", choices=["A", "A2", "A3", "B", "summary"])
    ap.add_argument("--models", default="ds,ernie,sol,opus")
    ap.add_argument("--runs", type=int, default=2)
    ap.add_argument("--cap-usd", type=float, default=1.0)
    a = ap.parse_args()
    if a.part == "summary":
        summary()
    else:
        run(a.part, a.models.split(","), a.runs, {"opus": 3, "ernie": 2, "ds": 6, "sol": 4, "sol61m": 4, "erniem": 4}, a.cap_usd)
