# Golden tests — `tests/golden/`

These are the **behavioural goldens**: they state what the typing spec obliges
Basilisk to do, and they judge only that. Nothing here asserts on an internal
data structure, a helper's return value, or a diagnostic's prose.

**Unit tests do not belong in this folder.** They live in [`tests/unit/`](../unit/README.md).
Keeping the two apart is the point: a golden that fails tells you Basilisk is
wrong about Python, whereas a unit test that fails may only mean an internal
signature moved.

## The two oracles

Every case is stated against one of two oracles, both defined in
[`harness.rs`](harness.rs).

**Family B — directed.** Source that the spec says is ill-typed must draw at
least one diagnostic; source the spec says is well-typed must draw none.
`SpecObligation` pairs the two so a rule cannot pass by rejecting everything.

**Family A — invariant.** A semantics-preserving respelling of the same program
must produce an *identical multiset of diagnostic codes*. This is the oracle
that catches text-matched logic, and it is why every case carries variants:

| Class | What it changes | What it proves |
|---|---|---|
| `reformatted` | whitespace, line breaks, quote style, comments | the rule does not key on layout |
| `renamed` | every user identifier, alpha-equivalently | the rule does not key on a fixture's names |
| `aliased` | `from m import X as SomethingElse` | the rule resolves symbol *identity*, not spelling |
| `import_form` | `import m` + `m.X` attribute access | same, through a different binding form |

A rule that passes the directed leg but fails an `aliased` or `import_form`
variant is matching text. That failure is the finding — see
`[CHKARCH-TEXT-MATCHED-LOGIC]` in `CLAUDE.md` for what happens to the rule next.

## Vocabulary rule — `[PERMTEST-VOCABULARY]`

A test copied from `conformance/tests/` cannot detect a rule fitted to
`conformance/tests/`. So:

- The 913 identifiers the conformance suite defines are **banned** here.
- The 55 `typing` / `typing_extensions` symbols it imports are **quarantined**:
  reachable only under an alias, or by attribute (`typing.X`,
  `collections.abc.X`) — never bare.

Builtins are covered the same way. `int` has no import statement to alias, so
the variants manufacture one: `from builtins import int as Whole` and
`import builtins` + `builtins.int`. Both are lawful Python and both are direct
tests of whether builtin identity is resolved or merely recognised.

## Failing is a valid state

A golden that fails because Basilisk does not yet conform is **correct and
stays**. It is an accurate map of what the checker cannot do. Never delete one,
never weaken an assertion to make it pass, and never soften a variant to dodge a
respelling failure.
