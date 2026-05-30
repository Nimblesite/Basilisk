#!/usr/bin/env bash
# Benchmark: Basilisk vs Pyright vs mypy vs ty vs Pyrefly.
#
# Uses hyperfine for accurate wall-clock timing. Each fixture is a large
# single-rule stress file (2k–3.5k lines) so the numbers reflect steady-state
# checking throughput rather than process startup.
#
# Run from the repo root:  make bench   (or:  bash benchmarks/run.sh)
#
# Competitor tools are OPTIONAL — any that are not installed are skipped, and
# the summary simply omits their column. Only `basilisk` and `hyperfine` are
# required. Fixtures intentionally contain errors, so every command is run with
# hyperfine --ignore-failure (a non-zero "errors found" exit is expected).

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BSK="$ROOT/target/release/basilisk"
FX="$ROOT/benchmarks/fixtures"
OUT="$ROOT/benchmarks/results"
RUNS="${RUNS:-10}"
WARMUP="${WARMUP:-2}"
mkdir -p "$OUT"

# ─── Preconditions ────────────────────────────────────────────────────────────
if ! command -v hyperfine >/dev/null 2>&1; then
  echo "ERROR: hyperfine is not installed (brew install hyperfine / cargo install hyperfine)." >&2
  exit 1
fi
if [[ ! -x "$BSK" ]]; then
  echo "ERROR: basilisk release binary not found at $BSK" >&2
  echo "       Build it first:  cargo build --release --bin basilisk" >&2
  exit 1
fi

# ─── Tool discovery (basilisk required, competitors optional) ─────────────────
# Each entry: "name|command-template"  where {} is replaced by the fixture path.
declare -a TOOL_NAMES=() TOOL_CMDS=()
add_tool() { TOOL_NAMES+=("$1"); TOOL_CMDS+=("$2"); }

add_tool "basilisk" "$BSK check {}"
command -v pyright  >/dev/null 2>&1 && add_tool "pyright"  "pyright {}"
command -v mypy     >/dev/null 2>&1 && add_tool "mypy"     "mypy --ignore-missing-imports --no-error-summary {}"
command -v ty       >/dev/null 2>&1 && add_tool "ty"       "ty check {}"
command -v pyrefly  >/dev/null 2>&1 && add_tool "pyrefly"  "pyrefly check {}"

version_of() {
  local v=""
  case "$1" in
    basilisk) v="$("$BSK" --version 2>&1 | head -1)" ;;
    *)        v="$("$1" --version 2>&1 | head -1)" ;;
  esac
  printf '%s' "${v:-n/a}"
}

echo "======================================================="
echo "  Basilisk Benchmark — $(date '+%Y-%m-%d %H:%M')"
echo "======================================================="
echo "  host    : $(uname -sm)"
echo "  runs    : $RUNS (warmup $WARMUP)"
echo "  tools   :"
for name in "${TOOL_NAMES[@]}"; do
  printf "    %-9s %s\n" "$name" "$(version_of "$name")"
done
echo ""

# ─── Per-file benchmarks ──────────────────────────────────────────────────────
# (portable fixture collection — macOS ships bash 3.2, which lacks `mapfile`)
FIXTURES=()
while IFS= read -r _f; do
  [[ -n "$_f" ]] && FIXTURES+=("$_f")
done < <(cd "$FX" && ls -1 ./*.py 2>/dev/null | sed 's|^\./||' | sort)
if [[ ${#FIXTURES[@]} -eq 0 ]]; then
  echo "ERROR: no fixtures found in $FX" >&2
  exit 1
fi

echo "─── Per-file timing ($RUNS runs each) ──────────────────────────────────"
echo ""

for FILE in "${FIXTURES[@]}"; do
  FPATH="$FX/$FILE"
  STEM="${FILE%.py}"
  echo "┌─ $STEM ($(wc -l < "$FPATH" | tr -d ' ') lines)"

  HF=(hyperfine --ignore-failure --warmup "$WARMUP" --runs "$RUNS"
      --export-json "$OUT/${STEM}.json")
  for i in "${!TOOL_NAMES[@]}"; do
    CMD="${TOOL_CMDS[$i]//\{\}/$FPATH}"
    HF+=(--command-name "${TOOL_NAMES[$i]}" "$CMD")
  done

  "${HF[@]}" 2>&1 | grep -E "Time|Summary|ran|faster|slower" | sed 's/^/│  /' || true
  echo "└──"
  echo ""
done

# ─── Summary table (mean ms per tool, parsed from hyperfine JSON) ─────────────
echo "─── Summary: mean wall-clock per fixture (ms) ──────────────────────────"
echo ""
python3 - "$OUT" "${TOOL_NAMES[@]}" <<'PY'
import json, os, sys, glob

out_dir = sys.argv[1]
tools = sys.argv[2:]

rows = []
for jf in sorted(glob.glob(os.path.join(out_dir, "*.json"))):
    stem = os.path.splitext(os.path.basename(jf))[0]
    with open(jf) as fh:
        data = json.load(fh)
    means = {r["command"]: r["mean"] * 1000.0 for r in data["results"]}
    rows.append((stem, means))

if not rows:
    print("  (no JSON results)")
    raise SystemExit

w = max(len(s) for s, _ in rows)
header = f"  {'fixture':<{w}}  " + "  ".join(f"{t:>9}" for t in tools)
print(header)
print("  " + "-" * (len(header) - 2))
for stem, means in rows:
    cells = []
    fastest = min((means[t] for t in tools if t in means), default=None)
    for t in tools:
        if t in means:
            v = means[t]
            mark = " *" if fastest is not None and abs(v - fastest) < 1e-9 else "  "
            cells.append(f"{v:7.1f}{mark}")
        else:
            cells.append(f"{'n/a':>9}")
    print(f"  {stem:<{w}}  " + "  ".join(cells))
print("\n  (* = fastest for that fixture; lower is better)")

# Machine-readable markdown summary for the website / docs.
md = [f"# Benchmark summary\n", f"Host: `{os.uname().sysname} {os.uname().machine}`\n",
      "", "| fixture | " + " | ".join(tools) + " |",
      "|" + "---|" * (len(tools) + 1)]
for stem, means in rows:
    cells = [f"{means[t]:.1f} ms" if t in means else "n/a" for t in tools]
    md.append(f"| {stem} | " + " | ".join(cells) + " |")
with open(os.path.join(out_dir, "summary.md"), "w") as fh:
    fh.write("\n".join(md) + "\n")
print(f"\n  Markdown summary: {os.path.join(out_dir, 'summary.md')}")
PY

# ─── Diagnostic coverage (best-effort error counts) ───────────────────────────
echo ""
echo "─── Diagnostic coverage (errors reported per tool) ─────────────────────"
echo ""
printf "  %-38s" "fixture"
for name in "${TOOL_NAMES[@]}"; do printf " %9s" "$name"; done
echo ""
for FILE in "${FIXTURES[@]}"; do
  FPATH="$FX/$FILE"
  printf "  %-38s" "${FILE%.py}"
  for i in "${!TOOL_NAMES[@]}"; do
    name="${TOOL_NAMES[$i]}"
    CMD="${TOOL_CMDS[$i]//\{\}/$FPATH}"
    case "$name" in
      basilisk) n=$($CMD 2>/dev/null | grep -cE "error\[BSK" || true) ;;
      pyright)  n=$($CMD 2>/dev/null | grep -cE " - error:" || true) ;;
      mypy)     n=$($CMD 2>/dev/null | grep -cE ": error:" || true) ;;
      ty)       n=$($CMD 2>/dev/null | grep -cE "error\[|error:" || true) ;;
      pyrefly)  n=$($CMD 2>/dev/null | grep -cE "error\[|ERROR " || true) ;;
      *)        n=0 ;;
    esac
    printf " %9s" "$n"
  done
  echo ""
done

echo ""
echo "Done. Per-fixture JSON + summary.md saved to $OUT/"
