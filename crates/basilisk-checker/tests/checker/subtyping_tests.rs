use basilisk_checker::subtyping::{is_subtype_with_context, SubtypeContext};
use basilisk_checker::types::{CallableInfo, InferredType};
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn build_ctx(source: &str) -> basilisk_resolver::ResolvedModule {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned()).expect("parse failed");
    resolve(&parsed).expect("resolve failed")
}

// ── SubtypeContext construction and nominal subtyping ──

#[test]
fn subtype_context_basic_identity() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("int", "int"));
    assert!(ctx.is_subtype("str", "str"));
    assert!(ctx.is_subtype("float", "float"));
}

#[test]
fn subtype_context_object_universal_supertype() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("int", "object"));
    assert!(ctx.is_subtype("str", "object"));
    assert!(ctx.is_subtype("list", "object"));
    assert!(ctx.is_subtype("SomeUnknown", "object"));
}

#[test]
fn subtype_context_never_bottom_type() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("Never", "int"));
    assert!(ctx.is_subtype("Never", "str"));
    assert!(ctx.is_subtype("Never", "object"));
}

#[test]
fn subtype_context_numeric_widening() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("bool", "int"));
    assert!(ctx.is_subtype("bool", "float"));
    assert!(ctx.is_subtype("bool", "complex"));
    assert!(ctx.is_subtype("int", "float"));
    assert!(ctx.is_subtype("int", "complex"));
    assert!(ctx.is_subtype("float", "complex"));
    assert!(!ctx.is_subtype("float", "int"));
    assert!(!ctx.is_subtype("complex", "float"));
}

#[test]
fn subtype_context_builtin_mro_nominal() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    // str is a Sequence
    assert!(ctx.is_subtype("str", "Sequence"));
    assert!(ctx.is_subtype("str", "Hashable"));
    // list is a MutableSequence, Sequence, Iterable
    assert!(ctx.is_subtype("list", "MutableSequence"));
    assert!(ctx.is_subtype("list", "Sequence"));
    assert!(ctx.is_subtype("list", "Iterable"));
    // dict is a Mapping
    assert!(ctx.is_subtype("dict", "MutableMapping"));
    assert!(ctx.is_subtype("dict", "Mapping"));
    // set
    assert!(ctx.is_subtype("set", "MutableSet"));
    assert!(ctx.is_subtype("set", "AbstractSet"));
    // frozenset
    assert!(ctx.is_subtype("frozenset", "AbstractSet"));
    assert!(!ctx.is_subtype("frozenset", "MutableSet"));
    // tuple
    assert!(ctx.is_subtype("tuple", "Sequence"));
    // bytes, bytearray
    assert!(ctx.is_subtype("bytes", "Sequence"));
    assert!(ctx.is_subtype("bytearray", "MutableSequence"));
    // range, memoryview
    assert!(ctx.is_subtype("range", "Sequence"));
    assert!(ctx.is_subtype("memoryview", "Sequence"));
    // NoneType
    assert!(ctx.is_subtype("NoneType", "Hashable"));
    // type
    assert!(ctx.is_subtype("type", "object"));
    // Negative cases
    assert!(!ctx.is_subtype("int", "str"));
    assert!(!ctx.is_subtype("str", "int"));
}

#[test]
fn subtype_context_case_insensitive_match() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    // from_annotation lowercases, so "sequence" should match "Sequence"
    assert!(ctx.is_subtype("list", "sequence"));
    assert!(ctx.is_subtype("str", "hashable"));
}

#[test]
fn subtype_context_generic_base_name_match() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    // list[int] and list[str] share same base "list"
    assert!(ctx.is_subtype("list[int]", "list[str]"));
    assert!(ctx.is_subtype("dict[str, int]", "dict[int, str]"));
}

#[test]
fn subtype_context_abstract_container_subtype() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    // list[int] <: Sequence[int] via abstract container subtyping
    assert!(ctx.is_subtype("list[int]", "Sequence[int]"));
    assert!(ctx.is_subtype("dict[str, int]", "Mapping[str, int]"));
}

// ── User-defined class hierarchy ──

#[test]
fn subtype_context_user_class_hierarchy() {
    let source = r#"
class Animal:
    name: str

class Dog(Animal):
    breed: str

class Puppy(Dog):
    age: int
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("Dog", "Animal"));
    assert!(ctx.is_subtype("Puppy", "Dog"));
    assert!(ctx.is_subtype("Puppy", "Animal"));
    assert!(!ctx.is_subtype("Animal", "Dog"));
}

#[test]
fn subtype_context_unknown_class_defaults_to_object() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    // Unknown classes get MRO [self, object]
    assert!(ctx.is_subtype("SomeRandom", "object"));
    assert!(!ctx.is_subtype("SomeRandom", "int"));
}

// ── Protocol structural subtyping ──

#[test]
fn subtype_context_protocol_satisfied() {
    let source = r#"
from typing import Protocol

class Drawable(Protocol):
    def draw(self) -> None: ...

class Circle:
    def draw(self) -> None:
        pass

class Square:
    pass
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("Circle", "Drawable"));
    assert!(!ctx.is_subtype("Square", "Drawable"));
}

#[test]
fn subtype_context_protocol_empty() {
    let source = r#"
from typing import Protocol

class EmptyProto(Protocol):
    pass

class Anything:
    x: int = 1
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    // Empty protocol is satisfied by anything
    assert!(ctx.is_subtype("Anything", "EmptyProto"));
}

#[test]
fn subtype_context_protocol_with_property() {
    let source = r#"
from typing import Protocol

class HasName(Protocol):
    @property
    def name(self) -> str: ...

class Person:
    name: str

class Robot:
    def name(self) -> str:
        return "R2D2"
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("Person", "HasName"));
    assert!(ctx.is_subtype("Robot", "HasName"));
}

#[test]
fn subtype_context_protocol_with_attribute() {
    let source = r#"
from typing import Protocol

class HasX(Protocol):
    x: int

class WithX:
    x: int = 0

class WithoutX:
    y: int = 0
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("WithX", "HasX"));
    assert!(!ctx.is_subtype("WithoutX", "HasX"));
}

#[test]
fn subtype_context_protocol_inherited_members() {
    let source = r#"
from typing import Protocol

class HasFoo(Protocol):
    def foo(self) -> None: ...

class Base:
    def foo(self) -> None:
        pass

class Child(Base):
    pass
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    // Child inherits foo from Base
    assert!(ctx.is_subtype("Child", "HasFoo"));
}

#[test]
fn subtype_context_protocol_builtin_methods() {
    let source = r#"
from typing import Protocol

class Sized(Protocol):
    def __len__(self) -> int: ...

class HasIter(Protocol):
    def __iter__(self) -> object: ...
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    // Builtins have these methods
    assert!(ctx.is_subtype("str", "Sized"));
    assert!(ctx.is_subtype("list", "Sized"));
    assert!(ctx.is_subtype("dict", "Sized"));
    assert!(ctx.is_subtype("set", "Sized"));
    assert!(ctx.is_subtype("tuple", "Sized"));
    assert!(ctx.is_subtype("bytes", "Sized"));
    assert!(ctx.is_subtype("frozenset", "Sized"));
    assert!(ctx.is_subtype("str", "HasIter"));
    assert!(ctx.is_subtype("list", "HasIter"));
}

#[test]
fn subtype_context_protocol_skips_inherited_dunders() {
    let source = r#"
from typing import Protocol

class Proto(Protocol):
    def __init__(self) -> None: ...
    def real_method(self) -> None: ...

class Impl:
    def real_method(self) -> None:
        pass
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    // __init__ should be skipped as a protocol-inherited dunder
    assert!(ctx.is_subtype("Impl", "Proto"));
}

// ── TypedDict structural subtyping ──

#[test]
fn subtype_context_typeddict_subtype() {
    let source = r#"
from typing import TypedDict

class Point(TypedDict):
    x: int
    y: int

class Point3D(TypedDict):
    x: int
    y: int
    z: int
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    // Point3D has all fields of Point → subtype
    assert!(ctx.is_subtype("Point3D", "Point"));
    // But Point doesn't have z → not a subtype of Point3D
    assert!(!ctx.is_subtype("Point", "Point3D"));
}

#[test]
fn subtype_context_typeddict_non_typeddict_not_subtype() {
    let source = r#"
from typing import TypedDict

class TD(TypedDict):
    x: int

class Regular:
    x: int = 0
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    // Regular class is not a TypedDict → not a typeddict subtype
    assert!(!ctx.is_subtype("Regular", "TD"));
}

// ── is_subtype_with_context ──

#[test]
fn subtype_with_context_named_to_named() {
    let source = r#"
class Animal:
    pass

class Dog(Animal):
    pass
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    let src = InferredType::Named("Dog".into());
    let tgt = InferredType::Named("Animal".into());
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
    assert!(!is_subtype_with_context(&tgt, &src, &ctx));
}

#[test]
fn subtype_with_context_named_to_builtin() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    let src = InferredType::Named("int".into());
    let tgt = InferredType::Float;
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
}

#[test]
fn subtype_with_context_builtin_to_named() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    let src = InferredType::Int;
    let tgt = InferredType::Named("float".into());
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
}

#[test]
fn subtype_with_context_union_source() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    // Union[int, int] <: float (int widens to float via is_assignable_to)
    let src = InferredType::Union(vec![InferredType::Int, InferredType::Int]);
    let tgt = InferredType::Float;
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
}

#[test]
fn subtype_with_context_union_target() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    // int <: Union[str, int]
    let src = InferredType::Int;
    let tgt = InferredType::Union(vec![InferredType::Str, InferredType::Int]);
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
}

#[test]
fn subtype_with_context_optional_source() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    // Optional[int] <: Optional[int] = int | None <: int | None
    let src = InferredType::Optional(Box::new(InferredType::Int));
    let tgt = InferredType::Optional(Box::new(InferredType::Int));
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
}

#[test]
fn subtype_with_context_optional_target() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    // int <: Optional[int]
    let src = InferredType::Int;
    let tgt = InferredType::Optional(Box::new(InferredType::Int));
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
    // None <: Optional[int]
    let src_none = InferredType::None_;
    assert!(is_subtype_with_context(&src_none, &tgt, &ctx));
}

#[test]
fn subtype_with_context_list_covariant() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    let src = InferredType::List(Box::new(InferredType::Int));
    let tgt = InferredType::List(Box::new(InferredType::Float));
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
}

#[test]
fn subtype_with_context_set_covariant() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    let src = InferredType::Set(Box::new(InferredType::Int));
    let tgt = InferredType::Set(Box::new(InferredType::Float));
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
}

#[test]
fn subtype_with_context_dict_covariant() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    let src = InferredType::Dict(Box::new(InferredType::Str), Box::new(InferredType::Int));
    let tgt = InferredType::Dict(Box::new(InferredType::Str), Box::new(InferredType::Float));
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
}

#[test]
fn subtype_with_context_tuple() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    let src = InferredType::Tuple(vec![InferredType::Int, InferredType::Str]);
    let tgt = InferredType::Tuple(vec![InferredType::Float, InferredType::Str]);
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
    // Different lengths → not subtype
    let src2 = InferredType::Tuple(vec![InferredType::Int]);
    assert!(!is_subtype_with_context(&src2, &tgt, &ctx));
}

#[test]
fn subtype_with_context_callable() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);

    // Callable[[float], int] <: Callable[[int], float]
    // Return covariant: int <: float ✓
    // Params contravariant: int <: float ✓ (target param assignable to source param)
    let src = InferredType::Callable(CallableInfo {
        param_types: vec![InferredType::Float],
        return_type: Box::new(InferredType::Int),
    });
    let tgt = InferredType::Callable(CallableInfo {
        param_types: vec![InferredType::Int],
        return_type: Box::new(InferredType::Float),
    });
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
}

#[test]
fn subtype_with_context_callable_ellipsis() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);

    // Callable[..., int] <: Callable[..., float]
    let src = InferredType::Callable(CallableInfo {
        param_types: vec![],
        return_type: Box::new(InferredType::Int),
    });
    let tgt = InferredType::Callable(CallableInfo {
        param_types: vec![],
        return_type: Box::new(InferredType::Float),
    });
    assert!(is_subtype_with_context(&src, &tgt, &ctx));
}

#[test]
fn subtype_with_context_callable_param_count_mismatch() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);

    let src = InferredType::Callable(CallableInfo {
        param_types: vec![InferredType::Int, InferredType::Str],
        return_type: Box::new(InferredType::Int),
    });
    let tgt = InferredType::Callable(CallableInfo {
        param_types: vec![InferredType::Int],
        return_type: Box::new(InferredType::Int),
    });
    assert!(!is_subtype_with_context(&src, &tgt, &ctx));
}

#[test]
fn subtype_with_context_fallback() {
    let module = build_ctx("x: int = 1\n");
    let ctx = SubtypeContext::from_module(&module);
    // Any is assignable to everything
    assert!(is_subtype_with_context(
        &InferredType::Any,
        &InferredType::Int,
        &ctx
    ));
    // Never is subtype of everything
    assert!(is_subtype_with_context(
        &InferredType::Never,
        &InferredType::Str,
        &ctx
    ));
}

// ── Builtin method checking via protocol subtyping ──

#[test]
fn subtype_context_builtin_str_methods() {
    let source = r#"
from typing import Protocol

class HasSplit(Protocol):
    def split(self) -> object: ...

class HasUpper(Protocol):
    def upper(self) -> object: ...

class HasReplace(Protocol):
    def replace(self) -> object: ...
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("str", "HasSplit"));
    assert!(ctx.is_subtype("str", "HasUpper"));
    assert!(ctx.is_subtype("str", "HasReplace"));
}

#[test]
fn subtype_context_builtin_int_methods() {
    let source = r#"
from typing import Protocol

class HasAdd(Protocol):
    def __add__(self) -> object: ...

class HasBitLength(Protocol):
    def bit_length(self) -> object: ...
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("int", "HasAdd"));
    assert!(ctx.is_subtype("int", "HasBitLength"));
    assert!(ctx.is_subtype("bool", "HasAdd"));
}

#[test]
fn subtype_context_builtin_float_methods() {
    let source = r#"
from typing import Protocol

class HasIsInteger(Protocol):
    def is_integer(self) -> object: ...

class HasAsIntegerRatio(Protocol):
    def as_integer_ratio(self) -> object: ...
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("float", "HasIsInteger"));
    assert!(ctx.is_subtype("float", "HasAsIntegerRatio"));
}

#[test]
fn subtype_context_builtin_list_methods() {
    let source = r#"
from typing import Protocol

class HasAppend(Protocol):
    def append(self) -> object: ...

class HasSort(Protocol):
    def sort(self) -> object: ...
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("list", "HasAppend"));
    assert!(ctx.is_subtype("list", "HasSort"));
}

#[test]
fn subtype_context_builtin_dict_methods() {
    let source = r#"
from typing import Protocol

class HasKeys(Protocol):
    def keys(self) -> object: ...

class HasGet(Protocol):
    def get(self) -> object: ...
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("dict", "HasKeys"));
    assert!(ctx.is_subtype("dict", "HasGet"));
}

#[test]
fn subtype_context_builtin_set_methods() {
    let source = r#"
from typing import Protocol

class HasAdd(Protocol):
    def add(self) -> object: ...

class HasUnion(Protocol):
    def union(self) -> object: ...
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("set", "HasAdd"));
    assert!(ctx.is_subtype("set", "HasUnion"));
}

#[test]
fn subtype_context_builtin_tuple_methods() {
    let source = r#"
from typing import Protocol

class HasCount(Protocol):
    def count(self) -> object: ...

class HasIndex(Protocol):
    def index(self) -> object: ...
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("tuple", "HasCount"));
    assert!(ctx.is_subtype("tuple", "HasIndex"));
}

#[test]
fn subtype_context_builtin_bytes_methods() {
    let source = r#"
from typing import Protocol

class HasDecode(Protocol):
    def decode(self) -> object: ...

class HasHex(Protocol):
    def hex(self) -> object: ...
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("bytes", "HasDecode"));
    assert!(ctx.is_subtype("bytes", "HasHex"));
}

#[test]
fn subtype_context_builtin_frozenset_methods() {
    let source = r#"
from typing import Protocol

class HasIssubset(Protocol):
    def issubset(self) -> object: ...

class HasCopy(Protocol):
    def copy(self) -> object: ...
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("frozenset", "HasIssubset"));
    assert!(ctx.is_subtype("frozenset", "HasCopy"));
}

// ── Multiple inheritance ──

#[test]
fn subtype_context_multiple_inheritance() {
    let source = r#"
class A:
    pass

class B:
    pass

class C(A, B):
    pass
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("C", "A"));
    assert!(ctx.is_subtype("C", "B"));
    assert!(ctx.is_subtype("C", "object"));
}

// ── Protocol with inherited attribute via MRO ──

#[test]
fn subtype_context_protocol_inherited_attribute() {
    let source = r#"
from typing import Protocol

class HasX(Protocol):
    x: int

class Base:
    x: int = 0

class Child(Base):
    pass
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    assert!(ctx.is_subtype("Child", "HasX"));
}

// ── Non-protocol target returns false for protocol check ──

#[test]
fn subtype_context_non_protocol_target() {
    let source = r#"
class NotAProtocol:
    def foo(self) -> None: ...

class Impl:
    def foo(self) -> None:
        pass
"#;
    let module = build_ctx(source);
    let ctx = SubtypeContext::from_module(&module);
    // NotAProtocol is not a Protocol, so protocol subtyping doesn't apply
    // Only nominal subtyping applies, and Impl doesn't inherit from NotAProtocol
    assert!(!ctx.is_subtype("Impl", "NotAProtocol"));
}
