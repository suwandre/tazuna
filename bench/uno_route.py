#!/usr/bin/env python3
"""UnoRouter: pick a group for a model, or none, before a paid run.

    UNOROUTER_API_KEY=... ./uno_route.py MODEL [groups to try, default 4]
    ./uno_route.py MODEL --price GROUP [--budget USD]   # right before a run, and during it


For the cheapest groups by estimated run cost, in order, until one passes:
1. price: input, cache read and output from UnoRouter's live list (`/api/pricing`), checked against OpenRouter's public
   list as an independent reference (UnoRouter's own "-90%" was once computed from a $24 output price that no one charges);
2. cache: a conversation grown over 6 calls the way an agent grows it; at least 50% of each previous call cached (below
   that the group keeps switching upstream accounts; above it the price at the measured rate decides, reported with it);
3. long reasoning with tools: a counting question that takes a minute of thinking must come back with an answer
   (groups that cut such calls left Luna and Sol unable to finish hard steps);
4. identity: that answer must be in English and near the right number (a "Haiku" group answered in Chinese and scored 1-3).
Prints PIN <group> for the first group that passes. Prices move between the check and the run (a Grok group went from
$0.024 to $0.10 per M input within minutes), so `--price` reads the group's price again: it prints the
`TAZUNA_PRICE=... TAZUNA_MAX_USD=...` to start the run with (the cap at 5x the estimated run, at most the budget), and exits 1
when the group no longer serves the model, 2 when the estimated run costs more than the budget. Costs a cent or two per group tried; uses the key's own access with an
`X-Group` header per group. A pass is no guarantee (a7-5053 passed a cache probe, then missed most of a run): the run
script still stops a run whose cache rate falls.
"""
import json, os, random, re, sys, time, urllib.error, urllib.request

API = "https://api.unorouter.com"
KEY = os.environ.get("UNOROUTER_API_KEY", "")
TEXT = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src", "engine.rs")).read()
# a typical rclone run: input sent over all calls, share cached when the cache works, output
RUN_IN, RUN_HIT, RUN_OUT = 6e6, 0.95, 80e3
QUESTION, ANSWER = "How many primes p < 20000 satisfy p ≡ 3 (mod 8)? Work it out carefully, then give only the number on the last line.", 571


def get(url, body=None, group=None, timeout=600):
    h = {"Content-Type": "application/json"}
    if KEY and url.startswith(API + "/v1"):
        # only to UnoRouter: OpenRouter answers its model list in another shape when it sees anthropic-version
        h.update({"Authorization": f"Bearer {KEY}", "anthropic-version": "2023-06-01"})
    if group:
        h["X-Group"] = group
    r = urllib.request.Request(url, data=json.dumps(body).encode() if body else None, headers=h)
    return json.load(urllib.request.urlopen(r, timeout=timeout))


def norm(name):
    """'claude-haiku-5-5' and 'anthropic/claude-haiku-5.5' -> 'claudehaiku55'."""
    return re.sub(r"[^a-z0-9]", "", name.split("/")[-1].lower())


def run_cost(price, hit=RUN_HIT):
    """USD for a typical run at (input, cached, output) per M; a miss is billed as a cache write, 1.25x input."""
    i, c, o = price
    return (RUN_IN * ((1 - hit) * i * 1.25 + hit * c) + RUN_OUT * o) / 1e6


def groups(model):
    """[(group, (input, cached, output) USD per M)] for every group serving the model, by estimated run cost."""
    d = get(API + "/api/pricing")
    m = next(m for m in d["data"] if m["model_name"] == model)
    base, out_x, cache_x = m["model_ratio"] * 2, m["completion_ratio"], m.get("cache_ratio") or 1
    gs = [(g, (base * r, base * r * cache_x, base * r * out_x)) for g, r in d["group_ratio"].items() if g in m["enable_groups"]]
    return sorted(gs, key=lambda g: run_cost(g[1]))


def reference(model):
    """OpenRouter's (input, cached, output) USD per M for the model, or None."""
    for m in get("https://openrouter.ai/api/v1/models")["data"]:
        if norm(m["id"]) == norm(model):
            p = m["pricing"]
            i = float(p["prompt"]) * 1e6
            return i, float(p.get("input_cache_read") or p["prompt"]) * 1e6, float(p["completion"]) * 1e6


def call(model, group, msgs, effort, tools=False, max_out=64):
    """One unstreamed call in the API tazuna uses for the model; returns (input, cached, text)."""
    if model.startswith("gpt-"):
        body = {"model": model, "input": msgs, "store": False, "reasoning": {"effort": effort}, "prompt_cache_key": "tz-route"}
        if max_out:
            body["max_output_tokens"] = max_out
        if tools:
            body["tools"] = [{"type": "function", "name": "bash", "description": "run a command", "parameters": {"type": "object", "properties": {"command": {"type": "string"}}}}]
            body["instructions"] = "Answer from your own reasoning; do not call tools for this."
            body["tool_choice"] = "none"  # tools present (they change how some models reason), but the answer must be text
        r = get(API + "/v1/responses", body, group)
        u = r.get("usage", {})
        text = " ".join(c.get("text", "") for i in r.get("output", []) if i.get("type") == "message" for c in i.get("content", []))
        return u.get("input_tokens") or 0, (u.get("input_tokens_details") or {}).get("cached_tokens") or 0, text
    if model.startswith("claude-"):
        body = {"model": model, "messages": msgs, "max_tokens": max_out or 32000, "thinking": {"type": "adaptive"}, "output_config": {"effort": effort}}
        if tools:
            body["tools"] = [{"name": "bash", "description": "run a command", "input_schema": {"type": "object", "properties": {"command": {"type": "string"}}}}]
            body["system"] = "Answer from your own reasoning; do not call tools for this."
            body["tool_choice"] = {"type": "none"}
        r = get(API + "/v1/messages", body, group)
        u = r.get("usage", {})
        text = " ".join(b.get("text", "") for b in r.get("content", []) if b.get("type") == "text")
        return (u.get("input_tokens") or 0) + (u.get("cache_read_input_tokens") or 0), u.get("cache_read_input_tokens") or 0, text
    body = {"model": model, "messages": msgs, "reasoning_effort": effort}
    if max_out:
        body["max_tokens"] = max_out
    if tools:
        body["tools"] = [{"type": "function", "function": {"name": "bash", "description": "run a command", "parameters": {"type": "object", "properties": {"command": {"type": "string"}}}}}]
        body["messages"] = [{"role": "system", "content": "Answer from your own reasoning; do not call tools for this."}] + msgs
        body["tool_choice"] = "none"
    r = get(API + "/v1/chat/completions", body, group)
    u = r.get("usage", {})
    det = u.get("prompt_tokens_details") or {}
    m = r["choices"][0]["message"]
    # a tool call despite tool_choice none says nothing about cut calls: report it as such
    text = m.get("content") or ("[called a tool instead of answering]" if m.get("tool_calls") else "")
    return u.get("prompt_tokens") or u.get("input_tokens") or 0, det.get("cached_tokens") or 0, text


def cache_hit(model, group, calls=6):
    """Share of each previous call's input cached, over a conversation grown like an agent's."""
    nonce, msgs, rows = random.randrange(1 << 40), [], []
    start = random.randrange(0, len(TEXT) - 40000)
    for i in range(calls):
        chunk = TEXT[start + i * 4000: start + (i + 1) * 4000] if i else f"[{nonce}]\n" + TEXT[start: start + 24000]
        msgs.append({"role": "user", "content": chunk + "\n\nReply with OK only."})
        rows.append(call(model, group, msgs, "low")[:2])
        msgs.append({"role": "assistant", "content": "OK"})
        time.sleep(10)  # an agent's tool run between calls
    return sum(min(c, p[0]) for p, (_, c) in zip(rows, rows[1:])) / max(1, sum(p[0] for p in rows[:-1]))


def check(model, group, price):
    """None when the group passes, else why not."""
    try:
        hit = cache_hit(model, group)
    except (urllib.error.URLError, OSError, KeyError, ValueError) as e:
        return f"cache test failed: {str(e)[:80]}"
    if hit < 0.5:
        return f"cache {hit:.0%}"
    t = time.time()
    try:
        _, _, text = call(model, group, [{"role": "user", "content": QUESTION}], "high", tools=True, max_out=0)
    except (urllib.error.URLError, OSError, KeyError, ValueError) as e:
        return f"long reasoning call failed after {time.time() - t:.0f} s: {str(e)[:80]}"
    if text.startswith("[called a tool"):
        # wanting to compute with a tool is fine for an agent (GLM ignores tool_choice none): ask again without tools,
        # which still shows whether a long call is cut
        t = time.time()
        try:
            _, _, text = call(model, group, [{"role": "user", "content": QUESTION}], "high", max_out=0)
        except (urllib.error.URLError, OSError, KeyError, ValueError) as e:
            return f"long reasoning call (without tools) failed after {time.time() - t:.0f} s: {str(e)[:80]}"
    if not text.strip():
        return f"long reasoning call came back empty after {time.time() - t:.0f} s"
    if re.search(r"[぀-ヿ㐀-鿿가-힯]", text):
        return "answered an English question in another language"
    nums = [int(n) for n in re.findall(r"\d+", text.strip().splitlines()[-1])]
    if not nums or abs(nums[-1] - ANSWER) > 15:
        return f"answer {nums[-1:] or text.strip()[-40:]!r} is far from {ANSWER}"
    print(f"    cache {hit:.0%} (a typical run ${run_cost(price, hit):.3f} at that rate), long call {time.time() - t:.0f} s, answer {nums[-1]}", flush=True)
    return None


def price_env(model, group, budget):
    """(exit code, line): the env for a run on the group at today's price, or why not."""
    p = dict(groups(model)).get(group)
    if p is None:
        return 1, f"{group} no longer serves {model}"
    cost = run_cost(p)
    if cost > budget:
        return 2, f"{group}: a typical run now costs ${cost:.3f}, over the ${budget:.2f} budget"
    return 0, f"TAZUNA_PRICE={p[0]:.5f},{p[1]:.6f},{p[2]:.4f} TAZUNA_MAX_USD={min(budget, 5 * cost):.3f}"


if __name__ == "__main__":
    assert norm("anthropic/claude-haiku-5.5") == norm("claude-haiku-5-5") == "claudehaiku55"
    if "--price" in sys.argv:
        a = sys.argv
        code, line = price_env(a[1], a[a.index("--price") + 1], float(a[a.index("--budget") + 1]) if "--budget" in a else 1.0)
        print(line)
        sys.exit(code)
    assert abs(run_cost((1.0, 0.1, 2.0), hit=0.5) - (6 * (0.5 * 1.25 + 0.05) + 0.16)) < 1e-9
    model, tries = sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 4
    gs, ref = groups(model), reference(model)
    print(f"{model}: {len(gs)} groups; a typical run ({RUN_IN / 1e6:.0f}M input, {RUN_OUT / 1e3:.0f}K output, {RUN_HIT:.0%} cached):")
    if ref:
        print(f"  OpenRouter reference: ${ref[0]:.4f} / ${ref[1]:.5f} / ${ref[2]:.3f} per M -> ${run_cost(ref):.3f} per run")
    for g, p in gs[:8]:
        off = f"  {1 - run_cost(p) / run_cost(ref):+.0%} vs OpenRouter" if ref else ""
        print(f"  {g:44} ${p[0]:.4f} / ${p[1]:.5f} / ${p[2]:.3f} per M -> ${run_cost(p):.3f} per run{off}")
    for g, p in gs[:tries]:
        print(f"checking {g}", flush=True)
        why = check(model, g, p)
        if why is None:
            print(f"PIN {g}")
            sys.exit(0)
        print(f"    no: {why}", flush=True)
    print("PIN none: no group passed")
    sys.exit(1)
