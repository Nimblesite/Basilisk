# Chapter 10 checkpoint

This is the final, deliberately adopted Signal Box checkpoint for Chapter 10.
The reviewed decoder has precise `TypedDict` contracts. The remaining
`calls_argument_type` debt is still present in `status.py` and is graded to a
warning by the ordinary root rule entry in `pyproject.toml`.

Verify the checked-in checkpoint from this directory:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=src \
  python3 -m unittest discover -s tests -v

basilisk check --color never
basilisk analyze --color never
basilisk adopt --status .
```

To replay the walkthrough without touching the checked-in checkpoint, copy the
directory elsewhere and restore its baseline files:

```sh
cp stages/decoder.before src/signal_box/legacy/decoder.py
cp stages/pyproject.before pyproject.toml

basilisk check --color never
basilisk analyze --color never
basilisk fix src/signal_box/legacy
diff -u stages/decoder.before src/signal_box/legacy/decoder.py
```

The release's default fix tier inserts `Any` placeholders. It does not prove
that the edit is complete or add a missing `Any` import. This staged input
already imports `Any`, so runtime tests can cover the generated result. Compare
it with `stages/decoder.after-safe-fix`, run both tests and analysis, then apply
the human-reviewed contract before adopting the remaining error:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=src \
  python3 -m unittest discover -s tests -v
basilisk analyze --color never
cp stages/decoder.reviewed src/signal_box/legacy/decoder.py
basilisk adopt .
basilisk adopt --status .
basilisk check --color never
```

Run `basilisk unadopt .` only in the replay copy: it deletes the root's warning
entries and restores the ancestor/default severity. To graduate instead, fix
the bad `status_label("offline")` call, then run `basilisk adopt .` again; the
recompute removes the warning entry whose rule no longer fires.
