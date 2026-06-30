//! Implements [`typeddicts_readonly`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-structural
//! `typeddicts_readonly`: Mutation of `ReadOnly` `TypedDict` fields
//!
//! Fields marked as `ReadOnly` in `TypedDict`s cannot be mutated through:
//! - Direct assignment: `td["key"] = value`
//! - `.update()` calls
//!
//! ```python
//! from typing import TypedDict
//! from typing_extensions import ReadOnly
//!
//! class Config(TypedDict):
//!     name: str
//!     version: ReadOnly[str]
//!
//! cfg: Config = {"name": "test", "version": "1.0"}
//! cfg["version"] = "2.0"  # E0056
//! cfg.update(version="2.0")  # E0056
//! ```

use basilisk_resolver::{ReadOnlyViolationKind, ResolvedModule};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "typeddicts_readonly",
    docs_url: "https://www.basilisk-python.dev/errors/typeddicts_readonly",
};

/// Rule E0056: Detect mutation of `ReadOnly` `TypedDict` fields
pub(crate) struct ReadOnlyTypedDictMutation;

impl Rule for ReadOnlyTypedDictMutation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for v in &module.readonly_violations {
            let message = match v.kind {
                ReadOnlyViolationKind::SubscriptAssign => {
                    let field = v.field_name.as_deref().unwrap_or("?");
                    format!(
                        "Cannot assign to read-only field `{field}` of `TypedDict` `{}`",
                        v.var_name
                    )
                }
                ReadOnlyViolationKind::UpdateCall => {
                    format!(
                        "Cannot call `.update()` on `TypedDict` `{}`: it has `ReadOnly` fields",
                        v.var_name
                    )
                }
            };
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                message,
                v.span,
                &module.path,
                Some("Remove the mutation or make the field writable".to_owned()),
                Some(
                    "PEP 705: `ReadOnly` fields in a `TypedDict` may not be assigned after construction".to_owned(),
                ),
            ));
        }
    }
}
