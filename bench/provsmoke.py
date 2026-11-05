#!/usr/bin/env python3
"""Smoke test of SubRouter providers for one model (provsmoke.py MODEL SLUG...), 2026-11-04: per provider a key of its
own (tazuna-route-smoke-<slug>), one agent-like conversation of four growing streamed calls (~15k tokens of tazuna's
source as context, a bash tool, effort high or PROVSMOKE_EFFORT: DeepSeek V4 has only high and max, and providers map other values their own way), all providers at once. Per call: time to first token (reasoning counts),
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
    # one key per model and provider: keys named by provider alone were re-pointed from DeepSeek to Luna to Sol by the
    # next probe, so the DeepSeek gauge's keys asked for Sol and got 403 (2026-11-05)
    return "SUBROUTER_SMOKE_" + re.sub(r"[^A-Z0-9]+", "_", (M + "_" + slug).upper()).strip("_") + "_KEY"

def short_name(name):
    """SubRouter caps key names ("Token name is too long" at 53 characters): a long one keeps its start and a hash."""
    import hashlib
    return name if len(name) <= 40 else name[:33] + "-" + hashlib.md5(name.encode()).hexdigest()[:6]



def call_responses(key, msgs):
    """The Responses route (PROVSMOKE_API=responses), the one tazuna uses for GPT models: a provider can serve it and
    refuse the same model's Chat route with tools (xcgx with Luna, 2026-11-05)."""
    tool = {"type": "function", "name": "bash", "description": TOOL["function"]["description"], "parameters": TOOL["function"]["parameters"]}
    body = {"model": M, "instructions": msgs[0]["content"], "input": msgs[1:], "tools": [tool], "store": False, "stream": True,
            "reasoning": {"effort": os.environ.get("PROVSMOKE_EFFORT", "high")}, "max_output_tokens": 6000}
    q = urllib.request.Request("https://subrouter.ai/v1/responses", data=json.dumps(body).encode(),
                               headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "curl/8", "Accept": "text/event-stream"})
    t0, first, usage, text, tools = time.time(), None, {}, "", 0
    try:
        with urllib.request.urlopen(q, timeout=300) as r:
            for raw in r:
                line = raw.decode(errors="replace").strip()
                if not line.startswith("data: {"):
                    continue
                e = json.loads(line[6:])
                kind = e.get("type", "")
                if first is None and kind.endswith(".delta"):
                    first = time.time()
                if kind == "response.output_text.delta":
                    text += e.get("delta", "")
                if kind == "response.output_item.added" and (e.get("item") or {}).get("type") == "function_call":
                    tools += 1
                if kind in ("response.completed", "response.incomplete"):
                    usage = (e.get("response") or {}).get("usage") or {}
                if kind == "response.failed":
                    return dict(error=str((e.get("response") or {}).get("error"))[:120], secs=round(time.time() - t0, 1))
    except Exception as e:  # noqa: BLE001
        return dict(error=repr(e)[:120], secs=round(time.time() - t0, 1))
    end = time.time()
    out = usage.get("output_tokens") or 0
    return dict(ttft=round(first - t0, 2) if first else None, tps=round(out / (end - first), 1) if first and end > first else None,
                secs=round(end - t0, 1), prompt=usage.get("input_tokens"), cached=(usage.get("input_tokens_details") or {}).get("cached_tokens") or 0,
                out=out, reasoning=(usage.get("output_tokens_details") or {}).get("reasoning_tokens"), tools=tools, text=text[:200])


def call(key, msgs):
    if os.environ.get("PROVSMOKE_API") == "responses":
        return call_responses(key, msgs)
    body = {"model": M, "messages": msgs, "tools": [TOOL], "reasoning_effort": os.environ.get("PROVSMOKE_EFFORT", "high"), "max_tokens": 6000,
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


LINES = [f"Line {i}: Die Quellcode-Überprüfung #{i * 7919} prüft {i * 104729} Zeilen, émojis 🙂🚀, 中文字符 {i}, snake_case_{i}, CamelCase{i}.\n"
         for i in range(10)]


def deep(key):
    """PROVSMOKE_DEEP=1 (the gauge, 2026-11-05): checks that catch a provider swapping in a cheaper model or cutting the
    thinking at some hours. Tokens a line of distinct text (2 and 10 lines, the smaller of two counts each: a model's
    tokenizer, whatever prompt a provider adds; fxpayback's "Sol" counted 58 where Sol counts 43) and one fixed hard
    AIME 2025 problem at the probe's effort: its reasoning length and whether it is right."""
    import canary

    def tokens_in(text):
        body = {"model": M, "messages": [{"role": "user", "content": text}], "max_tokens": 16, "reasoning_effort": "low"}
        q = urllib.request.Request("https://subrouter.ai/v1/chat/completions", data=json.dumps(body).encode(),
                                   headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "curl/8"})
        with urllib.request.urlopen(q, timeout=180) as r:
            return json.load(r)["usage"]["prompt_tokens"]

    out = {}
    try:
        two, ten = min(tokens_in("".join(LINES[:2])) for _ in range(2)), min(tokens_in("".join(LINES)) for _ in range(2))
        out.update(tokens_a_line=round((ten - two) / 8, 1), hidden_prompt=round(two - (ten - two) / 4))
    except Exception as e:  # noqa: BLE001
        out["tokenizer_error"] = repr(e)[:80]
    item = [json.loads(l) for l in canary.SET.open() if '"aime25"' in l][27]  # late and hard, as in effortprobe.py
    try:
        text, usage = canary.ask(M, key, os.environ.get("PROVSMOKE_EFFORT", "high"), item["question"] + "\n\nSolve it. End with one line: Answer: <the integer>")
        out.update(depth_out=usage.get("completion_tokens"), depth_reasoning=(usage.get("completion_tokens_details") or {}).get("reasoning_tokens"),
                   depth_right=canary.grade(item, text)[0])
    except Exception as e:  # noqa: BLE001
        out["depth_error"] = str(e)[:80]
    return out


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
    if os.environ.get("PROVSMOKE_DEEP"):
        res[slug].update(deep(key))
    print(json.dumps(res[slug]), flush=True)


res = {}
for g in SLUGS:  # one at a time: each rewrites ~/.subrouter-key
    kid = s.key(short_name(f"tazuna-route-sm-{re.sub(r'[^a-z0-9]', '', M)[:10]}-{g}"), M, [g], var(g))  # SubRouter caps key names
    k = s.api(f"/api/token/{kid}")
    if bool(k.get("include_shared_subscriptions")) != g.startswith("shared-"):  # a shared pool serves only keys that allow them
        s.api("/api/token/", "PUT", {**k, "include_shared_subscriptions": g.startswith("shared-")})
th = [threading.Thread(target=provider, args=(g, res)) for g in SLUGS]
[t.start() for t in th]
[t.join() for t in th]
