#!/usr/bin/env bash
# Benchmark: Basilisk vs Pyright vs mypy vs ty
# Uses hyperfine for accurate wall-clock timing.
# Run from the repo root: bash benchmarks/run.sh

set -euo pipefail

BSK="$(dirname "$0")/../target/release/basilisk"
FX="$(dirname "$0")/fixtures"
OUT="$(dirname "$0")/results"
mkdir -p "$OUT"

PYRIGHT="python3 -m pyright"
MYPY="python3 -m mypy --ignore-missing-imports --no-error-summary"
TY="python3 -m ty"

echo "======================================================="
echo "  Basilisk Benchmark — $(date '+%Y-%m-%d %H:%M')"
echo "======================================================="
echo ""
echo "Tools:"
echo "  basilisk  : $("$BSK" --version 2>&1)"
echo "  pyright   : $($PYRIGHT --version 2>&1)"
echo "  mypy      : $($MYPY --version 2>&1)"
echo "  ty        : $($TY --version 2>&1)"
echo ""

# ─── Per-file benchmarks ──────────────────────────────────────────────────────

FIXTURES=(
  "e0001_missing_param.py:E0001 Missing param annotations (350 errors)"
  "e0002_missing_return.py:E0002 Missing return annotations (100 errors)"
  "e0016_incompatible_override.py:E0016 Incompatible override (20 errors)"
  "e0018_undefined_variable.py:E0018 Undefined variable (100 errors)"
  "e0023_nonexhaustive_match.py:E0023 Non-exhaustive match (10 errors)"
  "e0026_typevar_single_constraint.py:E0026 TypeVar single constraint (50 errors)"
  "e0030_nondefault_after_default.py:E0022 Unhashable dict key (50 errors)"
  "e0034_final_violation.py:E0034 Final violation (20 errors)"
)

echo "─── Per-file timing (10 runs each) ─────────────────────────────────────"
echo ""

for entry in "${FIXTURES[@]}"; do
  FILE="${entry%%:*}"
  DESC="${entry##*:}"
  FPATH="$FX/$FILE"

  echo "┌─ $DESC"
  echo "│  File: $FILE"

  hyperfine \
    --warmup 2 \
    --runs 10 \
    --export-json "$OUT/${FILE%.py}.json" \
    --command-name "basilisk" "$BSK check $FPATH" \
    --command-name "pyright"  "$PYRIGHT $FPATH" \
    --command-name "mypy"     "$MYPY $FPATH" \
    --command-name "ty"       "$TY check $FPATH" \
    2>&1 | grep -E "^  |Time|Benchmark|faster|slower|Summary" | sed 's/^/│  /'

  echo "└──"
  echo ""
done

# ─── Coverage comparison ─────────────────────────────────────────────────────

echo "─── Diagnostic coverage ─────────────────────────────────────────────────"
echo ""
printf "%-42s %8s %8s %8s %8s\n" "Rule / File" "basilisk" "pyright" "mypy" "ty"
printf "%-42s %8s %8s %8s %8s\n" "─────────────────────────────────────────" "────────" "────────" "────────" "────────"

for entry in "${FIXTURES[@]}"; do
  FILE="${entry%%:*}"
  DESC="${entry##*:}"
  FPATH="$FX/$FILE"

  bsk_n=$("$BSK" check "$FPATH" 2>/dev/null | grep -c "^error\[BSK" || true)
  pyr_n=$($PYRIGHT "$FPATH" 2>/dev/null | grep -cE "^.+ error$|error:" || true)
  myp_n=$($MYPY "$FPATH" 2>/dev/null | grep -c "^.*: error:" || true)
  ty_n=$($TY check "$FPATH" 2>/dev/null | grep -c "error\[" || true)

  printf "%-42s %8d %8d %8d %8d\n" "${FILE%.py}" "$bsk_n" "$pyr_n" "$myp_n" "$ty_n"
done

echo ""
echo "Done. JSON results saved to $OUT/"
