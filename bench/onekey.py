#!/usr/bin/env python3
"""One SubRouter key for several models, each limited to its own providers (2026-11-06): can a user's single key replace
tazuna's one key per model? A test key allows DeepSeek V4.1 Flash 1017 only on lingai and GPT 6.1 Sol only on bangai.
Their union holds a cheaper provider for each model (Sol is cheaper on lingai than on bangai), so if SubRouter applied the
union instead of the per-model lists, the calls would drift there. Six short calls per model, interleaved; then the
account log says where each call went. The key is deleted at the end; its value is never printed or stored."""
import json, time, urllib.request

import subrouter as s

NAME = "tazuna-route-onekey-test"
LIMITS = {"deepseek-v4.1-flash-1017": ["lingai"], "gpt-6.1-sol": ["bangai"]}


def mine():
    d = s.api("/api/token/?p=1&page_size=100")
    return next((k for k in (d["items"] if isinstance(d, dict) else d) if k["name"] == NAME), None)


def ask(key, model):
    body = {"model": model, "messages": [{"role": "user", "content": "Reply with the single word: ok"}], "max_tokens": 400,
            "reasoning_effort": "low"}
    q = urllib.request.Request("https://subrouter.ai/v1/chat/completions", data=json.dumps(body).encode(),
                               headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "curl/8"})
    try:
        with urllib.request.urlopen(q, timeout=120) as r:
            return json.load(r)["choices"][0]["message"].get("content", "")[:20]
    except Exception as e:  # noqa: BLE001
        return f"error {str(e)[:60]}"


limits = {"model_limits_enabled": True, "model_limits": ",".join(LIMITS), "subrouter_model_providers": json.dumps(LIMITS),
          "subrouter_providers": ",".join(sorted({p for ps in LIMITS.values() for p in ps})), "include_official_channels": False,
          "include_shared_subscriptions": False, "include_cache_hit_rate": True}
assert mine() is None, f"{NAME} exists already"
s.api("/api/token/", "POST", {"name": NAME, "expired_time": -1, "unlimited_quota": True, **limits})
k = mine()
try:
    stored = s.api(f"/api/token/{k['id']}")
    print("stored:", stored.get("model_limits"), stored.get("subrouter_model_providers"), stored.get("subrouter_providers"))
    key = "sk-" + stored["key"]
    start = int(time.time())
    for i in range(6):
        for m in LIMITS:
            print(m, ask(key, m), flush=True)
    time.sleep(30)  # the account log lags a little
    rows = [c for c in s.calls(15) if c.get("token_name") == NAME and int(c["created_at"]) >= start - 5]
    seen = {}
    for c in rows:
        o = json.loads(c.get("other") or "{}")
        seen.setdefault(c["model_name"], []).append(o.get("provider_slug") or c.get("group", "?"))
    for m, ps in seen.items():
        print(f"{m}: {len(ps)} calls on {sorted(set(ps))} (allowed {LIMITS.get(m)})")
    print("calls logged:", len(rows), " cost $%.4f" % (sum(int(c["quota"]) for c in rows) / s.QUOTA_PER_USD))
finally:
    s.api(f"/api/token/{k['id']}", "DELETE")
    print("test key deleted:", mine() is None)
