# Benchmark summary

Machine: `Apple M4 Max`


| fixture | basilisk | basilisk-warm | pyright | mypy | mypy-warm | ty | pyrefly | zuban |
|---|---|---|---|---|---|---|---|---|
| aliases_type_statement | 9.5 ms | 5.2 ms | 572.2 ms | 630.0 ms | 174.9 ms | 63.0 ms | 153.9 ms | n/a |
| assignment_compatibility | 11.3 ms | 7.2 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| call_argument_types | 15.8 ms | 5.8 ms | 661.8 ms | n/a | n/a | n/a | n/a | n/a |
| callables_subtyping | 13.7 ms | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| classvar_scoping | 17.7 ms | 7.8 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| constructors_call_init | 10.1 ms | 5.2 ms | 626.3 ms | 624.6 ms | 169.0 ms | 37.9 ms | n/a | n/a |
| dataclasses_usage | 10.2 ms | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| dict_key_hashability | 14.7 ms | 7.5 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| enums_member_values | 8.6 ms | 4.9 ms | 588.4 ms | 597.3 ms | 172.4 ms | 43.5 ms | 147.2 ms | 27.4 ms |
| final_reassignment | 8.1 ms | 4.9 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| generics_defaults_specialization | 11.3 ms | 5.0 ms | 572.0 ms | 598.4 ms | 168.6 ms | 34.1 ms | 137.4 ms | 26.4 ms |
| literals_semantics | 11.4 ms | 4.8 ms | 531.6 ms | 569.1 ms | 164.3 ms | 30.5 ms | 136.3 ms | 26.8 ms |
| match_exhaustiveness | 11.1 ms | 4.8 ms | 520.5 ms | 594.8 ms | 160.1 ms | 33.8 ms | 141.3 ms | 26.6 ms |
| narrowing_typeis | 9.8 ms | 4.6 ms | 540.8 ms | 570.6 ms | 162.3 ms | 33.2 ms | 137.2 ms | 25.8 ms |
| newtype_definition | 12.7 ms | 9.2 ms | 713.3 ms | 618.2 ms | 161.7 ms | 22.5 ms | 150.2 ms | 35.9 ms |
| overloads_evaluation | 13.0 ms | 5.0 ms | 599.0 ms | 607.2 ms | 160.4 ms | 58.4 ms | 149.3 ms | 33.5 ms |
| override_compatibility | 13.9 ms | 3.9 ms | 633.6 ms | 590.9 ms | 159.5 ms | 38.5 ms | 141.8 ms | 27.7 ms |
| protocols_definition | 9.3 ms | 4.7 ms | 566.6 ms | 567.7 ms | 162.2 ms | 33.8 ms | 136.0 ms | 26.4 ms |
| returns_compatibility | 8.2 ms | 6.1 ms | 496.8 ms | 564.5 ms | 166.5 ms | 29.8 ms | 133.3 ms | 23.4 ms |
| tuples_index | 9.6 ms | 6.0 ms | 550.0 ms | 562.1 ms | 162.9 ms | 32.4 ms | 135.9 ms | 25.2 ms |
| typeddict_key_access | 9.6 ms | 4.8 ms | 615.0 ms | 569.6 ms | 161.5 ms | 35.6 ms | 139.1 ms | 25.8 ms |
| typeddict_readonly_inheritance | 14.8 ms | 4.3 ms | 655.1 ms | 566.3 ms | 162.0 ms | 37.3 ms | 145.9 ms | 25.8 ms |
| typeddict_readonly_mutation | 10.7 ms | 4.1 ms | 616.8 ms | 566.7 ms | 162.6 ms | 39.3 ms | 140.1 ms | 25.4 ms |
| typevar_constraints | 19.7 ms | 7.2 ms | 719.2 ms | 571.9 ms | 163.2 ms | 38.8 ms | 141.2 ms | 32.4 ms |
| undefined_names | 16.9 ms | 7.1 ms | 497.9 ms | 624.1 ms | 163.3 ms | 49.4 ms | 576.3 ms | 33.7 ms |
| unresolved_imports | 13.0 ms | 6.7 ms | 465.3 ms | 676.2 ms | 166.0 ms | 239.2 ms | 581.6 ms | 236.5 ms |
