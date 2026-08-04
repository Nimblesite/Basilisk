# Benchmark summary

Machine: `Apple M4 Max`


| fixture | basilisk | basilisk-warm | pyright | mypy | mypy-warm | ty | pyrefly | zuban |
|---|---|---|---|---|---|---|---|---|
| aliases_type_statement | 9.5 ms | 4.6 ms | 545.8 ms | 612.2 ms | 163.5 ms | 64.4 ms | 112.5 ms | 28.0 ms |
| assignment_compatibility | 10.1 ms | 5.3 ms | 594.8 ms | 584.5 ms | 167.1 ms | 52.2 ms | 114.1 ms | 30.5 ms |
| call_argument_types | 14.9 ms | 4.6 ms | 642.6 ms | 609.1 ms | 165.2 ms | 56.3 ms | 116.8 ms | 48.4 ms |
| callables_subtyping | 13.3 ms | 4.9 ms | 522.0 ms | 580.4 ms | 164.9 ms | 38.5 ms | 107.8 ms | 27.9 ms |
| classvar_scoping | 15.9 ms | 5.6 ms | 606.0 ms | 616.2 ms | 166.0 ms | 58.0 ms | 134.1 ms | 32.5 ms |
| constructors_call_init | 9.8 ms | 4.3 ms | 610.7 ms | 593.4 ms | 165.0 ms | 39.7 ms | 106.1 ms | 26.7 ms |
| dataclasses_usage | 9.9 ms | 4.4 ms | 1570.7 ms | 650.4 ms | 166.0 ms | 65.2 ms | 178.4 ms | 57.3 ms |
| dict_key_hashability | 12.8 ms | 5.2 ms | 521.0 ms | 616.7 ms | 164.8 ms | 38.4 ms | 103.0 ms | 30.9 ms |
| enums_member_values | 8.4 ms | 4.3 ms | 566.4 ms | 575.3 ms | 164.3 ms | 41.9 ms | 104.7 ms | 25.9 ms |
| final_reassignment | 7.7 ms | 4.2 ms | 459.8 ms | 568.0 ms | 163.6 ms | 28.8 ms | 100.8 ms | 23.8 ms |
| generics_defaults_specialization | 10.7 ms | 4.6 ms | 549.9 ms | 583.3 ms | 165.2 ms | 35.0 ms | 105.6 ms | 25.9 ms |
| literals_semantics | 13.8 ms | 4.9 ms | 525.8 ms | 579.1 ms | 167.6 ms | 33.2 ms | 106.4 ms | 27.0 ms |
| match_exhaustiveness | 12.1 ms | 4.5 ms | 516.6 ms | 605.8 ms | 163.4 ms | 37.2 ms | 110.0 ms | 27.0 ms |
| narrowing_typeis | 10.8 ms | 4.5 ms | 540.6 ms | 583.2 ms | 166.8 ms | 35.8 ms | 105.8 ms | 25.8 ms |
| newtype_definition | 11.0 ms | 5.6 ms | 715.7 ms | 625.7 ms | 168.0 ms | 24.3 ms | 122.5 ms | 36.3 ms |
| overloads_evaluation | 13.6 ms | 4.2 ms | 590.5 ms | 621.8 ms | 164.7 ms | 60.7 ms | 118.4 ms | 34.7 ms |
| override_compatibility | 15.7 ms | 5.2 ms | 636.3 ms | 605.3 ms | 164.7 ms | 41.0 ms | 110.5 ms | 28.5 ms |
| protocols_definition | 10.2 ms | 4.5 ms | 577.3 ms | 580.3 ms | 168.1 ms | 35.9 ms | 103.9 ms | 27.0 ms |
| returns_compatibility | 7.8 ms | 4.7 ms | 494.5 ms | 573.6 ms | 164.8 ms | 32.4 ms | 102.1 ms | 24.9 ms |
| tuples_index | 9.9 ms | 4.5 ms | 549.0 ms | 575.5 ms | 165.7 ms | 34.1 ms | 103.4 ms | 25.2 ms |
| typeddict_key_access | 10.4 ms | 4.3 ms | 611.6 ms | 581.0 ms | 164.7 ms | 38.2 ms | 109.5 ms | 25.6 ms |
| typeddict_readonly_inheritance | 15.8 ms | 4.3 ms | 658.8 ms | 579.8 ms | 164.9 ms | 38.2 ms | 114.8 ms | 26.2 ms |
| typeddict_readonly_mutation | 11.1 ms | 4.6 ms | 614.5 ms | 585.1 ms | 165.1 ms | 42.9 ms | 109.9 ms | 26.9 ms |
| typevar_constraints | 17.9 ms | 5.8 ms | 775.1 ms | 586.5 ms | 168.6 ms | 41.3 ms | 114.1 ms | 33.0 ms |
| undefined_names | 16.5 ms | 5.6 ms | 485.5 ms | 636.1 ms | 168.6 ms | 51.7 ms | 554.7 ms | 35.5 ms |
| unresolved_imports | 13.9 ms | 5.4 ms | 459.8 ms | 681.9 ms | 169.3 ms | 281.5 ms | 917.9 ms | 306.0 ms |
