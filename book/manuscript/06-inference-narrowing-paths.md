# Chapter 6 — Inference, narrowing, and all the paths

*Part II — Think in types*

> **Reader promise:** Follow the type information through control flow and
> recognize when a branch has left a possible value unhandled.

## Inference starts with evidence

Show what Basilisk can learn from literals, expressions, calls, and declared
boundaries. Keep inference distinct from the separate policy choice to require
written annotations.

## Conditions change what remains possible

Trace both branches of `is None`, `isinstance`, membership, and equality checks.
Label the remaining type on the positive and negative path.

## Teach a predicate with `TypeGuard`

Use a dictionary validation function to narrow only its successful branch to a
`TypedDict`. State the trust placed in the guard's implementation.

## Match every closed case

Use an enum or literal union to demonstrate exhaustive pattern matching and an
unhandled path. Connect unreachable code to a closed set of possibilities rather
than presenting it as a mysterious checker feature.

## Signal Box checkpoint

Route absent, numeric, and error readings without a blind cast. Add a new
variant and predict every place that becomes non-exhaustive.

## Authoritative sources

- [Type narrowing](https://typing.python.org/en/latest/spec/narrowing.html)
- [Unreachable code and exhaustiveness](https://typing.python.org/en/latest/guides/unreachable.html)
- [PEP 647 — User-defined type guards](https://peps.python.org/pep-0647/)
- [Python 3.12 match statement](https://docs.python.org/3.12/reference/compound_stmts.html#the-match-statement)
- Use the [Basilisk rule reference](https://www.basilisk-python.dev/docs/rules/)
  for the captured narrowing and match diagnostics.

