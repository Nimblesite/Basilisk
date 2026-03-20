# basilisk-common

Shared constants and types for Basilisk — compiles to both native and `wasm32-wasip1`.

## Role in Basilisk

This is the **shared foundation crate** with zero dependencies. It defines constants, diagnostic codes, and types that are used across the entire workspace. Because it compiles to WASM, it can also be used by the Zed editor extension.

## Key concepts

- **Zero dependencies** — keeps the dependency graph minimal for fast compilation and WASM compatibility.
- **Diagnostic code constants** — canonical BSK-E/BSK-W code definitions used by the checker and LSP.
- **Cross-platform** — compiles to native targets and `wasm32-wasip1` for the Zed extension.

## Status

Complete — stable API consumed across the workspace.
