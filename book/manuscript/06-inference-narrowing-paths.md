# Chapter 6 — Inference, narrowing, and all the paths

*Part II — Think in types*

> **Reader promise:** Separate specified narrowing guarantees from
> checker-specific inference, then write and test every runtime path.

Chapter 5 left a value with type `float | None` outside a destination that
required `float`. The annotation was honest: sometimes no reading exists. The
destination was honest too: arithmetic needs a number. What can make both
statements true in one program?

Control flow supplies runtime evidence. Before a condition, several values may
be possible. Inside a branch, the condition can rule some of them out. When a
type checker uses that evidence to make a type more specific, the process is
called *type narrowing*.

There is an important limit to this chapter. The maintained
[type-narrowing specification](https://typing.python.org/en/latest/spec/narrowing.html)
says that narrowing is currently largely unspecified. It gives normative rules
for `TypeGuard` and `TypeIs`, and it explicitly describes the ordinary
`x is None` union case. It does not standardize a complete inference engine or
require identical flow analysis from every checker. Where the specification is
silent, this chapter teaches the runtime branch and leaves the exact inferred
type unstated. Basilisk's current implementation is not used to fill those
gaps.

The examples use `T | None` and `match`, both runtime syntax available from
Python 3.10. That is an example boundary, not a Basilisk-wide version target.
The union spelling is defined by [PEP 604](https://peps.python.org/pep-0604/),
and the [`match` statement](https://docs.python.org/3/reference/compound_stmts.html#the-match-statement)
is part of Python's compound-statement grammar.

## Start from declared boundaries

The typing specification permits tools to infer information, but it does not
define one general algorithm that assigns an exact inferred type to every
Python expression. Its [annotation rules](https://typing.python.org/en/latest/spec/annotations.html)
also allow a checker to infer more precise types for missing annotations
without requiring every checker to make the same choice. That makes a
catalogue of Basilisk's current inferred results the wrong foundation for this
chapter.

Use declared boundaries for the portable lesson:

```python
def read_celsius() -> float | None:
    return 21.5


def display(value: float | None) -> str:
    if value is None:
        return "no reading"
    return f"{value:.1f} °C"
```

The annotations state the contracts. `read_celsius` promises callers a value
compatible with `float | None`; `display` accepts that same union. The
specification explicitly uses `x is None` as a built-in guard that narrows a
union in both directions: the positive branch has the `None` case and the
negative branch has the other member. No broader claim about literal,
expression, or call inference is needed.

Written annotations do not validate external data, change runtime values, or
prove that a sensor is accurate. They also remain separate from a project
policy that requires annotations in selected locations. Keep important
boundaries explicit while inference behaviour is still being implemented and
standardized.

## Conditions change what remains possible

Start with the most common split. Predict the type of `value` in all three
return paths before reading the explanation:

```python
def normalize(value: float | str | None) -> float | None:
    if value is None:
        return None
    if isinstance(value, str):
        return float(value)
    return value
```

At runtime, the first branch handles absence, the second converts text, and the
last returns the numeric value. The specification clearly supports the
`is None` split. It does not comprehensively specify the exact narrowing result
for every subsequent `isinstance` path, so this book does not promise a
particular revealed type there. The function remains useful because its
declared return type is explicit and each runtime path is testable.

Early exits often make this especially clear:

```python
def add_offset(value: float | None, offset: float) -> float:
    if value is None:
        raise ValueError("reading is absent")
    return value + offset
```

The `raise` prevents the `None` path from reaching the final expression. The
normative `is None` rule leaves the other union member, `float`, on the
continuing path. The same shape works with an early `return`.

Do not generalize this one specified rule into a promise about every condition.
Equality, membership, truthiness, reassignment, loops, captured variables, and
attribute access all raise additional inference questions. Tools may support
useful behaviour for them, but this chapter deliberately leaves the exact
result out where the normative specification is not clear.

## Teach a predicate with `TypeGuard`

Built-in conditions cannot name every validation routine in your program.
Suppose an untrusted object should become a dictionary-shaped sensor event only
after its keys and values have been checked:

```python
from typing import TypeGuard, TypedDict

class ReadingEvent(TypedDict):
    sensor_id: str
    celsius: float

def is_reading_event(value: object) -> TypeGuard[ReadingEvent]:
    return (
        isinstance(value, dict)
        and isinstance(value.get("sensor_id"), str)
        and isinstance(value.get("celsius"), float)
    )
```

`TypeGuard[ReadingEvent]` tells a checker that a `True` result establishes the
first parameter as a `ReadingEvent`. It makes the successful path useful:

```python
def describe(value: object) -> str:
    if is_reading_event(value):
        return f"{value['sensor_id']}: {value['celsius']:.1f} °C"
    return "invalid event"
```

Inside the positive branch, the required keys and their value types are
available statically. In the negative branch, `TypeGuard` does not promise the
complement; `value` retains its original type. This one-sided behaviour is part
of the maintained narrowing specification. PEP 647 records the accepted design
history for [user-defined type guards](https://peps.python.org/pep-0647/).

A guard is a trusted contract, not a proof generated from its implementation.
If `is_reading_event` returns `True` after checking only the keys, code in the
positive branch may still fail at runtime. Test the predicate directly, keep
its validation complete, and avoid using a guard merely to silence an
incompatibility. At an external boundary, parsing into a domain object may be a
better design; Chapter 7 compares those data shapes.

The normative specification also defines `TypeIs`, but it has a different
contract. `TypeIs[T]` requires `T` to be assignable to the function's input
type. On a `True` result, the specified result combines the argument's previous
type with `T`; on `False`, it excludes `T`. In specification notation, those
results are intersections with `T` and with its complement. The specification
also warns that Python cannot always express those theoretical intersection
types precisely, so checkers use practical approximations.

That last qualification matters. This chapter uses `TypeGuard` because its
portable promise is smaller and easy to state: the first positional argument
becomes the guard's declared type only on the positive path. If you use
`TypeIs`, rely on its normative positive-and-negative contract, but avoid
printing a table of exact revealed types for complicated class hierarchies.
Those approximations can legitimately differ while the type system evolves.

Neither form verifies its own predicate. A misleading `TypeIs` implementation
is just as capable of creating an unsound runtime assumption as a misleading
`TypeGuard`. The annotation tells a checker how to interpret the Boolean
result; the function body and tests must justify that result.

## Write every runtime case

A union of literals, enum members, or distinct classes can describe a closed
set: the program knows all alternatives that are currently permitted. Pattern
matching makes each alternative visible:

```python
from typing import Literal, assert_never

Status = Literal["ready", "offline", "fault"]

def route_status(status: Status) -> str:
    match status:
        case "ready":
            return "accept readings"
        case "offline":
            return "wait for sensor"
        case "fault":
            return "request inspection"
        case unreachable:
            assert_never(unreachable)
```

[`Never`](https://typing.python.org/en/latest/spec/special-types.html)
represents the bottom type: no value should inhabit it. The maintained
typing guide on [unreachable code and exhaustiveness](https://typing.python.org/en/latest/guides/unreachable.html)
demonstrates this `assert_never` pattern. That page is guidance, however, not a
normative guarantee that every checker must infer every `match` remainder in
the same way. The Python `match` statement still routes the three values at
runtime, and the final assertion fails loudly if another value reaches it.

Treat any diagnostic from this pattern as tool evidence, not as the definition
of the typing rule. When the edition pins a Basilisk release, the book can show
only the behaviour verified for that release. Until then, the portable lesson
is to list the closed cases visibly and test each runtime route.

## Signal Box checkpoint

The executable snapshot for this chapter is
`book/examples/ch06-narrowing`:

```text
ch06-narrowing/
├── pyproject.toml
├── src/signal_box/
│   ├── __init__.py
│   └── routing.py
└── tests/
    └── test_routing.py
```

Open `src/signal_box/routing.py` and label the evidence before running either
tool:

1. Trace `float | str | None` through every return in `normalize`.
2. Identify the runtime checks that justify each promised key in
   `ReadingEvent`.
3. Explain why the false branch of `is_reading_event` cannot name one specific
   remaining type.
4. List the three runtime cases handled before the final assertion.
5. Predict which function and test need a new branch if `"maintenance"` joins
   `Status`.

Run runtime and static evidence separately:

```console
cd book/examples/ch06-narrowing
PYTHONPATH=src python3 -m unittest discover -s tests
basilisk check .
```

The tests execute representative paths and validate observable results. The
static check records what the installed Basilisk build currently accepts; it
does not establish a normative inference rule. Neither establishes that a real
sensor sent accurate data.

For a partially guided variation, add `"maintenance"` to `Status` and a test
that expects `"pause scheduled work"`. Run the runtime test before editing the
match. Explain the value that can now reach the final assertion, add the
missing case, and rerun the tests. You may also observe the static check, but
do not treat its result as portable unless the normative specification
requires it.

For an independent variation, find a function in your own code that accepts an
optional or union value. Draw its paths, label the type remaining after each
specified condition, and leave the static result unnamed for conditions the
normative specification does not cover. Add one runtime test per meaningful
path. If you find a cast, ask whether a real condition or a validation function
could supply the missing evidence. Do not replace the cast until the runtime
check is honest.

## What changed

- Python typing does not currently specify one complete inference and
  narrowing algorithm, so exact results outside normative cases may differ.
- Declared parameter and return boundaries provide the stable foundation for
  the examples in this chapter.
- The specification explicitly describes bidirectional narrowing for an
  `x is None` check over a union containing `None`.
- A `TypeGuard` gives a name to validated evidence on its positive path, while
  trusting the predicate implementation to be correct.
- `assert_never` is a useful exhaustiveness pattern, but exact `match`
  remainder inference is not presented here as a portable normative guarantee.
- Runtime validation and tests remain necessary even when a static tool accepts
  every path.

Chapter 7 will move from paths to shapes. You will choose among `TypedDict`, a
dataclass, an enum, a protocol, and a generic abstraction according to the
boundary each one describes.

## Authoritative sources

- [Type narrowing](https://typing.python.org/en/latest/spec/narrowing.html)
- [Type annotations](https://typing.python.org/en/latest/spec/annotations.html)
- [Special types: `Never`](https://typing.python.org/en/latest/spec/special-types.html)
- [Unreachable code and exhaustiveness](https://typing.python.org/en/latest/guides/unreachable.html)
- [PEP 647 — User-defined type guards](https://peps.python.org/pep-0647/)
- [PEP 604 — Allow writing union types as `X | Y`](https://peps.python.org/pep-0604/)
- [Python `match` statement](https://docs.python.org/3/reference/compound_stmts.html#the-match-statement)
- Inspect the current diagnostic families in the
  [Basilisk rule reference](https://www.basilisk-python.dev/docs/rules/).
