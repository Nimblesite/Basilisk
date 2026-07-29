//! Implements [CHKCACHE-LSP]. See docs/specs/CHECKER-CACHE-SPEC.md#CHKCACHE-LSP
//!
//! The language server's half of the opt-in persistent result cache
//! ([CHKCACHE-CONFIG]): the cold workspace scan replays entries whose
//! fingerprint and read-set still match, and stores fresh results for the
//! files it had to check in full — through the same shared core
//! ([`basilisk_checker::result_cache`]) `basilisk check` uses, so the two
//! surfaces provably run one cache (GitHub #367).
//!
//! Scope is deliberately the **initial scan only**: in-session invalidation
//! belongs to Salsa ([CHKCACHE-POSITIONING-SALSA]) — a live session already
//! knows which file changed, and re-verifying a disk cache on every keystroke
//! would buy nothing. The cold start is exactly the "repeated batch run over
//! a mostly-unchanged tree" the cache exists for
//! ([CHKCACHE-POSITIONING-WATCHER]).
//!
//! The cache engages only in `wholeModule` mode. In `crossModule` mode both
//! halves would be unsound: replayed entries carry CLI-parity diagnostics and
//! would drop cross-only findings, and the cross queries serve
//! [`module_exports`]/[`external_module`] memos across importers, so a
//! per-file read recorder misses dependencies first read during another
//! file's analysis — a stored entry would then survive edits it should not.
//!
//! [`module_exports`]: basilisk_checker::incremental::module_exports
//! [`external_module`]: basilisk_checker::incremental::external_module

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use basilisk_checker::result_cache::{self, CacheContext, CacheOptions, CacheOverride, CacheStats};
use basilisk_common::fs::{canonical_key, content_hash, ReadRecorder};
use basilisk_config::BasiliskConfig;

use crate::config::AnalysisMode;
use crate::workspace::{FileEntry, WorkspaceIndex};
use crate::workspace_analysis::{bsk_to_lsp, fnv1a, make_entry};

/// Per-scan persistent-cache state: one [`CacheContext`] per cache-enabled
/// workspace root, the engine's pre-scan tracked set (the store gate), and
/// the running hit/miss tally.
pub(crate) struct ScanCache {
    /// Cache context per owning workspace root; empty when the cache is
    /// disabled (config off, or a non-`wholeModule` mode).
    contexts: HashMap<PathBuf, CacheContext>,
    /// Paths the salsa engine already tracked before this scan primed it.
    /// Their memos are warm, so a scan-time recorder cannot capture their
    /// read-sets — storing those entries would be unsound.
    pre_tracked: HashSet<PathBuf>,
    /// Hit/miss tally for the end-of-scan log line.
    stats: CacheStats,
}

impl ScanCache {
    /// Log the scan's cache outcome; a tally nobody can observe is
    /// indistinguishable from a cache that never ran.
    pub(crate) fn log_outcome(&self) {
        if !self.contexts.is_empty() {
            tracing::info!(
                hits = self.stats.hits,
                misses = self.stats.misses,
                "persistent result cache served the cold scan"
            );
        }
    }
}

impl WorkspaceIndex {
    /// Build the persistent-cache state for one cold scan.
    ///
    /// MUST be called **before** the scan primes the salsa engine: the
    /// pre-scan tracked snapshot is what distinguishes files whose queries
    /// will genuinely execute under the recorder from files with warm memos.
    #[must_use]
    pub(crate) fn begin_scan_cache(&self, to_analyse: &[(PathBuf, String)]) -> ScanCache {
        let pre_tracked = self.salsa_engine.tracked_paths();
        if !matches!(self.mode(), AnalysisMode::WholeModule) {
            // Cross-module diagnostics are not CLI-parity and their queries
            // share memos across importers — both replay and store would be
            // unsound there (see the module docs).
            tracing::debug!(mode = ?self.mode(), "persistent result cache: disabled outside wholeModule");
            return ScanCache {
                contexts: HashMap::new(),
                pre_tracked,
                stats: CacheStats::default(),
            };
        }
        ScanCache {
            contexts: self.scan_cache_contexts(to_analyse),
            pre_tracked,
            stats: CacheStats::default(),
        }
    }

    /// One [`CacheContext`] per root whose project config enables the cache,
    /// fingerprinted over the same inputs as the CLI ([CHKCACHE-FINGERPRINT]):
    /// the per-directory config map of the files this scan will analyse, the
    /// root's import search paths, and its active typeshed identity.
    fn scan_cache_contexts(
        &self,
        to_analyse: &[(PathBuf, String)],
    ) -> HashMap<PathBuf, CacheContext> {
        let mut files_by_root: HashMap<PathBuf, Vec<&Path>> = HashMap::new();
        for (path, _text) in to_analyse {
            if let Some(root) = self.owning_root_for_path(path) {
                files_by_root.entry(root.clone()).or_default().push(path);
            }
        }
        files_by_root
            .into_iter()
            .filter_map(|(root, files)| {
                let config = self.root_configs.get(&root)?;
                if !config.cache_is_enabled() {
                    return None;
                }
                let dir_configs: BTreeMap<PathBuf, Arc<BasiliskConfig>> = files
                    .iter()
                    .copied()
                    .map(|path| (Self::config_root_key(path), self.config_for_file(path)))
                    .collect();
                let search_paths = self
                    .search_paths_for_file(files.first().copied()?)
                    .map(|(_, paths)| paths);
                // Before the first scan installs search paths (unit tests,
                // pre-config scans) the empty environment is hashed — identical
                // values hash identically, so that state stays stable too.
                let empty = crate::import_resolver::ImportSearchPaths::default();
                let context = result_cache::build_context(
                    &CacheOptions {
                        enabled: CacheOverride::Project,
                        dir: None,
                        stats: false,
                    },
                    config,
                    &dir_configs,
                    search_paths.as_deref().unwrap_or(&empty),
                    &root,
                )?;
                Some((root, context))
            })
            .collect()
    }

    /// Analyse one scanned file through the persistent cache: replay on a
    /// hit, full analysis (recorded and stored) on a miss, and the plain
    /// uncached path when no context covers the file.
    pub(crate) fn analyse_scanned(
        &self,
        cache: &mut ScanCache,
        text: &str,
        path: &Path,
    ) -> (FileEntry, Vec<tower_lsp::lsp_types::Diagnostic>) {
        let Some(context) = self
            .owning_root_for_path(path)
            .and_then(|root| cache.contexts.get(root))
        else {
            return self.analyse_and_resolve(text, path);
        };

        if let Some(diagnostics) = result_cache::lookup_diagnostics(context, path) {
            cache.stats.hits += 1;
            return self.replay_scanned_entry(text, path, diagnostics);
        }
        cache.stats.misses += 1;

        let recorder = ReadRecorder::start();
        let (entry, lsp_diags) = self.analyse_and_resolve(text, path);
        let read_set = recorder.finish();

        let suppressed = self.is_path_excluded(path) || self.is_outside_include_roots(path);
        if cache.pre_tracked.contains(path) || !execution_observed(&entry, &read_set) {
            // Warm memos execute nothing under the recorder — whether warmed
            // before the scan (`pre_tracked`) or by a concurrent handler
            // mid-scan (the scan is spawned precisely so requests keep being
            // served). Either way the captured read-set is incomplete and
            // must not be persisted; skipping the store only costs a future
            // cold check, never correctness.
            tracing::debug!(path = %path.display(), "cache store skipped: warm salsa memos hide the read-set");
        } else if entry.resolved.is_some() && !suppressed {
            // The target itself enters the engine from memory, never through
            // a tracked disk read — seed it, or the entry would replay after
            // edits ([CHKCACHE-LSP-STORE]).
            let mut read_set = read_set;
            let _ = read_set.insert(canonical_key(path), content_hash(text));
            if let Err(err) =
                result_cache::store_diagnostics(context, path, read_set, &entry.diagnostics)
            {
                tracing::warn!(path = %path.display(), %err, "failed to write cache entry");
            }
        }
        (entry, lsp_diags)
    }

    /// Materialise a [`FileEntry`] from replayed diagnostics: the memoized
    /// parse + resolve still runs (navigation must not degrade on a hit) but
    /// the check step — the expensive half — is skipped entirely.
    fn replay_scanned_entry(
        &self,
        text: &str,
        path: &Path,
        diagnostics: Vec<basilisk_checker::Diagnostic>,
    ) -> (FileEntry, Vec<tower_lsp::lsp_types::Diagnostic>) {
        let resolved = self.resolve_for_navigation(text, path);
        let mut entry = make_entry(fnv1a(text), text, resolved, diagnostics);
        // Suppression parity with `analyse_and_resolve`: excluded and
        // out-of-include files never publish diagnostics
        // ([CHKARCH-CONFIG-EXCLUDE] / [CHKARCH-CONFIG-INCLUDE]).
        if self.is_path_excluded(path) || self.is_outside_include_roots(path) {
            entry.diagnostics.clear();
            return (entry, Vec::new());
        }
        let lsp_diags = entry
            .diagnostics
            .iter()
            .map(|diagnostic| bsk_to_lsp(diagnostic, text))
            .collect();
        (entry, lsp_diags)
    }

    /// The import-resolved module for a replayed file, via the memoized
    /// engine when search paths exist (identical to a checked file's view)
    /// and a plain parse + resolve before the first scan installs them.
    fn resolve_for_navigation(
        &self,
        text: &str,
        path: &Path,
    ) -> Option<Arc<basilisk_resolver::ResolvedModule>> {
        if let Some((root, search_paths)) = self.search_paths_for_file(path) {
            return self
                .salsa_engine
                .resolve_only(path, text, &root, &search_paths);
        }
        let parsed =
            basilisk_parser::parse_source(text.to_owned(), path.to_string_lossy().into_owned())
                .ok()?;
        basilisk_resolver::resolve(&parsed).ok().map(Arc::new)
    }
}

/// Whether the analysis provably *executed* under the scan's recorder, rather
/// than being served from memos a concurrent handler warmed mid-scan.
///
/// A cold execution of a file with any import resolved to an on-disk file
/// reads that file through the tracked path, so an empty capture alongside
/// such imports means the memo was warm and the read-set is incomplete.
/// Imports that resolve elsewhere (typeshed snapshots live in an in-memory
/// archive) never read the filesystem, so an import-free — or disk-free —
/// module's empty capture is genuinely complete. Erring the other way only
/// skips a store, never poisons one.
fn execution_observed(entry: &FileEntry, read_set: &basilisk_common::fs::ReadSet) -> bool {
    if !read_set.is_empty() {
        return true;
    }
    let Some(resolved) = entry.resolved.as_ref() else {
        return true;
    };
    !resolved
        .imports
        .iter()
        .filter_map(|import| import.resolved_path.as_ref())
        .any(|resolved_path| resolved_path.exists())
}
