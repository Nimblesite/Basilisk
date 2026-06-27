//! Implements [generics_basic] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! generics_basic: `TypeVar` declared with exactly one constraint.
//!
//! PEP 484 requires a `TypeVar` to have either zero constraints (unconstrained)
//! or two or more constraints.  A single constraint makes no sense because it
//! would be equivalent to using the type directly.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diag_help_note, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "generics_basic",
    docs_url: "https://www.basilisk-python.dev/errors/generics_basic",
};

/// Emits generics_basic when a `TypeVar` is declared with exactly one constraint,
/// or when it has both constraints and a `bound=` keyword argument.
pub(crate) struct TypeVarSingleConstraint;

impl Rule for TypeVarSingleConstraint {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for tv in &module.typevar_calls {
            check_typevar_constraints(tv, module, diagnostics);
        }
    }
}

fn check_typevar_constraints(
    tv: &basilisk_resolver::TypeVarCallInfo,
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Name inconsistency: variable name must match the string argument.
    if let Some(ref string_name) = tv.string_name {
        if string_name != &tv.name {
            let kind = if tv.is_typevartuple {
                "TypeVarTuple"
            } else if tv.is_paramspec {
                "ParamSpec"
            } else {
                "TypeVar"
            };
            diagnostics.push(error_diag_help_note(
                CODE.clone(),
                format!(
                    "Variable name `{}` does not match the name string `{string_name}` \
                     passed to `{kind}`",
                    tv.name,
                ),
                tv.span,
                &module.path,
                format!(
                    "Rename the variable to `{string_name}` or change the name string \
                     to `\"{}\"`",
                    tv.name
                ),
                "PEP 484: the variable name and the string argument must match",
            ));
        }
    }

    if tv.constraint_count == 1 {
        diagnostics.push(error_diag_help_note(
            CODE.clone(),
            format!(
                "`{}` has a single constraint; TypeVar requires 0 or 2+ constraints",
                tv.name
            ),
            tv.span,
            &module.path,
            "Add a second constraint or remove the single constraint".to_owned(),
            "PEP 484: a TypeVar with one constraint is invalid",
        ));
    }
    // Cannot specify both constraints and a bound.
    if tv.constraint_count >= 2 && tv.has_bound {
        diagnostics.push(error_diag_help_note(
            CODE.clone(),
            format!(
                "`{}` specifies both constraints and `bound=`; these are mutually exclusive",
                tv.name
            ),
            tv.span,
            &module.path,
            "Use either constraints (positional type args) or `bound=`, not both".to_owned(),
            "PEP 484: `TypeVar` cannot have both constraints and a `bound`",
        ));
    }
    // Constraint must not itself be parameterized by a TypeVar (e.g. `list[T]`).
    if tv.has_parameterized_constraint && tv.constraint_count >= 2 {
        diagnostics.push(error_diag_help_note(
            CODE.clone(),
            format!(
                "`{}` has a constraint that is parameterized by a type variable",
                tv.name
            ),
            tv.span,
            &module.path,
            "TypeVar constraints must be plain types, not generic types".to_owned(),
            "PEP 484: TypeVar constraints cannot themselves be parameterized by type variables",
        ));
    }
    // Bound must not be parameterized by a TypeVar (e.g. `bound=list[T]`).
    if tv.has_parameterized_bound {
        diagnostics.push(error_diag_help_note(
            CODE.clone(),
            format!(
                "`{}` has a `bound=` that is parameterized by a type variable",
                tv.name
            ),
            tv.span,
            &module.path,
            "The `bound=` argument must be a plain type, not a generic type".to_owned(),
            "PEP 484: TypeVar bound cannot itself be parameterized by a type variable",
        ));
    }
}
