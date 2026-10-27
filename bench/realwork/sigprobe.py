#!/usr/bin/env python3
"""Does 0.0.88's bash tool text change how the model designs the prometheus fix? (2026-10-26)

Three of four 0.0.88 prometheus runs (one of four 0.0.87) changed `stripeSeries.gc`'s signature, which the hidden and
base tests call, and failed. The choice is made before any test run, where the two versions differ only in the bash
tool's description. This replays a real run up to just before its first edit and asks the model for its plan, N times
per arm: (a) 0.0.87's tools, (b) 0.0.88's tools, (c) 0.0.88's plus a rule against changing existing signatures. Same
history in every arm (the transcript has no reasoning, so none goes back). The plans are saved for reading.

    sigprobe.py RUN_LABEL CUT_TURN N [ARMS]  -> sigprobe-<label>.jsonl

(d) 0.0.88 with a rule scoped to exported APIs (the first rule, (c), did not stop the internal change and halved the
exported one).
"""
import concurrent.futures as cf, json, os, subprocess, sys, urllib.request
from pathlib import Path

SESS = Path.home() / "bench-homes/cc/.local/share/tazuna/sessions"
REPO, BASE = "/Users/suwandre/bench-repos/prometheus", "63691b23f2cbf56b4f2855554a4513c6a1c6bb29"
MODEL = "deepseek-v4.1-flash-1017"
MID = ("You are a coding agent working in {cwd}. Today is {date}.\n"
       "- Read the relevant code before editing. Search with grep -rn in bash; read whole files with read.\n"
       "- Fix root causes with the smallest change that follows the existing code style.\n"
       "- Add or update tests when behavior changes. Run the relevant tests and fix failures before finishing.\n"
       "- Never run destructive commands (rm -rf outside the working directory, git reset --hard, force push). Do not commit.\n"
       "- Finish with a short summary of what changed and how you verified it.")
RULE = ("\n- Keep the signatures of existing functions and methods that other code or tests call; add a new function or "
        "a wrapper instead, unless the task asks for an API change.")
BASH87 = "Run shell command. Working directory persists between calls."
BASH88 = ("Run shell command. Working directory persists between calls. It stops after 120 s unless you give a timeout in "
          "seconds (up to 1800): give one for long builds and test runs rather than running them in the background.")
RULE2 = ("\n- Do not change exported APIs (public functions, methods, interfaces and types other packages use) unless the "
         "task asks for it; changing unexported code is fine when you update every caller.")
ASK = "Before your next edit, write your plan for the change in a few lines: which functions you will add or modify and how."


def tools(bash_desc, timeout):
    s, i = {"type": "string"}, {"type": "integer"}
    t = [("bash", bash_desc, {"command": s, **({"timeout": i} if timeout else {})}, ["command"]),
         ("read", "Read file: the whole file (up to 2000 lines) unless you give a 1-based offset or a limit. Use it rather than cat, head or sed.",
          {"path": s, "offset": i, "limit": i}, ["path"]),
         ("write", "Write file, creating dirs.", {"path": s, "content": s}, ["path", "content"]),
         ("edit", "Replace exact unique text in file. Use it for every change to an existing file, rather than sed or scripts.",
          {"path": s, "old": s, "new": s}, ["path", "old", "new"])]
    return [{"type": "function", "function": {"name": n, "description": d, "parameters": {"type": "object", "properties": p, "required": r}}}
            for n, d, p, r in t]


def history(label, cut):
    f = next(SESS.glob(f"*rw-*-{label}@*"))
    msgs, turn, cwd, prompt = [], 0, None, None
    for line in f.open():
        e = json.loads(line).get("e", {})
        t = e.get("type")
        if t == "system" and e.get("subtype") == "init":
            cwd = e["cwd"]
        elif t == "harness_user":
            prompt = e["text"]
        elif t == "assistant":
            turn += 1
            if turn >= cut:
                break
            blocks = e["message"]["content"]
            text = "".join(b.get("text", "") for b in blocks if b.get("type") == "text")
            calls = [{"id": b["id"], "type": "function", "function": {"name": b["name"], "arguments": json.dumps(b["input"])}}
                     for b in blocks if b.get("type") == "tool_use"]
            msgs.append({"role": "assistant", "content": text or None, **({"tool_calls": calls} if calls else {})})
        elif t == "user" and turn and isinstance(e["message"].get("content"), list):
            for b in e["message"]["content"]:
                if b.get("type") == "tool_result":
                    c = b.get("content")
                    msgs.append({"role": "tool", "tool_call_id": b["tool_use_id"], "content": c if isinstance(c, str) else json.dumps(c)})
                elif b.get("type") == "text":
                    msgs.append({"role": "user", "content": b["text"]})
    return cwd, prompt, msgs


def system(cwd):
    s = MID.format(cwd=cwd, date="2026-10-26")
    for name in ("CLAUDE.md", "AGENTS.md"):
        text = subprocess.run(["git", "-C", REPO, "show", f"{BASE}:{name}"], capture_output=True, text=True).stdout.strip()
        if text:
            s += f"\n\n## From {name}\n\n{text}"
    return s


def call(body, key):
    req = urllib.request.Request("https://subrouter.ai/v1/chat/completions", json.dumps(body).encode(),
                                 {"Authorization": f"Bearer {key}", "Content-Type": "application/json", "User-Agent": "curl/8"})
    with urllib.request.urlopen(req, timeout=900) as r:
        return json.load(r)


def main():
    label, cut, n = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    key = os.environ["SUBROUTER_ROUTE_DEEPSEEK_V4_1_FLASH_1017_KEY"]
    cwd, prompt, hist = history(label, cut)
    sys0 = system(cwd)
    arms = {"a-0.0.87": (sys0, tools(BASH87, False)), "b-0.0.88": (sys0, tools(BASH88, True)), "c-0.0.88+rule": (sys0 + RULE, tools(BASH88, True)),
            "d-0.0.88+api-rule": (sys0 + RULE2, tools(BASH88, True))}
    only = sys.argv[4].split(",") if len(sys.argv) > 4 else list(arms)  # e.g. d-0.0.88+api-rule
    jobs = [(arm, k) for arm in only for k in range(n)]
    out = Path(__file__).with_name(f"sigprobe-{label}.jsonl")

    def one(job):
        arm, k = job
        s, t = arms[arm]
        body = {"model": MODEL, "messages": [{"role": "system", "content": s}, {"role": "user", "content": prompt}, *hist,
                                             {"role": "user", "content": ASK}], "tools": t, "reasoning_effort": "max",
                # a written plan, not another look around (the smoke test's three samples all went on reading)
                "tool_choice": "none"}
        try:
            r = call(body, key)
            m = r["choices"][0]["message"]
            return dict(arm=arm, k=k, text=m.get("content") or "", reasoning=m.get("reasoning_content") or "",
                        tools=[c["function"] for c in m.get("tool_calls") or []], usage=r.get("usage"))
        except Exception as e:
            return dict(arm=arm, k=k, error=str(e)[:300])

    with cf.ThreadPoolExecutor(6) as ex, out.open("a") as f:
        for r in ex.map(one, jobs):
            f.write(json.dumps(r) + "\n"); f.flush()
            print(r["arm"], r["k"], "error" if "error" in r else f"{len(r['text'])} chars, {len(r['tools'])} tool calls", flush=True)


if __name__ == "__main__":
    main()
