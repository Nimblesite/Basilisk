//! BSK-E0013: Return type mismatch — `-> None` with a valued `return`.
//!
//! When a function is annotated `-> None` it must not contain any `return`
//! statement that carries a value.  A bare `return` (or no `return` at all) is
//! fine; `return <expr>` is not.

use basilisk_resolver::{FunctionInfo, ResolvedModule, ReturnAnnotationKind, ReturnStmtInfo};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0013",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0013",
};

/// Emits BSK-E0013 for every `-> None` function that contains a valued return.
pub(crate) struct ReturnTypeMismatch;

impl Rule for ReturnTypeMismatch {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .functions
            .iter()
            .filter(|func| func.return_annotation == ReturnAnnotationKind::NoneType)
            .for_each(|func| check_function(func, &module.path, diagnostics));
    }
}

fn check_function(func: &FunctionInfo, path: &str, out: &mut Vec<Diagnostic>) {
    func.return_stmts
        .iter()
        .filter(|stmt| stmt.has_value)
        .for_each(|stmt| out.push(make_diagnostic(stmt, &func.name, path)));
}

fn make_diagnostic(stmt: &ReturnStmtInfo, func_name: &str, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Function `{func_name}` is annotated `-> None` but has a `return` statement with a value"
        ),
        span: stmt.span,
        path: path.to_owned(),
        help: Some(
            "Either remove the return value or change the return type annotation".to_owned(),
        ),
        note: Some(
            "A function annotated `-> None` must only use bare `return` or fall off the end"
                .to_owned(),
        ),
    }
}
