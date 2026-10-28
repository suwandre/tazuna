"""One provider for MODEL on SubRouter, both routes the harnesses use (chatprobe.py MODEL PROVIDER VET-KEY-NUMBER):
the Chat route (tazuna, pi) with four agent-like conversations at once, 8 growing calls each, and the Anthropic route
(stock Claude Code) with thinking and a tool, twice; identity; served-by and cost from the account log. 2026-10-28."""
import json, os, sys, time, urllib.request, threading
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__))); import subrouter as s
M, slug, i = sys.argv[1], sys.argv[2], int(sys.argv[3])
var = f"SUBROUTER_VET{i}_KEY"
vid = s.key(f"tazuna-vet-{i}", M, [slug], var); key = os.environ[var]
k = s.api(f"/api/token/{vid}")
if k.get("include_shared_subscriptions"): s.api("/api/token/", "PUT", {**k, "include_shared_subscriptions": False})
SRC = "/Users/suwandre/Desktop/agent-chats/tazuna/src/"
SYSTEM = open(SRC + "live.rs").read()[:24000]
FILES = [open(SRC + f).read() for f in ("engine.rs", "mcp.rs", "verify.rs", "session.rs")]
TOOL = {"type": "function", "function": {"name": "bash", "description": "Run a shell command.", "parameters": {"type": "object", "properties": {"command": {"type": "string"}}, "required": ["command"]}}}
def post(path, body, extra=None):
    h = {"Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "curl/8", **(extra or {})}
    q = urllib.request.Request("https://subrouter.ai/v1/" + path, data=json.dumps(body).encode(), headers=h)
    t = time.time()
    try:
        with urllib.request.urlopen(q, timeout=300) as r: return json.load(r), round(time.time() - t)
    except urllib.error.HTTPError as e: return {"error": f"HTTP {e.code} {e.read()[:140]}"}, round(time.time() - t)
    except Exception as e: return {"error": repr(e)[:140]}, round(time.time() - t)
t0 = int(time.time()); res = {}
def conv(c):
    msgs = [{"role": "system", "content": "You are a coding agent. Context:\n" + SYSTEM}, {"role": "user", "content": f"Task {c}: review the files I paste, one per turn; one sentence each."}]
    rows = []
    for n in range(8):
        msgs.append({"role": "user", "content": f"File part {n}:\n" + FILES[(c + n) % 4][n * 4000:(n + 1) * 4000]})
        r, secs = post("chat/completions", {"model": M, "messages": msgs, "tools": [TOOL], "reasoning_effort": "max", "max_tokens": 4000})
        if r.get("error"): rows.append(("err", r["error"][:80], secs)); time.sleep(3); continue
        u = r.get("usage") or {}; m = r["choices"][0]["message"]
        cached = (u.get("prompt_tokens_details") or {}).get("cached_tokens") or u.get("prompt_cache_hit_tokens") or 0
        rows.append((u.get("prompt_tokens"), cached, u.get("completion_tokens"), secs, bool(m.get("reasoning_content") or m.get("reasoning"))))
        msgs.append({"role": "assistant", "content": m.get("content") or "ok"})
    res[c] = rows
th = [threading.Thread(target=conv, args=(c,)) for c in range(4)]; [t.start() for t in th]; [t.join() for t in th]
ok = [x for rows in res.values() for x in rows if x[0] != "err"]; errs = [x for rows in res.values() for x in rows if x[0] == "err"]
warm = [x for rows in res.values() for x in rows[1:] if x[0] != "err"]
inp = sum(x[0] or 0 for x in warm); cached = sum(x[1] or 0 for x in warm)
secs = sorted(x[3] for x in ok)
print(f"{M} {slug} chat: {len(ok)} ok, {len(errs)} errors {errs[:1]}, warm cached {100 * cached // max(1, inp)}%, warm misses {sum(1 for x in warm if not x[1])}, reasoning on {sum(x[4] for x in ok)}/{len(ok)}, median {secs[len(secs)//2] if secs else '-'}s", flush=True)
# Anthropic route, as Claude Code sends it: system, a tool, thinking, cache_control on the system block
ab = {"model": M, "max_tokens": 8000, "thinking": {"type": "adaptive"}, "output_config": {"effort": "max"},
      "system": [{"type": "text", "text": "You are a coding agent. Context:\n" + SYSTEM, "cache_control": {"type": "ephemeral"}}],
      "tools": [{"name": "bash", "description": "Run a shell command.", "input_schema": TOOL["function"]["parameters"]}],
      "messages": [{"role": "user", "content": "What is 17*23? Reply with the number only."}]}
out = []
for _ in range(2):
    r, sec = post("messages", ab, {"anthropic-version": "2023-06-01"})
    if r.get("error"): out.append(r["error"][:100]); continue
    u = r.get("usage") or {}; txt = "".join(b.get("text", "") for b in r.get("content", []) if b.get("type") == "text")
    out.append(f"ok {sec}s answer {txt.strip()[:8]!r} thinking {any(b.get('type') == 'thinking' for b in r.get('content', []))} cache_read {u.get('cache_read_input_tokens')}")
print(f"{M} {slug} messages: {out}", flush=True)
time.sleep(25)
cs = [c for c in s.calls((int(time.time()) - t0) // 60 + 2, M) if int(c["token_id"]) == vid and int(c["created_at"]) >= t0]
print(f"{M} {slug} account log: {len(cs)} calls, served by {sorted({json.loads(c.get('other') or '{}').get('provider_slug', '?') for c in cs})}, ${sum(int(c['quota']) for c in cs) / s.QUOTA_PER_USD:.4f}")
