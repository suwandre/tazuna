#!/usr/bin/env python3
"""Humanity's Last Exam through a SubRouter route, next to Artificial Analysis' HLE score for the model: does the route
reason like the model's public number says? A fixed 400 of HLE's text-only questions (seed 1; HLE is gated on Hugging
Face: ~/.hf-token, and it asks not to be republished, so the set stays in bench/.model-check/, outside git), HLE's own
answer format and judge prompt, a GPT Luna judge at medium. 95% interval on 400 questions: about +-5 points, enough to
see a route 10% under the public score.

  canary_hle.py fetch                                   the set (once)
  canary_hle.py run MODEL --key-env VAR [--effort E] [--n N]   the first N (400) through the route in VAR (default effort
                                                        max), skipping ones already answered; the judge runs on
                                                        the key in SUBROUTER_LCR_JUDGE_KEY
  canary_hle.py show                                    accuracy per model, route and effort next to AA's HLE
"""
import json, os, random, re, sys, threading, time, urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from canary import ask  # noqa: E402
from canary_lcr import JUDGE_MODEL, show  # noqa: E402

SET, OUT, lock = HERE / ".model-check" / "hle" / "set-400.jsonl", HERE / ".model-check" / "hle.jsonl", threading.Lock()
ROWS = "https://datasets-server.huggingface.co/rows?dataset=cais%2Fhle&config=default&split=test&offset={}&length=100"
# HLE's evaluation code (github.com/centerforaisafety/hle): the answer formats and the judge prompt
SYSTEM_EXACT = ("Your response should be in the following format:\nExplanation: {your explanation for your final answer}\n"
                "Exact Answer: {your succinct, final answer}\nConfidence: {your confidence score between 0% and 100% for your answer}")
SYSTEM_MC = ("Your response should be in the following format:\nExplanation: {your explanation for your answer choice}\n"
             "Answer: {your chosen answer}\nConfidence: {your confidence score between 0% and 100% for your answer}")
JUDGE = """Judge whether the following [response] to [question] is correct or not based on the precise and unambiguous [correct_answer] below.

[question]: {question}

[response]: {response}

Your judgement must be in the format and criteria specified below:

extracted_final_answer: The final exact answer extracted from the [response]. Put the extracted answer as 'None' if there is no exact, final answer to extract from the response.

[correct_answer]: {correct_answer}

reasoning: Explain why the extracted_final_answer is correct or incorrect based on [correct_answer], focusing only on if there are meaningful differences between [correct_answer] and the extracted_final_answer. Do not comment on any background to the problem, do not attempt to solve the problem, do not argue for any answer different than [correct_answer], focus only on whether the answers match.

correct: Answer 'yes' if extracted_final_answer matches the [correct_answer] given above, or is within a small margin of error for numerical problems. Answer 'no' otherwise, i.e. if there if there is any inconsistency, ambiguity, non-equivalency, or if the extracted answer is incorrect.


confidence: The extracted confidence score between 0|\\%| and 100|\\%| from [response]. Put 100 if there is no confidence score available."""


def fetch():
    token = os.environ["HF_TOKEN"]  # sent only to huggingface.co
    rows = []
    for off in range(0, 2500, 100):
        q = urllib.request.Request(ROWS.format(off), headers={"Authorization": f"Bearer {token}", "User-Agent": "tazuna-bench/1"})
        for attempt in range(5):
            try:
                with urllib.request.urlopen(q, timeout=60) as r:
                    rows += [x["row"] for x in json.load(r)["rows"]]
                break
            except OSError:
                time.sleep(30 * (attempt + 1))
        else:
            sys.exit(f"rows at {off}: no answer after 5 tries")
    text = [r for r in rows if not r.get("image")]
    pick = random.Random(1).sample(text, 400)
    SET.parent.mkdir(parents=True, exist_ok=True)
    SET.write_text("".join(json.dumps({k: r[k] for k in ("id", "question", "answer", "answer_type", "category")}) + "\n" for r in pick))
    print(f"{SET}: 400 of {len(text)} text-only questions ({len(rows)} in HLE)")


def chat(model, key, system, user, effort):
    body = {"model": model, "reasoning_effort": effort, "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}]}
    req = urllib.request.Request("https://subrouter.ai/v1/chat/completions", data=json.dumps(body).encode(),
                                 headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "curl/8"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=300) as r:
                return json.load(r)["choices"][0]["message"]["content"] or ""
        except Exception:
            time.sleep(5 * (attempt + 1))
    return None


def run(model, key_env, effort, n=400):
    key, jkey, day = os.environ[key_env], os.environ["SUBROUTER_LCR_JUDGE_KEY"], time.strftime("%Y-%m-%d")
    # the first N of the fixed 400 (DeepSeek at max thinks ~43k tokens a question: 400 cost ~$1.50, 2026-11-01); questions
    # this model, route and effort already answered are not asked again
    seen = {(r["model"], r["route"], r["effort"], r["id"]) for r in map(json.loads, OUT.open())} if OUT.exists() else set()
    items = [q for q in map(json.loads, SET.open())][:n]
    items = [q for q in items if (model, key_env, effort, q["id"]) not in seen]
    print(f"{len(items)} questions to ask", flush=True)

    def one(q):
        t = time.time()
        system = SYSTEM_MC if q["answer_type"] == "multipleChoice" else SYSTEM_EXACT
        try:  # canary.ask streams (long reasoning outlives an unstreamed call); the format goes in front of the question
            # 32k output was not enough: DeepSeek at max thought through all of it and answered nothing (2026-11-01)
            text, usage = ask(model, key, effort, system + "\n\n" + q["question"], max_tokens=128000)
        except Exception as e:
            text, usage = f"[error: {e}]", {}
        verdict = None
        if text.strip() and not text.startswith("[error"):
            j = chat(JUDGE_MODEL, jkey, "", JUDGE.format(question=q["question"], response=text[-6000:], correct_answer=q["answer"]), "medium")
            m = re.search(r"correct:\s*\**\s*(yes|no)", (j or "").lower())
            verdict = ("CORRECT" if m.group(1) == "yes" else "INCORRECT") if m else None
        row = dict(date=day, model=model, route=key_env, effort=effort, id=q["id"], ok=verdict == "CORRECT", verdict=verdict,
                   secs=round(time.time() - t, 1), out_tokens=usage.get("completion_tokens"),
                   error=text[:300] if text.startswith("[error") else None)
        with lock, OUT.open("a") as f:
            f.write(json.dumps(row) + "\n")
        return row

    with ThreadPoolExecutor(8) as ex:
        done = list(ex.map(one, items))
    show(done, "hle")


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[:1] == ["fetch"]:
        fetch()
    elif a[:1] == ["run"] and len(a) >= 2 and "--key-env" in a:
        run(a[1], a[a.index("--key-env") + 1], a[a.index("--effort") + 1] if "--effort" in a else "max",
            int(a[a.index("--n") + 1]) if "--n" in a else 400)
    elif a[:1] == ["show"]:
        show([json.loads(l) for l in OUT.open()] if OUT.exists() else [], "hle")
    else:
        sys.exit(__doc__)
