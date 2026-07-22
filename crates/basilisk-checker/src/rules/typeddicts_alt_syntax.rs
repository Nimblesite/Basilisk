//! Implements [`typeddicts_alt_syntax`] from [CHKARCH-DIAG-OWNERSHIP]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-OWNERSHIP
//! `typeddicts_alt_syntax`: Invalid `TypedDict(...)` functional-syntax call.
//!
//! The `TypedDict(name, {...})` functional syntax has several constraints:
//!
//! 1. The second positional argument must be a dict literal `{...}`.
//! 2. All keys in the dict literal must be string literals.
//! 3. The first positional argument (the declared name) must match the
//!    variable name on the left-hand side of the assignment.
//! 4. Only `total=` is recognised as a keyword argument; anything else is
//!    an error.

use basilisk_resolver::{ResolvedModule, Span, TypedDictSecondArgKind};

use crate::diagnostic::{error_diagnostic, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "typeddicts_alt_syntax",
    docs_url: "https://www.basilisk-python.dev/errors/typeddicts_alt_syntax",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        None,
        Some("PEP 589: TypedDict functional syntax has strict requirements on its arguments"),
    )
}

/// Emits `typeddicts_alt_syntax` for invalid `TypedDict(...)` functional-syntax calls.
pub(crate) struct InvalidTypedDictCall;

/// Keyword arguments allowed in the `TypedDict(...)` functional syntax.
const ALLOWED_KEYWORDS: &[&str] = &["total", "extra_items", "closed"];

impl Rule for InvalidTypedDictCall {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for td in &module.typeddict_calls {
            // Check 1: second arg must be a dict literal.
            if td.second_arg_kind == TypedDictSecondArgKind::NotDictLiteral {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Second argument to `TypedDict(\"{}\", ...)` must be a dict literal",
                        td.lhs_name
                    ),
                    td.span,
                    &module.path,
                ));
                // Don't check keys if the second arg isn't a dict literal.
                continue;
            }

            // Check 2: all keys must be string literals.
            if td.has_non_string_key {
                diagnostics.push(make_diagnostic(
                    format!(
                        "All keys in `TypedDict(\"{}\", {{...}})` must be string literals",
                        td.lhs_name
                    ),
                    td.span,
                    &module.path,
                ));
            }

            // Check 3: declared name must match LHS.
            if let Some(declared) = &td.declared_name {
                if declared != &td.lhs_name {
                    diagnostics.push(make_diagnostic(
                        format!(
                            "TypedDict declared name `{declared}` does not match \
                             the variable name `{}`",
                            td.lhs_name
                        ),
                        td.span,
                        &module.path,
                    ));
                }
            }

            // Check 4: only recognised keywords are allowed when using dict form.
            // In keyword-only form (no second positional arg), all keyword args are
            // field name definitions and are not configuration keywords.
            if td.has_positional_dict {
                for kw in &td.keyword_names {
                    if !ALLOWED_KEYWORDS.contains(&kw.as_str()) {
                        diagnostics.push(make_diagnostic(
                            format!(
                                "Unrecognised keyword `{kw}=` in `TypedDict(\"{}\", ...)` call",
                                td.lhs_name
                            ),
                            td.span,
                            &module.path,
                        ));
                    }
                }
            }
        }
    }
}
