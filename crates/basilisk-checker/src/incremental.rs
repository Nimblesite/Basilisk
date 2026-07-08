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

/// The outcome of parsing and resolving one file — the value of the
/// [`resolved_module`] query.
///
/// Distinguishes a parse failure (which a consumer surfaces as `BSK-PARSE`) from
/// a resolve failure (no module, no diagnostics) so a single memoized parse
/// serves diagnostics, navigation, and parse-error reporting alike. `PartialEq`
/// gives it salsa's `Update` via the fallback (`Arc<ResolvedModule>` compares by
/// value; `basilisk-resolver` stays salsa-free).
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedFile {
    /// Parsed and resolved: the import-resolved, navigable module.
    Resolved(std::sync::Arc<basilisk_resolver::ResolvedModule>),
    /// The file failed to parse; the error message drives `BSK-PARSE`.
    ParseError(String),
    /// The file parsed but failed to resolve — no module, no diagnostics.
    ResolveError,
}

/// A path → [`SourceFile`] map — the value of the [`WorkspaceFiles`] input.
///
/// Lets an import-resolving query depend on the *content* of the files it
/// imports: [`resolved_module`] reads each imported file's `SourceFile` text
/// through this map, so editing an imported file invalidates its importers'
/// memos (content-precise cross-file invalidation). A path absent from the map
/// simply creates no edge — a file the workspace does not track (e.g. a
/// third-party package) is treated as external.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileRegistry(pub std::collections::HashMap<std::path::PathBuf, SourceFile>);

/// The workspace's known source files as a Salsa **input**, keyed by absolute
/// path.
///
/// Editing a file's `SourceFile` text invalidates every importer whose
/// [`resolved_module`] read it (fine-grained); this *input* changes only when
/// files are added to or removed from the workspace — a coarse but rare event.
/// [CHKARCH-INCREMENTAL-SALSA]
#[salsa::input]
pub struct WorkspaceFiles {
    /// The path → `SourceFile` map.
    #[returns(ref)]
    pub files: FileRegistry,
}

/// Tracked query: the **import-resolved module** for one file, memoized by salsa
/// and keyed on the `(file, search_paths, workspace)` triple.
///
/// Runs `parse → resolve → `[`crate::imports::resolve_module_imports`], yielding
/// the module with its imports resolved against the search paths — the navigable
/// view a consumer (the LSP) needs for hover / references / go-to-definition.
/// The [`ResolvedFile`] outcome also carries a parse error, so the one memoized
/// parse serves diagnostics, navigation, and `BSK-PARSE` reporting. It is
/// **not** keyed on the config (resolution is config-independent), so a
/// config-only edit reuses the memoized module: [`checked_file_resolved`] re-runs
/// only the cheap `check_with_config` step.
///
/// **Cross-file invalidation.** After resolution, the query reads the
/// `SourceFile` text of every **user-stub** import the [`WorkspaceFiles`]
/// input tracks — the one import kind whose resolved-module output depends on
/// the imported file's *content* (its member API is derived from the text) —
/// so editing a tracked stub invalidates exactly its importers. All other
/// content-dependence is exports-level and tracked by
/// [`cross_resolved_module`]'s [`module_exports`] edges; recording raw text
/// edges for those too would re-parse every importer on any dependency edit.
/// Imported-file *existence* probes (and the content of files outside the
/// workspace, e.g. third-party packages) remain untracked, mirroring the
/// [CHKCACHE-LIMITS](CHECKER-CACHE-SPEC.md#CHKCACHE-LIMITS) boundary; those
/// invalidate only on a re-set `SearchPathsInput`/`WorkspaceFiles`.
#[salsa::tracked(returns(ref))]
pub fn resolved_module(
    db: &dyn Db,
    file: SourceFile,
    search_paths: SearchPathsInput,
    workspace: WorkspaceFiles,
) -> ResolvedFile {
    let parsed = match basilisk_parser::parse_source(file.text(db).clone(), file.path(db).clone()) {
        Ok(parsed) => parsed,
        Err(error) => return ResolvedFile::ParseError(error.to_string()),
    };
    let Ok(mut resolved) = basilisk_resolver::resolve(&parsed) else {
        return ResolvedFile::ResolveError;
    };
    // Reading `search_paths.value(db)` registers the salsa dependency edge.
    crate::imports::resolve_module_imports(&mut resolved, search_paths.value(db));
    register_import_dependencies(db, &mut resolved, search_paths.value(db), workspace);
    ResolvedFile::Resolved(std::sync::Arc::new(resolved))
}

/// Record a salsa dependency on the content of every **user-stub** import the
/// workspace tracks, and re-capture those stubs' APIs from that in-memory
/// content.
///
/// The text edge is deliberately narrow: a user-stub `.pyi` is the only import
/// whose *resolved-module output* depends on the imported file's content (its
/// member API is derived from the text, feeding `imports_module_attribute`) —
/// so reading its `SourceFile` here means an edited-but-unsaved stub updates
/// its importers. Every other import's content-dependence is exports-level and
/// flows through the memoized [`module_exports`] edge in
/// [`cross_resolved_module`]; recording raw text edges for those too would
/// re-execute every importer's parse+resolve on any dependency keystroke,
/// making a workspace sweep O(importers × parse) for no output change.
fn register_import_dependencies(
    db: &dyn Db,
    resolved: &mut basilisk_resolver::ResolvedModule,
    search_paths: &crate::imports::ImportSearchPaths,
    workspace: WorkspaceFiles,
) {
    let registry = workspace.files(db);
    let mut updates = Vec::new();
    for import in &resolved.imports {
        if !crate::imports::is_user_stub_import(import, search_paths) {
            continue;
        }
        let Some(imported) = import
            .resolved_path
            .as_ref()
            .and_then(|path| registry.0.get(path))
        else {
            continue;
        };
        // Reading the stub's text records the content edge...
        let text = imported.text(db);
        // ...and re-derives its API from that (possibly unsaved) text.
        if let Some(update) =
            crate::imports::recapture_user_stub_from_source(import, search_paths, text)
        {
            updates.push(update);
        }
    }
    for (binding, api) in updates {
        let _ = resolved.imported_modules.insert(binding, api);
    }
}

/// A module's exported top-level symbols — the value of the [`module_exports`]
/// query.
///
/// `PartialEq` gives it salsa's `Update` via the fallback, and — more
/// importantly — enables **backdating**: when an edit re-runs [`module_exports`]
/// but the export set is unchanged (a function-body edit), salsa marks the
/// result unchanged and every importer's [`cross_resolved_module`] memo stays
/// valid without re-executing. Export-set edits are the only cross-file edits
/// that propagate.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModuleExports(pub Vec<(String, basilisk_resolver::scope::ExternalSymbol)>);

/// Tracked query: the exported top-level symbols of one workspace file, from
/// its tracked source text.
///
/// Runs its **own** parse + resolve (deliberately *not* [`resolved_module`]:
/// exports need no import resolution, and depending on the full resolved module
/// would make mutually-importing files a salsa cycle). A file that fails to
/// parse or resolve exports nothing.
#[salsa::tracked(returns(ref))]
pub fn module_exports(db: &dyn Db, file: SourceFile) -> ModuleExports {
    let Ok(parsed) = basilisk_parser::parse_source(file.text(db).clone(), file.path(db).clone())
    else {
        return ModuleExports::default();
    };
    let Ok(resolved) = basilisk_resolver::resolve(&parsed) else {
        return ModuleExports::default();
    };
    let path = std::path::PathBuf::from(file.path(db));
    ModuleExports(crate::exports::extract_exports(&resolved, &path))
}

/// Tracked query: the **cross-module** resolved view of one file —
/// [`resolved_module`] plus `imported_symbols` populated from what its imports
/// actually export.
///
/// Workspace-tracked imports resolve through the memoized [`module_exports`],
/// which records a salsa edge **on the export set** rather than the raw text:
/// a body-only edit to an imported file backdates to "unchanged exports" and
/// this query's memo survives. Imports outside the workspace fall back to
/// on-demand disk parsing of `.pyi` stubs and PEP 561 `py.typed` packages —
/// untracked reads, mirroring the
/// [CHKCACHE-LIMITS](CHECKER-CACHE-SPEC.md#CHKCACHE-LIMITS) boundary (they
/// refresh when the `SearchPathsInput`/`WorkspaceFiles` inputs are re-set).
///
/// This is deliberately a **separate query** from [`resolved_module`]: the
/// plain query stays byte-for-byte equivalent to the CLI pipeline (which never
/// populates `imported_symbols`), so the LSP's non-cross-module modes keep CLI
/// parity, while cross-module mode opts into this richer view.
#[salsa::tracked(returns(ref))]
pub fn cross_resolved_module(
    db: &dyn Db,
    file: SourceFile,
    search_paths: SearchPathsInput,
    workspace: WorkspaceFiles,
) -> ResolvedFile {
    let base = resolved_module(db, file, search_paths, workspace);
    let ResolvedFile::Resolved(module) = base else {
        return base.clone();
    };
    let mut resolved = (**module).clone();
    let registry = workspace.files(db);
    // Reading `search_paths.value(db)` registers the salsa dependency edge, so a
    // changed `typeshed-path` re-runs this query and re-tags stub provenance.
    let custom_typeshed = search_paths.value(db).typeshed_path.as_deref();
    crate::exports::populate_imported_symbols(
        &mut resolved,
        |path| {
            registry
                .0
                .get(path)
                .map(|imported| module_exports(db, *imported).0.as_slice())
        },
        custom_typeshed,
    );
    ResolvedFile::Resolved(std::sync::Arc::new(resolved))
}

/// Tracked query: the **cross-module** diagnostics for one file — a thin
/// `check_with_config` over [`cross_resolved_module`], so checker rules see the
/// `imported_symbols` populated from other workspace files' current (in-memory)
/// content.
#[salsa::tracked(returns(ref))]
pub fn checked_file_cross(
    db: &dyn Db,
    file: SourceFile,
    config: ConfigInput,
    search_paths: SearchPathsInput,
    workspace: WorkspaceFiles,
) -> Vec<CachedDiagnostic> {
    let ResolvedFile::Resolved(resolved) = cross_resolved_module(db, file, search_paths, workspace)
    else {
        return Vec::new();
    };
    crate::check_with_config(resolved, &config.value(db).0)
        .iter()
        .map(CachedDiagnostic::from)
        .collect()
}

/// Run [`checked_file_cross`] and materialise owned [`Diagnostic`]s.
#[must_use]
pub fn file_diagnostics_cross(
    db: &dyn Db,
    file: SourceFile,
    config: ConfigInput,
    search_paths: SearchPathsInput,
    workspace: WorkspaceFiles,
) -> Vec<Diagnostic> {
    checked_file_cross(db, file, config, search_paths, workspace)
        .iter()
        .cloned()
        .map(CachedDiagnostic::into_diagnostic)
        .collect()
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
    workspace: WorkspaceFiles,
) -> Vec<CachedDiagnostic> {
    let ResolvedFile::Resolved(resolved) = resolved_module(db, file, search_paths, workspace)
    else {
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
    workspace: WorkspaceFiles,
) -> Vec<Diagnostic> {
    checked_file_resolved(db, file, config, search_paths, workspace)
        .iter()
        .cloned()
        .map(CachedDiagnostic::into_diagnostic)
        .collect()
}
