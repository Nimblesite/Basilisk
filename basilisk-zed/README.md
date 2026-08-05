# basilisk-zed

<p align="center"><strong>English</strong> · <a href="README.zh.md">简体中文</a></p>

Zed editor extension for Basilisk — WASM-based Python type checking and language server integration.

> **Metrics notice:** Basilisk has retracted its former 100% conformance claim and all published benchmark figures. The conformance result was not robust under semantics-preserving mutations, and Basilisk has been removed from the [official `python/typing` results table](https://github.com/python/typing/blob/main/conformance/results/results.html). Its actual conformance level is temporarily unknown while the fitted code is deleted and the affected logic is reimplemented from the specification. New results will be published only after mutation robustness and independent off-suite cases derived from the specification validate the rebuilt behavior. [Read the audit and recovery plan](https://www.basilisk-python.dev/docs/conformance/).

<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/website/src/assets/images/zed-screenshot.png" alt="Basilisk in the Zed editor — Python type checking and diagnostics inline" width="900">
</p>

## Install

Command palette (`Cmd+Shift+P` / `Ctrl+Shift+P`) → **zed: install dev extension** → select this directory (clone [`Nimblesite/basilisk-zed`](https://github.com/Nimblesite/basilisk-zed) first if you do not have the monorepo). Zed compiles the extension to WASM itself — you never pre-build or copy a `.wasm` file.

**You do not install the Basilisk binary separately.** On first activation the extension downloads the matching binary for your platform from the [GitHub release](https://github.com/Nimblesite/Basilisk/releases), caches it inside Zed's extension directory, and reuses it until a newer release appears. Override it only for development or a system install, via `lsp.basilisk.binary.path` in `settings.json` or the `BASILISK_PATH` environment variable.

> The extension is not yet listed in the [Zed extension registry](https://github.com/zed-industries/extensions); until that listing lands, the dev-extension flow above is the install path.

Full instructions, settings, debugging, and the slash-command reference: [basilisk-python.dev/docs/install-zed](https://www.basilisk-python.dev/docs/install-zed/).

## Role in Basilisk

This is the **Zed editor integration**. It is a native Zed extension compiled to WASM that connects the Basilisk language server to Zed, providing real-time diagnostics, hover, go-to-definition, code actions, and debugging via DAP.

## Key concepts

- **WASM extension** — compiled as a `cdylib` crate targeting `wasm32-wasip2`, loaded natively by Zed.
- **`zed_extension_api`** — uses Zed's official extension API for language server lifecycle management.
- **`basilisk-common`** — shares diagnostic codes and constants with the rest of the Basilisk workspace (also WASM-compatible).
- **Built-in Python, untouched** — binds to Zed's own Python language by name. The extension ships no `languages/` directory and no grammar, so Zed compiles nothing from source and your highlighting, brackets, indent rules, and runnables stay exactly as Zed ships them.
- **DAP debugging** — supports the Debug Adapter Protocol for integrated Python debugging.

## Building

From a monorepo checkout, build the extension and set up the local dev loop:

```sh
make package-zed
```

Standalone (this repository on its own), the build is exactly the one the release pipeline gates the publish on:

```sh
cargo build --release --target wasm32-wasip2
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `zed_extension_api` | Zed extension API |
| `basilisk-common` | Shared constants and types |

## License

MIT.
