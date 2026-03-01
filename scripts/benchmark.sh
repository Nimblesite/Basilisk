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

printf 'Running benchmarks (this takes a while)...\n\n'

for entry in "${FIXTURES[@]}"; do
  FILE="${entry%%:*}"
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
  printf '  ✓ %s\n' "${entry##*:}"
done

# Generate report from JSON results
python3 - "$OUT" <<'PYEOF'
import json, sys, os, glob
from pathlib import Path

out = Path(sys.argv[1])
TOOLS = ["basilisk", "pyright", "mypy", "pyrefly", "ty"]
COL = 12

FIXTURE_LABELS = {
    "e0002_missing_return":          "E0002 Missing return",
    "e0016_incompatible_override":   "E0016 Incompatible override",
    "e0022_unhashable_dict_key":     "E0022 Unhashable dict key",
    "e0023_nonexhaustive_match":     "E0023 Non-exhaustive match",
    "e0026_typevar_single_constraint": "E0026 TypeVar constraint",
}

rows = []
for stem, label in FIXTURE_LABELS.items():
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
