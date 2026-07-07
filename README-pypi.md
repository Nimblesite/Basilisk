# Basilisk

**Basilisk is the only Python type checker with a perfect 100% score** on the
[official `python/typing` conformance results](https://github.com/python/typing/blob/main/conformance/results/results.html) —
published on the Python typing repository's own leaderboard, ahead of Pyright,
mypy, Pyrefly and ty. It is a strict-by-default type checker and complete
language server, written in Rust.

- Website: <https://www.basilisk-python.dev>
- Conformance: <https://www.basilisk-python.dev/docs/conformance/>
- Source: <https://github.com/Nimblesite/Basilisk>

This package (`basilisk-python`) bundles the native `basilisk` binary as a
Python wheel so it can be installed with `pip`/`uv`. It is the **same binary**
distributed via GitHub Releases, Homebrew and Scoop — the wheel is a convenience
for Python-managed environments, not a separate build.

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

## Acknowledgments

The bundled binary is built on [Ruff](https://github.com/astral-sh/ruff) by
[Astral](https://astral.sh/) (MIT) and [typeshed](https://github.com/python/typeshed)
(Apache-2.0), among other open-source projects. Full third-party notices:
<https://github.com/Nimblesite/Basilisk/blob/main/NOTICES>.

## License

MIT
