//! Implements [ANALYSIS-INDEX]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INDEX
//! Workspace index — persistent per-file analysis state for whole-module and
//! cross-module analysis modes.
//!
//! See `docs/LSP-ANALYSIS-MODES-SPEC.md` for the full specification.

use std::path::PathBuf;
use std::sync::Arc;

use basilisk_config::BasiliskConfig;
use basilisk_uv::PackageRegistry;
use dashmap::DashMap;
use tower_lsp::lsp_types::{TextEdit, Url};

use crate::config::AnalysisMode;
use crate::import_graph::ImportGraph;
use crate::workspace_analysis::{
    analyse_with_config, bsk_to_lsp, fnv1a, make_entry, parse_error_diagnostic,
};
use crate::workspace_scan::{collect_python_files, deduplicate_by_stem, path_to_uri};

// ── FileEntry ────────────────────────────────────────────────────────────────

/// Per-file analysis state cached in the workspace index.
// Implements [ANALYSIS-INDEX-STRUCT] — the spec's FileEntry shape (source_hash,
// resolved, diagnostics, version, is_open); `text` is carried so reload/recheck
// need not re-read the buffer.
// Implements [LSPARCH-ARCH-CACHE] — caches the resolved module (the spec's
// `DocumentState.resolved`), refreshed on did_open/did_change and reused by every
// feature handler.
#[derive(Debug)]
pub struct FileEntry {
    /// FNV-1a hash of the source text at last analysis; used for invalidation.
    pub source_hash: u64,
    /// Raw source text — always present, even when parsing/resolving failed.
    pub text: String,
    /// Resolved symbol table from the last successful parse+resolve cycle.
    /// `None` if the file failed to parse or resolve.
    pub resolved: Option<Arc<basilisk_resolver::ResolvedModule>>,
    /// Diagnostics from the last check cycle.
    pub diagnostics: Vec<basilisk_checker::Diagnostic>,
    /// LSP document version (non-zero for open documents).
    pub version: i32,
    /// `true` iff the editor currently has this file open; editor text is authoritative.
    pub is_open: bool,
    /// The most recent revision that parsed AND resolved, carried across
    /// failures so display surfaces can serve a stale-but-coherent view.
    ///
    /// Kept separate from `text`/`resolved`: those two must stay current
    /// (completion patches `text`, diagnostics describe it), while this pair is
    /// only ever read together — a resolved module's spans index the exact
    /// source it came from. Implements [ANALYSIS-INDEX-LASTGOOD].
    pub last_good: Option<Arc<LastGoodResolve>>,
}

/// A source text and the resolved module built from it, kept as one unit.
///
/// The pairing is the point: `ResolvedModule` holds byte spans into the text it
/// was resolved from, so rendering it against any other revision misplaces
/// every position. Implements [ANALYSIS-INDEX-LASTGOOD].
#[derive(Debug)]
pub struct LastGoodResolve {
    /// The source text that produced [`Self::resolved`].
    pub text: String,
    /// The symbol table resolved from [`Self::text`].
    pub resolved: Arc<basilisk_resolver::ResolvedModule>,
}

/// Whether a workspace re-analysis publishes every file or only real changes.
///
/// `ChangedOnly` assumes the client's diagnostic state matches the server's
/// store (steady-state sweeps); `Always` is for paths where the client may
/// have diverged — post-scan open-file convergence, re-enable rescans.
#[derive(Clone, Copy)]
enum PublishPolicy {
    /// Publish every re-analysed file.
    Always,
    /// Publish only files whose checker diagnostics differ from the stored ones.
    ChangedOnly,
}

// ── WorkspaceIndex ───────────────────────────────────────────────────────────

/// Process-scoped index of all analysed files.
///
/// Owned by `LspServer`. All handlers access file state through this type
/// rather than the old `DashMap<Url, DocumentState>`.
// Implements [ANALYSIS-INDEX-STRUCT] — the spec's WorkspaceIndex shape (roots,
// files, config, optional import_graph populated in crossModule).
pub struct WorkspaceIndex {
    /// Workspace root directories.
    pub roots: Vec<PathBuf>,
    /// File path → analysis state.
    pub files: DashMap<PathBuf, FileEntry>,
    /// Analysis mode controlling which files are analysed.
    ///
    /// Interior-mutable so a runtime mode switch needs only a READ guard on
    /// the index: a mode-flip WRITE queued behind a long-running scan's read
    /// guard turns tokio's fair `RwLock` into a barrier for every subsequent
    /// reader — and with tower-lsp's bounded handler concurrency that stalls
    /// the whole message loop, starving `didClose` clears (GitHub #264).
    mode: std::sync::RwLock<AnalysisMode>,
    /// Import dependency graph for cross-module invalidation.
    ///
    /// Built during workspace scan in `crossModule` mode.
    /// Protected by a `Mutex` for interior mutability.
    pub import_graph: std::sync::Mutex<ImportGraph>,
    /// Package registry from uv lock file, if this is a uv project.
    ///
    /// Built during workspace initialisation and rebuilt when `uv.lock`
    /// changes. Used for import classification and dependency diagnostics.
    pub registry: Option<Arc<PackageRegistry>>,
    /// Per-root project-level checker configuration.
    ///
    /// Each workspace root can have its own `pyproject.toml` `[tool.basilisk]`
    /// with different rule severity overrides, per-module, and per-path settings.
    /// Files are matched to their owning root to apply the correct config.
    pub root_configs: std::collections::HashMap<PathBuf, BasiliskConfig>,
    /// Fallback checker configuration used when a file doesn't belong to any
    /// known root, or for single-root backwards compatibility.
    pub checker_config: BasiliskConfig,
    /// Per-directory merged rule config cache ([CHKARCH-CONFIG-DISCOVERY]):
    /// the owning root's config merged with configs discovered on the file's
    /// own ancestor chain, so a child directory's config applies exactly as
    /// in `basilisk check` (GitHub #311). Cleared by `reload_root_configs`.
    dir_configs: std::sync::RwLock<std::collections::HashMap<PathBuf, Arc<BasiliskConfig>>>,
    /// Import search paths (venv site-packages, workspace members, stub paths,
    /// uv registry) cached per owning workspace root from the last full scan.
    ///
    /// Reused by the incremental single-file analysis path (`didOpen` /
    /// `didChange` / disk reload) so third-party import resolution matches the
    /// full scan and the editor does not resurrect false `imports_unresolved`.
    /// Implements [ANALYSIS-INCR-IMPORTS]. See
    /// docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-INCR-IMPORTS
    pub search_paths_by_root: std::sync::RwLock<
        std::collections::HashMap<PathBuf, Arc<crate::import_resolver::ImportSearchPaths>>,
    >,
    /// In-session Salsa engine backing the incremental single-file analysis path
    /// once the search paths are known. Memoizes `parse → resolve →
    /// resolve_module_imports → check` per file. Implements
    /// [CHKARCH-INCREMENTAL-SALSA].
    pub(crate) salsa_engine: crate::salsa_engine::SalsaAnalysisEngine,
}

fn authoritative_edited_text(
    current: String,
    original: &str,
    edited: String,
    is_open: bool,
) -> String {
    if is_open && current != original && current != edited {
        return current;
    }
    edited
}

fn apply_non_overlapping_edits(source: &str, edits: &[TextEdit]) -> Option<String> {
    let mut indexed = edits
        .iter()
        .enumerate()
        .map(|(order, edit)| {
            let start = crate::util::position_to_byte_offset(source, edit.range.start);
            let end = crate::util::position_to_byte_offset(source, edit.range.end);
            (start, end, order, edit.new_text.as_str())
        })
        .collect::<Vec<_>>();
    indexed.sort_by_key(|&(start, end, order, _)| (start, end, order));
    if indexed.iter().any(|&(start, end, _, _)| start > end)
        || indexed
            .windows(2)
            .any(|pair| matches!(pair, [left, right] if left.1 > right.0))
    {
        return None;
    }
    let mut result = source.to_owned();
    for (start, end, _, replacement) in indexed.into_iter().rev() {
        result.replace_range(start..end, replacement);
    }
    Some(result)
}

impl std::fmt::Debug for WorkspaceIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceIndex")
            .field("roots", &self.roots)
            .field("mode", &self.mode())
            .field("file_count", &self.files.len())
            .finish_non_exhaustive()
    }
}

impl WorkspaceIndex {
    /// Create an empty index for the given roots, mode, and project config.
    ///
    /// Each root is checked for its own `pyproject.toml` `[tool.basilisk]`.
    /// If a root has no config file, the provided `checker_config` is used as
    /// the fallback for that root.
    #[must_use]
    pub fn new(roots: Vec<PathBuf>, mode: AnalysisMode, checker_config: BasiliskConfig) -> Self {
        let root_configs = Self::load_root_configs(&roots, &checker_config);
        Self {
            roots,
            files: DashMap::new(),
            mode: std::sync::RwLock::new(mode),
            import_graph: std::sync::Mutex::new(ImportGraph::new()),
            registry: None,
            root_configs,
            checker_config,
            dir_configs: std::sync::RwLock::new(std::collections::HashMap::new()),
            search_paths_by_root: std::sync::RwLock::new(std::collections::HashMap::new()),
            salsa_engine: crate::salsa_engine::SalsaAnalysisEngine::default(),
        }
    }

    /// The current analysis mode.
    ///
    /// Poisoning is unrecoverable only for non-atomic state; `AnalysisMode` is
    /// `Copy`, so a poisoned guard still holds a valid value — recover it.
    #[must_use]
    pub fn mode(&self) -> AnalysisMode {
        *self
            .mode
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Switch the analysis mode at runtime (`didChangeConfiguration`).
    ///
    /// Takes `&self` deliberately — see the `mode` field docs (GitHub #264).
    pub fn set_mode(&self, mode: AnalysisMode) {
        *self
            .mode
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = mode;
    }

    /// Load each root's `BasiliskConfig` from its `pyproject.toml` /
    /// falling back to `fallback` for roots without a config
    /// file.
    ///
    /// [CHKARCH-VERSION-TARGET] An explicit `python-version` in the config wins;
    /// otherwise the project's target is detected from `.python-version` /
    /// `requires-python` / `uv.lock` so version-aware rules follow the real
    /// target (issue #93).
    fn load_root_configs(
        roots: &[PathBuf],
        fallback: &BasiliskConfig,
    ) -> std::collections::HashMap<PathBuf, BasiliskConfig> {
        roots
            .iter()
            .map(|root| {
                // [CHKARCH-CONFIG-DISCOVERY] A root without its own config
                // file still discovers ancestor configs (e.g. a workspace
                // folder opened inside a project) — GitHub #311.
                let has_config = basilisk_config::discover_config_dir(root).is_some();
                let mut cfg = if has_config {
                    basilisk_config::load_basilisk_config(root)
                } else {
                    fallback.clone()
                };
                cfg.project_root = Some(root.clone());
                if cfg.python_version.is_none() {
                    cfg.python_version =
                        basilisk_uv::python_version::resolve_target_python_version(root);
                }
                if cfg.python_platform.is_none() {
                    let analysis_config = crate::config::load_config(root);
                    cfg.python_platform = crate::debug::python_platform_evidence(
                        analysis_config.python_interpreter.as_deref(),
                    )
                    .or_else(|| fallback.python_platform.clone());
                }
                (root.clone(), cfg)
            })
            .collect()
    }

    /// Re-read every root's `BasiliskConfig` from disk so a change to a watched
    /// config file (`pyproject.toml` / `.python-version`)
    /// takes effect — version-aware rules and severity overrides — without an
    /// LSP restart. The caller re-checks open files afterwards (e.g. via
    /// [`Self::recheck_all_files`]). Implements [CHKARCH-VERSION-TARGET].
    pub fn reload_root_configs(&mut self) {
        self.root_configs = Self::load_root_configs(&self.roots, &self.checker_config);
        // Discovered per-directory configs derive from disk state — drop them
        // so the next lookup re-walks ([CHKARCH-CONFIG-DISCOVERY]).
        if let Ok(mut dir_configs) = self.dir_configs.write() {
            dir_configs.clear();
        }
    }

    /// Replace one root's checker config with an already-resolved document
    /// (a live configuration buffer or a freshly observed disk edit) and drop
    /// the discovered per-directory config cache so the next lookup re-merges
    /// instead of serving the stale entry. Implements [LSPARCH-CONFIG]
    /// via [CHKARCH-CONFIG-DISCOVERY].
    pub fn set_root_config(&mut self, root: PathBuf, config: BasiliskConfig) {
        let _ = self.root_configs.insert(root, config);
        if let Ok(mut dir_configs) = self.dir_configs.write() {
            dir_configs.clear();
        }
    }

    /// Cache the import search paths built during the workspace scan.
    ///
    /// Subsequent incremental analyses (`didOpen` / `didChange` / disk reload)
    /// resolve imports against these so the editor's diagnostics match the
    /// full-scan diagnostics. Implements [ANALYSIS-INCR-IMPORTS].
    pub fn set_search_paths(&self, search_paths: crate::import_resolver::ImportSearchPaths) {
        let search_paths = Arc::new(search_paths);
        let mut by_root = std::collections::HashMap::new();
        let roots = if self.roots.is_empty() {
            search_paths.roots.clone()
        } else {
            self.roots.clone()
        };
        if roots.is_empty() {
            let _ = by_root.insert(PathBuf::new(), Arc::clone(&search_paths));
        } else {
            for root in roots {
                let _ = by_root.insert(root, Arc::clone(&search_paths));
            }
        }
        self.set_search_paths_by_root(by_root);
    }

    /// Atomically replace all per-root import environments.
    pub fn set_search_paths_by_root(
        &self,
        search_paths: std::collections::HashMap<
            PathBuf,
            Arc<crate::import_resolver::ImportSearchPaths>,
        >,
    ) {
        if let Ok(mut guard) = self.search_paths_by_root.write() {
            *guard = search_paths;
        }
    }

    /// Cache one root's import environment without disturbing other roots.
    fn set_root_search_paths(
        &self,
        root: PathBuf,
        search_paths: crate::import_resolver::ImportSearchPaths,
    ) {
        if let Ok(mut guard) = self.search_paths_by_root.write() {
            let _ = guard.insert(root, Arc::new(search_paths));
        }
    }

    /// Whether at least one scan has installed import search paths.
    #[must_use]
    fn has_search_paths(&self) -> bool {
        self.search_paths_by_root
            .read()
            .is_ok_and(|guard| !guard.is_empty())
    }

    /// Snapshot the import environment for `path` using the longest owning
    /// workspace-root prefix. Nested workspace folders therefore select the
    /// most specific target and never inherit a sibling root's interpreter.
    #[must_use]
    pub(crate) fn search_paths_for_file(
        &self,
        path: &std::path::Path,
    ) -> Option<(PathBuf, Arc<crate::import_resolver::ImportSearchPaths>)> {
        let guard = self.search_paths_by_root.read().ok()?;
        let selected = guard
            .keys()
            .filter(|root| !root.as_os_str().is_empty() && path.starts_with(root))
            .max_by_key(|root| root.components().count())
            .cloned()
            .or_else(|| {
                guard
                    .contains_key(std::path::Path::new(""))
                    .then(PathBuf::new)
            })
            .or_else(|| {
                (guard.len() == 1)
                    .then(|| guard.keys().next().cloned())
                    .flatten()
            })?;
        guard
            .get(&selected)
            .map(|search_paths| (selected, Arc::clone(search_paths)))
    }

    /// Return the longest workspace-root prefix that owns `path`.
    #[must_use]
    fn owning_root_for_path(&self, path: &std::path::Path) -> Option<&PathBuf> {
        self.roots
            .iter()
            .filter(|root| !root.as_os_str().is_empty() && path.starts_with(root))
            .max_by_key(|root| root.components().count())
    }

    /// Whether `root` is the longest-prefix owner of `path`.
    #[must_use]
    pub(crate) fn path_is_owned_by_root(
        &self,
        path: &std::path::Path,
        root: &std::path::Path,
    ) -> bool {
        self.owning_root_for_path(path)
            .is_some_and(|owner| owner == root)
    }

    fn path_is_owned_by_any_root(&self, path: &std::path::Path, roots: &[PathBuf]) -> bool {
        self.owning_root_for_path(path)
            .is_some_and(|owner| roots.iter().any(|root| root == owner))
    }

    /// Run the analysis pipeline for one file, then resolve its imports against
    /// the cached search paths and re-check.
    ///
    /// When no search paths are cached yet (before the first scan completes),
    /// this is identical to a plain parse → resolve → check. Once the scan has
    /// populated the search paths, incremental edits resolve third-party and
    /// workspace imports exactly like the full scan — without this, every
    /// `didOpen` / `didChange` re-marks imports `Unresolved`, resurrecting
    /// false `imports_unresolved` in the editor for packages the CLI resolves fine.
    /// Implements [ANALYSIS-INCR-IMPORTS].
    fn analyse_and_resolve(
        &self,
        text: &str,
        path: &std::path::Path,
    ) -> (FileEntry, Vec<tower_lsp::lsp_types::Diagnostic>) {
        let config = self.config_for_file(path);

        // Before the first scan populates the search paths, keep the import-free
        // path (no salsa engine): it matches the pre-scan behaviour exactly and
        // avoids marking every third-party import unresolved.
        let Some((search_paths_root, search_paths)) = self.search_paths_for_file(path) else {
            let (mut entry, lsp_diags) = analyse_with_config(text, path, &config);
            if self.is_path_excluded(path) || self.is_outside_include_roots(path) {
                entry.diagnostics.clear();
                return (entry, Vec::new());
            }
            return (entry, lsp_diags);
        };

        // Search paths are known: run the memoized salsa engine, which resolves
        // imports so third-party/workspace imports match the bulk scan and
        // `basilisk check`. In cross-module mode the engine's cross queries also
        // populate `imported_symbols` from the other tracked files' current
        // content, so cross-file diagnostics and navigation stay live.
        // Implements [ANALYSIS-INCR-IMPORTS] via [CHKARCH-INCREMENTAL-SALSA].
        let root_key = Self::config_root_key(path);
        let cross_module = matches!(self.mode(), AnalysisMode::CrossModule);
        let analysis = self.salsa_engine.analyse(
            path,
            text,
            crate::salsa_engine::AnalysisInputs {
                config: &config,
                config_key: &root_key,
                search_paths_root: &search_paths_root,
                search_paths: &search_paths,
            },
            cross_module,
        );
        let hash = fnv1a(text);

        // Parse failure: surface BSK-PARSE, no navigable module (matches the
        // non-salsa path).
        if let Some(message) = analysis.parse_error {
            return (
                make_entry(hash, text, None, Vec::new()),
                vec![parse_error_diagnostic(&message)],
            );
        }

        let mut entry = make_entry(hash, text, analysis.resolved, analysis.diagnostics);

        // Excluded files, and files outside the configured `include` roots, are
        // parsed so navigation still works but never contribute diagnostics — the
        // per-file path must match the bulk scan and `basilisk check`, which skip
        // them. Without this, opening a `bundled/` or out-of-include file
        // squiggles every line. Implements [CHKARCH-CONFIG-EXCLUDE] /
        // [CHKARCH-CONFIG-INCLUDE].
        if self.is_path_excluded(path) || self.is_outside_include_roots(path) {
            entry.diagnostics.clear();
            return (entry, Vec::new());
        }

        let lsp_diags = entry
            .diagnostics
            .iter()
            .map(|d| bsk_to_lsp(d, text))
            .collect();
        (entry, lsp_diags)
    }

    /// The config-input key for `file_path`: its own directory. Files in one
    /// directory share a merged discovered config
    /// ([CHKARCH-CONFIG-DISCOVERY]), hence one salsa `ConfigInput`; a shared
    /// per-root key would let files with different child-dir configs thrash a
    /// single input.
    #[must_use]
    fn config_root_key(file_path: &std::path::Path) -> PathBuf {
        file_path.parent().unwrap_or(file_path).to_path_buf()
    }

    /// Get the checker config for a file.
    ///
    /// Implements [CHKARCH-CONFIG-DISCOVERY] (GitHub #311): the owning root's
    /// config (falling back to `checker_config`) merged with configs
    /// discovered on the file's own ancestor chain, so a child directory's
    /// config applies exactly as in `basilisk check`. Memoized per directory;
    /// `reload_root_configs` clears the cache.
    #[must_use]
    pub fn config_for_file(&self, file_path: &std::path::Path) -> Arc<BasiliskConfig> {
        let dir = Self::config_root_key(file_path);
        let cached = self
            .dir_configs
            .read()
            .ok()
            .and_then(|cache| cache.get(&dir).cloned());
        if let Some(hit) = cached {
            return hit;
        }

        // Find the longest matching root (most specific) for the base config,
        // then merge the discovered ancestor chain over it. Inside a root the
        // walk stops BELOW the root ([LSPARCH-CONFIG]): the in-memory root
        // config is the authoritative effective config — an applied
        // configuration-editor change or an open, unsaved config buffer beats
        // disk — so re-reading the root's file here would resurrect stale
        // disk state over it.
        let owning_root = self
            .roots
            .iter()
            .filter(|root| file_path.starts_with(root))
            .max_by_key(|root| root.components().count());
        let base = owning_root
            .and_then(|root| self.root_configs.get(root))
            .unwrap_or(&self.checker_config)
            .clone();
        let discovered = owning_root.map_or_else(
            || basilisk_config::load_basilisk_config(&dir),
            |root| basilisk_config::load_basilisk_config_below(&dir, root),
        );
        let merged = Arc::new(base.merged_with(discovered));
        if let Ok(mut cache) = self.dir_configs.write() {
            let _ = cache.insert(dir, Arc::clone(&merged));
        }
        merged
    }

    /// Whether `file_path` matches the owning root's `exclude` patterns.
    ///
    /// Uses the same gitignore-style matcher as the workspace scan
    /// (`basilisk_config::path_matches_pattern`, relative to the owning root),
    /// so the incremental per-file path agrees with the bulk scan on which
    /// vendored/bundled files are skipped. Implements [CHKARCH-CONFIG-EXCLUDE].
    #[must_use]
    fn is_path_excluded(&self, file_path: &std::path::Path) -> bool {
        let Some(root) = self
            .roots
            .iter()
            .filter(|root| file_path.starts_with(root))
            .max_by_key(|root| root.components().count())
        else {
            return false;
        };
        let config = self.root_configs.get(root).unwrap_or(&self.checker_config);
        let relative = file_path.strip_prefix(root).unwrap_or(file_path);
        config
            .exclude
            .iter()
            .any(|pattern| basilisk_config::path_matches_pattern(relative, pattern))
    }

    /// Whether `file_path` lies outside the owning root's `[tool.basilisk]
    /// include` roots. When `include` is empty the whole root is included, so
    /// nothing is "outside". Mirrors the scan's `scan_dirs_for` for the per-file
    /// path, so an opened file outside the include roots is suppressed just like
    /// an excluded one. Implements [CHKARCH-CONFIG-INCLUDE].
    #[must_use]
    fn is_outside_include_roots(&self, file_path: &std::path::Path) -> bool {
        let Some(root) = self
            .roots
            .iter()
            .filter(|root| file_path.starts_with(root))
            .max_by_key(|root| root.components().count())
        else {
            return false;
        };
        let Some(config) = self.root_configs.get(root) else {
            return false;
        };
        if config.include.is_empty() {
            return false;
        }
        !config
            .include
            .iter()
            .any(|inc| file_path.starts_with(root.join(inc)))
    }

    /// Whether configuration impact analysis should include `file_path`.
    ///
    /// The configuration editor rechecks retained open documents under a
    /// hypothetical rule policy. It must preserve the same include/exclude
    /// boundary as normal analysis instead of resurrecting diagnostics for a
    /// document that is indexed only for navigation.
    #[must_use]
    pub(crate) fn configuration_path_is_in_scope(&self, file_path: &std::path::Path) -> bool {
        !self.is_path_excluded(file_path) && !self.is_outside_include_roots(file_path)
    }

    /// How many files the in-session Salsa engine currently holds memos for
    /// ([CHKARCH-INCREMENTAL-SALSA]).
    ///
    /// Reported by the configuration editor's caching panel ([LSPCFGED-CACHE]):
    /// the in-session layer has no configuration key, so this live count is
    /// the only evidence a reader has that it is running at all.
    #[must_use]
    pub(crate) fn tracked_source_count(&self) -> usize {
        self.salsa_engine.tracked_source_count()
    }

    /// Return the `FileEntry` for a URI, if present.
    ///
    /// Canonicalizes the path to handle macOS `/var` → `/private/var` symlinks
    /// and other platform symlink differences.
    #[must_use]
    pub fn get_by_uri(
        &self,
        uri: &Url,
    ) -> Option<(
        String,
        Arc<basilisk_resolver::ResolvedModule>,
        Vec<basilisk_checker::Diagnostic>,
    )> {
        let entry = self.entry_for_uri(uri)?;
        let resolved = entry.resolved.clone()?;
        let text = entry.text.clone();
        let diagnostics = entry.diagnostics.clone();
        Some((text, resolved, diagnostics))
    }

    /// The indexed entry for a URI, trying the literal path first and then the
    /// canonicalized one (macOS `/var` → `/private/var` and other symlinks).
    fn entry_for_uri(
        &self,
        uri: &Url,
    ) -> Option<dashmap::mapref::one::Ref<'_, PathBuf, FileEntry>> {
        let path = uri.to_file_path().ok()?;
        self.files.get(&path).or_else(|| {
            let canonical = path.canonicalize().ok()?;
            self.files.get(&canonical)
        })
    }

    /// The text and resolved module a DISPLAY surface should render, falling
    /// back to the last revision that parsed when the current one does not.
    ///
    /// A buffer stops parsing on the way through almost every edit — typing `.`
    /// to reach an attribute leaves a mid-token line for one keystroke — and
    /// blanking a file's hints because of it discards work that is still
    /// correct for every line the user is not on (GitHub #386). The returned
    /// text is whichever revision the module was resolved from, never a mix.
    ///
    /// Diagnostics deliberately have no equivalent: a stale error under the
    /// cursor is a wrong claim about the code, whereas a stale hint is a
    /// momentarily out-of-date description of a line nobody is editing.
    /// Implements [ANALYSIS-INDEX-LASTGOOD].
    #[must_use]
    pub fn get_for_display(
        &self,
        uri: &Url,
    ) -> Option<(String, Arc<basilisk_resolver::ResolvedModule>)> {
        let entry = self.entry_for_uri(uri)?;
        if let Some(resolved) = entry.resolved.clone() {
            return Some((entry.text.clone(), resolved));
        }
        entry
            .last_good
            .as_ref()
            .map(|last| (last.text.clone(), Arc::clone(&last.resolved)))
    }

    /// Store an entry, carrying the last parsing revision forward.
    ///
    /// The single write path into [`Self::files`], so a revision that fails to
    /// parse can never erase the snapshot display surfaces fall back to
    /// ([`Self::get_for_display`]). Implements [ANALYSIS-INDEX-LASTGOOD].
    ///
    /// Only OPEN buffers carry a snapshot. The snapshot costs a second copy of
    /// the file's text, and only a buffer someone is typing in can reach a
    /// non-parsing revision — so the cost scales with open editor tabs, not
    /// with workspace size, and closing a file frees it.
    fn store_entry(&self, path: PathBuf, mut entry: FileEntry) {
        entry.last_good = match (entry.is_open, entry.resolved.clone()) {
            (false, _) => None,
            // This revision resolved — it becomes the snapshot.
            (true, Some(resolved)) => Some(Arc::new(LastGoodResolve {
                text: entry.text.clone(),
                resolved,
            })),
            // It did not: keep whatever the previous revision left behind.
            (true, None) => entry.last_good.take().or_else(|| {
                self.files
                    .get(&path)
                    .and_then(|prev| prev.last_good.clone())
            }),
        };
        let _ = self.files.insert(path, entry);
    }

    /// Return just the source text for a URI (used by handlers that don't need
    /// the resolved module, e.g. formatting and code actions).
    ///
    /// Returns the raw text even when parsing/resolving failed, so that
    /// handlers like completion can attempt their own recovery.
    #[must_use]
    pub fn get_text(&self, uri: &Url) -> Option<String> {
        Some(self.entry_for_uri(uri)?.text.clone())
    }

    /// Analyse a file from in-memory text (called on `didOpen` / `didChange`).
    ///
    /// Marks the file as open and updates the index. Returns the LSP
    /// diagnostics ready for publishing.
    // Implements [ANALYSIS-OPEN] (per-open-file analysis) and [ANALYSIS-INCR-CHANGE]
    // (in-memory text is authoritative; parse → resolve → check runs on the edit).
    #[must_use]
    pub fn set_open(
        &self,
        uri: &Url,
        text: &str,
        version: i32,
    ) -> Vec<tower_lsp::lsp_types::Diagnostic> {
        let path = uri.to_file_path().unwrap_or_default();

        // The analysis is authoritative: once search paths are known, the salsa
        // engine resolves imports and (in cross-module mode) populates
        // `imported_symbols` from the tracked files' current content — there is
        // no stale prior state to restore, and carrying one forward would keep
        // suppressing diagnostics for imports the edit just removed.
        let (entry, lsp_diags) = self.analyse_and_resolve(text, &path);
        let mut entry = entry;
        entry.is_open = true;
        entry.version = version;

        self.store_entry(path, entry);
        lsp_diags
    }

    /// Track an open buffer without parsing or checking it.
    ///
    /// Used while the owning root has no gate-accepted Typeshed generation.
    /// The text remains authoritative and is analysed when that root becomes
    /// Ready; no fallback generation is allowed to produce interim results.
    pub(crate) fn set_open_without_analysis(&self, uri: &Url, text: &str, version: i32) {
        let path = uri.to_file_path().unwrap_or_default();
        self.store_entry(
            path,
            FileEntry {
                source_hash: fnv1a(text),
                text: text.to_owned(),
                resolved: None,
                diagnostics: Vec::new(),
                version,
                is_open: true,
                last_good: None,
            },
        );
    }

    /// Mirror client-accepted LSP edits into the analysis index and reanalyse.
    ///
    /// Open-buffer text and its version remain authoritative if a newer
    /// `didChange` races the edit response. Otherwise the exact accepted edit
    /// set is applied to the source snapshot used to construct the request.
    /// Implements [ANALYSIS-INDEX-OPEN] and [AUTOFIX-CONFLICTS].
    #[must_use]
    pub fn apply_accepted_text_edits(
        &self,
        uri: &Url,
        original: &str,
        edits: &[TextEdit],
    ) -> Option<Vec<tower_lsp::lsp_types::Diagnostic>> {
        let path = self.indexed_path(uri)?;
        let (current, version, is_open) = self.entry_authority(&path)?;
        let edited = apply_non_overlapping_edits(original, edits)?;
        let next = authoritative_edited_text(current, original, edited, is_open);
        let (mut entry, diagnostics) = self.analyse_and_resolve(&next, &path);
        entry.version = version;
        entry.is_open = is_open;
        self.store_entry(path, entry);
        Some(diagnostics)
    }

    fn indexed_path(&self, uri: &Url) -> Option<PathBuf> {
        let path = uri.to_file_path().ok()?;
        if self.files.contains_key(&path) {
            return Some(path);
        }
        let canonical = path.canonicalize().ok()?;
        self.files.contains_key(&canonical).then_some(canonical)
    }

    fn entry_authority(&self, path: &std::path::Path) -> Option<(String, i32, bool)> {
        self.files
            .get(path)
            .map(|entry| (entry.text.clone(), entry.version, entry.is_open))
    }

    /// Like [`Self::set_open`], but in cross-module mode also refreshes
    /// dependents when the edited (open) file's exported symbol set changes — so
    /// editing an OPEN module updates its importers live. `set_open` alone
    /// re-analyses only the edited file, and the file-watcher path skips open
    /// files (`reload_from_disk` bails when `is_open`), so without this an
    /// in-editor export edit leaves dependents stale until the file is closed.
    /// Implements [ANALYSIS-SYMBOLS-INVAL] for the open-file path (GitHub #56).
    #[must_use]
    pub fn set_open_refresh_dependents(
        &self,
        uri: &Url,
        text: &str,
        version: i32,
    ) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        let path = uri.to_file_path().unwrap_or_default();
        let track_exports = matches!(self.mode(), AnalysisMode::CrossModule);
        let before = track_exports.then(|| self.exported_symbol_names(&path));
        let own_diags = self.set_open(uri, text, version);
        if before.is_some_and(|prev| self.exported_symbol_names(&path) != prev) {
            // Exports changed: re-resolve + re-check so importers' stale symbol
            // diagnostics refresh without closing the file or reloading the server.
            let mut results = self.reresolve_imports_and_recheck();
            // The edited file must always republish after a didChange — the
            // sweep's changed-only filter would skip it (set_open already
            // stored its fresh diagnostics before the sweep compared).
            if !results.iter().any(|(target, _)| target == uri) {
                results.push((uri.clone(), own_diags));
            }
            return results;
        }
        vec![(uri.clone(), own_diags)]
    }

    /// Re-read a file from disk (called on `didClose` or file-watcher events).
    ///
    /// If the file is currently open, this is a no-op (editor text is
    /// authoritative). Returns `None` if the file could not be read or the
    /// hash is unchanged.
    // Implements [ANALYSIS-INDEX-OPEN] (open files are authoritative — watcher
    // events for an open path are ignored) and [ANALYSIS-INCR-WATCH] /
    // [ANALYSIS-INDEX-INVAL] (skip when `source_hash` unchanged; otherwise
    // re-run the pipeline). The 150 ms debounce is upstream in server/document.rs.
    #[must_use]
    pub fn reload_from_disk(
        &self,
        uri: &Url,
    ) -> Option<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        let path = uri.to_file_path().ok()?;

        // Skip if the editor has the file open.
        if self.files.get(&path).is_some_and(|e| e.is_open) {
            return None;
        }

        let text = std::fs::read_to_string(&path).ok()?;
        let new_hash = fnv1a(&text);

        // Skip if content unchanged.
        if self
            .files
            .get(&path)
            .is_some_and(|e| e.source_hash == new_hash)
        {
            return None;
        }

        let (entry, lsp_diags) = self.analyse_and_resolve(&text, &path);
        self.store_entry(path, entry);
        Some((uri.clone(), lsp_diags))
    }

    /// Mark a file as closed. After this call, file-watcher events for the
    /// path will cause a disk re-read. Returns the disk-based diagnostics.
    #[must_use]
    pub fn set_closed(&self, uri: &Url) -> (Url, Vec<tower_lsp::lsp_types::Diagnostic>) {
        let Some(path) = uri.to_file_path().ok() else {
            return (uri.clone(), vec![]);
        };
        if let Some(mut entry) = self.files.get_mut(&path) {
            entry.is_open = false;
            entry.version = 0;
        }
        // Re-analyse from disk now that the editor is no longer authoritative.
        // If the file no longer exists on disk, remove it from the index and
        // clear its diagnostics (e.g. an in-memory-only test file).
        let Ok(text) = std::fs::read_to_string(&path) else {
            let _ = self.files.remove(&path);
            self.salsa_engine.remove(&path);
            return (uri.clone(), vec![]);
        };
        let (entry, lsp_diags) = self.analyse_and_resolve(&text, &path);
        self.store_entry(path, entry);
        (uri.clone(), lsp_diags)
    }

    /// Drop a file from the index entirely.
    ///
    /// Used when a watched file is deleted on disk so that a subsequent
    /// workspace re-resolution does not resurrect its diagnostics from a stale
    /// entry. Implements [ANALYSIS-INCR-IMPORTS].
    pub fn forget_file(&self, uri: &Url) {
        if let Ok(path) = uri.to_file_path() {
            let _ = self.files.remove(&path);
            self.salsa_engine.remove(&path);
        }
    }

    /// Re-check every indexed file with its current `ResolvedModule` and return
    /// the freshly converted LSP diagnostics keyed by URI.
    ///
    /// Updates each entry's stored diagnostics in place. Shared by the
    /// re-resolution path so a single recheck loop serves every caller.
    #[must_use]
    pub fn recheck_all_files(&self) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        self.files
            .iter_mut()
            .filter_map(|mut entry| {
                // Excluded / out-of-include files never publish diagnostics, even
                // on a re-resolve — otherwise an open out-of-scope file's errors
                // reappear. [CHKARCH-CONFIG-EXCLUDE] / [CHKARCH-CONFIG-INCLUDE].
                let suppressed = self.is_path_excluded(entry.key())
                    || self.is_outside_include_roots(entry.key());
                let resolved = entry.resolved.clone()?;
                let checker_diags = if suppressed {
                    Vec::new()
                } else {
                    basilisk_checker::check_with_config(
                        &resolved,
                        &self.config_for_file(entry.key()),
                    )
                };
                let lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic> = checker_diags
                    .iter()
                    .map(|d| bsk_to_lsp(d, &entry.text))
                    .collect();
                entry.diagnostics = checker_diags;
                let uri = path_to_uri(entry.key())?;
                Some((uri, lsp_diags))
            })
            .collect()
    }

    /// Re-analyse every indexed file through the salsa engine and return fresh
    /// LSP diagnostics keyed by URI.
    ///
    /// Called when the resolution environment changes — a new module is
    /// created, an open dependency's exports change, `uv.lock`/config edits —
    /// so stale cross-file state clears without an LSP reload. The engine is
    /// primed with every indexed file's **current** text first (open files
    /// contribute their in-memory buffers), so the cross-file salsa edges see
    /// the whole workspace; each file is then re-analysed through the memoized
    /// queries, and only files whose inputs actually changed recompute — the
    /// rest are revalidated memos ([CHKARCH-INCREMENTAL-SALSA]). When no search
    /// paths are cached yet (before the first scan), degrades to a plain
    /// recheck. Implements [ANALYSIS-INCR-IMPORTS] / [ANALYSIS-SYMBOLS-INVAL].
    #[must_use]
    pub fn reresolve_imports_and_recheck(
        &self,
    ) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        if !self.has_search_paths() {
            return self.recheck_all_files();
        }
        self.salsa_engine.prime(
            self.files
                .iter()
                .map(|entry| (entry.key().clone(), entry.value().text.clone())),
        );

        let paths: Vec<PathBuf> = self.files.iter().map(|entry| entry.key().clone()).collect();
        let results = self.reanalyse_paths(paths, PublishPolicy::ChangedOnly);

        // The import graph serves navigation's reverse lookups (cross-file
        // references / rename); invalidation itself is salsa's job now.
        if matches!(self.mode(), AnalysisMode::CrossModule) {
            self.build_import_graph();
        }
        results
    }

    /// Re-analyse each indexed path through the salsa engine, preserving its
    /// open-state, and return fresh LSP diagnostics per the publish policy.
    ///
    /// [`PublishPolicy::ChangedOnly`] is valid ONLY when the client's
    /// diagnostic state is known to match the server's store (a steady-state
    /// sweep): re-publishing an identical set is then a client no-op, so
    /// skipping it saves O(workspace) publish traffic. When the client may
    /// have diverged (cleared on disable, pre-scan state), use
    /// [`PublishPolicy::Always`].
    fn reanalyse_paths(
        &self,
        paths: Vec<PathBuf>,
        policy: PublishPolicy,
    ) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        let mut results = Vec::new();
        for path in paths {
            let Some((text, version, is_open, prev_diagnostics)) =
                self.files.get(&path).map(|entry| {
                    (
                        entry.text.clone(),
                        entry.version,
                        entry.is_open,
                        entry.diagnostics.clone(),
                    )
                })
            else {
                continue;
            };
            let (mut entry, lsp_diags) = self.analyse_and_resolve(&text, &path);
            entry.version = version;
            entry.is_open = is_open;
            let publish = match policy {
                PublishPolicy::Always => true,
                PublishPolicy::ChangedOnly => entry.diagnostics != prev_diagnostics,
            };
            self.store_entry(path.clone(), entry);
            if publish {
                if let Some(uri) = path_to_uri(&path) {
                    results.push((uri, lsp_diags));
                }
            }
        }
        results
    }

    /// Reload one file from disk, reporting whether its exported top-level
    /// symbol set changed. Implements [ANALYSIS-SYMBOLS-INVAL] (GitHub #56).
    pub fn reload_and_diff_exports(
        &self,
        uri: &Url,
    ) -> Option<((Url, Vec<tower_lsp::lsp_types::Diagnostic>), bool)> {
        let path = uri.to_file_path().ok()?;
        let before = self.exported_symbol_names(&path);
        let result = self.reload_from_disk(uri)?;
        let changed = self.exported_symbol_names(&path) != before;
        Some((result, changed))
    }

    /// The directories to scan under `root`: the configured `[tool.basilisk]
    /// include` roots (relative to `root`) if any, else `root` itself. Mirrors
    /// the CLI's `effective_check_paths` so the editor and `basilisk check`
    /// agree on which files are analysed. Implements [CHKARCH-CONFIG-INCLUDE].
    fn scan_dirs_for(&self, root: &std::path::Path) -> Vec<PathBuf> {
        match self.root_configs.get(root) {
            Some(cfg) if !cfg.include.is_empty() => {
                cfg.include.iter().map(|inc| root.join(inc)).collect()
            }
            _ => vec![root.to_path_buf()],
        }
    }

    /// Scan all workspace roots and populate the index.
    ///
    /// Returns a list of `(Uri, diagnostics)` pairs ready for publishing.
    /// Files already open in the editor are skipped (their in-memory text is
    /// authoritative and already analysed).
    ///
    /// When the import search paths are cached (the caller sets them before
    /// scanning — see `scan_resolve_and_check_with_roots`), every file's text
    /// is read up front and the salsa engine is **primed with the whole
    /// workspace before the first analysis**, so cross-file edges see every
    /// file from the first query and each file is parsed exactly once —
    /// through the same memoized queries every later edit uses. Without search
    /// paths (unit tests, pre-config scans) each file falls back to the
    /// import-free direct pipeline, exactly as before.
    // Implements [ANALYSIS-STARTUP-WHOLE] — collects all `.py`/`.pyi` under the
    // roots (respecting include/exclude), analyses them, and returns diagnostics
    // for every file — via [CHKARCH-INCREMENTAL-SALSA] once search paths exist.
    // The crossModule import graph is wired in server/init.rs.
    #[must_use]
    pub fn scan(
        &self,
    ) -> (
        Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)>,
        usize,
        usize,
    ) {
        self.scan_roots(&self.roots)
    }

    /// Scan only files whose longest-prefix owning root is in `roots`.
    ///
    /// This keeps a blocked nested root out of a healthy parent-root scan while
    /// allowing the healthy root to continue independently.
    #[must_use]
    pub(crate) fn scan_roots(
        &self,
        roots: &[PathBuf],
    ) -> (
        Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)>,
        usize,
        usize,
    ) {
        let mut all_files: Vec<PathBuf> = Vec::new();

        for root in roots {
            let cfg = crate::config::load_config(root);
            for scan_dir in self.scan_dirs_for(root) {
                collect_python_files(&scan_dir, &mut all_files, &cfg.exclude, root);
            }
        }

        // Prefer .pyi over .py when both exist for the same stem.
        let deduped = deduplicate_by_stem(all_files)
            .into_iter()
            .filter(|path| self.path_is_owned_by_any_root(path, roots))
            .collect::<Vec<_>>();
        let file_count = deduped.len();

        // Read every closed file's text before analysing anything, so the
        // engine can be primed with the complete workspace. Open files keep
        // their in-memory text — already tracked by the engine.
        let to_analyse: Vec<(PathBuf, String)> = deduped
            .into_iter()
            .filter_map(|path| {
                if self.files.get(&path).is_some_and(|e| e.is_open) {
                    return None;
                }
                let text = std::fs::read_to_string(&path).ok()?;
                Some((path, text))
            })
            .collect();

        if self.has_search_paths() {
            self.salsa_engine.prime(
                to_analyse
                    .iter()
                    .map(|(path, text)| (path.clone(), text.clone())),
            );
        }

        let results: Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> = to_analyse
            .into_iter()
            .filter_map(|(path, text)| {
                let uri = path_to_uri(&path)?;
                let (entry, lsp_diags) = self.analyse_and_resolve(&text, &path);
                self.store_entry(path, entry);
                Some((uri, lsp_diags))
            })
            .collect();

        let error_count = results
            .iter()
            .map(|(_, diags)| {
                diags
                    .iter()
                    .filter(|d| d.severity == Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR))
                    .count()
            })
            .sum();

        (results, file_count, error_count)
    }

    /// Populate one root for configuration-editor inventory without publishing.
    ///
    /// `openFilesOnly` intentionally skips the startup scan, but project policy
    /// operations still need complete counts and occurrences. Open buffers stay
    /// authoritative; closed files are refreshed from disk and retained only in
    /// the server index. The caller deliberately discards LSP diagnostics.
    #[must_use]
    pub fn preload_root_for_configuration(&self, root: &std::path::Path) -> usize {
        if !self.roots.iter().any(|candidate| candidate == root) {
            return 0;
        }
        self.ensure_configuration_search_paths(root);
        let config = crate::config::load_config(root);
        let mut files = Vec::new();
        for scan_dir in self.scan_dirs_for(root) {
            collect_python_files(&scan_dir, &mut files, &config.exclude, root);
        }
        let files = deduplicate_by_stem(files);
        self.remove_stale_configuration_entries(root, &files);
        let sources = self.closed_sources(files);
        if self.has_search_paths() {
            self.salsa_engine.prime(
                sources
                    .iter()
                    .map(|(path, text)| (path.clone(), text.clone())),
            );
        }
        for (path, text) in sources {
            let (entry, _diagnostics_for_client) = self.analyse_and_resolve(&text, &path);
            self.store_entry(path, entry);
        }
        self.files
            .iter()
            .filter(|entry| entry.key().starts_with(root))
            .count()
    }

    fn ensure_configuration_search_paths(&self, root: &std::path::Path) {
        if self
            .search_paths_by_root
            .read()
            .is_ok_and(|paths| paths.contains_key(root))
        {
            return;
        }
        let roots = self.roots.clone();
        let config = crate::config::load_config(root);
        let search_paths =
            crate::server::init::build_root_search_paths(&roots, root, config, None, None);
        self.set_root_search_paths(root.to_path_buf(), search_paths);
    }

    fn remove_stale_configuration_entries(&self, root: &std::path::Path, files: &[PathBuf]) {
        let expected: std::collections::HashSet<&std::path::Path> =
            files.iter().map(PathBuf::as_path).collect();
        self.files.retain(|path, entry| {
            !path.starts_with(root) || entry.is_open || expected.contains(path.as_path())
        });
    }

    fn closed_sources(&self, files: Vec<PathBuf>) -> Vec<(PathBuf, String)> {
        files
            .into_iter()
            .filter(|path| !self.files.get(path).is_some_and(|entry| entry.is_open))
            .filter_map(|path| {
                let text = std::fs::read_to_string(&path).ok()?;
                Some((path, text))
            })
            .collect()
    }

    /// Re-analyse every OPEN file through the engine and return its fresh
    /// diagnostics — **always**, even when they are unchanged.
    ///
    /// The scan skips open files (editor text is authoritative), but their
    /// previous diagnostics were computed under different conditions (before
    /// the search paths existed, or before type checking was re-enabled and
    /// the client's diagnostics were cleared) — so the client's state may have
    /// diverged from the server's store and a changed-only filter would leave
    /// the editor stale. Called after the startup scan and mode/enable
    /// rescans so open editors converge with the workspace.
    #[must_use]
    pub fn refresh_open_files(&self) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        self.refresh_open_files_for_roots(&self.roots)
    }

    /// Re-analyse open files owned by one of `roots`.
    #[must_use]
    pub(crate) fn refresh_open_files_for_roots(
        &self,
        roots: &[PathBuf],
    ) -> Vec<(Url, Vec<tower_lsp::lsp_types::Diagnostic>)> {
        let open_paths: Vec<PathBuf> = self
            .files
            .iter()
            .filter(|entry| {
                entry.value().is_open && self.path_is_owned_by_any_root(entry.key(), roots)
            })
            .map(|entry| entry.key().clone())
            .collect();
        self.reanalyse_paths(open_paths, PublishPolicy::Always)
    }

    /// Collect all `(uri, resolved, text)` triples currently in the index,
    /// used by workspace symbol search.
    #[must_use]
    pub fn all_resolved(&self) -> Vec<(Url, Arc<basilisk_resolver::ResolvedModule>, String)> {
        self.files
            .iter()
            .filter_map(|entry| {
                let path = entry.key().clone();
                let resolved = entry.resolved.clone()?;
                let text = resolved.source.clone();
                let uri = path_to_uri(&path)?;
                Some((uri, resolved, text))
            })
            .collect()
    }

    /// Build (or rebuild) the import graph from the current index state.
    ///
    /// Called after workspace scan or when the analysis mode is `CrossModule`.
    // Implements [ANALYSIS-GRAPH-BUILD] (rebuilds the graph from the index) and
    // the import-graph step of [ANALYSIS-STARTUP-CROSS].
    pub fn build_import_graph(&self) {
        let Ok(mut graph) = self.import_graph.lock() else {
            return;
        };
        *graph = ImportGraph::new();
        graph.build_from_index(self);
    }

    /// Map uv workspace members to LSP workspace folder URIs.
    ///
    /// Parses `[tool.uv.workspace]` from `pyproject.toml` at each workspace
    /// root, resolves member paths, and converts them to `lsp_types::WorkspaceFolder`
    /// entries. This enables multi-root LSP features (diagnostics, navigation)
    /// to work seamlessly across workspace members.
    ///
    /// Returns an empty vec if no uv workspace is configured.
    #[must_use]
    pub fn workspace_member_folders(&self) -> Vec<tower_lsp::lsp_types::WorkspaceFolder> {
        let mut folders = Vec::new();

        for root in &self.roots {
            let Ok(Some(workspace)) = basilisk_uv::parse_uv_workspace(root) else {
                continue;
            };

            for member_dir in &workspace.members {
                let Some(uri) = Url::from_file_path(member_dir).ok() else {
                    continue;
                };
                let name = member_dir
                    .file_name()
                    .map_or_else(|| uri.to_string(), |n| n.to_string_lossy().into_owned());

                folders.push(tower_lsp::lsp_types::WorkspaceFolder { uri, name });
            }
        }

        folders
    }

    /// Extract the set of exported symbol names from a file.
    ///
    /// Used for diffing exports before and after a re-analysis to determine
    /// whether dependents need invalidation.
    #[must_use]
    pub fn exported_symbol_names(
        &self,
        path: &std::path::Path,
    ) -> std::collections::HashSet<String> {
        let mut names = std::collections::HashSet::new();
        let Some(entry) = self.files.get(path) else {
            return names;
        };
        let Some(resolved) = &entry.resolved else {
            return names;
        };
        for func in &resolved.functions {
            let _ = names.insert(func.name.clone());
        }
        for class in &resolved.classes {
            let _ = names.insert(class.name.clone());
        }
        for var in &resolved.module_vars {
            let _ = names.insert(var.name.clone());
        }
        names
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::config::AnalysisMode;
    use crate::workspace_analysis::fnv1a;
    use crate::workspace_analysis::resolve_analysis_mode;
    use crate::workspace_scan::{deduplicate_by_stem, is_excluded};

    static TEST_CTR: AtomicU64 = AtomicU64::new(0);

    /// Generate a unique temp dir path to avoid races between parallel tests.
    fn unique_tmp(prefix: &str) -> std::path::PathBuf {
        let n = TEST_CTR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("{prefix}_{n}_{}", std::process::id()))
    }

    #[test]
    fn test_fnv1a_differs_for_different_strings() {
        assert_ne!(fnv1a("hello"), fnv1a("world"));
    }

    #[test]
    fn test_fnv1a_stable() {
        // Must be deterministic across calls.
        assert_eq!(fnv1a("basilisk"), fnv1a("basilisk"));
    }

    #[test]
    fn test_deduplicate_prefers_pyi() {
        let files = vec![
            PathBuf::from("/workspace/foo.py"),
            PathBuf::from("/workspace/foo.pyi"),
            PathBuf::from("/workspace/bar.py"),
        ];
        let deduped = deduplicate_by_stem(files);
        let has_pyi = deduped
            .iter()
            .any(|p| p.extension().is_some_and(|e| e == "pyi"));
        let has_py_foo = deduped
            .iter()
            .any(|p| p.file_name().is_some_and(|n| n == "foo.py"));
        assert!(has_pyi, "should have kept foo.pyi");
        assert!(!has_py_foo, "should have dropped foo.py");
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_is_excluded() {
        let root = PathBuf::from("/ws");
        let exclude = vec![PathBuf::from("vendor"), PathBuf::from("build")];
        assert!(is_excluded(
            &PathBuf::from("/ws/vendor/lib.py"),
            &exclude,
            &root
        ));
        assert!(!is_excluded(
            &PathBuf::from("/ws/src/main.py"),
            &exclude,
            &root
        ));
    }

    #[test]
    fn test_resolve_analysis_mode_from_init_options() {
        let opts = serde_json::json!({ "analysisMode": "openFilesOnly" });
        let mode = resolve_analysis_mode(Some(&opts), &[]);
        assert_eq!(mode, AnalysisMode::OpenFilesOnly);
    }

    #[test]
    fn test_resolve_analysis_mode_default() {
        let mode = resolve_analysis_mode(None, &[]);
        assert_eq!(mode, AnalysisMode::WholeModule);
    }

    #[test]
    fn test_resolve_analysis_mode_cross_module() {
        let opts = serde_json::json!({ "analysisMode": "crossModule" });
        let mode = resolve_analysis_mode(Some(&opts), &[]);
        assert_eq!(mode, AnalysisMode::CrossModule);
    }

    #[test]
    fn test_resolve_analysis_mode_whole_module_explicit() {
        let opts = serde_json::json!({ "analysisMode": "wholeModule" });
        let mode = resolve_analysis_mode(Some(&opts), &[]);
        assert_eq!(mode, AnalysisMode::WholeModule);
    }

    #[test]
    fn test_resolve_analysis_mode_unknown_falls_back_to_whole() {
        let opts = serde_json::json!({ "analysisMode": "bogusMode" });
        let mode = resolve_analysis_mode(Some(&opts), &[]);
        assert_eq!(mode, AnalysisMode::WholeModule);
    }

    // ── WorkspaceIndex set_open / get_text ───────────────────────────────────

    fn make_index() -> WorkspaceIndex {
        WorkspaceIndex::new(vec![], AnalysisMode::WholeModule, BasiliskConfig::default())
    }

    fn make_uri(path: &str) -> tower_lsp::lsp_types::Url {
        tower_lsp::lsp_types::Url::parse(&format!("file://{path}")).unwrap()
    }

    #[test]
    fn test_set_open_stores_text_even_on_parse_error() {
        let idx = make_index();
        let uri = make_uri("/tmp/broken.py");
        // Trailing dot is a syntax error.
        let src = "class Dog:\n    pass\n\nDog.";
        let _ = idx.set_open(&uri, src, 1);
        // get_text must return the raw text even though parsing failed.
        let text = idx.get_text(&uri).unwrap();
        assert_eq!(text, src);
    }

    #[test]
    fn test_set_open_stores_text_on_success() {
        let idx = make_index();
        let uri = make_uri("/tmp/valid.py");
        let src = "def foo(x: int) -> int:\n    return x\n";
        let _ = idx.set_open(&uri, src, 1);
        let text = idx.get_text(&uri).unwrap();
        assert_eq!(text, src);
    }

    #[test]
    fn test_set_open_marks_is_open() {
        let idx = make_index();
        let uri = make_uri("/tmp/open.py");
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);
        let path = uri.to_file_path().unwrap();
        let entry = idx.files.get(&path).unwrap();
        assert!(entry.is_open);
        assert_eq!(entry.version, 1);
    }

    // Exercises [ANALYSIS-OPEN] / [ANALYSIS-INCR-CHANGE]: per-open-file analysis
    // runs the pipeline on in-memory text and publishes its diagnostics.
    #[test]
    fn test_set_open_produces_diagnostics_for_type_error() {
        let idx = make_index_with_config(annotations_on());
        let uri = make_uri("/tmp/err.py");
        // Missing return type annotation (BSK-0002) — a house rule, off by
        // default, so the index opts in. See [CHKARCH-CONFIGURATION-ONLY].
        let src = "def foo(x: int):\n    return x\n";
        let diags = idx.set_open(&uri, src, 1);
        assert!(
            !diags.is_empty(),
            "expected diagnostics for missing return annotation"
        );
    }

    #[test]
    fn blocked_open_tracks_text_without_running_analysis() {
        let index = make_index_with_config(annotations_on());
        let uri = make_uri("/tmp/blocked.py");
        let source = "def missing_annotations(value):\n    return value\n";
        index.set_open_without_analysis(&uri, source, 7);

        assert_eq!(index.get_text(&uri).as_deref(), Some(source));
        let path = uri.to_file_path().unwrap();
        let entry = index.files.get(&path).unwrap();
        assert!(entry.is_open);
        assert_eq!(entry.version, 7);
        assert!(entry.resolved.is_none());
        assert!(entry.diagnostics.is_empty());
    }

    // ── Issue #80 (editor): opening a vendored/excluded file must NOT publish
    //    diagnostics. Fix #80 excluded `bundled`/`_vendored` from the workspace
    //    *scan*, but the per-file path (didOpen/didChange -> set_open ->
    //    analyse_and_resolve) ignored `exclude` and squiggled any opened file.
    //    The editor must match the scan and `basilisk check`.
    #[test]
    fn test_set_open_excluded_file_publishes_no_diagnostics() {
        let root = unique_tmp("bsk_excluded_open");
        // House rules enabled (so the vendored file WOULD fire if not excluded),
        // keeping DEFAULT_EXCLUDES (`bundled` / `_vendored`). This proves the
        // exclusion — not an off-by-default rule — is what suppresses diagnostics.
        // See [CHKARCH-CONFIGURATION-ONLY].
        let idx = WorkspaceIndex::new(
            vec![root.clone()],
            AnalysisMode::WholeModule,
            annotations_on(),
        );
        // A vendored file with blatant type errors that WOULD normally fire.
        let vendored = root.join("bundled").join("debugpy").join("vendored.py");
        let uri = Url::from_file_path(&vendored).unwrap();
        let diags = idx.set_open(&uri, "def f(x):\n    return x\n", 1);
        assert!(
            diags.is_empty(),
            "opening an excluded (bundled/) file must publish no diagnostics, got: {diags:?}"
        );
    }

    // Complement: a non-excluded file under the same root must STILL be checked,
    // so the exclusion is specific rather than disabling diagnostics wholesale.
    #[test]
    fn test_set_open_non_excluded_file_under_root_still_publishes() {
        let root = unique_tmp("bsk_included_open");
        // House rules enabled so a non-excluded file has something to publish.
        // See [CHKARCH-CONFIGURATION-ONLY].
        let idx = WorkspaceIndex::new(
            vec![root.clone()],
            AnalysisMode::WholeModule,
            annotations_on(),
        );
        let src_file = root.join("src").join("app.py");
        let uri = Url::from_file_path(&src_file).unwrap();
        let diags = idx.set_open(&uri, "def f(x):\n    return x\n", 1);
        assert!(
            !diags.is_empty(),
            "a non-excluded file under the root must still be checked, got none"
        );
    }

    #[test]
    fn test_get_text_missing_uri_returns_none() {
        let idx = make_index();
        let uri = make_uri("/tmp/nonexistent.py");
        assert!(idx.get_text(&uri).is_none());
    }

    #[test]
    fn test_get_by_uri_returns_none_on_parse_error() {
        // When the file fails to parse, resolved is None, so get_by_uri returns None.
        let idx = make_index();
        let uri = make_uri("/tmp/bad.py");
        let src = "class Dog:\n    pass\n\nDog.";
        let _ = idx.set_open(&uri, src, 1);
        assert!(
            idx.get_by_uri(&uri).is_none(),
            "get_by_uri should be None when resolved is None"
        );
    }

    #[test]
    fn test_get_by_uri_returns_data_on_success() {
        let idx = make_index();
        let uri = make_uri("/tmp/ok.py");
        let src = "x: int = 1\n";
        let _ = idx.set_open(&uri, src, 1);
        let result = idx.get_by_uri(&uri);
        assert!(
            result.is_some(),
            "expected Some from get_by_uri on valid source"
        );
        let (text, _resolved, _diags) = result.unwrap();
        assert_eq!(text, src);
    }

    // ── set_closed ───────────────────────────────────────────────────────────

    #[test]
    fn test_set_closed_nonexistent_file_returns_empty_diagnostics() {
        // A file that was opened in memory but doesn't exist on disk.
        let idx = make_index();
        let uri = make_uri("/tmp/memory_only_xyz123.py");
        let src = "def greet(name):\n    return f\"Hello, {name}!\"\n";
        let _ = idx.set_open(&uri, src, 1);
        // Closing it: file doesn't exist on disk → should return empty diagnostics.
        let (ret_uri, diags) = idx.set_closed(&uri);
        assert_eq!(ret_uri, uri);
        assert!(
            diags.is_empty(),
            "expected empty diagnostics for non-disk file after close"
        );
        // Entry should be removed from the index.
        let path = uri.to_file_path().unwrap();
        assert!(idx.files.get(&path).is_none());
    }

    #[test]
    fn test_set_closed_existing_file_re_analyses() {
        let idx = make_index();
        // Write a real temp file.
        let dir = unique_tmp("bsk_set_closed");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test.py");
        std::fs::write(&file_path, "x: int = 1\n").unwrap();

        let uri = Url::from_file_path(&file_path).unwrap();
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);
        let (ret_uri, _diags) = idx.set_closed(&uri);
        assert_eq!(ret_uri, uri);
        // Entry is still in the index.
        assert!(idx.files.get(&file_path).is_some());
        // is_open should be false.
        assert!(!idx.files.get(&file_path).unwrap().is_open);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── reload_from_disk ─────────────────────────────────────────────────────

    // Exercises [ANALYSIS-INDEX-OPEN]: open files are authoritative; watcher
    // reloads are ignored while open.
    #[test]
    fn test_reload_from_disk_skips_open_files() {
        let idx = make_index();
        let uri = make_uri("/tmp/openfile.py");
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);
        // reload_from_disk must return None for open files.
        let result = idx.reload_from_disk(&uri);
        assert!(result.is_none(), "should skip open files");
    }

    // Exercises [ANALYSIS-INCR-WATCH] / [ANALYSIS-INDEX-INVAL]: unchanged
    // source_hash leaves the entry as-is (no re-analysis).
    #[test]
    fn test_reload_from_disk_skips_unchanged_hash() {
        let dir = unique_tmp("bsk_reload");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("unchanged.py");
        let src = "x: int = 1\n";
        std::fs::write(&file_path, src).unwrap();

        let idx = make_index();
        let uri = Url::from_file_path(&file_path).unwrap();
        // First load.
        let _ = idx.reload_from_disk(&uri);
        // Second load — same content, should return None.
        let result = idx.reload_from_disk(&uri);
        // Note: first call returns Some (newly added), second call returns None (no change).
        // We only assert the second call behaviour.
        assert!(result.is_none(), "unchanged file should not re-analyse");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── scan ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_scan_empty_roots_produces_no_results() {
        let idx = WorkspaceIndex::new(vec![], AnalysisMode::WholeModule, BasiliskConfig::default());
        let (results, file_count, _) = idx.scan();
        assert!(results.is_empty());
        assert_eq!(file_count, 0);
    }

    #[test]
    fn test_scan_collects_python_files() {
        let dir = unique_tmp("bsk_scan");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.py"), "x: int = 1\n").unwrap();
        std::fs::write(dir.join("b.py"), "y: str = 'hi'\n").unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let (results, file_count, _) = idx.scan();
        assert_eq!(file_count, 2, "expected 2 files scanned");
        assert_eq!(results.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn selected_root_scan_excludes_files_owned_by_nested_blocked_root() {
        let parent = unique_tmp("bsk_partial_ready_parent");
        let child = parent.join("nested");
        std::fs::create_dir_all(&child).unwrap();
        let parent_file = parent.join("healthy.py");
        let child_file = child.join("blocked.py");
        std::fs::write(&parent_file, "healthy: int = 1\n").unwrap();
        std::fs::write(&child_file, "blocked: int = 2\n").unwrap();

        let index = WorkspaceIndex::new(
            vec![parent.clone(), child.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let (results, file_count, _) = index.scan_roots(std::slice::from_ref(&parent));
        assert_eq!(file_count, 1);
        assert_eq!(results.len(), 1);
        assert!(results
            .first()
            .is_some_and(|(uri, _)| uri.to_file_path().ok().as_ref() == Some(&parent_file)));
        assert!(index.path_is_owned_by_root(&parent_file, &parent));
        assert!(!index.path_is_owned_by_root(&child_file, &parent));
        assert!(index.path_is_owned_by_root(&child_file, &child));

        let (inverse, inverse_count, _) = index.scan_roots(std::slice::from_ref(&child));
        assert_eq!(inverse_count, 1);
        assert!(inverse
            .first()
            .is_some_and(|(uri, _)| uri.to_file_path().ok().as_ref() == Some(&child_file)));

        let _ = std::fs::remove_dir_all(parent);
    }

    // ── Issue #80: vendored / bundled third-party code must not be scanned ───
    //
    // The extension vendors third-party Python under `vscode-extension/bundled/`
    // (debugpy and its nested `_vendored/` tree). Without a default exclude for
    // `bundled`/`_vendored`, the workspace scan type-checks code we ship verbatim
    // and never edit, flooding ~34k irrelevant diagnostics and burying the user's
    // real errors. The scan must skip these directories by default.
    #[test]
    fn test_scan_excludes_bundled_and_vendored_dirs() {
        let dir = unique_tmp("bsk_scan_bundled");
        let bundled = dir.join("vscode-extension").join("bundled").join("debugpy");
        let vendored = dir.join("pkg").join("_vendored").join("pydevd");
        std::fs::create_dir_all(&bundled).unwrap();
        std::fs::create_dir_all(&vendored).unwrap();

        // A real source file that SHOULD be scanned.
        std::fs::write(dir.join("main.py"), "x: int = 1\n").unwrap();
        // Vendored files that SHOULD be skipped.
        std::fs::write(bundled.join("peb_teb.py"), "def f(x):\n    return x\n").unwrap();
        std::fs::write(vendored.join("pydevd.py"), "def g(y):\n    return y\n").unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let (results, file_count, _) = idx.scan();

        assert_eq!(
            file_count, 1,
            "only main.py should be scanned; bundled/_vendored must be excluded"
        );
        let scanned: Vec<String> = results.iter().map(|(uri, _)| uri.to_string()).collect();
        assert!(
            !scanned.iter().any(|u| u.contains("/bundled/")),
            "bundled debugpy code must not be scanned: {scanned:?}"
        );
        assert!(
            !scanned.iter().any(|u| u.contains("/_vendored/")),
            "_vendored code must not be scanned: {scanned:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn configuration_inventory_preloads_closed_files_in_open_files_only_mode() {
        let root = unique_tmp("bsk_configuration_inventory");
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("closed.py");
        std::fs::write(&source, "value: int = 'not an int'\n").unwrap();
        let index = WorkspaceIndex::new(
            vec![root.clone()],
            AnalysisMode::OpenFilesOnly,
            BasiliskConfig::default(),
        );
        assert!(index.files.is_empty());

        assert_eq!(index.preload_root_for_configuration(&root), 1);
        let entry = index.files.get(&source).unwrap();
        assert!(!entry.is_open);
        assert!(entry
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.code == "assignment_compatibility"));
        drop(entry);
        let _ = std::fs::remove_dir_all(root);
    }

    // ── Issue #80: user-facing `exclude` must accept glob patterns ───────────
    //
    // The workspace `exclude` config is the user's knob for extending the
    // default ignore set. It must support gitignore-style globs applied to both
    // nested directories (`**/generated/**`) and individual files (`*.pb.py`),
    // not just literal path prefixes.
    #[test]
    fn test_scan_user_exclude_supports_glob_patterns() {
        let dir = unique_tmp("bsk_scan_glob_exclude");
        let gen = dir.join("src").join("generated");
        std::fs::create_dir_all(&gen).unwrap();
        std::fs::write(dir.join("app.py"), "x: int = 1\n").unwrap();
        // Excluded by `**/generated/**` (nested directory, any depth).
        std::fs::write(gen.join("models.py"), "y: int = 2\n").unwrap();
        // Excluded by `*.pb.py` (file glob, any depth).
        std::fs::write(dir.join("schema.pb.py"), "z: int = 3\n").unwrap();
        // The user-facing exclude knob, read by the scan via load_config.
        std::fs::write(
            dir.join("pyproject.toml"),
            "[tool.basilisk]\nexclude = [\"**/generated/**\", \"*.pb.py\"]\n",
        )
        .unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let (results, file_count, _) = idx.scan();
        let scanned: Vec<String> = results.iter().map(|(uri, _)| uri.to_string()).collect();

        assert_eq!(
            file_count, 1,
            "only app.py should survive the glob excludes: {scanned:?}"
        );
        assert!(
            scanned.iter().any(|u| u.ends_with("/app.py")),
            "app.py must still be scanned: {scanned:?}"
        );
        assert!(
            !scanned.iter().any(|u| u.contains("generated")),
            "**/generated/** must exclude the nested directory: {scanned:?}"
        );
        assert!(
            !scanned.iter().any(|u| u.contains("schema.pb.py")),
            "*.pb.py glob must exclude the file: {scanned:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// With search paths cached, the scan analyses THROUGH the salsa engine:
    /// every scanned file becomes a tracked `SourceFile`, so cross-file edges
    /// see the whole workspace and later edits hit the memos this pass primed.
    /// Implements [CHKARCH-INCREMENTAL-SALSA] / [ANALYSIS-STARTUP-WHOLE].
    #[test]
    fn test_scan_with_search_paths_primes_the_engine() {
        let dir = unique_tmp("bsk_scan_primes");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.py"), "x: int = 1\n").unwrap();
        std::fs::write(dir.join("b.py"), "y: str = 'hi'\n").unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        idx.set_search_paths(crate::import_resolver::ImportSearchPaths {
            roots: vec![dir.clone()],
            extra_paths: vec![],
            stub_paths: vec![],
            workspace_members: vec![],
            site_packages: None,
            registry: None,
            typeshed_snapshot: None,
        });

        let (results, file_count, _) = idx.scan();
        assert_eq!(file_count, 2);
        assert_eq!(results.len(), 2, "the scan publishes every scanned file");
        assert_eq!(
            idx.salsa_engine.tracked_source_count(),
            2,
            "the scan must analyse through the engine — every file tracked"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `refresh_open_files` must republish open files even when their
    /// diagnostics are UNCHANGED: its consumers publish to a client whose
    /// state may have diverged from the server's store — re-enabling type
    /// checking cleared the client's diagnostics but not the stored entry, so
    /// a changed-only filter here would leave the editor empty forever
    /// (regression caught by the VSIX `basilisk.enabled` toggle e2e).
    #[test]
    fn test_refresh_open_files_republishes_unchanged_open_files() {
        let dir = unique_tmp("bsk_refresh_open");
        std::fs::create_dir_all(&dir).unwrap();
        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            annotations_on(),
        );
        idx.set_search_paths(crate::import_resolver::ImportSearchPaths {
            roots: vec![dir.clone()],
            extra_paths: vec![],
            stub_paths: vec![],
            workspace_members: vec![],
            site_packages: None,
            registry: None,
            typeshed_snapshot: None,
        });

        // Open a file with a diagnostic; its fresh state is now stored.
        let uri = Url::from_file_path(dir.join("open.py")).unwrap();
        let opened = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert!(!opened.is_empty(), "precondition: the open file diagnoses");

        // Nothing changed since — refresh must STILL return the open file,
        // with its diagnostics, so the caller can repopulate the client.
        let refreshed = idx.refresh_open_files();
        assert!(
            refreshed
                .iter()
                .any(|(target, diags)| target == &uri && !diags.is_empty()),
            "refresh_open_files must republish an unchanged open file — the \
             client's state may have been cleared (e.g. type-checking toggle); \
             got: {:?}",
            refreshed
                .iter()
                .map(|(u, d)| (u.to_string(), d.len()))
                .collect::<Vec<_>>()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The workspace sweep republishes ONLY files whose diagnostics changed:
    /// a sweep over an unchanged workspace publishes nothing (a client no-op
    /// either way, without O(workspace) publish traffic), while a real change
    /// still republishes the affected file.
    #[test]
    fn test_sweep_republishes_only_changed_diagnostics() {
        let dir = unique_tmp("bsk_sweep_diff");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("clean.py"), "x: int = 1\n").unwrap();
        let broken = dir.join("broken.py");
        std::fs::write(&broken, "def f() -> int:\n    return 1\n").unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        idx.set_search_paths(crate::import_resolver::ImportSearchPaths {
            roots: vec![dir.clone()],
            extra_paths: vec![],
            stub_paths: vec![],
            workspace_members: vec![],
            site_packages: None,
            registry: None,
            typeshed_snapshot: None,
        });
        let _ = idx.scan();

        // Nothing changed since the scan: the sweep must publish nothing.
        let unchanged = idx.reresolve_imports_and_recheck();
        assert!(
            unchanged.is_empty(),
            "a sweep over an unchanged workspace must republish nothing, got: {:?}",
            unchanged
                .iter()
                .map(|(u, _)| u.to_string())
                .collect::<Vec<_>>()
        );

        // Change one file's stored text to something that changes diagnostics.
        if let Some(mut entry) = idx.files.get_mut(&broken) {
            entry.text = "def f() -> int:\n    return undefined_name\n".to_owned();
        }
        let changed = idx.reresolve_imports_and_recheck();
        assert_eq!(
            changed.len(),
            1,
            "only the changed file republishes, got: {:?}",
            changed
                .iter()
                .map(|(u, _)| u.to_string())
                .collect::<Vec<_>>()
        );
        assert!(
            changed
                .first()
                .is_some_and(|(uri, _)| uri.to_string().ends_with("broken.py")),
            "the republished file is the changed one"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scan_skips_open_files() {
        let dir = unique_tmp("bsk_scan_skip_open");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("open.py");
        std::fs::write(&file_path, "x: int = 1\n").unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let uri = Url::from_file_path(&file_path).unwrap();
        let _ = idx.set_open(&uri, "x: int = 1\n", 1);

        let (results, file_count, _) = idx.scan();
        // File is open, so scan should skip it.
        assert_eq!(file_count, 1, "file_count should count the file");
        assert_eq!(
            results.len(),
            0,
            "open file should be excluded from scan results"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── all_resolved ─────────────────────────────────────────────────────────

    #[test]
    fn test_all_resolved_returns_entries_with_resolved() {
        let idx = make_index();
        let uri1 = make_uri("/tmp/r1.py");
        let uri2 = make_uri("/tmp/r2.py");
        let _ = idx.set_open(&uri1, "x: int = 1\n", 1);
        let _ = idx.set_open(&uri2, "class Bad:\n    pass\nBad.", 1); // parse error → no resolved
        let resolved_list = idx.all_resolved();
        // Only uri1 should appear (uri2 failed to parse).
        assert_eq!(resolved_list.len(), 1);
    }

    // ── Phase 5: uv.lock change triggers registry reparse ───────────────────

    /// Helper: create a minimal uv project with a `uv.lock` and `pyproject.toml`.
    fn create_uv_project(dir: &std::path::Path, packages: &[(&str, &str)]) {
        // pyproject.toml with [tool.uv] so it's detected as a uv project.
        let dep_names: Vec<String> = packages
            .iter()
            .map(|(name, _)| format!("\"{name}\""))
            .collect();
        let pyproject = format!(
            "[project]\nname = \"test-project\"\nversion = \"0.1.0\"\ndependencies = [{}]\n\n[tool.uv]\n",
            dep_names.join(", ")
        );
        std::fs::write(dir.join("pyproject.toml"), pyproject).unwrap();

        // uv.lock with the specified packages.
        write_uv_lock(dir, packages);

        // Marker file so detect_uv_project finds it.
        std::fs::write(
            dir.join("uv.lock"),
            std::fs::read_to_string(dir.join("uv.lock")).unwrap(),
        )
        .unwrap();
    }

    /// Helper: write a uv.lock TOML file with the given packages.
    fn write_uv_lock(dir: &std::path::Path, packages: &[(&str, &str)]) {
        use std::fmt::Write as _;
        let mut lock_content = String::from("version = 1\nrequires-python = \">=3.12\"\n\n");
        for (name, version) in packages {
            let _ = write!(
                lock_content,
                "[[package]]\nname = \"{name}\"\nversion = \"{version}\"\nsource = {{ registry = \"https://pypi.org/simple\" }}\n\n"
            );
        }
        std::fs::write(dir.join("uv.lock"), lock_content).unwrap();
    }

    #[test]
    fn test_lockfile_change_triggers_registry_reparse() {
        let dir = unique_tmp("bsk_uv_reparse");
        std::fs::create_dir_all(&dir).unwrap();
        create_uv_project(&dir, &[("requests", "2.31.0")]);

        // Build initial registry.
        let roots = vec![dir.clone()];
        let registry1 = build_registry_from_roots(&roots);
        assert!(registry1.is_some(), "registry should be built from uv.lock");
        let reg1 = registry1.unwrap();
        assert!(reg1.has_package("requests"));
        assert!(!reg1.has_package("flask"));

        // Simulate uv.lock change: add flask.
        write_uv_lock(&dir, &[("requests", "2.31.0"), ("flask", "3.0.0")]);

        // Re-parse — should pick up flask.
        let registry2 = build_registry_from_roots(&roots);
        assert!(registry2.is_some());
        let reg2 = registry2.unwrap();
        assert!(reg2.has_package("requests"));
        assert!(
            reg2.has_package("flask"),
            "flask should appear after lock change"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Phase 5: add/remove package updates diagnostics ─────────────────────

    #[test]
    fn test_lockfile_add_package_clears_e0010() {
        let dir = unique_tmp("bsk_uv_add_pkg");
        std::fs::create_dir_all(&dir).unwrap();
        create_uv_project(&dir, &[("requests", "2.31.0")]);

        let roots = vec![dir.clone()];
        let config = crate::config::load_config(&dir);

        // Build workspace index with a file that imports `flask`.
        let idx = WorkspaceIndex::new(
            roots.clone(),
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let uri = make_uri(&format!("{}/app.py", dir.display()));
        let _ = idx.set_open(&uri, "import flask\n", 1);

        // Resolve with registry that does NOT have flask: the flask import
        // should be unresolved (E0010).
        rebuild_and_resolve_imports(&idx, &roots, &config);
        let diags_before = get_diagnostics(&idx, &uri);
        assert!(
            has_diag(&diags_before, "imports_unresolved", "flask"),
            "expected imports_unresolved for unresolved flask import, got: {diags_before:?}"
        );

        // Now add flask to the lock file and rebuild.
        write_uv_lock(&dir, &[("requests", "2.31.0"), ("flask", "3.0.0")]);
        // Also add flask to pyproject dependencies.
        let pyproject = "[project]\nname = \"test-project\"\nversion = \"0.1.0\"\ndependencies = [\"requests\", \"flask\"]\n\n[tool.uv]\n";
        std::fs::write(dir.join("pyproject.toml"), pyproject).unwrap();

        rebuild_and_resolve_imports(&idx, &roots, &config);

        // After adding flask to the registry, classify_unresolved should now
        // return NeedsSync (in registry but not on filesystem) instead of
        // NotInstalled. The diagnostic message changes accordingly.
        let diags_after = get_diagnostics(&idx, &uri);
        assert!(
            !has_diag(&diags_after, "imports_unresolved", "not a dependency"),
            "flask should no longer show 'not a dependency' after being added to lock: {diags_after:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_lockfile_remove_package_fires_e0010() {
        let dir = unique_tmp("bsk_uv_rm_pkg");
        std::fs::create_dir_all(&dir).unwrap();
        create_uv_project(&dir, &[("requests", "2.31.0"), ("flask", "3.0.0")]);

        let roots = vec![dir.clone()];
        let config = crate::config::load_config(&dir);

        let idx = WorkspaceIndex::new(
            roots.clone(),
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let uri = make_uri(&format!("{}/app.py", dir.display()));
        let _ = idx.set_open(&uri, "import flask\n", 1);

        // Resolve with registry that HAS flask.
        rebuild_and_resolve_imports(&idx, &roots, &config);

        // Now remove flask from the lock file.
        write_uv_lock(&dir, &[("requests", "2.31.0")]);
        let pyproject = "[project]\nname = \"test-project\"\nversion = \"0.1.0\"\ndependencies = [\"requests\"]\n\n[tool.uv]\n";
        std::fs::write(dir.join("pyproject.toml"), pyproject).unwrap();

        rebuild_and_resolve_imports(&idx, &roots, &config);

        let diags = get_diagnostics(&idx, &uri);
        assert!(
            has_diag(&diags, "imports_unresolved", "flask"),
            "expected imports_unresolved for flask after removal from lock, got: {diags:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // Regression for issue #252, exercises the [LSPUV-DIAGNOSTICS-MODULE-NOT-FOUND]
    // resolution contract (`import_resolver::resolve_site_packages_with_env`):
    // the two lockfile E0010 tests above were not hermetic — a uv-locked
    // project with no venv fell back to the ambient `python3` interpreter's
    // site-packages, so `import flask` resolved on any machine whose first
    // ambient site-packages dir carried flask and the "expected
    // imports_unresolved" assertions failed. The contract: a uv-locked project
    // resolves third-party imports against its lock and its own (or explicitly
    // activated) venv ONLY — never the ambient interpreter.
    #[test]
    fn test_locked_project_ignores_ambient_interpreter_site_packages() {
        let dir = unique_tmp("bsk_uv_no_ambient");
        std::fs::create_dir_all(&dir).unwrap();
        create_uv_project(&dir, &[("requests", "2.31.0")]);

        let roots = vec![dir.clone()];
        let config = crate::config::load_config(&dir);
        let registry = build_registry_from_roots(&roots);
        assert!(registry.is_some(), "temp uv project must yield a registry");

        // The temp project is uv-locked and has NO venv of its own, and no
        // VIRTUAL_ENV is injected: site-packages must stay unset instead of
        // being probed from whatever `python3` happens to be on PATH.
        let search_paths =
            crate::import_resolver::search_paths_from_config(&roots, &config, registry);
        assert!(
            search_paths.site_packages.is_none(),
            "uv-locked project without a venv must not inherit the ambient \
             interpreter's site-packages (issue #252), got: {:?}",
            search_paths.site_packages
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Issue #22: sibling-module imports in script directories ─────────────
    //
    // `import configure_agent_backend` from `scripts/configure_agent_backend_test.py`
    // must resolve to the sibling `scripts/configure_agent_backend.py` even when
    // the workspace root is the project root (not `scripts/`). This mirrors
    // Python's `sys.path[0]` behaviour and prevents imports_unresolved false positives
    // for the common scripts-with-tests pattern.
    #[test]
    fn test_sibling_import_in_scripts_dir_does_not_emit_e0010() {
        let project_root = unique_tmp("bsk_e0010_sibling_root");
        let scripts_dir = project_root.join("scripts");
        std::fs::create_dir_all(&scripts_dir).unwrap();

        // The sibling module being imported.
        std::fs::write(
            scripts_dir.join("configure_agent_backend.py"),
            "VALUE: int = 1\n",
        )
        .unwrap();
        // The importing file lives next to the sibling.
        let test_path = scripts_dir.join("configure_agent_backend_test.py");
        std::fs::write(&test_path, "import configure_agent_backend\n").unwrap();

        // Workspace root is the *project* root — `scripts/` is NOT listed.
        let roots = vec![project_root.clone()];
        let config = crate::config::load_config(&project_root);

        let idx = WorkspaceIndex::new(
            roots.clone(),
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let uri = Url::from_file_path(&test_path).unwrap();
        let _ = idx.set_open(&uri, "import configure_agent_backend\n", 1);

        let search_paths = crate::import_resolver::search_paths_from_config(
            &roots, &config, /*registry=*/ None,
        );
        idx.set_search_paths(search_paths);
        let _ = idx.reresolve_imports_and_recheck();

        let diags = get_diagnostics(&idx, &uri);
        assert!(
            !has_diag(&diags, "imports_unresolved", "configure_agent_backend"),
            "sibling-module import in a script directory must resolve via sys.path[0] \
             fallback; got imports_unresolved: {diags:?}"
        );

        let _ = std::fs::remove_dir_all(&project_root);
    }

    // ── Issue #24: src layout test helpers must resolve ─────────────────────
    //
    // Project layout (very common — pytest src layout):
    //   pyproject.toml
    //   src/agent_backend/__init__.py
    //   src/agent_backend/db/models.py
    //   tests/helpers.py        ← imports `from agent_backend.db.models import X`
    //   tests/test_foo.py       ← imports `from tests.helpers import Y`
    //
    // Both imports must resolve:
    //   * `agent_backend.db.models` — via `src/` on search path (workspace_member)
    //   * `tests.helpers` — via the workspace root being on the search path
    #[test]
    fn test_src_layout_test_helpers_resolve() {
        let root = unique_tmp("bsk_e0010_src_layout");
        let src_pkg = root.join("src").join("agent_backend").join("db");
        let tests_dir = root.join("tests");
        std::fs::create_dir_all(&src_pkg).unwrap();
        std::fs::create_dir_all(&tests_dir).unwrap();

        // pyproject.toml so src layout discovery picks up `src/`.
        std::fs::write(
            root.join("pyproject.toml"),
            "[project]\nname = \"agent_backend\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        // Production package.
        std::fs::write(
            root.join("src").join("agent_backend").join("__init__.py"),
            "",
        )
        .unwrap();
        std::fs::write(
            root.join("src")
                .join("agent_backend")
                .join("db")
                .join("__init__.py"),
            "",
        )
        .unwrap();
        std::fs::write(src_pkg.join("models.py"), "class AgentConfig: ...\n").unwrap();

        // Test helpers (PEP 420 namespace, no __init__.py at tests/).
        let helpers_path = tests_dir.join("helpers.py");
        std::fs::write(
            &helpers_path,
            "from agent_backend.db.models import AgentConfig\n",
        )
        .unwrap();
        let test_path = tests_dir.join("test_foo.py");
        std::fs::write(&test_path, "from tests.helpers import AgentConfig\n").unwrap();

        let roots = vec![root.clone()];
        let config = crate::config::load_config(&root);

        let idx = WorkspaceIndex::new(
            roots.clone(),
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );

        // Open the helper and the test file (mirrors a real LSP session).
        let helpers_uri = Url::from_file_path(&helpers_path).unwrap();
        let _ = idx.set_open(
            &helpers_uri,
            "from agent_backend.db.models import AgentConfig\n",
            1,
        );
        let test_uri = Url::from_file_path(&test_path).unwrap();
        let _ = idx.set_open(&test_uri, "from tests.helpers import AgentConfig\n", 1);

        // Mirror the LSP init flow: from_config discovers workspace_members
        // (src/ for src-layout projects), then imports are resolved.
        let search_paths = crate::import_resolver::search_paths_from_config(
            &roots, &config, /*registry=*/ None,
        );
        idx.set_search_paths(search_paths);
        let _ = idx.reresolve_imports_and_recheck();

        // tests/helpers.py — imports agent_backend.db.models (via src/).
        let helpers_diags = get_diagnostics(&idx, &helpers_uri);
        assert!(
            !has_diag(&helpers_diags, "imports_unresolved", "agent_backend"),
            "imports_unresolved false positive: src-layout production import from a test \
             helper must resolve via src/ on the search path; got: {helpers_diags:?}"
        );

        // tests/test_foo.py — imports tests.helpers (via workspace root).
        let test_diags = get_diagnostics(&idx, &test_uri);
        assert!(
            !has_diag(&test_diags, "imports_unresolved", "tests.helpers"),
            "imports_unresolved false positive: `tests.helpers` import must resolve when the \
             workspace root is on the search path; got: {test_diags:?}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Editor edits must resolve third-party imports (no false imports_unresolved) ───
    //
    // Regression: the full workspace scan resolves `import requests` against the
    // venv site-packages (no imports_unresolved), but opening/editing a file ran parse →
    // syntactic-resolve → check WITHOUT the import search paths, so every
    // third-party import was re-marked `Unresolved` and imports_unresolved fired in the
    // editor for packages the CLI resolves fine. The diagnostics that
    // `set_open` *publishes* must already reflect import resolution.
    // Implements [ANALYSIS-INCR-IMPORTS].
    #[test]
    fn test_set_open_resolves_site_package_imports_no_e0010() {
        let root = unique_tmp("bsk_incr_imports_e0010");
        // Fake site-packages with a typed `requests` package (py.typed marker).
        let site_packages = root.join("site-packages");
        let requests = site_packages.join("requests");
        std::fs::create_dir_all(&requests).unwrap();
        std::fs::write(requests.join("__init__.py"), "").unwrap();
        std::fs::write(requests.join("py.typed"), "").unwrap();

        let main_path = root.join("main.py");
        std::fs::write(&main_path, "import requests\n").unwrap();

        let roots = vec![root.clone()];
        let idx = WorkspaceIndex::new(
            roots.clone(),
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );

        // Mirror the LSP scan: cache the import search paths in the index.
        // Built directly (not via `from_config`) so an ambient `VIRTUAL_ENV`
        // in the test environment cannot redirect site-packages discovery.
        idx.set_search_paths(crate::import_resolver::ImportSearchPaths {
            roots,
            extra_paths: vec![],
            stub_paths: vec![],
            workspace_members: vec![],
            site_packages: Some(site_packages),
            typeshed_snapshot: None,
            registry: None,
        });

        // Simulate the editor opening the file. The diagnostics it PUBLISHES
        // (the return value) must not contain imports_unresolved for `requests`.
        let uri = Url::from_file_path(&main_path).unwrap();
        let published = idx.set_open(&uri, "import requests\n", 1);
        assert!(
            !lsp_codes(&published)
                .iter()
                .any(|c| c == "imports_unresolved"),
            "editor-opened file must resolve `requests` via the cached search \
             paths; got imports_unresolved in published diagnostics: {published:?}"
        );

        // The cached checker diagnostics must agree (used by other features).
        let stored = get_diagnostics(&idx, &uri);
        assert!(
            !has_diag(&stored, "imports_unresolved", "requests"),
            "stored diagnostics must not carry imports_unresolved for resolved `requests`: {stored:?}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Phase 5: .python-version change updates stdlib availability ──────────

    #[test]
    fn test_python_version_change_updates_config() {
        let dir = unique_tmp("bsk_uv_pyver");
        std::fs::create_dir_all(&dir).unwrap();

        // Start with Python 3.11.
        std::fs::write(dir.join(".python-version"), "3.11\n").unwrap();
        let ver1 = basilisk_uv::python_version::read_python_version(&dir);
        assert_eq!(ver1, Some("3.11".to_owned()));

        // Simulate .python-version change to 3.12.
        std::fs::write(dir.join(".python-version"), "3.12\n").unwrap();
        let ver2 = basilisk_uv::python_version::read_python_version(&dir);
        assert_eq!(ver2, Some("3.12".to_owned()));

        // Verify that the change is detected and a different value is returned.
        assert_ne!(ver1, ver2, "python version should change after file update");

        let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot().unwrap();
        assert!(snapshot.read_stub("tomllib").is_some());
        assert!(snapshot.read_stub("os").is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Phase 6: multi-root LSP workspace folder mapping ────────────────────

    #[test]
    fn test_workspace_member_folders_with_uv_workspace() {
        let dir = unique_tmp("bsk_uv_ws_folders");
        std::fs::create_dir_all(&dir).unwrap();

        // Create workspace members.
        let pkg_a = dir.join("packages").join("alpha");
        let pkg_b = dir.join("packages").join("beta");
        std::fs::create_dir_all(&pkg_a).unwrap();
        std::fs::create_dir_all(&pkg_b).unwrap();

        // pyproject.toml with workspace members.
        let pyproject = "[tool.uv.workspace]\nmembers = [\"packages/*\"]\n";
        std::fs::write(dir.join("pyproject.toml"), pyproject).unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let folders = idx.workspace_member_folders();

        assert_eq!(
            folders.len(),
            2,
            "expected 2 workspace folders, got: {folders:?}"
        );

        let names: Vec<&str> = folders.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"alpha"), "should contain alpha: {names:?}");
        assert!(names.contains(&"beta"), "should contain beta: {names:?}");

        // Each folder should have a valid file:// URI.
        for folder in &folders {
            assert!(
                folder.uri.scheme() == "file",
                "folder URI should be file://: {}",
                folder.uri
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_workspace_member_folders_no_uv_workspace() {
        let dir = unique_tmp("bsk_uv_ws_none");
        std::fs::create_dir_all(&dir).unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let folders = idx.workspace_member_folders();

        assert!(
            folders.is_empty(),
            "non-uv workspace should return no folders"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_workspace_member_folders_excludes_are_still_enumerated() {
        // Workspace exclude patterns are stored in the UvWorkspace but
        // the folder mapping enumerates all physical members — filtering
        // by excludes is the caller's responsibility.
        let dir = unique_tmp("bsk_uv_ws_excl");
        std::fs::create_dir_all(&dir).unwrap();

        let pkg = dir.join("libs").join("core");
        std::fs::create_dir_all(&pkg).unwrap();

        let pyproject = "[tool.uv.workspace]\nmembers = [\"libs/*\"]\nexclude = [\"libs/core\"]\n";
        std::fs::write(dir.join("pyproject.toml"), pyproject).unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let folders = idx.workspace_member_folders();

        // The folder mapping reports what's physically present; the caller
        // applies exclude logic.
        assert_eq!(folders.len(), 1);
        if let Some(folder) = folders.first() {
            assert_eq!(folder.name, "core");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Test helpers ────────────────────────────────────────────────────────

    /// Build a `PackageRegistry` from workspace roots, mirroring the LSP init flow.
    fn build_registry_from_roots(
        roots: &[std::path::PathBuf],
    ) -> Option<Arc<basilisk_uv::PackageRegistry>> {
        let uv_info = basilisk_uv::detect_uv_project(roots)?;
        if !uv_info.has_lockfile {
            return None;
        }
        let lock_path = uv_info.root.join("uv.lock");
        let lock_file = basilisk_uv::parse_lock_file(&lock_path).ok()?;
        let deps = basilisk_uv::extract_pyproject_deps(&uv_info.root);
        let registry = basilisk_uv::PackageRegistry::from_lock_file(&lock_file, &deps);
        Some(Arc::new(registry))
    }

    /// Build the registry from `roots`, derive `ImportSearchPaths`, cache them
    /// on the index, and re-analyse the workspace through the salsa engine —
    /// the same flow the LSP scan and config-watcher paths run.
    fn rebuild_and_resolve_imports(
        idx: &WorkspaceIndex,
        roots: &[std::path::PathBuf],
        config: &crate::config::WorkspaceConfig,
    ) {
        let registry = build_registry_from_roots(roots);
        let search_paths =
            crate::import_resolver::search_paths_from_config(roots, config, registry);
        idx.set_search_paths(search_paths);
        let _ = idx.reresolve_imports_and_recheck();
    }

    /// Extract checker diagnostics for a given URI from the workspace index.
    fn get_diagnostics(index: &WorkspaceIndex, uri: &Url) -> Vec<basilisk_checker::Diagnostic> {
        let path = uri.to_file_path().unwrap();
        index
            .files
            .get(&path)
            .map(|e| e.diagnostics.clone())
            .unwrap_or_default()
    }

    // ── Config-driven severity tests ────────────────────────────────────────
    //
    // These prove that the LSP honours `BasiliskConfig` rule severity
    // overrides identically to the CLI. Before the fix, the LSP always used
    // `BasiliskConfig::default()` and ignored project-level configuration.

    /// Source that triggers BSK-0001 (missing parameter annotation).
    const SRC_MISSING_ANNOTATION: &str = "def greet(name):\n    return name\n";

    /// Source that triggers BSK-0050 (redundant type annotation).
    const SRC_REDUNDANT_ANNOTATION: &str = "x: int = 42\n";

    /// Helper: build a `WorkspaceIndex` with a custom `BasiliskConfig`.
    fn make_index_with_config(config: BasiliskConfig) -> WorkspaceIndex {
        WorkspaceIndex::new(vec![], AnalysisMode::WholeModule, config)
    }

    /// Config with explicit native severities for the annotation rules used by
    /// these workspace tests. See [CHKARCH-CONFIGURATION-ONLY].
    fn annotations_on() -> BasiliskConfig {
        use basilisk_config::RuleSeverity::{Error, Warning};

        BasiliskConfig::with_rule_entries(
            [
                ("BSK-0001", Error),
                ("BSK-0002", Error),
                ("BSK-0003", Error),
                ("BSK-0004", Error),
                ("BSK-0005", Error),
                ("BSK-0025", Error),
                ("BSK-0014", Warning),
                ("BSK-0040", Warning),
                ("BSK-0050", Warning),
            ]
            .into_iter()
            .map(|(code, severity)| (code.to_owned(), severity))
            .collect(),
        )
    }

    /// Build a `WorkspaceIndex` that explicitly assigns one rule's severity.
    /// A non-disabled severity also selects an opt-in rule.
    fn make_index_with_rule_override(
        code: &str,
        severity: basilisk_config::RuleSeverity,
    ) -> WorkspaceIndex {
        let mut config = annotations_on();
        if let Some(tables) = config.rule_chain.first_mut() {
            let _ = tables.rules.insert(code.to_owned(), severity);
        }
        make_index_with_config(config)
    }

    /// Helper: extract LSP diagnostic codes for a URI.
    fn lsp_codes(diags: &[tower_lsp::lsp_types::Diagnostic]) -> Vec<String> {
        diags
            .iter()
            .filter_map(|d| match &d.code {
                Some(tower_lsp::lsp_types::NumberOrString::String(s)) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }

    /// Count diagnostics with `code` in `diags`.
    fn count_code(diags: &[basilisk_checker::Diagnostic], code: &str) -> usize {
        diags.iter().filter(|d| d.code.code == code).count()
    }

    /// Build a minimal `basilisk_checker::Diagnostic` for severity-mapping
    /// tests where only `code`, `severity`, and `message` are interesting.
    fn make_test_diag(
        code: &'static str,
        severity: basilisk_checker::Severity,
        message: &str,
    ) -> basilisk_checker::Diagnostic {
        basilisk_checker::Diagnostic {
            code: basilisk_checker::ErrorCode {
                code,
                docs_url: "https://www.basilisk-python.dev/errors/test",
            },
            severity,
            message: message.to_owned(),
            span: basilisk_resolver::Span::new(0, 1),
            path: "test.py".to_owned(),
            help: None,
            note: None,
            provenance: None,
        }
    }

    /// Returns `true` when at least one diagnostic in `diags` has `code` and a
    /// message containing `substring`. Used by tests that look for a
    /// specific (code, message-fragment) pair, e.g. unresolved-flask vs.
    /// unresolved-requests in the same E0010 firing.
    fn has_diag(diags: &[basilisk_checker::Diagnostic], code: &str, substring: &str) -> bool {
        diags
            .iter()
            .any(|d| d.code.code == code && d.message.contains(substring))
    }

    /// Assert that every checker diagnostic for `code` has the expected severity.
    fn assert_checker_severity(
        index: &WorkspaceIndex,
        uri: &Url,
        code: &str,
        expected: basilisk_checker::Severity,
    ) {
        let diags = get_diagnostics(index, uri);
        let matching: Vec<_> = diags.iter().filter(|d| d.code.code == code).collect();
        assert!(!matching.is_empty(), "expected {code} diagnostic, got none");
        for d in &matching {
            assert_eq!(
                d.severity, expected,
                "{code} severity must be {expected:?}, got {:?}",
                d.severity
            );
        }
    }

    /// Assert that every LSP diagnostic for `code` in `diags` has the expected severity.
    fn assert_lsp_severity(
        diags: &[tower_lsp::lsp_types::Diagnostic],
        code: &str,
        expected: tower_lsp::lsp_types::DiagnosticSeverity,
    ) {
        let codes = lsp_codes(diags);
        assert!(
            codes.contains(&code.to_owned()),
            "expected {code} in LSP diagnostics, got {codes:?}"
        );
        for d in diags {
            if let Some(tower_lsp::lsp_types::NumberOrString::String(c)) = &d.code {
                if c == code {
                    assert_eq!(
                        d.severity,
                        Some(expected),
                        "{code} LSP severity must be {expected:?}, got {:?}",
                        d.severity
                    );
                }
            }
        }
    }

    // ── House rules enabled: W-codes are warnings, E-codes are errors ───────
    // These rules are off by default (the default config is pure PEP
    // conformance); the index opts in so their default severities are
    // observable. See [CHKARCH-CONFIGURATION-ONLY].

    #[test]
    fn house_rules_w0050_is_warning_in_checker_diagnostics() {
        let idx = make_index_with_config(annotations_on());
        let uri = make_uri("/tmp/cfg_w0050_default.py");
        let _ = idx.set_open(&uri, SRC_REDUNDANT_ANNOTATION, 1);
        assert_checker_severity(&idx, &uri, "BSK-0050", basilisk_checker::Severity::Warning);
    }

    #[test]
    fn house_rules_w0050_lsp_severity_is_warning() {
        let idx = make_index_with_config(annotations_on());
        let uri = make_uri("/tmp/cfg_w0050_lsp.py");
        let lsp_diags = idx.set_open(&uri, SRC_REDUNDANT_ANNOTATION, 1);
        assert_lsp_severity(
            &lsp_diags,
            "BSK-0050",
            tower_lsp::lsp_types::DiagnosticSeverity::WARNING,
        );
    }

    #[test]
    fn house_rules_e0001_is_error_in_checker_diagnostics() {
        let idx = make_index_with_config(annotations_on());
        let uri = make_uri("/tmp/cfg_e0001_default.py");
        let _ = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert_checker_severity(&idx, &uri, "BSK-0001", basilisk_checker::Severity::Error);
    }

    #[test]
    fn house_rules_e0001_lsp_severity_is_error() {
        let idx = make_index_with_config(annotations_on());
        let uri = make_uri("/tmp/cfg_e0001_lsp.py");
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert_lsp_severity(
            &lsp_diags,
            "BSK-0001",
            tower_lsp::lsp_types::DiagnosticSeverity::ERROR,
        );
    }

    // ── Global rule severity override: demote error to warning ──────────────

    #[test]
    fn config_override_demotes_e0001_to_warning_in_checker() {
        let idx = make_index_with_rule_override("BSK-0001", basilisk_config::RuleSeverity::Warning);
        let uri = make_uri("/tmp/cfg_demote_e0001.py");
        let _ = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert_checker_severity(&idx, &uri, "BSK-0001", basilisk_checker::Severity::Warning);
    }

    #[test]
    fn config_override_demotes_e0001_to_warning_in_lsp() {
        let idx = make_index_with_rule_override("BSK-0001", basilisk_config::RuleSeverity::Warning);
        let uri = make_uri("/tmp/cfg_demote_e0001_lsp.py");
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert_lsp_severity(
            &lsp_diags,
            "BSK-0001",
            tower_lsp::lsp_types::DiagnosticSeverity::WARNING,
        );
    }

    // ── Global rule severity override: demote error to info ─────────────────

    #[test]
    fn config_override_demotes_e0001_to_info_in_checker() {
        let idx = make_index_with_rule_override("BSK-0001", basilisk_config::RuleSeverity::Info);
        let uri = make_uri("/tmp/cfg_info_e0001.py");
        let _ = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert_checker_severity(&idx, &uri, "BSK-0001", basilisk_checker::Severity::Info);
    }

    #[test]
    fn config_override_demotes_e0001_to_info_in_lsp() {
        let idx = make_index_with_rule_override("BSK-0001", basilisk_config::RuleSeverity::Info);
        let uri = make_uri("/tmp/cfg_info_e0001_lsp.py");
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        assert_lsp_severity(
            &lsp_diags,
            "BSK-0001",
            tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION,
        );
    }

    // ── Global rule severity override: disable rule entirely ────────────────

    #[test]
    fn config_override_disables_e0001_removes_from_checker() {
        let idx =
            make_index_with_rule_override("BSK-0001", basilisk_config::RuleSeverity::Disabled);
        let uri = make_uri("/tmp/cfg_disable_e0001.py");
        let _ = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);

        let diags = get_diagnostics(&idx, &uri);
        let e0001_count = count_code(&diags, "BSK-0001");
        assert_eq!(
            e0001_count, 0,
            "disabled rule BSK-0001 must produce zero diagnostics, got {e0001_count}"
        );
    }

    #[test]
    fn config_override_disables_e0001_removes_from_lsp() {
        let idx =
            make_index_with_rule_override("BSK-0001", basilisk_config::RuleSeverity::Disabled);
        let uri = make_uri("/tmp/cfg_disable_e0001_lsp.py");
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);

        let codes = lsp_codes(&lsp_diags);
        assert!(
            !codes.contains(&"BSK-0001".to_owned()),
            "disabled BSK-0001 must not appear in LSP diagnostics, got {codes:?}"
        );
    }

    // ── BSK-0050 severity override: promote warning to error ───────────────────

    #[test]
    fn config_override_promotes_w0050_to_error_in_lsp() {
        // `RuleSeverity::Error` promotes a warning-default rule UP to a hard
        // error, so a project can dial strictness up (e.g. make "no type stubs"
        // a red error) — not just down. BSK-0050 defaults to Warning; with the
        // override it must surface as ERROR through the LSP.
        let idx = make_index_with_rule_override("BSK-0050", basilisk_config::RuleSeverity::Error);
        let uri = make_uri("/tmp/cfg_promote_w0050.py");
        let lsp_diags = idx.set_open(&uri, SRC_REDUNDANT_ANNOTATION, 1);
        assert_lsp_severity(
            &lsp_diags,
            "BSK-0050",
            tower_lsp::lsp_types::DiagnosticSeverity::ERROR,
        );
    }

    // ── BSK-0050 disabled via config ───────────────────────────────────────────

    #[test]
    fn config_override_disables_w0050() {
        let idx =
            make_index_with_rule_override("BSK-0050", basilisk_config::RuleSeverity::Disabled);
        let uri = make_uri("/tmp/cfg_disable_w0050.py");
        let lsp_diags = idx.set_open(&uri, SRC_REDUNDANT_ANNOTATION, 1);

        let codes = lsp_codes(&lsp_diags);
        assert!(
            !codes.contains(&"BSK-0050".to_owned()),
            "disabled BSK-0050 must not appear in LSP diagnostics, got {codes:?}"
        );

        let diags = get_diagnostics(&idx, &uri);
        let w0050_count = count_code(&diags, "BSK-0050");
        assert_eq!(
            w0050_count, 0,
            "disabled BSK-0050 must produce zero checker diagnostics"
        );
    }

    // ── Opt-in stub diagnostics ──────────────────────────────────────────────

    #[test]
    fn unconfigured_stub_rule_stays_disabled() {
        let idx = make_index_with_config(BasiliskConfig::default());
        let uri = make_uri("/tmp/cfg_no_stubs.py");
        let src = "import os\n";
        let _ = idx.set_open(&uri, src, 1);

        let diags = get_diagnostics(&idx, &uri);
        let e0152_count = count_code(&diags, "BSK-0152");
        assert_eq!(
            e0152_count, 0,
            "BSK-0152 must remain off without an explicit rule severity"
        );
    }

    // ── Config stored on WorkspaceIndex ─────────────────────────────────────

    #[test]
    fn workspace_index_stores_checker_config() {
        let config = BasiliskConfig::with_rule_entries(std::collections::HashMap::from([(
            "BSK-0001".to_owned(),
            basilisk_config::RuleSeverity::Warning,
        )]));
        let idx = make_index_with_config(config);

        // Verify config is stored and accessible.
        assert_eq!(
            idx.checker_config.resolve_severity("BSK-0001", &[]),
            Some(basilisk_config::RuleSeverity::Warning),
            "checker_config must store the rule severity override"
        );
    }

    // ── Config applies across all analysis entry points ─────────────────────

    #[test]
    fn config_applies_to_set_open() {
        let idx =
            make_index_with_rule_override("BSK-0001", basilisk_config::RuleSeverity::Disabled);
        let uri = make_uri("/tmp/cfg_set_open.py");
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);

        let codes = lsp_codes(&lsp_diags);
        assert!(
            !codes.contains(&"BSK-0001".to_owned()),
            "set_open must apply checker_config — disabled BSK-0001 should be absent"
        );
    }

    #[test]
    fn config_applies_to_reload_from_disk() {
        let dir = unique_tmp("bsk_cfg_reload");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("reload_cfg.py");
        std::fs::write(&file_path, SRC_MISSING_ANNOTATION).unwrap();

        let idx =
            make_index_with_rule_override("BSK-0001", basilisk_config::RuleSeverity::Disabled);

        // First, set_open to get it in the index, then close to allow reload.
        let uri = Url::from_file_path(&file_path).unwrap();
        let _ = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        idx.files.get_mut(&file_path).unwrap().is_open = false;

        // Modify the content on disk (different hash) so reload_from_disk runs.
        std::fs::write(
            &file_path,
            "def greet(name):\n    return name\n\n# changed\n",
        )
        .unwrap();

        let result = idx.reload_from_disk(&uri);
        assert!(
            result.is_some(),
            "reload_from_disk should return diagnostics"
        );

        let (_, lsp_diags) = result.unwrap();
        let codes = lsp_codes(&lsp_diags);
        assert!(
            !codes.contains(&"BSK-0001".to_owned()),
            "reload_from_disk must apply checker_config — disabled BSK-0001 should be absent"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reload_root_configs_applies_changed_python_version() {
        // [CHKARCH-VERSION-TARGET] Editing `[tool.basilisk] python-version` must
        // make version-aware rules (the version_target_syntax PEP 695 gate) update without an
        // LSP restart: reload_root_configs re-reads the target, and the next
        // recheck reflects it.
        let dir = unique_tmp("bsk_cfg_pyver");
        std::fs::create_dir_all(&dir).unwrap();
        let write_version = |v: &str| {
            std::fs::write(
                dir.join("pyproject.toml"),
                format!("[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\npython-version = \"{v}\"\n"),
            )
            .unwrap();
        };
        write_version("3.11");
        let src = "type Alias = int\n";
        let file_path = dir.join("pep695.py");
        std::fs::write(&file_path, src).unwrap();

        let mut idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let uri = Url::from_file_path(&file_path).unwrap();
        let recheck_has_e0155 = |idx: &WorkspaceIndex| {
            idx.recheck_all_files()
                .into_iter()
                .find(|(u, _)| *u == uri)
                .is_some_and(|(_, d)| lsp_codes(&d).contains(&"version_target_syntax".to_owned()))
        };

        // 3.11 target: PEP 695 `type` syntax is gated.
        let initial = idx.set_open(&uri, src, 1);
        assert!(
            lsp_codes(&initial).contains(&"version_target_syntax".to_owned()),
            "PEP 695 on a 3.11 target must fire version_target_syntax"
        );

        // Switch the configured target to 3.12 on disk.
        write_version("3.12");

        // A recheck WITHOUT reloading config reuses the target cached at
        // construction — still stale 3.11 (the bug this guards against).
        assert!(
            recheck_has_e0155(&idx),
            "without reload, the recheck still uses the stale 3.11 target"
        );

        // Reloading per-root config picks up 3.12, where PEP 695 is native.
        idx.reload_root_configs();
        assert!(
            !recheck_has_e0155(&idx),
            "reload_root_configs must apply the new python-version (3.12 allows PEP 695)"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_applies_to_set_closed() {
        let dir = unique_tmp("bsk_cfg_closed");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("close_cfg.py");
        std::fs::write(&file_path, SRC_MISSING_ANNOTATION).unwrap();

        let idx =
            make_index_with_rule_override("BSK-0001", basilisk_config::RuleSeverity::Disabled);

        let uri = Url::from_file_path(&file_path).unwrap();
        let _ = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);

        let (_, lsp_diags) = idx.set_closed(&uri);
        let codes = lsp_codes(&lsp_diags);
        assert!(
            !codes.contains(&"BSK-0001".to_owned()),
            "set_closed must apply checker_config — disabled BSK-0001 should be absent"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_honors_include_roots() {
        // [CHKARCH-CONFIG-INCLUDE] The LSP workspace scan must walk only the
        // configured `[tool.basilisk] include` roots, like `basilisk check` —
        // a file outside them (e.g. generated code) must not be scanned.
        let dir = unique_tmp("bsk_scan_include");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("gen")).unwrap();
        std::fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ninclude = [\"src\"]\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/ok.py"),
            "def add(a: int, b: int) -> int:\n    return a + b\n",
        )
        .unwrap();
        // A file OUTSIDE the include roots — must not be scanned.
        std::fs::write(
            dir.join("gen/outside.py"),
            "def bad() -> int:\n    return undefined_name\n",
        )
        .unwrap();

        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let (results, file_count, _errors) = idx.scan();
        let scanned: Vec<String> = results.iter().map(|(u, _)| u.to_string()).collect();

        assert!(
            scanned.iter().any(|u| u.ends_with("src/ok.py")),
            "files inside include roots must be scanned, got: {scanned:?}"
        );
        assert!(
            !scanned.iter().any(|u| u.ends_with("gen/outside.py")),
            "files outside include roots must NOT be scanned, got: {scanned:?}"
        );
        assert_eq!(file_count, 1, "only the included file should be scanned");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_file_outside_include_roots_is_suppressed() {
        // [CHKARCH-CONFIG-INCLUDE] A file outside the include roots must show no
        // diagnostics even when opened on demand — consistent with `exclude`.
        let dir = unique_tmp("bsk_open_outside_include");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join("gen")).unwrap();
        std::fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ninclude = [\"src\"]\n",
        )
        .unwrap();
        let idx = WorkspaceIndex::new(
            vec![dir.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let src = "def f() -> int:\n    return undefined_name\n";

        // Inside the include roots: diagnosed even when opened.
        let inside = Url::from_file_path(dir.join("src/inside.py")).unwrap();
        assert!(
            !idx.set_open(&inside, src, 1).is_empty(),
            "a file inside include roots must be diagnosed when opened"
        );

        // Outside the include roots: suppressed when opened.
        let outside = Url::from_file_path(dir.join("gen/outside.py")).unwrap();
        assert!(
            idx.set_open(&outside, src, 1).is_empty(),
            "a file outside include roots must NOT be diagnosed even when opened"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_applies_to_scan() {
        let dir = unique_tmp("bsk_cfg_scan");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("scan_cfg.py"), SRC_MISSING_ANNOTATION).unwrap();

        let config = BasiliskConfig::with_rule_entries(std::collections::HashMap::from([(
            "BSK-0001".to_owned(),
            basilisk_config::RuleSeverity::Disabled,
        )]));
        let idx = WorkspaceIndex::new(vec![dir.clone()], AnalysisMode::WholeModule, config);

        let (results, file_count, _) = idx.scan();
        assert!(file_count > 0, "scan should find at least one file");

        for (_, lsp_diags) in &results {
            let codes = lsp_codes(lsp_diags);
            assert!(
                !codes.contains(&"BSK-0001".to_owned()),
                "scan must apply checker_config — disabled BSK-0001 should be absent, got {codes:?}"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Multi-root workspace ───────────────────────────────────────────────

    #[test]
    fn multi_root_per_root_config() {
        let root_a = unique_tmp("bsk_multiroot_a");
        let root_b = unique_tmp("bsk_multiroot_b");
        std::fs::create_dir_all(&root_a).unwrap();
        std::fs::create_dir_all(&root_b).unwrap();

        // Root A: disable BSK-0001 via pyproject.toml
        std::fs::write(
            root_a.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-0001\" = \"disabled\"\n",
        )
        .unwrap();
        std::fs::write(root_a.join("a.py"), SRC_MISSING_ANNOTATION).unwrap();

        // Root B: no config file (default rules apply)
        std::fs::write(root_b.join("b.py"), SRC_MISSING_ANNOTATION).unwrap();

        let idx = WorkspaceIndex::new(
            vec![root_a.clone(), root_b.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );

        // Check that root A's config disables BSK-0001
        let cfg_a = idx.config_for_file(&root_a.join("a.py"));
        assert_eq!(
            cfg_a.resolve_severity("BSK-0001", &[]),
            Some(basilisk_config::RuleSeverity::Disabled),
            "root A should have BSK-0001 disabled"
        );

        // Check that root B uses default config (BSK-0001 not overridden)
        let cfg_b = idx.config_for_file(&root_b.join("b.py"));
        assert_eq!(
            cfg_b.resolve_severity("BSK-0001", &[]),
            None,
            "root B should have default config (no BSK-0001 override)"
        );

        let _ = std::fs::remove_dir_all(&root_a);
        let _ = std::fs::remove_dir_all(&root_b);
    }

    #[test]
    fn config_for_file_falls_back_to_default() {
        let root = unique_tmp("bsk_cfgfallback");
        std::fs::create_dir_all(&root).unwrap();

        let idx = WorkspaceIndex::new(
            vec![root.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );

        // File outside any root should fall back to default config.
        let cfg = idx.config_for_file(std::path::Path::new("/nonexistent/foo.py"));
        assert!(
            !cfg.has_config_table(),
            "fallback config should have no rule tables"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn set_root_config_is_authoritative_over_stale_disk_config() {
        // [LSPARCH-CONFIG] via [CHKARCH-CONFIG-DISCOVERY]: after an applied
        // configuration-editor change or an open, unsaved config buffer, the
        // in-memory root config decides. config_for_file must not re-read the
        // root's pyproject.toml from disk and merge its stale severities back
        // over the applied one (crates/basilisk-lsp/src/workspace.rs
        // config_for_file bounded walk).
        let root = unique_tmp("bsk_cfg_authority");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n",
        )
        .unwrap();

        let mut idx = WorkspaceIndex::new(
            vec![root.clone()],
            AnalysisMode::WholeModule,
            BasiliskConfig::default(),
        );
        let file = root.join("app.py");
        assert_eq!(
            idx.config_for_file(&file).resolve_severity("BSK-0001", &[]),
            Some(basilisk_config::RuleSeverity::Error),
            "disk config decides before any in-memory update"
        );

        // The applied document parses exactly as the configuration editor
        // builds it; disk still holds the stale "error" entry.
        let applied = basilisk_config::discover_config_document_with_content(
            &root,
            "[tool.basilisk.rules]\n\"BSK-0001\" = \"warning\"\n".to_owned(),
        )
        .unwrap();
        idx.set_root_config(root.clone(), applied.config);
        assert_eq!(
            idx.config_for_file(&file).resolve_severity("BSK-0001", &[]),
            Some(basilisk_config::RuleSeverity::Warning),
            "the in-memory root config must beat the stale on-disk entry"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    // ── Multiple overrides in one config ────────────────────────────────────

    #[test]
    fn config_multiple_overrides_applied_together() {
        let mut config = annotations_on();
        if let Some(tables) = config.rule_chain.first_mut() {
            let _ = tables.rules.insert(
                "BSK-0001".to_owned(),
                basilisk_config::RuleSeverity::Warning,
            );
            let _ = tables.rules.insert(
                "BSK-0050".to_owned(),
                basilisk_config::RuleSeverity::Disabled,
            );
        }
        let idx = make_index_with_config(config);

        // File with both BSK-0001 and BSK-0050 triggers.
        let uri = make_uri("/tmp/cfg_multi.py");
        let src = "x: int = 42\n\ndef greet(name):\n    return name\n";
        let lsp_diags = idx.set_open(&uri, src, 1);
        let codes = lsp_codes(&lsp_diags);

        // BSK-0050 should be gone (disabled).
        assert!(
            !codes.contains(&"BSK-0050".to_owned()),
            "disabled BSK-0050 must not appear, got {codes:?}"
        );

        // BSK-0001 should be present but demoted to Warning.
        assert!(
            codes.contains(&"BSK-0001".to_owned()),
            "demoted BSK-0001 should still appear, got {codes:?}"
        );

        for d in &lsp_diags {
            if let Some(tower_lsp::lsp_types::NumberOrString::String(code)) = &d.code {
                if code == "BSK-0001" {
                    assert_eq!(
                        d.severity,
                        Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
                        "demoted BSK-0001 must be WARNING in combined config"
                    );
                }
            }
        }

        // Also verify the raw checker diagnostics match.
        let diags = get_diagnostics(&idx, &uri);
        let w0050_count = count_code(&diags, "BSK-0050");
        assert_eq!(w0050_count, 0, "BSK-0050 disabled in checker too");
        for d in diags.iter().filter(|d| d.code.code == "BSK-0001") {
            assert_eq!(d.severity, basilisk_checker::Severity::Warning);
        }
    }

    // ── Severity values are correct LSP numbers ─────────────────────────────

    #[test]
    fn lsp_severity_constants_are_distinct() {
        // LSP protocol: ERROR, WARNING, INFORMATION, HINT must all differ.
        let error = tower_lsp::lsp_types::DiagnosticSeverity::ERROR;
        let warning = tower_lsp::lsp_types::DiagnosticSeverity::WARNING;
        let info = tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION;
        let hint = tower_lsp::lsp_types::DiagnosticSeverity::HINT;

        assert_ne!(error, warning, "ERROR and WARNING must differ");
        assert_ne!(error, info, "ERROR and INFORMATION must differ");
        assert_ne!(error, hint, "ERROR and HINT must differ");
        assert_ne!(warning, info, "WARNING and INFORMATION must differ");
        assert_ne!(warning, hint, "WARNING and HINT must differ");
        assert_ne!(info, hint, "INFORMATION and HINT must differ");
    }

    #[test]
    fn bsk_to_lsp_maps_warning_to_warning_not_error() {
        let diag = make_test_diag(
            "BSK-0050",
            basilisk_checker::Severity::Warning,
            "test warning",
        );
        let lsp_diag = crate::workspace_analysis::bsk_to_lsp(&diag, "x\n");
        assert_eq!(
            lsp_diag.severity,
            Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
            "Warning severity must map to LSP WARNING (2), not ERROR (1)"
        );
        assert_ne!(
            lsp_diag.severity,
            Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
            "Warning must NEVER map to ERROR"
        );
    }

    #[test]
    fn bsk_to_lsp_maps_error_to_error() {
        let diag = make_test_diag("BSK-0001", basilisk_checker::Severity::Error, "test error");
        let lsp_diag = crate::workspace_analysis::bsk_to_lsp(&diag, "x\n");
        assert_eq!(
            lsp_diag.severity,
            Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
            "Error severity must map to LSP ERROR (1)"
        );
    }

    #[test]
    fn bsk_to_lsp_maps_info_to_information() {
        let diag = make_test_diag("BSK-8902", basilisk_checker::Severity::Info, "test info");
        let lsp_diag = crate::workspace_analysis::bsk_to_lsp(&diag, "x\n");
        assert_eq!(
            lsp_diag.severity,
            Some(tower_lsp::lsp_types::DiagnosticSeverity::INFORMATION),
            "Info severity must map to LSP INFORMATION (3)"
        );
    }

    // ── Config loaded from pyproject.toml via WorkspaceIndex constructor ────

    #[cfg(unix)]
    #[test]
    fn multi_root_platform_evidence_is_resolved_per_root() {
        use std::os::unix::fs::PermissionsExt;

        let base = unique_tmp("bsk_multi_root_platform");
        let roots = [base.join("a"), base.join("b")];
        for (root, platform) in roots.iter().zip(["platform-a", "platform-b"]) {
            std::fs::create_dir_all(root).unwrap();
            let interpreter = root.join("python");
            std::fs::write(&interpreter, format!("#!/bin/sh\nprintf '{platform}\\n'\n")).unwrap();
            std::fs::set_permissions(&interpreter, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::write(
                root.join("pyproject.toml"),
                format!("[tool.basilisk]\npython = '{}'\n", interpreter.display()),
            )
            .unwrap();
        }

        let index = WorkspaceIndex::new(
            roots.to_vec(),
            AnalysisMode::OpenFilesOnly,
            BasiliskConfig::default(),
        );

        assert_eq!(
            index
                .root_configs
                .get(&roots[0])
                .and_then(|config| config.python_platform.as_deref()),
            Some("platform-a")
        );
        assert_eq!(
            index
                .root_configs
                .get(&roots[1])
                .and_then(|config| config.python_platform.as_deref()),
            Some("platform-b")
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn workspace_index_with_pyproject_config_applies_overrides() {
        let dir = unique_tmp("bsk_cfg_pyproject");
        std::fs::create_dir_all(&dir).unwrap();

        // Write a pyproject.toml that disables BSK-0001.
        std::fs::write(
            dir.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-0001\" = \"disabled\"\n",
        )
        .unwrap();

        // Write a Python file that triggers BSK-0001.
        std::fs::write(dir.join("check_me.py"), SRC_MISSING_ANNOTATION).unwrap();

        // Load config the same way the LSP init does.
        let config = basilisk_config::load_basilisk_config(&dir);
        assert_eq!(
            config.resolve_severity("BSK-0001", &[]),
            Some(basilisk_config::RuleSeverity::Disabled),
            "pyproject.toml should disable BSK-0001"
        );

        let idx = WorkspaceIndex::new(vec![dir.clone()], AnalysisMode::WholeModule, config);

        // Scan should apply the config.
        let (results, file_count, _) = idx.scan();
        assert!(file_count > 0, "should find at least one file");

        for (_, lsp_diags) in &results {
            let codes = lsp_codes(lsp_diags);
            assert!(
                !codes.contains(&"BSK-0001".to_owned()),
                "pyproject.toml disabled BSK-0001 must not appear in scan results"
            );
        }

        // Also verify via set_open.
        let uri = Url::from_file_path(dir.join("check_me.py")).unwrap();
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        let codes = lsp_codes(&lsp_diags);
        assert!(
            !codes.contains(&"BSK-0001".to_owned()),
            "pyproject.toml disabled BSK-0001 must not appear via set_open either"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// GitHub #311 (CLI⇄LSP parity): per-file rule config must merge a child
    /// directory's config over the workspace-root config, exactly like the
    /// CLI's per-file ancestor walk — not pin every file to the workspace
    /// root's config.
    #[test]
    fn set_open_merges_child_dir_config_over_workspace_root_config() {
        let root = unique_tmp("bsk_cfg_child_merge");
        let child = root.join("child");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(
            root.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n\"BSK-0002\" = \"error\"\n",
        )
        .unwrap();
        std::fs::write(
            child.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-0001\" = \"disabled\"\n",
        )
        .unwrap();
        std::fs::write(child.join("open_me.py"), SRC_MISSING_ANNOTATION).unwrap();

        // Load config the same way the LSP init does for the workspace root.
        let config = basilisk_config::load_basilisk_config(&root);
        let idx = WorkspaceIndex::new(vec![root.clone()], AnalysisMode::WholeModule, config);

        let uri = Url::from_file_path(child.join("open_me.py")).unwrap();
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);
        let _ = std::fs::remove_dir_all(&root);

        let codes = lsp_codes(&lsp_diags);
        assert!(
            codes.contains(&"BSK-0002".to_owned()),
            "the root's rule opt-ins must still apply to files in \
             the child dir (cumulative merge, GitHub #311); got: {codes:?}"
        );
        assert!(
            !codes.contains(&"BSK-0001".to_owned()),
            "the child dir's `BSK-0001 = disabled` must be honored for files \
             under it (CLI⇄LSP parity, GitHub #311); got: {codes:?}"
        );
    }

    #[test]
    fn workspace_index_with_pyproject_demotes_to_warning() {
        let dir = unique_tmp("bsk_cfg_pyproject_demote");
        std::fs::create_dir_all(&dir).unwrap();

        // A non-disabled severity selects this off-by-default rule and demotes it.
        std::fs::write(
            dir.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-0001\" = \"warning\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("demote_me.py"), SRC_MISSING_ANNOTATION).unwrap();

        let config = basilisk_config::load_basilisk_config(&dir);
        let idx = WorkspaceIndex::new(vec![dir.clone()], AnalysisMode::WholeModule, config);

        let uri = Url::from_file_path(dir.join("demote_me.py")).unwrap();
        let lsp_diags = idx.set_open(&uri, SRC_MISSING_ANNOTATION, 1);

        let codes = lsp_codes(&lsp_diags);
        assert!(
            codes.contains(&"BSK-0001".to_owned()),
            "demoted BSK-0001 should still appear"
        );

        for d in &lsp_diags {
            if let Some(tower_lsp::lsp_types::NumberOrString::String(code)) = &d.code {
                if code == "BSK-0001" {
                    assert_eq!(
                        d.severity,
                        Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
                        "pyproject.toml demoted BSK-0001 must be WARNING in LSP"
                    );
                }
            }
        }

        // Verify checker diagnostics too.
        let diags = get_diagnostics(&idx, &uri);
        for d in diags.iter().filter(|d| d.code.code == "BSK-0001") {
            assert_eq!(
                d.severity,
                basilisk_checker::Severity::Warning,
                "pyproject.toml demoted BSK-0001 must be Warning in checker"
            );
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── Default severity vs override produce different results ───────────────

    #[test]
    fn default_severity_and_override_produce_different_severity() {
        // Prove the fix: same source, two configs that both enable the house
        // rule, different severities. See [CHKARCH-CONFIGURATION-ONLY].
        let uri_path = "/tmp/cfg_diff.py";

        // House rules enabled, BSK-0001 at its default severity: Error.
        let default_idx = make_index_with_config(annotations_on());
        let default_uri = make_uri(uri_path);
        let default_diags = default_idx.set_open(&default_uri, SRC_MISSING_ANNOTATION, 1);
        let default_severities: Vec<_> = default_diags
            .iter()
            .filter(|d| {
                matches!(&d.code, Some(tower_lsp::lsp_types::NumberOrString::String(c)) if c == "BSK-0001")
            })
            .filter_map(|d| d.severity)
            .collect();

        // Custom config: BSK-0001 demoted to Warning.
        let custom_idx =
            make_index_with_rule_override("BSK-0001", basilisk_config::RuleSeverity::Warning);
        let custom_uri = make_uri(uri_path);
        let custom_diags = custom_idx.set_open(&custom_uri, SRC_MISSING_ANNOTATION, 1);
        let custom_severities: Vec<_> = custom_diags
            .iter()
            .filter(|d| {
                matches!(&d.code, Some(tower_lsp::lsp_types::NumberOrString::String(c)) if c == "BSK-0001")
            })
            .filter_map(|d| d.severity)
            .collect();

        // Both should have BSK-0001 diagnostics.
        assert!(!default_severities.is_empty(), "default must have BSK-0001");
        assert!(!custom_severities.is_empty(), "custom must have BSK-0001");

        // Default = ERROR, Custom = WARNING.
        assert!(
            default_severities
                .iter()
                .all(|s| *s == tower_lsp::lsp_types::DiagnosticSeverity::ERROR),
            "default config BSK-0001 must be ERROR"
        );
        assert!(
            custom_severities
                .iter()
                .all(|s| *s == tower_lsp::lsp_types::DiagnosticSeverity::WARNING),
            "custom config BSK-0001 must be WARNING"
        );

        // They must differ — this is the core assertion proving the fix.
        assert_ne!(
            default_severities, custom_severities,
            "default and custom configs MUST produce different LSP severities for BSK-0001"
        );
    }
}
