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
pub(crate) mod e0013;
pub(crate) mod e0014;
pub(crate) mod e0015;
pub(crate) mod e0020;
pub(crate) mod e0021;
pub(crate) mod e0023;
pub(crate) mod e0024;
pub(crate) mod e0025;

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
        &e0013::ReturnTypeMismatch,
        &e0014::AssignmentTypeMismatch,
        &e0015::InvalidTypeArgCount,
        &e0020::MissingOverloadImpl,
        &e0021::OverlappingOverloads,
        &e0023::NonExhaustiveMatch,
        &e0024::InvalidTypeForm,
        &e0025::MissingOverrideDecorator,
    ];

    rules.iter().fold(Vec::new(), |mut acc, rule| {
        rule.check(module, &mut acc);
        acc
    })
}
