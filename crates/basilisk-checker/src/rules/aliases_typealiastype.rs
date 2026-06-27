//! Implements [`aliases_typealiastype`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! `aliases_typealiastype`: Invalid `TypeAliasType(...)` call.
//!
//! Detects violations in `TypeAliasType(...)` calls:
//!
//! 1. **Invalid type expression**: The value argument is not a valid type form
//!    (e.g. a list literal, dict literal, lambda, conditional expression).
//!
//! 2. **Circular reference**: The alias value references itself directly or
//!    through a forward-reference string.
//!
//! 3. **Undeclared type variable**: A `TypeVar` / `ParamSpec` / `TypeVarTuple`
//!    used in the value is not listed in `type_params`.
//!
//! 4. **Non-literal `type_params`**: The `type_params` keyword argument is not
//!    a literal tuple expression.
//!
//! ```python
//! from typing import TypeAliasType, TypeVar
//!
//! T = TypeVar("T")
//! S = TypeVar("S")
//!
//! Bad1 = TypeAliasType("Bad1", [int, str])        # E: list is not a type expression
//! Bad2 = TypeAliasType("Bad2", "Bad2")             # E: circular reference
//! Bad3 = TypeAliasType("Bad3", list[S], type_params=(T,))  # E: S not in type_params
//! Bad4 = TypeAliasType("Bad4", int, type_params=my_tuple)  # E: not a literal tuple
//! ```

use basilisk_resolver::{ResolvedModule, TypeAliasTypeViolationKind};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "aliases_typealiastype",
    docs_url: "https://www.basilisk-python.dev/errors/aliases_typealiastype",
};

/// Emits `aliases_typealiastype` for invalid `TypeAliasType(...)` calls.
pub(crate) struct TypeAliasTypeViolation;

impl Rule for TypeAliasTypeViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for violation in &module.type_alias_type_violations {
            let (message, help) = match &violation.kind {
                TypeAliasTypeViolationKind::InvalidTypeExpression => (
                    format!(
                        "Invalid type expression in `TypeAliasType` for `{}`",
                        violation.alias_name
                    ),
                    "The value argument to `TypeAliasType` must be a valid type \
                     expression (e.g. `int`, `list[str]`, `T | None`)"
                        .to_owned(),
                ),
                TypeAliasTypeViolationKind::CircularReference => (
                    format!(
                        "Type alias `{}` references itself, creating a circular dependency",
                        violation.alias_name
                    ),
                    "Remove the self-reference from the type alias value".to_owned(),
                ),
                TypeAliasTypeViolationKind::UndeclaredTypeVar { typevar_name } => (
                    format!(
                        "Type variable `{typevar_name}` used in `{}` is not declared \
                         in `type_params`",
                        violation.alias_name
                    ),
                    format!(
                        "Add `{typevar_name}` to the `type_params` tuple, e.g. \
                         `type_params=(..., {typevar_name})`"
                    ),
                ),
                TypeAliasTypeViolationKind::NonLiteralTypeParams => (
                    format!(
                        "`type_params` for `{}` must be a literal tuple expression",
                        violation.alias_name
                    ),
                    "Use a literal tuple, e.g. `type_params=(T, S)`".to_owned(),
                ),
                TypeAliasTypeViolationKind::InvalidAttributeAccess { attr_name } => (
                    format!(
                        "`TypeAliasType` object `{}` has no attribute `{attr_name}`",
                        violation.alias_name
                    ),
                    "Valid attributes are `__value__`, `__type_params__`, and `__name__`"
                        .to_owned(),
                ),
                TypeAliasTypeViolationKind::IncorrectTypeArgCount { expected, actual } => (
                    format!(
                        "Incorrect type arguments for `{}`: expected {expected}, got {actual}",
                        violation.alias_name
                    ),
                    format!(
                        "`{}` expects {expected} type argument(s)",
                        violation.alias_name
                    ),
                ),
            };

            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                message,
                violation.span,
                &module.path,
                Some(help),
                None,
            ));
        }
    }
}
