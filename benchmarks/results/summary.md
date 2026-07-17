# Benchmark summary

Machine: `Apple M4 Max`


| fixture | basilisk | basilisk-warm | pyright | mypy | mypy-warm | ty | pyrefly | zuban |
|---|---|---|---|---|---|---|---|---|
| aliases_type_statement | 8.1 ms | 4.4 ms | 559.9 ms | 595.9 ms | 162.8 ms | 54.5 ms | 110.0 ms | 27.1 ms |
| assignment_compatibility | 8.6 ms | 4.9 ms | 598.5 ms | 574.0 ms | 164.0 ms | 51.9 ms | 112.6 ms | 30.3 ms |
| call_argument_types | 13.5 ms | 4.2 ms | 841.8 ms | 613.3 ms | 171.5 ms | 55.5 ms | 114.3 ms | 48.2 ms |
| callables_subtyping | 12.1 ms | 4.1 ms | 529.6 ms | 574.5 ms | 162.0 ms | 38.3 ms | 107.4 ms | 28.4 ms |
| classvar_scoping | 15.3 ms | 5.7 ms | 606.2 ms | 620.3 ms | 170.3 ms | 57.4 ms | 133.5 ms | 31.5 ms |
| constructors_call_init | 9.1 ms | 4.1 ms | 613.2 ms | 573.4 ms | 156.1 ms | 37.0 ms | 100.4 ms | 26.2 ms |
| dataclasses_usage | 9.2 ms | 3.8 ms | 1579.1 ms | 640.6 ms | 158.3 ms | 57.9 ms | 170.5 ms | 55.1 ms |
| dict_key_hashability | 11.4 ms | 4.7 ms | 517.4 ms | 592.6 ms | 157.4 ms | 39.1 ms | 103.5 ms | 29.9 ms |
| enums_member_values | 7.2 ms | 3.7 ms | 569.8 ms | 549.7 ms | 155.9 ms | 40.2 ms | 100.3 ms | 26.3 ms |
| final_reassignment | 6.2 ms | 3.7 ms | 473.2 ms | 545.3 ms | 156.0 ms | 26.3 ms | 95.7 ms | 23.3 ms |
| generics_defaults_specialization | 9.7 ms | 3.8 ms | 576.8 ms | 557.4 ms | 157.8 ms | 33.7 ms | 100.9 ms | 25.4 ms |
| literals_semantics | 10.7 ms | 3.9 ms | 526.5 ms | 550.6 ms | 176.9 ms | 39.3 ms | 105.4 ms | 26.9 ms |
| match_exhaustiveness | 10.4 ms | 4.1 ms | 547.1 ms | 970.8 ms | 162.0 ms | 26.0 ms | 109.9 ms | 27.9 ms |
| narrowing_typeis | 8.7 ms | 3.7 ms | 549.5 ms | 573.8 ms | 161.9 ms | 33.6 ms | 108.1 ms | 27.2 ms |
| newtype_definition | 11.0 ms | 4.8 ms | 710.5 ms | 616.6 ms | 164.3 ms | 44.6 ms | 118.1 ms | 35.3 ms |
| overloads_evaluation | 12.2 ms | 4.4 ms | 593.0 ms | 603.8 ms | 162.0 ms | 60.3 ms | 115.7 ms | 33.6 ms |
| override_compatibility | 13.6 ms | 4.4 ms | 634.6 ms | 597.6 ms | 162.9 ms | 40.8 ms | 109.2 ms | 28.2 ms |
| protocols_definition | 8.4 ms | 3.8 ms | 563.0 ms | 564.7 ms | 160.8 ms | 34.1 ms | 102.2 ms | 26.9 ms |
| returns_compatibility | 7.1 ms | 4.5 ms | 503.1 ms | 562.7 ms | 165.0 ms | 32.6 ms | 100.8 ms | 24.1 ms |
| tuples_index | 8.7 ms | 3.6 ms | 550.2 ms | 558.1 ms | 159.8 ms | 34.1 ms | 101.8 ms | 25.0 ms |
| typeddict_key_access | 8.9 ms | 3.5 ms | 627.3 ms | 563.3 ms | 160.2 ms | 36.7 ms | 105.4 ms | 26.2 ms |
| typeddict_readonly_inheritance | 13.9 ms | 3.8 ms | 653.3 ms | 562.6 ms | 161.6 ms | 38.0 ms | 114.4 ms | 26.3 ms |
| typeddict_readonly_mutation | 10.3 ms | 4.0 ms | 614.3 ms | 575.0 ms | 179.5 ms | 43.2 ms | 110.0 ms | 26.9 ms |
| typevar_constraints | 17.0 ms | 5.5 ms | 726.8 ms | 568.3 ms | 159.8 ms | 39.9 ms | 110.2 ms | 32.2 ms |
| undefined_names | 14.5 ms | 4.7 ms | 489.9 ms | 611.5 ms | 162.6 ms | 50.4 ms | 539.4 ms | 34.2 ms |
| unresolved_imports | 12.9 ms | 5.0 ms | 456.2 ms | 650.9 ms | 167.8 ms | 264.8 ms | 853.8 ms | 299.0 ms |
