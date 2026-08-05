# Type-torture results

Methodology: see the header of `benchmarks/torture/run_torture.py` and of
`benchmarks/torture/status/torture.csv`. Every case file states the spec
section or PEP that makes its expectations authoritative.

Measured basilisk binary: local working-tree build: /Users/christianfindlay/Documents/Code/Basilisk/target/release/basilisk, built from v0.39.0-42-g3741268b-dirty

| case | basilisk | pyright | mypy | ty | pyrefly | zuban |
|---|---|---|---|---|---|---|
| enum_literal_expansion | pass | fail(m0,x3) | pass | pass | pass | fail(m0,x2) |
| generic_constructor | pass | pass | pass | fail(m0,x2) | pass | pass |
| none_class_objects | pass | pass | pass | pass | pass | pass |
| param_inference | pass | pass | pass | pass | pass | pass |
| paramspec_decorator | pass | pass | pass | fail(m0,x1) | pass | pass |
| recursive_aliases | pass | pass | pass | fail(m0,x3) | pass | pass |
| recursive_bases | pass | pass | pass | pass | pass | pass |
| scope_shadowing | pass | fail(m0,x1) | pass | fail(m0,x2) | fail(m0,x1) | pass |
| ternary_narrowing | pass | pass | pass | pass | pass | pass |
| tuple_index | pass | pass | pass | pass | pass | pass |
| typeddict_transitive | pass | pass | fail(m0,x4) | fail(m0,x3) | pass | pass |
| typeis_narrowing | pass | pass | pass | fail(m0,x3) | pass | pass |
| **passed** | 12/12 | 10/12 | 11/12 | 6/12 | 11/12 | 11/12 |

Versions measured: basilisk basilisk 0.0.0-PLACEHOLDER; pyright pyright 1.1.408; mypy mypy 1.19.1 (compiled: yes); ty ty 0.0.19 (ae10022c2 2026-02-26); pyrefly pyrefly 0.54.0; zuban zuban 0.9.0

## enum_literal_expansion
- pyright: missed error lines [], false positives on [20, 21, 23]
- zuban: missed error lines [], false positives on [20, 21]

## generic_constructor
- ty: missed error lines [], false positives on [14, 19]

## paramspec_decorator
- ty: missed error lines [], false positives on [14]

## recursive_aliases
- ty: missed error lines [], false positives on [11, 12, 13]

## scope_shadowing
- pyright: missed error lines [], false positives on [23]
- ty: missed error lines [], false positives on [23, 25]
- pyrefly: missed error lines [], false positives on [23]

## typeddict_transitive
- mypy: missed error lines [], false positives on [13, 22, 26, 27]
- ty: missed error lines [], false positives on [22, 26, 27]

## typeis_narrowing
- ty: missed error lines [], false positives on [11, 20, 22]

