# Chapter 5 — Compatibility is the question

*Part II — Think in types*

> **Reader promise:** Predict the ordinary assignment, call-argument, and return
> errors that appear when a value crosses a typed boundary.

## Can this value be used here?

Make assignability the repeated question behind several different-looking
diagnostics. Use concrete source and destination types before introducing the
more general relationship.

## Assignment and subtyping

Show compatible and incompatible assignments from Signal Box. Use structural
examples only after the reader can explain the simpler cases.

## Functions receive and promise

Treat parameter types as accepted inputs and return annotations as caller-facing
promises. Demonstrate an honest broad input and a precise output rather than
making both sides unnecessarily narrow.

## Mutable collections change the stakes

Use a read and a write through the same list to show why a tempting substitution
can be unsafe. Introduce variance vocabulary only after the concrete write makes
the problem visible.

## Callbacks are contracts too

End with one callback used by the alert pipeline. Keep callable compatibility
practical and defer advanced callable forms to official reference material.

## Signal Box checkpoint

Repair the formatter's input, its return, and one mutable collection boundary
without adding a cast or `Any`.

## Authoritative sources

- [Type system concepts](https://typing.python.org/en/latest/spec/concepts.html)
- [Callables](https://typing.python.org/en/latest/spec/callables.html)
- [Generics](https://typing.python.org/en/latest/spec/generics.html)
- Inspect the relevant live entries in the
  [Basilisk rule reference](https://www.basilisk-python.dev/docs/rules/).

