# Chapter 9 checkpoint

This checkpoint keeps Python typing-spec diagnostics and opt-in project policy
in separate command lanes. From this directory, run:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=src \
  python3 -m unittest discover -s tests -v

basilisk check --color never
basilisk analyze --color never
```

Under Basilisk 0.39.0, `check` reports no diagnostics. `analyze` reports the
missing parameter and return annotations in `src/` as errors, then reports the
missing return annotation in `tests/` as a warning. The difference comes from
`tests/pyproject.toml`, whose nearer `BSK-0002` entry overrides the root entry
for files in that folder only.
