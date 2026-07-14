# basilisk-zed

<p align="center"><strong>English</strong> · <a href="README.zh.md">简体中文</a></p>

Zed editor extension for Basilisk — WASM-based Python type checking and language server integration.

Basilisk is the only Python type checker scoring 100% on the [official `python/typing` conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html) — and the fastest we've measured. A complete, open-source Python dev environment in Rust: type checker, language server, debugger, profiler, plus VS Code, Cursor, Zed & Neovim extensions. Strict by default.

<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/website/src/assets/images/zed-screenshot.png" alt="Basilisk in the Zed editor — Python type checking and diagnostics inline" width="900">
</p>

## Role in Basilisk

This is the **Zed editor integration**. It is a native Zed extension compiled to WASM that connects the Basilisk language server to Zed, providing real-time diagnostics, hover, go-to-definition, code actions, and debugging via DAP.

## Key concepts

- **WASM extension** — compiled as a `cdylib` crate targeting `wasm32-wasip1`, loaded natively by Zed.
- **`zed_extension_api`** — uses Zed's official extension API for language server lifecycle management.
- **`basilisk-common`** — shares diagnostic codes and constants with the rest of the Basilisk workspace (also WASM-compatible).
- **Tree-sitter grammars** — provides Python syntax highlighting via tree-sitter.
- **DAP debugging** — supports the Debug Adapter Protocol for integrated Python debugging.

## Building

```sh
make package-zed
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `zed_extension_api` | Zed extension API |
| `basilisk-common` | Shared constants and types |

## Status

Phase 2 — extension structure complete, connecting to the Basilisk LSP.

## License

MIT.
