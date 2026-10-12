"""python3 bench/test_model_check.py: model_check.py's scoring and flags on made-up data (no network)."""
import model_check as mc

# names as each source spells them
assert mc.split_effort("glm-5.3_max") == ("glm53", "max")
assert mc.split_effort("claude-opus-5-5-medium") == ("claudeopus55", "medium")
assert mc.split_effort("claude-fable-5-1_xhigh") == ("claudefable51", "xhigh")
assert mc.split_effort("DeepSeek V4.1 Flash (Max)") == ("deepseekv41flash", "max")
assert mc.split_effort("minimax-m3") == ("minimaxm3", "default")
assert mc.category("SWE-bench Verified") == "agentic_coding" and mc.category("terminalbench_v4_0") == "agentic_coding"
assert mc.category("HellaSwag") is None and mc.category("brand-new-agent-bench") == "agentic_coding"

PROFILES = {"implementer": {"agentic_coding": 0.5, "coding": 0.2, "reasoning": 0.3}}
rows = [
    # pick: deepseek at max (and a weaker low variant that must not be what we compare against)
    ("deepseekv41flash", "max", "SWE-bench Verified", 0.60, "deepseek", 0.5),
    ("deepseekv41flash", "max", "livecodebench", 0.50, "deepseek", 0.5),
    ("deepseekv41flash", "max", "hle", 0.30, "deepseek", 0.5),
    ("deepseekv41flash", "low", "SWE-bench Verified", 0.20, "deepseek", 0.5),
    # better, cheap, served: a flag
    ("mimov26pro", "default", "SWE-bench Verified", 0.70, "xiaomi", 0.4),
    ("mimov26pro", "default", "livecodebench", 0.55, "xiaomi", 0.4),
    ("mimov26pro", "default", "hle", 0.35, "xiaomi", 0.4),
    # better but far dearer: no flag
    ("kimik3", "max", "SWE-bench Verified", 0.80, "moonshot", 6.0),
    ("kimik3", "max", "livecodebench", 0.70, "moonshot", 6.0),
    ("kimik3", "max", "hle", 0.50, "moonshot", 6.0),
    # better and cheap, but only two shared benchmarks: too little evidence
    ("thin", "max", "SWE-bench Verified", 0.90, "x", 0.1),
    ("thin", "max", "hle", 0.90, "x", 0.1),
    # worse: no flag
    ("weak", "max", "SWE-bench Verified", 0.40, "x", 0.1),
    ("weak", "max", "livecodebench", 0.40, "x", 0.1),
    ("weak", "max", "hle", 0.20, "x", 0.1),
]
scores, info = mc.table(rows)
assert mc.at_effort(scores, "deepseekv41flash", "max")[1] == "max"
assert mc.at_effort(scores, "mimov26pro", "max")[1] == "default"  # measured only at its default
ROLES = [{"role": "cheap", "pick": "deepseek-v4.1-flash", "effort": "max", "pool": "cheap", "max_blended_usd_per_m": 1.0, "profile": "implementer"}]
SERVED = {"el": ["deepseek-v4.1-flash", "mimo-v2.6-pro", "kimi-k3", "thin", "weak"]}
flags = mc.check(ROLES, PROFILES, scores, info, SERVED)
assert [f[1] for f in flags] == ["mimov26pro"], flags
lead = flags[0][3]
assert abs(lead - (0.5 * 0.10 + 0.2 * 0.05 + 0.3 * 0.05)) < 1e-9, lead
print("model_check: ok")
