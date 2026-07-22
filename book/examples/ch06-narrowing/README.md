# Chapter 6 checkpoint

This Signal Box snapshot supports Chapter 6's examples for inference,
control-flow narrowing, user-defined type guards, and exhaustive routing.

The final checkpoint is deliberately clean. The incomplete branches in the
chapter are short variations to try and then repair; they are not hidden with
casts or suppressions.

The checkpoint uses `T | None` syntax and structural pattern matching, so its
runtime interpreter must be Python 3.10 or later. That is a syntax boundary for
this example, not a Basilisk-wide support target.

From this directory, run:

```console
PYTHONPATH=src python3 -m unittest discover -s tests
```

This checkpoint intentionally makes no claim about Basilisk's current
inference output. Chapter 6 uses normative typing rules only and leaves
unspecified or incompletely implemented inference detail out.
