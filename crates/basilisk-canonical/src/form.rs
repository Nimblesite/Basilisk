//! Implements [RESOLV-CANONICAL-REGISTRY].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL-REGISTRY
//!
//! The specification forms Basilisk recognises, and the registry that maps a
//! canonical definition site to one.
//!
//! Every variant of [`TypingForm`] is Basilisk's OWN name for a concept the
//! typing specification defines. None of them is ever compared against text in
//! the file being checked — a variant is the ANSWER that binding resolution
//! produces, never the question it asks. The Python spellings that identify
//! each definition site live in `resources/typing_symbols.toml`, as data, and
//! appear in no Rust file.

use serde::Deserialize;

use crate::registry::registry;

/// A fully-qualified definition site: the module a symbol is defined in, and
/// the name it is defined under there.
///
/// This is produced by resolving a use-site expression through the module's
/// imports and local bindings — never by reading the characters at the use
/// site. `from typing import ClassVar as CV` and `import typing as t;
/// t.ClassVar` both produce the same canonical symbol, and a local
/// `class ClassVar:` produces none.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalSymbol {
    /// Dotted module path the symbol is defined in.
    pub module: String,
    /// The name the symbol is defined under in that module.
    pub name: String,
}

impl CanonicalSymbol {
    /// Build a canonical symbol from its module and name.
    pub fn new(module: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            name: name.into(),
        }
    }
}

/// A construct the typing specification defines.
///
/// Variants are ordered as the registry groups them. Adding one requires a
/// corresponding entry in `resources/typing_symbols.toml`; the registry test
/// fails the build if a form has no definition site or a definition site names
/// a declaration absent from bundled typeshed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TypingForm {
    // Type-parameter factories
    /// `TypeVar` — a single type parameter (PEP 484, PEP 695).
    TypeVar,
    /// `TypeVarTuple` — a variadic type parameter (PEP 646).
    TypeVarTuple,
    /// `ParamSpec` — a callable parameter-list parameter (PEP 612).
    ParamSpec,
    // Class-construction special bases
    /// `Protocol` — the structural base class (PEP 544).
    Protocol,
    /// `Generic` — the explicit generic base class (PEP 484).
    Generic,
    /// `TypedDict` — a dictionary with per-key types (PEP 589).
    TypedDict,
    /// `typing.NamedTuple` — the typed tuple subclass (PEP 484).
    NamedTuple,
    /// `collections.namedtuple` — the untyped runtime factory.
    CollectionsNamedTuple,
    // Alias and distinct-type factories
    /// `NewType` — a distinct type over a base type (PEP 484).
    NewType,
    /// `TypeAliasType` — the PEP 695 alias object.
    TypeAliasType,
    /// `TypeAlias` — the PEP 613 explicit-alias qualifier.
    TypeAliasQualifier,
    // Annotation qualifiers
    /// `ClassVar` — a class-level attribute, never per-instance (PEP 526).
    ClassVar,
    /// `Final` as an annotation qualifier — no rebinding (PEP 591).
    FinalQualifier,
    /// `Annotated` — a type carrying arbitrary metadata (PEP 593).
    Annotated,
    /// `Required` — a `TypedDict` key that must be present (PEP 655).
    Required,
    /// `NotRequired` — a `TypedDict` key that may be absent (PEP 655).
    NotRequired,
    /// `ReadOnly` — a `TypedDict` key that cannot be reassigned (PEP 705).
    ReadOnly,
    /// `dataclasses.InitVar` — an `__init__`-only pseudo-field.
    InitVar,
    /// `dataclasses.KW_ONLY` — the keyword-only field separator.
    KwOnlySentinel,
    // Type forms
    /// `Union` — a union of types (PEP 484).
    Union,
    /// `Optional[T]` — shorthand for `T | None` (PEP 484).
    Optional,
    /// `Literal` — a type inhabited by specific values (PEP 586).
    Literal,
    /// `LiteralString` — any literal-derived string (PEP 675).
    LiteralString,
    /// `Self` — the enclosing class's own type (PEP 673).
    SelfType,
    /// `Never` — the empty type, inhabited by no value.
    Never,
    /// `NoReturn` — a callable that never returns normally (PEP 484).
    NoReturn,
    /// `Any` — the gradual type (PEP 484).
    Any,
    /// `Concatenate` — prepends positional parameters to a `ParamSpec` (PEP 612).
    Concatenate,
    /// `Unpack` — unpacks a `TypeVarTuple` or `TypedDict` (PEP 646, PEP 692).
    Unpack,
    /// `TypeGuard` — a user-defined one-way narrowing return type (PEP 647).
    TypeGuard,
    /// `TypeIs` — a user-defined two-way narrowing return type (PEP 742).
    TypeIs,
    /// `TypeForm` — the type of a type expression itself.
    TypeForm,
    /// `Callable` — a callable's parameter and return types (PEP 484).
    Callable,
    // Decorators
    /// `@overload` — one signature of an overloaded callable (PEP 484).
    Overload,
    /// `@final` — no subclassing or overriding (PEP 591).
    FinalDecorator,
    /// `@override` — declares a method overrides a base method (PEP 698).
    Override,
    /// `@runtime_checkable` — permits `isinstance` against a protocol (PEP 544).
    RuntimeCheckable,
    /// `@dataclass_transform` — dataclass-like decorator semantics (PEP 681).
    DataclassTransform,
    /// `@no_type_check` — suppresses checking of a definition.
    NoTypeCheck,
    /// `@abstractmethod` — a concrete subclass must implement it.
    AbstractMethod,
    /// `staticmethod` — the builtin static-method descriptor.
    StaticMethod,
    /// `classmethod` — the builtin class-method descriptor.
    ClassMethod,
    /// `abc.ABC` — the abstract base class.
    AbstractBase,
    /// `abc.ABCMeta` — the abstract base metaclass.
    AbstractBaseMeta,
    /// `@dataclass` — synthesizes `__init__` and friends (PEP 557).
    Dataclass,
    /// `dataclasses.field` — a per-field specifier.
    DataclassField,
    /// `@total_ordering` — fills in the remaining comparisons.
    TotalOrdering,
    /// `@cached_property` — a property computed once per instance.
    CachedProperty,
    /// `@deprecated` — marks a symbol deprecated (PEP 702).
    Deprecated,
    // Diagnostic and introspection calls
    /// `assert_type` — asserts the static type at a point.
    AssertType,
    /// `reveal_type` — reports the inferred type.
    RevealType,
    /// `cast` — asserts a type the checker cannot verify.
    Cast,
    /// `assert_never` — asserts exhaustiveness at a point.
    AssertNever,
    /// `TYPE_CHECKING` — true only during static analysis.
    TypeCheckingFlag,
    /// `get_type_hints` — resolves annotations at runtime.
    GetTypeHints,
    // Abstract collection protocols
    /// `Iterable` — supports `__iter__`.
    Iterable,
    /// `Iterator` — supports `__iter__` and `__next__`.
    Iterator,
    /// `Generator` — a synchronous generator's yield/send/return types.
    Generator,
    /// `AsyncGenerator` — an asynchronous generator's yield/send types.
    AsyncGenerator,
    /// `AsyncIterator` — supports `__aiter__` and `__anext__`.
    AsyncIterator,
    /// `AsyncIterable` — supports `__aiter__`.
    AsyncIterable,
    /// `Awaitable` — supports `__await__`.
    Awaitable,
    /// `Coroutine` — an awaitable with send and throw.
    Coroutine,
    /// `Sequence` — an ordered, indexable, sized collection.
    Sequence,
    /// `MutableSequence` — a `Sequence` supporting in-place mutation.
    MutableSequence,
    /// `Mapping` — a read-only key-to-value collection.
    Mapping,
    /// `MutableMapping` — a `Mapping` supporting in-place mutation.
    MutableMapping,
    /// `Collection` — sized, iterable, and supports `in`.
    Collection,
    /// `Container` — supports `in`.
    Container,
    /// `Hashable` — supports `__hash__`.
    Hashable,
    /// `Sized` — supports `__len__`.
    Sized,
    /// `AbstractSet` — a read-only set.
    AbstractSet,
    // Deprecated capitalised container aliases (PEP 585)
    /// `typing.List` — the deprecated alias for `list` (PEP 585).
    ListAlias,
    /// `typing.Dict` — the deprecated alias for `dict` (PEP 585).
    DictAlias,
    /// `typing.Set` — the deprecated alias for `set` (PEP 585).
    SetAlias,
    /// `typing.FrozenSet` — the deprecated alias for `frozenset` (PEP 585).
    FrozensetAlias,
    /// `typing.Tuple` — the deprecated alias for `tuple` (PEP 585).
    TupleAlias,
    /// `typing.Type` — the deprecated alias for `type` (PEP 585).
    TypeAliasBuiltin,
    /// `typing.Deque` — the deprecated alias for `collections.deque` (PEP 585).
    DequeAlias,
    /// `OrderedDict` — an insertion-ordered mapping.
    OrderedDict,
    /// `defaultdict` — a mapping with a default factory.
    DefaultDict,
    // Builtin classes
    /// `int` — the builtin integer class.
    IntClass,
    /// `float` — the builtin floating-point class.
    FloatClass,
    /// `complex` — the builtin complex-number class.
    ComplexClass,
    /// `bool` — the builtin boolean class, a subclass of `int`.
    BoolClass,
    /// `str` — the builtin string class.
    StrClass,
    /// `bytes` — the builtin bytes class.
    BytesClass,
    /// `bytearray` — the builtin mutable bytes class.
    BytearrayClass,
    /// `object` — the universal base class.
    ObjectClass,
    /// `list` — the builtin list class.
    ListClass,
    /// `dict` — the builtin dictionary class.
    DictClass,
    /// `set` — the builtin set class.
    SetClass,
    /// `frozenset` — the builtin frozen-set class.
    FrozensetClass,
    /// `tuple` — the builtin tuple class.
    TupleClass,
    /// `type` — the builtin metaclass.
    TypeClass,
    /// `types.NoneType` — the type of `None`.
    NoneTypeClass,
    // Builtin narrowing functions
    /// `isinstance` — the builtin instance check, a narrowing guard.
    IsinstanceFunction,
    /// `issubclass` — the builtin subclass check, a narrowing guard.
    IssubclassFunction,
    /// `hasattr` — the builtin attribute probe, a narrowing guard.
    HasattrFunction,
    // Enumerations
    /// `enum.Enum` — the enumeration base class.
    EnumBase,
    /// `enum.EnumMeta` — the enumeration metaclass.
    EnumMeta,
    /// `enum.IntEnum` — an enumeration whose members are `int`s.
    IntEnum,
    /// `enum.StrEnum` — an enumeration whose members are `str`s.
    StrEnum,
    /// `enum.Flag` — a bit-combinable enumeration.
    FlagEnum,
    /// `enum.IntFlag` — a bit-combinable `int` enumeration.
    IntFlagEnum,
    /// `enum.ReprEnum` — an enumeration keeping its mixin's `__repr__`.
    ReprEnum,
    /// `enum.member` — forces a value to be an enumeration member.
    EnumMember,
    /// `enum.nonmember` — excludes a value from membership.
    EnumNonmember,
    /// `enum.auto` — assigns the next automatic member value.
    EnumAuto,
}

impl TypingForm {
    /// Whether this form introduces a type parameter when called.
    #[must_use]
    pub const fn is_type_parameter_factory(self) -> bool {
        matches!(self, Self::TypeVar | Self::TypeVarTuple | Self::ParamSpec)
    }

    /// Whether this form is a generator-like return type.
    #[must_use]
    pub const fn is_generator_like(self) -> bool {
        matches!(
            self,
            Self::Generator
                | Self::AsyncGenerator
                | Self::Iterator
                | Self::Iterable
                | Self::AsyncIterator
                | Self::AsyncIterable
        )
    }

    /// Whether this form qualifies an annotation rather than being a type.
    #[must_use]
    pub const fn is_annotation_qualifier(self) -> bool {
        matches!(
            self,
            Self::ClassVar
                | Self::FinalQualifier
                | Self::Annotated
                | Self::Required
                | Self::NotRequired
                | Self::ReadOnly
                | Self::InitVar
        )
    }

    /// Whether this form is a `TypedDict` item qualifier.
    #[must_use]
    pub const fn is_typed_dict_qualifier(self) -> bool {
        matches!(self, Self::Required | Self::NotRequired | Self::ReadOnly)
    }

    /// Whether this form is one of the enumeration base classes.
    #[must_use]
    pub const fn is_enum_base(self) -> bool {
        matches!(
            self,
            Self::EnumBase
                | Self::IntEnum
                | Self::StrEnum
                | Self::FlagEnum
                | Self::IntFlagEnum
                | Self::ReprEnum
        )
    }
}

/// The specification form defined at `symbol`'s definition site, if any.
///
/// The lookup is by DEFINITION SITE. It is only ever reached after binding
/// resolution has established which definition an expression refers to.
#[must_use]
pub fn form_at(symbol: &CanonicalSymbol) -> Option<TypingForm> {
    registry()
        .get(symbol.module.as_str())
        .and_then(|names| names.get(symbol.name.as_str()))
        .copied()
}

/// Whether `module` defines any symbol the specification registry knows.
///
/// Used to decide whether a star-import can introduce specification forms.
#[must_use]
pub fn module_is_registered(module: &str) -> bool {
    registry().contains_key(module)
}

/// The form `name` has when it is defined in `module`, for star-import
/// resolution.
#[must_use]
pub fn form_in_module(module: &str, name: &str) -> Option<TypingForm> {
    registry()
        .get(module)
        .and_then(|names| names.get(name))
        .copied()
}

/// The form `name` carries when it resolves to the builtin scope.
///
/// The builtin scope is the outermost LEGB scope; its definition site is the
/// `builtins` module, so the lookup is still by definition site. Callers must
/// first establish that the module does not bind `name` itself.
#[must_use]
pub(crate) fn builtin_form_of_name(name: &str) -> Option<TypingForm> {
    form_in_module("builtins", name)
}

/// Every (module, name) definition site in the registry, for validation.
#[must_use]
pub fn all_definition_sites() -> Vec<(String, String, TypingForm)> {
    registry()
        .iter()
        .flat_map(|(module, names)| {
            names
                .iter()
                .map(move |(name, form)| (module.clone(), name.clone(), *form))
        })
        .collect()
}
