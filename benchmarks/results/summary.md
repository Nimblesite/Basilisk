# Benchmark summary

Machine: `Apple M4 Max`


| fixture | basilisk | basilisk-warm | pyright | mypy | mypy-warm | ty | pyrefly | zuban |
|---|---|---|---|---|---|---|---|---|
| aliases_type_statement | 8.8 ms | 4.4 ms | 547.1 ms | 610.0 ms | 161.0 ms | 63.9 ms | 112.3 ms | 28.8 ms |
| assignment_compatibility | 8.9 ms | 5.1 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| call_argument_types | 13.6 ms | 4.4 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| callables_subtyping | 12.8 ms | 4.3 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| classvar_scoping | 15.0 ms | 5.4 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| constructors_call_init | 9.3 ms | 4.7 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| dataclasses_usage | 10.3 ms | 4.3 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| dict_key_hashability | 12.1 ms | 5.2 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| enums_member_values | 7.9 ms | 3.7 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| final_reassignment | 7.7 ms | 4.3 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| generics_defaults_specialization | 10.3 ms | 4.3 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| literals_semantics | 13.1 ms | 4.3 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| match_exhaustiveness | 11.7 ms | 4.4 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| narrowing_typeis | 9.7 ms | 4.3 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| newtype_definition | 10.7 ms | 5.0 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| overloads_evaluation | 12.9 ms | 4.2 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| override_compatibility | 14.3 ms | 3.8 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| protocols_definition | 9.1 ms | 4.3 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| returns_compatibility | 7.3 ms | 4.5 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| tuples_index | 9.2 ms | 4.1 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| typeddict_key_access | 9.5 ms | 4.1 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| typeddict_readonly_inheritance | 14.2 ms | 3.8 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| typeddict_readonly_mutation | 10.2 ms | 4.6 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| typevar_constraints | 17.1 ms | 5.3 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| undefined_names | 15.3 ms | 5.1 ms | n/a | n/a | n/a | n/a | n/a | n/a |
| unresolved_imports | 14.0 ms | 5.2 ms | n/a | n/a | n/a | n/a | n/a | n/a |
