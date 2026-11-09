"""Replay recorded claude-path runs (5m cache) as if run with a 1h cache, which lets the first main call read Claude
Code's shared prefix (H tokens, measured 13.4k on 2026-11-09). Per-call usage from the session files; no API calls.
Delta per run = B_main*(0.75*W_main - 1.9*H) + sum over subagent models B_m*0.75*W_m, where W = 5m cache writes.
B (input price per token) per model is fitted from claude's own modelUsage costs in the same file."""
import glob, json, os, statistics as st, sys
H = 13400
files = [f for f in glob.glob(os.path.expanduser("~/bench-homes/cc/.local/share/tazuna/sessions/*@claude-opus-5-5:medium*.jsonl"))]
out = []
for f in files:
    seen, W, prices, cost, L, prev, prev_t = set(), {}, {}, 0.0, 0, None, None
    for line in open(f):
        try: d = json.loads(line)
        except ValueError: continue
        e = d.get("e") or {}
        if e.get("type") == "assistant":
            m = e["message"]; u = m.get("usage") or {}
            if m.get("id") in seen: continue
            seen.add(m.get("id"))
            key = "main" if not e.get("parent_tool_use_id") else m.get("model")
            W.setdefault(key, [0, m.get("model")])[0] += u.get("cache_creation_input_tokens", 0)
            if key == "main":
                w, r = u.get("cache_creation_input_tokens", 0), u.get("cache_read_input_tokens", 0)
                gap = (d.get("t", 0) - prev_t) / 1000 if prev_t is not None else 0
                if prev and prev > 20000 and r < 0.5 * prev and 300 <= gap < 3600:
                    L += w  # the 5m cache expired during a wait: with 1h these tokens are read, not written
                prev, prev_t = r + w + u.get("input_tokens", 0), d.get("t")
        elif e.get("type") == "result":
            cost += e.get("total_cost_usd") or 0
            for mdl, mu in (e.get("modelUsage") or {}).items():
                d = mu.get("inputTokens", 0) + 1.25 * mu.get("cacheCreationEphemeral5mInputTokens", 0) + 2 * mu.get("cacheCreationEphemeral1hInputTokens", 0) \
                    + 0.1 * mu.get("cacheReadInputTokens", 0) + 5 * mu.get("outputTokens", 0)
                if d and mu.get("costUSD"): prices.setdefault(mdl, []).append(mu["costUSD"] / d)
    if "main" not in W or not cost: continue
    B = {k: st.median(v) for k, v in prices.items()}
    wm, mm = W["main"]
    delta = B.get(mm, 0) * (0.75 * wm - 1.9 * L - 1.9 * min(H, wm)) + sum(B.get(k, 0) * 0.75 * w for k, (w, _) in W.items() if k != "main")
    out.append((os.path.basename(f).split("-tazuna-")[1].split("-v")[0] if "-tazuna-" in f else "?", cost, delta, wm, B.get(mm), L))
print("runs", len(out), "B_opus median", st.median([o[4] for o in out if o[4]]))
by = {}
for o in out: by.setdefault(o[0], []).append(o)
for k, v in sorted(by.items(), key=lambda x: -len(x[1])):
    c, d = sum(x[1] for x in v), sum(x[2] for x in v)
    print(f"{k:14} n={len(v):3} cost ${c:7.2f}  1h-5m ${d:+6.2f} ({100*d/c:+5.1f}%)  W_main median {int(st.median(x[3] for x in v)):6}  runs where 1h cheaper {sum(x[2] < 0 for x in v)}")
c, d = sum(o[1] for o in out), sum(o[2] for o in out)
print(f"ALL n={len(out)} cost ${c:.2f}  1h-5m ${d:+.2f} ({100*d/c:+.1f}%)")
