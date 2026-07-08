---
layout: layouts/docs.njk
title: "Basilisk for Zed — Install & Use the Python Extension"
description: "Install and use the Basilisk Python language server in the Zed editor. The matching binary downloads automatically with the extension — zero configuration, no separate install. Diagnostics, completions, debugging, and profiling."
keywords: basilisk, zed, zed editor, python, language server, lsp, install, extension, debugging, profiling, slash commands
date: 2026-02-28
dateModified: 2026-03-31
author: The Basilisk Project
eleventyNavigation:
  key: Zed
  parent: Installation
  order: 2
---

# Basilisk for Zed

Basilisk ships a native [Zed](https://zed.dev) extension that registers the Basilisk language server for Python. Once installed, Basilisk activates automatically for every `.py` file — diagnostics, completions, hover, go-to-definition, rename, code actions, formatting, inlay hints, debugging, and profiling.

## Install the extension

1. Open the extensions view: command palette (`Cmd+Shift+P`) → **zed: extensions**
2. Search for **Basilisk**
3. Click **Install**

That's it. Open a Python file and Basilisk is your language server.

> **Installing from source?** If you are working in a checkout of the repository, install it as a dev extension instead: command palette → **zed: install dev extension** → select the `basilisk-zed/` directory. Zed compiles the extension to WASM automatically — you never pre-build or copy a `.wasm` file.

## The binary comes with the extension

**You do not install the Basilisk binary separately.** On first activation the extension downloads the matching binary for your platform straight from the [GitHub release](https://github.com/Nimblesite/Basilisk/releases), caches it inside Zed's extension directory, and reuses it until a newer release appears. No `cargo install`, no Homebrew, no PATH setup — installing the extension is the whole process.

When a newer release is available, the extension logs an update notice; restart Zed to pick it up.

## Configure Basilisk settings

Basilisk works with zero configuration. To adjust behavior, add settings under `lsp.basilisk.settings` in your Zed `settings.json`:

```json
{
  "lsp": {
    "basilisk": {
      "settings": {
        "analysisMode": "wholeModule"
      }
    }
  },
  "languages": {
    "Python": {
      "language_servers": ["basilisk", "..."]
    }
  }
}
```

> The language server currently honors `analysisMode` (`wholeModule` or
> `openFilesOnly`) and the `testExplorer` settings. Other keys are accepted but
> not yet read by the server — see the [configuration reference](/docs/configuration/)
> for what is wired up today.

## Debugging

Press **F5** on a Python file to debug it. Basilisk brokers a `debugpy` session over the Debug Adapter Protocol — breakpoints, stepping, variables, the call stack, and watch expressions all work natively in Zed. See [Debugging](/docs/debugging/) for how the session is brokered.

## Slash commands

Basilisk registers slash commands in Zed's AI assistant panel for profiling, memory analysis, tests, and workspace insight. The profiling and memory commands are **guides**: each explains the matching `basilisk.profiler.*` / `basilisk.memory.*` language-server command and how to drive it — profiling itself runs through the LSP:

| Command | What it does |
|---------|--------------|
| `/profile` | How to start CPU profiling (optional PID) |
| `/profstop` | How to stop profiling and where results land |
| `/profsnapshot` | How to snapshot hotspots without stopping |
| `/memleak` | The memory-tracking workflow via `tracemalloc` |
| `/memstop` | How to stop memory tracking |
| `/memrefs <Type>` | How to walk the reference graph for a Python type |
| `/tests` | Discover pytest/unittest tests |
| `/runtests` | Run tests by node ID or file |
| `/testfile` | Run all tests in the current file |
| `/modules` | Show the workspace module tree |
| `/symbols <module>` | Show symbols in a module |
| `/health` | Type-coverage health statistics |
| `/basilisk` | Server info and command reference |

See the [Profiler](/docs/profiler/) guide for the full profiling workflow.

## Advanced: override the binary

You only need this for development (running a locally built binary) or to point Zed at a system install. Either set the path explicitly in `settings.json`:

```json
{
  "lsp": {
    "basilisk": {
      "binary": { "path": "/absolute/path/to/basilisk" }
    }
  }
}
```

…or set the `BASILISK_PATH` environment variable. The setting takes precedence over the environment variable; with neither set, the extension downloads the release binary (the default above).

## Next steps

- [Quick Start](/docs/quick-start/) — your first type check
- [Refactoring](/docs/refactoring/) — extract, inline, move, and more
- [Configuration](/docs/configuration/) — `pyproject.toml` reference
