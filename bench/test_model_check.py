"""python3 bench/test_model_check.py: the flag logic of model_check.py on made-up data (no network)."""
import model_check as mc


def m(name, slug, creator, coding, intel, price):
    return {"name": name, "slug": slug, "model_creator": {"slug": creator},
            "evaluations": {"artificial_analysis_coding_index": coding, "artificial_analysis_intelligence_index": intel},
            "pricing": {"price_1m_blended_3_to_1": price}}


AA = [
    m("DeepSeek V4.1 Flash (max)", "deepseek-v4-1-flash-max", "deepseek", 40, 39, 0.12),
    m("DeepSeek V4.1 Flash (low)", "deepseek-v4-1-flash-low", "deepseek", 30, 31, 0.12),  # a weaker variant of the pick
    m("MiniMax M3", "minimax-m3", "minimax", 44, 41, 0.40),        # better, cheap, served: a flag
    m("Kimi K3", "kimi-k3", "moonshot", 55, 52, 5.20),             # better but over the price cap
    m("Mystery 9", "mystery-9", "other", 60, 60, 0.10),            # better and cheap, but none of our providers serves it
    m("Claude Opus 5.5", "claude-opus-5-5", "anthropic", 51, 51, 9.0),
    m("Claude Opus 6", "claude-opus-6", "anthropic", 58, 57, 9.0),  # a stronger Anthropic model
]
ROLES = [
    {"role": "cheap", "pick": "deepseek-v4.1-flash", "pool": "cheap", "metric": "coding_index", "max_blended_usd_per_m": 1.0},
    {"role": "main", "pick": "claude-opus-5-5", "pool": "anthropic", "metric": "coding_index"},
    {"role": "gone", "pick": "no-such-model", "pool": "anthropic", "metric": "coding_index"},
]
SERVED = {"cheaperinference": ["deepseek-v4.1-flash", "minimax-m3", "kimi-k3"]}

assert mc.norm("DeepSeek V4.1 Flash (max)") == mc.norm("deepseek-v4.1-flash") == "deepseekv41flash"
assert mc.best_variant(AA, mc.norm("deepseek-v4.1-flash"), "coding_index")["name"] == "DeepSeek V4.1 Flash (max)"
flags = mc.check(ROLES, AA, SERVED, {})
got = {(f[0], f[1]) for f in flags}
assert got == {("cheap", "MiniMax M3"), ("main", "Claude Opus 6"), ("gone", "(no data for no-such-model)")}, got
# an override maps a provider id the automatic match misses
assert mc.check(ROLES[:1], AA, {"x": ["mm3"]}, {"mm3": "minimax-m3"})[0][5] == "x:mm3"
print("model_check: ok")
