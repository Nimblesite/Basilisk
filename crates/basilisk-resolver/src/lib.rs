//! Name resolution and scope analysis for Basilisk.
//!
//! The resolver walks the parsed AST and produces a [`ResolvedModule`]
//! containing structured information about every function definition,
//! class, import, and module-level variable.  The checker operates on
//! [`ResolvedModule`] without touching the raw AST.

pub mod scope;
mod visitor;

pub use scope::{
    AnnotatedTooFewArgs, AssertTypeCallInfo, AttributeInfo, BaseSubscriptEntry,
    BoundedTypeVarAttrViolation, CallSite, ClassInfo, CompareOp,
    EnumValueTypeViolationInfo, EnumValueTypeViolationKind, FinalViolationInfo, FinalViolationKind,
    FloatParamIntAttrAccess, FunctionInfo, GenericParamInfo, HistoricalPositionalViolation,
    HistoricalPositionalViolationKind, ImportInfo, ImportKind, InvalidStringAnnotation,
    InvalidStringAnnotationKind, LiteralAugmentedAssignViolation, LiteralStringEnumMismatch,
    LocalClassVarViolation, MatchStmtInfo,
    ModuleAttrAccessInfo, ModuleAttrAssignment, ModuleBareAssignment, ModuleOrderComparisonInfo,
    NamedTupleDefInfo, NewTypeCallInfo, ParameterInfo, Pep695BoundViolation,
    ProtocolClassObjectViolation, ProtocolInstantiationViolation, ProtocolRtcViolation,
    ProtocolRtcViolationKind, ProtocolSelfViolation,
    Pep695BoundViolationKind, ReadOnlyViolationInfo, ReadOnlyViolationKind, ResolvedModule,
    ReturnAnnotationKind, ReturnStmtInfo, RevealTypeCallInfo, RhsKind, RhsStringRef, Span,
    TupleIndexViolation, TypeAliasDefInfo,
    TypeAliasTypeCallInfo, TypeArg, TypeStatementInfo, TypeVarCallInfo, TypedDictCallInfo,
    TypedDictKeyViolation, TypedDictKeyViolationKind, TypedDictSecondArgKind,
    UnhashableHashCallViolation, UnhashableKeyRef, VariableInfo,
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