"""Build the cargo-mutants --re pattern from the mutation-safe test --list output.

Each #[mutation_safe(rule = "eNNNN", fns = "fn_a|fn_b")] annotation emits a
wrapper module named `mutation_safe_eNNNN_fns__fn_a__fn_b__<test_fn>`. This
script extracts (rule, function) pairs and produces a file-scoped regex like:

    rules/e0014[./].*\\bcheck_vars\\b|rules/e0048[./].*\\bhas_top_level_token\\b

Annotations without `fns` fall back to a whole-file pattern.
"""

import pathlib
import re
import sys


def build_examine_re(list_file: str) -> str:
    lines = pathlib.Path(list_file).read_text(encoding="utf-8").splitlines()
    rule_fns: dict[str, set[str]] = {}
    rules_only: list[str] = []
    prefix = "mutation_safe_"

    for line in lines:
        idx = line.find(prefix)
        if idx < 0:
            continue
        rest = line[idx + len(prefix):]
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

    if not rule_fns and not rules_only:
        sys.exit(
            'no mutation-safe tests found; add #[mutation_safe(rule = "eNNNN")]'
        )

    parts: list[str] = []
    for rule, fns in sorted(rule_fns.items()):
        for fn_name in sorted(fns):
            parts.append(rf"rules/{rule}[./].*\b{fn_name}\b")
    for code in rules_only:
        if code not in rule_fns:
            parts.append(rf"rules/{code}[./]")

    return "|".join(parts)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(f"usage: {sys.argv[0]} <test-list-file>")
    print(build_examine_re(sys.argv[1]))
