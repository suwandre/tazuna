#!/bin/bash
# Releases the version in Cargo.toml: tags it (vX.Y.Z), pushes the tag, which starts CI (.github/workflows/release.yml:
# the release with its CHANGELOG notes, Linux and Windows builds), and adds the macOS (Apple silicon) build made here,
# which keeps macOS off CI's minutes. Run on main, committed and pushed, on an Apple-silicon Mac.
set -euo pipefail
cd "$(dirname "$0")"
v=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
[ "$(git branch --show-current)" = main ] || { echo "release from main"; exit 1; }
[ -z "$(git status --porcelain -- src Cargo.toml Cargo.lock)" ] || { echo "src/Cargo changes not committed"; exit 1; }
git fetch -q origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || { echo "push main first"; exit 1; }
git rev-parse -q --verify "refs/tags/v$v" >/dev/null && { echo "v$v is tagged already: bump the version"; exit 1; }
[ "$(uname -sm)" = "Darwin arm64" ] || { echo "the macOS build needs an Apple-silicon Mac"; exit 1; }
cargo build --release -q
[ "$(target/release/tazuna --version)" = "tazuna $v" ] || { echo "the build does not say $v"; exit 1; }
git tag "v$v" && git push -q origin "v$v"
echo "v$v pushed; waiting for CI to create the release"
for _ in $(seq 60); do gh release view "v$v" >/dev/null 2>&1 && break; sleep 5; done
d=$(mktemp -d); tar -C target/release -czf "$d/tazuna-aarch64-apple-darwin.tar.gz" tazuna
gh release upload "v$v" "$d/tazuna-aarch64-apple-darwin.tar.gz"
rm -r "$d"
echo "macOS build uploaded; Linux and Windows follow from CI: gh run watch"
