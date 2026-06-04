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
#[must_use]
pub fn build_context(
    options: &CacheOptions,
    config: &BasiliskConfig,
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
        config_hash: hash_config(config),
        env_hash: hash_env(search_paths, project_root),
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

/// Hash the *effective* config. Canonicalised through `serde_json::Value` so the
/// hash is stable across runs despite `HashMap` iteration order.
fn hash_config(config: &BasiliskConfig) -> u64 {
    serde_json::to_value(config)
        .ok()
        .and_then(|value| serde_json::to_string(&value).ok())
        .map_or(0, |json| content_hash(&json))
}

/// Hash the resolution environment: search paths plus `uv.lock` contents.
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
