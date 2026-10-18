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
Groups listed for the last 3 hours in the price log go first when the logger runs (cheap groups that just appeared often
vanish within the hour). Prints PIN <group> for the first group that passes and GROUPS <a,b,c> for up to three that pass (cheapest first), the
list tazuna switches along when a group drops out or stops caching (`TAZUNA_GROUPS`). Prices move between the check and the run (a Grok group went from
$0.024 to $0.10 per M input within minutes), so `--price` reads the group's price again: it prints the
`TAZUNA_PRICE=... TAZUNA_MAX_USD=...` to start the run with (the cap at 5x a run at 50% cached, at most the budget), and exits 1
when the group no longer serves the model, 2 when the estimated run costs more than the budget. Costs a cent or two per group tried; uses the key's own access with an
`X-Group` header per group. A pass is no guarantee (a7-5053 passed a cache probe, then missed most of a run): the run
script still stops a run whose cache rate falls.
"""
import json, os, random, re, signal, sys, time, urllib.error, urllib.request

API = "https://api.unorouter.com"
KEY = os.environ.get("UNOROUTER_API_KEY", "")
TEXT = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src", "engine.rs")).read()
# a typical rclone run: input sent over all calls, share cached when the cache works, output
RUN_IN, RUN_HIT, RUN_OUT = 6e6, 0.95, 80e3
QUESTION, ANSWER = "How many primes p < 20000 satisfy p ≡ 3 (mod 8)? Work it out carefully, then give only the number on the last line.", 571
# the long call's output limit: DeepSeek thought through 128K tokens on the question without an answer (0.45 cents); hitting
# the limit after 2 minutes still shows the call is not cut, which is what the test is for
LONG_CAP, LONG_MIN_S = 40000, 120
IDENTITY = ("What is 17 times 23? Answer in one short English sentence that ends with the number.", 391)


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
    steady = listed_for(model) or set()
    return sorted(gs, key=lambda g: (g[0] not in steady, run_cost(g[1])))


def listed_for(model, hours=3):
    """Groups listed for the model in every reading of the price log (realwork/uno_price_log.sh) over the last hours, or
    None without a log covering them. Of DeepSeek's cheapest groups 43% were gone within the hour; of the cheapest listed
    3 hours or more, 10%, at 1.18x the price (29 hours of readings, 2026-10-17/18)."""
    try:
        rows = [json.loads(l) for l in open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "realwork", "uno-prices.jsonl"))]
    except OSError:
        return None
    now = time.time()
    rows = [r for r in rows if now - r["t"] <= hours * 3600 and model in r]
    if not rows or now - rows[-1]["t"] > 1800 or rows[0]["t"] > now - hours * 3600 + 1800:
        return None
    return set.intersection(*(set(r[model]["in"] if "in" in r[model] else r[model]) for r in rows))


def reference(model):
    """OpenRouter's (input, cached, output) USD per M for the model, or None."""
    for m in get("https://openrouter.ai/api/v1/models")["data"]:
        if norm(m["id"]) == norm(model):
            p = m["pricing"]
            i = float(p["prompt"]) * 1e6
            return i, float(p.get("input_cache_read") or p["prompt"]) * 1e6, float(p["completion"]) * 1e6


def call(model, group, msgs, effort, tools=False, max_out=64):
    """One unstreamed call in the API tazuna uses for the model; returns (input, cached, text, stopped at the output limit)."""
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
        return u.get("input_tokens") or 0, (u.get("input_tokens_details") or {}).get("cached_tokens") or 0, text, r.get("status") == "incomplete"
    if model.startswith("claude-"):
        body = {"model": model, "messages": msgs, "max_tokens": max_out or 32000, "thinking": {"type": "adaptive"}, "output_config": {"effort": effort}}
        if tools:
            body["tools"] = [{"name": "bash", "description": "run a command", "input_schema": {"type": "object", "properties": {"command": {"type": "string"}}}}]
            body["system"] = "Answer from your own reasoning; do not call tools for this."
            body["tool_choice"] = {"type": "none"}
        r = get(API + "/v1/messages", body, group)
        u = r.get("usage", {})
        text = " ".join(b.get("text", "") for b in r.get("content", []) if b.get("type") == "text")
        return (u.get("input_tokens") or 0) + (u.get("cache_read_input_tokens") or 0), u.get("cache_read_input_tokens") or 0, text, r.get("stop_reason") == "max_tokens"
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
    return u.get("prompt_tokens") or u.get("input_tokens") or 0, det.get("cached_tokens") or 0, text, r["choices"][0].get("finish_reason") == "length"


def ask_streamed(model, group):
    """The long question once more as a stream, as tazuna sends a call that UnoRouter cut unstreamed (it caps those at
    ~90-180 s, streams it does not); returns (answer text, stopped at the output limit)."""
    msgs = [{"role": "user", "content": QUESTION}]
    if model.startswith("gpt-"):
        path, body = "/v1/responses", {"model": model, "input": msgs, "store": False, "reasoning": {"effort": "high"}, "max_output_tokens": LONG_CAP, "stream": True}
    elif model.startswith("claude-"):
        path, body = "/v1/messages", {"model": model, "messages": msgs, "max_tokens": LONG_CAP, "thinking": {"type": "adaptive"}, "output_config": {"effort": "high"}, "stream": True}
    else:
        path, body = "/v1/chat/completions", {"model": model, "messages": msgs, "reasoning_effort": "high", "max_tokens": LONG_CAP, "stream": True}
    h = {"Content-Type": "application/json", "Authorization": f"Bearer {KEY}", "anthropic-version": "2023-06-01", "X-Group": group}
    text, capped = "", False
    for raw in urllib.request.urlopen(urllib.request.Request(API + path, data=json.dumps(body).encode(), headers=h), timeout=600):
        line = raw.decode(errors="replace").strip()
        if not line.startswith("data:") or line == "data: [DONE]":
            continue
        e = json.loads(line[5:])
        text += "".join((c.get("delta") or {}).get("content") or "" for c in e.get("choices") or [])
        capped |= any(c.get("finish_reason") == "length" for c in e.get("choices") or [])
        d = e.get("delta")
        capped |= e.get("type") == "response.incomplete" or (isinstance(d, dict) and d.get("stop_reason") == "max_tokens")
        if e.get("type") == "response.output_text.delta" or (e.get("type") == "content_block_delta" and (e.get("delta") or {}).get("type") == "text_delta"):
            text += e.get("delta", "") if isinstance(e.get("delta"), str) else e["delta"].get("text", "")
    return text, capped


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
    cjk = re.compile(r"[\u3040-\u30ff\u3400-\u9fff\uac00-\ud7af]")
    t, capped = time.time(), False
    try:
        _, _, text, capped = call(model, group, [{"role": "user", "content": QUESTION}], "high", tools=True, max_out=LONG_CAP)
        if text.startswith("[called a tool"):
            # wanting to compute with a tool is fine for an agent (GLM ignores tool_choice none): ask again without tools,
            # which still shows whether a long call is cut
            _, _, text, capped = call(model, group, [{"role": "user", "content": QUESTION}], "high", max_out=LONG_CAP)
    except (urllib.error.URLError, OSError, KeyError, ValueError):
        text = ""  # cut or failed unstreamed: asked again as a stream below, as tazuna does
    if not text.strip() and not capped:
        try:
            text, capped = ask_streamed(model, group)
        except (urllib.error.URLError, OSError, KeyError, ValueError) as e:
            return f"long reasoning call failed, also streamed, after {time.time() - t:.0f} s: {str(e)[:80]}"
    secs = time.time() - t
    nums = [int(n) for n in re.findall(r"\d+", text.strip().splitlines()[-1])] if text.strip() else []
    if nums and abs(nums[-1] - ANSWER) <= 15:
        long_call = f"long call {secs:.0f} s, answer {nums[-1]}"
    elif capped and secs >= LONG_MIN_S:
        long_call = f"long call still thinking at the {LONG_CAP // 1000}K limit after {secs:.0f} s (not cut)"
    elif not text.strip():
        return f"long reasoning call came back empty, also streamed, after {secs:.0f} s"
    else:
        return f"long call answer {nums[-1:] or text.strip()[-40:]!r} is far from {ANSWER}"
    if cjk.search(text):
        return "answered an English question in another language"
    try:
        _, _, short, _ = call(model, group, [{"role": "user", "content": IDENTITY[0]}], "low", max_out=4000)
    except (urllib.error.URLError, OSError, KeyError, ValueError) as e:
        return f"identity question failed: {str(e)[:80]}"
    if cjk.search(short) or str(IDENTITY[1]) not in short:
        return f"identity question answered {short.strip()[-60:]!r}"
    print(f"    cache {hit:.0%} (a typical run ${run_cost(price, hit):.3f} at that rate), {long_call}, identity ok", flush=True)
    return None


def price_env(model, group, budget):
    """(exit code, line): the env for a run on the group at today's price, or why not."""
    p = dict(groups(model)).get(group)
    if p is None:
        return 1, f"{group} no longer serves {model}"
    cost = run_cost(p)
    if cost > budget:
        return 2, f"{group}: a typical run now costs ${cost:.3f}, over the ${budget:.2f} budget"
    return 0, f"TAZUNA_PRICE={p[0]:.5f},{p[1]:.6f},{p[2]:.4f} TAZUNA_MAX_USD={min(budget, 5 * run_cost(p, 0.5)):.3f}"


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
    def too_slow(*_):
        raise TimeoutError("over 15 minutes for this group")
    signal.signal(signal.SIGALRM, too_slow)
    passed = []
    for g, p in gs[:tries]:
        if len(passed) == 3:
            break
        print(f"checking {g}", flush=True)
        # a group that holds a call open stalled the check for 40 minutes; the slowest pass so far took 11 (DeepSeek)
        signal.alarm(900)
        try:
            why = check(model, g, p)
        except TimeoutError as e:
            why = str(e)
        signal.alarm(0)
        if why is None:
            passed.append(g)
        else:
            print(f"    no: {why}", flush=True)
    if not passed:
        print("PIN none: no group passed")
        sys.exit(1)
    print(f"PIN {passed[0]}\nGROUPS {','.join(passed)}")
