//! Implements [CHKARCH-INCREMENTAL-SALSA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
//! The Salsa-memoized diagnostics query for a single source file.

use basilisk_db::{Db, SourceFile};

use crate::cached::CachedDiagnostic;
use crate::diagnostic::Diagnostic;

/// Tracked query: the diagnostics for one file, memoized by salsa.
///
/// Runs the **pure** pipeline — parse → resolve → [`crate::check`], in the
/// default configuration (PEP rules only, [CHKARCH-CONFIGURATION-ONLY]) — and
/// returns the result as owned [`CachedDiagnostic`]s so the value satisfies
/// salsa's `Update` bound. Salsa re-executes this only when the file's
/// [`SourceFile::text`] changes; an unchanged file is served from the memo
/// ([CHKARCH-INCREMENTAL-SALSA]).
///
/// Equivalence is **exactly** to `check(&resolved)`: for any file that parses
/// and resolves, [`file_diagnostics`] equals the direct pipeline byte-for-byte,
/// so salsa memoization never corrupts a result. It is deliberately **not**
/// equal to the batch CLI's output: `process_file`
/// (`basilisk-cli/src/main.rs`) additionally runs
/// `basilisk_lsp::import_resolver::resolve_module_imports` between resolve and
/// check, which resolves imports against the venv/`uv.lock` and so changes both
/// the `imports_unresolved` rule and cascade suppression for import-bearing
/// files. That step reads the filesystem and cannot be a pure salsa query
/// without promoting the search paths to a salsa input — so this query covers
/// the import-free, default-config pipeline only, and is not yet a drop-in for
/// the CLI/LSP diagnostics paths (see [CHKARCH-INCREMENTAL-SALSA] for the
/// adoption plan). Configuration is likewise fixed at the default and is not a
/// tracked input.
///
/// A file that fails to parse or resolve yields no diagnostics, matching the
/// batch CLI's handling of such files (it logs and emits nothing for them).
#[salsa::tracked(returns(ref))]
pub fn checked_file(db: &dyn Db, file: SourceFile) -> Vec<CachedDiagnostic> {
    let Ok(parsed) = basilisk_parser::parse_source(file.text(db).clone(), file.path(db).clone())
    else {
        return Vec::new();
    };
    let Ok(resolved) = basilisk_resolver::resolve(&parsed) else {
        return Vec::new();
    };
    crate::check(&resolved)
        .iter()
        .map(CachedDiagnostic::from)
        .collect()
}

/// Run [`checked_file`] and materialise owned [`Diagnostic`]s.
///
/// Convenience for consumers that want the full diagnostic type rather than the
/// cache projection; reconstructs the `&'static` code/URL via the bounded
/// interner in [`crate::cached`].
#[must_use]
pub fn file_diagnostics(db: &dyn Db, file: SourceFile) -> Vec<Diagnostic> {
    checked_file(db, file)
        .iter()
        .cloned()
        .map(CachedDiagnostic::into_diagnostic)
        .collect()
}
