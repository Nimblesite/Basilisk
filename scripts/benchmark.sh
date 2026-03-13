#!/usr/bin/env bash
# Basilisk benchmark
# Run from repo root: bash scripts/benchmark.sh [--rule PATTERN]
# Examples:
#   bash scripts/benchmark.sh                   # run all rules
#   bash scripts/benchmark.sh --rule e0034      # run only e0034
#   bash scripts/benchmark.sh --rule E0034      # case-insensitive match

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BSK="$REPO_ROOT/target/release/basilisk"
FX="$REPO_ROOT/benchmarks/fixtures"
OUT="$REPO_ROOT/benchmarks/results"
mkdir -p "$OUT"

# Optional rule filter: --rule e0034 or just e0034 as first arg
RULE_FILTER=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --rule) RULE_FILTER="$(echo "$2" | tr '[:upper:]' '[:lower:]')"; shift 2 ;;
    -*) echo "Unknown option: $1" >&2; exit 1 ;;
    *) RULE_FILTER="$(echo "$1" | tr '[:upper:]' '[:lower:]')"; shift ;;
  esac
done

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
  "e0054_final_reassignment.py:E0054 Final reassignment"
)

printf 'Running benchmarks (this takes a while)...\n\n'

for entry in "${FIXTURES[@]}"; do
  FILE="${entry%%:*}"
  LABEL="${entry##*:}"
  # Apply rule filter if specified (match against filename or label, case-insensitive)
  if [[ -n "$RULE_FILTER" ]]; then
    LOWER_FILE="$(echo "$FILE" | tr '[:upper:]' '[:lower:]')"
    LOWER_LABEL="$(echo "$LABEL" | tr '[:upper:]' '[:lower:]')"
    if [[ "$LOWER_FILE" != *"$RULE_FILTER"* && "$LOWER_LABEL" != *"$RULE_FILTER"* ]]; then
      continue
    fi
  fi
  FPATH="$FX/$FILE"
  [[ -f "$FPATH" ]] || continue
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
    > /dev/null 2>&1
  printf '  ✓ %s\n' "$LABEL"
done

# Generate report from JSON results
python3 - "$OUT" "$RULE_FILTER" <<'PYEOF'
import json, sys
from pathlib import Path

out = Path(sys.argv[1])
rule_filter = sys.argv[2].lower() if len(sys.argv) > 2 else ""
TOOLS = ["basilisk", "pyright", "mypy", "pyrefly", "ty"]
COL = 12

FIXTURE_LABELS = {
    "e0002_missing_return":            "E0002 Missing return",
    "e0016_incompatible_override":     "E0016 Incompatible override",
    "e0022_unhashable_dict_key":       "E0022 Unhashable dict key",
    "e0023_nonexhaustive_match":       "E0023 Non-exhaustive match",
    "e0026_typevar_single_constraint": "E0026 TypeVar constraint",
    "e0054_final_reassignment":        "E0054 Final reassignment",
}

rows = []
for stem, label in FIXTURE_LABELS.items():
    if rule_filter and rule_filter not in stem.lower() and rule_filter not in label.lower():
        continue
    path = out / f"{stem}.json"
    if not path.exists():
        continue
    data = json.loads(path.read_text())
    by_tool = {r["command"]: r["mean"] * 1000 for r in data["results"]}
    rows.append((label, by_tool))

if not rows:
    print("No results found.")
    sys.exit(0)

# Header
rule_w = max(len(r[0]) for r in rows) + 2
header = f"{'Rule':<{rule_w}}" + "".join(f"{t:>{COL}}" for t in TOOLS)
sep    = "─" * len(header)

print()
print(f"  Basilisk Benchmark Report")
print(f"  {'─' * (len(header))}")
print(f"  {header}")
print(f"  {sep}")

for label, by_tool in rows:
    timings = []
    best = min(by_tool.get(t, float("inf")) for t in TOOLS)
    for t in TOOLS:
        ms = by_tool.get(t)
        if ms is None:
            timings.append(f"{'n/a':>{COL}}")
        else:
            val = f"{ms:.0f} ms"
            marker = " ✓" if ms == best else ""
            timings.append(f"{val + marker:>{COL}}")
    print(f"  {label:<{rule_w}}{''.join(timings)}")

print(f"  {sep}")

# Per-tool averages
print(f"  {'Average':<{rule_w}}", end="")
for t in TOOLS:
    vals = [r[1][t] for r in rows if t in r[1]]
    avg = sum(vals) / len(vals) if vals else None
    cell = f"{avg:.0f} ms" if avg else "n/a"
    print(f"{cell:>{COL}}", end="")
print()
print()

# Fastest tool per fixture
print("  Fastest per rule:")
for label, by_tool in rows:
    fastest = min(by_tool, key=by_tool.get)
    print(f"    {label:<{rule_w-2}}→ {fastest}  ({by_tool[fastest]:.0f} ms)")
print()
PYEOF
