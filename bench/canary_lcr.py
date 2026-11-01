#!/usr/bin/env python3
"""Does a model on a SubRouter route reason over long documents like its public score says? Artificial Analysis' long
context benchmark (AA-LCR v1.1: 100 questions over document sets of ~100k tokens, Apache-2.0, Hugging Face) asked
through the route, graded the way AA grades it (their equality-checker prompts, a GPT Luna checker at medium), compared
with AA's own LCR score for the model (bench/.model-check/aa.json, from model_check.py). Long inputs are where a cheap
route can cut corners (a shortened context, a smaller quantization), which the short canary.py set cannot see.
95% interval on 100 questions: about +-9 points.

  canary_lcr.py fetch                                   questions and documents into bench/.model-check/aa-lcr/
  canary_lcr.py run MODEL --key-env VAR [--effort E]    every question through the route in VAR (default effort max);
                                                        the checker runs on the key in SUBROUTER_LCR_JUDGE_KEY
  canary_lcr.py show                                    accuracy per model, route and effort next to AA's score
"""
import csv, io, json, os, re, sys, threading, time, urllib.request, zipfile
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from canary import ask, wilson  # noqa: E402

DATA, OUT, lock = HERE / ".model-check" / "aa-lcr", HERE / ".model-check" / "lcr.jsonl", threading.Lock()
HF = "https://huggingface.co/datasets/ArtificialAnalysis/AA-LCR/resolve/main/"
JUDGE_MODEL = "gpt-6-luna"
# AA-LCR's dataset card, version 1.1 (the question template and the equality checker's prompts)
PROMPT = """BEGIN INPUT DOCUMENTS

{documents_text}

END INPUT DOCUMENTS

Answer the following question using the input documents provided above.

START QUESTION

{question}

END QUESTION
"""
JUDGE_SYSTEM = """Decide whether the CANDIDATE ANSWER is correct or incorrect against the OFFICIAL ANSWER.
Note the following points when assessing correctness:

- Numbers should still match when they are the same value written differently, e.g., a
  percentage, a count of percentage points, and the equivalent decimal fraction are the same
  value: 0.675, "67.5%" and "67.5 percentage points" all match. So do different scales
  (thousand, million, bn) and different notations (thousands separators, currency symbols,
  LaTeX markup, and numbers written as words).
- Where the question asks for a particular format (e.g., a percentage, a number of decimal
  places, a unit, a rounding, or an ordering) the CANDIDATE ANSWER must meet it. If the
  question asks for no particular format, accept any equivalent form.
- In cases where the question asks for an ordered list, a title, honorific or article added
  to an entry in the CANDIDATE ANSWER can change where that entry sorts. Accept the ordering
  if it is correct either with those additions or without them.
- Grade the value the CANDIDATE ANSWER finally commits to, and it must commit to one. Values
  reached while working, and alternatives it considers and sets aside, do not count. If it
  offers several values without selecting one, it is incorrect even if one of them is right.
  Hedging is fine as long as one clearly definitive answer is given."""
JUDGE_USER = """Assess whether the following CANDIDATE ANSWER is CORRECT or INCORRECT.
For the CANDIDATE ANSWER to be correct, it must be consistent with the OFFICIAL ANSWER.

The question, for reference only: START QUESTION {question}

END QUESTION

The OFFICIAL ANSWER: {official_answer}

END OFFICIAL ANSWER

BEGIN CANDIDATE ANSWER TO ASSESS

{candidate_answer}

END CANDIDATE ANSWER TO ASSESS

Reply as JSON, with a verdict of CORRECT or INCORRECT."""


def get(url):
    with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "tazuna-bench/1"}), timeout=300) as r:
        return r.read()


def fetch():
    DATA.mkdir(parents=True, exist_ok=True)
    (DATA / "AA-LCR_Dataset.csv").write_bytes(get(HF + "AA-LCR_Dataset.csv"))
    with zipfile.ZipFile(io.BytesIO(get(HF + "extracted_text/AA-LCR_extracted-text.zip"))) as z:
        for n in z.namelist():  # lcr/<category>/<set>/<file>; nothing outside DATA
            p = (DATA / n).resolve()
            if n.endswith("/") or not str(p).startswith(str(DATA.resolve())):
                continue
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(z.read(n))
    print(f"{DATA}: {len(questions())} questions")


def questions():
    return list(csv.DictReader(open(DATA / "AA-LCR_Dataset.csv", encoding="utf-8")))


def prompt(q):
    folder = DATA / "lcr" / q["document_category"] / q["document_set_id"]
    docs = [(folder / f).read_text(encoding="utf-8") for f in q["data_source_filenames"].split(";")]
    text = "\n\n".join(f"BEGIN DOCUMENT {i + 1}:\n{d}\nEND DOCUMENT {i + 1}" for i, d in enumerate(docs))
    return PROMPT.format(documents_text=text, question=q["question"])


def judge(q, answer, key):
    body = {"model": JUDGE_MODEL, "reasoning_effort": "medium", "messages": [
        {"role": "system", "content": JUDGE_SYSTEM},
        {"role": "user", "content": JUDGE_USER.format(question=q["question"], official_answer=q["answer"], candidate_answer=answer)}]}
    req = urllib.request.Request("https://subrouter.ai/v1/chat/completions", data=json.dumps(body).encode(),
                                 headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "curl/8"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=300) as r:
                text = json.load(r)["choices"][0]["message"]["content"] or ""
            m = re.search(r"\b(INCORRECT|CORRECT)\b", text.upper())
            return m.group(1) if m else None
        except Exception:
            time.sleep(5 * (attempt + 1))
    return None


def run(model, key_env, effort, n=100):
    key, jkey, day = os.environ[key_env], os.environ["SUBROUTER_LCR_JUDGE_KEY"], time.strftime("%Y-%m-%d")

    def one(q):
        t = time.time()
        try:
            text, usage = ask(model, key, effort, prompt(q))
        except Exception as e:
            text, usage = f"[error: {e}]", {}
        # the checker grades the final answer; a model's visible text is that answer (its reasoning is not in content)
        verdict = None if text.startswith("[error") or not text.strip() else judge(q, text[-4000:], jkey)
        row = dict(date=day, model=model, route=key_env, effort=effort, id=q["question_id"], ok=verdict == "CORRECT",
                   verdict=verdict, secs=round(time.time() - t, 1), in_tokens=usage.get("prompt_tokens"),
                   cached=(usage.get("prompt_tokens_details") or {}).get("cached_tokens"), out_tokens=usage.get("completion_tokens"),
                   error=text[:300] if text.startswith("[error") else None)
        with lock, OUT.open("a") as f:
            f.write(json.dumps(row) + "\n")
        return row

    with ThreadPoolExecutor(8) as ex:
        done = list(ex.map(one, questions()[:n]))
    show(done)
    return done


def aa_scores(metric="lcr"):
    f = HERE / ".model-check" / "aa.json"
    rows = json.load(open(f))["data"] if f.exists() else []
    return {m["slug"]: (m.get("evaluations") or {}).get(metric) for m in rows}


def show(rows, metric="lcr"):
    aa = aa_scores(metric)
    by = defaultdict(list)
    for r in rows:
        by[(r["date"], r["model"], r["route"], r["effort"])].append(r)
    for (day, model, route, effort), rs in sorted(by.items()):
        k, n = sum(r["ok"] for r in rs), len(rs)
        lo, hi = wilson(k, n)
        errs = sum(bool(r.get("error")) or r.get("verdict") is None for r in rs)
        base = model.replace(".", "-")
        ref = aa.get(f"{base}-{effort}") or (aa.get(base) if effort == "max" else None)
        print(f"{day} {model:26} {route:26} {effort:6} {k:3}/{n:<3} {100 * k / n:5.1f}% (95%: {100 * lo:.0f}-{100 * hi:.0f}%)"
              + (f"  AA {100 * ref:.1f}%" if ref else "  AA -") + (f"  {errs} unanswered or ungraded" if errs else ""))


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[:1] == ["fetch"]:
        fetch()
    elif a[:1] == ["run"] and len(a) >= 2 and "--key-env" in a:
        run(a[1], a[a.index("--key-env") + 1], a[a.index("--effort") + 1] if "--effort" in a else "max")
    elif a[:1] == ["show"]:
        show([json.loads(l) for l in OUT.open()] if OUT.exists() else [])
    else:
        sys.exit(__doc__)
