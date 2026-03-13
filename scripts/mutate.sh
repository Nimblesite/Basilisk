#!/usr/bin/env bash
# Run mutation tests against basilisk-checker rules.
#
# Strategy: per-group runs with small batches of rules (~200-300 mutants each).
# Each group runs all basilisk-checker tests (16s total), so per-mutant cost
# is ~2s build + 16s test = ~18s. At 4 jobs: 300 mutants / 4 * 18s ≈ 22 min max.
#
# Usage:
#   ./scripts/mutate.sh              # run all groups sequentially
#   ./scripts/mutate.sh --group N    # run only group N (1-based)
#   ./scripts/mutate.sh --list       # list groups and mutant counts
#   ./scripts/mutate.sh --rule e0014 # mutate a single rule only
#
# NOT RUN (excluded — unviable or too slow for mutation testing):
#   basilisk-parser   — 2 mutants, 0 tests, all unviable
#   basilisk-resolver — cold builds 300s+; mutants in bounded_typevar unlinked
#   basilisk-lsp      — no unit tests; spawns language server subprocess
#   basilisk-cli      — e2e tests spawn compiled binary; hangs per-mutant
#   src/inference.rs  — branch-depth counters cause infinite loops (all timeout)
#   src/types.rs      — 128 mutants shared by all rules; all 150+ test binaries
#                       run per mutant = hours. Excluded for now.
#   src/guards.rs     — same issue as types.rs

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

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

# ── Parse args ────────────────────────────────────────────────────────────────
RUN_GROUP=0
LIST_GROUPS=false
SINGLE_RULE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --group)
            RUN_GROUP="${2:?--group requires a number}"
            shift 2
            ;;
        --rule)
            SINGLE_RULE="${2:?--rule requires a rule name e.g. e0014}"
            shift 2
            ;;
        --list)
            LIST_GROUPS=true
            shift
            ;;
        *)
            echo "Unknown argument: $1"
            echo "Usage: $0 [--group N] [--list] [--rule eNNNN]"
            exit 1
            ;;
    esac
done

# ── Prerequisites ─────────────────────────────────────────────────────────────
MUTANTS_VERSION="26.0.0"
if ! cargo mutants --version &>/dev/null; then
    warn "cargo-mutants not found — installing v$MUTANTS_VERSION"
    cargo install "cargo-mutants@$MUTANTS_VERSION" --locked
fi

# ── Rule groups ───────────────────────────────────────────────────────────────
# Target: ≤300 mutants per group ≈ 22 min max at 4 jobs.
# Use unique var name to avoid collision with any exported GROUPS env var.
# NO inline comments inside array assignments (bash 3.2 compat).
unset RULE_GROUPS
declare -a RULE_GROUPS

RULE_GROUPS[0]="e0001 e0002 e0003 e0004 e0005 e0010 e0011 e0012 e0013 e0015 e0016 e0017 e0018 e0019 e0020"
RULE_GROUPS[1]="e0021 e0022 e0023 e0024 e0025 e0026 e0027 e0029 e0030 e0031 e0032 e0033 e0034 e0035 e0037"
RULE_GROUPS[2]="e0038 e0039 e0040 e0042 e0043 e0044 e0046 e0048 e0049 e0050 e0052 e0053 e0054 e0055 e0056"
RULE_GROUPS[3]="e0057 e0058 e0059 e0060 e0061 e0062 e0063 e0064 e0065 e0066 e0067 e0068 e0069 e0070 e0071"
RULE_GROUPS[4]="e0072 e0073 e0074 e0075 e0077 e0078 e0079 e0080 e0081 e0082 e0083 e0084 e0085 e0086 e0088"
RULE_GROUPS[5]="e0089 e0090 e0091 e0092 e0093 e0094 e0095 e0096 e0097 e0098 e0099 e0100 e0101 e0102 e0103"
RULE_GROUPS[6]="e0104 e0105 e0106 e0107 e0108 e0109 e0110 e0112 e0113 e0114 e0116 e0117 e0118 e0119 e0120"
RULE_GROUPS[7]="e0121 e0122 e0123 e0124 e0125 e0127 e0132 e0133 e0134 e0139 e0141"
RULE_GROUPS[8]="e0129 e0136 e0137 e0138"
RULE_GROUPS[9]="e0036 e0041 e0051 e0076 e0143 e0144 e0145 e0146 e0149"
RULE_GROUPS[10]="e0014 e0045 e0047 e0111 e0115 e0148"
RULE_GROUPS[11]="e0126 e0128 e0142"
RULE_GROUPS[12]="e0130 e0131"
RULE_GROUPS[13]="e0140 e0147"

NUM_GROUPS=${#RULE_GROUPS[@]}

# ── List mode ─────────────────────────────────────────────────────────────────
if [[ "$LIST_GROUPS" == true ]]; then
    echo ""
    echo "Mutation groups (~22 min max each at 4 jobs):"
    echo ""
    echo "NOT RUN: basilisk-parser, basilisk-resolver, basilisk-lsp, basilisk-cli"
    echo "NOT RUN: src/inference.rs, src/types.rs, src/guards.rs (hangs/unviable)"
    echo ""
    total=0
    for (( i=0; i<NUM_GROUPS; i++ )); do
        rules="${RULE_GROUPS[$i]}"
        re_pat="${rules// /|}"
        count=$(cargo mutants --list --package basilisk-checker \
            --re "rules/(${re_pat})[.]rs" \
            --exclude-re 'src/inference' 2>/dev/null | wc -l | tr -d ' ')
        total=$(( total + count ))
        printf "  %2d) %-60s %4s mutants\n" "$((i+1))" "${rules// /,}" "$count"
    done
    echo ""
    echo "  Total viable mutants: $total"
    echo ""
    exit 0
fi

# ── Single-rule mode ──────────────────────────────────────────────────────────
if [[ -n "$SINGLE_RULE" ]]; then
    header "Single rule: $SINGLE_RULE"
    out_dir="$REPO_ROOT/mutation_testing/mutants.out.${SINGLE_RULE}"
    diag "Output: $out_dir"
    cargo mutants --jobs 4 --timeout 30 --baseline skip \
        --package basilisk-checker \
        --re "rules/${SINGLE_RULE}[.]rs" \
        --exclude-re 'src/inference' \
        --cargo-test-arg --test --cargo-test-arg "${SINGLE_RULE}_tests" \
        --output "$out_dir"
    exit $?
fi

# ── Pre-flight ────────────────────────────────────────────────────────────────
if [[ $RUN_GROUP -eq 0 ]]; then
    header "Pre-flight: build check"
    if ! cargo test --no-run --package basilisk-checker 2>&1 | sed 's/^/  [build] /'; then
        fail "Pre-flight build failed"; exit 1
    fi
    ok "Build OK"

    header "Pre-flight: tests pass"
    cargo test --package basilisk-checker 2>&1 | grep "^test result" | sed 's/^/  /'
    if cargo test --package basilisk-checker 2>&1 | grep -q "FAILED"; then
        fail "Tests failing — fix before mutating"; exit 1
    fi
    ok "Tests OK"
fi

# ── Run a group ────────────────────────────────────────────────────────────────
RESULTS_BASE="$REPO_ROOT/mutation_testing"
mkdir -p "$RESULTS_BASE"
OVERALL_EXIT=0

run_group() {
    local idx="$1"
    local rules="${RULE_GROUPS[$((idx-1))]}"
    local re_pat="${rules// /|}"

    header "Group $idx: ${rules// /,}"

    local out_dir="$RESULTS_BASE/mutants.out.group${idx}"
    local log_file="/tmp/basilisk-mutants-group${idx}.log"

    diag "Pattern: rules/(${re_pat}).rs"
    diag "Output:  $out_dir"

    # Build --cargo-test-arg flags: one --test eNNNN_tests per rule in the group.
    # This means cargo only runs the matching test binaries per mutant (~0.3s)
    # instead of all 150+ checker test binaries (~16s). ~50x faster.
    local test_args=()
    for rule in $rules; do
        test_args+=(--cargo-test-arg --test --cargo-test-arg "${rule}_tests")
    done

    local baseline_arg="--baseline skip"
    [[ $RUN_GROUP -eq 0 ]] && baseline_arg=""

    # shellcheck disable=SC2086
    cargo mutants --jobs 4 --timeout 30 $baseline_arg \
        --package basilisk-checker \
        --re "rules/(${re_pat})[.]rs" \
        --exclude-re 'src/inference' \
        "${test_args[@]}" \
        --output "$out_dir" 2>&1 | tee "$log_file"
    local exit_code=${PIPESTATUS[0]}

    echo ""
    local out="$out_dir/mutants.out"
    if [[ -f "$out/missed.txt" ]] && [[ -s "$out/missed.txt" ]]; then
        fail "Surviving mutants in group $idx:"
        cat "$out/missed.txt"
    else
        ok "Group $idx: no surviving mutants"
    fi
    if [[ -f "$out/caught.txt" ]]; then
        ok "Caught: $(wc -l < "$out/caught.txt" | tr -d ' ')"
    fi
    if [[ -f "$out/timeout.txt" ]] && [[ -s "$out/timeout.txt" ]]; then
        warn "Timed out (add rule to NOT RUN list if recurring):"
        cat "$out/timeout.txt"
    fi

    [[ $exit_code -ne 0 ]] && OVERALL_EXIT=$exit_code
    return 0
}

if [[ $RUN_GROUP -gt 0 ]]; then
    run_group "$RUN_GROUP"
else
    for (( i=1; i<=NUM_GROUPS; i++ )); do
        run_group "$i"
    done
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
header "Overall summary"
total_missed=0; total_caught=0; total_timeout=0
for dir in "$RESULTS_BASE"/mutants.out.group*/; do
    [[ -d "$dir" ]] || continue
    out="$dir/mutants.out"
    [[ -f "$out/missed.txt" ]]  && total_missed=$(( total_missed  + $(wc -l < "$out/missed.txt"  | tr -d ' ') ))
    [[ -f "$out/caught.txt" ]]  && total_caught=$(( total_caught  + $(wc -l < "$out/caught.txt"  | tr -d ' ') ))
    [[ -f "$out/timeout.txt" ]] && total_timeout=$(( total_timeout + $(wc -l < "$out/timeout.txt" | tr -d ' ') ))
done

ok "Caught:  $total_caught"
[[ $total_missed  -gt 0 ]] && fail "Missed:  $total_missed"  || ok "Missed:  0"
[[ $total_timeout -gt 0 ]] && warn "Timeout: $total_timeout" || ok "Timeout: 0"
echo ""
echo -e "${BOLD}Results:${RESET} $RESULTS_BASE/"

exit "$OVERALL_EXIT"
