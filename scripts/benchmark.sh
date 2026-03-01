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

printf 'Building basilisk...'
BUILD_OUT=$(cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1) \
  || { printf ' FAILED\n\n%s\n' "$BUILD_OUT" >&2; exit 1; }
printf ' OK\n'
[[ -x "$BSK" ]] || fail "Release binary not found at $BSK after build"

# ─── Pre-flight ───────────────────────────────────────────────────────────────

command -v hyperfine &>/dev/null  || fail "hyperfine not installed (brew install hyperfine)"

BSK_VER=$("$BSK" --version 2>&1)                                  || fail "basilisk --version failed"
PYR_VER=$(python3 -m pyright --version 2>&1)                      || fail "pyright unavailable"
MYP_VER=$(python3 -m mypy --version 2>&1)                         || fail "mypy unavailable"
PRF_VER=$(pyrefly --version 2>&1)                                  || fail "pyrefly unavailable — pip install pyrefly"
TY_VER=$(python3 -m ty --version 2>&1)                            || fail "ty unavailable"

# ─── Generate scaled fixtures ────────────────────────────────────────────────

SCALE="${BENCH_SCALE:-2000}"

printf 'Generating fixtures (scale=%d)...' "$SCALE"
BENCH_SCALE="$SCALE" BENCH_FX="$FX" python3 << 'PYEOF' >/dev/null || fail "Fixture generation failed"
import os

SCALE = int(os.environ["BENCH_SCALE"])
SCALE_MULTI = max(SCALE // 4, 200)
FX = os.environ["BENCH_FX"]

path = os.path.join(FX, "e0001_missing_param.py")
with open(path, "w") as f:
    f.write(f"# BSK-E0001: Missing parameter type annotations\n")
    f.write(f"# {SCALE} functions, ~{SCALE * 3} untyped params\n\n")
    for i in range(1, SCALE + 1):
        f.write(f"def f{i}(a, b, c) -> int: return 0\n")

path = os.path.join(FX, "e0002_missing_return.py")
with open(path, "w") as f:
    f.write(f"# BSK-E0002: Missing return type annotations\n")
    f.write(f"# {SCALE} functions without return type\n\n")
    for i in range(1, SCALE + 1):
        f.write(f"def f{i}(a: int, b: int): return a + b\n")

path = os.path.join(FX, "e0016_incompatible_override.py")
with open(path, "w") as f:
    f.write(f"# BSK-E0016: Incompatible method override\n")
    f.write(f"# {SCALE_MULTI} child classes with wrong param types\n\n")
    for i in range(1, SCALE_MULTI + 1):
        f.write(f"class Base{i}:\n")
        f.write(f"    def method(self, x: int) -> int: return x\n")
        f.write(f"class Child{i}(Base{i}):\n")
        f.write(f"    def method(self, x: str) -> str: return x\n\n")

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

path = os.path.join(FX, "e0026_typevar_single_constraint.py")
with open(path, "w") as f:
    f.write("from typing import TypeVar\n\n")
    f.write(f"# BSK-E0026: TypeVar with single constraint\n")
    f.write(f"# {SCALE} single-constraint TypeVars\n\n")
    for i in range(1, SCALE + 1):
        f.write(f'T{i} = TypeVar("T{i}", int)\n')

path = os.path.join(FX, "e0022_unhashable_dict_key.py")
with open(path, "w") as f:
    f.write(f"# BSK-E0022: Unhashable dict key\n")
    f.write(f"# {SCALE} functions using list literals as dict keys\n\n")
    for i in range(1, SCALE + 1):
        f.write(f'def f{i}() -> None: mapping = {{[{i}, {i + 1}]: "v"}}\n')

path = os.path.join(FX, "e0034_final_violation.py")
with open(path, "w") as f:
    f.write("from typing import Final\n\n")
    f.write(f"# BSK-E0034: Final variable reassignment\n")
    f.write(f"# {SCALE_MULTI} Final violations\n\n")
    for i in range(1, SCALE_MULTI + 1):
        f.write(f"def bad{i}() -> None:\n")
        f.write(f"    X{i}: Final = {i}\n")
        f.write(f"    X{i} = {i + 1}\n\n")
PYEOF
printf ' OK\n'

# Delete the old misnamed fixture
rm -f "$FX/e0030_nondefault_after_default.py"

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

for entry in "${FIXTURES[@]}"; do
  FILE="${entry%%:*}"
  [[ -f "$FX/$FILE" ]] || fail "Fixture missing after generation: $FX/$FILE"
done

# Smoke test — basilisk must produce diagnostics on the first fixture
SMOKE_OUT=$("$BSK" check "$FX/e0001_missing_param.py" 2>&1 || true)
echo "$SMOKE_OUT" | grep -q "error\[BSK" \
  || fail "Smoke test failed: basilisk produced no BSK errors on e0001_missing_param.py"

# ─── Timings ─────────────────────────────────────────────────────────────────

echo ""
echo "Basilisk Benchmark — $(date '+%Y-%m-%d %H:%M') — scale: $SCALE"
echo "  basilisk $BSK_VER | pyright $PYR_VER | mypy $MYP_VER | pyrefly $PRF_VER | ty $TY_VER"
echo ""

for entry in "${FIXTURES[@]}"; do
  FILE="${entry%%:*}"
  DESC="${entry##*:}"
  FPATH="$FX/$FILE"

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
    2>&1 | grep -E "Time \(mean|fastest|Summary|±" | sed 's/^/  /'

  echo ""
done

# ─── Diagnostic coverage ─────────────────────────────────────────────────────

count_errors() {
  local pattern="$1"; shift
  local output rc=0
  output=$("$@" 2>&1) || rc=$?
  [[ $rc -eq 127 ]] && fail "Tool not found when counting errors: $1"
  echo "$output" | grep -c "$pattern" || echo 0
}

printf "%-42s %8s %8s %8s %8s %8s\n" "Rule / File" "basilisk" "pyright" "mypy" "pyrefly" "ty"
printf "%-42s %8s %8s %8s %8s %8s\n" "─────────────────────────────────────────" "────────" "────────" "────────" "────────" "────────"

for entry in "${FIXTURES[@]}"; do
  FILE="${entry%%:*}"
  FPATH="$FX/$FILE"

  bsk_n=$(count_errors "^error\[BSK"    "$BSK" check "$FPATH")
  pyr_n=$(count_errors "error:"         python3 -m pyright "$FPATH")
  myp_n=$(count_errors "^.*: error:"    python3 -m mypy --ignore-missing-imports --no-error-summary "$FPATH")
  prf_n=$(count_errors "^ERROR"         pyrefly check "$FPATH")
  ty_n=$(count_errors  "error\["        python3 -m ty check "$FPATH")

  printf "%-42s %8d %8d %8d %8d %8d\n" "${FILE%.py}" "$bsk_n" "$pyr_n" "$myp_n" "$prf_n" "$ty_n"
done

echo ""
echo "JSON results saved to $OUT/"
