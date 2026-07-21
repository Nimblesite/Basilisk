<p align="center">
  <img src="images/basilisk-logo.png" alt="Basilisk" width="160">
</p>

<h1 align="center">Basilisk</h1>

<p align="center"><strong>English</strong> · <a href="README.zh.md">简体中文</a></p>

<p align="center">
  <strong>The only Python type checker scoring 100% on the official <a href="https://github.com/python/typing/blob/main/conformance/results/results.html"><code>python/typing</code> conformance suite</a> — and the fastest we&rsquo;ve measured.</strong><br>
  Complete open-source Python dev environment in <strong>Rust</strong>: type checker, language server, debugger, profiler, plus VS Code, Cursor, Zed &amp; Neovim extensions. Strict by default.
</p>

<p align="center">
  <a href="https://www.basilisk-python.dev">Website</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/installation/">Install</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/quick-start/">Quick Start</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/rules/">Rules</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/refactoring/">Refactoring</a> &nbsp;&bull;&nbsp;
  <a href="https://www.basilisk-python.dev/docs/comparison/">Compare</a>
</p>

<p align="center">
  <a href="https://www.basilisk-python.dev/docs/conformance/"><strong><!--g:score-->100.0%<!--/g:score--> PEP conformance</strong></a> &mdash; <!--g:pass-->141<!--/g:pass--> of <!--g:total-->141<!--/g:total--> tests in the official
  <a href="https://github.com/python/typing/tree/6ef9f7719ecfff09dad8724ef42b621fd994fb5e/conformance"><code>python/typing</code></a>
  conformance suite (commit <code><!--g:short-->6ef9f77<!--/g:short--></code>), scored on the wheel-installed CLI in its default config by the real upstream harness.
  We target <code>python/typing@main</code> and ratchet the score up only.
</p>

## The only 100% checker &mdash; and the fastest according to our benchmarks

Basilisk is the **only** Python type checker with a perfect score on the official
[`python/typing` conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html):
**<!--g:score-->100.0%<!--/g:score-->** (<!--g:pass-->141<!--/g:pass-->/<!--g:total-->141<!--/g:total--> files, <!--g:caught-->970<!--/g:caught--> required errors caught, <!--g:fp-->0<!--/g:fp--> false positives),
measured by the real upstream harness on the wheel-installed CLI in its default config.

<p align="center">
  <img src="images/screenshot.png" alt="Basilisk in action — type checking, diagnostics, and refactoring in the editor" width="900">
</p>

And it is the **fastest checker we&rsquo;ve measured** &mdash; median cold full-file check, from scratch:

| Type checker | Median cold check |
| --- | --- |
| ⚡ **Basilisk** | **<!--g:benchBasilisk-->10<!--/g:benchBasilisk--> ms** |
| zuban | <!--g:benchZuban-->28<!--/g:benchZuban--> ms |
| ty | <!--g:benchTy-->40<!--/g:benchTy--> ms |
| Pyrefly | <!--g:benchPyrefly-->110<!--/g:benchPyrefly--> ms |
| Pyright | <!--g:benchPyright-->573<!--/g:benchPyright--> ms |
| mypy | <!--g:benchMypy-->574<!--/g:benchMypy--> ms |

Median cold full-file check across <!--g:benchCount-->26<!--/g:benchCount--> single-construct typing-spec stress fixtures on an <!--g:benchMachine-->Apple M4 Max<!--/g:benchMachine--> &mdash; lower is better. Basilisk&rsquo;s warm re-check drops to ~<!--g:benchWarm-->4<!--/g:benchWarm--> ms. Every figure is produced by [`hyperfine`](https://github.com/sharkdp/hyperfine) and committed per machine, so nothing here is hand-typed. **Clone the repo, run `make bench` on your own hardware, and send us the CSV &mdash; independent audits are welcome.** [Full benchmarks &amp; methodology &rarr;](https://www.basilisk-python.dev/docs/benchmarks/)



## Install

The CLI is on [PyPI as `basilisk-python`](https://pypi.org/project/basilisk-python/) — install it as a standalone tool; the installed command is `basilisk`:

```sh
uv tool install basilisk-python     # or: pipx install basilisk-python
```

Also available via Homebrew (`brew tap Nimblesite/tap && brew install basilisk`), Scoop, and
[GitHub Releases](https://github.com/Nimblesite/Basilisk/releases) — every channel ships the same
single Rust CLI, built from this repository at the same version, with no runtime dependencies.
Full options: [install guide](https://www.basilisk-python.dev/docs/install-cli/).

## Try it

The `examples/` folder has ready-to-go Python files:

```sh
basilisk check   examples/bad.py    # 8 typing-spec errors — always on, no config needed
basilisk analyze examples/bad.py    # the opt-in strictness warnings on the same file
basilisk analyze examples/good.py   # clean, even at full strictness
basilisk check   examples/mixed.py  # one real type error
basilisk check   examples/          # the whole folder at once
```

The two commands read one rule universe split by provenance ([`CHKARCH-COMMANDS`](docs/specs/CHECKER-ARCHITECTURE-SPEC.md)): `check` reports
the `pep`-tagged typing-spec rules and nothing else — that set is always on, and
while a config table may grade one of them down to `warning`/`info`, none may
switch it off. `analyze` reports the non-`pep` house rules, which stay silent
until a table selects them. Only `analyze` emits `BSK-` diagnostics.

## Standard-library types, online or offline

Basilisk resolves the standard library from [typeshed](https://github.com/python/typeshed).
By default it verifies `python/typeshed@main` over HTTPS once per run, gates and
caches the archive for up to 24 hours, and reports the source as unpinned. With no
network — or on any download failure — it falls back to the complete typeshed
`stdlib/` snapshot compiled into the binary, so stdlib types still work offline.

Pin an exact commit with `typeshed-commit = "<40-char sha>"` under
`[tool.basilisk]`; its cache is re-hashed on every reuse and remains until eviction
or caching is disabled. The pin fails closed rather than substituting another
commit. Alternatively, point `typeshed-path` at your own typeshed tree. Full options:
[configuration guide](https://www.basilisk-python.dev/docs/configuration/).



## Editors

One extension, the whole workflow: strict-by-default diagnostics, autocomplete, hover, go-to-definition, refactoring code actions, debugging, and profiling. No Node.js or Python runtime &mdash; a single Rust binary drives it all.

- **VS Code, Cursor &amp; Windsurf** &mdash; install from [Open VSX](https://open-vsx.org/)
- **Zed** &bull; **Neovim 0.10+**

Every diagnostic teaches: rustc-style output with a `help`, a `note`, and a link to a per-rule explainer. See the [full diagnostic reference](https://www.basilisk-python.dev/docs/rules/) and the [install guide](https://www.basilisk-python.dev/docs/installation/).



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
and [RUST-DEPENDENCY-LICENSES](RUST-DEPENDENCY-LICENSES).



---

## License

Basilisk source code is MIT licensed. Binary distributions also contain
third-party components under the licenses shipped beside each artifact.

Built by [NIMBLESITE PTY LTD](https://www.nimblesite.co).
