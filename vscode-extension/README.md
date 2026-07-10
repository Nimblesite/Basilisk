<p align="center">
  <img src="https://basilisk-python.dev/assets/images/favicon.png" alt="Basilisk" width="140">
</p>

<h1 align="center">Basilisk for VS Code</h1>

<p align="center"><strong>English</strong> · <a href="https://github.com/Nimblesite/Basilisk/blob/main/vscode-extension/README.zh.md">简体中文</a></p>

<p align="center">
  <strong>The only Python type checker scoring 100% on the official <a href="https://github.com/python/typing/blob/main/conformance/results/results.html"><code>python/typing</code> conformance suite</a> — and the fastest we&rsquo;ve measured.</strong><br>
  Complete open-source Python dev environment in <strong>Rust</strong>: type checker, language server, debugger, profiler, plus VS Code, Cursor, Zed &amp; Neovim extensions. Strict by default.
</p>

<p align="center">
  <a href="https://www.basilisk-python.dev">Website</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/quick-start/">Quick Start</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/rules/">Rules</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/conformance/">Conformance</a> &nbsp;&bull;&nbsp;
  <a href="https://github.com/Nimblesite/Basilisk">GitHub</a>
</p>

<p align="center">
  <a href="https://github.com/python/typing/blob/main/conformance/results/results.html"><strong><!--g:score-->100.0%<!--/g:score--> PEP conformance</strong></a> — <!--g:pass-->141<!--/g:pass--> of <!--g:total-->141<!--/g:total--> tests in the official
  <a href="https://github.com/python/typing/tree/main/conformance"><code>python/typing</code></a>
  conformance suite, scored on the wheel-installed CLI in its default config by the real upstream harness. The only checker on the board at 100%.
</p>

## The only 100% checker — and the fastest

Basilisk is the **only** Python type checker with a perfect score on the official
[`python/typing` conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html):
**<!--g:score-->100.0%<!--/g:score-->** (<!--g:pass-->141<!--/g:pass-->/<!--g:total-->141<!--/g:total--> files, <!--g:caught-->970<!--/g:caught--> required errors caught, <!--g:fp-->0<!--/g:fp--> false positives),
measured by the real upstream harness on the wheel-installed CLI in its default config.

<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/vscode-extension/images/screenshot.png" alt="Basilisk in action — type checking, diagnostics, and refactoring in VS Code" width="900">
</p>

And it is the **fastest checker we&rsquo;ve measured** &mdash; median cold full-file check, from scratch:

| Type checker | Median cold check |
| --- | --- |
| ⚡ **Basilisk** | **<!--g:benchBasilisk-->12<!--/g:benchBasilisk--> ms** |
| zuban | <!--g:benchZuban-->27<!--/g:benchZuban--> ms |
| ty | <!--g:benchTy-->37<!--/g:benchTy--> ms |
| Pyrefly | <!--g:benchPyrefly-->145<!--/g:benchPyrefly--> ms |
| Pyright | <!--g:benchPyright-->568<!--/g:benchPyright--> ms |
| mypy | <!--g:benchMypy-->588<!--/g:benchMypy--> ms |

Median cold full-file check across <!--g:benchCount-->26<!--/g:benchCount--> single-construct typing-spec stress fixtures on an <!--g:benchMachine-->Apple M4 Max<!--/g:benchMachine--> &mdash; lower is better; inside the editor a warm re-check is faster again. Every figure is produced by [`hyperfine`](https://github.com/sharkdp/hyperfine) and committed per machine, so nothing here is hand-typed. **Clone the repo, run `make bench` on your own hardware, and send us the CSV &mdash; independent audits are welcome.** [Full benchmarks &amp; methodology &rarr;](https://www.basilisk-python.dev/docs/benchmarks/)

## Everything in one extension

One extension replaces Pylance and gives you the whole workflow — no Node.js, no Python runtime, no pip, no npm. A single bundled Rust binary drives it all:

- **Strict-by-default diagnostics** — inline as you type, incremental analysis powered by Salsa (the rust-analyzer engine)
- **Autocomplete, hover, go-to-definition, find references, rename**
- **Refactoring code actions** — extract, inline, move symbol, organize imports
- **Integrated debugging** — F5 to debug via bundled debugpy; no separate extension
- **Integrated profiling** — CPU heat map, flame graph, and a memory dashboard with leak detection
- **Activity panel** — module tree with per-module type-health coverage, plus feature toggles
- **Inlay hints** and **Ruff** formatting/import-organization, built in

Every diagnostic teaches: rustc-style output with a `help`, a `note`, and a link to a per-rule explainer, so a red squiggle always tells you *why*. Other checkers default to permissive; Basilisk **starts strict** and stays strict.

## Zero install

The Basilisk binary is bundled with this extension for macOS (Apple Silicon), Linux (x86_64, aarch64), and Windows (x86_64, aarch64). Install the extension and go.

Want the `basilisk` CLI on your PATH too (for CI or the terminal)? `brew install Nimblesite/tap/basilisk`, `scoop install basilisk` (after `scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket`), or grab a binary from [GitHub Releases](https://github.com/Nimblesite/Basilisk/releases). Point `basilisk.executablePath` at it to use your own build.

## Acknowledgments

Built on [Ruff](https://github.com/astral-sh/ruff) by [Astral](https://astral.sh/) (MIT) and [typeshed](https://github.com/python/typeshed) (Apache-2.0); bundles [debugpy](https://github.com/microsoft/debugpy) (Microsoft, MIT). Full notices: [NOTICES](https://github.com/Nimblesite/Basilisk/blob/main/NOTICES).

## License

MIT.

Built by [NIMBLESITE PTY LTD](https://www.nimblesite.co).
