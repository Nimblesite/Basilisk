# Benchmark summary

Machine: `Apple M4 Max`


| fixture | basilisk | basilisk-warm | pyright | mypy | mypy-warm | ty | pyrefly | zuban |
|---|---|---|---|---|---|---|---|---|
| aliases_type_statement | 9.4 ms | 5.2 ms | 561.8 ms | 626.7 ms | 164.7 ms | 61.0 ms | 150.8 ms | 28.7 ms |
| assignment_compatibility | 10.8 ms | 7.3 ms | 609.5 ms | 596.2 ms | 166.1 ms | 50.4 ms | 150.1 ms | 30.1 ms |
| call_argument_types | 15.4 ms | 6.3 ms | 664.4 ms | 621.2 ms | 166.6 ms | 55.7 ms | 153.5 ms | 48.7 ms |
| callables_subtyping | 13.2 ms | 5.0 ms | 526.1 ms | 590.9 ms | 164.2 ms | 37.5 ms | 147.4 ms | 28.4 ms |
| classvar_scoping | 17.3 ms | 7.5 ms | 625.7 ms | 629.2 ms | 165.0 ms | 57.8 ms | 170.4 ms | 31.8 ms |
| constructors_call_init | 10.2 ms | 4.8 ms | 614.3 ms | 609.7 ms | 167.5 ms | 39.2 ms | 142.2 ms | 26.6 ms |
| dataclasses_usage | 10.4 ms | 5.2 ms | 1618.8 ms | 662.0 ms | 168.4 ms | 58.8 ms | 208.8 ms | 56.9 ms |
| dict_key_hashability | 14.4 ms | 7.8 ms | 536.2 ms | 625.6 ms | 168.7 ms | 38.6 ms | 139.3 ms | 30.9 ms |
| enums_member_values | 8.6 ms | 4.9 ms | 581.9 ms | 586.5 ms | 167.2 ms | 41.6 ms | 141.1 ms | 26.5 ms |
| final_reassignment | 7.6 ms | 4.8 ms | 466.6 ms | 582.1 ms | 165.3 ms | 26.8 ms | 136.4 ms | 23.4 ms |
| generics_defaults_specialization | 11.2 ms | 4.9 ms | 567.7 ms | 596.1 ms | 164.5 ms | 33.9 ms | 140.4 ms | 27.0 ms |
| literals_semantics | 12.4 ms | 5.5 ms | 537.2 ms | 594.2 ms | 167.0 ms | 31.1 ms | 139.9 ms | 27.1 ms |
| match_exhaustiveness | 11.3 ms | 4.5 ms | 538.8 ms | 618.3 ms | 166.3 ms | 34.1 ms | 145.7 ms | 27.1 ms |
| narrowing_typeis | 10.5 ms | 5.1 ms | 555.3 ms | 595.0 ms | 170.2 ms | 35.3 ms | 143.0 ms | 26.4 ms |
| newtype_definition | 13.2 ms | 7.9 ms | 734.5 ms | 638.4 ms | 171.5 ms | 23.2 ms | 155.2 ms | 35.4 ms |
| overloads_evaluation | 13.6 ms | 5.1 ms | 615.6 ms | 642.3 ms | 179.3 ms | 59.3 ms | 156.2 ms | 34.1 ms |
| override_compatibility | 14.8 ms | 4.4 ms | 660.8 ms | 621.9 ms | 169.4 ms | 40.4 ms | 147.7 ms | 28.2 ms |
| protocols_definition | 9.8 ms | 5.1 ms | 580.6 ms | 594.5 ms | 169.0 ms | 37.9 ms | 140.9 ms | 26.6 ms |
| returns_compatibility | 8.5 ms | 6.0 ms | 504.5 ms | 583.4 ms | 164.5 ms | 31.4 ms | 139.5 ms | 23.9 ms |
| tuples_index | 9.4 ms | 5.4 ms | 560.4 ms | 585.7 ms | 164.7 ms | 31.9 ms | 139.8 ms | 24.3 ms |
| typeddict_key_access | 10.7 ms | 5.3 ms | 620.2 ms | 594.8 ms | 164.8 ms | 35.8 ms | 144.2 ms | 25.5 ms |
| typeddict_readonly_inheritance | 14.9 ms | 5.2 ms | 669.9 ms | 594.7 ms | 165.2 ms | 37.1 ms | 153.5 ms | 25.9 ms |
| typeddict_readonly_mutation | 10.9 ms | 4.7 ms | 626.2 ms | 596.1 ms | 164.0 ms | 40.7 ms | 145.2 ms | 25.6 ms |
| typevar_constraints | 20.2 ms | 7.7 ms | 727.9 ms | 595.2 ms | 165.9 ms | 38.8 ms | 147.6 ms | 33.1 ms |
| undefined_names | 18.3 ms | 7.7 ms | 500.7 ms | 645.6 ms | 168.6 ms | 49.6 ms | 580.0 ms | 34.4 ms |
| unresolved_imports | 13.9 ms | 7.5 ms | 474.2 ms | 695.7 ms | 173.3 ms | 239.9 ms | 562.5 ms | 245.3 ms |
