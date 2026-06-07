//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Name resolution and scope analysis for Basilisk.
//!
//! The resolver walks the parsed AST and produces a [`ResolvedModule`]
//! containing structured information about every function definition,
//! class, import, and module-level variable.  The checker operates on
//! [`ResolvedModule`] without touching the raw AST.

mod bounded_typevar;
mod ident;
pub mod scope;
mod visitor;

pub use ident::{is_simple_ascii_python_identifier, is_simple_python_identifier};
pub use visitor::walks::{
    is_name_or_attr_named, iter_all_params, visit_calls, walk_all_stmts, walk_function_stmts,
};

pub use scope::{
    class_by_name, collect_name_set, collect_name_set_where, collect_names, collect_names_where,
    has_extra_items_transitive, is_transitive_typeddict, name_lookup, strip_typeddict_qualifiers,
    transitive_typeddict_names, AnnotatedTooFewArgs, AssertTypeCallInfo, AttrAccess, AttributeInfo,
    BaseSubscriptEntry, BoundedTypeVarAttrViolation, CallSite, ClassInfo, CompareOp, DecoratorRef,
    EnumValueTypeViolationInfo, EnumValueTypeViolationKind, FinalViolationInfo, FinalViolationKind,
    FloatParamIntAttrAccess, FunctionInfo, GenericDefKind, GenericParamInfo,
    HistoricalPositionalViolation, HistoricalPositionalViolationKind, ImportInfo, ImportKind,
    ImportResolution, ImportedModuleApi, InvalidStringAnnotation, InvalidStringAnnotationKind,
    LiteralAugmentedAssignViolation, LiteralStringEnumMismatch, LocalClassVarViolation,
    MatchCaseNarrowing, MatchStmtInfo, ModuleAttrAccessInfo, ModuleAttrAssignment,
    ModuleBareAssignment, ModuleOrderComparisonInfo, Named, NamedTupleDefInfo, NarrowingGuard,
    NarrowingGuardKind, NewTypeCallInfo, PackageDepKind, ParameterInfo, Pep695AliasDef,
    Pep695BoundViolation, Pep695BoundViolationKind, Pep695Def, Pep695Param, Pep695ParamKind,
    Pep695Scoping, ProtocolClassObjectViolation, ProtocolInstantiationViolation,
    ProtocolRtcViolation, ProtocolRtcViolationKind, ProtocolSelfViolation, ReadOnlyViolationInfo,
    ReadOnlyViolationKind, ResolvedModule, ReturnAnnotationKind, ReturnStmtInfo,
    RevealTypeCallInfo, RhsKind, RhsStringRef, Span, TupleIndexViolation, TypeAliasDefInfo,
    TypeAliasTypeCallInfo, TypeAliasTypeViolation, TypeAliasTypeViolationKind, TypeArg,
    TypeStatementInfo, TypeVarCallInfo, TypedDictCallInfo, TypedDictKeyViolation,
    TypedDictKeyViolationKind, TypedDictSecondArgKind, UnboundTypeVarUsage,
    UnhashableHashCallViolation, UnhashableKeyRef, UnresolvedReason, VariableInfo, YieldExprInfo,
};

use basilisk_parser::ParsedModule;

/// Errors produced during resolution.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    /// A resolution invariant was violated (reserved for future phases).
    #[error("internal resolve error: {0}")]
    Internal(String),
}

/// Resolve all definitions in a parsed module.
///
/// Returns a [`ResolvedModule`] describing every function, class, import,
/// and module-level variable, together with annotation completeness data.
///
/// # Errors
///
/// Currently infallible in Phase 1; future phases may add import resolution
/// errors.
pub fn resolve(module: &ParsedModule) -> Result<ResolvedModule, ResolveError> {
    Ok(visitor::collect(module))
}
