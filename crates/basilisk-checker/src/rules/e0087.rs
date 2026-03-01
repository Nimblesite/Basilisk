//! BSK-E0087: Reserved for future PEP 695 type parameter checks.
//!
//! PEP 695 type parameter bound and constraint validation is handled by BSK-E0089.
//! This module is reserved for any future distinct PEP 695 violations.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::Diagnostic;

use super::Rule;

/// Placeholder for future PEP 695 type parameter checks.
pub(crate) struct Pep695InvalidBound;

impl Rule for Pep695InvalidBound {
    fn check(&self, _module: &ResolvedModule, _diagnostics: &mut Vec<Diagnostic>) {
        // PEP 695 bound violations are handled by BSK-E0089.
    }
}
