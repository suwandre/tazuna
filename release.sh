#!/bin/bash
# Releases the version in Cargo.toml from any machine (macOS, Linux, or Windows in Git Bash): checks it, tags it (vX.Y.Z)
# and pushes the tag, which starts CI (.github/workflows/release.yml): the release with the version's CHANGELOG notes and
# the builds for every platform. Run on main, committed and pushed. Until 0.1.77 it built macOS here, so only on a Mac.
set -euo pipefail
cd "$(dirname "$0")"
v=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
[ "$(git branch --show-current)" = main ] || { echo "release from main"; exit 1; }
[ -z "$(git status --porcelain -- src Cargo.toml Cargo.lock)" ] || { echo "src/Cargo changes not committed"; exit 1; }
git fetch -q origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || { echo "push main first"; exit 1; }
git rev-parse -q --verify "refs/tags/v$v" >/dev/null && { echo "v$v is tagged already: bump the version"; exit 1; }
git ls-remote --exit-code --tags origin "refs/tags/v$v" >/dev/null && { echo "v$v is tagged on GitHub already: bump the version"; exit 1; }
# the release notes: without a section CI would publish just "tazuna X"
grep -q "^## $v " CHANGELOG.md || { echo "CHANGELOG.md has no \"## $v \" section (the release notes)"; exit 1; }
cargo build --release -q
[ "$(target/release/tazuna --version | tr -d '\r')" = "tazuna $v" ] || { echo "the build does not say $v"; exit 1; }
git tag "v$v" && git push -q origin "v$v"
echo "v$v pushed: CI creates the release and builds macOS, Linux and Windows (gh run watch); then \`tazuna update\`"
