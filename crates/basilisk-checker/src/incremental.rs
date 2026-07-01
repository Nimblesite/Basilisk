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
use crate::imports::ImportSearchPaths;

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

/// The import search paths as a Salsa **input**.
///
/// Resolution environment (workspace roots, `extraPaths`, stub dirs, venv
/// site-packages, and the `uv.lock`-derived [`basilisk_uv::PackageRegistry`])
/// is a genuine input of an import-resolving check: a venv sync, a `uv.lock`
/// edit, or a new stub dir changes *which* files an import resolves to. Re-set
/// it via `set_value` and salsa re-runs exactly the files whose diagnostics
/// depend on it. [CHKARCH-INCREMENTAL-SALSA]
#[salsa::input(debug)]
pub struct SearchPathsInput {
    /// The effective [`ImportSearchPaths`] applied to the check.
    #[returns(ref)]
    pub value: ImportSearchPaths,
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

/// Tracked query: the **import-resolved module** for one file, memoized by salsa
/// and keyed on the `(file, search_paths)` pair.
///
/// Runs `parse → resolve → `[`crate::imports::resolve_module_imports`], yielding
/// the module with its imports resolved against the search paths — the navigable
/// view a consumer (the LSP) needs for hover / references / go-to-definition.
/// Returns `None` if the file fails to parse or resolve. It is **not** keyed on
/// the config (resolution is config-independent), so a config-only edit reuses
/// the memoized module: [`checked_file_resolved`] re-runs only the cheap
/// `check_with_config` step.
///
/// **Filesystem-impurity boundary** (mirrors
/// [CHKCACHE-LIMITS](CHECKER-CACHE-SPEC.md#CHKCACHE-LIMITS)): the existence
/// probes and imported-file *content* that `resolve_module_imports` reads are
/// **not** tracked salsa edges — the memo invalidates only on `file.text` or a
/// re-set `SearchPathsInput`. Content-precise cross-file invalidation is a later
/// step.
#[salsa::tracked(returns(ref))]
pub fn resolved_module(
    db: &dyn Db,
    file: SourceFile,
    search_paths: SearchPathsInput,
) -> Option<std::sync::Arc<basilisk_resolver::ResolvedModule>> {
    let Ok(parsed) = basilisk_parser::parse_source(file.text(db).clone(), file.path(db).clone())
    else {
        return None;
    };
    let Ok(mut resolved) = basilisk_resolver::resolve(&parsed) else {
        return None;
    };
    // Reading `search_paths.value(db)` registers the salsa dependency edge.
    crate::imports::resolve_module_imports(&mut resolved, search_paths.value(db));
    Some(std::sync::Arc::new(resolved))
}

/// Tracked query: the **import-resolved** diagnostics for one file, memoized by
/// salsa and keyed on the `(file, config, search_paths)` triple.
///
/// A thin `check_with_config` over [`resolved_module`], so it honours import
/// resolution: the `imports_unresolved` rule and the cascade suppression of
/// downstream errors from unresolved imports both see the resolved import set.
/// Sharing [`resolved_module`] means parse + resolve + import-resolution run at
/// most once per `(file, search_paths)`, reused by both diagnostics and
/// navigation. Equivalence is **exactly** to `basilisk-cli`'s `process_file`
/// core: for any file that parses and resolves, [`file_diagnostics_resolved`]
/// equals `{ resolve_module_imports; check_with_config }` byte-for-byte.
#[salsa::tracked(returns(ref))]
pub fn checked_file_resolved(
    db: &dyn Db,
    file: SourceFile,
    config: ConfigInput,
    search_paths: SearchPathsInput,
) -> Vec<CachedDiagnostic> {
    let Some(resolved) = resolved_module(db, file, search_paths) else {
        return Vec::new();
    };
    crate::check_with_config(resolved, &config.value(db).0)
        .iter()
        .map(CachedDiagnostic::from)
        .collect()
}

/// Run [`checked_file_resolved`] and materialise owned [`Diagnostic`]s.
#[must_use]
pub fn file_diagnostics_resolved(
    db: &dyn Db,
    file: SourceFile,
    config: ConfigInput,
    search_paths: SearchPathsInput,
) -> Vec<Diagnostic> {
    checked_file_resolved(db, file, config, search_paths)
        .iter()
        .cloned()
        .map(CachedDiagnostic::into_diagnostic)
        .collect()
}
