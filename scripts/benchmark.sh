#!/usr/bin/env bash
# Basilisk benchmark
# Run from repo root: bash scripts/benchmark.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BSK="$REPO_ROOT/target/release/basilisk"
FX="$REPO_ROOT/benchmarks/fixtures"
OUT="$REPO_ROOT/benchmarks/results"
mkdir -p "$OUT"

# Build
printf 'Building...'
BUILD_OUT=$(cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1) \
  || { printf ' FAILED\n\n%s\n' "$BUILD_OUT" >&2; exit 1; }
printf ' OK\n\n'

[[ -x "$BSK" ]] || { echo "Binary not found: $BSK" >&2; exit 1; }
command -v hyperfine &>/dev/null || { echo "hyperfine not installed (brew install hyperfine)" >&2; exit 1; }

FIXTURES=(
  "e0002_missing_return.py:E0002 Missing return annotations"
  "e0016_incompatible_override.py:E0016 Incompatible override"
  "e0022_unhashable_dict_key.py:E0022 Unhashable dict key"
  "e0023_nonexhaustive_match.py:E0023 Non-exhaustive match"
  "e0026_typevar_single_constraint.py:E0026 TypeVar single constraint"
)

echo "Basilisk Benchmark — $(date '+%Y-%m-%d %H:%M')"
echo ""

for entry in "${FIXTURES[@]}"; do
  FILE="${entry%%:*}"
  DESC="${entry##*:}"
  FPATH="$FX/$FILE"

  [[ -f "$FPATH" ]] || { echo "── $DESC  [SKIP — fixture not found]"; echo ""; continue; }

  echo "── $DESC"
  hyperfine \
    --warmup 2 \
    --runs 10 \
    --ignore-failure \
    --export-json "$OUT/${FILE%.py}.json" \
    --command-name "basilisk" "$BSK check $FPATH >/dev/null 2>&1" \
    --command-name "pyright"  "python3 -m pyright $FPATH >/dev/null 2>&1" \
    --command-name "mypy"     "python3 -m mypy --ignore-missing-imports --no-error-summary $FPATH >/dev/null 2>&1" \
    --command-name "pyrefly"  "pyrefly check $FPATH >/dev/null 2>&1" \
    --command-name "ty"       "python3 -m ty check $FPATH >/dev/null 2>&1" \
    2>&1 | grep -E "Time \(mean|Summary|faster" | sed 's/^/  /'
  echo ""
done
