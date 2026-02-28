//! BSK-E0056: Mutation of ReadOnly TypedDict fields
//!
//! Fields marked as `ReadOnly` in TypedDicts cannot be mutated through:
//! - Direct assignment: `td["key"] = value`
//! - `.update()` calls
//! - `**kwargs` mutation
//!
//! ```python
//! from typing import TypedDict, ReadOnly
//!
//! class Config(TypedDict):
//!     name: str
//!     version: ReadOnly[str]
//!
//! cfg: Config = {"name": "test", "version": "1.0"}
//! cfg["version"] = "2.0"  # E0056 - Cannot mutate ReadOnly field
//! cfg.update(version="2.0")  # E0056 - Cannot mutate ReadOnly field
//! ```

use basilisk_resolver::{ResolvedModule, Span};
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0056",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0056",
};

fn make_diag(field_name: &str, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE,
        severity: Severity::Error,
        message: format!("Cannot mutate ReadOnly TypedDict field '{}'", field_name),
        span,
        path: path.to_owned(),
        help: Some("ReadOnly fields can only be read, not written".to_owned()),
        note: None,
    }
}

/// Rule E0056: Detect mutation of ReadOnly TypedDict fields
pub(crate) struct ReadOnlyTypedDictMutation;

impl Rule for ReadOnlyTypedDictMutation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        // TODO: Implement actual ReadOnly TypedDict mutation detection
        // Placeholder implementation that doesn't break compilation
        // TODO: Implement actual ReadOnly TypedDict mutation detection
        // For now this is a placeholder — no false positives
        let _ = (module, diagnostics);
    }
}

