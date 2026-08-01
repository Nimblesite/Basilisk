# Benchmark summary

Machine: `Apple M4 Max`


| fixture | basilisk | basilisk-warm | pyright | mypy | mypy-warm | ty | pyrefly | zuban |
|---|---|---|---|---|---|---|---|---|
| aliases_type_statement | 10.2 ms | 5.4 ms | 547.1 ms | 610.0 ms | 161.0 ms | 63.9 ms | 112.3 ms | 28.8 ms |
| assignment_compatibility | 11.6 ms | 6.3 ms | 585.4 ms | 583.5 ms | 164.7 ms | 52.2 ms | 113.4 ms | 30.6 ms |
| call_argument_types | 15.3 ms | 5.0 ms | 642.7 ms | 611.6 ms | 163.7 ms | 56.1 ms | 114.3 ms | 48.4 ms |
| callables_subtyping | 14.5 ms | 5.2 ms | 522.1 ms | 571.4 ms | 164.2 ms | 39.3 ms | 109.7 ms | 29.1 ms |
| classvar_scoping | 17.5 ms | 6.2 ms | 599.6 ms | 614.3 ms | 163.3 ms | 58.8 ms | 134.7 ms | 32.4 ms |
| constructors_call_init | 12.0 ms | 5.6 ms | 592.2 ms | 596.6 ms | 162.6 ms | 38.8 ms | 103.8 ms | 26.6 ms |
| dataclasses_usage | 13.1 ms | 5.0 ms | 1559.4 ms | 642.0 ms | 164.7 ms | 61.8 ms | 176.1 ms | 56.6 ms |
| dict_key_hashability | 13.6 ms | 6.6 ms | 518.9 ms | 613.3 ms | 160.7 ms | 39.2 ms | 103.9 ms | 31.9 ms |
| enums_member_values | 10.0 ms | 4.5 ms | 564.1 ms | 576.0 ms | 160.9 ms | 42.0 ms | 103.8 ms | 26.7 ms |
| final_reassignment | 9.4 ms | 5.5 ms | 456.9 ms | 562.5 ms | 167.2 ms | 28.9 ms | 100.6 ms | 24.4 ms |
| generics_defaults_specialization | 12.2 ms | 5.0 ms | 549.6 ms | 579.0 ms | 162.1 ms | 35.0 ms | 104.7 ms | 27.4 ms |
| literals_semantics | 15.4 ms | 5.4 ms | 518.2 ms | 577.6 ms | 162.5 ms | 32.5 ms | 104.5 ms | 27.0 ms |
| match_exhaustiveness | 13.0 ms | 5.6 ms | 521.6 ms | 600.0 ms | 163.2 ms | 36.7 ms | 111.4 ms | 27.4 ms |
| narrowing_typeis | 11.0 ms | 4.1 ms | 539.3 ms | 582.8 ms | 160.2 ms | 34.5 ms | 104.9 ms | 26.4 ms |
| newtype_definition | 12.7 ms | 7.0 ms | 715.1 ms | 628.9 ms | 164.4 ms | 25.1 ms | 118.3 ms | 35.8 ms |
| overloads_evaluation | 14.4 ms | 4.5 ms | 591.8 ms | 627.4 ms | 163.9 ms | 60.2 ms | 119.9 ms | 34.2 ms |
| override_compatibility | 15.9 ms | 4.2 ms | 635.9 ms | 598.1 ms | 164.0 ms | 42.0 ms | 111.2 ms | 28.2 ms |
| protocols_definition | 11.1 ms | 4.6 ms | 562.6 ms | 580.4 ms | 163.1 ms | 35.5 ms | 103.7 ms | 27.5 ms |
| returns_compatibility | 10.4 ms | 5.8 ms | 488.7 ms | 572.5 ms | 162.5 ms | 33.0 ms | 101.9 ms | 24.5 ms |
| tuples_index | 11.0 ms | 4.5 ms | 549.3 ms | 566.6 ms | 162.1 ms | 35.0 ms | 106.4 ms | 25.8 ms |
| typeddict_key_access | 11.8 ms | 5.4 ms | 610.2 ms | 582.1 ms | 162.0 ms | 37.4 ms | 107.3 ms | 26.6 ms |
| typeddict_readonly_inheritance | 16.9 ms | 5.3 ms | 653.8 ms | 579.7 ms | 165.6 ms | 38.7 ms | 114.4 ms | 25.9 ms |
| typeddict_readonly_mutation | 13.0 ms | 4.6 ms | 613.3 ms | 579.8 ms | 163.3 ms | 42.7 ms | 107.9 ms | 26.0 ms |
| typevar_constraints | 19.7 ms | 5.9 ms | 720.8 ms | 577.9 ms | 165.2 ms | 42.3 ms | 113.6 ms | 34.1 ms |
| undefined_names | 17.7 ms | 5.8 ms | 487.5 ms | 631.7 ms | 168.3 ms | 51.2 ms | 544.6 ms | 34.4 ms |
| unresolved_imports | 15.8 ms | 6.0 ms | 455.6 ms | 710.6 ms | 167.7 ms | 284.5 ms | 897.7 ms | 294.6 ms |
