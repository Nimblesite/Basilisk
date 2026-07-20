# Chapter 5 checkpoint

This Signal Box snapshot supports Chapter 5's examples for assignment,
argument, return, mutable-collection, and callback compatibility.

The final checkpoint is deliberately clean. The incompatible examples in the
chapter are short variations to try and then remove; they are not hidden in
the runnable project with suppressions.

The checkpoint source reuses the built-in collection syntax boundary introduced
in Chapter 4:

- built-in collection annotations such as `list[float]` follow PEP 585 and
  require Python 3.9 or later at runtime.

That is a syntax boundary for this checkpoint, not a Basilisk-wide support
target. The chapter prose also reuses Chapter 4's separately cited PEP 604
union syntax.

From this directory, run:

```console
PYTHONPATH=src python3 -m unittest discover -s tests
basilisk check .
```
