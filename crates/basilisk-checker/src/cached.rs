//! Implements [CHKCACHE-DIAG] / [CHKCACHE-DIAG-INTERN].
//! See docs/specs/CHECKER-CACHE-SPEC.md#chkcache-entry
//!
//! Owned, serde-serialisable projection of [`Diagnostic`] for the result cache.
//!
//! [`Diagnostic`] cannot itself round-trip through serde because its
//! [`ErrorCode`] holds `&'static str` fields, which cannot be reconstructed from
//! deserialised data. [`CachedDiagnostic`] owns its strings; on replay the
//! `&'static` code/URL are recovered through a bounded process-wide interner
//! (one leaked entry per distinct string — at most one per BSK code, so memory
//! is constant regardless of how many diagnostics are replayed).

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use basilisk_resolver::Span;
use basilisk_stubs::TypeProvenance;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

/// Owned, serialisable form of a [`Diagnostic`].
///
/// Also the value type memoized by the Salsa `checked_file` query
/// ([CHKARCH-INCREMENTAL-SALSA]): it is fully owned, so it satisfies salsa's
/// `Update` bound. The derive resolves each field through salsa's dispatch
/// helper — `String`/`Option<String>` use their `Update` impls, while `Span`,
/// `Severity`, and `Option<TypeProvenance>` fall back to their `PartialEq`
/// impls (all are `'static`), so deriving here needs no salsa dependency in
/// `basilisk-resolver` or `basilisk-stubs`.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update, serde::Serialize, serde::Deserialize)]
pub struct CachedDiagnostic {
    /// Full code string, e.g. `"returns_compatibility"`.
    pub code: String,
    /// Documentation URL for the code.
    pub docs_url: String,
    /// Severity level.
    pub severity: Severity,
    /// Human-readable primary message.
    pub message: String,
    /// Highlighted source span.
    pub span: Span,
    /// Source file path.
    pub path: String,
    /// Optional help text.
    pub help: Option<String>,
    /// Optional note text.
    pub note: Option<String>,
    /// Provenance of the related type information, if any.
    pub provenance: Option<TypeProvenance>,
}

impl From<&Diagnostic> for CachedDiagnostic {
    fn from(diagnostic: &Diagnostic) -> Self {
        Self {
            code: diagnostic.code.code.to_owned(),
            docs_url: diagnostic.code.docs_url.to_owned(),
            severity: diagnostic.severity,
            message: diagnostic.message.clone(),
            span: diagnostic.span,
            path: diagnostic.path.clone(),
            help: diagnostic.help.as_deref().map(str::to_owned),
            note: diagnostic.note.as_deref().map(str::to_owned),
            provenance: diagnostic.provenance,
        }
    }
}

impl CachedDiagnostic {
    /// Reconstruct a full [`Diagnostic`], recovering `&'static` code/URL via the
    /// bounded interner.
    #[must_use]
    pub fn into_diagnostic(self) -> Diagnostic {
        Diagnostic {
            code: ErrorCode {
                code: intern_str(&self.code),
                docs_url: intern_str(&self.docs_url),
            },
            severity: self.severity,
            message: self.message,
            span: self.span,
            path: self.path,
            help: self.help.map(Cow::Owned),
            note: self.note.map(Cow::Owned),
            provenance: self.provenance,
        }
    }
}

/// Intern a string to `&'static str`, leaking at most once per distinct value.
///
/// The set of interned values is bounded by the finite set of BSK diagnostic
/// codes and their documentation URLs, so total leaked memory is constant for
/// the process lifetime.
fn intern_str(value: &str) -> &'static str {
    static POOL: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let pool = POOL.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = pool.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(&existing) = guard.get(value) {
        return existing;
    }
    let leaked: &'static str = Box::leak(value.to_owned().into_boxed_str());
    let _ = guard.insert(value.to_owned(), leaked);
    leaked
}
