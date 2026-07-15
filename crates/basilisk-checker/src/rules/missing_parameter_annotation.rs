//! Implements [BSK-0001] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-missing
//! BSK-0001: Missing parameter type annotation.

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-0001",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-0001",
};

/// Emits BSK-0001 for every unannotated regular parameter (not `*args`/`**kwargs`).
///
/// `*args` and `**kwargs` are handled by [`super::missing_vararg_annotation`].
/// Skipped for `@overload`, `@abstractmethod`, and `Protocol` methods.
// Implements [TYPEINF-FUNC-PARAMS] / [TYPEINF-EXCEEDS-REQUIRED] — every missing
// parameter annotation is a diagnostic, never a silent inference.
pub(crate) struct MissingParameterAnnotation;

impl Rule for MissingParameterAnnotation {
    fn opt_in_spec(&self) -> Option<crate::rule_tags::OptInSpec> {
        Some(crate::rule_tags::OptInSpec {
            code: CODE.code,
            tags: &["strictness"],
        })
    }

    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        module
            .functions
            .iter()
            .filter(|func| !is_stub_context(func, &module.classes))
            .for_each(|func| check_function(func, &module.path, diagnostics));
    }
}

// Implements [TYPEINF-FUNC-SELFCLS] — the self/cls exemption side: `self` and
// `cls` are never required to be annotated (their types are implicit). Note: the
// spec's full Self / type[Self] inference is not modelled here.
fn check_function(func: &FunctionInfo, path: &str, out: &mut Vec<Diagnostic>) {
    func.parameters
        .iter()
        .filter(|p| !p.has_annotation && p.name != "self" && p.name != "cls")
        .for_each(|p| out.push(make_diagnostic(p, path)));
}

fn make_diagnostic(param: &ParameterInfo, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("Missing parameter type annotation for `{}`", param.name),
        param.name_span,
        path,
        Some(format!("Add a type annotation: `{}: <type>`", param.name)),
        Some("In Basilisk, all function parameters require explicit types".to_owned()),
    )
}
