# Python Typing Conformance PR

Use `scripts/prepare_typing_conformance_pr.py` to prepare an upstream
`python/typing` checkout for a Basilisk conformance PR.

The script makes the upstream harness install and run `basilisk` from the
`basilisk-python` wheel. It injects a `BasiliskTypeChecker`, adds the wheel
dependency, refreshes `uv.lock`, runs `src/main.py --only-run basilisk`, and
leaves generated `results/basilisk/*.toml` files in the checkout.

Typical release proof:

```bash
python3 scripts/prepare_typing_conformance_pr.py \
  --typing-repo ../typing \
  --verbose \
  --write-proof
```

Pre-PyPI local wheel proof:

```bash
python3 scripts/prepare_typing_conformance_pr.py \
  --typing-repo ../typing \
  --wheel dist/basilisk_python-*.whl \
  --verbose \
  --write-proof
```

Submit the resulting upstream diff from the `python/typing` checkout. The
expected upstream files are:

- `conformance/pyproject.toml`
- `conformance/uv.lock`
- `conformance/src/type_checker.py`
- `conformance/results/basilisk/*.toml`
- `conformance/results/results.html`

Basilisk-local scoring is not submission proof. Upstream submission proof must
come from the wheel-installed `basilisk` command running through the real
`python/typing` harness.
