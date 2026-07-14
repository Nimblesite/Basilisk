# Chapter 4 — The everyday type vocabulary

*Part II — Think in types*

> **Reader promise:** Read and write the types used at ordinary application
> boundaries without confusing an annotation with runtime conversion.

## Values, names, and annotations

Use one Signal Box reading to separate the runtime value, the name bound to it,
and the static information written or inferred for that name. Demonstrate that
adding an annotation does not coerce a string into a float.

## Built-in values and collections

Cover the scalar and collection forms the running project actually needs. Show
why the element types of a collection carry more useful information than a bare
container name.

## Unions and absence

Introduce `T | None`, then a small closed union of reading variants. Keep the
focus on permitted possibilities; narrowing comes in Chapter 6.

## `object` is not `Any`

Compare the operations a checker can justify after a value becomes `object`
with the information discarded by `Any`. Avoid describing either as merely the
“top type.”

## Aliases name a domain idea

Create one Python 3.12 `type` alias because it makes a boundary easier to read,
not because every long annotation deserves a second name.

## Signal Box checkpoint

Annotate raw and normalized readings, including absent values and a small union,
then confirm which facts are explicit and which Basilisk still infers.

## Authoritative sources

- [Type system concepts](https://typing.python.org/en/latest/spec/concepts.html)
- [Type annotations](https://typing.python.org/en/latest/spec/annotations.html)
- [Type aliases](https://typing.python.org/en/latest/spec/aliases.html)
- [CPython 3.12 typing implementation](https://github.com/python/cpython/blob/3.12/Lib/typing.py)
- Return to the [Basilisk rule reference](https://www.basilisk-python.dev/docs/rules/)
  for the diagnostics used in the examples.

