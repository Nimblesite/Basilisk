"""Generate a benchmark comparison report from hyperfine JSON results."""

import json
import sys
from pathlib import Path

TOOLS = ["basilisk", "pyright", "mypy", "pyrefly", "ty"]
COL = 12

FIXTURE_LABELS = {
    "e0002_missing_return": "E0002 Missing return",
    "e0016_incompatible_override": "E0016 Incompatible override",
    "e0022_unhashable_dict_key": "E0022 Unhashable dict key",
    "e0023_nonexhaustive_match": "E0023 Non-exhaustive match",
    "e0026_typevar_single_constraint": "E0026 TypeVar constraint",
    "e0054_final_reassignment": "E0054 Final reassignment",
}


def main() -> None:
    out = Path(sys.argv[1])
    rule_filter = sys.argv[2].lower() if len(sys.argv) > 2 else ""

    rows: list[tuple[str, dict[str, float]]] = []
    for stem, label in FIXTURE_LABELS.items():
        if rule_filter and rule_filter not in stem and rule_filter not in label.lower():
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

    rule_w = max(len(r[0]) for r in rows) + 2
    header = f"{'Rule':<{rule_w}}" + "".join(f"{t:>{COL}}" for t in TOOLS)
    sep = "\u2500" * len(header)

    print("\n  Basilisk Benchmark Report")
    print(f"  {sep}")
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
                marker = " \u2713" if ms == best else ""
                timings.append(f"{val + marker:>{COL}}")
        print(f"  {label:<{rule_w}}{''.join(timings)}")

    print(f"  {sep}")
    print(f"  {'Average':<{rule_w}}", end="")
    for t in TOOLS:
        vals = [r[1][t] for r in rows if t in r[1]]
        avg = sum(vals) / len(vals) if vals else None
        cell = f"{avg:.0f} ms" if avg else "n/a"
        print(f"{cell:>{COL}}", end="")
    print()
    print()

    print("  Fastest per rule:")
    for label, by_tool in rows:
        fastest = min(by_tool, key=by_tool.get)  # type: ignore[arg-type]
        print(f"    {label:<{rule_w - 2}}\u2192 {fastest}  ({by_tool[fastest]:.0f} ms)")
    print()


if __name__ == "__main__":
    main()
