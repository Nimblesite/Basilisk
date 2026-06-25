#!/usr/bin/env bash
# FP-elimination verification harness.
#
# Runs the PEP conformance suite, captures per-FP rule mapping, and diffs the
# regenerated conformance_status.csv against a saved baseline to enforce the
# HARD INVARIANT: no file may regress PASS->FAIL, total `caught` may not drop,
# total `missed` may not rise, and `fp` must trend DOWN.
#
# Usage:
#   scripts/fp_verify.sh                  # verify against /tmp/conf_baseline.csv
#   scripts/fp_verify.sh --save-baseline  # snapshot current CSV as the baseline
set -euo pipefail

ROOT="/Users/christianfindlay/Documents/Code/Basilisk"
cd "$ROOT"
BASELINE="/tmp/conf_baseline.csv"
CSV="conformance/conformance_status.csv"

if [[ "${1:-}" == "--save-baseline" ]]; then
  cp "$CSV" "$BASELINE"
  echo "baseline saved: $BASELINE"
  exit 0
fi

# Regenerate the conformance CSV with the official scorer against the release
# binary. score.py writes per-file caught/missed/fp to $CSV, which we diff below.
cargo build --release -p basilisk-cli --bin basilisk >/dev/null 2>&1
python3 conformance/score.py --bin target/release/basilisk >/dev/null 2>&1 || true

echo "=== totals (current) ==="
awk -F, 'NR>1{c+=$5;m+=$6;f+=$7; if($4=="PASS")p++; else if($4=="FAIL")fl++} \
  END{printf "PASS=%d FAIL=%d caught=%d missed=%d fp=%d\n",p,fl,c,m,f}' "$CSV"

echo "=== totals (baseline) ==="
awk -F, 'NR>1{c+=$5;m+=$6;f+=$7; if($4=="PASS")p++; else if($4=="FAIL")fl++} \
  END{printf "PASS=%d FAIL=%d caught=%d missed=%d fp=%d\n",p,fl,c,m,f}' "$BASELINE"

echo "=== per-file regressions (status flip / caught down / missed up) ==="
# Join baseline and current by filename (col 2); flag any regression.
awk -F, '
  FNR==NR { if(FNR>1){bs[$2]=$4; bc[$2]=$5; bm[$2]=$6; bf[$2]=$7} next }
  FNR>1 {
    f=$2
    if (f in bs) {
      reg=""
      if (bs[f]=="PASS" && $4!="PASS") reg=reg" STATUS:"bs[f]"->"$4
      if ($5+0 < bc[f]+0)             reg=reg" CAUGHT:"bc[f]"->"$5
      if ($6+0 > bm[f]+0)             reg=reg" MISSED:"bm[f]"->"$6
      if (reg!="") printf "  REGRESSION %s:%s\n", f, reg
    }
  }
' "$BASELINE" "$CSV" || true
echo "(none above = clean)"

echo "=== FP delta by file (baseline -> current; only changes) ==="
awk -F, '
  FNR==NR { if(FNR>1) bf[$2]=$7; next }
  FNR>1 { if ($7+0 != bf[$2]+0) printf "  %s: fp %s -> %s\n", $2, bf[$2], $7 }
' "$BASELINE" "$CSV" || true
echo "(none above = no FP change)"
