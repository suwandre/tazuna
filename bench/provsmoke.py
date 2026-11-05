#!/usr/bin/env python3
"""Smoke test of SubRouter providers for one model (provsmoke.py MODEL SLUG...), 2026-11-04: per provider a key of its
own (tazuna-route-smoke-<slug>), one agent-like conversation of four growing streamed calls (~15k tokens of tazuna's
source as context, a bash tool, effort xhigh), all providers at once. Per call: time to first token (reasoning counts),
output tokens a second after it, total seconds, cached share, output and reasoning tokens, tool calls. One JSON line per
provider on stdout. Not a quality test: the real-task check is one caddy run per provider (provsmoke.sh)."""
import json, os, re, statistics as st, sys, threading, time, urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import subrouter as s  # noqa: E402

M, SLUGS = sys.argv[1], sys.argv[2:]
SRC = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src")
CONTEXT = open(os.path.join(SRC, "engine.rs")).read()[:60000]
TOOL = {"type": "function", "function": {"name": "bash", "description": "Run a shell command in the repo.",
        "parameters": {"type": "object", "properties": {"command": {"type": "string"}}, "required": ["command"]}}}
ASKS = ["Where does the loop decide whether earlier reasoning goes back to the provider? Name the function, then run one "
        "bash command that would confirm it.",
        "What happens to the history when a route switch happens? Answer in three sentences.",
        "Find one edge case in how long bash output is shortened. Use bash to look if you need to.",
        "Summarise what you found in two sentences."]


def var(slug):
    return "SUBROUTER_SMOKE_" + re.sub(r"[^A-Z0-9]+", "_", slug.upper()).strip("_") + "_KEY"


def call(key, msgs):
    body = {"model": M, "messages": msgs, "tools": [TOOL], "reasoning_effort": "xhigh", "max_tokens": 6000,
            "stream": True, "stream_options": {"include_usage": True}}
    q = urllib.request.Request("https://subrouter.ai/v1/chat/completions", data=json.dumps(body).encode(),
                               headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "curl/8"})
    t0, first, usage, text, tools = time.time(), None, {}, "", 0
    try:
        with urllib.request.urlopen(q, timeout=300) as r:
            for raw in r:
                line = raw.decode(errors="replace").strip()
                if not line.startswith("data:") or line == "data: [DONE]":
                    continue
                d = json.loads(line[5:])
                usage = d.get("usage") or usage
                for c in d.get("choices") or []:
                    delta = c.get("delta") or {}
                    if first is None and (delta.get("content") or delta.get("reasoning_content") or delta.get("reasoning") or delta.get("tool_calls")):
                        first = time.time()
                    text += delta.get("content") or ""
                    tools += sum(1 for t in delta.get("tool_calls") or [] if (t.get("function") or {}).get("name"))
    except Exception as e:  # noqa: BLE001
        return dict(error=repr(e)[:120], secs=round(time.time() - t0, 1))
    end = time.time()
    out = usage.get("completion_tokens") or 0
    det = usage.get("completion_tokens_details") or {}
    cached = (usage.get("prompt_tokens_details") or {}).get("cached_tokens") or usage.get("prompt_cache_hit_tokens") or 0
    return dict(ttft=round(first - t0, 2) if first else None, tps=round(out / (end - first), 1) if first and end > first else None,
                secs=round(end - t0, 1), prompt=usage.get("prompt_tokens"), cached=cached, out=out,
                reasoning=det.get("reasoning_tokens"), tools=tools, text=text[:200])


def provider(slug, res):
    key = os.environ[var(slug)]
    msgs = [{"role": "system", "content": "You are a coding agent in the tazuna repo. src/engine.rs:\n" + CONTEXT}]
    rows = []
    for ask in ASKS:
        msgs.append({"role": "user", "content": ask})
        r = call(key, msgs)
        rows.append(r)
        if r.get("error"):
            time.sleep(3)
            continue
        msgs.append({"role": "assistant", "content": r["text"] or "ok"})
    ok = [r for r in rows if not r.get("error")]
    med = lambda k: round(st.median([r[k] for r in ok if r.get(k) is not None]), 2) if any(r.get(k) is not None for r in ok) else None
    warm = ok[1:]
    res[slug] = dict(at=time.strftime("%Y-%m-%dT%H:%MZ", time.gmtime()), slug=slug, ok=len(ok), errors=[r["error"] for r in rows if r.get("error")][:2], ttft=med("ttft"), tps=med("tps"),
                     secs=med("secs"), total=round(sum(r["secs"] for r in rows), 1),
                     cached=round(100 * sum(r["cached"] for r in warm) / max(1, sum(r["prompt"] or 0 for r in warm))),
                     out=sum(r["out"] for r in ok), reasoning=sum(r["reasoning"] or 0 for r in ok), tools=sum(r["tools"] for r in ok))
    print(json.dumps(res[slug]), flush=True)


res = {}
for g in SLUGS:  # one at a time: each rewrites ~/.subrouter-key
    s.key(f"tazuna-route-smoke-{g}", M, [g], var(g))
th = [threading.Thread(target=provider, args=(g, res)) for g in SLUGS]
[t.start() for t in th]
[t.join() for t in th]
