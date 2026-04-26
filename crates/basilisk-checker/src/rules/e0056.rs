//! BSK-E0056: Mutation of `ReadOnly` `TypedDict` fields
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

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0056",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0056",
};

/// Rule E0056: Detect mutation of `ReadOnly` `TypedDict` fields
pub(crate) struct ReadOnlyTypedDictMutation;

impl Rule for ReadOnlyTypedDictMutation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
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
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message,
                span: v.span,
                path: module.path.clone(),
                help: Some("Remove the mutation or make the field writable".to_owned()),
                note: Some(
                    "PEP 705: `ReadOnly` fields in a `TypedDict` may not be assigned after construction".to_owned(),
                ),
                provenance: None,
            });
        }
    }
}
