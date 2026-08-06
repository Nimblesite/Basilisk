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

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

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
    TypeVar,
    TypeVarTuple,
    ParamSpec,
    // Class-construction special bases
    Protocol,
    Generic,
    TypedDict,
    NamedTuple,
    CollectionsNamedTuple,
    // Alias and distinct-type factories
    NewType,
    TypeAliasType,
    TypeAliasQualifier,
    // Annotation qualifiers
    ClassVar,
    FinalQualifier,
    Annotated,
    Required,
    NotRequired,
    ReadOnly,
    InitVar,
    KwOnlySentinel,
    // Type forms
    Union,
    Optional,
    Literal,
    LiteralString,
    SelfType,
    Never,
    NoReturn,
    Any,
    Concatenate,
    Unpack,
    TypeGuard,
    TypeIs,
    TypeForm,
    Callable,
    // Decorators
    Overload,
    FinalDecorator,
    Override,
    RuntimeCheckable,
    DataclassTransform,
    NoTypeCheck,
    AbstractMethod,
    AbstractBase,
    AbstractBaseMeta,
    Dataclass,
    DataclassField,
    TotalOrdering,
    CachedProperty,
    Deprecated,
    // Diagnostic and introspection calls
    AssertType,
    RevealType,
    Cast,
    AssertNever,
    TypeCheckingFlag,
    GetTypeHints,
    // Abstract collection protocols
    Iterable,
    Iterator,
    Generator,
    AsyncGenerator,
    AsyncIterator,
    AsyncIterable,
    Awaitable,
    Coroutine,
    Sequence,
    MutableSequence,
    Mapping,
    MutableMapping,
    Collection,
    Container,
    Hashable,
    Sized,
    AbstractSet,
    // Deprecated capitalised container aliases (PEP 585)
    ListAlias,
    DictAlias,
    SetAlias,
    FrozensetAlias,
    TupleAlias,
    TypeAliasBuiltin,
    DequeAlias,
    OrderedDict,
    DefaultDict,
    // Enumerations
    EnumBase,
    EnumMeta,
    IntEnum,
    StrEnum,
    FlagEnum,
    IntFlagEnum,
    ReprEnum,
    EnumMember,
    EnumNonmember,
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

/// One registry entry as it appears in the data file.
#[derive(Debug, Deserialize)]
struct RegistryEntry {
    modules: Vec<String>,
    name: String,
    form: TypingForm,
}

/// The registry data file's top-level shape.
#[derive(Debug, Deserialize)]
struct RegistryFile {
    symbol: Vec<RegistryEntry>,
}

/// The specification registry, as data. No Rust file contains these spellings.
const REGISTRY_SOURCE: &str = include_str!("../../resources/typing_symbols.toml");

/// Module → name → form, built once from the registry data file.
type RegistryIndex = HashMap<String, HashMap<String, TypingForm>>;

/// The parsed registry, or an empty index if the data file is malformed.
///
/// A malformed registry is a build-time defect caught by
/// `tests/canonical_registry.rs`; degrading to an empty index here keeps the
/// resolver total rather than panicking in a library.
fn registry() -> &'static RegistryIndex {
    static REGISTRY: OnceLock<RegistryIndex> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut index: RegistryIndex = HashMap::new();
        let Ok(parsed) = toml::from_str::<RegistryFile>(REGISTRY_SOURCE) else {
            return index;
        };
        for entry in parsed.symbol {
            for module in entry.modules {
                index
                    .entry(module)
                    .or_default()
                    .insert(entry.name.clone(), entry.form);
            }
        }
        index
    })
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
