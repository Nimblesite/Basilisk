# basilisk-mojo

Mojo-inspired ownership and immutability analysis for Basilisk.

## Role in Basilisk

This crate is **scaffolding** for static ownership semantics expressed as type
annotations over standard Python syntax. The design goal is to let developers
declare ownership contracts — catching mutation of borrowed values and
use-after-move — using only `typing` constructs, with no compiler or runtime
support required.

Per [CHKARCH-MOJO-SAFETY](../../docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-MOJO-SAFETY),
this analysis is "planned, opt-in, and not wired into the checker pipeline".
Nothing here enforces anything today: the crate's only public entry point,
`check_ownership`, has no caller outside `tests/mojo_tests.rs`.

## Key concepts (design vocabulary)

- **`Borrowed`** — a read-only reference; mutation is intended to be an error.
- **`InOut`** — a mutable reference, explicitly declared.
- **`Owned`** — ownership is transferred; use after transfer is intended to be an error.
- **Standard Python syntax** — the target form is `Annotated[T, Borrowed|InOut|Owned]`
  ([CHKARCH-MOJO-OWNERSHIP](../../docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-MOJO-OWNERSHIP)),
  so no compiler or runtime is required.

Of these, only `Borrowed` is recognised by the current prototype, and only in
the bare subscript/name form shown below — not yet in the `Annotated[...]` form
that the spec targets.

## What the prototype actually detects

`check_ownership(source: &str) -> Vec<String>` is a line-oriented text scan, not
an AST pass (`src/lib.rs`):

1. For each line starting with `def `, it splits the parenthesised parameter list
   on `,` and collects the names of parameters whose annotation either starts
   with `Borrowed[` or is exactly `Borrowed`.
2. It then flags any line containing `<param>.<method>(` for `<method>` in
   `MUTATING_METHODS` (`append`, `extend`, `insert`, `remove`, `pop`, `clear`,
   `sort`, `reverse`, `update`, `add`, `discard`).

Because step 1 splits on `,`, an annotation written as
`Annotated[list[int], Borrowed]` is broken into two fragments and matches
neither accepted shape, so it is **not** detected.

## Example

```python
def summarise(items: Borrowed[list[int]]) -> int:
    items.append(99)  # detected: mutation of a Borrowed parameter
    return sum(items)
```

`check_ownership` returns strings, not structured diagnostics. For the source
above it yields:

```
directives_cast: mutation of Borrowed parameter `items` via `.append()` is not allowed
```

**Known defect:** that `directives_cast:` prefix is not a diagnostic code for
this crate. `directives_cast` is a real, registered, shipping PEP rule meaning
"Invalid `cast()` call" (`crates/basilisk-checker/src/rules/directives_cast.rs`,
documented at <https://www.basilisk-python.dev/errors/directives_cast>).
Reusing it here contradicts [CHKARCH-MOJO-SAFETY], which states that shipping
PEP rules and this scaffolding "must not reuse these anchors or diagnostic
descriptions". The string is hard-coded in `src/lib.rs` and needs its own code
before any of this is wired up.

## Status

Planned. The crate is scaffolding targeted at Phase 4 (`src/lib.rs`); it is not
registered with the checker and produces no user-visible diagnostics. Remaining
work is tracked as unchecked TODOs under
[CHKADVPLAN-TODO-MOJO](../../docs/plans/CHECKER-ADVANCED-FEATURES-PLAN.md#CHKADVPLAN-TODO-MOJO).
