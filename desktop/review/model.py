#!/usr/bin/env python3
"""A scripted model for reviews: an OpenAI-compatible server on 127.0.0.1:1234 (LM Studio's port, so tazuna, the
terminal and the desktop app alike, finds it as a local server: "Mock Coder 7B" in /model). Real tazuna runs against
it, its own loop, tools, permission questions and all; only the model's replies are scripted. $0, no key.

  python3 desktop/review/model.py [--port 1234]
  # a scratch project for the coding flow:
  mkdir -p /tmp/proj && cd /tmp/proj && git init -q && printf 'fn main() {\n    println!("hi");\n}\n' > main.rs \
    && git add -A && git -c user.name=t -c user.email=t@t commit -qm init && tazuna --model mock-coder-7b

What a message gets, by a word in it:
  (anything else)   the coding flow: thinking, `ls`, an edit of main.rs, a failing command, a new NOTES.md, an answer
  guard             auto mode's check: a safe command runs unasked, `git push --force` asks
  fail              a provider error (HTTP 429)
  slow              a reply that streams slowly (busy states, Esc to stop)
  long, closure     a long answer: headings, a table, code, maths, a quote
  weather, bitcoin  a chat that hands the question to the agent ([[agent]])
  haiku             a plain chat answer
"""
import json, sys, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

LONG = """Here is a fuller look at **closures** in JavaScript.

## What a closure is

A closure is a function bundled with the variables of the scope it was made in.

```js
function counter() {
  let n = 0;
  return () => ++n; // n lives on after counter() returns
}
```

| Pattern | Captures | Typical use |
|---|---|---|
| Counter | `n` | ids, sequences |
| Memoize | `cache` | expensive pure functions |

The lookup is constant: $$t_{lookup} = \\Theta(1)$$

> Closures are not magic: they are scope that outlives its call.
""" + "\n\n".join(f"### Section {i}\n\n" + "Lorem ipsum dolor sit amet, consectetur adipiscing elit. " * 6 for i in range(1, 6))

FINAL = """Done. The greeting now says **hello, tazuna**, on two lines:

- `main.rs`: the `println!` changed, and a second line added
- `NOTES.md`: new, a note on the change

`cat nope.txt` failed because the file does not exist; nothing depends on it."""

CHAT = "Sure. A **haiku** is a three-line poem of 5, 7 and 5 syllables:\n\n> An old silent pond\n> a frog jumps into the pond\n> splash! silence again\n"


def sse(w, obj):
    w.write(b"data: " + json.dumps(obj).encode() + b"\n\n")
    w.flush()


def text(w, s, step=24, dt=0.03):
    for i in range(0, len(s), step):
        sse(w, {"choices": [{"delta": {"content": s[i:i + step]}}]})
        time.sleep(dt)


def tool(w, cid, name, args):
    sse(w, {"choices": [{"delta": {"tool_calls": [{"index": 0, "id": cid, "type": "function", "function": {"name": name, "arguments": json.dumps(args)}}]}}]})


class Model(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, format, *args):
        pass

    def reply(self, code, body):
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        self.reply(200, json.dumps({"data": [{"id": "mock-coder-7b"}, {"id": "mock-chat-3b"}]}).encode())

    def do_POST(self):
        req = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
        msgs, tools = req.get("messages", []), req.get("tools", [])
        users = [m for m in msgs if m.get("role") == "user"]
        last = users[-1]["content"] if users else ""
        last = last if isinstance(last, str) else json.dumps(last)
        low = last.lower()
        # the model's turns since the user's message: where the coding flow is
        n = 0
        for m in reversed(msgs):
            if m.get("role") == "user":
                break
            n += m.get("role") == "assistant"
        if "fail" in low:
            return self.reply(429, json.dumps({"error": {"message": "Rate limit reached for mock-coder-7b: 0 of 0 requests left. Try again in 20s.",
                                                         "type": "rate_limit_error", "code": "rate_limit_exceeded"}}).encode())
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Connection", "close")
        self.end_headers()
        w, chatty = self.wfile, any(k in low for k in ("haiku", "weather", "bitcoin", "long", "closure"))
        if "code or general" in low:
            # the router's check: a chat for the chatty words, the agent for the rest
            text(w, "GENERAL" if chatty else "CODE")
        elif "slow" in low:
            text(w, "Thinking this through slowly. " * 30, step=12, dt=0.25)
        elif not tools or chatty:
            if "weather" in low or "bitcoin" in low:
                text(w, "[[agent]]")
            elif "long" in low or "closure" in low:
                text(w, LONG, step=30, dt=0.02)
            else:
                text(w, CHAT)
        elif "guard" in low:
            [lambda: tool(w, "g1", "bash", {"command": "ls -la && rm -rf build"}),
             lambda: tool(w, "g2", "bash", {"command": "git push --force origin main"}),
             lambda: text(w, "Done.")][min(n, 2)]()
        elif n == 0:
            for part in ["The user wants the greeting changed. ", "Look at the folder, ", "edit main.rs, write notes."]:
                sse(w, {"choices": [{"delta": {"reasoning_content": part}}]})
                time.sleep(0.4)
            text(w, "I'll look at the folder first.")
            tool(w, "c1", "bash", {"command": "ls -la"})
        elif n == 1:
            tool(w, "c2", "edit", {"path": "main.rs", "old": '    println!("hi");', "new": '    println!("hello, tazuna");\n    println!("second line");'})
        elif n == 2:
            tool(w, "c3", "bash", {"command": "cat nope.txt"})
        elif n == 3:
            tool(w, "c4", "write", {"path": "NOTES.md", "content": "# Notes\n\nGreeting changed.\n"})
        else:
            text(w, FINAL, step=40, dt=0.05)
        sse(w, {"choices": [], "usage": {"prompt_tokens": 1800 + 600 * n, "completion_tokens": 80 + 40 * n,
                                         "prompt_tokens_details": {"cached_tokens": 1200 if n else 0}}})
        w.write(b"data: [DONE]\n\n")
        w.flush()
        self.close_connection = True


if __name__ == "__main__":
    port = int(sys.argv[sys.argv.index("--port") + 1]) if "--port" in sys.argv else 1234
    print(f"scripted model on http://127.0.0.1:{port}/v1 (Ctrl-C stops it)", flush=True)
    ThreadingHTTPServer(("127.0.0.1", port), Model).serve_forever()
