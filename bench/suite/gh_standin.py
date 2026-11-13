#!/usr/bin/env python3
"""A recording stand-in for `gh` in ops tasks (DESIGN.md), installed on the bench VM as ~/.local/bench-bin/gh (that
directory first on the agents' PATH; the proxy keeps the real GitHub out of reach).

Answers from the task repo's .git/gh-scenario.json, a list of {"match": [tokens], "json": ..., "text": ..., "run": ...,
"exit": n}: the first entry whose tokens all appear in the arguments, in order, wins. `--json` prints "json" (through
`jq -r` with --jq), otherwise "text"; "run" is a shell command run in the repo first (`pr checkout`). Every call goes to
.git/gh-log.jsonl with its arguments and any body read from stdin, for the task's check. Outside an ops task it says
that gh is not available, as on a machine without it.
"""
import json, os, subprocess, sys, time


def common_dir():
    r = subprocess.run(["git", "rev-parse", "--path-format=absolute", "--git-common-dir"], capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else None


def matches(tokens, args):
    i = 0
    for a in args:
        if i < len(tokens) and a == tokens[i]:
            i += 1
    return i == len(tokens)


def main(args):
    d = common_dir()
    scen = os.path.join(d, "gh-scenario.json") if d else ""
    if not os.path.exists(scen):
        print("gh: command not available in this environment", file=sys.stderr)
        return 127
    stdin = sys.stdin.read() if "-" in args and not sys.stdin.isatty() else None
    # a body from a file (`--body-file PATH`, `-F PATH`): its text, for the check to read as it would on GitHub
    path = next((args[i + 1] for i, a in enumerate(args[:-1]) if a in ("--body-file", "-F") and args[i + 1] != "-"), None)
    body_file = open(path, errors="replace").read()[:20000] if path and os.path.isfile(path) else None
    with open(os.path.join(d, "gh-log.jsonl"), "a") as f:
        f.write(json.dumps({"t": time.time(), "args": args, "stdin": stdin, "body_file": body_file}) + "\n")
    entry = next((e for e in json.load(open(scen)) if matches(e["match"], args)), None)
    if entry is None:
        print(f"gh: `gh {' '.join(args[:2])}` failed: HTTP 404: Not Found", file=sys.stderr)
        return 1
    if entry.get("run"):
        top = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True).stdout.strip()
        r = subprocess.run(entry["run"], shell=True, cwd=top or None)
        if r.returncode:
            return r.returncode
    if "--json" in args and "json" in entry:
        out = json.dumps(entry["json"])
        jq = next((args[i + 1] for i, a in enumerate(args[:-1]) if a in ("--jq", "-q")), None)
        if jq:
            out = subprocess.run(["jq", "-r", jq], input=out, capture_output=True, text=True).stdout.rstrip("\n")
        print(out)
    elif entry.get("text"):
        print(entry["text"])
    return entry.get("exit", 0)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
