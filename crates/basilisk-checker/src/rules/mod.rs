//! Type checking rules.
//!
//! Each rule is a zero-size struct implementing [`Rule`]. Rules are
//! registered in [`run_all`] and executed in order against a resolved module.

/// Declare a list of crate-private submodules in one go.
macro_rules! rule_modules {
    ($($name:ident),* $(,)?) => {
        $( pub(crate) mod $name; )*
    };
}

rule_modules!(
    e0001, e0002, e0003, e0004, e0005, e0010, e0011, e0012, e0013, e0014, e0015, e0016, e0017,
    e0018, e0019, e0020, e0021, e0022, e0023, e0024, e0025, e0026, e0027, e0029, e0030, e0031,
    e0032, e0033, e0034, e0035, e0036, e0037, e0038, e0039, e0040, e0041, e0042, e0043, e0044,
    e0045, e0046, e0047, e0048, e0049, e0050, e0051, e0052, e0053, e0054, e0055, e0056, e0057,
    e0058, e0059, e0060, e0061, e0062, e0063, e0064, e0065, e0066, e0067, e0068, e0069, e0070,
    e0071, e0072, e0073, e0074, e0075, e0076, e0077, e0078, e0079, e0080, e0081, e0082, e0083,
    e0084, e0085, e0086, e0087, e0088, e0089, e0090, e0091, e0092, e0093, e0094, e0095, e0096,
    e0097, e0098, e0099, e0100, e0101, e0102, e0103, e0104, e0105, e0106, e0107, e0108, e0109,
    e0110, e0111, e0112, e0113, e0114, e0115, e0116, e0117, e0118, e0119, e0120, e0120_helpers,
    e0121, e0122, e0123, e0124, e0125, e0126, e0126_helpers, e0127, e0128, e0128_helpers, e0129,
    e0130, e0131, e0132, e0133, e0134, e0136, e0137, e0138, e0139, e0140, e0141, e0142, e0143,
    e0144, e0145, e0146, e0147, e0148, e0149, e0150, e0151, guards, shared, w0010, w0011, w0012,
    w0013, w0040, w0050,
);

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

/// A single type checking rule.
pub(crate) trait Rule {
    /// Run the rule against a resolved module and push any diagnostics.
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>);
}

/// All registered Phase 1 rules.
#[expect(
    clippy::too_many_lines,
    reason = "rule registry listing is inherently long"
)]
fn all_rules() -> &'static [&'static dyn Rule] {
    &[
        &e0001::MissingParameterAnnotation,
        &e0002::MissingReturnAnnotation,
        &e0003::MissingVariableType,
        &e0004::MissingVarArgAnnotation,
        &e0005::MissingAttributeAnnotation,
        &e0010::ImportFromUntypedModule,
        &e0011::ReturnTypeMismatch,
        &e0012::ArgumentTypeMismatch,
        &e0013::ReturnTypeMismatch,
        &e0014::AssignmentTypeMismatch,
        &e0015::InvalidTypeArgCount,
        &e0016::IncompatibleOverride,
        &e0017::IncompatibleVariableOverride,
        &e0018::UndefinedVariable,
        &e0019::UnboundVariable,
        &e0020::MissingOverloadImpl,
        &e0021::OverlappingOverloads,
        &e0022::UnhashableDictKey,
        &e0023::NonExhaustiveMatch,
        &e0024::InvalidTypeForm,
        &e0025::MissingOverrideDecorator,
        &e0026::TypeVarSingleConstraint,
        &e0027::DuplicateTypeVarInGeneric,
        &e0029::TypedDictMethodNotAllowed,
        &e0030::NonDefaultAfterDefault,
        &e0031::InvalidCastCall,
        &e0032::InvalidTypedDictBase,
        &e0033::InvalidRevealTypeCall,
        &e0034::FinalViolation,
        &e0035::RequiredNotRequiredContext,
        &e0036::ClassVarInvalidContext,
        &e0037::InvalidTypedDictCall,
        &e0038::InvalidTypedDictInheritance,
        &e0039::InvalidAssertTypeCall,
        &e0040::EnumWithMembersFinal,
        &e0041::TooFewArguments,
        &e0042::Pep695TraditionalTypeVarMix,
        &e0043::NonTypeVarInGeneric,
        &e0044::FinalInvalidPosition,
        &e0045::AnnotatedInvalidFirstArg,
        &e0046::EnumMemberAnnotated,
        &e0047::InvalidTypeAnnotation,
        &e0048::TypeAliasInvalidRhs,
        &e0049::MultipleUnboundedTupleTypes,
        &e0050::InvalidNewType,
        &e0051::InvalidLiteralParam,
        &e0052::FrozenDataclassAssignment,
        &e0053::AssertTypeMismatch,
        &e0054::FinalAnnotationViolation,
        &e0055::TypeVarInvalidKwargs,
        &e0056::ReadOnlyTypedDictMutation,
        &e0057::TypeStatementInvalidRhs,
        &e0058::AnnotatedTooFewArguments,
        &e0059::MatchArgsFalseAccess,
        &e0060::CrossTypeDataclassOrderComparison,
        &e0061::AssertTypeEnumLiteralMismatch,
        &e0062::NoReturnFallThrough,
        &e0063::NonHashableDataclassAssignment,
        &e0064::InvalidNamedTupleCall,
        &e0065::FloatParamIntAttrAccess,
        &e0066::EnumValueTypeMismatch,
        &e0067::EnumNonMemberInLiteral,
        &e0068::LiteralStringEnumMismatch,
        &e0069::DataclassKwOnlyViolation,
        &e0070::NeverTypeCompatibility,
        &e0071::HistoricalPositionalViolation,
        &e0072::NoMatchingOverload,
        &e0073::NamedTupleTupleCompat,
        &e0074::ConstructorCallNewMismatch,
        &e0075::SelfTypeAttributeIncompatible,
        &e0076::OverloadUnionExpansionFailure,
        &e0077::ProtocolSelfViolation,
        &e0078::SelfTypeViolation,
        &e0079::ModuleProtocolIncompatible,
        &e0080::TypeVarBoundViolation,
        &e0081::TypeVarTupleUnpackViolation,
        &e0082::TypeVarTupleCallableMismatch,
        &e0083::TypeVarTupleUnpackRequired,
        &e0084::TypeVarTupleInvalidParams,
        &e0085::TypeVarTupleArgCountMismatch,
        &e0086::MultipleTypeVarTuplesInGeneric,
        &e0088::TypedDictRuntimeViolation,
        &e0089::Pep695InvalidBound,
        &e0090::InvalidTupleTypeSyntax,
        &e0091::TypeVarDefaultIncompatible,
        &e0092::TooFewTypeArguments,
        &e0093::TypedDictKeyValidation,
        &e0094::SelfInvalidLocation,
        &e0095::InitVarViolation,
        &e0096::DataclassFieldDefaultFactoryMismatch,
        &e0097::ProtocolNewSelfAttrViolation,
        &e0098::NonProtocolBaseInProtocol,
        &e0099::ProtocolInstantiation,
        &e0100::LiteralAugmentedAssign,
        &e0101::TypeGuardNoNarrowingParam,
        &e0102::TypeVarDefaultReferential,
        &e0103::TupleIndexOutOfBounds,
        &e0104::CyclicalTypeAliasReference,
        &e0105::BoundedTypeVarAttrAccess,
        &e0106::ProtocolClassObject,
        &e0107::VarianceIncompatibleBase,
        &e0108::DataclassSlotsViolation,
        &e0109::TypeVarBoundCallViolation,
        &e0110::ProtocolVarianceViolation,
        &e0111::ConstructorCallError,
        &e0112::TypeGuardCallableReturnMismatch,
        &e0113::TypeIsInconsistentNarrowing,
        &e0114::ProtocolRuntimeCheckableViolation,
        &e0115::DeprecatedUsage,
        &e0116::NamedTupleDefError,
        &e0117::UnboundTypeVarScope,
        &e0118::SuperAbstractCall,
        &e0119::ProtocolUnsafeOverlap,
        &e0120::GeneratorReturnTypeViolation,
        &e0121::ProtocolAssignmentConformance,
        &e0122::CallableCallSiteViolation,
        &e0123::SuperCallOnAbstractProtocolMethod,
        &e0124::ProtocolTupleElementMismatch,
        &e0125::InstanceAttrOnClass,
        &e0126::LiteralStringAssignment,
        &e0127::TupleIndexOutOfRange,
        &e0128::TypeVarDefaultReferential,
        &e0129::LiteralValueIncompatible,
        &e0130::TypeVarScopeViolation,
        &e0131::GeneratorTypeMismatch,
        &e0132::InconsistentTypeVarOrder,
        &e0133::ProtocolVarianceMismatch,
        &e0134::InvariantGenericArgMismatch,
        &e0136::CallableSubtypingViolation,
        &e0137::GenericProtocolViolation,
        &e0138::DataclassTransformMetaViolation,
        &e0139::TypeVarTupleSpecializationViolation,
        &e0140::CallableAssignmentViolation,
        &e0141::UnpackKwargsViolation,
        &e0142::DataclassTransformClassViolation,
        &e0143::NamedTupleUsageViolation,
        &e0144::TypeCallConstructorViolation,
        &e0145::TypeBracketViolation,
        &e0146::ProtocolClassObjectViolation,
        &e0147::TupleStarredUnpackCompatibility,
        &e0148::GenericTypeArgViolation,
        &e0149::Pep695TypeParamScopingViolation,
        &e0150::DeadBranchVariable,
        &e0151::TypeAliasTypeViolation,
        &w0010::MissingTypeStubs,
        &w0011::UndeclaredDependencyImport,
        &w0012::UnusedDependency,
        &w0013::StaleLockFile,
        &w0040::LambdaMissingAnnotations,
        &w0050::RedundantAnnotationWarning,
    ]
}

/// Run all registered Phase 1 rules against a resolved module.
#[must_use]
pub fn run_all(module: &ResolvedModule) -> Vec<Diagnostic> {
    all_rules().iter().fold(Vec::new(), |mut acc, rule| {
        rule.check(module, &mut acc);
        acc
    })
}
