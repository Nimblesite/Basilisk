//! Implements [BSK-0002] from [CHKARCH-DIAG-MISSING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-missing
//! BSK-0002: Missing return type annotation.
//!
//! Never fires where the current engine already infers the return type
//! ([TYPEINF-FUNC-RETURN]): a body whose every `return` is bare or carries a
//! type-determining literal — or that has no `return` at all (`None`) — needs
//! no annotation. Returns the engine cannot infer (calls, names, arbitrary
//! expressions) and generators (`Generator[...]`) still require one.
//!
//! ```python
//! def answer():                 # ✓ — return type inferred as int
//!     return 42
//!
//! def log_it(msg: str):         # ✓ — no return: inferred as None
//!     print(msg)
//!
//! def fetch():                  # BSK-0002 — call result is not inferable
//!     return make_value()
//! ```

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::inference::rhs_fully_determines_type;

use super::{guards::is_stub_context, Rule};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-0002",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-0002",
};

/// Emits BSK-0002 for every function without a return type annotation.
///
/// Skipped for `@overload`, `@abstractmethod`, and `Protocol` methods.
pub(crate) struct MissingReturnAnnotation;

impl Rule for MissingReturnAnnotation {
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
            .filter(|func| {
                !func.return_annotation.is_present()
                    && !is_stub_context(func, &module.classes)
                    && !return_type_is_inferable(func)
            })
            .for_each(|func| diagnostics.push(make_diagnostic(func, &module.path)));
    }
}

/// Implements [TYPEINF-FUNC-RETURN]: `true` when the current engine already
/// infers the full return type, making an annotation redundant.
///
/// - No `return` statements at all → `None` (trivially inferred).
/// - Every `return` is bare (`None`) or carries a type-determining literal →
///   the union of those types is inferred.
/// - Generators are excluded: their `Generator[...]` type is not inferable
///   from `return` statements, so they still require an annotation.
fn return_type_is_inferable(func: &FunctionInfo) -> bool {
    !func.is_generator
        && func
            .return_stmts
            .iter()
            .all(|ret| !ret.has_value || rhs_fully_determines_type(&ret.rhs_kind))
}

fn make_diagnostic(func: &FunctionInfo, path: &str) -> Diagnostic {
    error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Missing return type annotation for function `{}`",
            func.name
        ),
        Span {
            start: func.name_span.start,
            end: func.params_end,
        },
        path,
        Some(format!(
            "Add a return type: `def {}(...) -> <type>:`",
            func.name
        )),
        Some(
            "Basilisk requires an explicit return type wherever it cannot be inferred; \
             literal-only returns (e.g. `return 42`) infer the type and need no annotation"
                .to_owned(),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::{MissingReturnAnnotation, Rule};

    /// [CHKARCH-CONFIG-MODEL]: BSK-0002 is an opt-in `strictness` house rule,
    /// never a PEP rule. Provenance is read from `opt_in_spec`, so a `None`
    /// here would silently promote the rule to always-on — guard it directly.
    #[test]
    fn opt_in_spec_marks_a_strictness_house_rule() {
        let spec = MissingReturnAnnotation.opt_in_spec();
        assert!(spec.is_some(), "BSK-0002 must stay opt-in");
        if let Some(spec) = spec {
            assert_eq!(spec.code, "BSK-0002");
            assert_eq!(spec.tags, &["strictness"]);
        }
    }
}
