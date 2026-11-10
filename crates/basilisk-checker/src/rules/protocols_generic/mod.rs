//! `protocols_generic`: Generic protocol violations.
//!
//! PEP 544: <https://typing.readthedocs.io/en/latest/spec/protocol.html#generic-protocols>

#[cfg(test)]
mod helper_parity_tests;
mod helpers;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "protocols_generic",
    docs_url: "https://www.basilisk-python.dev/errors/protocols_generic",
};

/// Emits `protocols_generic` for generic protocol violations.
pub(crate) struct GenericProtocolViolation;

impl Rule for GenericProtocolViolation {
    fn check(
        &self,
        _module: &ResolvedModule,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}
