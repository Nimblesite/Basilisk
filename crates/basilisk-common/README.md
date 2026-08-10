# basilisk-common

> **A record, not a product claim.** Basilisk is unlisted and its type checker is
> inert ([WITHDRAWAL](../../docs/specs/DOCS-WITHDRAWAL-MESSAGING-SPEC.md#WITHDRAWAL)).
> Nothing described below ships in anything a user can install: the `basilisk`
> binary analyses nothing, and the editor extensions carry no checker. This file
> is kept as an account of what was built, and nothing in it authorises
> rebuilding what it describes.

Shared constants and types for Basilisk — compiles to both native and `wasm32-wasip1`.

## Role in Basilisk

This is the **shared foundation crate** with zero dependencies. It defines constants, diagnostic codes, and types that are used across the entire workspace. Because it compiles to WASM, it can also be used by the Zed editor extension.

## Key concepts

- **Zero dependencies** — keeps the dependency graph minimal for fast compilation and WASM compatibility.
- **Diagnostic code constants** — canonical BSK-E/BSK-W code definitions used by the checker and LSP.
- **Cross-platform** — compiles to native targets and `wasm32-wasip1` for the Zed extension.

## Status

Nothing user-facing consumes this crate. The Zed extension used to link it
for its shared command and diagnostic constants and no longer does; what
remains is consumed only by the language server, which ships in nothing.
