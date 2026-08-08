<!-- GENERATED FILE — DO NOT EDIT.
     Source: docs/readme/README.src.md · Regenerate: python3 scripts/gen_readmes.py
     Spec: docs/specs/DOCS-README-SPEC.md [README] -->
<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/images/basilisk-logo.png" alt="Basilisk" width="160">
</p>

<h1 align="center">Basilisk</h1>

<p align="center"><strong>English</strong> · <a href="https://github.com/Nimblesite/Basilisk/blob/main/README.zh.md">简体中文</a></p>

<p align="center">
  <strong>An open-source Python type checker and language server, built in Rust.</strong><br>
  One extension for the whole workflow &mdash; diagnostics, autocomplete, refactoring, formatting, debugging, and profiling &mdash; driven by a single bundled binary.
</p>

> **You are reading the `basilisk-python` wheel listing** — the Basilisk CLI packaged for `pip`/`uv`. The distribution is named `basilisk-python` because `basilisk` was taken on PyPI; the installed command is still `basilisk`.

<p align="center">
  <a href="https://www.basilisk-python.dev">Website</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/installation/">Install</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/quick-start/">Quick Start</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/rules/">Rules</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/refactoring/">Refactoring</a> &nbsp;&bull;&nbsp;
  <a href="https://github.com/Nimblesite/Basilisk">GitHub</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/Nimblesite/Basilisk/main/images/screenshot.png" alt="Basilisk in action — type checking, diagnostics, and refactoring in the editor" width="900">
</p>

> ## ⚠️ Do not use Basilisk's type checker in your pipeline
>
> **The type checker still contains code that isn't doing real type checking, and
> it is not yet trustworthy.** Some rules decide from the way code is *spelled*
> rather than what it means, so they can be wrong in both directions — a false
> error on correct code, or silence where there is a genuine bug. Until the audit
> below is finished, don't gate CI on `basilisk check`, don't block a merge with
> it, and don't read a clean run as a clean codebase.
>
> The rest of Basilisk — language server, refactoring, formatting, debugging,
> profiling — does not depend on those rules and is unaffected.

## Restoring trust: audit, delete, and lean on a checker that works

We withdrew our former conformance claim and our benchmark figures, and asked to be
[removed from the official `python/typing` results](https://github.com/python/typing/blob/main/conformance/results/results.html).
The cause was checker logic fitted to the contents of conformance test files
instead of implementing the typing specification generally: rules that matched
the *spelling* of code rather than its meaning. Rename an import or reformat a
file and the answer changed. A score produced that way is not evidence.

**This was a mistake and a failure to verify — not an attempt to game the suite.**
Nobody set out to defeat the conformance tests, and nothing was concealed from
`python/typing`: the submission ran the suite's own unmodified harness, with
default configuration and every rule enabled. Our process treated the score as
the goal, matching text raises a score faster than real analysis does, and we
published without ever asking whether a rule still held when the same program was
spelled differently. Basilisk's author has published a
[personal account and apology](https://www.christianfindlay.com/blog/basilisk-conformance-apology).

**So we are auditing every rule and deleting the ones that don't do real type
checking.** Not rewriting them, not patching them, not marking them TODO —
deleting them, with a failing test left behind so the gap is visible instead of
hidden. A rule stays only if it decides from the resolved syntax tree and gives
the same answer when the code is spelled differently.

**Where a rule can't be made reliable in a straightforward way, we will depend on
a different, established type checker rather than ship our own unreliable version
of it.** An answer from an engine that has earned trust is worth more to you than
a Basilisk-branded one that hasn't. No replacement figure gets published until it
survives off-suite and mutation testing.

That means Basilisk gets **smaller** before it gets better. Expect fewer rules,
fewer diagnostics, and a lower conformance number. We will report each drop
rather than avoid it. What is left will be code that is honest about what it
does — nothing else.

### Basilisk is much more than a type checker

Type checking is one part of it. The rest is a complete Python workflow in a
single Rust binary — language server, refactoring, formatting, integrated
debugging, profiling, and the editor extensions — and none of it rests on the
rules under audit. That is what we are sharpening while the audit runs: make the
parts that are genuinely useful solid, and remove anything that could hand you a
misleading result. The point of getting smaller is to end up with a tool you can
believe.

[Read the full correction &rarr;](https://www.basilisk-python.dev/docs/conformance/) &nbsp;&bull;&nbsp;
[Integrity audit &rarr;](https://github.com/Nimblesite/Basilisk/blob/main/docs/CONFORMANCE-INTEGRITY-AUDIT.md)

## What you get

One extension covers the whole Python workflow. A single bundled Rust binary
drives it — no Node.js, no npm, no `pip install`:

- **Diagnostics as you type** — incremental analysis powered by [Salsa](https://github.com/salsa-rs/salsa)
- **Autocomplete, hover, go-to-definition, find references, rename**
- **Refactoring code actions** — extract, inline, move symbol, organize imports
- **Integrated debugging** — F5 to debug via bundled [debugpy](https://github.com/microsoft/debugpy); no separate extension
- **Integrated profiling** — CPU heat map, flame graph, and a memory dashboard with leak detection
- **Activity panel** — module tree with per-module type-health coverage, plus feature toggles
- **Inlay hints** and **Ruff** formatting/import-organization, built in
- **Standard-library types from [typeshed](https://github.com/python/typeshed)** — a complete `stdlib/` snapshot is compiled into the binary, so hover and diagnostics work offline with no configuration

Strictness is configured **per rule**, never by a mode: the unconfigured default
enables the typing-spec rule set, and each rule can be graded down to
`warning`/`info` so a codebase can adopt type safety incrementally. Every
diagnostic carries a `help`, a `note`, and a link to a per-rule explainer, so a
red squiggle tells you *why*.

## Install

**Editor extension** — install *Basilisk* from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=Nimblesite.basilisk) or [Open VSX](https://open-vsx.org/extension/Nimblesite/basilisk) (Cursor, Windsurf, and other forks read Open VSX). The Basilisk binary is bundled for macOS (Apple Silicon), Linux (x86_64, aarch64), and Windows (x86_64, aarch64) — nothing else to install. Zed and Neovim 0.10+ extensions are available too.

**CLI** — on [PyPI as `basilisk-python`](https://pypi.org/project/basilisk-python/); the installed command is `basilisk`:

```sh
uv tool install basilisk-python     # or: pipx install basilisk-python, pip install basilisk-python
```

Also via Homebrew (`brew install Nimblesite/tap/basilisk`), Scoop (`scoop bucket add nimblesite https://github.com/Nimblesite/scoop-bucket && scoop install basilisk`), and [GitHub Releases](https://github.com/Nimblesite/Basilisk/releases). Every channel ships the same single Rust CLI, built from this repository at the same version, with no runtime dependencies. Point `basilisk.executablePath` at your own build to have the extension use it. Full options: [install guide](https://www.basilisk-python.dev/docs/installation/).

## Try it

The [`examples/`](https://github.com/Nimblesite/Basilisk/blob/main/examples/) folder has ready-to-go Python files:

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

The two commands read one rule universe split by provenance ([`CHKARCH-COMMANDS`](https://github.com/Nimblesite/Basilisk/blob/main/docs/specs/CHECKER-ARCHITECTURE-SPEC.md)): `check` reports
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
[CONTRIBUTING.md](https://github.com/Nimblesite/Basilisk/blob/main/CONTRIBUTING.md) — **For Humans** (testing, code-quality review,
conformance/security audits, IDE feature parity, sharpening the AI instructions) and
**For AI** (the technical execution, under the standing rules in [CLAUDE.md](https://github.com/Nimblesite/Basilisk/blob/main/CLAUDE.md)).

## Acknowledgments

Basilisk builds on the open-source community — with thanks to:

- **[Astral](https://astral.sh/)** — [Ruff](https://github.com/astral-sh/ruff), whose parser, AST, and formatter crates Basilisk embeds (MIT). The foundation we rely on most.
- **[typeshed](https://github.com/python/typeshed)** — standard-library type stubs (Apache-2.0, with MIT-licensed parts).
- **[Salsa](https://github.com/salsa-rs/salsa)** — incremental query engine.
- **[Rayon](https://github.com/rayon-rs/rayon)** — data parallelism.
- **[tower-lsp](https://github.com/ebkalderon/tower-lsp)** — LSP scaffolding.
- **[debugpy](https://github.com/microsoft/debugpy)** — debug adapter (bundled in the VS Code extension).
- The [`python/typing`](https://github.com/python/typing) conformance suite.

Full component list, selected licenses, and required notices: [NOTICES](https://github.com/Nimblesite/Basilisk/blob/main/NOTICES)
and [RUST-DEPENDENCY-LICENSES](https://github.com/Nimblesite/Basilisk/blob/main/RUST-DEPENDENCY-LICENSES). Each published
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
