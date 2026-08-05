# Benchmark summary

Machine: `Apple M4 Max`


| fixture | basilisk | basilisk-warm | pyright | mypy | mypy-warm | ty | pyrefly | zuban |
|---|---|---|---|---|---|---|---|---|
| aliases_type_statement | 10.9 ms | 4.9 ms | 556.3 ms | 629.9 ms | 168.0 ms | 64.7 ms | 114.1 ms | 27.9 ms |
| assignment_compatibility | 10.5 ms | 5.4 ms | 613.5 ms | 598.2 ms | 168.9 ms | 51.8 ms | 115.8 ms | 29.9 ms |
| call_argument_types | 17.2 ms | 4.6 ms | 649.6 ms | 620.6 ms | 166.3 ms | 56.8 ms | 119.2 ms | 48.8 ms |
| callables_subtyping | 14.5 ms | 5.0 ms | 532.8 ms | 594.6 ms | 167.3 ms | 39.1 ms | 107.9 ms | 28.2 ms |
| classvar_scoping | 19.0 ms | 5.8 ms | 626.5 ms | 631.7 ms | 170.8 ms | 59.9 ms | 137.5 ms | 31.8 ms |
| constructors_call_init | 10.3 ms | 4.4 ms | 639.4 ms | 611.6 ms | 169.4 ms | 39.1 ms | 107.7 ms | 26.9 ms |
| dataclasses_usage | 11.1 ms | 4.6 ms | 1594.0 ms | 665.3 ms | 171.6 ms | 60.6 ms | 180.9 ms | 57.9 ms |
| dict_key_hashability | 14.1 ms | 5.5 ms | 547.6 ms | 630.8 ms | 163.4 ms | 37.7 ms | 103.1 ms | 30.4 ms |
| enums_member_values | 9.6 ms | 4.9 ms | 577.5 ms | 590.8 ms | 169.1 ms | 42.8 ms | 105.4 ms | 27.2 ms |
| final_reassignment | 9.3 ms | 4.8 ms | 469.1 ms | 584.7 ms | 168.1 ms | 27.7 ms | 101.5 ms | 24.9 ms |
| generics_defaults_specialization | 12.2 ms | 5.2 ms | 559.9 ms | 595.8 ms | 167.2 ms | 34.8 ms | 104.8 ms | 26.2 ms |
| literals_semantics | 15.2 ms | 4.6 ms | 534.3 ms | 590.1 ms | 167.9 ms | 32.5 ms | 107.6 ms | 27.4 ms |
| match_exhaustiveness | 13.3 ms | 4.8 ms | 533.4 ms | 612.2 ms | 166.7 ms | 35.3 ms | 110.2 ms | 27.3 ms |
| narrowing_typeis | 11.7 ms | 4.4 ms | 544.1 ms | 590.7 ms | 169.7 ms | 36.0 ms | 107.2 ms | 26.4 ms |
| newtype_definition | 12.3 ms | 5.5 ms | 722.5 ms | 634.2 ms | 168.6 ms | 23.8 ms | 121.2 ms | 35.6 ms |
| overloads_evaluation | 17.6 ms | 6.1 ms | 600.0 ms | 628.4 ms | 166.5 ms | 59.8 ms | 118.9 ms | 34.4 ms |
| override_compatibility | 16.6 ms | 4.4 ms | 652.0 ms | 615.7 ms | 166.9 ms | 40.0 ms | 111.1 ms | 28.5 ms |
| protocols_definition | 10.7 ms | 4.6 ms | 587.2 ms | 590.8 ms | 166.1 ms | 35.7 ms | 105.9 ms | 27.2 ms |
| returns_compatibility | 9.1 ms | 4.6 ms | 499.7 ms | 587.4 ms | 168.8 ms | 32.2 ms | 102.3 ms | 24.8 ms |
| tuples_index | 10.8 ms | 5.3 ms | 569.9 ms | 585.5 ms | 167.7 ms | 33.6 ms | 105.1 ms | 26.1 ms |
| typeddict_key_access | 11.4 ms | 4.5 ms | 616.2 ms | 597.5 ms | 167.1 ms | 36.8 ms | 108.5 ms | 25.9 ms |
| typeddict_readonly_inheritance | 17.9 ms | 4.5 ms | 673.3 ms | 589.5 ms | 169.1 ms | 38.5 ms | 115.9 ms | 26.4 ms |
| typeddict_readonly_mutation | 11.5 ms | 4.2 ms | 625.2 ms | 594.8 ms | 166.3 ms | 42.1 ms | 111.3 ms | 26.2 ms |
| typevar_constraints | 18.4 ms | 5.3 ms | 743.7 ms | 613.4 ms | 168.6 ms | 40.5 ms | 113.5 ms | 33.4 ms |
| undefined_names | 19.2 ms | 5.7 ms | 495.7 ms | 647.8 ms | 174.1 ms | 53.3 ms | 550.3 ms | 34.8 ms |
| unresolved_imports | 15.0 ms | 6.2 ms | 467.1 ms | 687.3 ms | 175.4 ms | 278.8 ms | 894.2 ms | 305.9 ms |
