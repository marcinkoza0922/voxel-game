#!/usr/bin/env bash
# Usage: tools/profile-cpu.sh [output-directory] [repetitions]
set -euo pipefail
cd "$(dirname "$0")/.."
out=${1:-target/performance}
repetitions=${2:-12}
mkdir -p "$out"
cargo build --release
{
    rustc --version
    uname -srmo
    printf 'Command: voxel-game --stress --seed 314159 --frames %s\n' "$repetitions"
    git rev-parse HEAD
    git status --short
} > "$out/environment.txt"
# Always produce usable workload timings, even when perf counters are restricted.
target/release/voxel-game --stress --seed 314159 --frames "$repetitions" --profile "$out/cpu.csv" | tee "$out/summary.txt"
if command -v perf >/dev/null; then
    if perf record -q -e cycles:u --user-callchains --call-graph dwarf -o "$out/perf.data" -- \
        target/release/voxel-game --stress --seed 314159 --frames "$repetitions" > "$out/perf-run.txt" 2> "$out/perf-error.txt"; then
        perf report --stdio --no-children -i "$out/perf.data" --percent-limit 1 > "$out/perf-report.txt"
        printf 'Call stacks: %s/perf-report.txt (raw: perf.data)\n' "$out"
    else
        printf 'CPU timings completed; perf sampling unavailable. Details: %s/perf-error.txt\n' "$out"
    fi
fi
