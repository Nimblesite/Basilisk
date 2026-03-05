use basilisk_resolver::ResolvedModule;

use super::Rule;
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

/// BSK-E0100: Augmented assignment widens `Literal` type.
///
/// When a function parameter is annotated with `Literal[...]`, augmented
/// assignment (`+=`, `-=`, etc.) effectively reassigns the variable to a
/// widened type (e.g. `int` instead of `Literal[3, 4, 5]`), violating the
/// declared `Literal` constraint.
///
/// ```python
/// def func(a: Literal[3, 4, 5]):
///     a += 3  # E0100 — augmented assign widens Literal type
/// ```
const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0100",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0100",
};

/// Emits BSK-E0100 for augmented assignment on `Literal`-typed parameters.
pub(crate) struct LiteralAugmentedAssign;

impl Rule for LiteralAugmentedAssign {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for violation in &module.literal_augmented_assign_violations {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Augmented assignment to `{}` widens its `Literal` type",
                    violation.var_name
                ),
                span: violation.span,
                path: module.path.clone(),
                help: Some(format!(
                    "Use a separate variable instead: `result = {} + ...`",
                    violation.var_name
                )),
                note: Some(
                    "`a += x` is equivalent to `a = a + x`, which changes the type of `a` \
                     from `Literal[...]` to the wider base type"
                        .to_owned(),
                ),
            });
        }
    }
}
