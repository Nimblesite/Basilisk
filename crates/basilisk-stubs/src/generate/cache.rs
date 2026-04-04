//! Cache management for generated stubs.
//!
//! Generated stubs are stored in `.basilisk/stubs/{module_name}.pyi`.
//! Cache invalidation uses a hash of the source file content + Python version.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Default cache directory relative to the project root.
pub const DEFAULT_CACHE_DIR: &str = ".basilisk/stubs";

/// Compute the cache path for a module's generated stub.
#[must_use]
pub fn cache_path(cache_dir: &Path, module_name: &str) -> PathBuf {
    let filename = format!("{}.pyi", module_name.replace('.', "/"));
    cache_dir.join(filename)
}

/// Check if a cached stub exists and is still valid.
///
/// A cached stub is valid if the source file hasn't changed since it was
/// generated.  Validity is checked by comparing the source hash stored in
/// the stub comment header against the current source hash.
#[must_use]
pub fn is_cache_valid(cache_dir: &Path, module_name: &str, source_hash: u64) -> bool {
    let path = cache_path(cache_dir, module_name);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return false;
    };

    // Look for the hash comment in the first few lines.
    for line in content.lines().take(5) {
        if let Some(hash_str) = line.strip_prefix("# source-hash: ") {
            if let Ok(cached_hash) = hash_str.trim().parse::<u64>() {
                return cached_hash == source_hash;
            }
        }
    }

    false
}

/// Write a generated stub to the cache directory.
///
/// Prepends a source hash comment for cache invalidation.
///
/// # Errors
///
/// Returns `std::io::Error` if the directory cannot be created or the
/// file cannot be written.
pub fn write_cache(
    cache_dir: &Path,
    module_name: &str,
    pyi_content: &str,
    source_hash: u64,
) -> std::io::Result<PathBuf> {
    let path = cache_path(cache_dir, module_name);

    // Create parent directories.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = format!("# source-hash: {source_hash}\n{pyi_content}");
    std::fs::write(&path, content)?;

    Ok(path)
}

/// Compute a hash of file contents for cache invalidation.
#[must_use]
pub fn hash_source(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_path_simple_module() {
        let path = cache_path(Path::new(".basilisk/stubs"), "requests");
        assert_eq!(path, PathBuf::from(".basilisk/stubs/requests.pyi"));
    }

    #[test]
    fn cache_path_dotted_module() {
        let path = cache_path(Path::new(".basilisk/stubs"), "requests.auth");
        assert_eq!(path, PathBuf::from(".basilisk/stubs/requests/auth.pyi"));
    }

    #[test]
    fn cache_roundtrip() {
        let dir = std::env::temp_dir().join("bsk_cache_test_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);

        let content = "def get(url: str) -> Any: ...\n";
        let hash = hash_source("original source");

        let path = write_cache(&dir, "requests", content, hash).unwrap();
        assert!(path.exists());

        assert!(is_cache_valid(&dir, "requests", hash));
        assert!(!is_cache_valid(&dir, "requests", hash.wrapping_add(1)));
        assert!(!is_cache_valid(&dir, "nonexistent", hash));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hash_source_deterministic() {
        let h1 = hash_source("hello world");
        let h2 = hash_source("hello world");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_source_different_inputs() {
        let h1 = hash_source("hello");
        let h2 = hash_source("world");
        assert_ne!(h1, h2);
    }
}
