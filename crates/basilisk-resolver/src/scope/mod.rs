//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Scope and function information types produced by the resolver.

mod class_types;
mod external_symbol;
mod function_types;
mod import_types;
mod module_types;
mod named;
pub(crate) mod narrowing_types;
mod pep695_scoping;
mod resolved_module;
mod rhs;
mod span;
mod typeddict_meta;
mod variable_types;
mod violations;

pub use class_types::{BaseSubscriptEntry, ClassInfo, GenericParamInfo, TypeArg};
pub use external_symbol::{ExternalMethod, ExternalSymbol, ExternalSymbolKind, IndexedStubClass};
pub use function_types::{
    FunctionInfo, ParameterInfo, ReturnAnnotationKind, ReturnStmtInfo, YieldExprInfo,
};
pub use import_types::{
    ImportInfo, ImportKind, ImportResolution, ImportedModuleApi, PackageDepKind, UnresolvedReason,
};
pub use module_types::{
    AnnotatedTooFewArgs, AssertTypeCallInfo, CallReceiver, CallSite, CompareOp,
    FloatParamIntAttrAccess, GenericSubscriptSite, LiteralStringEnumMismatch, MatchStmtInfo,
    ModuleAttrAccessInfo, ModuleAttrAssignment, ModuleBareAssignment, ModuleOrderComparisonInfo,
    NamedTupleDefInfo, NewTypeCallInfo, RevealTypeCallInfo, RhsStringRef, TypeAliasDefInfo,
    TypeAliasTypeCallInfo, TypeStatementInfo, TypeVarCallInfo, TypedDictCallInfo,
    TypedDictKeyViolation, TypedDictKeyViolationKind, TypedDictSecondArgKind,
    UnhashableHashCallViolation, UnhashableKeyRef,
};
pub use named::{
    collect_name_set, collect_name_set_where, collect_names, collect_names_where, name_lookup,
    Named,
};
pub use narrowing_types::{MatchCaseNarrowing, NarrowingGuard, NarrowingGuardKind};
pub use pep695_scoping::{
    AttrAccess, DecoratorRef, GenericDefKind, Pep695AliasDef, Pep695Def, Pep695Param,
    Pep695ParamKind, Pep695Scoping,
};
pub use resolved_module::{LazyAst, ModuleBindings, ResolvedModule};
pub use rhs::RhsKind;
pub use span::Span;
pub use typeddict_meta::{
    class_by_name, has_extra_items_transitive, is_transitive_typeddict, transitive_typeddict_names,
};
pub use variable_types::{AttributeInfo, DescriptorKind, VariableInfo};
pub use violations::{
    BoundedTypeVarAttrViolation, EnumValueTypeViolationInfo, EnumValueTypeViolationKind,
    FinalViolationInfo, FinalViolationKind, GeneratorViolation, GeneratorViolationKind,
    HistoricalPositionalViolation, HistoricalPositionalViolationKind, InvalidStringAnnotation,
    InvalidStringAnnotationKind, LiteralAugmentedAssignViolation, LocalClassVarViolation,
    Pep695BoundViolation, Pep695BoundViolationKind, ProtocolClassObjectViolation,
    ProtocolInstantiationViolation, ProtocolRtcViolation, ProtocolRtcViolationKind,
    ProtocolSelfViolation, ReadOnlyViolationInfo, ReadOnlyViolationKind, TupleIndexViolation,
    TypeAliasTypeViolation, TypeAliasTypeViolationKind, UnboundTypeVarUsage,
};
