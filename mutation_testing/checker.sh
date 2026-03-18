#!/usr/bin/env bash
# Mutation testing for basilisk-checker.
#
# Usage:
#   ./scripts/mutate/checker.sh              # run all groups
#   ./scripts/mutate/checker.sh --group N    # run only group N (1-based)
#   ./scripts/mutate/checker.sh --list       # list groups and mutant counts
#   ./scripts/mutate/checker.sh --rule e0014 # mutate a single rule
#
# NOT RUN (excluded — unviable or too slow):
#   src/inference.rs  — branch-depth counters cause infinite loops
#   src/types.rs      — 128 mutants shared by all rules; too slow
#   src/guards.rs     — same issue as types.rs

# shellcheck source=_common.sh
source "$(dirname "${BASH_SOURCE[0]}")/_common.sh"

# ── Parse args ────────────────────────────────────────────────────────────────
RUN_GROUP=0
LIST_GROUPS=false
SINGLE_RULE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --group)  RUN_GROUP="${2:?--group requires a number}"; shift 2 ;;
        --rule)   SINGLE_RULE="${2:?--rule requires a rule name e.g. e0014}"; shift 2 ;;
        --list)   LIST_GROUPS=true; shift ;;
        *)        echo "Unknown argument: $1"; echo "Usage: $0 [--group N] [--list] [--rule eNNNN]"; exit 1 ;;
    esac
done

ensure_cargo_mutants

# ── Rule groups ───────────────────────────────────────────────────────────────
# Target: ≤300 mutants per group ≈ 22 min max at 4 jobs.
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
    echo "Mutation groups for basilisk-checker (~22 min max each at 4 jobs):"
    echo ""
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
    run_mutant_group "basilisk-checker" "checker-${SINGLE_RULE}" \
        "rules/${SINGLE_RULE}[.]rs" \
        "--cargo-test-arg --test --cargo-test-arg ${SINGLE_RULE}_tests" \
        "--baseline skip"
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

# ── Run groups ────────────────────────────────────────────────────────────────
OVERALL_EXIT=0

run_checker_group() {
    local idx="$1"
    local rules="${RULE_GROUPS[$((idx-1))]}"
    local re_pat="${rules// /|}"

    local test_args=""
    for rule in $rules; do
        test_args+="--cargo-test-arg --test --cargo-test-arg ${rule}_tests "
    done

    local extra=""
    [[ $RUN_GROUP -gt 0 ]] && extra="--baseline skip"

    run_mutant_group "basilisk-checker" "checker-$(printf '%02d' "$idx")" \
        "rules/(${re_pat})[.]rs" \
        "$test_args" \
        "$extra" || OVERALL_EXIT=$?
}

if [[ $RUN_GROUP -gt 0 ]]; then
    run_checker_group "$RUN_GROUP"
else
    for (( i=1; i<=NUM_GROUPS; i++ )); do
        run_checker_group "$i"
    done
fi

print_summary "checker" || OVERALL_EXIT=1
exit "$OVERALL_EXIT"
