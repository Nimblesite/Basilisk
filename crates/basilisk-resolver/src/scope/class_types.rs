//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Class-related types: class info, generic parameters, type arguments.

use basilisk_canonical::TypingForm;

use super::{span::Span, variable_types::AttributeInfo};

/// What a declared base class expression resolves to.
///
/// This is the identity a hierarchy is built on. It is produced at collection
/// time by resolving the base EXPRESSION through the module's binding table,
/// so it is unaffected by how the base happens to be written: an aliased
/// import, a module-qualified path, and a bare name that all denote one class
/// give one answer, and two classes that merely share a rendered name never
/// collapse into one. Implements [RESOLV-CANONICAL-BINDING].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedBase {
    /// A class defined in this module, keyed on its definition site — the
    /// span of the `class` statement's name token ([`ClassInfo::name_span`]).
    LocalClass(Span),
    /// A definition the canonical registry describes: `builtins.object`,
    /// `typing.Generic`, `typing.Protocol`, `typing.TypedDict`, and so on.
    Form(TypingForm),
    /// A base that resolves to no definition visible from here — a class
    /// imported from a module outside the registry, a computed base, a name
    /// bound to something opaque.
    ///
    /// Means "unknown", never "no base": a consumer must abstain on it rather
    /// than conclude anything from its absence.
    Unknown,
}

impl ResolvedBase {
    /// The definition site, when this base is a class defined in this module.
    #[must_use]
    pub const fn local_site(self) -> Option<Span> {
        match self {
            Self::LocalClass(span) => Some(span),
            Self::Form(_) | Self::Unknown => None,
        }
    }

    /// Whether this base denotes exactly `form`.
    #[must_use]
    pub fn is_form(self, form: TypingForm) -> bool {
        self == Self::Form(form)
    }
}

/// One declared base class: where it is written, and what it denotes.
///
/// The span locates a diagnostic and supplies its message text; every verdict
/// comes from [`Self::resolved`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseRef {
    /// The span of the base expression, as written.
    pub span: Span,
    /// What the base expression resolves to.
    pub resolved: ResolvedBase,
}

/// Type parameters declared in a `Generic[T1, T2, ...]` base expression.
#[derive(Debug, Clone, PartialEq)]
pub struct GenericParamInfo {
    /// The name of the type parameter (e.g. `"T"`, `"T_co"`).
    pub name: String,
    /// The source span of this parameter name inside `Generic[...]`.
    pub span: Span,
    /// `true` when this param was extracted from a starred expression (`*Ts`),
    /// indicating it is a `TypeVarTuple` unpack in `Generic[...]`.
    pub is_typevartuple: bool,
}

/// A type argument in a subscript expression, possibly nested.
///
/// Represents both simple names (`T`) and parameterised types (`list[T]`).
#[derive(Debug, Clone, PartialEq)]
pub enum TypeArg {
    /// A simple name reference (e.g. `T`, `int`).
    Simple(String),
    /// A subscript expression (e.g. `list[T]`, `Mapping[K, V]`).
    Subscript {
        /// The base name of the subscript.
        base: String,
        /// The type arguments inside the brackets.
        args: Vec<TypeArg>,
    },
}

/// A base class subscript entry, recording the base name and its type arguments.
///
/// For `class Foo(Base[T, int])`, this captures `base_name = "Base"`,
/// `type_arg_names = ["T", "int"]`, and the structured `type_args`.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseSubscriptEntry {
    /// The base class name being subscripted.
    pub base_name: String,
    /// Flat list of type argument names (for simple cases).
    pub type_arg_names: Vec<String>,
    /// Rich structured type arguments (for nested generics).
    pub type_args: Vec<TypeArg>,
    /// The source span of the subscript expression.
    pub span: Span,
}

/// A class definition with its attributes and method names.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Python classes have many boolean flags"
)]
pub struct ClassInfo {
    /// The class name.
    pub name: String,
    /// The span of the class name token.
    pub name_span: Span,
    /// The span of the `class` keyword.
    pub def_span: Span,
    /// Base class names (simple names only; complex expressions ignored).
    ///
    /// A RENDERING, kept for diagnostic message text only. It cannot answer
    /// which class a base denotes: `other.Movie` is recorded as `Movie`, and a
    /// base reached through an alias is recorded under the alias. Use
    /// [`Self::resolved_bases`] for identity.
    pub bases: Vec<String>,
    /// Every declared base, in declaration order: where it is written and
    /// what it denotes.
    ///
    /// The identity half of `bases`, and the only one a verdict may come
    /// from. See [`BaseRef`] and [`ResolvedBase`].
    pub resolved_bases: Vec<BaseRef>,
    /// Attributes declared directly in the class body.
    pub attributes: Vec<AttributeInfo>,
    /// Names of methods defined in the class body.
    pub method_names: Vec<String>,
    /// Decorators for each method: `(method_name, [decorator_names])`.
    pub method_decorators: Vec<(String, Vec<String>)>,
    /// Class-level decorator name spans (for semantic token highlighting).
    pub decorator_spans: Vec<(String, Span)>,
    /// Type parameter names extracted from a `Generic[...]` base, if present.
    pub generic_params: Vec<GenericParamInfo>,
    /// `true` when the class inherits from `TypedDict`.
    pub is_typed_dict: bool,
    /// `true` when the `TypedDict` has `total=True` (the default). `false` when `total=False`.
    pub is_typeddict_total: bool,
    /// Keyword argument names in the class definition (e.g. `metaclass`, `total`, `other`).
    pub class_keywords: Vec<String>,
    /// `true` when the class is decorated with `@dataclass` or `@dataclass(...)`.
    pub is_dataclass: bool,
    /// `true` when the dataclass is decorated with `frozen=True`.
    pub is_dataclass_frozen: bool,
    /// `true` when the dataclass is decorated with `kw_only=True`.
    ///
    /// When `kw_only=True`, all fields are keyword-only in `__init__`
    /// unless individually overridden with `field(kw_only=False)`.
    pub is_dataclass_kw_only: bool,
    /// `true` when the dataclass is decorated with `match_args=False`
    /// (suppresses `__match_args__` generation).
    pub is_dataclass_match_args_false: bool,
    /// `true` when the dataclass is decorated with `order=True`
    /// (synthesizes ordering comparison methods).
    pub is_dataclass_order: bool,
    /// `true` when the dataclass is decorated with `unsafe_hash=True`.
    ///
    /// `unsafe_hash=True` forces generation of `__hash__` even when `eq=True`
    /// and the class is not frozen.
    pub is_dataclass_unsafe_hash: bool,
    /// `true` when the dataclass is decorated with `eq=False`.
    ///
    /// When `eq=False`, `__hash__` is not touched by the dataclass machinery,
    /// so the class retains the inherited `__hash__` from `object`.
    pub is_dataclass_eq_false: bool,
    /// `true` when the dataclass is decorated with `init=False`.
    ///
    /// When `init=False`, no `__init__` is synthesized.  If the class also
    /// defines no explicit `__init__`, calling it with arguments is an error.
    pub is_dataclass_init_false: bool,
    /// `true` when the class is decorated with `@final` or `typing.final`.
    pub is_final: bool,
    /// `true` when the class directly or transitively inherits from an `Enum` family class.
    pub is_enum: bool,
    /// `true` when a declared base resolves to `typing.Protocol` (bare or subscripted).
    ///
    /// Resolved through the module's bindings at collection time — never from
    /// the base's spelling. Implements [RESOLV-CANONICAL-BINDING].
    pub is_protocol: bool,
    /// `true` when a declared base resolves to `typing.NamedTuple` or
    /// `collections.namedtuple`'s class form.
    ///
    /// Resolved through the module's bindings at collection time — never from
    /// the base's spelling. Implements [RESOLV-CANONICAL-BINDING].
    pub is_namedtuple: bool,
    /// `true` when the class uses PEP 695 type parameter syntax (`class Foo[T]: ...`).
    pub has_pep695_type_params: bool,
    /// Names of the PEP 695 type parameters declared in `[...]` for this class.
    ///
    /// For `class Foo[T, *Ts, **P]: ...`, this is `["T", "Ts", "P"]`.
    pub pep695_type_param_names: Vec<String>,
    /// All simple names referenced inside base class expressions (including subscript arguments).
    ///
    /// For `class Foo[V](dict[K, V])`, this would include `"dict"`, `"K"`, and `"V"`.
    /// Used by E0042 to detect traditional `TypeVars` mixed into PEP 695 classes.
    pub base_expression_names: Vec<String>,
    /// Every NAME referenced inside this class's base expressions that an
    /// assignment in this module bound, as `(reference span, value site)`.
    ///
    /// [`Self::base_expression_names`] is a `Vec<String>` of RENDERED simple
    /// names, which cannot answer whether a base argument denotes a particular
    /// value: `Alias = T; class Foo(Generic[Alias])` names no `T`, while an
    /// unrelated `class T` matches one. The value site here is the range of
    /// the EXPRESSION the assignment bound — `TypeVarCallInfo::span` for a
    /// `T = TypeVar("T")` — so identity survives aliasing and rebinding.
    pub base_name_value_sites: Vec<(Span, Span)>,
    /// Spans of arguments in `Generic[...]` or `Protocol[...]` that are NOT simple names
    /// (i.e. not plain `TypeVar` references, but literals, subscripts, etc.).
    ///
    /// For `class Foo(Generic[int])`, this would contain the span of `int`.
    /// Used by E0043 to detect non-TypeVar arguments to Generic/Protocol.
    pub generic_non_typevar_args: Vec<Span>,
    /// The metaclass name if specified via `metaclass=Meta` keyword.
    ///
    /// For `class Foo(metaclass=Meta): ...`, this is `Some("Meta")`.
    /// `None` when no explicit metaclass is specified.
    pub metaclass_name: Option<String>,
    /// DEFINITION SITE of the metaclass, when `metaclass=` names a class this
    /// module defines.
    ///
    /// [`Self::metaclass_name`] is a RENDERING: it is filled only when the
    /// `metaclass=` value is a bare word, so `metaclass=mod.Meta` records
    /// nothing, and an assignment alias records the alias's spelling rather
    /// than the class it names. This field is the metaclass EXPRESSION
    /// resolved through the binding table, and is the lawful key for "whose
    /// `__call__` governs construction?". `None` means the metaclass is not a
    /// class of this module — abstention, never "no metaclass".
    pub metaclass_site: Option<Span>,
    /// `true` when at least one base class expression is a subscript.
    ///
    /// For example, `class Foo(SubclassMe[float])` has a subscript base.
    /// Used by `generics_defaults_specialization` to detect classes that have fully specialised their
    /// generic bases and therefore cannot be further subscripted.
    pub has_subscript_base: bool,
    /// Structured information about subscripted base classes.
    ///
    /// For `class Foo(Base[T, int])`, this contains an entry with `base_name = "Base"`.
    /// Used by `generics_variance` for variance checking.
    pub base_subscripts: Vec<BaseSubscriptEntry>,
    /// `true` when the dataclass is decorated with `slots=True`.
    pub is_dataclass_slots: bool,
    /// `true` when the class has a manual `__slots__` definition in the body.
    pub has_manual_slots: bool,
    /// The docstring of this class, if present (first statement is a string literal).
    pub docstring: Option<String>,
}
