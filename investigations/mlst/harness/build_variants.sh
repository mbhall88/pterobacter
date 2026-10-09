#!/usr/bin/env bash
# Build each probe variant into its own target directory so binary sizes are comparable.
set -euo pipefail
cd "$(dirname "$0")/../probe"
build() { local name=$1; shift; CARGO_TARGET_DIR=target/$name cargo build --release "$@"; }
build core
build exact-bio --features engine-exact-bio
build exact-ba --features engine-exact-blockaligner
build minimap2 --features engine-minimap2
build minimap2-pure --features engine-minimap2-pure
if [ "${1:-}" = "--check-macos" ]; then
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    for spec in "core:" "exact-bio:engine-exact-bio" "exact-ba:engine-exact-blockaligner" "minimap2:engine-minimap2" "minimap2-pure:engine-minimap2-pure"; do
      n=${spec%%:*}; f=${spec#*:}
      if [ -n "$f" ]; then args=(--features "$f"); else args=(); fi
      if CARGO_TARGET_DIR=target/chk-$t cargo check --release --target "$t" "${args[@]}" >/dev/null 2>"target/chk-$t.$n.err"; then echo "$t $n: ok"; else echo "$t $n: FAILED ($(grep -m1 -E '^error' target/chk-$t.$n.err))"; fi
    done
  done
fi
