//! Implements [BSK-E0084] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! BSK-E0084: `TypeVarTuple` variance/bounds/constraints violation.
//!
//! `TypeVarTuple` does not support specification of variance, bounds, or constraints.
//! Using these parameters with `TypeVarTuple` is invalid.
//!
//! ```python
//! # BAD
//! Ts = TypeVarTuple("Ts", covariant=True)  # E: TypeVarTuple does not support variance
//! Ts = TypeVarTuple("Ts", int, float)      # E: TypeVarTuple does not support constraints
//! Ts = TypeVarTuple("Ts", bound=int)       # E: TypeVarTuple does not support bounds
//!
//! # GOOD
//! Ts = TypeVarTuple("Ts")  # OK
//! ```

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0084",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0084",
};

/// Emits BSK-E0084 when a `TypeVarTuple` has invalid parameters.
pub(crate) struct TypeVarTupleInvalidParams;

impl Rule for TypeVarTupleInvalidParams {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        for call in &module.calls {
            if call.callee != "TypeVarTuple" {
                continue;
            }

            let has_variance = call
                .keywords
                .iter()
                .any(|(name, _)| name == "covariant" || name == "contravariant");
            let has_bound = call.keywords.iter().any(|(name, _)| name == "bound");
            // args[0] is the name string; any further positional args are constraints
            let has_constraints = call.args.len() > 1;

            let error_msg = if has_variance {
                Some("`TypeVarTuple` does not support variance specification")
            } else if has_bound {
                Some("`TypeVarTuple` does not support bounds")
            } else if has_constraints {
                Some("`TypeVarTuple` does not support type constraints")
            } else {
                None
            };

            if let Some(msg) = error_msg {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!("Invalid `TypeVarTuple` parameters: {msg}"),
                    call.span,
                    &module.path,
                    Some("Use `TypeVarTuple(\"Ts\")` without additional parameters".to_owned()),
                    Some(
                        "`TypeVarTuple` does not support variance, bounds, or constraints"
                            .to_owned(),
                    ),
                ));
            }
        }
    }
}
