# basilisk-zed

<p align="center"><strong>English</strong> · <a href="README.zh.md">简体中文</a></p>

Zed editor extension for Basilisk — WASM-based Python type checking and language server integration.

Basilisk is an open-source Python type checker and language server built in Rust: diagnostics, autocomplete, refactoring, debugging, and profiling, with strictness configured per rule.

<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/website/src/assets/images/zed-screenshot.png" alt="Basilisk in the Zed editor — Python type checking and diagnostics inline" width="900">
</p>

> ## ⚠️ Do not use Basilisk's type checker in your pipeline
>
> **The type checker still contains code that isn't doing real type checking, and it is not yet trustworthy.** Some rules decide from the way code is *spelled* rather than what it means, so they can be wrong in both directions — a false error on correct code, or silence where there is a genuine bug. Don't gate CI on it, and don't read a clean run as a clean codebase. Our former conformance claim and our benchmark figures are withdrawn, and Basilisk was [removed from the official results](https://github.com/python/typing/blob/main/conformance/results/results.html) at our request.
>
> **This was a mistake and a failure to verify — not an attempt to game the suite.** Nothing was concealed from `python/typing`; the submission ran the suite's own unmodified harness, and we published on a green run without ever checking whether our rules survived a semantics-preserving change. Basilisk's author has published a [personal account and apology](https://www.christianfindlay.com/blog/basilisk-conformance-apology).
>
> **We are auditing every rule and deleting the ones that don't hold up** — not rewriting them, not patching them, with a failing test left behind so the gap stays visible. Where a rule can't be made reliable in a straightforward way, we will depend on a different, established type checker rather than ship our own unreliable version of it.
>
> **Basilisk is much more than a type checker.** The language server, refactoring, formatting, debugging, and profiling don't rest on the rules under audit — those are what we are sharpening while it runs, removing anything that could hand you a misleading result. We are doing this to restore trust and turn Basilisk back into a tool you can believe. [Read the correction](https://www.basilisk-python.dev/docs/conformance/).

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
