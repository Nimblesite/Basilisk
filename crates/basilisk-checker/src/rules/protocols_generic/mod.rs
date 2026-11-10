//! `protocols_generic`: Generic protocol violations.
//!
//! PEP 544: <https://typing.readthedocs.io/en/latest/spec/protocol.html#generic-protocols>

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;

#[expect(
    dead_code,
    reason = "rule is registered but INERT: its text-matched verdict path was deleted under [ASTREBUILD-LAW] and no diagnostic is emitted until the semantic rebuild ([ASTREBUILD-PHASE-RESOLVER])"
)]
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
