# Unit tests — `tests/unit/`

Tests of internal machinery: helper functions, data structures, caching,
incremental invalidation, config parsing, and the like.

They are **sequestered here deliberately**, away from
[`tests/golden/`](../golden/README.md). A unit test asserts that an
implementation detail behaves as its author intended; a golden test asserts that
Basilisk is right about Python. Only the second is evidence of conformance, and
mixing them makes a suite look like it proves more than it does.

## What belongs here

- Tests that construct an internal type and assert on its fields or `Display`.
- Tests that call a helper directly rather than through `check`.
- Tests of caching, Salsa revisions, and incremental recomputation.
- Tests of config loading, rule tagging, and suppression bookkeeping.

## What does not

Anything of the form "this Python source should/should not produce a
diagnostic". That is a golden, and it goes in `tests/golden/` with its
respelling variants attached.

## Line coverage is not assertion

A unit test that exercises a rule without asserting on the behaviour that rule
changes is worse than no test: it reports coverage it has not earned. Judge a
test by what it would catch.
