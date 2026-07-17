# Chapter 4 checkpoint

This Signal Box snapshot supports Chapter 4's examples for annotations,
parameterized collections, unions, absence, and type aliases.

The source uses two syntax features with explicit language boundaries:

- built-in collection parameters such as `dict[str, float]` follow PEP 585
  and require Python 3.9 or later at runtime;
- union expressions such as `str | float | None` follow PEP 604 and require
  Python 3.10 or later at runtime.

Those are syntax boundaries for this checkpoint, not Basilisk-wide support
targets. A project using an earlier interpreter can express the same lessons
with `typing.Dict`, `typing.List`, `typing.Union`, and `typing.Optional`.

From this directory, run:

```console
PYTHONPATH=src python3 -m unittest discover -s tests
basilisk check src
```
