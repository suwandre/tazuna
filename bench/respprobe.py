"""One provider on SubRouter's Responses route at max effort (respprobe.py PROVIDER VET-KEY-NUMBER [MODEL]): one long reasoning call with tools, streamed; then four agent-like conversations at once, 10 growing calls each. Served-by and cost from the account log. Sol, 2026-10-28: shared-openai-pro-50 88% cached, 0 errors, $0.008; foxrouter 52%, $0.031.
"""
import json, os, sys, time, urllib.request, threading
sys.path.insert(0, "/Users/suwandre/Desktop/agent-chats/tazuna/bench"); import subrouter as s
M, slug, i = (sys.argv[3] if len(sys.argv) > 3 else "gpt-6.1-sol"), sys.argv[1], int(sys.argv[2])
var = f"SUBROUTER_VET{i}_KEY"
vid = s.key(f"tazuna-vet-{i}", M, [slug], var); key = os.environ[var]
k = s.api(f"/api/token/{vid}")
if bool(k.get("include_shared_subscriptions")) != slug.startswith("shared-"):
    s.api("/api/token/", "PUT", {**k, "include_shared_subscriptions": slug.startswith("shared-")})
SRC = "/Users/suwandre/Desktop/agent-chats/tazuna/src/"
SYSTEM = open(SRC + "live.rs").read()[:24000]
FILES = [open(SRC + f).read() for f in ("engine.rs", "mcp.rs", "verify.rs", "session.rs")]
TOOLS = [{"type": "function", "name": "bash", "description": "Run a shell command.", "parameters": {"type": "object", "properties": {"command": {"type": "string"}}, "required": ["command"]}}]
def stream(body):
    q = urllib.request.Request("https://subrouter.ai/v1/responses", data=json.dumps({**body, "stream": True}).encode(),
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "curl/8", "Accept": "text/event-stream"})
    t = time.time(); done = None
    try:
        with urllib.request.urlopen(q, timeout=900) as r:
            for line in r:
                if line.startswith(b"data: {"):
                    e = json.loads(line[6:])
                    if e.get("type") in ("response.completed", "response.failed", "response.incomplete"): done = e.get("response")
    except urllib.error.HTTPError as e: return {"error": f"HTTP {e.code} {e.read()[:120]}"}, round(time.time() - t)
    except Exception as e: return {"error": repr(e)[:120]}, round(time.time() - t)
    return (done or {"error": "no completed event"}), round(time.time() - t)
def text(r): return "".join(c.get("text", "") for o in r.get("output", []) if o.get("type") == "message" for c in o.get("content", []))
t0 = int(time.time())
# (1) long reasoning with tools available
r, secs = stream({"model": M, "reasoning": {"effort": "max"}, "tools": TOOLS, "max_output_tokens": 60000, "input": [
    {"role": "user", "content": "Without running code: how many positive integers n <= 2000 have the property that n^2 + 1 is divisible by 5 and the sum of the decimal digits of n is prime? Think it through carefully and give the number, then one line on how you counted."}]})
u = r.get("usage") or {}
print(f"{slug} long reasoning: {secs}s status {r.get('status', r.get('error'))} out {u.get('output_tokens')} reasoning {(u.get('output_tokens_details') or {}).get('reasoning_tokens')} answer {text(r)[:80]!r}", flush=True)
# (2) four conversations at once
res = {}
def conv(c):
    hist = [{"role": "developer", "content": "You are a coding agent. Context:\n" + SYSTEM}, {"role": "user", "content": f"Task {c}: review the files I paste, one per turn; answer in one sentence each."}]
    rows = []
    for n in range(10):
        hist.append({"role": "user", "content": f"File part {n}:\n" + FILES[(c + n) % 4][n * 4000:(n + 1) * 4000]})
        r, secs = stream({"model": M, "input": hist, "tools": TOOLS, "reasoning": {"effort": "max"}, "max_output_tokens": 8000, "prompt_cache_key": f"p2-{slug}-{c}-{t0}"})
        if r.get("error"): rows.append(("err", str(r["error"])[:80], secs)); time.sleep(5); continue
        u = r.get("usage") or {}; rows.append((u.get("input_tokens"), (u.get("input_tokens_details") or {}).get("cached_tokens"), u.get("output_tokens"), secs))
        hist.append({"role": "assistant", "content": text(r) or "ok"})
    res[c] = rows
th = [threading.Thread(target=conv, args=(c,)) for c in range(4)]
[t.start() for t in th]; [t.join() for t in th]
calls = errs = 0; inp = cached = 0; warm_miss = 0; secs = []
for c, rows in sorted(res.items()):
    for n, x in enumerate(rows):
        if x[0] == "err": errs += 1; print("   conv", c, "call", n, x); continue
        calls += 1; inp += x[0] or 0; cached += x[1] or 0; secs.append(x[3])
        if n > 0 and not x[1]: warm_miss += 1
print(f"{slug} 4 conversations: {calls} ok, {errs} errors, cached {100 * cached // max(1, inp)}% of input, warm misses {warm_miss}, median {sorted(secs)[len(secs)//2] if secs else '-'}s, max {max(secs) if secs else '-'}s", flush=True)
time.sleep(25)
cs = [c for c in s.calls((int(time.time()) - t0) // 60 + 2, M) if int(c["token_id"]) == vid and int(c["created_at"]) >= t0]
print(f"{slug} account log: {len(cs)} calls, served by {sorted({json.loads(c.get('other') or '{}').get('provider_slug', '?') for c in cs})}, ${sum(int(c['quota']) for c in cs) / s.QUOTA_PER_USD:.4f}")
