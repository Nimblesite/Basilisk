# basilisk-compiler

Basilisk compiler — compiles typed Python to native code.

## Role in Basilisk

This crate is the **native compilation backend**. It takes fully-typed Python (as verified by the checker) and compiles it to native machine code. This is a future capability — standard CPython execution remains the default.

## Key concepts

- **Typed Python only** — requires all code to pass Basilisk's strict type checker before compilation.
- **Ownership-aware** — leverages `basilisk-mojo` ownership annotations for memory management decisions.
- **Not a replacement for CPython** — an optional compilation target for performance-critical code paths.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `basilisk-parser` | AST input |
| `basilisk-resolver` | Name resolution |

## Status

Future — architecture designed, implementation planned.
