#!/usr/bin/env bash
# Tier 0: free checks for every change. Prints one JSON line of metrics (append it to bench/tier0.jsonl to track them).
#   bench/tier0.sh            # needs the claude CLI logged in; spends ~$0.003 on one Haiku call
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release -q
cargo test --release -q 2>&1 | grep -q "test result: ok" || { echo "tests failed" >&2; exit 1; }
BIN=target/release/tazuna

size=$(stat -f%z "$BIN" 2>/dev/null || stat -c%s "$BIN")

# render cost: replay a whole recorded session headlessly (session state + layout + wrapping)
start=$(python3 -c 'import time; print(time.time())')
"$BIN" recordings/caddy-opus-medium.jsonl --snapshot 100000 --size 120x40 >/dev/null
render_ms=$(python3 -c "import time; print(round((time.time()-$start)*1000))")

# per-turn overhead: input tokens of a one-word task (system prompt + tool schemas), via the real live path
work=$(mktemp -d)
git -C "$work" init -q
echo "Reply with exactly: ok" > "$work/prompt.txt"
result=$(cd "$work" && "$OLDPWD/$BIN" --run prompt.txt --model claude-haiku-4-5-20251001 --effort low)
rm -rf "$work"

RESULT="$result" SIZE="$size" RENDER="$render_ms" python3 - <<'EOF'
import json, os, subprocess
r = json.loads(os.environ["RESULT"])
u = r["usage"]
print(json.dumps({
    "commit": subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True).stdout.strip(),
    "binary_bytes": int(os.environ["SIZE"]),
    "replay_render_ms": int(os.environ["RENDER"]),
    "first_turn_input_tokens": u["input_tokens"] + u.get("cache_creation_input_tokens", 0) + u.get("cache_read_input_tokens", 0),
    "probe_cost_usd": r["total_cost_usd"],
}))
EOF
