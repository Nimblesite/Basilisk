#!/usr/bin/env bash
# Benchmark: Basilisk vs Pyright vs mypy vs Pyrefly vs ty
# Uses hyperfine for accurate wall-clock timing.
# Run from the repo root: bash scripts/benchmark.sh
#
# Override fixture scale:  BENCH_SCALE=5000 bash scripts/benchmark.sh

set -euo pipefail

# ─── Fail-fast infrastructure ────────────────────────────────────────────────

fail() {
  printf '\n\033[1;31mFATAL:\033[0m %s\n' "$*" >&2
  exit 1
}

trap 'printf "\n\033[1;31mScript failed at line %d (exit code %d)\033[0m\n" "$LINENO" "$?" >&2' ERR

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BSK="$REPO_ROOT/target/release/basilisk"
FX="$REPO_ROOT/benchmarks/fixtures"
OUT="$REPO_ROOT/benchmarks/results"
mkdir -p "$OUT" "$FX"

# ─── Build release binary ────────────────────────────────────────────────────

echo "Building basilisk release binary..."
cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1
[[ -x "$BSK" ]] || fail "Release binary not found at $BSK after build"
echo "Release binary ready."
echo ""

# ─── Pre-flight: every tool MUST be reachable or we die ──────────────────────

PYRIGHT="python3 -m pyright"
MYPY="python3 -m mypy --ignore-missing-imports --no-error-summary"
PYREFLY="pyrefly"
TY="python3 -m ty"

echo "Pre-flight checks..."
command -v hyperfine &>/dev/null  || fail "hyperfine not installed (brew install hyperfine)"

BSK_VER=$("$BSK" --version 2>&1)       || fail "basilisk --version failed"
PYR_VER=$($PYRIGHT --version 2>&1)     || fail "pyright unavailable — python3 -m pyright --version failed"
MYP_VER=$($MYPY --version 2>&1)        || fail "mypy unavailable — python3 -m mypy --version failed"
PRF_VER=$($PYREFLY --version 2>&1)     || fail "pyrefly unavailable — pip install pyrefly"
TY_VER=$($TY --version 2>&1)           || fail "ty unavailable — python3 -m ty --version failed"

echo "  basilisk : $BSK_VER"
echo "  pyright  : $PYR_VER"
echo "  mypy     : $MYP_VER"
echo "  pyrefly  : $PRF_VER"
echo "  ty       : $TY_VER"
echo "All tools OK."
echo ""

# ─── Generate scaled fixtures ────────────────────────────────────────────────
# Previous fixtures were 50-100 lines — so small that every tool timing was
# pure startup overhead (~500ms pyright, ~150ms mypy, ~95ms pyrefly, ~35ms ty
# regardless of file content). We now generate 2000+ error-site fixtures so
# that actual analysis time dominates process launch cost.

SCALE="${BENCH_SCALE:-2000}"

echo "Generating benchmark fixtures (scale=$SCALE)..."

BENCH_SCALE="$SCALE" BENCH_FX="$FX" python3 << 'PYEOF' || fail "Fixture generation failed"
import os

SCALE = int(os.environ["BENCH_SCALE"])
SCALE_MULTI = max(SCALE // 4, 200)  # multi-line patterns: fewer sites, similar line count
FX = os.environ["BENCH_FX"]

# E0001: Missing parameter type annotations (~3 errors per function)
path = os.path.join(FX, "e0001_missing_param.py")
with open(path, "w") as f:
    f.write(f"# BSK-E0001: Missing parameter type annotations\n")
    f.write(f"# {SCALE} functions, ~{SCALE * 3} untyped params\n\n")
    for i in range(1, SCALE + 1):
        f.write(f"def f{i}(a, b, c) -> int: return 0\n")
print(f"  {os.path.basename(path)}: {SCALE} functions ({SCALE} lines)")

# E0002: Missing return type annotations
path = os.path.join(FX, "e0002_missing_return.py")
with open(path, "w") as f:
    f.write(f"# BSK-E0002: Missing return type annotations\n")
    f.write(f"# {SCALE} functions without return type\n\n")
    for i in range(1, SCALE + 1):
        f.write(f"def f{i}(a: int, b: int): return a + b\n")
print(f"  {os.path.basename(path)}: {SCALE} functions ({SCALE} lines)")

# E0016: Incompatible method overrides (~4 lines each)
path = os.path.join(FX, "e0016_incompatible_override.py")
with open(path, "w") as f:
    f.write(f"# BSK-E0016: Incompatible method override\n")
    f.write(f"# {SCALE_MULTI} child classes with wrong param types\n\n")
    for i in range(1, SCALE_MULTI + 1):
        f.write(f"class Base{i}:\n")
        f.write(f"    def method(self, x: int) -> int: return x\n")
        f.write(f"class Child{i}(Base{i}):\n")
        f.write(f"    def method(self, x: str) -> str: return x\n\n")
print(f"  {os.path.basename(path)}: {SCALE_MULTI} class pairs (~{SCALE_MULTI * 5} lines)")

# E0023: Non-exhaustive match (Literal types, ~7 lines each)
path = os.path.join(FX, "e0023_nonexhaustive_match.py")
with open(path, "w") as f:
    f.write("from typing import Literal\n\n")
    f.write(f"# BSK-E0023: Non-exhaustive match statements\n")
    f.write(f"# {SCALE_MULTI} match blocks missing cases\n\n")
    for i in range(1, SCALE_MULTI + 1):
        f.write(f'T{i} = Literal["a{i}", "b{i}", "c{i}"]\n')
        f.write(f"def check{i}(v: T{i}) -> str:\n")
        f.write(f"    match v:\n")
        f.write(f'        case "a{i}": return "a"\n')
        f.write(f'        case "b{i}": return "b"\n')
        f.write(f'    return "?"\n\n')
print(f"  {os.path.basename(path)}: {SCALE_MULTI} match blocks (~{SCALE_MULTI * 7} lines)")

# E0026: TypeVar with single constraint
path = os.path.join(FX, "e0026_typevar_single_constraint.py")
with open(path, "w") as f:
    f.write("from typing import TypeVar\n\n")
    f.write(f"# BSK-E0026: TypeVar with single constraint\n")
    f.write(f"# {SCALE} single-constraint TypeVars\n\n")
    for i in range(1, SCALE + 1):
        f.write(f'T{i} = TypeVar("T{i}", int)\n')
print(f"  {os.path.basename(path)}: {SCALE} TypeVars ({SCALE} lines)")

# E0022: Unhashable dict key (list literal as key)
path = os.path.join(FX, "e0022_unhashable_dict_key.py")
with open(path, "w") as f:
    f.write(f"# BSK-E0022: Unhashable dict key\n")
    f.write(f"# {SCALE} functions using list literals as dict keys\n\n")
    for i in range(1, SCALE + 1):
        f.write(f'def f{i}() -> None: mapping = {{[{i}, {i + 1}]: "v"}}\n')
print(f"  {os.path.basename(path)}: {SCALE} functions ({SCALE} lines)")

# E0034: Final violations (~4 lines each)
path = os.path.join(FX, "e0034_final_violation.py")
with open(path, "w") as f:
    f.write("from typing import Final\n\n")
    f.write(f"# BSK-E0034: Final variable reassignment\n")
    f.write(f"# {SCALE_MULTI} Final violations\n\n")
    for i in range(1, SCALE_MULTI + 1):
        f.write(f"def bad{i}() -> None:\n")
        f.write(f"    X{i}: Final = {i}\n")
        f.write(f"    X{i} = {i + 1}\n\n")
print(f"  {os.path.basename(path)}: {SCALE_MULTI} violations (~{SCALE_MULTI * 4} lines)")

print("Fixtures generated.")
PYEOF

# Delete the old misnamed fixture
rm -f "$FX/e0030_nondefault_after_default.py"

echo ""

# ─── Fixture definitions ─────────────────────────────────────────────────────

FIXTURES=(
  "e0001_missing_param.py:E0001 Missing param annotations"
  "e0002_missing_return.py:E0002 Missing return annotations"
  "e0016_incompatible_override.py:E0016 Incompatible override"
  "e0023_nonexhaustive_match.py:E0023 Non-exhaustive match"
  "e0026_typevar_single_constraint.py:E0026 TypeVar single constraint"
  "e0022_unhashable_dict_key.py:E0022 Unhashable dict key"
  "e0034_final_violation.py:E0034 Final violation"
)

# Validate every fixture exists BEFORE any benchmarking starts
for entry in "${FIXTURES[@]}"; do
  FILE="${entry%%:*}"
  [[ -f "$FX/$FILE" ]] || fail "Fixture missing after generation: $FX/$FILE"
done

# Smoke test — basilisk must produce diagnostics on the first fixture
SMOKE_OUT=$("$BSK" check "$FX/e0001_missing_param.py" 2>&1 || true)
echo "$SMOKE_OUT" | grep -q "error\[BSK" \
  || fail "Smoke test failed: basilisk produced no BSK errors on e0001_missing_param.py\nOutput:\n$SMOKE_OUT"

echo "======================================================="
echo "  Basilisk Benchmark — $(date '+%Y-%m-%d %H:%M')"
echo "  Fixture scale: $SCALE error sites"
echo "======================================================="
echo ""
echo "Tools:"
echo "  basilisk  : $BSK_VER"
echo "  pyright   : $PYR_VER"
echo "  mypy      : $MYP_VER"
echo "  pyrefly   : $PRF_VER"
echo "  ty        : $TY_VER"
echo ""

# ─── Per-file benchmarks ─────────────────────────────────────────────────────

echo "─── Per-file timing (10 runs each) ─────────────────────────────────────"
echo ""

for entry in "${FIXTURES[@]}"; do
  FILE="${entry%%:*}"
  DESC="${entry##*:}"
  FPATH="$FX/$FILE"
  LINES=$(wc -l < "$FPATH")

  echo "┌─ $DESC ($LINES lines)"
  echo "│  File: $FILE"

  # --ignore-failure is needed because checkers exit non-zero when they
  # find type errors — that's expected behaviour, not a failure.
  HYPER_ARGS=(
    --warmup 2
    --runs 10
    --ignore-failure
    --shell=none
    --export-json "$OUT/${FILE%.py}.json"
    --command-name "basilisk" "$BSK check $FPATH"
    --command-name "pyright"  "python3 -m pyright $FPATH"
    --command-name "mypy"     "python3 -m mypy --ignore-missing-imports --no-error-summary $FPATH"
    --command-name "pyrefly"  "$PYREFLY check $FPATH"
    --command-name "ty"       "python3 -m ty check $FPATH"
  )

  hyperfine "${HYPER_ARGS[@]}" \
    2>&1 | grep -E "^  |Time|Benchmark|faster|slower|Summary" | sed 's/^/│  /'

  echo "└──"
  echo ""
done

# ─── Coverage comparison ─────────────────────────────────────────────────────

echo "─── Diagnostic coverage ─────────────────────────────────────────────────"
echo ""

# count_errors PATTERN CMD [ARGS...]
# Runs a type checker and counts output lines matching PATTERN.
# Checker non-zero exit (found errors) is expected and handled.
# grep exit 1 (no matches) returns 0 count.
# Tool not found (exit 127) is fatal.
count_errors() {
  local pattern="$1"; shift
  local output rc=0
  output=$("$@" 2>&1) || rc=$?
  if [[ $rc -eq 127 ]]; then
    fail "Tool not found when counting errors: $1"
  fi
  local count
  count=$(echo "$output" | grep -c "$pattern") || count=0
  echo "$count"
}

printf "%-42s %8s %8s %8s %8s %8s\n" "Rule / File" "basilisk" "pyright" "mypy" "pyrefly" "ty"
printf "%-42s %8s %8s %8s %8s %8s\n" "─────────────────────────────────────────" "────────" "────────" "────────" "────────" "────────"

for entry in "${FIXTURES[@]}"; do
  FILE="${entry%%:*}"
  FPATH="$FX/$FILE"

  bsk_n=$(count_errors "^error\[BSK"      "$BSK" check "$FPATH")
  pyr_n=$(count_errors "error:"           python3 -m pyright "$FPATH")
  myp_n=$(count_errors "^.*: error:"      python3 -m mypy --ignore-missing-imports --no-error-summary "$FPATH")
  prf_n=$(count_errors "^ERROR"           "$PYREFLY" check "$FPATH")
  ty_n=$(count_errors  "error\["          python3 -m ty check "$FPATH")

  printf "%-42s %8d %8d %8d %8d %8d\n" "${FILE%.py}" "$bsk_n" "$pyr_n" "$myp_n" "$prf_n" "$ty_n"
done

echo ""
echo "NOTE: E0001 (missing param types) and E0002 (missing return types) are"
echo "Basilisk-strict rules. Other tools do not enforce these by default."
echo ""
echo "Done. JSON results saved to $OUT/"
