# Every diagnostic in the first section is a genuine PEP typing-spec
# violation. Basilisk reports all of them out of the box — no configuration,
# every one an error.
#
# Run: basilisk check examples/bad.py
#
# The final section violates only Basilisk's opt-in strictness rules. Those
# stay silent until a project enables them — per rule, at any severity — in
# `[tool.basilisk.rules]` or via "Basilisk: Open Configuration Editor" in
# VS Code. This repository enables them for `examples/**` as warnings in the
# root `pyproject.toml`: the incremental-adoption setup, where warnings mean
# "this type-checks, but strictness isn't at full yet".

from typing import override


def greet(name: str) -> str:
    return "Hello, " + name


greet(42)  # error[calls_argument_type]: `name` expects `str`, got an `int`


def get_score() -> int:  # error[returns_compatibility]: declared `int`, returns `str`
    return "high"  # error[returns_compatibility_2]: `str` is not assignable to `int`


count: int = "zero"  # error[assignment_compatibility]: annotated `int`, assigned `str`


def add(x: int, y: int) -> int:
    return x + y


add(1)  # error[calls_argument_count]: missing required argument `y`


class Shape:
    def area(self, scale: float) -> float:
        return scale


class Circle(Shape):
    @override
    def area(
        self, scale: str
    ) -> float:  # error[classes_override]: incompatible with `Shape.area`
        return 1.0


def describe(flag: bool) -> str:
    if flag:
        label = "on"
    return label  # error[names_unbound]: `label` is unbound when `flag` is false


def classify(value: int | str) -> str:
    match value:  # error[match_exhaustiveness]: no `case _:` branch
        case int():
            return "number"


# ── Opt-in strictness rules — silent until enabled ──────────────────────────


def process(data):  # BSK-E0001: `data` untyped; BSK-E0002: no return type
    return data.upper()


def log_all(*args, **kwargs):  # BSK-E0004: `*args` / `**kwargs` untyped
    pass
