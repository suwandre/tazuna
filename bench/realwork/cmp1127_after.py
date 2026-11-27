#!/usr/bin/env python3
"""What the compaction A/B's 80k runs did around each compaction (did one lose the thread?): for each session recording of
the arm with a compact_boundary, the last tool calls before it, the boundary's numbers, the first ones after it (failed
edits, files read again, the same command again) and the run's last words; `--summaries` prints the summaries the model got
(the history file and the old histories beside it)."""
import json, re, sys
from pathlib import Path

HOME = Path.home() / "bench-homes/cc/.local/share/tazuna"
ARM = "tazuna-ownmisvcmp-v1235@deepseek-v4-1-flash-1017~high"


def calls(events):
    """(index, kind, text) of each tool call, tool error, compaction and answer, in order."""
    events = [e for e in events if isinstance(e, dict)]
    out = []
    for i, e in enumerate(events):
        t = e.get("type")
        if t == "assistant":
            for b in e["message"].get("content", []):
                if b.get("type") == "tool_use":
                    inp = b.get("input", {})
                    arg = str(inp.get("command") or inp.get("path") or json.dumps(inp)[:80])
                    # without the work tree's path (each command starts with a cd into it)
                    arg = re.sub(r'^cd "?[^"&]*"? && ', "", arg)
                    arg = re.sub(r"/\S*/rw-[^/]*/", "", arg)
                    out.append((i, b["name"], arg.replace("\n", " ")[:120]))
                elif b.get("type") == "text" and b.get("text", "").strip():
                    out.append((i, "said", b["text"].strip().replace("\n", " ")[:160]))
        elif t == "user":
            for b in e["message"].get("content", []) if isinstance(e["message"].get("content"), list) else []:
                if b.get("type") == "tool_result" and b.get("is_error"):
                    out.append((i, "ERROR", str(b.get("content"))[:110].replace("\n", " ")))
        elif t == "system" and e.get("subtype") in ("compact_boundary", "compact_failed", "compact_note"):
            out.append((i, e["subtype"], json.dumps(e.get("compact_metadata") or e.get("message"))))
    return out


def main():
    summaries = "--summaries" in sys.argv
    for f in sorted((HOME / "sessions").glob(f"*rw-*-{ARM}-*.jsonl")):
        events = []
        for l in f.read_text().splitlines():
            try:
                events.append(json.loads(l).get("e", {}))
            except ValueError:  # a line still being written
                continue
        if not any(e.get("subtype") == "compact_boundary" for e in events):
            continue
        task = f.name.split("rw-")[1].split("-tazuna")[0]
        sid = next((e.get("session_id") for e in events if e.get("subtype") == "init"), "?")
        cs = calls(events)
        print(f"\n=== {task} {sid} ({len(cs)} steps)")
        for k, (i, kind, text) in enumerate(cs):
            if kind == "compact_boundary":
                print("  ...")
                for c in cs[max(0, k - 4):k]:
                    print(f"  before  {c[1]:8} {c[2]}")
                print(f"  >>> COMPACTED {text}")
                for c in cs[k + 1:k + 9]:
                    print(f"  after   {c[1]:8} {c[2]}")
        for c in cs[-2:]:
            print(f"  end     {c[1]:8} {c[2]}")
        if summaries and sid != "?":
            main_file = HOME / "own-sessions" / f"{sid}.json"
            if main_file.exists():
                h = json.loads(main_file.read_text())
                chain, b = [h], h.get("before")
                while b and (HOME / "own-sessions" / f"{b}.json").exists():
                    chain.append(json.loads((HOME / "own-sessions" / f"{b}.json").read_text()))
                    b = chain[-1].get("before")
                for n, x in enumerate(chain):
                    first = (x.get("msgs") or [{}])[0].get("content", "")
                    if isinstance(first, str) and first.startswith("[A summary"):
                        print(f"  --- summary {n} ---\n" + first[:2500])


if __name__ == "__main__":
    main()
