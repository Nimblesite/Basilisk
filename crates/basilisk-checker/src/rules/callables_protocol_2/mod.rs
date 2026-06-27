//! callables_protocol_2: Callable and Protocol assignment compatibility.
//!
//! Checks that when a function is assigned to a variable annotated with a
//! `Callable` type or a callback `Protocol`, the signatures are compatible.

mod callable;
mod checks;
mod context;
mod protocol;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;
use checks::check_stmts;
use context::ModuleContext;

const CODE: ErrorCode = ErrorCode {
    code: "callables_protocol_2",
    docs_url: "https://www.basilisk-python.dev/errors/callables_protocol_2",
};

/// Emits callables_protocol_2 for incompatible callable/protocol assignments.
pub(crate) struct CallableAssignmentViolation;

impl Rule for CallableAssignmentViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };
        let ctx = ModuleContext::from_ast(&parsed.ast.body);
        check_stmts(&parsed.ast.body, &ctx, &module.path, &CODE, diagnostics);
    }
}
