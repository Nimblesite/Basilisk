//! BSK-E0055: Invalid `TypeVar` / `TypeVarTuple` / `ParamSpec` keyword argument combination.
//!
//! PEP 484 / PEP 695 forbid certain combinations of keyword arguments in
//! `TypeVar(...)` calls, and PEP 646 / PEP 612 restrict what kwargs
//! `TypeVarTuple` and `ParamSpec` accept:
//!
//! 1. `covariant=True` and `contravariant=True` together — a `TypeVar` cannot be
//!    both covariant and contravariant.
//! 2. `infer_variance=True` with `covariant=True` or `contravariant=True` —
//!    when variance is inferred, the explicit flags are redundant and disallowed.
//! 3. Constraints (2+ positional type args) combined with `bound=` — a `TypeVar`
//!    may have one or the other, but not both.
//! 4. `TypeVarTuple` and `ParamSpec` do not support `covariant`, `contravariant`,
//!    `bound`, or type constraint arguments.
//!
//! ```python
//! from typing import TypeVar, TypeVarTuple
//! T1 = TypeVar("T1", covariant=True, contravariant=True)        # E
//! T2 = TypeVar("T2", covariant=True, infer_variance=True)       # E
//! T3 = TypeVar("T3", str, int, bound="int")                     # E
//! Ts = TypeVarTuple("Ts", covariant=True)                       # E
//! Ts2 = TypeVarTuple("Ts2", int, float)                         # E
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0055",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0055",
};

fn make_diag(msg: &str, span: basilisk_resolver::Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: msg.to_owned(),
        span,
        path: path.to_owned(),
        help: Some(
            "TypeVar variance flags are mutually exclusive: use at most one of \
             covariant/contravariant/infer_variance, and not both constraints and bound"
                .to_owned(),
        ),
        note: Some(
            "PEP 484: TypeVar cannot be both covariant and contravariant; \
             PEP 695: infer_variance is incompatible with explicit variance flags"
                .to_owned(),
        ),
    }
}

/// Emits BSK-E0055 for invalid `TypeVar` / `TypeVarTuple` / `ParamSpec` keyword combinations.
pub(crate) struct TypeVarInvalidKwargs;

impl Rule for TypeVarInvalidKwargs {
    #[allow(clippy::too_many_lines)]
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let path = &module.path;
        for tv in &module.typevar_calls {
            // TypeVarTuple and ParamSpec do not support variance flags, bound, or constraints.
            if tv.is_typevartuple {
                let kind = "TypeVarTuple";
                if tv.is_covariant || tv.is_contravariant {
                    diagnostics.push(make_diag(
                        &format!(
                            "`{kind}` `{}` does not support `covariant` or `contravariant` arguments",
                            tv.name
                        ),
                        tv.span,
                        path,
                    ));
                }
                if tv.has_bound {
                    diagnostics.push(make_diag(
                        &format!(
                            "`{kind}` `{}` does not support a `bound=` argument",
                            tv.name
                        ),
                        tv.span,
                        path,
                    ));
                }
                if tv.constraint_count > 0 {
                    diagnostics.push(make_diag(
                        &format!(
                            "`{kind}` `{}` does not accept type constraint arguments",
                            tv.name
                        ),
                        tv.span,
                        path,
                    ));
                }
                continue;
            }

            if tv.is_paramspec {
                let kind = "ParamSpec";
                if tv.is_covariant || tv.is_contravariant {
                    diagnostics.push(make_diag(
                        &format!(
                            "`{kind}` `{}` does not support `covariant` or `contravariant` arguments",
                            tv.name
                        ),
                        tv.span,
                        path,
                    ));
                }
                if tv.has_bound {
                    diagnostics.push(make_diag(
                        &format!(
                            "`{kind}` `{}` does not support a `bound=` argument",
                            tv.name
                        ),
                        tv.span,
                        path,
                    ));
                }
                if tv.constraint_count > 0 {
                    diagnostics.push(make_diag(
                        &format!(
                            "`{kind}` `{}` does not accept type constraint arguments",
                            tv.name
                        ),
                        tv.span,
                        path,
                    ));
                }
                continue;
            }

            // Plain TypeVar checks below.

            // covariant=True + contravariant=True
            if tv.is_covariant && tv.is_contravariant {
                diagnostics.push(make_diag(
                    &format!(
                        "`TypeVar` `{}` cannot be both covariant and contravariant",
                        tv.name
                    ),
                    tv.span,
                    path,
                ));
            }
            // infer_variance + covariant or contravariant
            if tv.has_infer_variance && (tv.is_covariant || tv.is_contravariant) {
                diagnostics.push(make_diag(
                    &format!(
                        "`TypeVar` `{}` cannot use `infer_variance=True` with explicit variance flags",
                        tv.name
                    ),
                    tv.span,
                    path,
                ));
            }
            // constraints + bound
            if tv.constraint_count >= 2 && tv.has_bound {
                diagnostics.push(make_diag(
                    &format!(
                        "`TypeVar` `{}` cannot have both type constraints and an upper bound",
                        tv.name
                    ),
                    tv.span,
                    path,
                ));
            }
        }
    }
}
