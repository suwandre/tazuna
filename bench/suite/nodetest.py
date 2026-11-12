#!/usr/bin/env python3
"""Hidden tests of a TypeScript change task (Astro), reported as `go test -json` events for run.py's hidden_score.

  nodetest.py [--only expected.json] TEST_FILE...   (run in the task's tree; paths relative to packages/astro)

Rebuilds what the tests import (Astro's unit tests import its dist/): the workspace packages Astro depends on, then
Astro's own sources. Runs each file with node's test runner (types stripped) and reports one event per test case,
named "<file stem> > <describe> > <test>". With --only, just the tests that file lists (the ones the PR's fix turns
from failing to passing).
"""
import json, os, subprocess, sys, tempfile
import xml.etree.ElementTree as ET

PKG = "packages/astro"


def build():
    subprocess.run("corepack pnpm --filter 'astro^...' run build && cd packages/astro && "
                   "corepack pnpm run build:ci",
                   shell=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def cases(node, prefix):
    for c in node:
        if c.tag == "testsuite":
            yield from cases(c, prefix + [c.get("name", "")])
        elif c.tag == "testcase":
            state = "fail" if c.find("failure") is not None or c.find("error") is not None else \
                    "skip" if c.find("skipped") is not None else "pass"
            yield " > ".join(prefix + [c.get("name", "")]).replace("/", "∕"), state


def run(files):
    out = {}
    for f in files:
        stem = os.path.basename(f).split(".")[0]
        with tempfile.NamedTemporaryFile(suffix=".xml") as x:
            subprocess.run(["node", "--experimental-strip-types", "--no-warnings", "--test", "--test-reporter=junit",
                                f"--test-reporter-destination={x.name}", f], cwd=PKG, capture_output=True, text=True, timeout=600)
            try:
                root = ET.parse(x.name).getroot()
            except ET.ParseError:
                root = None
        found = dict(cases(root, [stem])) if root is not None else {}
        if not found:  # the file did not load (a syntax or import error): every test in it fails
            found = {f"{stem} > (file did not run)": "fail"}
        out.update(found)
    return out


def main(argv):
    only = None
    if argv[:1] == ["--only"]:
        only = set(json.load(open(argv[1]))["tests"])
        argv = argv[2:]
    build()
    res = run(argv)
    if only is not None:
        res = {t: res.get(t, "fail") for t in only}  # a hidden test that vanished counts as failed
    for t, a in sorted(res.items()):
        print(json.dumps({"Test": t, "Action": a}))


if __name__ == "__main__":
    main(sys.argv[1:])
