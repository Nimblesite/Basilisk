//! uv binary resolution.
//!
//! Locates the `uv` executable using a priority cascade:
//! 1. Explicit path from config (`basilisk.uv.executablePath`)
//! 2. `UV_PATH` environment variable
//! 3. `~/.cargo/bin/uv`
//! 4. `~/.local/bin/uv`
//! 5. OS PATH search (`which uv`)

use std::path::PathBuf;

/// Well-known filesystem locations where the `uv` binary might live.
const WELL_KNOWN_PATHS: &[&str] = &["~/.cargo/bin/uv", "~/.local/bin/uv"];

/// Locate the `uv` binary using the resolution cascade.
///
/// `config_path` is the user-configured explicit path, if any (from
/// `basilisk.uv.executablePath`). Returns `None` if `uv` cannot be found.
#[must_use]
pub fn find_uv_binary(config_path: Option<&str>) -> Option<PathBuf> {
    // 1. Explicit config path.
    if let Some(explicit) = config_path {
        if !explicit.is_empty() {
            let path = PathBuf::from(explicit);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    // 2. UV_PATH environment variable.
    if let Ok(env_path) = std::env::var("UV_PATH") {
        let path = PathBuf::from(&env_path);
        if path.is_file() {
            return Some(path);
        }
    }

    // 3. Well-known filesystem locations.
    if let Some(home) = home_dir() {
        for template in WELL_KNOWN_PATHS {
            let expanded = template.replacen('~', &home.display().to_string(), 1);
            let path = PathBuf::from(&expanded);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    // 4. OS PATH search.
    which_uv()
}

/// Check whether `uv` is available (any resolution succeeds).
#[must_use]
pub fn is_uv_available(config_path: Option<&str>) -> bool {
    find_uv_binary(config_path).is_some()
}

/// Get the user's home directory.
fn home_dir() -> Option<PathBuf> {
    // Use HOME env var (Unix) or USERPROFILE (Windows).
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

/// Search OS PATH for `uv` using `which`.
fn which_uv() -> Option<PathBuf> {
    let output = std::process::Command::new("which")
        .arg("uv")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let path_str = String::from_utf8_lossy(&output.stdout);
    let trimmed = path_str.trim();
    if trimmed.is_empty() {
        return None;
    }

    let path = PathBuf::from(trimmed);
    if path.is_file() {
        Some(path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_with_nonexistent_config_path_falls_through() {
        let result = find_uv_binary(Some("/nonexistent/path/to/uv"));
        // Should fall through to other resolution steps (may or may not find uv).
        // We just verify it doesn't panic.
        let _ = result;
    }

    #[test]
    fn find_with_empty_config_path_falls_through() {
        let result = find_uv_binary(Some(""));
        let _ = result;
    }

    #[test]
    fn find_with_none_config_path() {
        let result = find_uv_binary(None);
        let _ = result;
    }

    #[test]
    fn is_uv_available_returns_bool() {
        // Just verify it doesn't panic.
        let _ = is_uv_available(None);
    }

    #[test]
    fn home_dir_returns_something_on_unix() {
        // On any Unix-like system, HOME should be set.
        if std::env::var("HOME").is_ok() {
            assert!(home_dir().is_some());
        }
    }
}
