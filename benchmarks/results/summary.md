# Benchmark summary

Machine: `Apple M4 Max`


| fixture | basilisk | basilisk-warm | pyright | mypy | mypy-warm | ty | pyrefly | zuban |
|---|---|---|---|---|---|---|---|---|
| aliases_type_statement | 8.3 ms | 4.5 ms | 563.6 ms | 633.7 ms | 167.4 ms | 60.1 ms | 152.5 ms | 27.3 ms |
| assignment_compatibility | 8.8 ms | 5.5 ms | 608.9 ms | 611.4 ms | 169.9 ms | 50.3 ms | 150.8 ms | 31.2 ms |
| call_argument_types | 14.6 ms | 5.0 ms | 670.7 ms | 636.8 ms | 168.9 ms | 53.9 ms | 160.1 ms | 49.0 ms |
| callables_subtyping | 10.6 ms | 2.5 ms | 536.5 ms | 597.5 ms | 166.0 ms | 35.4 ms | 141.9 ms | 27.0 ms |
| classvar_scoping | 15.2 ms | 5.8 ms | 621.9 ms | 639.4 ms | 167.8 ms | 56.8 ms | 169.4 ms | 31.6 ms |
| constructors_call_init | 9.5 ms | 4.5 ms | 630.1 ms | 625.1 ms | 168.3 ms | 38.5 ms | 142.4 ms | 26.2 ms |
| dataclasses_usage | 9.2 ms | 3.7 ms | 1627.8 ms | 667.6 ms | 170.0 ms | 58.6 ms | 215.3 ms | 56.9 ms |
| dict_key_hashability | 11.9 ms | 5.2 ms | 541.5 ms | 638.6 ms | 168.7 ms | 39.1 ms | 143.4 ms | 30.3 ms |
| enums_member_values | 7.8 ms | 4.1 ms | 586.5 ms | 593.5 ms | 167.5 ms | 43.1 ms | 146.0 ms | 26.7 ms |
| final_reassignment | 6.9 ms | 4.0 ms | 467.5 ms | 590.6 ms | 166.9 ms | 26.7 ms | 137.6 ms | 24.0 ms |
| generics_defaults_specialization | 10.5 ms | 4.3 ms | 578.0 ms | 608.2 ms | 167.0 ms | 34.9 ms | 141.8 ms | 26.5 ms |
| literals_semantics | 11.4 ms | 4.2 ms | 544.8 ms | 600.0 ms | 169.4 ms | 31.7 ms | 141.1 ms | 27.2 ms |
| match_exhaustiveness | 10.6 ms | 4.0 ms | 541.3 ms | 623.8 ms | 168.5 ms | 36.3 ms | 147.9 ms | 26.6 ms |
| narrowing_typeis | 9.5 ms | 4.2 ms | 552.5 ms | 605.0 ms | 167.3 ms | 35.6 ms | 145.5 ms | 25.6 ms |
| newtype_definition | 10.7 ms | 5.8 ms | 730.7 ms | 643.2 ms | 168.6 ms | 22.9 ms | 156.8 ms | 35.6 ms |
| overloads_evaluation | 12.5 ms | 4.3 ms | 609.2 ms | 637.7 ms | 166.8 ms | 59.1 ms | 157.6 ms | 34.7 ms |
| override_compatibility | 14.3 ms | 3.7 ms | 655.0 ms | 624.0 ms | 167.2 ms | 40.7 ms | 150.8 ms | 28.1 ms |
| protocols_definition | 8.8 ms | 4.2 ms | 576.2 ms | 603.7 ms | 166.7 ms | 35.0 ms | 140.9 ms | 27.0 ms |
| returns_compatibility | 7.4 ms | 5.2 ms | 503.4 ms | 591.7 ms | 169.5 ms | 31.0 ms | 137.2 ms | 23.9 ms |
| tuples_index | 8.4 ms | 3.7 ms | 563.4 ms | 587.5 ms | 166.5 ms | 32.7 ms | 139.2 ms | 25.7 ms |
| typeddict_key_access | 9.6 ms | 4.0 ms | 630.9 ms | 595.7 ms | 170.7 ms | 37.2 ms | 144.7 ms | 25.7 ms |
| typeddict_readonly_inheritance | 14.2 ms | 4.3 ms | 679.5 ms | 594.6 ms | 168.2 ms | 38.5 ms | 154.0 ms | 25.8 ms |
| typeddict_readonly_mutation | 9.9 ms | 4.4 ms | 635.7 ms | 598.9 ms | 167.4 ms | 43.1 ms | 147.7 ms | 25.7 ms |
| typevar_constraints | 18.2 ms | 5.0 ms | 740.0 ms | 603.2 ms | 168.9 ms | 39.3 ms | 148.6 ms | 32.6 ms |
| undefined_names | 15.2 ms | 4.9 ms | 499.9 ms | 651.6 ms | 170.5 ms | 49.5 ms | 587.3 ms | 35.5 ms |
| unresolved_imports | 11.7 ms | 5.2 ms | 475.3 ms | 688.2 ms | 174.5 ms | 241.7 ms | 565.8 ms | 247.3 ms |
