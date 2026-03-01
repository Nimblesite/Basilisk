//! Type checking rules.
//!
//! Each rule is a zero-size struct implementing [`Rule`]. Rules are
//! registered in [`run_all`] and executed in order against a resolved module.

pub(crate) mod e0001;
pub(crate) mod e0002;
pub(crate) mod e0003;
pub(crate) mod e0004;
pub(crate) mod e0005;
pub(crate) mod e0010;
pub(crate) mod e0011;
pub(crate) mod e0012;
pub(crate) mod e0013;
pub(crate) mod e0014;
pub(crate) mod e0015;
pub(crate) mod e0016;
pub(crate) mod e0017;
pub(crate) mod e0018;
pub(crate) mod e0019;
pub(crate) mod e0020;
pub(crate) mod e0021;
pub(crate) mod e0022;
pub(crate) mod e0023;
pub(crate) mod e0024;
pub(crate) mod e0025;
pub(crate) mod e0026;
pub(crate) mod e0027;
pub(crate) mod e0029;
pub(crate) mod e0030;
pub(crate) mod e0031;
pub(crate) mod e0032;
pub(crate) mod e0033;
pub(crate) mod e0034;
pub(crate) mod e0035;
pub(crate) mod e0036;
pub(crate) mod e0037;
pub(crate) mod e0038;
pub(crate) mod e0039;
pub(crate) mod e0040;
pub(crate) mod e0041;
pub(crate) mod e0042;
pub(crate) mod e0043;
pub(crate) mod e0044;
pub(crate) mod e0045;
pub(crate) mod e0046;
pub(crate) mod e0047;
pub(crate) mod e0048;
pub(crate) mod e0049;
pub(crate) mod e0050;
pub(crate) mod e0051;
pub(crate) mod e0052;
pub(crate) mod e0053;
pub(crate) mod e0054;
pub(crate) mod e0055;
pub(crate) mod e0056;
pub(crate) mod e0057;
pub(crate) mod e0058;
pub(crate) mod e0059;
pub(crate) mod e0060;
pub(crate) mod e0061;
pub(crate) mod e0062;
pub(crate) mod e0063;
pub(crate) mod e0064;
pub(crate) mod e0065;
pub(crate) mod e0066;
pub(crate) mod e0067;
pub(crate) mod e0068;
pub(crate) mod e0069;
pub(crate) mod e0070;
pub(crate) mod e0071;
pub(crate) mod e0072;
pub(crate) mod e0073;
pub(crate) mod e0074;
pub(crate) mod e0075;
pub(crate) mod e0076;
pub(crate) mod e0077;
pub(crate) mod e0078;
pub(crate) mod e0079;
pub(crate) mod e0080;
pub(crate) mod e0081;
pub(crate) mod e0082;
pub(crate) mod e0083;
pub(crate) mod e0084;
pub(crate) mod e0085;
pub(crate) mod e0086;
pub(crate) mod e0087;
pub(crate) mod e0088;
pub(crate) mod e0089;
pub(crate) mod e0090;
pub(crate) mod e0091;
pub(crate) mod e0092;
pub(crate) mod e0093;
pub(crate) mod e0094;
pub(crate) mod e0095;
pub(crate) mod e0096;
pub(crate) mod guards;
pub(crate) mod w0040;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

/// A single type checking rule.
pub(crate) trait Rule {
    /// Run the rule against a resolved module and push any diagnostics.
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>);
}

/// Run all registered Phase 1 rules against a resolved module.
#[must_use]
pub fn run_all(module: &ResolvedModule) -> Vec<Diagnostic> {
    let rules: &[&dyn Rule] = &[
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
        &e0087::Pep695InvalidBound,
        &e0088::TypedDictRuntimeViolation,
        &e0089::Pep695InvalidBound,
        &e0090::InvalidTupleTypeSyntax,
        &e0091::TypeVarDefaultIncompatible,
        &e0092::TooFewTypeArguments,
        &e0093::TypedDictKeyValidation,
        &e0094::SelfInvalidLocation,
        &e0095::InitVarViolation,
        &e0096::DataclassFieldDefaultFactoryMismatch,
        &w0040::LambdaMissingAnnotations,
    ];

    rules.iter().fold(Vec::new(), |mut acc, rule| {
        rule.check(module, &mut acc);
        acc
    })
}
