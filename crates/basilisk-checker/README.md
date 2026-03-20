# basilisk-checker

Core type checking rules and diagnostic emission for Basilisk.

## Role in Basilisk

This is the **third stage** of the analysis pipeline. After `basilisk-resolver` builds the scope tree and resolves names, the checker walks the AST with full type information and emits diagnostics for every violation.

```
AST + scopes ➜ [basilisk-checker] ➜ diagnostics (BSK-E0001 through BSK-E0025)
```

## Key concepts

- **Strict by default** — every rule is on. There is no gradual mode.
- **25 diagnostic rules** — annotation enforcement (E0001-E0005), type correctness (E0010-E0025).
- **Stub resolution** — resolves types from bundled stubs (`basilisk-stubs`) for stdlib and third-party modules.
- **Configuration-aware** — reads per-path overrides from `basilisk-config` for gradual adoption.

## Diagnostic rules

### Annotations (E0001-E0005)

Missing parameter types, return types, variable types, `*args`/`**kwargs` types, and class attribute types.

### Type correctness (E0010-E0025)

Untyped imports, implicit `Any`, argument/return/assignment mismatches, wrong type arguments, incompatible overrides, undefined names, use-before-assignment, overload issues, unhashable dict keys, non-exhaustive match, invalid type expressions, and missing `@override`.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `basilisk-parser` | AST input |
| `basilisk-resolver` | Scope and name resolution |
| `basilisk-config` | Per-path configuration |
| `basilisk-stubs` | Type stub resolution |

## Status

Complete — all 25 rules implemented and tested.
