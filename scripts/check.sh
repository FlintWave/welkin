#!/usr/bin/env bash
set -euo pipefail

cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"

pass=0
fail=0

run() {
    printf '\n=== %s ===\n' "$1"
    shift
    if "$@"; then
        pass=$((pass + 1))
    else
        fail=$((fail + 1))
    fi
}

run "format"       cargo fmt --all -- --check
run "clippy"       cargo clippy --all-targets --all-features -- -D warnings
run "test"         cargo test --all-targets
run "deny"         cargo deny check

printf '\n=== %d passed, %d failed ===\n' "$pass" "$fail"
[ "$fail" -eq 0 ]
