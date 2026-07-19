//! Implements [CHKCACHE-CLI] / [CHKCACHE-FINGERPRINT].
//! See docs/specs/CHECKER-CACHE-SPEC.md#chkcache-cli
//!
//! CLI glue for the opt-in result cache: turns the `--cache*` flags into a
//! [`CacheContext`], wraps the per-file cold check with a lookup/store, and
//! tracks hit/miss counts.

use std::path::{Path, PathBuf};

use basilisk_checker::{CachedDiagnostic, Diagnostic};
use basilisk_common::fs::{content_hash, ReadRecorder};
use basilisk_config::BasiliskConfig;
use basilisk_db::cache::{CheckCache, Fingerprint};
use basilisk_lsp::import_resolver::ImportSearchPaths;

/// Parsed `--cache*` flags.
#[derive(Debug, Clone)]
pub struct CacheOptions {
    /// Whether the cache is enabled (`--cache`).
    pub enabled: bool,
    /// Override for the cache directory (`--cache-dir`).
    pub dir: Option<PathBuf>,
    /// Whether to print hit/miss stats (`--cache-stats`).
    pub stats: bool,
}

/// Running hit/miss tally for one `check` invocation.
#[derive(Debug, Default)]
pub struct CacheStats {
    /// Number of cache hits.
    pub hits: usize,
    /// Number of cache misses (full checks).
    pub misses: usize,
}

impl CacheStats {
    /// Print the tally to stderr (kept off stdout so JSON output stays clean).
    pub fn report(&self) {
        eprintln!("cache: {} hit / {} miss", self.hits, self.misses);
    }
}

/// A built cache plus the fingerprint of the non-file inputs for this run.
#[derive(Debug)]
pub struct CacheContext {
    cache: CheckCache,
    fingerprint: Fingerprint,
}

/// Build a [`CacheContext`] when the cache is enabled, else `None`.
///
/// `dir_configs` is the per-directory rule-config map for this run
/// ([CHKARCH-CONFIG-DISCOVERY]) — every directory's config participates in
/// the fingerprint so a child config edit invalidates cached results.
#[must_use]
pub fn build_context(
    options: &CacheOptions,
    dir_configs: &std::collections::BTreeMap<PathBuf, std::sync::Arc<BasiliskConfig>>,
    search_paths: &ImportSearchPaths,
    project_root: &Path,
) -> Option<CacheContext> {
    if !options.enabled {
        return None;
    }
    let dir = options
        .dir
        .clone()
        .unwrap_or_else(|| default_cache_dir(project_root));
    let fingerprint = Fingerprint {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        config_hash: hash_dir_configs(dir_configs),
        env_hash: hash_env(search_paths, project_root),
        typeshed_id: typeshed_snapshot_identity(search_paths),
    };
    Some(CacheContext {
        cache: CheckCache::new(dir),
        fingerprint,
    })
}

/// Default cache location: `<project-root>/.basilisk/cache/check`.
fn default_cache_dir(project_root: &Path) -> PathBuf {
    project_root.join(".basilisk").join("cache").join("check")
}

/// Identity of the active step-3 typeshed snapshot for the fingerprint
/// ([STUBRES-TYPESHED], [CHKCACHE-FINGERPRINT]).
///
/// The gate-accepted snapshot is the only step-3 identity. Configuration
/// values cannot substitute for bytes the checker actually consumed.
fn typeshed_snapshot_identity(search_paths: &ImportSearchPaths) -> String {
    search_paths.typeshed_snapshot.as_ref().map_or_else(
        || "unavailable".to_owned(),
        basilisk_checker::imports::ActiveTypeshed::identity_fingerprint,
    )
}

/// Hash the *effective* per-directory configs. Canonicalised through
/// `serde_json::Value` so the hash is stable across runs despite `HashMap`
/// iteration order; the `BTreeMap` fixes the directory order.
fn hash_dir_configs(
    dir_configs: &std::collections::BTreeMap<PathBuf, std::sync::Arc<BasiliskConfig>>,
) -> u64 {
    let parts: Vec<String> = dir_configs
        .iter()
        .map(|(dir, config)| {
            let json = serde_json::to_value(config.as_ref())
                .ok()
                .and_then(|value| serde_json::to_string(&value).ok())
                .unwrap_or_default();
            format!("{}={json}", dir.display())
        })
        .collect();
    content_hash(&parts.join("\n"))
}

/// Hash the resolution environment: search paths plus `uv.lock` contents.
///
/// This is the v1 boundary: site-packages changes without a `uv.lock` edit are
/// not detected, which is why the cache is opt-in.
// Implements [CHKCACHE-LIMITS]
fn hash_env(search_paths: &ImportSearchPaths, project_root: &Path) -> u64 {
    let mut parts = vec![paths_field("roots", &search_paths.roots)];
    parts.push(paths_field("extra", &search_paths.extra_paths));
    parts.push(paths_field("stub", &search_paths.stub_paths));
    parts.push(paths_field("members", &search_paths.workspace_members));
    let site = search_paths
        .site_packages
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    parts.push(format!("site={site}"));
    parts.push(format!("registry={}", search_paths.registry.is_some()));
    if let Ok(lock) = std::fs::read_to_string(project_root.join("uv.lock")) {
        parts.push(format!("lock={}", content_hash(&lock)));
    }
    content_hash(&parts.join("\n"))
}

/// Render a labelled, order-preserving list of paths for the env fingerprint.
fn paths_field(label: &str, paths: &[PathBuf]) -> String {
    let joined = paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("{label}=[{joined}]")
}

/// Run a single file's check, served from cache when possible.
///
/// On a miss, `cold` runs under a [`ReadRecorder`] so the exact read-set is
/// captured and stored. On a hit, the stored diagnostics are replayed and the
/// target source is re-read for rendering.
///
/// # Errors
///
/// Propagates `cold`'s error, or an I/O error reading the source on a hit.
pub fn check_file<F>(
    context: Option<&CacheContext>,
    stats: &mut CacheStats,
    path: &str,
    cold: F,
) -> Result<(Vec<Diagnostic>, String), String>
where
    F: FnOnce() -> Result<(Vec<Diagnostic>, String), String>,
{
    let Some(context) = context else {
        return cold();
    };
    let target = Path::new(path);
    if let Some(hit) = context
        .cache
        .lookup::<Vec<CachedDiagnostic>>(target, &context.fingerprint)
    {
        stats.hits += 1;
        let source = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
        let diagnostics = hit
            .into_iter()
            .map(CachedDiagnostic::into_diagnostic)
            .collect();
        return Ok((diagnostics, source));
    }
    stats.misses += 1;
    store_fresh(context, target, path, cold)
}

/// Run `cold` under a recorder and persist the result.
fn store_fresh<F>(
    context: &CacheContext,
    target: &Path,
    path: &str,
    cold: F,
) -> Result<(Vec<Diagnostic>, String), String>
where
    F: FnOnce() -> Result<(Vec<Diagnostic>, String), String>,
{
    let recorder = ReadRecorder::start();
    let result = cold();
    let read_set = recorder.finish();
    let (diagnostics, source) = result?;
    let cached: Vec<CachedDiagnostic> = diagnostics.iter().map(CachedDiagnostic::from).collect();
    match context
        .cache
        .store(target, &context.fingerprint, read_set, &cached)
    {
        Ok(()) => tracing::debug!(path, "cache miss: stored fresh result"),
        Err(err) => tracing::warn!(path, %err, "failed to write cache entry"),
    }
    Ok((diagnostics, source))
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only fixed Snapshot fixtures must fail loudly"
)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use basilisk_checker::imports::ActiveTypeshed;
    use basilisk_stubs::typeshed::archive::{Archive, ArchiveEntry, ArchiveVfs};
    use basilisk_stubs::typeshed::gittree::{FileMode, Oid};
    use basilisk_stubs::typeshed::snapshot::Snapshot;
    use basilisk_stubs::typeshed::source::{
        LicenseStatus, Provenance, SourceIdentity, SourceKind, Transport, TypeshedStatus,
    };

    use super::{build_context, typeshed_snapshot_identity, CacheContext, CacheOptions};

    fn snapshot(identity: SourceIdentity) -> Arc<Snapshot> {
        let status = TypeshedStatus {
            active_source: if matches!(identity, SourceIdentity::Custom { .. }) {
                SourceKind::Custom
            } else {
                SourceKind::ExactCommit
            },
            commit: identity.commit(),
            tree: identity.commit(),
            transport: if matches!(identity, SourceIdentity::Custom { .. }) {
                Transport::CustomPath
            } else {
                Transport::Codeload
            },
            license_status: if matches!(identity, SourceIdentity::Custom { .. }) {
                LicenseStatus::NotSupplied
            } else {
                LicenseStatus::Approved
            },
            license_reference: None,
            provenance: if matches!(identity, SourceIdentity::Custom { .. }) {
                Provenance::UserManaged
            } else {
                Provenance::GithubTlsAttested
            },
            signed_release: false,
            warnings: Vec::new(),
        };
        let archive = Archive::new(vec![
            ArchiveEntry {
                path: "stdlib/VERSIONS".to_owned(),
                mode: FileMode::Regular,
                data: b"os: 3.0-\n".to_vec(),
            },
            ArchiveEntry {
                path: "stdlib/os.pyi".to_owned(),
                mode: FileMode::Regular,
                data: b"name: str\n".to_vec(),
            },
        ]);
        let uri = identity.uri_component();
        Arc::new(
            Snapshot::build(identity, status, ArchiveVfs::new(uri, archive), None)
                .expect("valid cache-identity fixture"),
        )
    }

    fn fingerprint(snapshot: Arc<Snapshot>) -> String {
        let mut paths = crate::import_search::roots_only(Vec::new());
        paths.typeshed_snapshot = Some(ActiveTypeshed::new(snapshot, None));
        typeshed_snapshot_identity(&paths)
    }

    fn cache_context(cache_dir: &std::path::Path, snapshot: Arc<Snapshot>) -> CacheContext {
        let mut paths = crate::import_search::roots_only(Vec::new());
        paths.typeshed_snapshot = Some(ActiveTypeshed::new(snapshot, None));
        build_context(
            &CacheOptions {
                enabled: true,
                dir: Some(cache_dir.to_path_buf()),
                stats: false,
            },
            &BTreeMap::new(),
            &paths,
            cache_dir,
        )
        .expect("enabled cache context")
    }

    #[test]
    fn active_snapshot_identity_distinguishes_commits_custom_and_bundle() {
        let commit_a =
            Oid::from_hex("1111111111111111111111111111111111111111").expect("valid commit A");
        let commit_b =
            Oid::from_hex("2222222222222222222222222222222222222222").expect("valid commit B");
        let a = fingerprint(snapshot(SourceIdentity::Commit {
            commit: commit_a,
            pinned: true,
        }));
        let same_a = fingerprint(snapshot(SourceIdentity::Commit {
            commit: commit_a,
            pinned: false,
        }));
        let b = fingerprint(snapshot(SourceIdentity::Commit {
            commit: commit_b,
            pinned: false,
        }));
        let custom = fingerprint(snapshot(SourceIdentity::Custom {
            digest: "custom-tree".to_owned(),
        }));
        let bundled = fingerprint(snapshot(SourceIdentity::Bundled { commit: commit_a }));

        assert_eq!(a, same_a, "pin policy does not change identical bytes");
        assert_ne!(a, b);
        assert_ne!(a, custom);
        assert_ne!(a, bundled);
    }

    #[test]
    fn checker_cache_hits_only_for_the_identical_active_snapshot_identity() {
        let directory = tempfile::tempdir().expect("cache directory");
        let target = std::path::Path::new("/workspace/module.py");
        let commit_a = Oid::from_hex("1111111111111111111111111111111111111111").expect("commit A");
        let commit_b = Oid::from_hex("2222222222222222222222222222222222222222").expect("commit B");
        let stored = cache_context(
            directory.path(),
            snapshot(SourceIdentity::Commit {
                commit: commit_a,
                pinned: true,
            }),
        );
        let payload = vec!["cached diagnostics".to_owned()];
        stored
            .cache
            .store(target, &stored.fingerprint, BTreeMap::new(), &payload)
            .expect("store checker result");

        let identical = cache_context(
            directory.path(),
            snapshot(SourceIdentity::Commit {
                commit: commit_a,
                pinned: false,
            }),
        );
        assert_eq!(
            identical
                .cache
                .lookup::<Vec<String>>(target, &identical.fingerprint),
            Some(payload)
        );

        for identity in [
            SourceIdentity::Commit {
                commit: commit_b,
                pinned: false,
            },
            SourceIdentity::Custom {
                digest: "custom-tree".to_owned(),
            },
            SourceIdentity::Bundled { commit: commit_a },
        ] {
            let changed = cache_context(directory.path(), snapshot(identity));
            assert!(changed
                .cache
                .lookup::<Vec<String>>(target, &changed.fingerprint)
                .is_none());
        }
    }
}
