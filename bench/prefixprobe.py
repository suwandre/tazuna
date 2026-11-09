"""First-call cache read of one `claude -p` call per variant: which flag breaks the shared prompt-cache prefix?
  prefixprobe.py MODEL VARIANT [VARIANT ...]"""
import json, os, subprocess, sys, tempfile
BASE = ["--output-format", "stream-json", "--verbose", "--max-turns", "1"]
V = {
    "stock": ([], {}),
    "stdin": (["--input-format", "stream-json"], {}),
    "permtool": (["--permission-prompt-tool", "stdio", "--allow-dangerously-skip-permissions", "--permission-mode", "bypassPermissions"], {}),
    "ttl5m": ([], {"CLAUDE_CODE_PROMPT_CACHE_TTL": "5m"}),
    "ttl1h": ([], {"CLAUDE_CODE_PROMPT_CACHE_TTL": "1h"}),
    "exclude": (["--exclude-dynamic-system-prompt-sections"], {}),
    "append": (["--append-system-prompt", "Say OK."], {}),
    "sources": (["--setting-sources", "project,local", "--strict-mcp-config"], {}),
    "partial": (["--include-partial-messages"], {}),
}
def tazuna(extra=(), env=None):
    return (["--input-format", "stream-json", "--include-partial-messages", "--permission-prompt-tool", "stdio",
             "--allow-dangerously-skip-permissions", "--permission-mode", "bypassPermissions", "--setting-sources", "project,local",
             "--strict-mcp-config", "--exclude-dynamic-system-prompt-sections", "--append-system-prompt", "Say OK.", *extra],
            env or {"CLAUDE_CODE_PROMPT_CACHE_TTL": "5m"})
V["tazuna"] = tazuna()
V["tazuna-1h"] = tazuna(env={"CLAUDE_CODE_PROMPT_CACHE_TTL": "1h"})
model, names = sys.argv[1], sys.argv[2:]
tok = {l[7:].split("=", 1)[0]: l[7:].split("=", 1)[1].strip().strip('"') for l in open(os.path.expanduser("~/.bench-token")) if l.startswith("export ")}
for n in names:
    flags, extra_env = V[n]
    with tempfile.TemporaryDirectory() as cwd, tempfile.TemporaryDirectory() as cfg:
        subprocess.run(["git", "init", "-q"], cwd=cwd)
        env = {**os.environ, **tok, "CLAUDE_CONFIG_DIR": cfg, **extra_env}
        stdin = None
        cmd = ["claude", "-p", *BASE, "--model", model, *flags]
        if "--input-format" in flags:
            stdin = json.dumps({"type": "user", "message": {"role": "user", "content": "Reply with OK"}}) + "\n"
        else:
            cmd.insert(2, "Reply with OK")
        r = subprocess.run(cmd, cwd=cwd, env=env, input=stdin, capture_output=True, text=True, timeout=300)
        u = None
        for line in r.stdout.splitlines():
            e = json.loads(line) if line.startswith("{") else {}
            if e.get("type") == "assistant":
                u = e["message"]["usage"]; break
            if e.get("type") == "system" and e.get("subtype") == "init":
                tools = e.get("tools")
        print(json.dumps({"variant": n, "read": u and u.get("cache_read_input_tokens"), "write": u and u.get("cache_creation_input_tokens"),
                          "input": u and u.get("input_tokens"), "tools": len(tools or []), "rc": r.returncode,
                          **({} if u else {"err": (r.stderr or r.stdout)[-300:]})}), flush=True)
