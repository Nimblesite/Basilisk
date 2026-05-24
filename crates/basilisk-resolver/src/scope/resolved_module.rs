//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! The `ResolvedModule` type — the complete resolved view of a parsed module.

use super::{
    class_types::ClassInfo,
    external_symbol::ExternalSymbol,
    function_types::FunctionInfo,
    import_types::ImportInfo,
    module_types::{
        AnnotatedTooFewArgs, AssertTypeCallInfo, CallSite, FloatParamIntAttrAccess,
        GenericSubscriptSite, LiteralStringEnumMismatch, MatchStmtInfo, ModuleAttrAccessInfo,
        ModuleAttrAssignment, ModuleBareAssignment, ModuleOrderComparisonInfo, NamedTupleDefInfo,
        NewTypeCallInfo, RevealTypeCallInfo, TypeAliasDefInfo, TypeAliasTypeCallInfo,
        TypeStatementInfo, TypeVarCallInfo, TypedDictCallInfo, TypedDictKeyViolation,
        UnhashableHashCallViolation,
    },
    span::Span,
    variable_types::VariableInfo,
    violations::{
        BoundedTypeVarAttrViolation, EnumValueTypeViolationInfo, FinalViolationInfo,
        GeneratorViolation, HistoricalPositionalViolation, InvalidStringAnnotation,
        LiteralAugmentedAssignViolation, LocalClassVarViolation, Pep695BoundViolation,
        ProtocolClassObjectViolation, ProtocolInstantiationViolation, ProtocolRtcViolation,
        ProtocolSelfViolation, ReadOnlyViolationInfo, TupleIndexViolation, TypeAliasTypeViolation,
        UnboundTypeVarUsage,
    },
};

/// The complete resolved view of a parsed module.
#[derive(Debug, Clone, Default)]
pub struct ResolvedModule {
    /// All function definitions found at any nesting level.
    pub functions: Vec<FunctionInfo>,
    /// All class definitions found at module level.
    pub classes: Vec<ClassInfo>,
    /// All module-level variable assignments.
    pub module_vars: Vec<VariableInfo>,
    /// All import statements.
    pub imports: Vec<ImportInfo>,
    /// All match statements found at any nesting level.
    pub match_stmts: Vec<MatchStmtInfo>,
    /// Module-level call sites (calls appearing in module-level expressions or assignments).
    pub calls: Vec<CallSite>,
    /// Module-level `TypeVar(...)` call sites.
    pub typevar_calls: Vec<TypeVarCallInfo>,
    /// All `reveal_type(...)` call sites found anywhere in the module.
    pub reveal_type_calls: Vec<RevealTypeCallInfo>,
    /// All `assert_type(...)` call sites found anywhere in the module.
    pub assert_type_calls: Vec<AssertTypeCallInfo>,
    /// Module-level `TypedDict(...)` functional-syntax call sites.
    pub typeddict_calls: Vec<TypedDictCallInfo>,
    /// Module-level `NewType(...)` call sites.
    pub newtype_calls: Vec<NewTypeCallInfo>,
    /// Spans of type annotations that contain multiple unbounded tuple unpacks.
    ///
    /// A tuple type is invalid if it contains more than one unbounded component,
    /// e.g. `tuple[*tuple[str, ...], *tuple[int, ...]]` — two `*tuple[T, ...]` unpacks.
    pub multiple_unbounded_tuple_spans: Vec<Span>,
    /// `Final` violations detected during AST resolution.
    ///
    /// Populated by the resolver visitor so that `BSK-E0047` can emit them
    /// without re-walking the AST.
    pub final_violations: Vec<FinalViolationInfo>,
    /// Module-level bare assignments (`name = expr`) — used to detect re-assignments
    /// to `Final`-annotated module variables.
    pub module_bare_assignments: Vec<ModuleBareAssignment>,
    /// Module-level attribute assignments (`Class.attr = expr`).
    pub module_attr_assignments: Vec<ModuleAttrAssignment>,
    /// Module-level attribute accesses (`Name.attr` as a standalone expression statement).
    ///
    /// Used by `BSK-E0057` to detect reads of `__match_args__` on dataclasses where
    /// `match_args=False` was set.
    pub module_attr_accesses: Vec<ModuleAttrAccessInfo>,
    /// Module-level ordering comparisons (`a < b`, `a <= b`, etc.) between two simple names.
    ///
    /// Used by `BSK-E0058` to detect cross-type comparisons of `order=True` dataclass instances.
    pub module_order_comparisons: Vec<ModuleOrderComparisonInfo>,
    /// `ReadOnly` `TypedDict` field mutation violations detected at module level.
    ///
    /// Used by `BSK-E0056`.
    pub readonly_violations: Vec<ReadOnlyViolationInfo>,
    /// Spans of direct calls to `Annotated` (whether bare or parameterized).
    ///
    /// PEP 593 forbids calling `Annotated` as a callable:
    /// - `Annotated()` — bare call with no type argument
    /// - `Annotated[int, ""]()` — calling a parameterized `Annotated[...]` subscript
    ///
    /// Populated by the resolver; used by `BSK-E0045`.
    pub annotated_direct_call_spans: Vec<Span>,
    /// Names that were imported from other modules where they were declared `Final`.
    ///
    /// E.g. `from _qualifiers_final_annotation_1 import TEN` when `TEN: Final[int] = 10`
    /// in that module — `TEN` would appear here.  Any bare re-assignment to such a name
    /// in the current module is a violation.
    ///
    /// Populated lazily by the resolver when the imported module can be found.
    pub imported_final_names: std::collections::HashSet<String>,
    /// Module-level `TypeAliasType(...)` call sites.
    pub type_alias_type_calls: Vec<TypeAliasTypeCallInfo>,
    /// Violations detected in `TypeAliasType(...)` calls.
    pub type_alias_type_violations: Vec<TypeAliasTypeViolation>,
    /// PEP 695 `type X = rhs` alias statements.
    pub type_statements: Vec<TypeStatementInfo>,
    /// `Annotated[...]` subscriptions with too few type arguments (< 2).
    pub annotated_too_few_args: Vec<AnnotatedTooFewArgs>,
    /// `NamedTuple(...)` definitions at module level.
    ///
    /// Each entry represents a `N = NamedTuple("N", [(field, type), ...])` call
    /// with field names resolved from Final string constants.
    /// Used by checker rules to validate `NamedTuple` call sites.
    pub namedtuple_defs: Vec<NamedTupleDefInfo>,
    /// Attribute accesses on `float`-typed parameters using `int`-only attributes.
    ///
    /// Only top-level accesses in function bodies are collected (not inside
    /// `if`/`for`/`while`/`match` blocks) so that `isinstance`-guarded uses are excluded.
    ///
    /// Used by `BSK-E0065`.
    pub float_param_int_attr_accesses: Vec<FloatParamIntAttrAccess>,
    /// Annotated local assignments where the declared type is `Literal["X.Y"]` (a string
    /// that resembles an enum member) but the RHS is a parameter typed as `Literal[X.Y]`
    /// (the actual enum member).  `Literal["Color.RED"]` ≠ `Literal[Color.RED]`.
    ///
    /// Used by `BSK-E0066`.
    pub literal_string_enum_mismatches: Vec<LiteralStringEnumMismatch>,
    /// Enum `_value_` type violations detected during AST resolution.
    ///
    /// Populated by the resolver visitor so that `BSK-E0063` can emit them
    /// without re-walking the AST.
    pub enum_value_type_violations: Vec<EnumValueTypeViolationInfo>,
    /// `ClassVar` annotations used in function-local variable or self-attribute
    /// positions, where they are forbidden by PEP 526.
    ///
    /// Populated by the resolver visitor; used by `BSK-E0036`.
    pub local_classvar_violations: Vec<LocalClassVarViolation>,
    /// PEP 695 type parameter bound violations detected during AST resolution.
    ///
    /// Covers invalid bound/constraint expressions in `class Foo[T: ...]` syntax.
    /// Used by `BSK-E0067`.
    pub pep695_bound_violations: Vec<Pep695BoundViolation>,
    /// Historical positional-only parameter violations.
    ///
    /// Covers the pre-PEP 570 `__`-prefix convention for positional-only parameters.
    /// Used by `BSK-E0068`.
    pub historical_positional_violations: Vec<HistoricalPositionalViolation>,
    /// Invalid string annotations detected during AST resolution.
    ///
    /// String annotations that contain non-type expressions (e.g. list literals,
    /// lambda calls, conditional expressions, etc.).
    /// Used by `BSK-E0069`.
    pub invalid_string_annotations: Vec<InvalidStringAnnotation>,
    /// Protocol `Self`-return conformance violations detected during resolution.
    ///
    /// When a class is passed where a `Protocol` with `Self`-returning methods
    /// is expected, but the class's corresponding method returns a different type.
    /// Used by `BSK-E0073`.
    pub protocol_self_violations: Vec<ProtocolSelfViolation>,
    /// Protocol instantiation violations: direct `Proto()` calls or instantiation
    /// of concrete subclasses that fail to implement all required protocol members.
    ///
    /// Used by `BSK-E0099`.
    pub protocol_instantiation_violations: Vec<ProtocolInstantiationViolation>,
    /// Spans of `isinstance(x, T)` calls where `T` is a `TypedDict` class.
    ///
    /// PEP 589: `TypedDict` type objects cannot be used in `isinstance()` tests.
    /// Used by `BSK-E0088`.
    pub isinstance_typeddict_violations: Vec<Span>,
    /// `TypedDict` subscript key/value violations and invalid dict-literal assignments.
    ///
    /// Covers:
    /// - `td["invalid_key"] = val` where `"invalid_key"` is not a `TypedDict` field.
    /// - `td["field"] = wrong_type_val` where the value type mismatches the field type.
    /// - `var: TypedDict = {invalid/missing keys}` where the literal doesn't match the schema.
    ///   Used by `BSK-E0089`.
    pub typeddict_key_violations: Vec<TypedDictKeyViolation>,
    /// Module-level `TypeAlias` annotated assignments.
    ///
    /// Each entry represents `Name: TypeAlias = expr` at module level.
    /// Used by `BSK-E0092` to check that subscript sites respect the alias arity.
    pub type_alias_defs: Vec<TypeAliasDefInfo>,
    /// Module-level subscript expression sites (`Name[args...]` used as a statement).
    ///
    /// Collected so that rules can check whether user-defined generic types are
    /// subscripted with the correct number of type arguments.
    /// Used by `BSK-E0092`.
    pub generic_subscript_sites: Vec<GenericSubscriptSite>,
    /// Augmented-assignment violations on `Literal`-typed variables.
    ///
    /// Used by `BSK-E0100`.
    pub literal_augmented_assign_violations: Vec<LiteralAugmentedAssignViolation>,
    /// Tuple index out-of-bounds violations.
    ///
    /// Used by `BSK-E0103`.
    pub tuple_index_violations: Vec<TupleIndexViolation>,
    /// Invalid attribute accesses on bounded type variables.
    ///
    /// Used by `BSK-E0105`.
    pub bounded_typevar_attr_violations: Vec<BoundedTypeVarAttrViolation>,
    /// Protocol class used where `type[Proto]` is expected.
    ///
    /// Used by `BSK-E0106`.
    pub protocol_class_object_violations: Vec<ProtocolClassObjectViolation>,
    /// `ClassName(args).__hash__()` calls on non-hashable dataclasses.
    ///
    /// A `@dataclass` with `eq=True` (the default) sets `__hash__` to `None`
    /// unless `frozen=True`, `unsafe_hash=True`, or `__hash__` is defined explicitly.
    /// Calling `.__hash__()` on such an instance is an error.
    ///
    /// Used by `BSK-E0063`.
    pub unhashable_hash_call_violations: Vec<UnhashableHashCallViolation>,
    /// Protocol `isinstance`/`issubclass` violations.
    ///
    /// Covers:
    /// - `isinstance(x, Proto)` / `issubclass(x, Proto)` where `Proto` is not
    ///   decorated with `@runtime_checkable`.
    /// - `issubclass(x, Proto)` where `Proto` is a data protocol (has attributes).
    ///
    /// Used by `BSK-E0114`.
    pub protocol_runtime_checkable_violations: Vec<ProtocolRtcViolation>,
    /// Generator-related type violations (invalid return type, yield mismatches).
    ///
    /// Used by `BSK-E0115`.
    pub generator_violations: Vec<GeneratorViolation>,
    /// Unbound type variable usages detected during AST resolution.
    ///
    /// Covers inner class `TypeVar` reuse and function-nested Generic classes
    /// that cannot be detected from the flattened `ResolvedModule` data alone.
    ///
    /// Used by `BSK-E0117`.
    pub unbound_typevar_usages: Vec<UnboundTypeVarUsage>,
    /// Symbols imported from other modules during cross-module analysis.
    ///
    /// Populated by the workspace layer after import resolution, not during
    /// the initial `resolve()` pass. Maps import name → external symbol info.
    pub imported_symbols: std::collections::HashMap<String, ExternalSymbol>,
    /// AST-derived PEP 695 scoping facts used by `BSK-E0149`.
    ///
    /// Populated from `ruff_python_ast` nodes (never from raw line scanning) so
    /// that docstring/comment/string content is never mistaken for real
    /// `class` / `def` / `type` declarations.
    pub pep695_scoping: super::pep695_scoping::Pep695Scoping,
    /// The source file path.
    pub path: String,
    /// The original source text (forwarded from parser for span restoration).
    pub source: String,
}
