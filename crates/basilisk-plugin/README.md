# basilisk-plugin

WASM plugin host for Basilisk.

## Role in Basilisk

This crate provides a **plugin system** that lets framework authors extend Basilisk's type checking with framework-specific knowledge. Plugins are compiled to WASM and loaded at runtime, allowing Django, Pydantic, SQLAlchemy, and other frameworks to teach Basilisk about their dynamic patterns.

## Key concepts

- **WASM sandboxing** — plugins run in a sandboxed WASM environment for safety and portability.
- **Framework-specific types** — plugins can declare that `Model.objects.filter()` returns a `QuerySet[MyModel]`, that Pydantic `BaseModel` subclasses have typed `__init__` signatures, etc.
- **No Python runtime** — plugins are compiled Rust/WASM, not Python scripts.

## Status

Phase 5 — architecture designed, implementation planned.
