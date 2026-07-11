# Benchmark summary

Machine: `Apple M4 Max`


| fixture | basilisk | basilisk-warm | pyright | mypy | mypy-warm | ty | pyrefly | zuban |
|---|---|---|---|---|---|---|---|---|
| aliases_type_statement | 8.9 ms | 4.9 ms | 1861.7 ms | n/a | n/a | n/a | n/a | n/a |
| assignment_compatibility | 11.3 ms | 7.3 ms | 1914.0 ms | 595.3 ms | 171.4 ms | 49.6 ms | 150.5 ms | 30.6 ms |
| call_argument_types | 15.4 ms | 5.3 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| callables_subtyping | 13.9 ms | 6.1 ms | 1831.1 ms | 597.5 ms | n/a | n/a | n/a | n/a |
| classvar_scoping | 17.2 ms | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| constructors_call_init | 10.2 ms | 4.8 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| dataclasses_usage | 10.3 ms | 4.8 ms | 2930.4 ms | n/a | n/a | n/a | n/a | n/a |
| dict_key_hashability | 14.4 ms | n/a | n/a | n/a | n/a | n/a | n/a | n/a |
| enums_member_values | 8.4 ms | 5.2 ms | 1882.3 ms | 594.8 ms | 168.6 ms | n/a | n/a | n/a |
| final_reassignment | 7.8 ms | 4.8 ms | 1757.5 ms | 582.9 ms | 169.2 ms | 26.3 ms | 137.8 ms | 23.6 ms |
| generics_defaults_specialization | 11.2 ms | 5.0 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| literals_semantics | 12.0 ms | 4.5 ms | 1836.2 ms | 599.0 ms | 164.2 ms | 30.4 ms | 141.0 ms | 25.7 ms |
| match_exhaustiveness | 11.7 ms | 5.3 ms | 1855.4 ms | 690.3 ms | 255.9 ms | 62.0 ms | 180.3 ms | 29.4 ms |
| narrowing_typeis | 11.1 ms | 5.6 ms | 1863.8 ms | 610.9 ms | 172.5 ms | 33.7 ms | 147.8 ms | 25.9 ms |
| newtype_definition | 13.4 ms | 8.0 ms | 2033.6 ms | 655.8 ms | 170.2 ms | 22.8 ms | 159.7 ms | 36.1 ms |
| overloads_evaluation | 13.6 ms | 5.0 ms | 1914.7 ms | 641.1 ms | 167.6 ms | 57.2 ms | 152.2 ms | 33.4 ms |
| override_compatibility | 14.7 ms | 4.6 ms | 1961.0 ms | 611.9 ms | 163.8 ms | 38.1 ms | 147.1 ms | 28.2 ms |
| protocols_definition | 9.9 ms | 4.9 ms | 1893.6 ms | 590.3 ms | 165.1 ms | 34.2 ms | 138.2 ms | 26.0 ms |
| returns_compatibility | 8.2 ms | 5.8 ms | 1806.7 ms | 580.7 ms | 167.2 ms | 30.6 ms | 138.3 ms | 23.9 ms |
| tuples_index | 9.3 ms | 4.8 ms | 1852.2 ms | 592.7 ms | 162.8 ms | 31.5 ms | 143.3 ms | 25.6 ms |
| typeddict_key_access | 9.9 ms | 4.5 ms | 1948.8 ms | 604.3 ms | 172.4 ms | 36.0 ms | 144.5 ms | 25.2 ms |
| typeddict_readonly_inheritance | 14.6 ms | 4.9 ms | 1988.9 ms | 593.0 ms | 165.5 ms | 36.2 ms | 150.1 ms | 25.8 ms |
| typeddict_readonly_mutation | 10.3 ms | 4.3 ms | 1984.1 ms | 623.0 ms | 168.3 ms | 42.6 ms | 251.9 ms | 46.4 ms |
| typevar_constraints | 18.8 ms | 5.6 ms | 2174.1 ms | 599.9 ms | 168.5 ms | 36.7 ms | 149.8 ms | 31.4 ms |
| undefined_names | 17.3 ms | 7.6 ms | 1809.1 ms | 668.1 ms | 172.3 ms | 49.3 ms | 590.6 ms | 34.7 ms |
| unresolved_imports | 14.3 ms | 7.3 ms | 1934.4 ms | 734.2 ms | 179.9 ms | 250.9 ms | 582.2 ms | 246.9 ms |
