# Basilisk

**Basilisk is the only Python type checker scoring 100%** on the
[official `python/typing` conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html) —
and the fastest we've measured. A complete, open-source Python development
environment in Rust: type checker, language server, debugger, profiler, plus
VS Code, Cursor, Zed & Neovim extensions. Strict by default.

- Website: <https://www.basilisk-python.dev>
- Conformance: <https://www.basilisk-python.dev/docs/conformance/>
- Source: <https://github.com/Nimblesite/Basilisk>

This package (`basilisk-python`) bundles the native `basilisk` binary as a
Python wheel so it can be installed with `pip`/`uv`. It is built from the **same
source, at the same version**, as the binaries distributed via GitHub Releases,
Homebrew and Scoop — every channel is stamped from the Cargo workspace version
by `scripts/stamp-version.sh` in one release run. The wheel is compiled by its
own `maturin --release` job, so the file itself is not byte-identical to the
release archive's binary.

> The distribution is named `basilisk-python` because the name `basilisk` was
> already taken on PyPI. The installed command is still `basilisk`.

## Install

```bash
pip install basilisk-python
# or
uv tool install basilisk-python
```

## Use

```bash
basilisk check path/to/your_code.py
basilisk --version
```

Machine-readable output for tooling:

```bash
basilisk check path/to/your_code.py --output json --color never
```

## Standard-library types

Standard-library types come from [typeshed](https://github.com/python/typeshed).
By default Basilisk verifies `python/typeshed@main` over HTTPS once per run and
caches the gated archive for 24 hours; with no network it falls back to the
complete typeshed `stdlib/` snapshot compiled into the binary, so offline and
air-gapped runs still get stdlib types. Pin an exact commit with
`typeshed-commit` under `[tool.basilisk]` in `pyproject.toml`, or skip the cache
for a single run with `--no-typeshed-cache`.

## Acknowledgments

The bundled binary is built on [Ruff](https://github.com/astral-sh/ruff) by
[Astral](https://astral.sh/) (MIT) and [typeshed](https://github.com/python/typeshed)
(Apache-2.0, with MIT-licensed parts), among other open-source projects. The
wheel carries the complete locked notices and license texts in its
`.dist-info/licenses/` directory.

## License

Basilisk source code is MIT licensed. This binary wheel also contains
third-party components under the composite `License-Expression` and the exact
license files shipped in `.dist-info/licenses/`.
