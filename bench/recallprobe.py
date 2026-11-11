#!/usr/bin/env python3
"""Does a provider pass a model's earlier reasoning back to it? (recallprobe.py MODEL SLUG [SLUG ...], 2026-11-11)

The own loop sends the model's reasoning back with the history only where a provider uses it (engine.rs reasoning_back):
on 2026-10-27/28 no SubRouter provider of DeepSeek did, so the model worked a design out again on later calls. This plants
a code word in the reasoning only, then asks for it with the reasoning sent back (as `reasoning_content`) and without (the
control). Recalled with it and not without: the provider passes it on. One provider at a time, through the key
tazuna-vet-9 limited to that provider (parallel key writes lost updates before). Three short calls per provider.
"""
import json, os, re, sys, time, urllib.request
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import subrouter as s

M, slugs = sys.argv[1], sys.argv[2:]
VAR = "SUBROUTER_VET9_KEY"


def post(key, msgs):
    body = {"model": M, "messages": msgs, "reasoning_effort": "high", "max_tokens": 3000}
    q = urllib.request.Request("https://subrouter.ai/v1/chat/completions", data=json.dumps(body).encode(),
                               headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "curl/8"})
    try:
        with urllib.request.urlopen(q, timeout=180) as r:
            m = json.load(r)["choices"][0]["message"]
            return (m.get("content") or "").strip(), m.get("reasoning_content") or m.get("reasoning") or ""
    except Exception as e:
        return f"ERROR {e}"[:120], ""


ASK = [{"role": "system", "content": "You are a careful assistant."},
       {"role": "user", "content": "In your private reasoning, write one line `CODE=` followed by a random made-up 7-letter word in "
                                   "capital letters (not a real word; DeepSeek otherwise only plans to invent one). Never write it in "
                                   "your reply. Reply with exactly: ready"}]
AGAIN = {"role": "user", "content": "What was the code word you invented? Reply with the word only, or UNKNOWN if you cannot see it."}

for slug in slugs:
    s.key("tazuna-vet-9", M, [slug], VAR)
    key = os.environ.get(VAR) or dict(l.strip().split("=", 1) for l in open(os.path.expanduser("~/.subrouter-key")) if "=" in l).get(VAR, "")
    time.sleep(2)
    reply, reasoning = post(key, ASK)
    planted = set(re.findall(r"\b[A-Z]{7}\b", reasoning)) - {"UNKNOWN"}
    if not reasoning or not planted:
        print(f"{slug:10} no code word in the reasoning (reply {reply[:30]!r}, reasoning {len(reasoning)} chars)", flush=True)
        continue
    with_r, _ = post(key, ASK + [{"role": "assistant", "content": reply or "ready", "reasoning_content": reasoning}, AGAIN])
    without, _ = post(key, ASK + [{"role": "assistant", "content": reply or "ready"}, AGAIN])
    hit = lambda a: any(w in a.upper() for w in planted)
    print(f"{slug:10} planted {sorted(planted)[:3]}  with reasoning sent back: {with_r[:20]!r} {'RECALLED' if hit(with_r) else 'not recalled'}"
          f"  control: {without[:20]!r} {'recalled?!' if hit(without) else 'not recalled'}", flush=True)
