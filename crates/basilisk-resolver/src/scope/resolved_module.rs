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

use std::sync::{Arc, OnceLock};

use basilisk_parser::ParsedModule;

/// A once-parsed AST for a resolved module, shared across all rules.
///
/// Dozens of checker rules need the raw `ruff` AST. Instead of each re-parsing
/// the whole source (which made a file cost ~one full parse *per parsing rule*),
/// the first rule to ask parses once and every later rule reuses the result
/// through this [`OnceLock`]. The AST is a pure function of
/// [`ResolvedModule::source`], so it never affects module equality — two modules
/// are equal exactly when their real fields (source included) are.
#[derive(Debug, Clone, Default)]
pub struct LazyAst(OnceLock<Option<Arc<ParsedModule>>>);

impl LazyAst {
    /// Return the parsed AST for `source`/`path`, parsing and caching on first
    /// call and returning the shared result thereafter. `None` iff parsing fails.
    #[must_use]
    pub fn get_or_parse(&self, source: &str, path: &str) -> Option<&ParsedModule> {
        self.0
            .get_or_init(|| {
                basilisk_parser::parse_source(source.to_owned(), path.to_owned())
                    .ok()
                    .map(Arc::new)
            })
            .as_deref()
    }
}

impl PartialEq for LazyAst {
    /// The cached AST is derived from `source`, so it carries no independent
    /// identity: module equality is decided entirely by the other fields.
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

/// The complete resolved view of a parsed module.
#[derive(Debug, Clone, Default, PartialEq)]
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
    /// Call sites from module-level expressions and assignments — and, because
    /// the collector walks every body, from function and class bodies too.
    /// Consumers that need genuinely module-level calls must filter by span
    /// against `functions`/`classes` `def_span`s.
    pub calls: Vec<CallSite>,
    /// Every `cast(...)` call site in the module, in **any** expression
    /// position — `return cast(...)`, `f(cast(...))`, and nested expressions
    /// included. `cast()` is invalid wherever it appears, so `directives_cast`
    /// needs a complete view that [`Self::calls`] deliberately does not give
    /// (issue #335).
    pub cast_calls: Vec<CallSite>,
    /// Every name bound at module scope, whatever the binding form: `=`,
    /// tuple/star unpacking, `for`/`with`/`except ... as` targets, walrus,
    /// `match` captures, `def`/`class`/`type` names, and import bindings —
    /// mapped to HOW MANY binding sites introduce it.
    ///
    /// `module_vars` only records plain single-target assignments, so rules
    /// deciding whether a name exists at module scope (`names_undefined`,
    /// issue #397) consult this map instead. The count distinguishes a name a
    /// statement binds ONLY for itself (`class D(D)` — count 1, the base `D`
    /// is unbound) from legal shadowing (`class D: ...` then `class D(D)` —
    /// count 2, the base resolves to the earlier binding).
    pub module_bindings: std::collections::HashMap<String, usize>,
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
    /// Populated by the resolver visitor so that `annotations_forward_refs` can emit them
    /// without re-walking the AST.
    pub final_violations: Vec<FinalViolationInfo>,
    /// Module-level bare assignments (`name = expr`) — used to detect re-assignments
    /// to `Final`-annotated module variables.
    pub module_bare_assignments: Vec<ModuleBareAssignment>,
    /// Module-level attribute assignments (`Class.attr = expr`).
    pub module_attr_assignments: Vec<ModuleAttrAssignment>,
    /// Module-level attribute accesses (`Name.attr` as a standalone expression statement).
    ///
    /// Used by `aliases_type_statement` to detect reads of `__match_args__` on dataclasses where
    /// `match_args=False` was set.
    pub module_attr_accesses: Vec<ModuleAttrAccessInfo>,
    /// Module-level ordering comparisons (`a < b`, `a <= b`, etc.) between two simple names.
    ///
    /// Used by `qualifiers_annotated_2` to detect cross-type comparisons of `order=True` dataclass instances.
    pub module_order_comparisons: Vec<ModuleOrderComparisonInfo>,
    /// `ReadOnly` `TypedDict` field mutation violations detected at module level.
    ///
    /// Used by `typeddicts_readonly`.
    pub readonly_violations: Vec<ReadOnlyViolationInfo>,
    /// Spans of direct calls to `Annotated` (whether bare or parameterized).
    ///
    /// PEP 593 forbids calling `Annotated` as a callable:
    /// - `Annotated()` — bare call with no type argument
    /// - `Annotated[int, ""]()` — calling a parameterized `Annotated[...]` subscript
    ///
    /// Populated by the resolver; used by `qualifiers_annotated`.
    pub annotated_direct_call_spans: Vec<Span>,
    /// Names that were imported from other modules where they were declared `Final`.
    ///
    /// E.g. `from _qualifiers_final_annotation_1 import TEN` when `TEN: Final[int] = 10`
    /// in that module — `TEN` would appear here.  Any bare re-assignment to such a name
    /// in the current module is a violation.
    ///
    /// Populated lazily by the resolver when the imported module can be found.
    pub imported_final_names: std::collections::HashSet<String>,
    /// For each base class imported from a sibling module, the set of its method
    /// names declared `@final`. Lets `qualifiers_final_decorator` detect overriding a `@final`
    /// method whose definition lives in an imported (e.g. `.pyi`) base.
    pub imported_final_methods:
        std::collections::HashMap<String, std::collections::HashSet<String>>,
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
    /// Used by `specialtypes_promotions`.
    pub float_param_int_attr_accesses: Vec<FloatParamIntAttrAccess>,
    /// Annotated local assignments where the declared type is `Literal["X.Y"]` (a string
    /// that resembles an enum member) but the RHS is a parameter typed as `Literal[X.Y]`
    /// (the actual enum member).  `Literal["Color.RED"]` ≠ `Literal[Color.RED]`.
    ///
    /// Used by `enums_member_values`.
    pub literal_string_enum_mismatches: Vec<LiteralStringEnumMismatch>,
    /// Enum `_value_` type violations detected during AST resolution.
    ///
    /// Populated by the resolver visitor so that `dataclasses_hash` can emit them
    /// without re-walking the AST.
    pub enum_value_type_violations: Vec<EnumValueTypeViolationInfo>,
    /// `ClassVar` annotations used in function-local variable or self-attribute
    /// positions, where they are forbidden by PEP 526.
    ///
    /// Populated by the resolver visitor; used by `classes_classvar`.
    pub local_classvar_violations: Vec<LocalClassVarViolation>,
    /// PEP 695 type parameter bound violations detected during AST resolution.
    ///
    /// Covers invalid bound/constraint expressions in `class Foo[T: ...]` syntax.
    /// Used by `enums_members_2`.
    pub pep695_bound_violations: Vec<Pep695BoundViolation>,
    /// Historical positional-only parameter violations.
    ///
    /// Covers the pre-PEP 570 `__`-prefix convention for positional-only parameters.
    /// Used by `literals_parameterizations_2`.
    pub historical_positional_violations: Vec<HistoricalPositionalViolation>,
    /// Invalid string annotations detected during AST resolution.
    ///
    /// String annotations that contain non-type expressions (e.g. list literals,
    /// lambda calls, conditional expressions, etc.).
    /// Used by `dataclasses_kwonly`.
    pub invalid_string_annotations: Vec<InvalidStringAnnotation>,
    /// Protocol `Self`-return conformance violations detected during resolution.
    ///
    /// When a class is passed where a `Protocol` with `Self`-returning methods
    /// is expected, but the class's corresponding method returns a different type.
    /// Used by `namedtuples_type_compat`.
    pub protocol_self_violations: Vec<ProtocolSelfViolation>,
    /// Protocol instantiation violations: direct `Proto()` calls or instantiation
    /// of concrete subclasses that fail to implement all required protocol members.
    ///
    /// Used by `protocols_explicit`.
    pub protocol_instantiation_violations: Vec<ProtocolInstantiationViolation>,
    /// Spans of `isinstance(x, T)` calls where `T` is a `TypedDict` class.
    ///
    /// PEP 589: `TypedDict` type objects cannot be used in `isinstance()` tests.
    /// Used by `typeddicts_usage`.
    pub isinstance_typeddict_violations: Vec<Span>,
    /// `TypedDict` subscript key/value violations and invalid dict-literal assignments.
    ///
    /// Covers:
    /// - `td["invalid_key"] = val` where `"invalid_key"` is not a `TypedDict` field.
    /// - `td["field"] = wrong_type_val` where the value type mismatches the field type.
    /// - `var: TypedDict = {invalid/missing keys}` where the literal doesn't match the schema.
    ///   Used by `generics_syntax_declarations`.
    pub typeddict_key_violations: Vec<TypedDictKeyViolation>,
    /// Module-level `TypeAlias` annotated assignments.
    ///
    /// Each entry represents `Name: TypeAlias = expr` at module level.
    /// Used by `generics_defaults_specialization` to check that subscript sites respect the alias arity.
    pub type_alias_defs: Vec<TypeAliasDefInfo>,
    /// Module-level subscript expression sites (`Name[args...]` used as a statement).
    ///
    /// Collected so that rules can check whether user-defined generic types are
    /// subscripted with the correct number of type arguments.
    /// Used by `generics_defaults_specialization`.
    pub generic_subscript_sites: Vec<GenericSubscriptSite>,
    /// Augmented-assignment violations on `Literal`-typed variables.
    ///
    /// Used by `literals_semantics`.
    pub literal_augmented_assign_violations: Vec<LiteralAugmentedAssignViolation>,
    /// Tuple index out-of-bounds violations.
    ///
    /// Used by `tuples_index`.
    pub tuple_index_violations: Vec<TupleIndexViolation>,
    /// Invalid attribute accesses on bounded type variables.
    ///
    /// Used by `generics_syntax_declarations_2`.
    pub bounded_typevar_attr_violations: Vec<BoundedTypeVarAttrViolation>,
    /// Protocol class used where `type[Proto]` is expected.
    ///
    /// Used by `protocols_class_objects`.
    pub protocol_class_object_violations: Vec<ProtocolClassObjectViolation>,
    /// `ClassName(args).__hash__()` calls on non-hashable dataclasses.
    ///
    /// A `@dataclass` with `eq=True` (the default) sets `__hash__` to `None`
    /// unless `frozen=True`, `unsafe_hash=True`, or `__hash__` is defined explicitly.
    /// Calling `.__hash__()` on such an instance is an error.
    ///
    /// Used by `dataclasses_hash`.
    pub unhashable_hash_call_violations: Vec<UnhashableHashCallViolation>,
    /// Protocol `isinstance`/`issubclass` violations.
    ///
    /// Covers:
    /// - `isinstance(x, Proto)` / `issubclass(x, Proto)` where `Proto` is not
    ///   decorated with `@runtime_checkable`.
    /// - `issubclass(x, Proto)` where `Proto` is a data protocol (has attributes).
    ///
    /// Used by `protocols_runtime_checkable`.
    pub protocol_runtime_checkable_violations: Vec<ProtocolRtcViolation>,
    /// Generator-related type violations (invalid return type, yield mismatches).
    ///
    /// Used by `directives_deprecated`.
    pub generator_violations: Vec<GeneratorViolation>,
    /// Unbound type variable usages detected during AST resolution.
    ///
    /// Covers inner class `TypeVar` reuse and function-nested Generic classes
    /// that cannot be detected from the flattened `ResolvedModule` data alone.
    ///
    /// Used by `generics_scoping`.
    pub unbound_typevar_usages: Vec<UnboundTypeVarUsage>,
    /// Symbols imported from other modules during cross-module analysis.
    ///
    /// Populated by the workspace layer after import resolution, not during
    /// the initial `resolve()` pass. Maps import name → external symbol info.
    pub imported_symbols: std::collections::HashMap<String, ExternalSymbol>,
    /// Structured built-in class declarations from the active Typeshed source.
    ///
    /// This is populated alongside imports from the same root-keyed snapshot,
    /// so editor features and diagnostics never consult a compiled hand table
    /// or another generation's `builtins.pyi`.
    pub builtin_classes: std::sync::Arc<std::collections::HashMap<String, super::IndexedStubClass>>,
    /// Public member API of plain-imported modules backed by a user/local stub,
    /// keyed by local binding name (`X` for `import X`).
    ///
    /// Populated by the workspace import-resolution layer (Phase 1: user stubs
    /// only). Consumed by `imports_module_attribute` to flag access to attributes a stub does
    /// not declare. Empty unless populated.
    pub imported_modules: std::collections::HashMap<String, super::ImportedModuleApi>,
    /// AST-derived PEP 695 scoping facts used by `generics_syntax_scoping`.
    ///
    /// Populated from `ruff_python_ast` nodes (never from raw line scanning) so
    /// that docstring/comment/string content is never mistaken for real
    /// `class` / `def` / `type` declarations.
    pub pep695_scoping: super::pep695_scoping::Pep695Scoping,
    /// The source file path.
    pub path: String,
    /// The original source text (forwarded from parser for span restoration).
    pub source: String,
    /// Exact ranges of Python comment tokens used for safe inline suppression.
    pub comment_ranges: Vec<Span>,
    /// Lazily-parsed AST shared by every rule that needs it (parsed at most once
    /// per module rather than once per rule). Excluded from equality — see
    /// [`LazyAst`].
    pub lazy_ast: LazyAst,
}

impl ResolvedModule {
    /// Resolve a statically-known method-call receiver to its active Typeshed
    /// built-in declaration and whether it is a literal receiver.
    #[must_use]
    pub fn builtin_class_for_receiver(
        &self,
        receiver: &super::CallReceiver,
    ) -> Option<(&super::IndexedStubClass, bool)> {
        let (type_name, literal) = match receiver {
            super::CallReceiver::StringLiteral => ("str", true),
            super::CallReceiver::BytesLiteral => ("bytes", true),
            super::CallReceiver::Name(name) => self.builtin_type_of_name(name)?,
        };
        self.builtin_classes
            .get(type_name)
            .map(|declaration| (declaration, literal))
    }

    /// Applicable overload declarations for a bound built-in method call.
    #[must_use]
    pub fn builtin_methods_for_call(
        &self,
        call: &super::CallSite,
    ) -> Vec<&basilisk_stubs::StubFunction> {
        let Some(receiver) = call.receiver.as_ref() else {
            return Vec::new();
        };
        let Some((class, literal_receiver)) = self.builtin_class_for_receiver(receiver) else {
            return Vec::new();
        };
        class
            .declaration
            .methods
            .iter()
            .filter(|method| method.name == call.callee)
            .filter(|method| {
                literal_receiver
                    || method
                        .receiver
                        .as_ref()
                        .and_then(|receiver| receiver.annotation.as_deref())
                        .is_none_or(|annotation| !annotation.contains("LiteralString"))
            })
            .collect()
    }

    fn builtin_type_of_name(&self, name: &str) -> Option<(&str, bool)> {
        if let Some(variable) = self
            .module_vars
            .iter()
            .find(|variable| variable.name == name)
        {
            if let Some(annotation) = variable
                .annotation_span
                .and_then(|span| span.slice_source(&self.source))
            {
                return builtin_annotation(annotation);
            }
            return match variable.rhs_kind {
                super::RhsKind::StrLiteral => Some(("str", true)),
                super::RhsKind::BytesLiteral => Some(("bytes", true)),
                _ => None,
            };
        }
        self.functions
            .iter()
            .flat_map(|function| &function.parameters)
            .find(|parameter| parameter.name == name)
            .and_then(|parameter| parameter.annotation_span)
            .and_then(|span| span.slice_source(&self.source))
            .and_then(builtin_annotation)
    }
}

fn builtin_annotation(annotation: &str) -> Option<(&str, bool)> {
    match annotation.trim() {
        "str" => Some(("str", false)),
        "LiteralString" | "typing.LiteralString" => Some(("str", true)),
        "bytes" => Some(("bytes", false)),
        _ => None,
    }
}
