<p align="center">
  <img src="images/basilisk-logo.png" alt="Basilisk" width="160">
</p>

<h1 align="center">Basilisk</h1>

<p align="center"><strong>English</strong> · <a href="README.zh.md">简体中文</a></p>

<p align="center">
  <strong>The open-source Python language server.</strong><br>
  The only Python type checker with a perfect 100% score on the official <a href="https://github.com/python/typing/blob/main/conformance/results/results.html"><code>python/typing</code> conformance results</a>.<br>
  Complete language server, type checker, debugger, and profiler — strict by default.<br>
  VS Code, Cursor &amp; Windsurf (Open VSX) &bull; Zed &bull; Neovim. Built in <strong>Rust</strong> — single binary, no runtime.
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
  <a href="https://github.com/python/typing/tree/f4f2952f3ac94d7af819c5c71b60a50a100370e0/conformance"><code>python/typing</code></a>
  conformance suite (commit <code><!--g:short-->f4f2952<!--/g:short--></code>), scored on the wheel-installed CLI in its default config by the real upstream harness.
  We target <code>python/typing@main</code> and ratchet the score up only.
</p>

## The only 100% checker &mdash; and the fastest

Basilisk is the **only** Python type checker with a perfect score on the official
[`python/typing` conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html):
**<!--g:score-->100.0%<!--/g:score-->** (<!--g:pass-->141<!--/g:pass-->/<!--g:total-->141<!--/g:total--> files, <!--g:caught-->970<!--/g:caught--> required errors caught, <!--g:fp-->0<!--/g:fp--> false positives),
measured by the real upstream harness on the wheel-installed CLI in its default config.

<p align="center">
  <img src="images/screenshot.png" alt="Basilisk in action — type checking, diagnostics, and refactoring in the editor" width="900">
</p>

And it is the **fastest checker we&rsquo;ve measured** &mdash; on every rule, checked cold from scratch:

| Type checker | Median cold check |
| --- | --- |
| ⚡ **Basilisk** | **<!--g:benchBasilisk-->12<!--/g:benchBasilisk--> ms** |
| zuban | <!--g:benchZuban-->27<!--/g:benchZuban--> ms |
| ty | <!--g:benchTy-->37<!--/g:benchTy--> ms |
| Pyrefly | <!--g:benchPyrefly-->145<!--/g:benchPyrefly--> ms |
| Pyright | <!--g:benchPyright-->558<!--/g:benchPyright--> ms |
| mypy | <!--g:benchMypy-->582<!--/g:benchMypy--> ms |

Median cold full-file check across <!--g:benchCount-->29<!--/g:benchCount--> single-rule stress fixtures on an <!--g:benchMachine-->Apple M4 Max<!--/g:benchMachine--> &mdash; lower is better. Basilisk&rsquo;s warm re-check drops to ~<!--g:benchWarm-->5<!--/g:benchWarm--> ms. Every figure is produced by [`hyperfine`](https://github.com/sharkdp/hyperfine) and committed per machine, so nothing here is hand-typed. **Clone the repo, run `make bench` on your own hardware, and send us the CSV &mdash; independent audits are welcome.** [Full benchmarks &amp; methodology &rarr;](https://www.basilisk-python.dev/docs/benchmarks/)



## Try it

The `examples/` folder has ready-to-go Python files:

```sh
basilisk check examples/bad.py    # everything flagged
basilisk check examples/good.py   # clean
basilisk check examples/mixed.py  # some errors, some clean
basilisk check examples/          # all three at once
```



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
- **[typeshed](https://github.com/python/typeshed)** — standard-library type stubs (Apache-2.0).
- **[Salsa](https://github.com/salsa-rs/salsa)** — incremental query engine.
- **[Rayon](https://github.com/rayon-rs/rayon)** — data parallelism.
- **[tower-lsp](https://github.com/ebkalderon/tower-lsp)** — LSP scaffolding.
- **[debugpy](https://github.com/microsoft/debugpy)** — debug adapter (bundled in the VS Code extension).
- The [`python/typing`](https://github.com/python/typing) conformance suite.

Full component list and licenses: [NOTICES](NOTICES). All dependencies are permissively licensed.



## License

MIT.

Built by [NIMBLESITE PTY LTD](https://www.nimblesite.co).
