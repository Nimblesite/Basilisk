# Chapter 7 checkpoint

This Signal Box snapshot supports Chapter 7's examples for dictionary-shaped
input, attribute-shaped domain data, closed choices, structural protocols,
generic containers, and overloads.

The checkpoint uses
[`typing.NotRequired`](https://docs.python.org/3/library/typing.html), which was
added to the standard library in Python 3.11. Run it with Python 3.11 or later.
That boundary belongs to this example; it is not a Basilisk-wide
Python-version target.

From this directory, run the runtime evidence:

```console
PYTHONPATH=src python3 -m unittest discover -s tests
```

Then check the same source statically with the Basilisk binary for the edition
you are reading:

```console
basilisk check --color never .
```

The committed checkpoint is clean. The deliberately incompatible snippets in
the chapter are explanatory variations, not files hidden with casts or
suppressions.
