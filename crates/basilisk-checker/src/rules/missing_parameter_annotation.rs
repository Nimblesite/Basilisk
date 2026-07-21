//! Implements [BSK-0001] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-MISSING
//! BSK-0001: Missing parameter type annotation.
//! Implements the parameter-policy slice of [TYPEINF-REQUIRED] and the
//! receiver exemption shared by [TYPEINF-SPECIAL-SELF].
//!
//! Never fires where the current engine already infers the parameter type: a
//! scalar-literal default (`timeout=30` → `int`) determines the type, so
//! demanding an annotation there would be redundant ([TYPEINF-FUNC-DEFAULTS]).
//! Defaults that do NOT determine the type — `None`, empty containers, calls,
//! lambdas, arbitrary expressions — still require an annotation.
//!
//! ```python
//! def connect(timeout=30):              # ✓ — type inferred as int
//!     pass
//!
//! def connect(retries):                 # BSK-0001 — nothing to infer from
//!     pass
//!
//! def connect(timeout=None):            # BSK-0001 — None does not determine T | None
//!     pass
//! ```

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::inference::rhs_fully_determines_type;

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

// Implements [TYPEINF-FUNC-SELFCLS] — only the first conventional receiver of
// an actual method is exempt. A free function parameter merely named `self` or
// `cls` has no implicit receiver type.
fn check_function(func: &FunctionInfo, path: &str, out: &mut Vec<Diagnostic>) {
    func.parameters
        .iter()
        .enumerate()
        .filter(|(index, p)| {
            !p.has_annotation
                && !is_implicit_receiver(func, *index, p)
                && !default_determines_type(p)
        })
        .for_each(|(_, p)| out.push(make_diagnostic(p, path)));
}

fn is_implicit_receiver(func: &FunctionInfo, index: usize, param: &ParameterInfo) -> bool {
    if index != 0
        || func.class_name.is_none()
        || func.decorators.iter().any(|name| name == "staticmethod")
    {
        return false;
    }
    let class_receiver = func.decorators.iter().any(|name| name == "classmethod")
        || matches!(func.name.as_str(), "__new__" | "__init_subclass__");
    param.name == if class_receiver { "cls" } else { "self" }
}

/// Implements [TYPEINF-FUNC-DEFAULTS]: `true` when the parameter's default
/// alone already tells the current engine the type — the annotation would be
/// redundant, so BSK-0001 must stay silent.
fn default_determines_type(param: &ParameterInfo) -> bool {
    param
        .default_rhs_kind
        .as_ref()
        .is_some_and(rhs_fully_determines_type)
}

fn make_diagnostic(param: &ParameterInfo, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!("Missing parameter type annotation for `{}`", param.name),
        param.name_span,
        path,
        Some(format!("Add a type annotation: `{}: <type>`", param.name)),
        Some(
            "Basilisk requires an explicit parameter type wherever it cannot be inferred; \
             a literal default (e.g. `timeout=30`) infers the type and needs no annotation"
                .to_owned(),
        ),
    )
}
