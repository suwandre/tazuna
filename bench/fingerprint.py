#!/usr/bin/env python3
"""Is a provider's model the model it is sold as? (fingerprint.py MODEL SLUG...; keys tazuna-route-smoke-<slug> from
provsmoke.py) 2026-11-05, after fxpayback's "Sol" scored 29/100 on HLE against xcgx's 45: the same fixed input to every
provider, low effort, a few output tokens. A model's tokenizer counts a given text the same way on every honest
provider, so a different input-token count means a different tokenizer (another model, or a wrapper that adds text);
the response's shape (model name, item types and id prefixes, encrypted reasoning) shows a proxy built on something else.
Costs well under a cent a provider."""
import json, os, re, sys, urllib.request

M, SLUGS = sys.argv[1], sys.argv[2:]
TEXT = ("Fingerprint check. Repeat nothing; answer with the single word OK.\n"
        + "Die Quellcode-Überprüfung prüft 1234567 Zeilen, émojis 🙂🚀, 中文字符, and identifiers like snake_case_name, "
        "CamelCaseName, kebab-case-name, 0x7FFF_FFFF,  non-breaking spaces and tabs\tinside.\n") * 3


def var(slug):  # as in provsmoke.py: one key per model and provider
    return "SUBROUTER_SMOKE_" + re.sub(r"[^A-Z0-9]+", "_", (M + "_" + slug).upper()).strip("_") + "_KEY"


def post(path, key, body):
    q = urllib.request.Request("https://subrouter.ai/v1/" + path, data=json.dumps(body).encode(),
                               headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "curl/8"})
    try:
        with urllib.request.urlopen(q, timeout=180) as r:
            return json.load(r)
    except urllib.error.HTTPError as e:
        return {"error": f"HTTP {e.code} {e.read()[:120]}"}
    except Exception as e:  # noqa: BLE001
        return {"error": repr(e)[:120]}


for g in SLUGS:
    key = os.environ.get(var(g))
    if not key:
        print(f"{g:14} no key {var(g)}")
        continue
    resp = post("responses", key, {"model": M, "input": TEXT, "reasoning": {"effort": "low"}, "max_output_tokens": 64, "store": False,
                                   "include": ["reasoning.encrypted_content"]})
    chat = post("chat/completions", key, {"model": M, "messages": [{"role": "user", "content": TEXT}], "max_tokens": 64, "reasoning_effort": "low"})
    out = resp.get("output") or []
    enc = [len(i.get("encrypted_content") or "") for i in out if i.get("type") == "reasoning"]
    print(json.dumps({"slug": g, "resp_input_tokens": (resp.get("usage") or {}).get("input_tokens"), "chat_prompt_tokens": (chat.get("usage") or {}).get("prompt_tokens"),
                      "model": resp.get("model"), "id": str(resp.get("id"))[:6], "items": [f"{i.get('type')}:{str(i.get('id'))[:4]}" for i in out],
                      "encrypted_reasoning_chars": enc, "chat_model": chat.get("model"), "chat_fingerprint": chat.get("system_fingerprint"),
                      "errors": [x["error"] for x in (resp, chat) if "error" in x]}, ensure_ascii=False))
