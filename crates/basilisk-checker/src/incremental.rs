//! Implements [CHKARCH-INCREMENTAL-SALSA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
//! The Salsa-memoized diagnostics query for a single source file.
//!
//! `missing_docs` is expected away for this module alone: salsa's `#[input]`
//! macro synthesises public constructors/accessors/setters for [`ConfigInput`]
//! that cannot carry doc comments (the same reason `basilisk-db`'s `db.rs`
//! carries this expectation). Every hand-written item below is still documented;
//! the expectation is self-cleaning (`unfulfilled_lint_expectations` is denied
//! workspace-wide, so it errors the moment the generated items stop tripping it).
#![expect(
    missing_docs,
    reason = "salsa macros generate undocumentable public constructors/accessors/setters"
)]

use basilisk_config::BasiliskConfig;
use basilisk_db::{Db, SourceFile};

use crate::cached::CachedDiagnostic;
use crate::diagnostic::Diagnostic;

/// The effective checker configuration, wrapped so it can be a Salsa input value.
///
/// `BasiliskConfig` lives in the `basilisk-config` leaf crate, which must stay
/// salsa-free. The `salsa::Update` derive on this newtype resolves the inner
/// `BasiliskConfig` through its `PartialEq` impl — the same dispatch fallback
/// `CachedDiagnostic` relies on for `Span`/`Severity`/`TypeProvenance` — so the
/// salsa dependency stays inside `basilisk-checker` and never reaches
/// `basilisk-config`. [CHKARCH-CONFIGURATION-ONLY]
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ConfigValue(pub BasiliskConfig);

/// The effective checker configuration as a Salsa **input**.
///
/// Configuration is a genuine input of a check, exactly like the source text:
/// toggling `strict_annotations` or changing a rule's severity must invalidate
/// and recompute the affected files' diagnostics. Mutating it via `set_value`
/// is what makes salsa re-execute the queries that read it — and only those.
/// [CHKARCH-INCREMENTAL-SALSA]
#[salsa::input(debug)]
pub struct ConfigInput {
    /// The effective `BasiliskConfig` applied to the check.
    #[returns(ref)]
    pub value: ConfigValue,
}

/// Tracked query: the diagnostics for one file, memoized by salsa.
///
/// Runs the **pure** pipeline — parse → resolve → [`crate::check_with_config`] —
/// and returns the result as owned [`CachedDiagnostic`]s so the value satisfies
/// salsa's `Update` bound. The query is keyed on the `(file, config)` pair, so
/// salsa re-executes it only when the file's [`SourceFile::text`] or the
/// [`ConfigInput`] it read changes; an unchanged input is served from the memo
/// ([CHKARCH-INCREMENTAL-SALSA]).
///
/// Equivalence is **exactly** to `check_with_config(&resolved, cfg)`: for any
/// file that parses and resolves, [`file_diagnostics`] equals the direct
/// pipeline byte-for-byte, so salsa memoization never corrupts a result. With
/// the default config it equals [`crate::check`]. It is deliberately **not**
/// equal to the batch CLI's output: `process_file`
/// (`basilisk-cli/src/main.rs`) additionally runs
/// [`crate::imports::resolve_module_imports`] (also surfaced as
/// `basilisk_lsp::import_resolver::resolve_module_imports`) between resolve and
/// check, which resolves imports against the venv/`uv.lock` and so changes both
/// the `imports_unresolved` rule and cascade suppression for import-bearing
/// files. That step reads the filesystem and cannot be a pure salsa query
/// without promoting the search paths to a salsa input — so this query covers
/// the import-free pipeline only, and is not yet a drop-in for the CLI/LSP
/// diagnostics paths (see [CHKARCH-INCREMENTAL-SALSA] for the adoption plan).
/// The engine now living in [`crate::imports`] is the enabling prerequisite for
/// that fold.
///
/// A file that fails to parse or resolve yields no diagnostics, matching the
/// batch CLI's handling of such files (it logs and emits nothing for them).
#[salsa::tracked(returns(ref))]
pub fn checked_file(db: &dyn Db, file: SourceFile, config: ConfigInput) -> Vec<CachedDiagnostic> {
    let Ok(parsed) = basilisk_parser::parse_source(file.text(db).clone(), file.path(db).clone())
    else {
        return Vec::new();
    };
    let Ok(resolved) = basilisk_resolver::resolve(&parsed) else {
        return Vec::new();
    };
    // Reading `config.value(db)` is what registers the salsa dependency edge, so
    // an edit to the `ConfigInput` invalidates exactly the files it affects.
    crate::check_with_config(&resolved, &config.value(db).0)
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
pub fn file_diagnostics(db: &dyn Db, file: SourceFile, config: ConfigInput) -> Vec<Diagnostic> {
    checked_file(db, file, config)
        .iter()
        .cloned()
        .map(CachedDiagnostic::into_diagnostic)
        .collect()
}
