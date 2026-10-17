#!/usr/bin/env python3
"""UnoRouter: rank a model's groups by today's price, then test caching on the cheapest ones.

Prices change by the hour (si was the dearest Luna group one night and the cheapest the next day), and a group can be
a pool of upstream accounts whose caches do not share, so a short probe of identical calls misleads (a7-5053 hit on
every repeat of 4 calls, then missed on most calls of a real run). This probe grows a conversation the way an agent does:
each call appends new text and the reply, with a pause between calls. A call whose input count is smaller than the one
before went to another upstream (each adds its own hidden prompt).

    UNOROUTER_API_KEY=... ./uno_route.py gpt-6-luna [groups to test, default 3] [calls per group, default 8]

Uses the key's own access with an `X-Group` header per group. Cost: about a cent per group at Luna's prices.
A probe narrows the choice, a short real run decides: a7-5053 passed and then missed most of a run. (Until fca1072 the
probe read `input_tokens`, which UnoRouter sets to 0 on Chat Completions, so every non-GPT model showed 0% cached.)
"""
import json, os, random, sys, time, urllib.error, urllib.request

API = "https://api.unorouter.com"
KEY = os.environ["UNOROUTER_API_KEY"]
TEXT = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "src", "engine.rs")).read()


def get(path, key=None, body=None, group=None):
    h = {"Content-Type": "application/json"}
    if key:
        h["Authorization"] = f"Bearer {key}"
    if group:
        h["X-Group"] = group
    r = urllib.request.Request(API + path, data=json.dumps(body).encode() if body else None, headers=h)
    return json.load(urllib.request.urlopen(r, timeout=600))


def groups(model):
    """(price ratio, group) for every group serving the model, cheapest first; input $/M = ratio x list."""
    d = get("/api/pricing")
    m = next(m for m in d["data"] if m["model_name"] == model)
    return m["model_ratio"] * 2, sorted((d["group_ratio"][g], g) for g in m["enable_groups"] if g in d["group_ratio"])


def probe(model, group, calls):
    """Grows one conversation over `calls` calls; returns (hit share of the previous prefix, upstream switches, rows)."""
    nonce, msgs, rows = random.randrange(1 << 40), [], []
    start = random.randrange(0, len(TEXT) - 40000)
    for i in range(calls):
        chunk = TEXT[start + i * 4000: start + (i + 1) * 4000] if i else f"[{nonce}]\n" + TEXT[start: start + 24000]
        msgs.append({"role": "user", "content": chunk + "\n\nReply with OK only."})
        # the API tazuna uses: Responses for GPT models, Chat Completions for the rest
        if model.startswith("gpt-"):
            path, body = "/v1/responses", {"model": model, "input": msgs, "store": False, "reasoning": {"effort": "low"},
                                           "max_output_tokens": 64, "prompt_cache_key": f"tz-route-{nonce}"}
        else:
            path, body = "/v1/chat/completions", {"model": model, "messages": msgs, "max_tokens": 64}
        t = time.time()
        try:
            r = get(path, KEY, body, group)
        except urllib.error.HTTPError as e:
            rows.append((None, None, f"HTTP {e.code}: {e.read().decode()[:80]}"))
            break
        u = r.get("usage", {})
        det = u.get("input_tokens_details") or u.get("prompt_tokens_details") or {}
        rows.append((u.get("input_tokens") or u.get("prompt_tokens") or 0, det.get("cached_tokens") or u.get("prompt_cache_hit_tokens") or 0, f"{time.time() - t:.0f}s"))
        msgs.append({"role": "assistant", "content": "OK"})
        time.sleep(15)  # an agent's tool run between calls
    ok = [r for r in rows if r[0] is not None]
    # a hit can cover at most the previous call's input
    hit = sum(min(c, p[0]) for p, (_, c, _) in zip(ok, ok[1:])) / max(1, sum(p[0] for p in ok[:-1]))
    switches = sum(1 for a, b in zip(ok, ok[1:]) if b[0] < a[0])
    return hit, switches, rows


if __name__ == "__main__":
    model = sys.argv[1] if len(sys.argv) > 1 else "gpt-6-luna"
    n, calls = int(sys.argv[2]) if len(sys.argv) > 2 else 3, int(sys.argv[3]) if len(sys.argv) > 3 else 8
    price, gs = groups(model)
    print(f"{model}: list input ${price:.3f}/M; groups by today's price:")
    for r, g in gs:
        print(f"  {g:40} x{r:<6} ${price * r:.4f}/M")
    for r, g in gs[:n]:
        hit, sw, rows = probe(model, g, calls)
        # misses are billed as cache writes (1.25x input), hits at 10%
        eff = price * r * (hit * 0.1 + (1 - hit) * 1.25)
        print(f"{g}: {hit:.0%} of the previous prefix cached, {sw} upstream switches, ~${eff:.4f}/M effective input")
        print("   input/cached/time per call:", [f"{i}/{c} {s}" for i, c, s in rows])
