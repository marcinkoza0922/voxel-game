#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
log=$(mktemp)
trap 'rm -f "$log"' EXIT
check() {
    if ! "$@" >"$log" 2>&1; then
        tail -60 "$log"
        exit 1
    fi
}
check cargo fmt --check
check cargo clippy --message-format=short -- -D warnings
check cargo nextest run
check cargo run -- --smoke
printf 'Formatting, clippy, nextest, and gameplay smoke passed.\n'
