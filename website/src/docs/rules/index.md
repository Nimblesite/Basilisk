---
layout: layouts/docs.njk
title: "Basilisk Diagnostic Rules — All BSK Error and Warning Codes"
description: "Complete reference for every Basilisk diagnostic code (BSK-E errors and BSK-W warnings) — missing annotations, type safety, generics, protocols, dataclasses, TypedDicts, and more."
keywords: basilisk rules, type errors, BSK-E, BSK-W, diagnostic codes
date: 2026-02-28
dateModified: 2026-05-30
author: The Basilisk Project
eleventyNavigation:
  key: Rules
  order: 5
---

# Diagnostic Rules

Every Basilisk diagnostic has a unique code in the format `BSK-EXXXX` (error) or `BSK-WXXXX` (warning).

Rules are enabled by default. You can dial individual rules down per-file or per-path from your editor or `pyproject.toml` — strict is the default, not a cage.

Basilisk ships **151 diagnostic codes** (146 errors, 5 warnings) spanning the full Python typing surface — generics, protocols, dataclasses, TypedDicts, overloads, literals, enums, and more — and is validated against the [official Python typing conformance suite](https://github.com/python/typing/blob/main/conformance/results/results.html) (currently **92.5%**, 135 / 146). The two foundational groups have worked examples:

| Group | Codes | Description |
|---|---|---|
| [Missing Annotations](/docs/rules/missing-annotations/) | E0001–E0009 | Unannotated parameters, return types, variables, and attributes |
| [Type Safety](/docs/rules/type-safety/) | E0010–E0029 | Type mismatches, incorrect annotations, unsound type usage |

> **Roadmap:** Mojo-inspired ownership and immutability analysis is planned for a future release. It is not yet part of the shipping rule set.

## Complete diagnostic reference

Every code currently emitted by the checker. This table is generated from the
checker source (`scripts/gen_rules_reference.py`) — it is the authoritative list.

| Code | Description |
|---|---|
| `BSK-E0001` | Missing parameter type annotation |
| `BSK-E0002` | Missing return type annotation |
| `BSK-E0003` | Missing variable type annotation |
| `BSK-E0004` | Missing `*args` / `**kwargs` type annotation |
| `BSK-E0005` | Missing class attribute type annotation |
| `BSK-E0010` | Unresolved import |
| `BSK-E0011` | Explicit `Any` annotation / return type mismatch |
| `BSK-E0012` | Argument type mismatch at a call site |
| `BSK-E0013` | Return type mismatch — inferred return type incompatible with annotation |
| `BSK-E0014` | Assignment type incompatibility (literal mismatches) |
| `BSK-E0015` | Invalid type argument count or form |
| `BSK-E0016` | Incompatible method override |
| `BSK-E0017` | Incompatible class attribute override |
| `BSK-E0018` | Undefined variable used in a return statement |
| `BSK-E0019` | Unbound variable on some code paths |
| `BSK-E0020` | Missing `@overload` implementation |
| `BSK-E0021` | Overlapping `@overload` signatures |
| `BSK-E0022` | Unhashable type used as a dict key |
| `BSK-E0023` | Non-exhaustive `match` statement |
| `BSK-E0024` | Invalid type form — numeric literal used as type annotation |
| `BSK-E0025` | Missing `@override` decorator |
| `BSK-E0026` | `TypeVar` declared with exactly one constraint |
| `BSK-E0027` | Duplicate `TypeVar` in a `Generic[...]` base |
| `BSK-E0029` | Method defined inside a `TypedDict` class |
| `BSK-E0030` | Non-default `TypeVar` follows a default `TypeVar` in `Generic[...]` |
| `BSK-E0031` | Invalid `cast()` call |
| `BSK-E0032` | Invalid keyword argument in `TypedDict` class definition |
| `BSK-E0033` | Invalid `reveal_type()` call |
| `BSK-E0034` | `@final` decorator violations |
| `BSK-E0035` | `Required` / `NotRequired` used in an invalid context |
| `BSK-E0036` | `ClassVar` used in an invalid context |
| `BSK-E0037` | Invalid `TypedDict(...)` functional-syntax call |
| `BSK-E0038` | Invalid `TypedDict` inheritance |
| `BSK-E0039` | Invalid `assert_type()` call |
| `BSK-E0040` | Invalid Enum subclassing |
| `BSK-E0041` | Too few arguments in a function call |
| `BSK-E0042` | PEP 695 type parameter syntax mixed with traditional `TypeVars` |
| `BSK-E0043` | Non-TypeVar argument in `Generic[...]` or `Protocol[...]` |
| `BSK-E0044` | `Final` used in an invalid position |
| `BSK-E0045` | Invalid first argument to `Annotated[...]` |
| `BSK-E0046` | Enum member annotated with an explicit type |
| `BSK-E0047` | Invalid type expression in annotation |
| `BSK-E0048` | Invalid right-hand side for a `TypeAlias` annotation |
| `BSK-E0049` | Multiple unbounded tuple components in a single tuple type |
| `BSK-E0050` | Invalid `NewType(...)` call |
| `BSK-E0051` | Invalid `Literal` parameterization |
| `BSK-E0052` | Assignment to attribute of a frozen dataclass instance, or invalid frozen/non-frozen dataclass inheritance |
| `BSK-E0053` | `assert_type()` type mismatch |
| `BSK-E0054` | `Final` type qualifier annotation violations |
| `BSK-E0055` | Invalid `TypeVar` / `TypeVarTuple` / `ParamSpec` keyword argument combination |
| `BSK-E0056` | Mutation of `ReadOnly` `TypedDict` fields |
| `BSK-E0057` | Invalid RHS in a PEP 695 `type X = rhs` statement |
| `BSK-E0058` | `Annotated[...]` requires at least two arguments |
| `BSK-E0059` | Access to `__match_args__` on a dataclass with `match_args=False` |
| `BSK-E0060` | Invalid ordering comparison of dataclass instances |
| `BSK-E0061` | `assert_type` with `Literal[Enum.MEMBER]` on enum-typed param |
| `BSK-E0062` | `-> NoReturn` / `-> Never` function can fall through |
| `BSK-E0063` | Non-hashable dataclass assigned to a `Hashable`-annotated variable |
| `BSK-E0064` | Invalid argument in a `NamedTuple` constructor call |
| `BSK-E0065` | Access to an `int`-only attribute on a `float`-typed parameter |
| `BSK-E0066` | Enum member value incompatible with `_value_` type annotation |
| `BSK-E0067` | Non-member referenced in `Literal[EnumClass.X]` annotation |
| `BSK-E0068` | `Literal["EnumClass.MEMBER"]` (string) used where `Literal[EnumClass.MEMBER]` (enum member reference) is required |
| `BSK-E0069` | Dataclass constructor argument violations |
| `BSK-E0070` | `Never` type compatibility violations |
| `BSK-E0071` | Historical positional-only parameter violations |
| `BSK-E0072` | No matching overload for subscript indexing |
| `BSK-E0073` | `NamedTuple`-to-tuple type incompatibility |
| `BSK-E0074` | Constructor call type mismatch with specialized generic class |
| `BSK-E0075` | Incompatible type for `Self`-typed attribute |
| `BSK-E0076` | Overload union expansion failure |
| `BSK-E0077` | Protocol `Self`-return conformance violation |
| `BSK-E0078` | `Self` type violations in generics |
| `BSK-E0079` | Module assigned to incompatible protocol type |
| `BSK-E0080` | `TypeVar` upper bound violation at call site |
| `BSK-E0081` | `TypeVarTuple` unpack minimum type argument violation |
| `BSK-E0082` | `TypeVarTuple` callable/tuple argument mismatch |
| `BSK-E0083` | `TypeVarTuple` must be unpacked with `*` operator |
| `BSK-E0084` | `TypeVarTuple` variance/bounds/constraints violation |
| `BSK-E0085` | `TypeVarTuple` argument count mismatch |
| `BSK-E0086` | Multiple `TypeVarTuple` unpacks in generic or tuple type |
| `BSK-E0087` | Reserved for future PEP 695 type parameter checks |
| `BSK-E0088` | `TypedDict` runtime violation |
| `BSK-E0089` | Invalid PEP 695 type parameter bound or constraint |
| `BSK-E0090` | Invalid tuple type syntax |
| `BSK-E0091` | Incompatible `TypeVar` bound or constraint with its default |
| `BSK-E0092` | Wrong number of type arguments to a generic class or type alias |
| `BSK-E0093` | Invalid key or value type in `TypedDict` assignment |
| `BSK-E0094` | `Self` type used in an invalid location |
| `BSK-E0095` | `InitVar` field validation in dataclasses |
| `BSK-E0096` | Type mismatch between a dataclass `field(default_factory=…)` and the field's declared type annotation |
| `BSK-E0097` | Protocol `__new__`/`__init__` sets self-attributes not declared in Protocol |
| `BSK-E0098` | Non-Protocol base class in a Protocol definition |
| `BSK-E0099` | Direct instantiation of a Protocol class |
| `BSK-E0100` | Augmented assignment widens `Literal` type |
| `BSK-E0101` | `TypeGuard` or `TypeIs` on method with no narrowing parameter |
| `BSK-E0102` | Invalid `TypeVar` default referencing another `TypeVar` |
| `BSK-E0103` | Tuple index out of bounds |
| `BSK-E0104` | Cyclical type alias reference |
| `BSK-E0105` | Invalid attribute access on bounded type variable |
| `BSK-E0106` | Protocol class used where `type[Proto]` is expected |
| `BSK-E0107` | Variance incompatibility in base class parameterisation |
| `BSK-E0108` | Dataclass slots violations |
| `BSK-E0109` | `TypeVar` bound violation at call site |
| `BSK-E0110` | Protocol variance violation |
| `BSK-E0111` | Constructor call errors via `__init__` method |
| `BSK-E0112` | TypeGuard/TypeIs return type incompatibility in callable arguments |
| `BSK-E0113` | `TypeIs` narrows to a type inconsistent with the input type |
| `BSK-E0114` | Protocol `isinstance`/`issubclass` violations |
| `BSK-E0115` | Use of deprecated class, function, or method |
| `BSK-E0116` | `NamedTuple` class definition errors |
| `BSK-E0117` | Unbound type variable in scope |
| `BSK-E0118` | Calling `super().method()` on an abstract method with no default implementation |
| `BSK-E0119` | Protocol `isinstance`/`issubclass` violations |
| `BSK-E0120` | Generator return type and yield type violations |
| `BSK-E0121` | Protocol conformance violation in annotated assignment |
| `BSK-E0122` | Callable call-site arity and argument validation |
| `BSK-E0123` | `super()` call on abstract protocol method with no default implementation |
| `BSK-E0124` | Protocol attribute tuple element type mismatch |
| `BSK-E0125` | Access to instance attribute on a class object |
| `BSK-E0126` | `LiteralString` and `Literal` assignment incompatibilities |
| `BSK-E0127` | Tuple index out of range |
| `BSK-E0128` | ```TypeVar``` default referential violations |
| `BSK-E0129` | Literal value assignment incompatibility |
| `BSK-E0130` | `TypeVar` scoping violation |
| `BSK-E0131` | Generator yield/send/return type mismatch |
| `BSK-E0132` | Inconsistent `TypeVar` ordering across base classes |
| `BSK-E0133` | Protocol `TypeVar` variance mismatch |
| `BSK-E0134` | Invariant generic type mismatch at call site |
| `BSK-E0136` | Callable subtyping violations (covariance / contravariance) |
| `BSK-E0137` | Generic protocol violations |
| `BSK-E0138` | `dataclass_transform` metaclass violations |
| `BSK-E0139` | Invalid `TypeVarTuple` specialization of generic alias |
| `BSK-E0140` | Callable and Protocol assignment compatibility |
| `BSK-E0141` | Unpack[`TypedDict`] kwargs violations |
| `BSK-E0142` | `dataclass_transform` violations when the transform is applied via a base class |
| `BSK-E0143` | `NamedTuple` usage violations |
| `BSK-E0144` | Invalid constructor call via `type[T]` parameter |
| `BSK-E0145` | Invalid `type[X]` usage violations |
| `BSK-E0146` | Protocol class object violations |
| `BSK-E0147` | Tuple starred-unpack type compatibility violation |
| `BSK-E0148` | Generic type argument violations |
| `BSK-E0149` | PEP 695 generic type parameter scoping violations |
| `BSK-E0150` | Variable defined only in dead version/platform branch |
| `BSK-E0151` | Invalid `TypeAliasType(...)` call |
| `BSK-E0152` | Missing type stubs for installed package |
| `BSK-E0153` | Invalid call to a constructor-derived callable |
| `BSK-W0011` | Undeclared dependency import |
| `BSK-W0012` | Unused dependency |
| `BSK-W0013` | Stale uv lock file |
| `BSK-W0040` | Lambda function missing type annotations |
| `BSK-W0050` | Redundant type annotation warning |
