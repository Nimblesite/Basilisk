#!/usr/bin/env bash
# Shared helpers for per-crate mutation test scripts.
# Source this file — do not execute directly.

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

header() { echo -e "\n${BOLD}${CYAN}▶ $*${RESET}"; }
ok()     { echo -e "${GREEN}✓ $*${RESET}"; }
warn()   { echo -e "${YELLOW}⚠ $*${RESET}"; }
fail()   { echo -e "${RED}✗ $*${RESET}"; }
diag()   { echo -e "${CYAN}  [diag] $*${RESET}"; }

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[1]}")/.." && pwd)"
cd "$REPO_ROOT"

RESULTS_BASE="$REPO_ROOT/mutation_testing"
mkdir -p "$RESULTS_BASE"

MUTANTS_VERSION="26.0.0"

ensure_cargo_mutants() {
    if ! cargo mutants --version &>/dev/null; then
        warn "cargo-mutants not found — installing v$MUTANTS_VERSION"
        cargo install "cargo-mutants@$MUTANTS_VERSION" --locked
    fi
}

# Run mutation testing for a single group of rules within a crate.
#
# Args:
#   $1 — crate package name (e.g. basilisk-checker)
#   $2 — group label (e.g. "checker-01")
#   $3 — regex pattern for --re (e.g. "rules/(e0001|e0002).rs")
#   $4 — space-separated list of --cargo-test-arg pairs (optional)
#   $5 — extra cargo-mutants flags (optional)
run_mutant_group() {
    local package="$1"
    local label="$2"
    local re_pattern="$3"
    local test_args_str="${4:-}"
    local extra_flags="${5:-}"

    local out_dir="$RESULTS_BASE/mutants.out.${label}"
    local log_file="/tmp/basilisk-mutants-${label}.log"

    header "$label"
    diag "Package: $package"
    diag "Pattern: $re_pattern"
    diag "Output:  $out_dir"

    local test_args=()
    if [[ -n "$test_args_str" ]]; then
        # shellcheck disable=SC2206
        test_args=($test_args_str)
    fi

    # shellcheck disable=SC2086
    cargo mutants --jobs 4 --timeout 30 \
        --package "$package" \
        --re "$re_pattern" \
        --exclude-re 'src/inference' \
        "${test_args[@]}" \
        $extra_flags \
        --output "$out_dir" 2>&1 | tee "$log_file"
    local exit_code=${PIPESTATUS[0]}

    echo ""
    local out="$out_dir/mutants.out"
    if [[ -f "$out/missed.txt" ]] && [[ -s "$out/missed.txt" ]]; then
        fail "Surviving mutants in $label:"
        cat "$out/missed.txt"
    else
        ok "$label: no surviving mutants"
    fi
    if [[ -f "$out/caught.txt" ]]; then
        ok "Caught: $(wc -l < "$out/caught.txt" | tr -d ' ')"
    fi
    if [[ -f "$out/timeout.txt" ]] && [[ -s "$out/timeout.txt" ]]; then
        warn "Timed out:"
        cat "$out/timeout.txt"
    fi

    return "$exit_code"
}

# Record mutation scores to the shared CSV file.
#
# Scans all mutants.out.* directories matching the given prefix and
# upserts rows into mutation_scores.csv (one row per output directory).
#
# Args:
#   $1 — prefix to match (e.g. "checker" matches mutants.out.checker-*)
record_scores() {
    local prefix="$1"
    local scores_file="$RESULTS_BASE/mutation_scores.csv"
    local today
    today="$(date +%Y-%m-%d)"

    # Ensure the CSV exists with a header row.
    if [[ ! -f "$scores_file" ]]; then
        echo "date,crate,total,caught,missed,timeout,unviable,kill_rate" > "$scores_file"
    fi

    for dir in "$RESULTS_BASE"/mutants.out.${prefix}*/; do
        [[ -d "$dir" ]] || continue
        local out="$dir/mutants.out"
        local label
        label=$(basename "$dir" | sed 's/^mutants\.out\.//')

        local missed=0 caught=0 timed_out=0 unviable=0
        [[ -f "$out/missed.txt" ]]   && missed=$(wc -l   < "$out/missed.txt"   | tr -d ' ')
        [[ -f "$out/caught.txt" ]]   && caught=$(wc -l   < "$out/caught.txt"   | tr -d ' ')
        [[ -f "$out/timeout.txt" ]]  && timed_out=$(wc -l < "$out/timeout.txt" | tr -d ' ')
        [[ -f "$out/unviable.txt" ]] && unviable=$(wc -l  < "$out/unviable.txt" | tr -d ' ')

        local total=$(( caught + missed + timed_out + unviable ))
        local viable=$(( caught + missed + timed_out ))
        local kill_rate=0
        if [[ $viable -gt 0 ]]; then
            kill_rate=$(( caught * 100 / viable ))
        fi

        local new_row="${today},${label},${total},${caught},${missed},${timed_out},${unviable},${kill_rate}%"

        # Remove any existing row for this label, then append the new one.
        local tmp_file
        tmp_file="$(mktemp)"
        awk -F',' -v lbl="$label" '$2 != lbl' "$scores_file" > "$tmp_file"
        mv "$tmp_file" "$scores_file"
        echo "$new_row" >> "$scores_file"

        diag "$label: ${kill_rate}% kill rate (${caught}/${viable} viable)"
    done

    ok "Scores written to $scores_file"
}

# Print summary across all result directories matching a prefix.
#
# Args:
#   $1 — prefix to match (e.g. "checker" matches mutants.out.checker-*)
print_summary() {
    local prefix="$1"
    local total_missed=0 total_caught=0 total_timeout=0

    for dir in "$RESULTS_BASE"/mutants.out.${prefix}*/; do
        [[ -d "$dir" ]] || continue
        local out="$dir/mutants.out"
        [[ -f "$out/missed.txt" ]]  && total_missed=$((  total_missed  + $(wc -l < "$out/missed.txt"  | tr -d ' ') ))
        [[ -f "$out/caught.txt" ]]  && total_caught=$((  total_caught  + $(wc -l < "$out/caught.txt"  | tr -d ' ') ))
        [[ -f "$out/timeout.txt" ]] && total_timeout=$(( total_timeout + $(wc -l < "$out/timeout.txt" | tr -d ' ') ))
    done

    # Record scores to the shared CSV before printing.
    record_scores "$prefix"

    echo ""
    header "Summary: $prefix"
    ok "Caught:  $total_caught"
    [[ $total_missed  -gt 0 ]] && fail "Missed:  $total_missed"  || ok "Missed:  0"
    [[ $total_timeout -gt 0 ]] && warn "Timeout: $total_timeout" || ok "Timeout: 0"
    echo -e "${BOLD}Results:${RESET} $RESULTS_BASE/"

    [[ $total_missed -gt 0 ]] && return 1 || return 0
}
