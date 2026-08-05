<!-- GENERATED FILE — DO NOT EDIT.
     Source: docs/readme/README.src.md · Regenerate: python3 scripts/gen_readmes.py
     Spec: docs/specs/DOCS-README-SPEC.md [README] -->
<p align="center">
  <img src="images/basilisk-logo.png" alt="Basilisk" width="160">
</p>

<h1 align="center">Basilisk</h1>

<p align="center"><strong>English</strong> · <a href="README.zh.md">简体中文</a></p>

<p align="center">
  <strong>Open-source Python type checking and developer tooling in Rust.</strong><br>
  Complete open-source Python dev environment in <strong>Rust</strong>: type checker, language server, debugger, profiler, plus VS Code, Cursor, Zed &amp; Neovim extensions. Strict by default.
</p>

> **You are reading the Basilisk source repository** — the checker, language server, editor extensions, and website all live here.

<p align="center">
  <a href="https://www.basilisk-python.dev">Website</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/installation/">Install</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/quick-start/">Quick Start</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/rules/">Rules</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/refactoring/">Refactoring</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/comparison/">Compare</a> &nbsp;&bull;&nbsp;
  <a href="https://github.com/Nimblesite/Basilisk">GitHub</a>
</p>

<p align="center">
  <strong>Current conformance: temporarily unknown.</strong> The former result and all published benchmark figures have been withdrawn pending a clean reimplementation and audit.
</p>

## Conformance and benchmark results withdrawn

> **Integrity notice:** We have retracted Basilisk&rsquo;s former 100% conformance claim. The result was not trustworthy: parts of the checker had been fitted to the exact text of upstream tests, and the score was not stable under semantics-preserving mutations such as consistent renames. At our request, Basilisk has been removed from the official [`python/typing` results table](https://github.com/python/typing/blob/main/conformance/results/results.html). Its actual conformance level is temporarily unknown.
>
> We have also withdrawn every published benchmark figure and performance ranking while we audit the measurement pipeline. We are deleting the fitted checker code and reimplementing the affected logic from the Python typing specification. Before we publish a replacement score or seek relisting, mutation tests &mdash; including semantics-preserving renames &mdash; must show that the result is robust, and independent off-suite cases derived from the specification must confirm the repaired behavior. We will publish new conformance and benchmark results as soon as they are trustworthy, even if they are lower or slower than the figures withdrawn here. [Read the conformance audit and recovery plan &rarr;](https://www.basilisk-python.dev/docs/conformance/)

<p align="center">
  <img src="images/screenshot.png" alt="Basilisk in action — type checking, diagnostics, and refactoring in the editor" width="900">
</p>

## Everything in one extension

One extension replaces Pylance and gives you the whole workflow — no Node.js, no Python runtime, no pip, no npm. A single bundled Rust binary drives it all:

- **Strict-by-default diagnostics** — inline as you type, incremental analysis powered by Salsa (the rust-analyzer engine)
- **Autocomplete, hover, go-to-definition, find references, rename**
- **Refactoring code actions** — extract, inline, move symbol, organize imports
- **Integrated debugging** — F5 to debug via bundled debugpy; no separate extension
- **Integrated profiling** — CPU heat map, flame graph, and a memory dashboard with leak detection
- **Activity panel** — module tree with per-module type-health coverage, plus feature toggles
- **Inlay hints** and **Ruff** formatting/import-organization, built in
- **Standard-library types from [typeshed](https://github.com/python/typeshed)** — a complete `stdlib/` snapshot is compiled into the binary, so hover and diagnostics work offline with no configuration

Every diagnostic teaches: rustc-style output with a `help`, a `note`, and a link to a per-rule explainer, so a red squiggle always tells you *why*. Basilisk **starts strict** and stays strict — the unconfigured default enables every currently registered PEP-tagged rule, and strictness is dialled per rule, never by a mode. That is a configuration property, not evidence that those rules are complete or correct.

## Install

**Editor extension** — install *Basilisk* from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=Nimblesite.basilisk) or [Open VSX](https://open-vsx.org/extension/Nimblesite/basilisk) (Cursor, Windsurf, and other forks read Open VSX). The Basilisk binary is bundled for macOS (Apple Silicon), Linux (x86_64, aarch64), and Windows (x86_64, aarch64) — nothing else to install. Zed and Neovim 0.10+ extensions are available too.

**CLI** — on [PyPI as `basilisk-python`](https://pypi.org/project/basilisk-python/); the installed command is `basilisk`:

```sh
uv tool install basilisk-python     # or: pipx install basilisk-python, pip install basilisk-python
```

Also via Homebrew (`brew install Nimblesite/tap/basilisk`), Scoop (`scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket && scoop install basilisk`), and [GitHub Releases](https://github.com/Nimblesite/Basilisk/releases). Every channel ships the same single Rust CLI, built from this repository at the same version, with no runtime dependencies. Point `basilisk.executablePath` at your own build to have the extension use it. Full options: [install guide](https://www.basilisk-python.dev/docs/installation/).

## Try it

The [`examples/`](examples/) folder has ready-to-go Python files:

```sh
basilisk check   examples/bad.py    # 8 typing-spec errors — always on, no config needed
basilisk analyze examples/bad.py    # the opt-in strictness warnings on the same file
basilisk analyze examples/good.py   # clean, even at full strictness
basilisk check   examples/mixed.py  # one real type error
basilisk check   examples/          # the whole folder at once
```

Machine-readable output for CI and tooling:

```sh
basilisk check path/to/your_code.py --output json --color never
```

The two commands read one rule universe split by provenance ([`CHKARCH-COMMANDS`](docs/specs/CHECKER-ARCHITECTURE-SPEC.md)): `check` reports
the `pep`-tagged typing-spec rules and nothing else — that set is always on, and
while a config table may grade one of them down to `warning`/`info`, none may
switch it off. `analyze` reports the non-`pep` house rules, which stay silent
until a table selects them. Only `analyze` emits `BSK-` diagnostics.

## Standard-library types, always offline

Basilisk resolves the standard library from [typeshed](https://github.com/python/typeshed),
and checking **never downloads anything**. Out of the box it uses the complete
typeshed `stdlib/` snapshot compiled into the binary, reporting the source as
unpinned — so stdlib types work on a plane, behind a firewall, or in an
air-gapped CI runner, with no configuration.

Pin an exact commit with `typeshed-commit = "<40-char sha>"` under
`[tool.basilisk]`. A pin does exactly one thing: it verifies, offline, that the
typeshed tree in the local store hashes to that commit. If the commit is not on
this machine the run fails hard with `NO SOURCE` rather than substituting
another source — bring it down first with `basilisk typeshed download` (with no
`--commit` it downloads the latest and writes the pin for you), or use the
editor's **Download latest** button. Alternatively, point `typeshed-path` at
your own typeshed tree. Full options:
[configuration guide](https://www.basilisk-python.dev/docs/configuration/).

## Development

```sh
cargo build          # build all crates
cargo test           # run all tests
cargo clippy         # lint (zero warnings policy)
cargo fmt            # format
```

Rust 1.87+ required.

## Contributing

Basilisk is built by a human + AI partnership, with the work split on purpose. See
[CONTRIBUTING.md](CONTRIBUTING.md) — **For Humans** (testing, code-quality review,
conformance/security audits, IDE feature parity, sharpening the AI instructions) and
**For AI** (the technical execution, under the standing rules in [CLAUDE.md](CLAUDE.md)).

## Acknowledgments

Basilisk builds on the open-source community — with thanks to:

- **[Astral](https://astral.sh/)** — [Ruff](https://github.com/astral-sh/ruff), whose parser, AST, and formatter crates Basilisk embeds (MIT). The foundation we rely on most.
- **[typeshed](https://github.com/python/typeshed)** — standard-library type stubs (Apache-2.0, with MIT-licensed parts).
- **[Salsa](https://github.com/salsa-rs/salsa)** — incremental query engine.
- **[Rayon](https://github.com/rayon-rs/rayon)** — data parallelism.
- **[tower-lsp](https://github.com/ebkalderon/tower-lsp)** — LSP scaffolding.
- **[debugpy](https://github.com/microsoft/debugpy)** — debug adapter (bundled in the VS Code extension).
- The [`python/typing`](https://github.com/python/typing) conformance suite.

Full component list, selected licenses, and required notices: [NOTICES](NOTICES)
and [RUST-DEPENDENCY-LICENSES](RUST-DEPENDENCY-LICENSES). Each published
artifact carries its own copies: the VSIX ships Rust notices in
`RUST-DEPENDENCY-LICENSES`, npm notices in `VSCODE-DEPENDENCY-LICENSES`, and
debugpy's license and `ThirdPartyNotices.txt` inside `bundled/debugpy`; the
wheel carries the complete locked notices in its `.dist-info/licenses/`
directory.

---

## License

Basilisk source code is MIT licensed. Binary distributions also contain
third-party components under the licenses shipped beside each artifact.

Built by [NIMBLESITE PTY LTD](https://www.nimblesite.co).
