//! BSK-E0140: Callable and Protocol assignment compatibility.
//!
//! Checks that when a function is assigned to a variable annotated with a
//! `Callable` type or a callback `Protocol`, the signatures are compatible.

mod checks;
mod types;

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode};

use super::Rule;
use checks::check_stmts;
use types::ModuleContext;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0140",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0140",
};

/// Emits BSK-E0140 for incompatible callable/protocol assignments.
pub(crate) struct CallableAssignmentViolation;

impl Rule for CallableAssignmentViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };
        let ctx = ModuleContext::from_ast(&parsed.ast.body);
        check_stmts(&parsed.ast.body, &ctx, &module.path, &CODE, diagnostics);
    }
}
