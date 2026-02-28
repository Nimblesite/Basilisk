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
pub(crate) mod guards;

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
        &e0011::ImplicitAny,
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
    ];

    rules.iter().fold(Vec::new(), |mut acc, rule| {
        rule.check(module, &mut acc);
        acc
    })
}
