//! Scope and function information types produced by the resolver.

mod class_types;
mod external_symbol;
mod function_types;
mod import_types;
mod module_types;
pub(crate) mod narrowing_types;
mod resolved_module;
mod rhs;
mod span;
mod variable_types;
mod violations;

pub use class_types::{BaseSubscriptEntry, ClassInfo, GenericParamInfo, TypeArg};
pub use external_symbol::{ExternalSymbol, ExternalSymbolKind};
pub use function_types::{
    FunctionInfo, ParameterInfo, ReturnAnnotationKind, ReturnStmtInfo, YieldExprInfo,
};
pub use import_types::{ImportInfo, ImportKind, ImportResolution, PackageDepKind, UnresolvedReason};
pub use module_types::{
    AnnotatedTooFewArgs, AssertTypeCallInfo, CallSite, CompareOp, FloatParamIntAttrAccess,
    GenericSubscriptSite, LiteralStringEnumMismatch, MatchStmtInfo, ModuleAttrAccessInfo,
    ModuleAttrAssignment, ModuleBareAssignment, ModuleOrderComparisonInfo, NamedTupleDefInfo,
    NewTypeCallInfo, RevealTypeCallInfo, RhsStringRef, TypeAliasDefInfo, TypeAliasTypeCallInfo,
    TypeStatementInfo, TypeVarCallInfo, TypedDictCallInfo, TypedDictKeyViolation,
    TypedDictKeyViolationKind, TypedDictSecondArgKind, UnhashableHashCallViolation,
    UnhashableKeyRef,
};
pub use narrowing_types::{MatchCaseNarrowing, NarrowingGuard, NarrowingGuardKind};
pub use resolved_module::ResolvedModule;
pub use rhs::RhsKind;
pub use span::Span;
pub use variable_types::{AttributeInfo, VariableInfo};
pub use violations::{
    BoundedTypeVarAttrViolation, EnumValueTypeViolationInfo, EnumValueTypeViolationKind,
    FinalViolationInfo, FinalViolationKind, GeneratorViolation, GeneratorViolationKind,
    HistoricalPositionalViolation, HistoricalPositionalViolationKind, InvalidStringAnnotation,
    InvalidStringAnnotationKind, LiteralAugmentedAssignViolation, LocalClassVarViolation,
    Pep695BoundViolation, Pep695BoundViolationKind, ProtocolClassObjectViolation,
    ProtocolInstantiationViolation, ProtocolRtcViolation, ProtocolRtcViolationKind,
    ProtocolSelfViolation, ReadOnlyViolationInfo, ReadOnlyViolationKind, TupleIndexViolation,
    UnboundTypeVarUsage,
};
