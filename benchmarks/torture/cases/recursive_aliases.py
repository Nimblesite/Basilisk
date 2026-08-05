"""Recursive PEP 695 type aliases — the #371 family.

PEP 695 formally mandates that recursive type aliases work
(https://typing.python.org/en/latest/spec/aliases.html), so every guarded
definition below must draw NO diagnostic. The two unguarded definitions are
required errors: upstream conformance `aliases_type_statement.py` marks
`type R3 = R3` and `type R4[T] = T | R4[str]` as `# E` — a self-reference
that never passes through a type constructor has no terminating expansion.
"""

type Json = None | bool | int | float | str | list[Json] | dict[str, Json]
type RecursiveTuple = str | int | tuple[RecursiveTuple, ...]
type Tree[T] = T | list[Tree[T]]


def keep(j: Json, t: RecursiveTuple, tr: Tree[int]) -> None:
    pass


type R3 = R3  # E
type R4[T] = T | R4[str]  # E
