"""Build cargo-mutants arguments from the mutation-safe test --list output.

Each #[mutation_safe(rule = "eNNNN", fns = "fn_a|fn_b")] annotation emits a
wrapper module named `mutation_safe_eNNNN_fns__fn_a__fn_b__<test_fn>`. This
script extracts (rule, function) pairs and test binary names, then prints two
lines for the Makefile to consume:

  Line 1: --re pattern, e.g. rules/e0048[./].*\\bhas_top_level_token\\b|...
  Line 2: --test args,  e.g. --test coverage_boost_33_tests --test mutation_kill_tests

Usage: python3 scripts/mutation_examine_re.py <test-list-file> [examine_re|test_args]
  examine_re  (default) — print the --re pattern
  test_args             — print the --test flags for cargo
"""

import pathlib
import re
import sys


def _parse(list_file: str) -> tuple[dict[str, set[str]], list[str], list[str]]:
    lines = pathlib.Path(list_file).read_text(encoding="utf-8").splitlines()
    rule_fns: dict[str, set[str]] = {}
    rules_only: list[str] = []
    test_binaries: list[str] = []
    prefix = "mutation_safe_"

    for line in lines:
        # Extract the top-level test binary name (before the first ::)
        binary = line.split("::")[0].strip()
        if binary and binary not in test_binaries:
            test_binaries.append(binary)

        idx = line.find(prefix)
        if idx < 0:
            continue
        rest = line[idx + len(prefix) :]
        m = re.match(r"(e\d{4})_fns__(.+?)__\w+::", rest)
        if m:
            rule, slug = m.group(1), m.group(2)
            for fn_name in slug.split("__"):
                if fn_name:
                    rule_fns.setdefault(rule, set()).add(fn_name)
        else:
            code = rest[:5]
            if (
                len(code) == 5
                and code[0] == "e"
                and code[1:].isdigit()
                and code not in rules_only
            ):
                rules_only.append(code)

    return rule_fns, rules_only, test_binaries


def build_examine_re(list_file: str) -> str:
    rule_fns, rules_only, _ = _parse(list_file)

    if not rule_fns and not rules_only:
        sys.exit('no mutation-safe tests found; add #[mutation_safe(rule = "eNNNN")]')

    parts: list[str] = []
    for rule, fns in sorted(rule_fns.items()):
        for fn_name in sorted(fns):
            parts.append(rf"rules/{rule}[./].*\b{fn_name}\b")
    for code in rules_only:
        if code not in rule_fns:
            parts.append(rf"rules/{code}[./]")

    return "|".join(parts)


def build_test_args(list_file: str, marker: str) -> str:
    _, _, binaries = _parse(list_file)
    # Only keep binaries that actually contain mutation_safe tests (not the
    # coverage_boost_tests umbrella that re-imports them as submodules).
    # Heuristic: exclude names that are known umbrella files.
    excluded = {"coverage_boost_tests", "coverage_boost_33"}
    targeted = [b for b in binaries if b not in excluded]
    if not targeted:
        # fallback: no --test filter, just the marker
        return marker
    test_flags = " ".join(f"--test {b}" for b in targeted)
    return f"{test_flags} {marker}"


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(
            f"usage: {sys.argv[0]} <test-list-file> [examine_re|test_args [marker]]"
        )
    mode = sys.argv[2] if len(sys.argv) > 2 else "examine_re"
    marker = sys.argv[3] if len(sys.argv) > 3 else "mutation_safe"
    if mode == "test_args":
        print(build_test_args(sys.argv[1], marker))
    else:
        print(build_examine_re(sys.argv[1]))
