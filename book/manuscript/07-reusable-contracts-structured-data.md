# Chapter 7 — Reusable contracts and structured data

*Part II — Think in types*

> **Reader promise:** Choose a data or behavior contract because it matches a
> boundary, not because it is the most advanced typing feature available.

## Dictionary-shaped external data

Introduce `TypedDict` at the JSON-like input boundary, where string keys are
part of the external shape. Show required and optional information only as the
fixture needs it.

## Attribute-shaped internal data

Transform validated input into a dataclass. Compare that choice with a named
tuple and an ordinary class using the same fields and operations.

## Closed choices

Use an enum or literal where the application owns a finite set of alert states.
Connect the choice directly to Chapter 6's exhaustiveness lesson.

## Behavior without inheritance

Define a small storage protocol from the operations Signal Box consumes. Show
two implementations satisfying it structurally without adding a shared base
class solely for the type checker.

## Preserve a payload with a type parameter

Create one generic page or result container using syntax supported by the
reader's project. If the example uses PEP 695 type-parameter syntax, label its
Python-version boundary. Demonstrate substitution with concrete payloads before
using the word “generic.”

## Overload only when calls truly differ

Use a small public function whose input form determines its output type. Keep a
single implementation and explain why unions are simpler when callers do not
receive a more precise result.

## Signal Box checkpoint

Separate raw input, the normalized domain model, the storage protocol, and the
generic report page. Run the same tests through two storage implementations.

## Authoritative sources

- [Typed dictionaries](https://typing.python.org/en/latest/spec/typeddict.html)
- [Dataclasses in the typing specification](https://typing.python.org/en/latest/spec/dataclasses.html)
- [Protocols](https://typing.python.org/en/latest/spec/protocol.html)
- [Generics](https://typing.python.org/en/latest/spec/generics.html)
- [Overloads](https://typing.python.org/en/latest/spec/overload.html)
- Browse related diagnostics in the
  [Basilisk rule reference](https://www.basilisk-python.dev/docs/rules/).
