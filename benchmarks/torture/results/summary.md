# Type-torture results

Methodology: see the header of `benchmarks/torture/run_torture.py` and of
`benchmarks/torture/status/torture.csv`. Every case file states the spec
section or PEP that makes its expectations authoritative.

| case | basilisk | pyright | mypy | ty | pyrefly | zuban |
|---|---|---|---|---|---|---|
| enum_literal_expansion | fail(m0,x1) | fail(m0,x3) | pass | pass | pass | fail(m0,x2) |
| generic_constructor | pass | pass | pass | fail(m0,x2) | pass | pass |
| param_inference | pass | pass | pass | pass | pass | pass |
| paramspec_decorator | pass | pass | pass | fail(m0,x1) | pass | pass |
| recursive_aliases | pass | pass | pass | fail(m0,x3) | pass | pass |
| recursive_bases | hang | pass | pass | pass | pass | pass |
| tuple_index | fail(m1,x0) | pass | pass | pass | pass | pass |
| typeis_narrowing | pass | pass | pass | fail(m0,x3) | pass | pass |
| **passed** | 5/8 | 7/8 | 8/8 | 4/8 | 8/8 | 7/8 |

Versions measured: basilisk basilisk 0.0.0-PLACEHOLDER; pyright pyright 1.1.408; mypy mypy 1.19.1 (compiled: yes); ty ty 0.0.19 (ae10022c2 2026-02-26); pyrefly pyrefly 0.54.0; zuban zuban 0.9.0

## enum_literal_expansion
- basilisk: missed error lines [], false positives on [20]
- pyright: missed error lines [], false positives on [20, 21, 23]
- zuban: missed error lines [], false positives on [20, 21]

## generic_constructor
- ty: missed error lines [], false positives on [14, 19]

## paramspec_decorator
- ty: missed error lines [], false positives on [14]

## recursive_aliases
- ty: missed error lines [], false positives on [11, 12, 13]

## recursive_bases
- basilisk: hang

## tuple_index
- basilisk: missed error lines [15], false positives on []

## typeis_narrowing
- ty: missed error lines [], false positives on [11, 20, 22]

