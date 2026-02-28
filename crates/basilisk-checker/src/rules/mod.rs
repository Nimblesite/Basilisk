//! Type checking rules.
//!
//! Each rule is a zero-size struct implementing [`Rule`]. Rules are
//! registered in [`run_all`] and executed in order against a resolved module.

pub(crate) mod e0001;
pub(crate) mod e0002;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

/// A single type checking rule.
pub(crate) trait Rule {
    /// Run the rule against a resolved module and push any diagnostics.
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>);
}

/// Run all registered Phase 1 rules against a resolved module.
pub fn run_all(module: &ResolvedModule) -> Vec<Diagnostic> {
    let rules: &[&dyn Rule] = &[
        &e0001::MissingParameterAnnotation,
        &e0002::MissingReturnAnnotation,
    ];

    rules.iter().fold(Vec::new(), |mut acc, rule| {
        rule.check(module, &mut acc);
        acc
    })
}
