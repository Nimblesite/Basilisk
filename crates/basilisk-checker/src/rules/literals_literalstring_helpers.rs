//! Implements [`literals_literalstring`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Diagnostic constructors for `literals_literalstring`.
//!
//! The rule's verdicts are structural over the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION]); this module only formats the resulting
//! diagnostics. The previous text machinery here — body-range scanning, line
//! reconstruction with a leading-keyword skip-list (including the
//! fixture-fitted `assert_type`), f-string char-walking — is deleted per
//! [CHKARCH-CONFORMANCE-MODE] (issue #408).

use basilisk_resolver::Span;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

/// `literals_literalstring` error code shared between this module and the rule.
pub(super) const CODE: ErrorCode = ErrorCode {
    code: "literals_literalstring",
    docs_url: "https://www.basilisk-python.dev/errors/literals_literalstring",
};

/// Emit a `literals_literalstring` diagnostic for a literal value mismatch.
pub(super) fn emit_literal_value_mismatch(
    name_span: Span,
    target_value: &str,
    source_value: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Cannot assign `Literal[\"{source_value}\"]` to `Literal[\"{target_value}\"]` \
             — literal values are incompatible"
        ),
        name_span,
        path,
        Some(format!(
            "The variable expects exactly `Literal[\"{target_value}\"]`, \
             but the parameter has type `Literal[\"{source_value}\"]`"
        )),
        Some(
            "PEP 586: Literal types are only compatible when their values match exactly".to_owned(),
        ),
    ));
}

/// Emit a `literals_literalstring` diagnostic for an f-string `LiteralString` violation.
pub(super) fn emit_fstring_literal_string_error(
    name_span: Span,
    name: &str,
    param_ann: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Cannot assign f-string to `LiteralString` — interpolated variable \
             `{name}` has type `{param_ann}`, which is not `LiteralString`"
        ),
        name_span,
        path,
        Some(format!(
            "Change `{name}` to `LiteralString` or use `str` as the target type"
        )),
        Some(
            "PEP 675: an f-string is `LiteralString` only if all interpolated \
             expressions are compatible with `LiteralString`"
                .to_owned(),
        ),
    ));
}

/// Emit a `literals_literalstring` diagnostic for an invariant container generic mismatch.
pub(super) fn emit_invariant_container_mismatch(
    name_span: Span,
    param_ann: &str,
    ann: &str,
    ann_container: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Cannot assign `{param_ann}` to `{ann}` — \
             `{ann_container}` is invariant in its type parameter"
        ),
        name_span,
        path,
        Some(format!(
            "Use `Sequence[str]` (covariant) instead of `{ann}` if you \
             need to accept `{param_ann}`"
        )),
        Some(
            "PEP 484: mutable generic containers like `list` are invariant — \
             `list[LiteralString]` is not a subtype of `list[str]`"
                .to_owned(),
        ),
    ));
}

/// Emit a `literals_literalstring` diagnostic for a container constructor call with str argument.
pub(super) fn emit_container_call_str_error(
    name_span: Span,
    rhs: &str,
    ann: &str,
    arg: &str,
    param_ann: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Cannot assign `{rhs}` to `{ann}` — argument `{arg}` \
             has type `{param_ann}`, not `LiteralString`"
        ),
        name_span,
        path,
        Some(format!(
            "Change `{arg}` to `LiteralString` or relax the target annotation"
        )),
        Some(
            "PEP 675: `str` is not assignable to `LiteralString` — \
             `LiteralString` is a strict subtype of `str`"
                .to_owned(),
        ),
    ));
}
