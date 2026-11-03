#!/usr/bin/env python3
"""Real-work harness benchmark on recent Go PRs. Same Claude model everywhere, stock harness configs.

  ./run.py --check                         # verify tasks (base fails hidden, solution passes)
  ./run.py --runs 2 --workers 8            # run everything (resumable: skips jobs already in results.jsonl)
  ./run.py --summary

Each harness runs with HOME=~/bench-homes/<harness> (own login, no user extensions/skills/plugins/memory),
Go caches shared so builds stay warm. Claude Code variants also get an empty CLAUDE_CONFIG_DIR.
"""
import argparse, json, os, re, shutil, statistics, subprocess, sys, tempfile, threading, time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
# BENCH_SET=tasks-hard: a separate task set with its own runs and results
SET = os.environ.get("BENCH_SET", "tasks")
TASKS, RUNS, RESULTS = HERE / SET, HERE / ("runs" if SET == "tasks" else f"runs-{SET}"), HERE / ("results.jsonl" if SET == "tasks" else f"results-{SET}.jsonl")
BENCH_HOMES = Path.home() / "bench-homes"
LEAN = HERE.parent  # bench/ (server.mjs, guard.py, addons/)
MODEL, EFFORT = "claude-sonnet-5-5", "high"
TIMEOUT = int(os.environ.get("BENCH_TIMEOUT", 5400))  # per agent run; the hard tasks need more than 90 min
PRICE = dict(inp=2, out=10, cache_r=0.2, cache_w=2.5)  # $/M tokens, Sonnet 5.5 (pi catalog)
GIT_ID = ["-c", "user.email=bench@local", "-c", "user.name=bench"]
LIMIT_RE = re.compile(r"rate.?limit|usage limit|429|overloaded|out of extra usage|limit reached|quota", re.I)
LOCK = threading.Lock()


def go_env():
    r = subprocess.run(["go", "env", "GOPATH", "GOMODCACHE", "GOCACHE"], capture_output=True, text=True, check=True)
    gp, gm, gc = r.stdout.split()
    # ponytail: dedicated build cache (per-workdir builds grew the shared one to ~300GB); delete ~/bench-gocache after runs
    return {"GOPATH": gp, "GOMODCACHE": gm, "GOCACHE": str(Path.home() / "bench-gocache"), "GOTOOLCHAIN": "local"}


GO_ENV = go_env()
_TL = threading.local()  # per-job overrides (sweep: private GOCACHE per run so disk use stays bounded)


def cur_go_env():
    e = dict(GO_ENV)
    if getattr(_TL, "gocache", None):
        e["GOCACHE"] = _TL.gocache
    # each task's own module cache, filled by its setup, and no fetching after it: the shared ~/go/pkg/mod held caddy and
    # rclone versions newer than the tasks' base (with the fixes, fetched by some earlier agent) and runs read the answer
    # there (v237's diff matched the fix byte for byte; 2026-10-24)
    if getattr(_TL, "gomod", None):
        e["GOMODCACHE"], e["GOPROXY"] = str(_TL.gomod), "off"
        # not in workspace mode, which refuses -mod=mod: with it prometheus's go.work stopped every go command until the
        # agent deleted it (2026-10-24 to 2026-10-26)
        if not getattr(_TL, "gowork", False):
            e["GOFLAGS"] = "-mod=mod"
    return e


# Hosts an agent could copy the merged upstream fix from (runs did, 2026-10-24: raw files, the commit API, .patch URLs,
# newer module versions). Agents and harnesses reach the internet through this proxy; these hosts are refused, the rest
# (model APIs, logins) tunnel through. Since 2026-10-28 the jail (JAIL below) also refuses any connection that is not to
# localhost, so a command that drops the proxy variables gets nowhere (pi's model fetched the fix commit with
# `curl --noproxy '*'`, v514); leakscan.py stays the backstop.
BLOCKED = ("github.com", "githubusercontent.com", "proxy.golang.org", "goproxy.io", "goproxy.cn", "pkg.go.dev",
           "sourcegraph.com", "gitlab.com", "codeberg.org")


def blocked(host):
    return any(host == b or host.endswith("." + b) for b in BLOCKED)


def _proxy_conn(c):
    import socket
    try:
        head = b""
        while b"\r\n\r\n" not in head and len(head) < 65536:
            d = c.recv(4096)
            if not d:
                return
            head += d
        parts = head.split(b" ", 2)
        if len(parts) < 2 or parts[0] != b"CONNECT":
            return c.sendall(b"HTTP/1.1 405 Only CONNECT\r\n\r\n")
        host, _, port = parts[1].decode().rpartition(":")
        if blocked(host.strip("[]").lower()):
            return c.sendall(b"HTTP/1.1 403 Blocked by the benchmark\r\n\r\n")
        u = socket.create_connection((host.strip("[]"), int(port)), timeout=30)
        u.settimeout(None)
        c.sendall(b"HTTP/1.1 200 OK\r\n\r\n")
        rest = head.split(b"\r\n\r\n", 1)[1]
        if rest:
            u.sendall(rest)

        def pipe(a, b):
            try:
                while d := a.recv(65536):
                    b.sendall(d)
            except OSError:
                pass
            finally:
                for x in (a, b):
                    try:
                        x.shutdown(socket.SHUT_RDWR)
                    except OSError:
                        pass
        threading.Thread(target=pipe, args=(u, c), daemon=True).start()
        pipe(c, u)
        u.close()
    except (OSError, ValueError):
        pass
    finally:
        c.close()


_PROXY, _PROXY_LOCK = [], threading.Lock()


# Models grep `env` (for Go or proxy settings) and the output goes to the provider with the conversation: four SubRouter
# keys did, 2026-10-28 (pi's own, and three that tazuna runs never used). A harness gets only the credentials it needs.
SECRET = re.compile("KEY|TOKEN|SECRET")


def only_keys(env, *keep):
    return {k: v for k, v in env.items() if k in keep or not SECRET.search(k)}


_RELAYS = {}


def relay(target, key, add=None):
    """A localhost base URL forwarding to `target` with `key` as the bearer token: the harness holds a dummy, so its model
    cannot print the key (pi keeps its key in the environment its tools inherit). Streams the reply as it comes. `add`:
    fields set on JSON request bodies that lack them (Command Code sends no reasoning effort for a BYOK model)."""
    import http.client, http.server, urllib.parse
    with _PROXY_LOCK:
        if (target, key, str(add)) in _RELAYS:
            return _RELAYS[target, key, str(add)]
        u = urllib.parse.urlsplit(target)

        class H(http.server.BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, *a):
                pass

            def do_POST(self):
                body = self.rfile.read(int(self.headers.get("content-length", 0)))
                if add and body:
                    try:
                        b = json.loads(body)
                        if isinstance(b, dict) and "model" in b:
                            body = json.dumps({**add, **b}).encode()
                    except ValueError:
                        pass
                hd = {k: v for k, v in self.headers.items()
                      if k.lower() not in ("host", "authorization", "x-api-key", "content-length", "connection", "accept-encoding")}
                c = http.client.HTTPSConnection(u.hostname, u.port or 443, timeout=900)
                try:
                    c.request(self.command, self.path, body or None, {**hd, "Authorization": f"Bearer {key}"})
                    r = c.getresponse()
                    self.send_response(r.status)
                    for k, v in r.getheaders():
                        if k.lower() not in ("transfer-encoding", "content-length", "connection", "content-encoding"):
                            self.send_header(k, v)
                    self.send_header("Transfer-Encoding", "chunked")
                    self.end_headers()
                    while d := r.read1(65536):
                        self.wfile.write(b"%x\r\n%s\r\n" % (len(d), d))
                        self.wfile.flush()
                    self.wfile.write(b"0\r\n\r\n")
                except OSError:
                    self.close_connection = True
                finally:
                    c.close()
            do_GET = do_POST

        srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
        srv.daemon_threads = True
        srv.handle_error = lambda *a: None  # a client that hangs up between requests
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        _RELAYS[target, key, str(add)] = f"http://127.0.0.1:{srv.server_address[1]}"
        return _RELAYS[target, key, str(add)]


def proxy_url():
    """Start the blocking CONNECT proxy once per bench process (a thread: it dies with run.py)."""
    import socket
    with _PROXY_LOCK:
        if not _PROXY:
            srv = socket.create_server(("127.0.0.1", 0))

            def serve():
                while True:
                    c, _ = srv.accept()
                    threading.Thread(target=_proxy_conn, args=(c,), daemon=True).start()
            threading.Thread(target=serve, daemon=True).start()
            _PROXY.append(f"http://127.0.0.1:{srv.getsockname()[1]}")
    return _PROXY[0]


def proxy_env():
    """The environment of an agent: the blocking proxy, and BENCH_JAIL so sh() runs it inside JAIL."""
    px, local = proxy_url(), "localhost,127.0.0.1,::1"
    return {k: px for k in ("HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy")} | {
        "NO_PROXY": local, "no_proxy": local, "BENCH_JAIL": "1"}


# Places on this machine that hold the tasks' fixes: the bench repos (full history), this directory (reference patches,
# hidden tests, earlier runs' diffs), the shared module cache (newer releases), and transcripts that quote them. OpenCode
# applied the fix commit from ~/bench-repos after the proxy blocked GitHub (2026-10-24). Agents run under sandbox-exec
# with reads of these denied; outbound connections go only to localhost (the proxy, local test servers); their HOME and the work tree are as before.
_H = Path.home()
JAIL_DENY = sorted({json.loads(g.read_text())["repo"] for g in HERE.glob("tasks*/*/git.json")}) + [
    HERE.parent, _H / "go/pkg/mod", _H / ".claude/projects", _H / ".codex/sessions", _H / ".local/share/opencode"]
# any copy of this directory too: a git worktree of the repo in ~/bench-homes held the tasks' reference patches, and a
# run read one (v353, 2026-10-25)
JAIL = "(version 1)(allow default)(deny file-read* " + " ".join(
    f'(subpath "{os.path.realpath(p)}")' for p in JAIL_DENY) + ' (regex #"/bench/realwork(/|$)") (regex #"/solution\\.patch$"))' + \
    '(deny network-outbound (remote ip "*:*"))(allow network-outbound (remote ip "localhost:*"))'  # the proxy, local servers



# Other runs' work trees and transcripts hold their solutions: tazuna v504 ran `git diff` in the Claude Code run beside it
# and copied tests from a killed run's tree left in TMPDIR; pi v518 mined an earlier pi transcript in its own HOME
# (2026-10-28). A run reads only its own work tree among rw-*; the harnesses' logs can be written and listed, not read.
LOGS = r"/bench-homes/[^/]+/(transcripts|\.local/share/(tazuna/sessions|opencode)|\.codex/sessions|\.claude/projects)/"


def jail_for(w, also=None):
    """`also`: one more rw- directory of this run's own (Kimi Code's home with its sessions), closed to the others."""
    return (JAIL + '(deny file-read* (regex #"/T/rw-"))' + f'(allow file-read* (subpath "{os.path.realpath(w)}"))'
            + (f'(allow file-read* (subpath "{os.path.realpath(also)}"))' if also else "")
            + f'(deny file-read-data (regex #"{LOGS}"))')


def env_for(harness, **extra):
    home = BENCH_HOMES / ("cc" if harness.startswith("cc") else harness)
    home.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, **cur_go_env(), "HOME": str(home), "PI_SKIP_VERSION_CHECK": "1", **extra}
    env.update(proxy_env())
    if harness.split("@")[0] == "cc-mid-jg":  # jg wrapper (no answer cache) ahead of the real binary
        env["PATH"] = f"{home / 'bin'}:{env['PATH']}"
    return env


def descendants(root):
    rows = subprocess.run(["ps", "-axo", "pid=,ppid="], capture_output=True, text=True).stdout.split()
    kids = {}
    for pid, ppid in zip(rows[::2], rows[1::2]):
        kids.setdefault(int(ppid), []).append(int(pid))
    out, todo = [], [root]
    while todo:
        for k in kids.get(todo.pop(), []):
            out.append(k)
            todo.append(k)
    return out


def sh(cmd, cwd, timeout=900, env=None, shell=False, stamp=None):
    # own session: on a timeout the whole tree goes (the harness, its engine, servers the agent left running), not only
    # the direct child; an orphaned engine once kept running for a day and a half
    # PWD follows cwd: the inherited one pointed at bench/realwork, which OpenCode took for its project, finding the tasks'
    # reference fixes and hidden tests there (2026-10-24)
    env = {**(env or {**os.environ, **cur_go_env()}), "PWD": str(cwd)}
    if env.get("BENCH_JAIL"):
        cmd = ["sandbox-exec", "-p", jail_for(cwd, env.get("BENCH_JAIL_ALSO")), *(["/bin/sh", "-c", cmd] if shell else cmd)]
        shell = False
    if stamp:  # a timestamped copy of the JSON lines on stdout (stamp.py, outside the jail)
        cmd = [sys.executable, "-I", str(HERE / "stamp.py"), str(stamp), *cmd]
    p = subprocess.Popen(cmd, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, stdin=subprocess.DEVNULL,
                         env=env, shell=shell, start_new_session=True)
    try:
        out, err = p.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        for pid in [p.pid, *descendants(p.pid)]:  # the agent's commands run in process groups of their own
            try:
                os.kill(pid, 9)
            except ProcessLookupError:
                pass
        p.communicate()
        raise
    return subprocess.CompletedProcess(cmd, p.returncode, out, err)


# ---------------------------------------------------------------- task plumbing

def setup_workdir(task, w):
    g = json.loads((task / "git.json").read_text())
    subprocess.run(f"git -C '{g['repo']}' archive {g['base']} | tar -x -C '{w}'", shell=True, check=True)
    sh(["git", "init", "-q"], w); sh(["git", "config", "gc.auto", "0"], w)  # no background gc racing copies of the tree
    sh(["git", "add", "-A"], w); sh(["git", *GIT_ID, "commit", "-qm", "base"], w)
    # the setup fetches the base's modules into the task's own cache (the agent then runs with GOPROXY=off)
    r = sh(g["setup"], w, shell=True, env={**os.environ, **cur_go_env(), "GOPROXY": os.environ.get("GOPROXY", "https://proxy.golang.org,direct")})
    if r.returncode:
        raise RuntimeError("setup failed: " + r.stderr[-500:])


def capture_diff(w):
    """Everything the agent changed since the base commit, committed or not (an agent that commits left an empty
    `git diff --cached` before: lazygit, two planner runs, one tester run)."""
    sh(["git", "add", "-A"], w)
    base = sh(["git", "rev-list", "--max-parents=0", "HEAD"], w).stdout.split()[-1]
    return sh(["git", "diff", "--cached", base], w).stdout


def expected_total(task):
    f = task / "expected.json"
    return json.loads(f.read_text())["hidden_total"] if f.exists() else 1


def hidden_score(task, w):
    """(passed, total, suite_failed) — hidden = PR tests copied over the agent's tree."""
    g = json.loads((task / "git.json").read_text())
    hidden = task / "hidden"
    # Score the agent's code against the PR's tests: in each package that has hidden tests, reset the agent's
    # *_test.go edits to base (agent-added tests can collide with hidden test names). The judge still sees them.
    sh(["git", "add", "-A"], w)
    for pkg in {f.parent.relative_to(hidden) for f in hidden.rglob("*_test.go")}:
        for t in (w / pkg).glob("*_test.go"):
            rel = str(t.relative_to(w))
            if sh(["git", "cat-file", "-e", f"HEAD:{rel}"], w).returncode == 0:
                sh(["git", "checkout", "HEAD", "--", rel], w)
            else:
                t.unlink()
    for f in (p for p in hidden.rglob("*") if p.is_file()):
        dst = w / f.relative_to(hidden)
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(f, dst)
    r = sh(g["hidden_cmd"], w, shell=True, timeout=900)
    final = {}
    for line in r.stdout.splitlines():
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        t, a = e.get("Test"), e.get("Action")
        if t and "/" not in t and a in ("pass", "fail", "skip"):
            final[t] = a
    passed = sum(a == "pass" for a in final.values())
    total = max(len([a for a in final.values() if a != "skip"]), expected_total(task))
    suite = sh(g["suite_cmd"], w, shell=True, timeout=1200)
    return passed, total, int(suite.returncode != 0)


# ---------------------------------------------------------------- harnesses

def mcp_config(env=None):
    """The lean tool server's MCP config, pointing at this checkout's server.mjs."""
    server = {"type": "stdio", "command": "node", "args": [str(LEAN / "server.mjs")], **({"env": env} if env else {})}
    p = Path(tempfile.gettempdir()) / f"tazuna-bench-mcp{'-jg' if env else ''}.json"
    p.write_text(json.dumps({"mcpServers": {"lean": server}}))
    return p


LEAN_TOOLS = ["--mcp-config", str(mcp_config()),
              "--allowedTools", "mcp__lean__bash,mcp__lean__read,mcp__lean__write,mcp__lean__edit"]
LEAN_BASE = ["--setting-sources", "", "--strict-mcp-config", "--disable-slash-commands",
             "--exclude-dynamic-system-prompt-sections"]
MID_PROMPT = """You are a coding agent working in {cwd}. Today is {date}.
- Read the relevant code before editing. Use Grep/Glob to search.
- Fix root causes with the smallest change that follows the existing code style.
- Add or update tests when behavior changes. Run the relevant tests and fix failures before finishing.
- Never run destructive commands (rm -rf outside the working directory, git reset --hard, force push). Do not commit.
- Finish with a short summary of what changed and how you verified it."""


def cc_extra(h, w):
    if h == "cc-nohooks":  # stock Claude Code (its prompt, tools, the repo's CLAUDE.md and skills) with the repo's hooks off
        return ["--settings", '{"disableAllHooks": true}']
    if h == "cc-lean":
        return [*LEAN_BASE, "--system-prompt", "You are a coding agent. Use tools to inspect and change files. Be concise.",
                "--tools", "", *LEAN_TOOLS]
    if h == "cc-mid-jg":
        skill = (HERE / "jevgrep-SKILL.md").read_text().split("---", 2)[2].strip()  # body without frontmatter
        prompt = MID_PROMPT.format(cwd=w, date=time.strftime("%Y-%m-%d")) + "\n\n" + skill.replace(
            "", "")
        prompt = prompt.replace("- Read the relevant code before editing. Use Grep/Glob to search.",
                                "- Before any other search or file read, run one `jg` query describing the behavior the task is about "
                                "(see Jevgrep below), and read its output. Then use Grep/Glob for exact symbols or strings.")
        jg_tools = [LEAN_TOOLS[0], str(mcp_config({"LEAN_MAX_OUTPUT": "120000"})), *LEAN_TOOLS[2:]]  # 120k bash output cap for jg
        return [*LEAN_BASE, "--system-prompt", prompt, "--tools", "Grep,Glob", *jg_tools]
    if h == "cc-mid":
        return [*LEAN_BASE, "--system-prompt", MID_PROMPT.format(cwd=w, date=time.strftime("%Y-%m-%d")),
                "--tools", "Grep,Glob", *LEAN_TOOLS]
    if h == "cc-mid-addons":  # cc-mid + ponytail, graphify, agent-skills plugins; skills need slash commands + Skill tool
        prompt = MID_PROMPT.format(cwd=w, date=time.strftime("%Y-%m-%d")) + "\n\n" + (ADDONS / "graphify/CLAUDE-section.md").read_text()
        base = [a for a in LEAN_BASE if a != "--disable-slash-commands"]
        plugins = [x for p in ("ponytail", "graphify", "agent-skills") for x in ("--plugin-dir", str(ADDONS / p))]
        return [*base, "--system-prompt", prompt, "--tools", "Grep,Glob,Skill", *LEAN_TOOLS, *plugins]
    if h in ("cc-ecc", "cc-superpowers", "cc-caveman"):  # stock cc + one popular plugin, loaded from its checkout
        return ["--plugin-dir", str(ADDONS / h.removeprefix("cc-"))]
    if h == "cc-allplugins":  # stock cc + all five at once, as people install them (GStack's skills are copied in run_cc)
        dirs = [x for p in ("ecc", "superpowers", "caveman", "graphify") for x in ("--plugin-dir", str(ADDONS / p))]
        return ["--append-system-prompt", (ADDONS / "graphify/CLAUDE-section.md").read_text(), *dirs]
    if h == "cc-graphify":  # stock cc + graphify alone (earlier only measured together with ponytail and agent-skills)
        return ["--append-system-prompt", (ADDONS / "graphify/CLAUDE-section.md").read_text(), "--plugin-dir", str(ADDONS / "graphify")]
    if h == "cc-addons":  # stock cc + the same plugins; graphify's CLAUDE.md section appended instead of written into the repo
        plugins = [x for p in ("ponytail", "graphify", "agent-skills") for x in ("--plugin-dir", str(ADDONS / p))]
        return ["--append-system-prompt", (ADDONS / "graphify/CLAUDE-section.md").read_text(), *plugins]
    return []


ADDONS = LEAN / "addons"


def graphify_build(w):
    """Pre-build the code graph (local AST, no LLM), as a user would once per repo; keep it out of the diff."""
    r = sh(["graphify", "update", "."], w, timeout=900)
    if r.returncode:
        raise RuntimeError("graphify update failed: " + (r.stdout + r.stderr)[-500:])
    with (w / ".git/info/exclude").open("a") as f:
        f.write("graphify-out/\n")


def fname(label):
    """A run's label as a file name: ':' (model:effort) is not allowed in Windows file names."""
    return label.replace(":", "~")


def model_effort(h):
    if "@" in h:
        m, _, e = h.split("@", 1)[1].partition(":")
        return m, e or "phased"
    return MODEL, EFFORT


PHASES = [  # (effort, instruction appended / sent as the resumed turn)
    ("high", "Phase 1 of 3 - plan. Explore the relevant code, find the root cause or the places to change, and write a "
             "concrete step-by-step plan including which tests to run. Do not edit any files in this phase."),
    ("medium", "Phase 2 of 3 - implement. Carry out your plan now: make the code changes and add or update tests."),
    ("medium", "Phase 3 of 3 - verify. Run the relevant tests and fix any failures. Check the diff touches only what the "
               "task needs. Finish with a short summary of what changed and how you verified it."),
]


def run_cc_phased(h, w, prompt):
    """One claude -p session split into resumed turns at different effort levels (cache-safe effort switches)."""
    import uuid
    model, _ = model_effort(h)
    sid = str(uuid.uuid4())
    tot = dict(inp=0, cache_w=0, cache_r=0, out=0, turns=0, reported_cost=0.0, final="")
    with tempfile.TemporaryDirectory() as cfg:
        for i, (effort, instr) in enumerate(PHASES):
            text = f"{prompt}\n\n{instr}" if i == 0 else instr
            sess = ["--session-id", sid] if i == 0 else ["--resume", sid]
            cmd = ["claude", "-p", text, "--model", model, "--effort", effort, "--output-format", "json",
                   "--dangerously-skip-permissions", *sess, *cc_extra("cc-mid", w)]
            r = sh(cmd, w, timeout=TIMEOUT, env=env_for(h, CLAUDE_CONFIG_DIR=cfg))
            try:
                d = json.loads(r.stdout.strip().splitlines()[-1])
            except (IndexError, json.JSONDecodeError):
                raise RuntimeError(f"phase {i + 1} rc={r.returncode}: {(r.stdout + r.stderr)[-800:]}")
            if d.get("is_error"):
                raise RuntimeError(f"phase {i + 1}: {d.get('result')}")
            u = d["usage"]
            tot["inp"] += u.get("input_tokens", 0); tot["cache_w"] += u.get("cache_creation_input_tokens", 0)
            tot["cache_r"] += u.get("cache_read_input_tokens", 0); tot["out"] += u.get("output_tokens", 0)
            tot["turns"] += d.get("num_turns", 0)
            tot["reported_cost"] = d.get("total_cost_usd", 0)  # resumed sessions report a running session total
            tot["final"] = d.get("result") or ""
    return tot


def run_cc(h, w, prompt):
    if h.startswith("cc-phased"):
        return run_cc_phased(h, w, prompt)
    model, effort = model_effort(h)
    base = base_name(h)
    if base in ("cc-mid-addons", "cc-addons", "cc-graphify", "cc-allplugins"):
        graphify_build(w)
    with tempfile.TemporaryDirectory() as cfg:
        if base in ("cc-gstack", "cc-allplugins"):  # GStack is a skills folder, not a plugin: its skills go into the run's config dir
            for d in (ADDONS / "gstack").iterdir():
                if (d / "SKILL.md").exists():
                    shutil.copytree(d, Path(cfg) / "skills" / d.name, symlinks=True)
        stream = os.environ.get("BENCH_STREAM")  # keep the full transcript for diagnostics
        # always stream: when claude waits on a background command it emits one result event per wake-up, and the
        # last one alone covers only the last segment (json mode prints only that one)
        cmd = ["claude", "-p", prompt, "--model", model, "--effort", effort, "--output-format", "stream-json", "--verbose",
               "--no-session-persistence", "--dangerously-skip-permissions", *cc_extra(base, w)]
        env = env_for(h, CLAUDE_CONFIG_DIR=cfg)
        if base == "cc-sub":
            # stock Claude Code (the official CLI, its own prompt and tools) on SubRouter's Anthropic route with a
            # non-Anthropic model: the harness DeepSeek, GLM and MiniMax recommend (2026-10-27). Only the SubRouter key
            # goes out (checked against a local stub: Authorization carries it, no x-api-key, no login token)
            env = only_keys(env)
            env.update({"ANTHROPIC_BASE_URL": "https://subrouter.ai", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1",
                        "ANTHROPIC_AUTH_TOKEN": os.environ[os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY")],
                        **{f"ANTHROPIC_DEFAULT_{t}_MODEL": model for t in ("HAIKU", "SONNET", "OPUS")}})
        stamp = None
        if os.environ.get("BENCH_STAMP"):  # Claude Code's timeline, for timesplit.py (2026-10-31)
            stamp = BENCH_HOMES / "cc" / "stamped" / f"{w.name}.jsonl"
            stamp.parent.mkdir(parents=True, exist_ok=True)
        r = sh(cmd, w, timeout=TIMEOUT, env=env, stamp=stamp)
        if base == "cc-sub":  # a transcript where leakscan.py looks (claude keeps none with --no-session-persistence)
            t = BENCH_HOMES / "cc" / "transcripts"
            t.mkdir(parents=True, exist_ok=True)
            (t / f"{w.name}.jsonl").write_text(r.stdout)
    out = sum_results(r.stdout)
    if out is None:
        raise RuntimeError(f"claude rc={r.returncode}: {(r.stdout + r.stderr)[-800:]}")
    return {**out, **({"transcript": r.stdout} if stream else {})}


def sum_results(stdout):
    """Usage and turns summed over every result event of a stream-json run; cost and final text from the last."""
    res = []
    for line in stdout.splitlines():
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        if e.get("type") == "result":
            res.append(e)
    if not res:
        return None
    if res[-1].get("is_error"):
        raise RuntimeError(str(res[-1].get("result")))
    tot = dict(inp=0, cache_w=0, cache_r=0, out=0, turns=0)
    for d in res:
        u = d.get("usage") or {}
        tot["inp"] += u.get("input_tokens", 0); tot["cache_w"] += u.get("cache_creation_input_tokens", 0)
        tot["cache_r"] += u.get("cache_read_input_tokens", 0); tot["out"] += u.get("output_tokens", 0)
        tot["turns"] += d.get("num_turns", 0)
    return dict(**tot, reported_cost=res[-1].get("total_cost_usd", 0), final=res[-1].get("result") or "", segments=len(res))


def parse_pi_events(stdout, stderr, rc):
    r = dict(inp=0, cache_w=0, cache_r=0, out=0, turns=0, reported_cost=0.0, final="")
    for line in stdout.splitlines():
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        m = e.get("message") or {}
        if e.get("type") == "message_end" and m.get("role") == "assistant":
            # a failed call pi retried is not the end of the run (v537 finished after one, 2026-10-28): only the last counts
            r["error"] = str(m.get("errorMessage")) if m.get("stopReason") == "error" or m.get("errorMessage") else None
            u = m.get("usage") or {}
            r["inp"] += u.get("input", 0); r["out"] += u.get("output", 0)
            r["cache_r"] += u.get("cacheRead", 0); r["cache_w"] += u.get("cacheWrite", 0)
            r["reported_cost"] += (u.get("cost") or {}).get("total", 0); r["turns"] += 1
            text = "".join(c.get("text", "") for c in m.get("content", []) if isinstance(c, dict) and c.get("type") == "text")
            r["final"] = text or r["final"]
    if r["turns"] == 0:
        raise RuntimeError(f"rc={rc} no assistant messages: {(stdout + stderr)[-800:]}")
    if err := r.pop("error", None):
        raise RuntimeError(err)
    return r


# pi's own catalog entry for each SubRouter model: (catalog file, id there)
PI_CATALOG = {"deepseek-v4.1-flash-1017": ("deepseek", "deepseek-flash"), "gpt-6.1-sol": ("openai", "gpt-6.1-sol"),
              "glm-5.3": ("zai", "glm-5.3"), "minimax-m3.1-flash": ("minimax", "MiniMax-M3.1-Flash"),
              "gpt-6-astra": ("openai", "gpt-6-astra")}


def pi_entry(model):
    """pi's settings for the model (API, thinking levels, compat, limits, cost) with SubRouter's model id."""
    f, mid = PI_CATALOG[model]
    sub = "node_modules/@earendil-works/pi-ai/dist/providers/data"
    data = next(d / sub for d in Path(shutil.which("pi")).resolve().parents if (d / sub).is_dir())
    e = next(m for ms in json.loads((data / f"{f}.json").read_text()).values() for m in ms.values() if m.get("id") == mid)
    return {k: v for k, v in e.items() if k not in ("provider", "type", "baseUrl")} | {"id": model, "name": model}


def run_pi_sub(h, w, prompt):
    """Stock pi on SubRouter: pi's own catalog entry for the model (API, thinking format and levels, compat) with only
    SubRouter's URL and model id changed, in a fresh agent dir (HOME is the bench home: no user settings, extensions or
    skills). Only the SubRouter key goes out (checked against a local stub, 2026-10-28)."""
    model, effort = model_effort(h)
    # an agent dir per run (PI_CODING_AGENT_DIR): each run.py has its own relay port
    agent = Path(tempfile.mkdtemp(prefix="pi-agent-"))
    entry = pi_entry(model)
    # the Anthropic client adds /v1/messages itself (pi talks to MiniMax that way); the OpenAI ones take the /v1 base
    url = relay("https://subrouter.ai", os.environ[os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY")]) + (
        "" if entry["api"] == "anthropic-messages" else "/v1")
    (agent / "models.json").write_text(json.dumps({"providers": {"subrouter": {"baseUrl": url, "apiKey": "relay",
                                                                         "models": [entry]}}}))
    # the one change from stock: tazuna's patience with provider errors (~16 min) instead of pi's 3 retries in ~14 s, which
    # lost three runs to SubRouter 503s (CPU overload, upstream quota) at 27-45 min on 2026-10-28
    (agent / "settings.json").write_text(json.dumps({"retry": {"enabled": True, "maxRetries": 11, "baseDelayMs": 2000, "maxAgentDelayMs": 120000}}))
    env = only_keys(env_for("pi-sub", PI_TELEMETRY="0", PI_CODING_AGENT_DIR=str(agent)))
    try:
        r = sh(["pi", "-p", prompt, "--mode", "json", "--no-session", "--thinking", effort, "--provider", "subrouter", "--model", model],
               w, timeout=TIMEOUT, env=env)
    finally:
        shutil.rmtree(agent, ignore_errors=True)
    t = BENCH_HOMES / "pi-sub" / "transcripts"  # where leakscan.py looks
    t.mkdir(parents=True, exist_ok=True)
    (t / f"{w.name}.jsonl").write_text(r.stdout)
    return parse_pi_events(r.stdout, r.stderr, r.returncode)


KIMI = Path.home() / "bench-homes/kimi-code-bin/node_modules/.bin/kimi"


def run_kimi_sub(h, w, prompt):
    """Stock Kimi Code (Moonshot's own CLI, 2.4.0) on SubRouter: an OpenAI-type provider through the relay (the CLI holds a
    dummy key), thinking on at the run's effort, a fresh KIMI_CODE_HOME (no user config, skills or sessions; rw-, so other
    runs cannot read it). Headless `-p`, stream-json: no token counts, so cost comes from the account log, one key per harness."""
    model, effort = model_effort(h)
    home = Path(tempfile.mkdtemp(prefix="rw-kimi-"))
    url = relay("https://subrouter.ai/v1", os.environ[os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY")])
    (home / "config.toml").write_text(f"""default_model = "sub/{model}"

[providers.sub]
type = "openai"
base_url = "{url}"
api_key = "relay"

[models."sub/{model}"]
provider = "sub"
model = "{model}"
max_context_size = 262144
capabilities = ["thinking", "tool_use"]
support_efforts = ["{effort}"]
default_effort = "{effort}"

[thinking]
enabled = true
effort = "{effort}"
""")
    env = only_keys(env_for("kimi-sub", KIMI_CODE_HOME=str(home), BENCH_JAIL_ALSO=str(home)))
    try:
        r = sh([str(KIMI), "-p", prompt, "--output-format", "stream-json"], w, timeout=TIMEOUT, env=env)
    finally:
        shutil.rmtree(home, ignore_errors=True)
    t = BENCH_HOMES / "kimi-sub" / "transcripts"  # where leakscan.py looks
    t.mkdir(parents=True, exist_ok=True)
    (t / f"{w.name}.jsonl").write_text(r.stdout)
    msgs = []
    for line in r.stdout.splitlines():
        try:
            msgs.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    final = next((m["content"] for m in reversed(msgs) if m.get("role") == "assistant" and isinstance(m.get("content"), str) and m["content"]), "")
    if r.returncode or not final:
        raise RuntimeError(f"kimi rc={r.returncode}: {(r.stdout[-400:] + r.stderr[-400:])}")
    return dict(inp=0, cache_w=0, cache_r=0, out=0, turns=sum(m.get("role") == "assistant" for m in msgs), reported_cost=0, final=final)


def run_pi_family(h, w, prompt):
    common = ["-p", prompt, "--mode", "json", "--no-session", "--thinking", EFFORT]
    cmd = {
        "pi": ["pi", *common, "--provider", "anthropic", "--model", MODEL],
        "omo": [str(Path.home() / ".bun/bin/omo"), *common, "--provider", "anthropic-subscription", "--model", MODEL],
        "omp": ["omp", *common, "--model", f"anthropic/{MODEL}", "--approval-mode", "yolo"],
    }[base_name(h)]
    r = sh(cmd, w, timeout=TIMEOUT, env=env_for(base_name(h)))
    return parse_pi_events(r.stdout, r.stderr, r.returncode)


def run_hermes(h, w, prompt):
    with tempfile.TemporaryDirectory() as t:
        uf = Path(t) / "usage.json"
        r = sh(["hermes", "-z", prompt, "--provider", "anthropic", "-m", MODEL, "--reasoning", EFFORT, "--yolo",
                "--in", str(w), "--usage-file", str(uf)], w, timeout=TIMEOUT, env=env_for(base_name(h)))
        if not uf.exists():
            raise RuntimeError(f"hermes rc={r.returncode}: {(r.stdout + r.stderr)[-800:]}")
        u = json.loads(uf.read_text())
    if u.get("failed") or not u.get("completed"):
        raise RuntimeError(f"hermes failed: {u.get('turn_exit_reason')} {(r.stdout + r.stderr)[-500:]}")
    a = u.get("auxiliary") or {}
    return dict(inp=u["input_tokens"] + a.get("input_tokens", 0), out=u["output_tokens"] + a.get("output_tokens", 0),
                cache_r=u["cache_read_tokens"] + a.get("cache_read_tokens", 0),
                cache_w=u["cache_write_tokens"] + a.get("cache_write_tokens", 0),
                turns=u.get("api_calls", 0), reported_cost=u.get("estimated_cost_usd", 0), final=r.stdout[-4000:])


def run_codex(h, w, prompt):
    """OpenAI's Codex CLI on the user's own ChatGPT plan (its login copied into a clean CODEX_HOME: no user config)."""
    model, effort = model_effort(h)
    home = BENCH_HOMES / "codex"
    r = sh(["codex", "exec", "--json", "--ephemeral", "--skip-git-repo-check", "--dangerously-bypass-approvals-and-sandbox",
            "-m", model, "-c", f'model_reasoning_effort="{effort}"', prompt], w, timeout=TIMEOUT,
           # only what codex and go need: no provider keys from the parent environment
           env={"PATH": os.environ["PATH"], "HOME": str(home), **cur_go_env(), **proxy_env(), "CODEX_HOME": str(home / ".codex")})
    inp = cached = out = turns = 0
    final = ""
    for line in r.stdout.splitlines():
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        u = e.get("usage") or {}
        if e.get("type") == "turn.completed" and u:
            inp += u.get("input_tokens", 0)
            cached += u.get("cached_input_tokens", 0)
            out += u.get("output_tokens", 0)
        it = e.get("item") or {}
        if it.get("type") in ("command_execution", "file_change", "mcp_tool_call"):
            turns += 1
        if it.get("type") == "agent_message":
            final = it.get("text", "")
        # an "error" event is also Codex's notice of a stream it reconnects ("Reconnecting... 1/5"; v539 finished after
        # one, 2026-10-28): only a failed turn ends the run
        if e.get("type") == "turn.failed":
            raise RuntimeError(f"codex: {str(e)[:800]}")
    if not final and r.returncode:
        raise RuntimeError(f"codex rc={r.returncode}: {(r.stdout + r.stderr)[-800:]}")
    return dict(inp=max(0, inp - cached), cache_r=cached, cache_w=0, out=out, turns=turns, reported_cost=0, final=final)


def run_codex_sub(h, w, prompt):
    """Codex CLI with SubRouter as its model provider (Responses API, the shared models key): the same harness as the
    Codex reference run, another provider, to tell the model from the harness."""
    model, effort = model_effort(h)
    # stock Codex with an API key needs no ChatGPT plan. Its own home (bench-homes/codex holds the user's ChatGPT login,
    # which the model could read), an empty CODEX_HOME per run, the key behind the relay (2026-10-28)
    home = BENCH_HOMES / "codex-sub"
    home.mkdir(parents=True, exist_ok=True)
    url = relay("https://subrouter.ai", os.environ[os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY")]) + "/v1"
    provider = f'{{name="SubRouter",base_url="{url}",env_key="CODEX_RELAY_KEY",wire_api="responses"}}'
    with tempfile.TemporaryDirectory(prefix="codex-home-") as ch:
        r = sh(["codex", "exec", "--json", "--ephemeral", "--skip-git-repo-check", "--dangerously-bypass-approvals-and-sandbox",
                "-m", model, "-c", f'model_reasoning_effort="{effort}"', "-c", 'model_provider="subrouter"',
                "-c", f"model_providers.subrouter={provider}", prompt], w, timeout=TIMEOUT,
               env={"PATH": os.environ["PATH"], "HOME": str(home), **cur_go_env(), **proxy_env(), "CODEX_HOME": ch,
                    "CODEX_RELAY_KEY": "relay"})
    t = home / "transcripts"  # where leakscan.py looks
    t.mkdir(parents=True, exist_ok=True)
    (t / f"{w.name}.jsonl").write_text(r.stdout)
    inp = cached = out = turns = 0
    final = ""
    for line in r.stdout.splitlines():
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        u = e.get("usage") or {}
        if e.get("type") == "turn.completed" and u:
            inp += u.get("input_tokens", 0)
            cached += u.get("cached_input_tokens", 0)
            out += u.get("output_tokens", 0)
        it = e.get("item") or {}
        if it.get("type") in ("command_execution", "file_change", "mcp_tool_call"):
            turns += 1
        if it.get("type") == "agent_message":
            final = it.get("text", "")
        # an "error" event is also Codex's notice of a stream it reconnects ("Reconnecting... 1/5"; v539 finished after
        # one, 2026-10-28): only a failed turn ends the run
        if e.get("type") == "turn.failed":
            raise RuntimeError(f"codex: {str(e)[:800]}")
    if not final and r.returncode:
        raise RuntimeError(f"codex rc={r.returncode}: {(r.stdout + r.stderr)[-800:]}")
    return dict(inp=max(0, inp - cached), cache_r=cached, cache_w=0, out=out, turns=turns, reported_cost=0, final=final)


def run_empryo(h, w, prompt):
    cmd = [str(Path.home() / ".empryo/bin/empryo"), "--headless", "--events", "--model", f"subscriptions/{MODEL}",
           "--effort", EFFORT, "--cwd", str(w), prompt]
    r = sh(cmd, w, timeout=TIMEOUT, env=env_for(base_name(h)))
    last, final, calls = None, None, 0
    for line in r.stdout.splitlines():
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        if e.get("type") == "step" and e.get("tokens"):
            last = e["tokens"]
        elif e.get("type") == "model-call":
            calls += 1
        elif e.get("type") == "done":
            final = e.get("output") or ""
        elif e.get("type") == "error":
            raise RuntimeError(str(e)[:800])
    if last is None or final is None:
        raise RuntimeError(f"empryo rc={r.returncode}: {(r.stdout + r.stderr)[-800:]}")
    cr, cw = last.get("cacheRead", 0), last.get("cacheWrite", 0)  # "input" includes cache read+write
    return dict(inp=max(0, last.get("input", 0) - cr - cw), cache_r=cr, cache_w=cw, out=last.get("output", 0),
                turns=calls, reported_cost=last.get("costUSD", 0), final=final)


HARNESS_BIN = Path(os.environ.get("HARNESS_BIN") or HERE.parent.parent / "target/release/tazuna")  # this repo's release build, or an older one for an A/B


JG = Path.home() / "bench-homes/jg-bin/node_modules/.bin/jg"
JG_NOTE = ("A code search tool is installed: `jg \"<question about what the code does>\"` returns the files that matter, reading "
           "leads and source excerpts. Use it to find where things are before reading files.\n\n")


def run_harness(h, w, prompt):
    """Our Rust harness in headless mode (cc-mid profile, driven over stream-json). Prints claude's result JSON."""
    model, effort = model_effort(h)
    with tempfile.TemporaryDirectory() as cfg, tempfile.TemporaryDirectory() as t:
        pf = Path(t) / "prompt.txt"
        pf.write_text(prompt)
        extra = {"tazuna-noverify": ["--no-verify"], "tazuna-nocap": ["--bash-cap", "0"], "tazuna-repo": ["--trust"],
                 "tazuna-repolean": ["--trust", "--profile", "repo-lean"], "tazuna-lean": ["--profile", "lean"],
                 "tazuna-diag": ["--trust", "--diagnostics"],
                 "tazuna-review": ["--trust", "--self-review"],
                 "tazuna-opusrev": ["--trust", "--review-model", "claude-opus-5-5", "--review-effort", "medium"],
                 "tazuna-ttl5m": ["--trust"], "tazuna-bash8k": ["--trust"], "tazuna-repomap": ["--trust", "--repo-map"], "tazuna-explorer": ["--trust", "--explorer", "sonnet"],
                 "tazuna-explorerhk": ["--trust", "--explorer", "haiku"], "tazuna-dsexplorer": ["--trust", "--data", "any", "--explorer", "deepseek-v4.1-flash@https://api.cheaperinference.com/v1", "--explorer-key-env", "CHEAPER_INFERENCE_API_KEY"], "tazuna-noexplorer": ["--trust", "--no-explorer"], "tazuna-noboss": ["--trust", "--no-boss"], "tazuna-rtall": ["--trust", "--profile", "repo", "--no-boss"], "tazuna-rtcore": ["--trust", "--profile", "repo", "--no-boss", "--repo-tools", "core"], "tazuna-rtk": ["--trust", "--profile", "repo", "--no-boss", "--rtk"], "tazuna-rtbatch": ["--trust", "--profile", "repo", "--no-boss", "--batch-calls"], "tazuna-tscope": ["--trust", "--profile", "repo", "--no-boss", "--test-scope"], "tazuna-hts": ["--trust", "--profile", "repo", "--no-boss"], "tazuna-hnts": ["--trust", "--profile", "repo", "--no-boss", "--no-test-scope"], "tazuna-bhmed": ["--trust", "--profile", "repo", "--boss", "haiku", "--boss-effort", "medium"], "tazuna-bhts": ["--trust", "--profile", "repo", "--boss", "haiku", "--boss-effort", "medium"], "tazuna-besc": ["--trust", "--profile", "repo", "--boss", "haiku", "--boss-effort", "medium", "--boss-escalate"], "tazuna-bhhigh": ["--trust", "--profile", "repo", "--boss", "haiku", "--boss-effort", "high"], "tazuna-bhxhigh": ["--trust", "--profile", "repo", "--boss", "haiku", "--boss-effort", "xhigh"], "tazuna-leanopus": ["--trust", "--profile", "lean", "--no-boss"], "tazuna-bpick": ["--trust", "--profile", "repo", "--boss", "haiku", "--boss-pick"], "tazuna-tester": ["--trust", "--tester", "haiku"], "tazuna-fablerev": ["--trust", "--review-model", "claude-fable-5-1", "--review-effort", "xhigh"],
                 # own loop on CheaperInference (key: CHEAPER_INFERENCE_API_KEY, from ~/.ci-key)
                 "tazuna-boss": ["--trust", "--boss", "haiku"], "tazuna-lensrev": ["--trust", "--review-model", "claude-opus-5-5", "--review-effort", "high"], "tazuna-bossrev": ["--trust", "--boss", "haiku", "--review-model", "claude-opus-5-5", "--review-effort", "high"], "tazuna-advisor": ["--trust", "--advisor", "claude-opus-5-5"], "tazuna-bosssonnet": ["--trust", "--boss", "sonnet"], "tazuna-cascade": ["--trust", "--escalate", "claude-opus-5-5:medium", "--data", "any", "--provider", "https://api.cheaperinference.com/v1", "--key-env", "CHEAPER_INFERENCE_API_KEY"], "tazuna-handmed": ["--trust", "--max-minutes", "30", "--escalate", "claude-opus-5-5:medium", "--data", "any", "--provider", "https://api.cheaperinference.com/v1", "--key-env", "CHEAPER_INFERENCE_API_KEY"], "tazuna-handhigh": ["--trust", "--max-minutes", "30", "--escalate", "claude-opus-5-5:high", "--data", "any", "--provider", "https://api.cheaperinference.com/v1", "--key-env", "CHEAPER_INFERENCE_API_KEY"],
                 "tazuna-own": ["--data", "any", "--provider", "https://api.cheaperinference.com/v1", "--key-env", "CHEAPER_INFERENCE_API_KEY", "--no-escalate"], "tazuna-ownbatch": ["--data", "any", "--provider", "https://api.cheaperinference.com/v1", "--key-env", "CHEAPER_INFERENCE_API_KEY", "--no-escalate"], "tazuna-ownel": ["--data", "any", "--provider", "https://api.experientiallabs.ai/v1", "--key-env", "EXPERIENTIAL_API_KEY", "--no-escalate"], "tazuna-ownbeat": ["--data", "any", "--provider", "https://api.beatapi.io/v1", "--key-env", "BEATAPI_API_KEY", "--no-escalate"], "tazuna-ownuno": ["--data", "any", "--provider", "https://api.unorouter.com/v1", "--key-env", "UNOROUTER_API_KEY", "--no-escalate"], "tazuna-ownunorev": ["--data", "any", "--provider", "https://api.unorouter.com/v1", "--key-env", "UNOROUTER_API_KEY", "--no-escalate", "--self-review"], "tazuna-ownmrt": ["--data", "any", "--provider", "https://mortera.cloud/v1", "--key-env", "MORTERA_API_KEY", "--no-escalate"], "tazuna-ownshnds": ["--data", "any", "--provider", "https://api.shannonapi.xyz/v1", "--key-env", "SHANNON_DS14_KEY", "--no-escalate"], "tazuna-ownshngpt": ["--data", "any", "--provider", "https://api.shannonapi.xyz/v1", "--key-env", "SHANNON_GPT12_KEY", "--no-escalate"], "tazuna-ownshn": ["--data", "any", "--provider", "https://api.shannonapi.xyz/v1", "--key-env", "SHANNON_RUN_KEY", "--no-escalate"], "tazuna-ownsub": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", "SUBROUTER_API_KEY", "--no-escalate"], "tazuna-ownsubla": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", "SUBROUTER_LA_KEY", "--no-escalate"], "tazuna-ownsubl1": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", "SUBROUTER_LUNA1_KEY", "--no-escalate"], "tazuna-ownsubl4": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", "SUBROUTER_LUNA4_KEY", "--no-escalate"], "tazuna-ownsubhk": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", "SUBROUTER_HAIKU_KEY", "--no-escalate"], "tazuna-ownsubglm": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", "SUBROUTER_GLM_KEY", "--no-escalate"], "tazuna-ownsubglmf": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", "SUBROUTER_GLMF_KEY", "--no-escalate"], "tazuna-ownsubm": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", "SUBROUTER_MODELS_KEY", "--no-escalate"], "tazuna-ownsubgate": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", "SUBROUTER_MODELS_KEY", "--no-escalate", "--review-model", "claude-opus-5-5", "--review-effort", "high"], "tazuna-ownsubgp": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", "SUBROUTER_GROK_POOL_KEY", "--no-escalate"], "tazuna-ownsubx": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY"), "--no-escalate"], "tazuna-ownsubxrev": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY"), "--no-escalate", "--review-model", "gpt-6-astra@https://subrouter.ai/v1", "--review-effort", "max", "--review-key-env", os.environ.get("SUBROUTER_REVIEW_KEYENV", "SUBROUTER_VET1_KEY")], "tazuna-ownsubxjg": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY"), "--no-escalate"], "tazuna-ownsubxth": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY"), "--no-escalate"], "tazuna-ownsubxtn": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY"), "--no-escalate"], "tazuna-ownsubxp3": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY"), "--no-escalate"], "tazuna-reviewed": ["--preset", "reviewed", "--data", "any", "--key-env", os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY")], "tazuna-ownsubxopus": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY"), "--no-escalate", "--review-model", "claude-opus-5-5", "--review-effort", "max"], "tazuna-ownsubxr": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY"), "--no-escalate"], "tazuna-ownsubxa2": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY"), "--no-escalate"], "tazuna-ownsubxp2": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY"), "--no-escalate"], "tazuna-ownsubmm": ["--data", "any", "--provider", "https://subrouter.ai/v1", "--key-env", "SUBROUTER_MINIMAX_KEY", "--no-escalate", "--model", "minimax/minimax-m3.1-flash:free"], "tazuna-ownapx": ["--data", "any", "--provider", "https://api.apinex.bond/v1", "--key-env", "APINEX_API_KEY", "--no-escalate"], "tazuna-ownapxfree": ["--data", "any", "--provider", "https://api.apinex.bond/v1", "--key-env", "APINEX_API_KEY", "--no-escalate", "--model", "free/deepseek-v4.1-flash"], "tazuna-ownelide": ["--data", "any", "--provider", "https://api.cheaperinference.com/v1", "--key-env", "CHEAPER_INFERENCE_API_KEY", "--no-escalate"], "tazuna-ownplan": ["--data", "any", "--provider", "https://api.cheaperinference.com/v1", "--key-env", "CHEAPER_INFERENCE_API_KEY", "--no-escalate"], "tazuna-ownboth": ["--data", "any", "--provider", "https://api.cheaperinference.com/v1", "--key-env", "CHEAPER_INFERENCE_API_KEY", "--no-escalate"]}.get(base_name(h), [])
        # tazuna-route: no model/effort flags, so tazuna's router decides
        pin = [] if h.startswith("tazuna-route") else ["--model", model, "--effort", effort]
        # claude settings under test, passed through tazuna's environment
        lever = {"tazuna-ttl5m": {"CLAUDE_CODE_PROMPT_CACHE_TTL": "5m"}, "tazuna-ownelide": {"TAZUNA_ELIDE": "1"}, "tazuna-ownbatch": {"TAZUNA_BATCH": "1"}, "tazuna-ownplan": {"TAZUNA_PLAN": "1"}, "tazuna-ownboth": {"TAZUNA_ELIDE": "1", "TAZUNA_PLAN": "1"}, "tazuna-bash8k": {"BASH_MAX_OUTPUT_LENGTH": "8000"}, "tazuna-ownsubxp2": {"TAZUNA_PROMPT2": "1"}, "tazuna-ownsubxa2": {"TAZUNA_API2": "1"}, "tazuna-ownsubxr": {"TAZUNA_READ_MAX": "12000"}, "tazuna-ownsubxrev": {"TAZUNA_REVIEW_PRICE": "0.2,0.02,1.0"}, "tazuna-ownsubxp3": {"TAZUNA_PROMPT3": "1"}, "tazuna-ownsubxtn": {"TAZUNA_TEST_NUDGE": "1"}, "tazuna-ownsubxth": {"TAZUNA_THOROUGH": "1"}}.get(base_name(h), {})
        env = env_for("cc", CLAUDE_CONFIG_DIR=cfg, **lever)
        if "--key-env" in extra:  # own loop: its key (tazuna drops it from its tools' environment), route keys, claude's login
            keep = [extra[extra.index("--key-env") + 1], "CLAUDE_CODE_OAUTH_TOKEN",
                    *([extra[extra.index("--review-key-env") + 1]] if "--review-key-env" in extra else []),
                    *(r.split()[1] for r in env.get("TAZUNA_ROUTES", "").replace("\n", ";").split(";") if len(r.split()) > 1)]
            env = only_keys(env, *keep)
        jg_dir = None
        if base_name(h) == "tazuna-ownsubxjg":
            # jevgrep (dzhng 0.8.5): its config in a sealed folder of the run's own, a custom endpoint through the relay (the
            # tool holds a dummy key), on PATH; the task tells the model it is there. It sends code excerpts to TypeSafe.
            jg_dir = Path(tempfile.mkdtemp(prefix="rw-jg-"))
            jg_env = {**os.environ, "XDG_CONFIG_HOME": str(jg_dir), "XDG_CACHE_HOME": str(jg_dir)}
            subprocess.run([str(JG), "auth", "--provider", "custom", "--base-url", relay("https://api.typesafe.ai", os.environ["TYPESAFE_API_KEY"]) + "/v1",
                            "--model", "jev-latest", "--stdin"], input="relay", text=True, env=jg_env, capture_output=True, check=True)
            env = {**env, "XDG_CONFIG_HOME": str(jg_dir), "XDG_CACHE_HOME": str(jg_dir), "BENCH_JAIL_ALSO": str(jg_dir),
                   "PATH": f"{JG.parent}:{env['PATH']}"}
            pf.write_text(JG_NOTE + prompt)
        try:
            r = sh([str(HARNESS_BIN), "--run", str(pf), *pin, *extra], w, timeout=TIMEOUT, env=env)
        finally:
            if jg_dir:
                shutil.rmtree(jg_dir, ignore_errors=True)
    try:
        d = json.loads(r.stdout.strip().splitlines()[-1])
    except (IndexError, json.JSONDecodeError):
        raise RuntimeError(f"harness rc={r.returncode}: {(r.stdout + r.stderr)[-800:]}")
    if d.get("is_error"):
        raise RuntimeError(str(d.get("result")))
    u = d["usage"]
    return dict(inp=u.get("input_tokens", 0), cache_w=u.get("cache_creation_input_tokens", 0),
                cache_r=u.get("cache_read_input_tokens", 0), out=u.get("output_tokens", 0),
                turns=d.get("num_turns", 0), reported_cost=d.get("total_cost_usd", 0), final=d.get("result") or "",
                verify=((d.get("tazuna") or {}).get("verify") or {}).get("status"),
                # the key variables the run billed to: real costs come from the provider's account log per key (Codex and
                # tazuna's own usage figures can differ from it, 2026-10-29)
                **{k: extra[extra.index(f"--{k.replace('_', '-')}") + 1] for k in ("key_env", "review_key_env") if f"--{k.replace('_', '-')}" in extra})


def run_opencode(h, w, prompt):
    """The official OpenCode CLI (`opencode run`, auto-approve, JSON events) with the model in `h`, e.g.
    `opencode-v1@deepseek-v4.1-flash-free` (OpenCode Zen's `opencode/` prefix added): OpenCode Zen's free models answer only from within OpenCode
    (403 FreeTierError to any other client), so they are measured in their own harness. Its own clean HOME (no user
    config or plugins); the effort is OpenCode's default for the model."""
    model, _, effort = h.split("@", 1)[1].partition(":")
    model = model if "/" in model else "opencode/" + model  # a / in the label would break the work tree's folder name
    if effort:  # OpenCode sets effort as a model variant (`#max`); without one the model's default (the free DeepSeek: max, high)
        model += "#" + effort
    r = sh(["opencode", "run", "--standalone", "--auto", "--format", "json", "-m", model, prompt], w, timeout=TIMEOUT, env=env_for("opencode"))
    return parse_opencode(r)


def run_opencode_sub(h, w, prompt):
    """Stock OpenCode on SubRouter: an OpenAI-compatible provider through the relay (the CLI holds a dummy key), its config
    inline (OPENCODE_CONFIG_CONTENT, one per run), the run's effort as reasoningEffort, its own bench HOME (its sessions
    there, read by leakscan)."""
    model, effort = model_effort(h)
    url = relay("https://subrouter.ai/v1", os.environ[os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY")])
    cfg = {"provider": {"subrouter": {"npm": "@ai-sdk/openai-compatible", "name": "SubRouter", "options": {"baseURL": url, "apiKey": "relay"},
                                      "models": {model: {"name": model, "reasoning": True, "tool_call": True, "options": {"reasoningEffort": effort}}}}}}
    # its data (the session database) in a sealed folder of the run's own: the jail closes OpenCode's usual one, where other
    # runs' sessions live, and the standalone server would not start without it
    data = Path(tempfile.mkdtemp(prefix="rw-opencode-"))
    env = only_keys(env_for("opencode-sub", OPENCODE_CONFIG_CONTENT=json.dumps(cfg), OPENCODE_DISABLE_AUTOUPDATE="1",
                            XDG_DATA_HOME=str(data), BENCH_JAIL_ALSO=str(data)))
    try:
        r = sh(["opencode", "run", "--standalone", "--auto", "--format", "json", "-m", f"subrouter/{model}", prompt], w, timeout=TIMEOUT, env=env)
    finally:
        shutil.rmtree(data, ignore_errors=True)
    t = BENCH_HOMES / "opencode-sub" / "transcripts"  # where leakscan.py looks
    t.mkdir(parents=True, exist_ok=True)
    (t / f"{w.name}.jsonl").write_text(r.stdout)
    return parse_opencode(r)


def run_cmd_sub(h, w, prompt):
    """Stock Command Code (1.88) on SubRouter, BYOK and local-only: a provider through the relay (the CLI holds a dummy key
    in an env reference), the user's Command Code login copied into a sealed per-run HOME (it refuses to run without one),
    no turn cap (its -p default is 100). It sends no reasoning effort for a BYOK model; the relay adds the run's."""
    model, effort = model_effort(h)
    home = Path(tempfile.mkdtemp(prefix="rw-cmd-"))
    (home / ".commandcode").mkdir()
    shutil.copy(BENCH_HOMES / "command-code-sub" / ".commandcode" / "auth.json", home / ".commandcode" / "auth.json")
    url = relay("https://subrouter.ai", os.environ[os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY")], {"reasoning_effort": effort}) + "/v1"
    (home / ".commandcode" / "providers.json").write_text(json.dumps({"provider": {"subrouter": {"name": "SubRouter", "baseURL": url,
                                                                    "apiKey": "$CMD_RELAY_KEY", "models": {model: {}}}}}))
    env = only_keys({**env_for("command-code-sub", CMD_RELAY_KEY="relay", BENCH_JAIL_ALSO=str(home)), "HOME": str(home)}, "CMD_RELAY_KEY")
    try:
        r = sh(["command-code", "-p", prompt, "--output-format", "json", "--local-only", "--yolo", "--skip-onboarding", "--no-auto-update",
                "--no-session", "--max-turns", "1000", "-m", f"subrouter/{model}"], w, timeout=TIMEOUT, env=env)
    finally:
        shutil.rmtree(home, ignore_errors=True)
    t = BENCH_HOMES / "command-code-sub" / "transcripts"  # where leakscan.py looks
    t.mkdir(parents=True, exist_ok=True)
    (t / f"{w.name}.jsonl").write_text(r.stdout)
    res, turns = {}, 0
    for line in r.stdout.splitlines():
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        turns += e.get("type") == "model_request_end"
        if e.get("type") == "result":
            res = e
    if r.returncode or res.get("subtype") != "success":
        raise RuntimeError(f"command-code rc={r.returncode}: {res.get('error') or (r.stdout + r.stderr)[-600:]}")
    u = res.get("usage") or {}
    return dict(inp=u.get("inputTokens", 0) - u.get("cacheReadTokens", 0), cache_w=u.get("cacheWriteTokens", 0), cache_r=u.get("cacheReadTokens", 0),
                out=u.get("outputTokens", 0), turns=turns, reported_cost=0.0, final=res.get("finalText") or "")


def run_omp_sub(h, w, prompt):
    """Stock oh-my-pi (omp) on SubRouter: pi's catalog entry for the model with omp's thinking format ("openai"; omp has no
    "deepseek" one), through the relay, in a fresh agent dir per run (PI_CODING_AGENT_DIR, rw-: closed to other runs). Its
    JSON events are pi's."""
    model, effort = model_effort(h)
    agent = Path(tempfile.mkdtemp(prefix="rw-omp-"))
    entry = pi_entry(model)
    entry["compat"]["thinkingFormat"] = "openai"
    entry["compat"].pop("requiresReasoningContentOnAssistantMessages", None)
    url = relay("https://subrouter.ai", os.environ[os.environ.get("SUBROUTER_X_KEYENV", "SUBROUTER_MODELS_KEY")]) + "/v1"
    (agent / "models.yml").write_text(json.dumps({"providers": {"subrouter": {"baseUrl": url, "apiKey": "relay", "api": "openai-completions", "models": [entry]}}}))
    env = only_keys(env_for("omp-sub", PI_CODING_AGENT_DIR=str(agent), BENCH_JAIL_ALSO=str(agent)))
    try:
        r = sh(["omp", "-p", "--mode", "json", "--no-session", "--thinking", effort, "--model", f"subrouter/{model}", "--approval-mode", "yolo", prompt],
               w, timeout=TIMEOUT, env=env)
    finally:
        shutil.rmtree(agent, ignore_errors=True)
    t = BENCH_HOMES / "omp-sub" / "transcripts"  # where leakscan.py looks
    t.mkdir(parents=True, exist_ok=True)
    (t / f"{w.name}.jsonl").write_text(r.stdout)
    return parse_pi_events(r.stdout, r.stderr, r.returncode)


def parse_opencode(r):
    tok, turns, final = dict(inp=0, cache_w=0, cache_r=0, out=0), 0, ""
    for line in r.stdout.splitlines():
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        part = e.get("part") or {}
        if e.get("type") == "step_finish":
            t = part.get("tokens") or {}
            c = t.get("cache") or {}
            turns += 1
            tok["inp"] += t.get("input", 0) - c.get("read", 0)
            tok["cache_r"] += c.get("read", 0)
            tok["cache_w"] += c.get("write", 0)
            tok["out"] += t.get("output", 0) + t.get("reasoning", 0)
        elif e.get("type") == "text":
            final = part.get("text") or final
    if not turns:
        raise RuntimeError(f"opencode rc={r.returncode}: {(r.stdout + r.stderr)[-800:]}")
    return dict(tok, turns=turns, reported_cost=0.0, final=final)


PICK_SYSTEM = """You review two candidate patches for the same task, as the maintainer who has to merge one of them. Pick the
one you would merge: it does what the task asks and handles the cases the task implies, it is correct, and it changes no
more than the task needs, in the style of the codebase. Extra features, rewrites of code the task did not touch and
speculative hardening count against a patch, not for it. Each patch comes with the result of the repository's own tests
after it: a patch whose tests failed needs a strong reason to win. Answer with JSON only: {"pick": "A" or "B", "why": "<one sentence>"}"""


def copy_tree(src, dst):
    """A copy of a work tree: the files as they are, the repository through git (a background `git gc` after the setup
    commit packed loose objects while copytree read them: v255 lost one mid-copy; v264's clone failed once)."""
    shutil.copytree(src, dst, symlinks=True, ignore=shutil.ignore_patterns(".git"))
    clone = Path(str(dst) + "-clone")
    for attempt in range(3):
        c = sh(["git", "clone", "-q", "--no-checkout", str(src), str(clone)], src)
        if c.returncode == 0:
            break
        shutil.rmtree(clone, ignore_errors=True)
        time.sleep(5)
    else:
        raise RuntimeError(f"could not clone {src}: " + c.stderr[-300:])
    (clone / ".git").rename(dst / ".git")
    shutil.rmtree(clone, ignore_errors=True)
    sh(["git", "reset", "-q"], dst)  # the index to HEAD; the files are already there


def run_best2(h, w, prompt):
    """Best of two (bench prototype): the same own-loop setup twice, in two copies of the work tree at once; Opus on the
    subscription (no tools, no reference) sees the task, both diffs and each attempt's test result and picks one; the
    pick's changes end up in `w` for scoring. `tazuna-<x>best2-vN@model:effort` runs `tazuna-<x>-vN` twice
    (tazuna-ownsubmbest2: SubRouter's default key; tazuna-ownsubxbest2: the key in SUBROUTER_X_KEYENV)."""
    single = h.replace("best2-", "-", 1)
    task = next(t for t in TASKS.iterdir() if w.name.startswith(f"rw-{t.name}-"))
    w2 = Path(str(w) + "-b")
    copy_tree(w, w2)
    try:
        with ThreadPoolExecutor(2) as ex:
            futs = [ex.submit(run_harness, single, d, prompt) for d in (w, w2)]
            res = []
            for f in futs:
                try:
                    res.append(f.result())
                except Exception as e:  # one attempt failing leaves the other
                    res.append(e)
        ok = [i for i, r in enumerate(res) if isinstance(r, dict)]
        if not ok:
            raise res[0]
        diffs = [capture_diff(d) for d in (w, w2)]
        # the second attempt's changes as a binary-safe patch for moving them into `w`: the plain diff says only
        # "Binary files differ" for a new test archive, which git apply cannot apply (smoke run v252)
        base2 = sh(["git", "rev-list", "--max-parents=0", "HEAD"], w2).stdout.split()[-1]
        patch_b = sh(["git", "diff", "--cached", "--binary", base2], w2).stdout
        pick, why, pick_cost = ok[0], "only one attempt finished", 0.0
        if len(ok) == 2 and diffs[0].strip() and diffs[1].strip():
            status = lambda r: {"passed": "passed", "failed": "FAILED"}.get(r["verify"], r["verify"] or "not run")
            text = (f"# Task\n\n{prompt}\n\n# Patch A (tests: {status(res[0])})\n\n```diff\n{diffs[0][:60000]}\n```\n\n"
                    f"# Patch B (tests: {status(res[1])})\n\n```diff\n{diffs[1][:60000]}\n```")
            r = subprocess.run(["claude", "-p", "--model", "claude-opus-5-5", "--effort", "high", "--output-format", "json",
                                "--no-session-persistence", "--setting-sources", "", "--strict-mcp-config",
                                "--disable-slash-commands", "--tools", "", "--system-prompt", PICK_SYSTEM],
                               input=text, capture_output=True, text=True, timeout=900)
            try:
                out = json.loads(r.stdout)
                t = out.get("result", "")
                p = json.loads(t[t.find("{"):t.rfind("}") + 1])
                pick, why, pick_cost = (0 if p["pick"].strip().upper().startswith("A") else 1), p.get("why", ""), out.get("total_cost_usd") or 0.0
            except (json.JSONDecodeError, KeyError, AttributeError) as e:
                why = f"pick failed ({e}); kept attempt A"
        elif len(ok) == 2 and not diffs[0].strip():
            pick, why = 1, "attempt A changed nothing"
        with (HERE / "best2-picks.jsonl").open("a") as f:  # before moving changes: a failure there keeps the record
            f.write(json.dumps(dict(harness=h, picked="AB"[pick], why=why, verify=[r["verify"] if isinstance(r, dict) else "error" for r in res],
                                    turns=[r["turns"] if isinstance(r, dict) else None for r in res], pick_cost_api=pick_cost)) + "\n")
        # each attempt scored on its own (hidden tests, its diff for the judge) as tazuna-<x>att{a,b}-vN: how often the
        # pick was the better attempt is then measured, not guessed
        for i, d in enumerate((w, w2)):
            if not isinstance(res[i], dict) or not diffs[i].strip():
                continue
            s = Path(str(w) + f"-s{i}")
            try:
                copy_tree(d, s)
                passed, total, regress = hidden_score(task, s)
            finally:
                shutil.rmtree(s, ignore_errors=True)
            att = h.replace("best2-", "att" + "ab"[i] + "-", 1)
            (RUNS / task.name).mkdir(parents=True, exist_ok=True)
            (RUNS / task.name / f"{fname(att)}-r0.diff").write_text(diffs[i])
            model, effort = model_effort(att)
            row = dict(task=task.name, harness=att, run=0, model=model, effort=effort, cost=round(res[i]["reported_cost"], 4),
                       **{k: res[i][k] for k in ("inp", "cache_w", "cache_r", "out", "turns", "reported_cost", "verify")},
                       secs=0, passed=passed, total=total, regress=regress, picked=(i == pick))
            with LOCK:
                with RESULTS.open("a") as f:
                    f.write(json.dumps(row) + "\n")
        if pick == 1:  # the second attempt's changes into the scored tree
            base = sh(["git", "rev-list", "--max-parents=0", "HEAD"], w).stdout.split()[-1]  # an agent may have committed
            sh(["git", "reset", "-q", "--hard", base], w)
            sh(["git", "clean", "-qfd"], w)
            if patch_b.strip():
                a = subprocess.run(["git", "apply", "--binary", "--whitespace=nowarn", "-"], cwd=w, input=patch_b, text=True, capture_output=True)
                if a.returncode:
                    raise RuntimeError("could not apply attempt B: " + a.stderr[-300:])
        parts = [r for r in res if isinstance(r, dict)]
        total = {k: sum(r[k] for r in parts) for k in ("inp", "cache_w", "cache_r", "out", "turns", "reported_cost")}
        return dict(total, final=parts[min(pick, len(parts) - 1)]["final"] if len(parts) == 2 else parts[0]["final"],
                    verify=res[pick]["verify"] if isinstance(res[pick], dict) else None)
    finally:
        shutil.rmtree(w2, ignore_errors=True)


PLAN_PROMPT = """You are the planner. A cheaper model will implement your plan without your context: it sees only the task and
your plan, and reads only the files the plan names. Explore the code until you know exactly what to change, then write the
plan as your final answer:
- each change: file, function or type, and exactly what to change (code for any non-trivial logic)
- tests to add or update: file, test name, what it asserts
- the exact commands that verify the change
Be as short as precision allows. Do not edit any files."""

EXEC_PROMPT = """{task}

# Plan (from the lead engineer; follow it)
{plan}

Implement exactly this plan. Do not redesign it; if a concrete detail is wrong (a name, a path, a signature), adapt it
minimally. Run the plan's verification commands and fix failures. Finish with a short summary of what you changed and the
test results."""

CHECK_PROMPT = """The implementer has finished. Its summary:
{summary}

The diff against the base:
```diff
{diff}
```
Check it against the task and your plan: anything missing, wrong or broken? Fix it directly and run the tests you need. If
everything is right, change nothing. Finish with a short summary."""


def run_plan_exec(h, w, prompt):
    """Opus plans (no edits), a cheap model executes the plan, the same Opus session checks the result and fixes gaps.
    plan-haiku: Haiku 5.5 high on the claude CLI (subscription only); plan-ds: DeepSeek V4.1 Flash max on tazuna's own loop."""
    import uuid
    model, effort = model_effort(h)
    base = base_name(h)
    sid, out, costs = str(uuid.uuid4()), dict(inp=0, cache_w=0, cache_r=0, out=0, turns=0), {}

    def add(d, key):
        u = d.get("usage") or {}
        out["inp"] += u.get("input_tokens", 0); out["cache_w"] += u.get("cache_creation_input_tokens", 0)
        out["cache_r"] += u.get("cache_read_input_tokens", 0); out["out"] += u.get("output_tokens", 0)
        out["turns"] += d.get("num_turns", 0)
        costs[key] = d.get("total_cost_usd", 0)

    def claude(text, mdl, eff, sess, cfg, key):
        cmd = ["claude", "-p", text, "--model", mdl, "--effort", eff, "--output-format", "stream-json", "--verbose",
               "--dangerously-skip-permissions", *sess]
        r = sh(cmd, w, timeout=TIMEOUT, env=env_for("cc", CLAUDE_CONFIG_DIR=cfg))
        d = sum_results(r.stdout)
        if d is None:
            raise RuntimeError(f"{key} rc={r.returncode}: {(r.stdout + r.stderr)[-600:]}")
        add({"usage": {"input_tokens": d["inp"], "cache_creation_input_tokens": d["cache_w"], "cache_read_input_tokens": d["cache_r"],
                       "output_tokens": d["out"]}, "num_turns": d["turns"], "total_cost_usd": d["reported_cost"]}, key)
        return d["final"]

    with tempfile.TemporaryDirectory() as cfg, tempfile.TemporaryDirectory() as t:
        plan = claude(f"{prompt}\n\n{PLAN_PROMPT}", model, effort, ["--session-id", sid], cfg, "plan")
        task = EXEC_PROMPT.format(task=prompt, plan=plan)
        if base == "plan-ds":
            pf = Path(t) / "prompt.txt"
            pf.write_text(task)
            r = sh([str(HARNESS_BIN), "--run", str(pf), "--model", "deepseek-v4.1-flash", "--effort", "max",
                    "--data", "any", "--provider", "https://api.cheaperinference.com/v1", "--key-env", "CHEAPER_INFERENCE_API_KEY"],
                   w, timeout=TIMEOUT, env=env_for("cc"))
            try:
                d = json.loads(r.stdout.strip().splitlines()[-1])
            except (IndexError, json.JSONDecodeError):
                raise RuntimeError(f"exec rc={r.returncode}: {(r.stdout + r.stderr)[-600:]}")
            add(d, "exec")
            summary = d.get("result") or ""
        else:
            summary = claude(task, "claude-haiku-5-5", "high", [], cfg, "exec")
        precheck = capture_diff(w)
        final = claude(CHECK_PROMPT.format(summary=summary[-4000:], diff=precheck[:30000]), model, effort, ["--resume", sid], cfg, "check_total")
    # the resumed planner session reports a running total: plan + check
    costs["check"] = costs.pop("check_total") - costs["plan"]
    return dict(**out, reported_cost=sum(costs.values()), final=final, plan=plan, precheck_diff=precheck,
                phase_cost={k: round(v, 4) for k, v in costs.items()})


HARNESSES = {"harness": run_harness, "plan-haiku": run_plan_exec, "plan-ds": run_plan_exec, "tazuna": run_harness, "tazuna-instr": run_harness, "tazuna-repo": run_harness, "tazuna-repolean": run_harness, "tazuna-lean": run_harness, "tazuna-diag": run_harness, "tazuna-review": run_harness, "tazuna-opusrev": run_harness, "tazuna-ttl5m": run_harness, "tazuna-repomap": run_harness, "tazuna-explorer": run_harness, "tazuna-explorerhk": run_harness, "tazuna-dsexplorer": run_harness, "tazuna-noexplorer": run_harness, "tazuna-noboss": run_harness, "tazuna-rtall": run_harness, "tazuna-rtcore": run_harness, "tazuna-rtk": run_harness, "tazuna-rtbatch": run_harness, "tazuna-tscope": run_harness, "tazuna-hts": run_harness, "tazuna-hnts": run_harness, "tazuna-bhmed": run_harness, "tazuna-bhts": run_harness, "tazuna-besc": run_harness, "tazuna-bhhigh": run_harness, "tazuna-bhxhigh": run_harness, "tazuna-leanopus": run_harness, "tazuna-bpick": run_harness, "tazuna-tester": run_harness, "tazuna-fablerev": run_harness, "tazuna-own": run_harness, "tazuna-ownbatch": run_harness, "tazuna-ownel": run_harness, "tazuna-ownapx": run_harness, "tazuna-ownuno": run_harness, "tazuna-ownunorev": run_harness, "tazuna-ownmrt": run_harness, "tazuna-ownshnds": run_harness, "tazuna-ownshngpt": run_harness, "tazuna-ownshn": run_harness, "tazuna-ownsub": run_harness, "tazuna-ownsubla": run_harness, "tazuna-ownsubl1": run_harness, "tazuna-ownsubl4": run_harness, "tazuna-ownsubhk": run_harness, "tazuna-ownsubglm": run_harness, "tazuna-ownsubglmf": run_harness, "tazuna-ownsubm": run_harness, "tazuna-ownsubgate": run_harness, "opencode": run_opencode, "tazuna-ownsubmbest2": run_best2, "tazuna-ownsubxbest2": run_best2, "tazuna-ownsubgp": run_harness, "tazuna-ownsubx": run_harness, "tazuna-ownsubxp2": run_harness, "tazuna-ownsubxa2": run_harness, "tazuna-ownsubxr": run_harness, "tazuna-ownsubxrev": run_harness, "tazuna-ownsubxopus": run_harness, "tazuna-reviewed": run_harness, "tazuna-ownsubxp3": run_harness, "tazuna-ownsubxtn": run_harness, "tazuna-ownsubxth": run_harness, "tazuna-ownsubxjg": run_harness, "tazuna-ownsubmm": run_harness, "tazuna-ownbeat": run_harness, "tazuna-ownapxfree": run_harness, "tazuna-ownelide": run_harness, "tazuna-ownplan": run_harness, "tazuna-ownboth": run_harness, "tazuna-boss": run_harness, "tazuna-lensrev": run_harness, "tazuna-bossrev": run_harness, "tazuna-advisor": run_harness, "tazuna-bosssonnet": run_harness, "tazuna-cascade": run_harness, "tazuna-handmed": run_harness, "tazuna-handhigh": run_harness, "tazuna-bash8k": run_harness, "tazuna-noverify": run_harness, "tazuna-nocap": run_harness, "tazuna-route": run_harness, "cc-mid-jg": run_cc, "codex": run_codex, "codexsub": run_codex_sub, "empryo": run_empryo, "pi": run_pi_family, "pi-sub": run_pi_sub, "kimi-sub": run_kimi_sub, "omp-sub": run_omp_sub, "opencode-sub": run_opencode_sub, "cmd-sub": run_cmd_sub, "omp": run_pi_family, "omo": run_pi_family, "hermes": run_hermes,
             "cc": run_cc, "cc-lean": run_cc, "cc-mid": run_cc, "cc-ecc": run_cc, "cc-superpowers": run_cc, "cc-caveman": run_cc,
             "cc-graphify": run_cc, "cc-gstack": run_cc, "cc-allplugins": run_cc}


# ---------------------------------------------------------------- runner

def norm_cost(r):
    return sum(r[k] * PRICE[k] for k in PRICE) / 1e6


GUARD_DIR = Path(__file__).resolve().parent.parent  # bench/


def wait_gate():
    if os.environ.get("BENCH_GATE") == "state":  # gate on the guard's meter reading, ignore the PAUSE file
        pause_at = float(os.environ.get("GUARD_PAUSE_AT", 85))
        while True:
            if (GUARD_DIR / "ABORT").exists():
                raise SystemExit("guard abort")
            try:
                s = json.loads((GUARD_DIR / "guard.state.json").read_text())
                # each limit with headroom for the runs already in flight (a run moves the 5h window ~3%, the week ~0.3%)
                fh, wk = s.get("five_hour", s["util"]), s.get("seven_day", s["util"])
                if fh < pause_at and wk < float(os.environ.get("GUARD_WEEK_AT", 95)) and time.time() - s["at"] < 600:
                    return
                fresh = time.time() - s["at"] < 600  # a stale reading (guard just started) means wait, not skip
                if os.environ.get("BENCH_GATE_EXIT") and fresh:  # over the limit: skip the job instead of waiting for the reset
                    raise SystemExit(f"usage gate: 5h {fh}%, week {wk}%")
            except (OSError, ValueError, KeyError):
                pass
            time.sleep(30)
    while (GUARD_DIR / "PAUSE").exists():
        if (GUARD_DIR / "ABORT").exists():
            raise SystemExit("guard abort")
        time.sleep(30)
    if (GUARD_DIR / "ABORT").exists():
        raise SystemExit("guard abort")


def base_name(h):
    """'tazuna-repo-v10@model:effort' -> 'tazuna-repo': a -vNN suffix labels a tazuna version, same flags."""
    return re.sub(r"-v\d+$", "", h.split("@")[0])


def runner(h):
    return run_cc if h.startswith("cc") else HARNESSES[base_name(h)]


def run_job(task, h, i):
    if os.environ.get("BENCH_PRIVATE_GOCACHE"):
        _TL.gocache = tempfile.mkdtemp(prefix="gocache-")
    try:
        return _run_job(task, h, i)
    finally:
        if getattr(_TL, "gocache", None):
            subprocess.run(["chmod", "-R", "u+w", _TL.gocache], capture_output=True)
            shutil.rmtree(_TL.gocache, ignore_errors=True)
            _TL.gocache = None


def _run_job(task, h, i):
    wait_gate()
    _TL.gomod = BENCH_HOMES / "gomod" / task.name
    g = json.loads((task / "git.json").read_text())
    _TL.gowork = subprocess.run(["git", "-C", g["repo"], "cat-file", "-e", f"{g['base']}:go.work"], capture_output=True).returncode == 0
    for attempt in range(8):
        with tempfile.TemporaryDirectory(prefix=f"rw-{task.name}-{h}-", ignore_cleanup_errors=True) as d:
            w = Path(d).resolve()
            try:
                setup_workdir(task, w)
                t0 = time.time()
                r = runner(h)(h, w, (task / "prompt.txt").read_text())
                r["secs"] = round(time.time() - t0, 1)
                diff = capture_diff(w)
                r["passed"], r["total"], r["regress"] = hidden_score(task, w)
                break
            except Exception as e:
                msg = str(e)
                if LIMIT_RE.search(msg) and attempt < 7:
                    log(f"{task.name} {h} r{i}: limit hit, sleeping 15m ({msg[:150]})")
                    time.sleep(900)
                    continue
                log(f"{task.name} {h} r{i}: ERROR {msg[:600]}")
                try:  # a run stopped by its budget or timeout keeps its work for a look (judge.py reads *.diff only)
                    out = RUNS / task.name
                    out.mkdir(parents=True, exist_ok=True)
                    (out / f"{fname(h)}-r{i}.diff.partial").write_text(capture_diff(w))
                except Exception:
                    pass
                return
    out = RUNS / task.name
    out.mkdir(parents=True, exist_ok=True)
    (out / f"{fname(h)}-r{i}.diff").write_text(diff)
    (out / f"{fname(h)}-r{i}.final.txt").write_text(r.pop("final"))
    model, effort = model_effort(h)
    cost = r["reported_cost"] if "@" in h else norm_cost(r)  # sweep: Claude Code's own per-model pricing
    row = dict(task=task.name, harness=h, run=i, model=model, effort=effort, cost=round(cost, 4), ts=int(t0), **r)
    with LOCK:
        with RESULTS.open("a") as f:
            f.write(json.dumps(row) + "\n")
    log(f"{task.name:<28} {h:<8} r{i}: {r['passed']}/{r['total']} suite_fail={r['regress']} {r['secs']}s "
        f"turns={r['turns']} in={r['inp'] + r['cache_w'] + r['cache_r']} out={r['out']} {r['out'] / max(1, r['secs']):.1f}tok/s ${row['cost']:.3f}")


def log(msg):
    with LOCK:
        print(time.strftime("%H:%M:%S"), msg, flush=True)


def check():
    ok = True
    for task in sorted(p for p in TASKS.iterdir() if p.is_dir()):
        with tempfile.TemporaryDirectory() as d:
            w = Path(d) / "base"; w.mkdir(); setup_workdir(task, w)
            before = hidden_score(task, w)
            w2 = Path(d) / "sol"; w2.mkdir(); setup_workdir(task, w2)
            sh(["git", "apply", str(task / "solution.patch")], w2).check_returncode()
            after = hidden_score(task, w2)
        (task / "expected.json").write_text(json.dumps({"hidden_total": after[1]}))
        good = before[0] < before[1] and after[0] == after[1] and after[2] == 0
        ok &= good
        print(f"{task.name:<28} base {before[0]}/{before[1]}  solution {after[0]}/{after[1]} suite_fail={after[2]}  "
              f"{'ok' if good else 'BAD'}", flush=True)
    sys.exit(0 if ok else 1)


def rescore(task_names):
    """Re-run hidden + suite scoring from saved diffs (after a task's scoring changes)."""
    rows = [json.loads(l) for l in RESULTS.read_text().splitlines()]

    def one(r):
        task = TASKS / r["task"]
        with tempfile.TemporaryDirectory() as d:
            w = Path(d)
            setup_workdir(task, w)
            diff = RUNS / r["task"] / f"{fname(r['harness'])}-r{r['run']}.diff"
            ap = sh(["git", "apply", "--whitespace=nowarn", str(diff)], w)
            if ap.returncode:
                raise RuntimeError(f"apply failed {diff}: {ap.stderr[-300:]}")
            r["passed"], r["total"], r["regress"] = hidden_score(task, w)
        log(f"{r['task']} {r['harness']} r{r['run']}: {r['passed']}/{r['total']} suite_fail={r['regress']}")

    with ThreadPoolExecutor(8) as ex:
        list(ex.map(one, [r for r in rows if r["task"] in task_names]))
    RESULTS.write_text("".join(json.dumps(r) + "\n" for r in rows))


def summarize(rows):
    hs = sorted({r["harness"] for r in rows})
    print(f"{'task':<28} {'harness':<8} {'n':>2} {'pass':>5} {'all':>4} {'sfail':>5} {'secs':>6} {'turns':>5} "
          f"{'in_total':>9} {'hit':>4} {'out':>6} {'tok/s':>5} {'cost$':>7}")
    for task in sorted({r["task"] for r in rows}) + ["ALL"]:
        for h in hs:
            rs = [r for r in rows if r["harness"] == h and task in ("ALL", r["task"])]
            if not rs:
                continue
            med = lambda k: statistics.median(r[k] for r in rs)
            tot = [r["inp"] + r["cache_w"] + r["cache_r"] for r in rs]
            print(f"{task:<28} {h:<8} {len(rs):>2} {sum(r['passed'] for r in rs) / max(1, sum(r['total'] for r in rs)):>5.0%} "
                  f"{sum(r['passed'] == r['total'] for r in rs) / len(rs):>4.0%} {sum(r['regress'] for r in rs):>5} "
                  f"{med('secs'):>6.0f} {med('turns'):>5.0f} {statistics.median(tot):>9.0f} "
                  f"{sum(r['cache_r'] for r in rs) / max(1, sum(tot)):>4.0%} {med('out'):>6.0f} "
                  f"{statistics.median(r['out'] / max(1, r['secs']) for r in rs):>5.1f} {med('cost'):>7.3f}")
        print()
    print("pass = hidden PR tests passed; all = runs passing every hidden test; sfail = runs whose package suite failed;\n"
          "tok/s = output tokens / wall-clock seconds of the whole run (model, tools, tests);\n"
          "cost = tokens x Sonnet 5.5 list price (same table for every harness)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--summary", action="store_true")
    ap.add_argument("--runs", type=int, default=2)
    ap.add_argument("--workers", type=int, default=8)
    ap.add_argument("--harness", default="pi,omp,omo,hermes,empryo,cc,cc-lean,cc-mid")
    ap.add_argument("--tasks", default="")
    ap.add_argument("--rescore", default="", help="comma-separated tasks to rescore from saved diffs")
    ap.add_argument("--skip", default="", help="comma-separated task/harness/run jobs to skip (e.g. in flight elsewhere)")
    a = ap.parse_args()
    if a.check:
        check()
    if a.rescore:
        return rescore(a.rescore.split(","))
    if a.summary:
        return summarize([json.loads(l) for l in RESULTS.read_text().splitlines()])
    if "cc" in a.harness and not os.environ.get("CLAUDE_CODE_OAUTH_TOKEN"):
        sys.exit("CLAUDE_CODE_OAUTH_TOKEN not set")
    done = {(r["task"], r["harness"], r["run"]) for r in map(json.loads, RESULTS.read_text().splitlines())} if RESULTS.exists() else set()
    done |= {(t, h, int(i)) for t, h, i in (s.split("/") for s in a.skip.split(",") if s)}
    tasks = [t for t in sorted(p for p in TASKS.iterdir() if p.is_dir()) if not a.tasks or t.name in a.tasks.split(",")]
    hs = a.harness.split(",")
    # interleave: run index outer, then task, then harness (rotated) so every harness progresses evenly
    jobs = [(t, hs[(j + k) % len(hs)], i) for i in range(a.runs) for k, t in enumerate(tasks) for j in range(len(hs))]
    jobs = [j for j in jobs if (j[0].name, j[1], j[2]) not in done]
    log(f"{len(jobs)} jobs, {a.workers} workers")
    with ThreadPoolExecutor(a.workers) as ex:
        list(ex.map(lambda j: run_job(*j), jobs))
    import leakscan
    leakscan.main()  # flags runs that copied the reference fix; summaries and the judge skip them
    summarize([json.loads(l) for l in RESULTS.read_text().splitlines()])


if __name__ == "__main__":
    main()
