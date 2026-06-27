---
layout: layouts/docs.njk
title: 诊断规则
description: 所有 Basilisk 诊断代码（BSK-E 错误和 BSK-W 警告）的完整参考。缺失注解、类型安全等。
keywords: basilisk规则, 类型错误, BSK-E, BSK-W, 诊断代码
lang: zh
---

# 诊断规则

每个 Basilisk 诊断都有一个 `BSK-EXXXX`（错误）或 `BSK-WXXXX`（警告）格式的唯一代码。

规则默认全部启用。您可以通过编辑器或 `pyproject.toml`，按文件或路径将单个规则调低——严格是默认值，而不是牢笼。

Basilisk 内置 **155 个诊断代码**（150 个错误，5 个警告），覆盖完整的 Python 类型表面（泛型、协议、dataclass、TypedDict、重载、字面量、枚举等），由[官方 Python 类型符合性套件](https://github.com/python/typing/blob/main/conformance/results/results.html)评分（当前符合率 **{{ conformance.scorePct }}%**，{{ conformance.pass }} / {{ conformance.total }}（错误加警告，最严格）；目标 100% —— [我们如何衡量](/zh/docs/conformance/)）。下面记录了两个基础组；完整集合由检查器强制执行。

| 组 | 代码 | 描述 |
|---|---|---|
| [缺失注解](/zh/docs/rules/missing-annotations/) | E0001–E0009 | 未标注的参数、返回类型、变量和属性 |
| [类型安全](/zh/docs/rules/type-safety/) | E0010–E0029 | 类型不匹配、错误的注解、不健全的类型使用 |

> **路线图：** Mojo 启发的所有权与不可变性分析计划在未来版本中推出。它尚未包含在当前发布的规则集中。

## 完整诊断参考

以下表格列出了检查器当前会产生的所有代码，由 `scripts/gen_rules_reference.py` 从检查器源代码生成，是权威参考列表。

| Code | Description |
|---|---|
| `BSK-E0001` | Missing parameter type annotation |
| `BSK-E0002` | Missing return type annotation |
| `BSK-E0003` | Missing variable type annotation |
| `BSK-E0004` | Missing `*args` / `**kwargs` type annotation |
| `BSK-E0005` | Missing class attribute type annotation |
| `imports_unresolved` | Unresolved import |
| `returns_compatibility` | Explicit `Any` annotation / return type mismatch |
| `calls_argument_type` | Argument type mismatch at a call site |
| `returns_compatibility_2` | Return type mismatch — inferred return type incompatible with annotation |
| `assignment_compatibility` | Assignment type incompatibility (literal mismatches) |
| `callables_annotation` | Invalid type argument count or form |
| `classes_override` | Incompatible method override |
| `classes_override_2` | Incompatible class attribute override |
| `names_undefined` | Undefined variable used in a return statement |
| `names_unbound` | Unbound variable on some code paths |
| `overloads_definitions` | Missing `@overload` implementation |
| `overloads_consistency` | Overlapping `@overload` signatures |
| `dict_key_hashable` | Unhashable type used as a dict key |
| `match_exhaustiveness` | Non-exhaustive `match` statement |
| `annotations_typeexpr` | Invalid type form — numeric literal used as type annotation |
| `BSK-E0025` | Missing `@override` decorator |
| `generics_basic` | `TypeVar` declared with exactly one constraint |
| `generics_base_class` | Duplicate `TypeVar` in a `Generic[...]` base |
| `typeddicts_class_syntax` | Method defined inside a `TypedDict` class |
| `generics_defaults` | Non-default `TypeVar` follows a default `TypeVar` in `Generic[...]` |
| `directives_cast` | Invalid `cast()` call |
| `typeddicts_class_syntax_2` | Invalid keyword argument in `TypedDict` class definition |
| `directives_reveal_type` | Invalid `reveal_type()` call |
| `qualifiers_final_decorator` | `@final` decorator violations |
| `typeddicts_required` | `Required` / `NotRequired` used in an invalid context |
| `BSK-E0036` | `ClassVar` used in an invalid context |
| `typeddicts_alt_syntax` | Invalid `TypedDict(...)` functional-syntax call |
| `typeddicts_inheritance` | Invalid `TypedDict` inheritance |
| `directives_assert_type` | Invalid `assert_type()` call |
| `enums_behaviors` | Invalid Enum subclassing |
| `calls_argument_count` | Too few arguments in a function call |
| `generics_syntax_compatibility` | PEP 695 type parameter syntax mixed with traditional `TypeVars` |
| `generics_basic_2` | Non-TypeVar argument in `Generic[...]` or `Protocol[...]` |
| `qualifiers_final_annotation` | `Final` used in an invalid position |
| `qualifiers_annotated` | Invalid first argument to `Annotated[...]` |
| `enums_members` | Enum member annotated with an explicit type |
| `annotations_forward_refs` | Invalid type expression in annotation |
| `aliases_implicit` | Invalid right-hand side for a `TypeAlias` annotation |
| `tuples_type_form` | Multiple unbounded tuple components in a single tuple type |
| `aliases_newtype` | Invalid `NewType(...)` call |
| `literals_parameterizations` | Invalid `Literal` parameterization |
| `dataclasses_frozen` | Assignment to attribute of a frozen dataclass instance, or invalid frozen/non-frozen dataclass inheritance |
| `directives_assert_type_2` | `assert_type()` type mismatch |
| `qualifiers_final_annotation_2` | `Final` type qualifier annotation violations |
| `generics_typevartuple_basic` | Invalid `TypeVar` / `TypeVarTuple` / `ParamSpec` keyword argument combination |
| `typeddicts_readonly` | Mutation of `ReadOnly` `TypedDict` fields |
| `aliases_type_statement` | Invalid RHS in a PEP 695 `type X = rhs` statement |
| `qualifiers_annotated_2` | `Annotated[...]` requires at least two arguments |
| `dataclasses_match_args` | Access to `__match_args__` on a dataclass with `match_args=False` |
| `dataclasses_order` | Invalid ordering comparison of dataclass instances |
| `enums_expansion` | `assert_type` with `Literal[Enum.MEMBER]` on enum-typed param |
| `specialtypes_never` | `-> NoReturn` / `-> Never` function can fall through |
| `dataclasses_hash` | Non-hashable dataclass assigned to a `Hashable`-annotated variable |
| `namedtuples_define_functional` | Invalid argument in a `NamedTuple` constructor call |
| `specialtypes_promotions` | Access to an `int`-only attribute on a `float`-typed parameter |
| `enums_member_values` | Enum member value incompatible with `_value_` type annotation |
| `enums_members_2` | Non-member referenced in `Literal[EnumClass.X]` annotation |
| `literals_parameterizations_2` | `Literal["EnumClass.MEMBER"]` (string) used where `Literal[EnumClass.MEMBER]` (enum member reference) is required |
| `dataclasses_kwonly` | Dataclass constructor argument violations |
| `specialtypes_never_2` | `Never` type compatibility violations |
| `historical_positional` | Historical positional-only parameter violations |
| `overloads_basic` | No matching overload for subscript indexing |
| `namedtuples_type_compat` | `NamedTuple`-to-tuple type incompatibility |
| `constructors_call_new` | Constructor call type mismatch with specialized generic class |
| `generics_self_attributes` | Incompatible type for `Self`-typed attribute |
| `overloads_evaluation` | Overload union expansion failure |
| `generics_self_protocols` | Protocol `Self`-return conformance violation |
| `generics_self_basic` | `Self` type violations in generics |
| `protocols_modules` | Module assigned to incompatible protocol type |
| `generics_upper_bound` | `TypeVar` upper bound violation at call site |
| `generics_typevartuple_unpack` | `TypeVarTuple` unpack minimum type argument violation |
| `generics_typevartuple_callable` | `TypeVarTuple` callable/tuple argument mismatch |
| `generics_typevartuple_basic_2` | `TypeVarTuple` must be unpacked with `*` operator |
| `generics_typevartuple_basic_3` | `TypeVarTuple` variance/bounds/constraints violation |
| `generics_typevartuple_args` | `TypeVarTuple` argument count mismatch |
| `generics_typevartuple_specialization` | Multiple `TypeVarTuple` unpacks in generic or tuple type |
| `BSK-E0087` | Reserved for future PEP 695 type parameter checks |
| `typeddicts_usage` | `TypedDict` runtime violation |
| `generics_syntax_declarations` | Invalid PEP 695 type parameter bound or constraint |
| `tuples_type_form_2` | Invalid tuple type syntax |
| `generics_defaults_2` | Incompatible `TypeVar` bound or constraint with its default |
| `generics_defaults_specialization` | Wrong number of type arguments to a generic class or type alias |
| `typeddicts_operations` | Invalid key or value type in `TypedDict` assignment |
| `generics_self_usage` | `Self` type used in an invalid location |
| `dataclasses_postinit` | `InitVar` field validation in dataclasses |
| `dataclasses_usage` | Type mismatch between a dataclass `field(default_factory=…)` and the field's declared type annotation |
| `protocols_definition` | Protocol `__new__`/`__init__` sets self-attributes not declared in Protocol |
| `protocols_merging` | Non-Protocol base class in a Protocol definition |
| `protocols_explicit` | Direct instantiation of a Protocol class |
| `literals_semantics` | Augmented assignment widens `Literal` type |
| `narrowing_typeguard` | `TypeGuard` or `TypeIs` on method with no narrowing parameter |
| `generics_defaults_referential` | Invalid `TypeVar` default referencing another `TypeVar` |
| `tuples_index` | Tuple index out of bounds |
| `aliases_recursive` | Cyclical type alias reference |
| `generics_syntax_declarations_2` | Invalid attribute access on bounded type variable |
| `protocols_class_objects` | Protocol class used where `type[Proto]` is expected |
| `generics_variance` | Variance incompatibility in base class parameterisation |
| `dataclasses_slots` | Dataclass slots violations |
| `generics_upper_bound_2` | `TypeVar` bound violation at call site |
| `protocols_variance` | Protocol variance violation |
| `BSK-E0111` | Constructor call errors via `__init__` method |
| `narrowing_typeis` | TypeGuard/TypeIs return type incompatibility in callable arguments |
| `narrowing_typeis_2` | `TypeIs` narrows to a type inconsistent with the input type |
| `protocols_runtime_checkable` | Protocol `isinstance`/`issubclass` violations |
| `directives_deprecated` | Use of deprecated class, function, or method |
| `namedtuples_define_class` | `NamedTuple` class definition errors |
| `generics_scoping` | Unbound type variable in scope |
| `protocols_explicit_2` | Calling `super().method()` on an abstract method with no default implementation |
| `protocols_runtime_checkable_2` | Protocol `isinstance`/`issubclass` violations |
| `BSK-E0120` | Generator return type and yield type violations |
| `protocols_definition_2` | Protocol conformance violation in annotated assignment |
| `callables_protocol` | Callable call-site arity and argument validation |
| `protocols_explicit_3` | `super()` call on abstract protocol method with no default implementation |
| `protocols_subtyping` | Protocol attribute tuple element type mismatch |
| `generics_type_erasure` | Access to instance attribute on a class object |
| `BSK-E0126` | `LiteralString` and `Literal` assignment incompatibilities |
| `tuples_index_2` | Tuple index out of range |
| `generics_defaults_referential_2` | ```TypeVar``` default referential violations |
| `literals_semantics_2` | Literal value assignment incompatibility |
| `generics_variance_inference` | `TypeVar` scoping violation |
| `annotations_generators_2` | Generator yield/send/return type mismatch |
| `generics_base_class_2` | Inconsistent `TypeVar` ordering across base classes |
| `protocols_variance_2` | Protocol `TypeVar` variance mismatch |
| `generics_base_class_3` | Invariant generic type mismatch at call site |
| `callables_subtyping` | Callable subtyping violations (covariance / contravariance) |
| `protocols_generic` | Generic protocol violations |
| `BSK-E0138` | `dataclass_transform` metaclass violations |
| `generics_typevartuple_specialization_2` | Invalid `TypeVarTuple` specialization of generic alias |
| `callables_protocol_2` | Callable and Protocol assignment compatibility |
| `callables_kwargs` | Unpack[`TypedDict`] kwargs violations |
| `BSK-E0142` | `dataclass_transform` violations when the transform is applied via a base class |
| `namedtuples_usage` | `NamedTuple` usage violations |
| `BSK-E0144` | Invalid constructor call via `type[T]` parameter |
| `specialtypes_type` | Invalid `type[X]` usage violations |
| `protocols_class_objects_2` | Protocol class object violations |
| `tuples_type_compat` | Tuple starred-unpack type compatibility violation |
| `BSK-E0148` | Generic type argument violations |
| `generics_syntax_scoping` | PEP 695 generic type parameter scoping violations |
| `directives_version_platform` | Variable defined only in dead version/platform branch |
| `aliases_typealiastype` | Invalid `TypeAliasType(...)` call |
| `BSK-E0152` | Missing type stubs for installed package |
| `constructors_callable` | Invalid call to a constructor-derived callable |
| `imports_module_attribute` | Access to a module attribute the local stub does not declare |
| `version_target_syntax` | PEP 695 syntax used below the configured target Python version |
| `BSK-W0011` | Undeclared dependency import |
| `BSK-W0012` | Unused dependency |
| `BSK-W0013` | Stale uv lock file |
| `BSK-W0040` | Lambda function missing type annotations |
| `BSK-W0050` | Redundant type annotation warning |
