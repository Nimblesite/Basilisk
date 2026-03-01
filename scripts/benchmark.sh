#!/usr/bin/env bash
# Benchmark: Basilisk vs Pyright vs mypy vs Pyrefly vs ty
# Uses hyperfine for accurate wall-clock timing.
# Run from the repo root: bash scripts/benchmark.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BSK="$REPO_ROOT/target/release/basilisk"
FX="$REPO_ROOT/benchmarks/fixtures"
OUT="$REPO_ROOT/benchmarks/results"
mkdir -p "$OUT"

# ─── Build release binary ────────────────────────────────────────────────────
echo "Building basilisk release binary..."
cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1
if [[ ! -x "$BSK" ]]; then
  echo "ERROR: Release binary not found at $BSK after build."
  exit 1
fi
echo "Release binary ready."
echo ""

PYRIGHT="python3 -m pyright"
MYPY="python3 -m mypy --ignore-missing-imports --no-error-summary"
PYREFLY="python3 -m pyrefly"
TY="python3 -m ty"

# Detect available tools
HAVE_PYREFLY=false
if $PYREFLY --version &>/dev/null; then
  HAVE_PYREFLY=true
fi

echo "======================================================="
echo "  Basilisk Benchmark — $(date '+%Y-%m-%d %H:%M')"
echo "======================================================="
echo ""
echo "Tools:"
echo "  basilisk  : $("$BSK" --version 2>&1)"
echo "  pyright   : $($PYRIGHT --version 2>&1)"
echo "  mypy      : $($MYPY --version 2>&1)"
if $HAVE_PYREFLY; then
echo "  pyrefly   : $($PYREFLY --version 2>&1)"
fi
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

  HYPER_ARGS=(
    --warmup 2
    --runs 10
    --ignore-failure
    --export-json "$OUT/${FILE%.py}.json"
    --command-name "basilisk" "$BSK check $FPATH"
    --command-name "pyright"  "$PYRIGHT $FPATH"
    --command-name "mypy"     "$MYPY $FPATH"
  )
  if $HAVE_PYREFLY; then
    HYPER_ARGS+=(--command-name "pyrefly" "$PYREFLY check $FPATH")
  fi
  HYPER_ARGS+=(--command-name "ty" "$TY check $FPATH")

  hyperfine "${HYPER_ARGS[@]}" \
    2>&1 | grep -E "^  |Time|Benchmark|faster|slower|Summary" | sed 's/^/│  /'

  echo "└──"
  echo ""
done

# ─── Coverage comparison ─────────────────────────────────────────────────────

echo "─── Diagnostic coverage ─────────────────────────────────────────────────"
echo ""
if $HAVE_PYREFLY; then
  printf "%-42s %8s %8s %8s %8s %8s\n" "Rule / File" "basilisk" "pyright" "mypy" "pyrefly" "ty"
  printf "%-42s %8s %8s %8s %8s %8s\n" "─────────────────────────────────────────" "────────" "────────" "────────" "────────" "────────"
else
  printf "%-42s %8s %8s %8s %8s\n" "Rule / File" "basilisk" "pyright" "mypy" "ty"
  printf "%-42s %8s %8s %8s %8s\n" "─────────────────────────────────────────" "────────" "────────" "────────" "────────"
fi

for entry in "${FIXTURES[@]}"; do
  FILE="${entry%%:*}"
  FPATH="$FX/$FILE"

  bsk_n=$("$BSK" check "$FPATH" 2>/dev/null | grep -c "^error\[BSK" || true)
  pyr_n=$($PYRIGHT "$FPATH" 2>/dev/null | grep -cE "^.+ error$|error:" || true)
  myp_n=$($MYPY "$FPATH" 2>/dev/null | grep -c "^.*: error:" || true)
  prf_n=0
  if $HAVE_PYREFLY; then
    prf_n=$($PYREFLY check "$FPATH" 2>/dev/null | grep -cE "error|Error" || true)
  fi
  ty_n=$($TY check "$FPATH" 2>/dev/null | grep -c "error\[" || true)

  if $HAVE_PYREFLY; then
    printf "%-42s %8d %8d %8d %8d %8d\n" "${FILE%.py}" "$bsk_n" "$pyr_n" "$myp_n" "$prf_n" "$ty_n"
  else
    printf "%-42s %8d %8d %8d %8d\n" "${FILE%.py}" "$bsk_n" "$pyr_n" "$myp_n" "$ty_n"
  fi
done

echo ""
echo "Done. JSON results saved to $OUT/"
