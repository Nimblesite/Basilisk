# Benchmark summary

Machine: `Apple M4 Max`


| fixture | basilisk | basilisk-warm | pyright | mypy | mypy-warm | ty | pyrefly | zuban |
|---|---|---|---|---|---|---|---|---|
| aliases_type_statement | 12.5 ms | 5.7 ms | 739.6 ms | 855.1 ms | 230.4 ms | 85.3 ms | 144.9 ms | 36.5 ms |
| assignment_compatibility | 13.6 ms | 7.5 ms | 821.3 ms | 788.2 ms | 221.1 ms | 66.1 ms | 146.5 ms | 40.3 ms |
| call_argument_types | 22.6 ms | 5.9 ms | 895.9 ms | 816.5 ms | 216.8 ms | 73.1 ms | 155.6 ms | 70.3 ms |
| callables_subtyping | 18.3 ms | 7.2 ms | 702.3 ms | 789.7 ms | 210.0 ms | 47.4 ms | 136.2 ms | 36.6 ms |
| classvar_scoping | 23.4 ms | 8.3 ms | 838.8 ms | 959.5 ms | 220.9 ms | 72.9 ms | 174.2 ms | 41.7 ms |
| constructors_call_init | 14.0 ms | 6.4 ms | 875.3 ms | 787.0 ms | 207.9 ms | 48.5 ms | 136.8 ms | 35.7 ms |
| dataclasses_usage | 16.0 ms | 7.6 ms | 2090.5 ms | 859.7 ms | 225.4 ms | 83.4 ms | 239.4 ms | 75.5 ms |
| dict_key_hashability | 17.3 ms | 6.5 ms | 695.2 ms | 835.3 ms | 211.4 ms | 47.1 ms | 131.5 ms | 40.9 ms |
| enums_member_values | 12.0 ms | 5.9 ms | 761.5 ms | 745.0 ms | 216.3 ms | 50.8 ms | 131.2 ms | 34.4 ms |
| final_reassignment | 9.6 ms | 5.0 ms | 575.5 ms | 741.6 ms | 204.5 ms | 33.8 ms | 125.5 ms | 31.3 ms |
| generics_defaults_specialization | 15.1 ms | 6.0 ms | 741.0 ms | 766.8 ms | 208.7 ms | 43.2 ms | 131.3 ms | 35.2 ms |
| literals_semantics | 18.0 ms | 5.7 ms | 683.2 ms | 760.2 ms | 208.6 ms | 38.9 ms | 134.6 ms | 35.8 ms |
| match_exhaustiveness | 16.9 ms | 5.5 ms | 687.0 ms | 805.3 ms | 205.9 ms | 44.5 ms | 138.4 ms | 35.1 ms |
| narrowing_typeis | 14.7 ms | 5.8 ms | 710.2 ms | 752.6 ms | 210.7 ms | 43.2 ms | 142.1 ms | 34.9 ms |
| newtype_definition | 14.8 ms | 7.2 ms | 940.5 ms | 837.2 ms | 210.0 ms | 29.0 ms | 152.8 ms | 47.5 ms |
| overloads_evaluation | 20.9 ms | 6.2 ms | 800.0 ms | 823.3 ms | 205.9 ms | 75.2 ms | 148.7 ms | 45.5 ms |
| override_compatibility | 20.1 ms | 5.1 ms | 966.6 ms | 793.3 ms | 214.9 ms | 49.7 ms | 138.2 ms | 37.6 ms |
| protocols_definition | 12.7 ms | 6.4 ms | 726.5 ms | 747.5 ms | 208.5 ms | 44.0 ms | 131.5 ms | 36.9 ms |
| returns_compatibility | 10.7 ms | 6.1 ms | 613.6 ms | 778.6 ms | 205.2 ms | 38.7 ms | 125.0 ms | 30.2 ms |
| tuples_index | 12.6 ms | 5.2 ms | 728.2 ms | 732.7 ms | 203.8 ms | 42.1 ms | 136.3 ms | 33.1 ms |
| typeddict_key_access | 14.6 ms | 5.6 ms | 814.7 ms | 769.8 ms | 202.9 ms | 44.6 ms | 134.4 ms | 33.7 ms |
| typeddict_readonly_inheritance | 20.0 ms | 5.1 ms | 913.3 ms | 752.1 ms | 217.1 ms | 46.2 ms | 144.3 ms | 35.0 ms |
| typeddict_readonly_mutation | 13.9 ms | 5.7 ms | 819.1 ms | 744.2 ms | 207.3 ms | 52.0 ms | 140.3 ms | 35.2 ms |
| typevar_constraints | 24.7 ms | 7.2 ms | 933.4 ms | 771.1 ms | 205.2 ms | 49.0 ms | 138.8 ms | 43.2 ms |
| undefined_names | 23.9 ms | 7.2 ms | 639.1 ms | 824.2 ms | 220.3 ms | 63.2 ms | 683.4 ms | 42.7 ms |
| unresolved_imports | 18.1 ms | 7.0 ms | 529.8 ms | 895.4 ms | 211.7 ms | 358.4 ms | 1127.4 ms | 352.0 ms |
