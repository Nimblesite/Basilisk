//! Implements [LSPUV]. See docs/specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV
//! uv workspace configuration parsing.
//!
//! Reads `[tool.uv.workspace]` from `pyproject.toml` and resolves glob
//! patterns to actual member directories.

use std::path::{Path, PathBuf};

use crate::error::UvError;

/// Parsed uv workspace configuration.
#[derive(Debug, Clone)]
pub struct UvWorkspace {
    /// Resolved member directory paths.
    pub members: Vec<PathBuf>,
    /// Exclude patterns from the workspace configuration.
    pub exclude: Vec<String>,
}

/// Intermediate TOML structure for `pyproject.toml`.
#[derive(serde::Deserialize)]
struct PyProjectToml {
    #[serde(default)]
    tool: Option<ToolSection>,
}

/// `[tool]` section.
#[derive(serde::Deserialize)]
struct ToolSection {
    #[serde(default)]
    uv: Option<UvSection>,
}

/// `[tool.uv]` section.
#[derive(serde::Deserialize)]
struct UvSection {
    #[serde(default)]
    workspace: Option<WorkspaceSection>,
}

/// `[tool.uv.workspace]` section.
#[derive(serde::Deserialize)]
struct WorkspaceSection {
    #[serde(default)]
    members: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

/// Parse the uv workspace configuration from `pyproject.toml` at the given
/// root directory.
///
/// Returns `Ok(None)` if the file exists but contains no
/// `[tool.uv.workspace]` section. Returns `Ok(None)` if no `pyproject.toml`
/// exists.
///
/// # Errors
///
/// Returns [`UvError::TomlParse`] if the file is malformed TOML.
pub fn parse_uv_workspace(root: &Path) -> Result<Option<UvWorkspace>, UvError> {
    let path = root.join("pyproject.toml");

    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(UvError::Io {
                path: path.display().to_string(),
                source,
            });
        }
    };

    let pyproject: PyProjectToml =
        toml::from_str(&content).map_err(|source| UvError::TomlParse {
            path: path.display().to_string(),
            source,
        })?;

    let workspace_section = pyproject
        .tool
        .and_then(|tool| tool.uv)
        .and_then(|uv| uv.workspace);

    let Some(ws) = workspace_section else {
        return Ok(None);
    };

    let members = resolve_member_patterns(root, &ws.members);

    Ok(Some(UvWorkspace {
        members,
        exclude: ws.exclude,
    }))
}

/// Resolve glob-like member patterns to actual directory paths.
///
/// Supports simple `*` wildcards at the end of a path segment (e.g.
/// `packages/*`). Non-wildcard patterns are treated as literal paths.
fn resolve_member_patterns(root: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let mut result = Vec::new();

    for pattern in patterns {
        if pattern.ends_with("/*") {
            let prefix = &pattern[..pattern.len() - 2];
            let parent = root.join(prefix);
            if let Ok(entries) = std::fs::read_dir(&parent) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.is_dir() {
                        result.push(path);
                    }
                }
            }
        } else {
            let member_path = root.join(pattern);
            if member_path.is_dir() {
                result.push(member_path);
            }
        }
    }

    result.sort();
    result
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test-only: unwrap/indexing acceptable in unit tests"
)]
mod tests {
    use super::*;

    #[test]
    fn parses_workspace_with_members_and_excludes() {
        let dir = tempfile::tempdir().unwrap();
        let member_a = dir.path().join("packages").join("alpha");
        let member_b = dir.path().join("packages").join("beta");
        std::fs::create_dir_all(&member_a).unwrap();
        std::fs::create_dir_all(&member_b).unwrap();

        let pyproject = r#"
[tool.uv.workspace]
members = ["packages/*"]
exclude = ["packages/beta"]
"#;
        std::fs::write(dir.path().join("pyproject.toml"), pyproject).unwrap();

        let ws = parse_uv_workspace(dir.path()).unwrap().unwrap();
        assert_eq!(ws.members.len(), 2);
        assert!(ws.members.contains(&member_a));
        assert!(ws.members.contains(&member_b));
        assert_eq!(ws.exclude, vec!["packages/beta".to_owned()]);
    }

    #[test]
    fn returns_none_without_workspace_section() {
        let dir = tempfile::tempdir().unwrap();
        let pyproject = "[project]\nname = \"foo\"\n";
        std::fs::write(dir.path().join("pyproject.toml"), pyproject).unwrap();

        let result = parse_uv_workspace(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn returns_none_when_no_pyproject() {
        let dir = tempfile::tempdir().unwrap();

        let result = parse_uv_workspace(dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn errors_on_malformed_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pyproject.toml"), "not { valid toml").unwrap();

        let result = parse_uv_workspace(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn literal_member_path() {
        let dir = tempfile::tempdir().unwrap();
        let member = dir.path().join("lib");
        std::fs::create_dir_all(&member).unwrap();

        let pyproject = "[tool.uv.workspace]\nmembers = [\"lib\"]\n";
        std::fs::write(dir.path().join("pyproject.toml"), pyproject).unwrap();

        let ws = parse_uv_workspace(dir.path()).unwrap().unwrap();
        assert_eq!(ws.members, vec![member]);
    }

    #[test]
    fn skips_nonexistent_literal_member() {
        let dir = tempfile::tempdir().unwrap();

        let pyproject = "[tool.uv.workspace]\nmembers = [\"nonexistent\"]\n";
        std::fs::write(dir.path().join("pyproject.toml"), pyproject).unwrap();

        let ws = parse_uv_workspace(dir.path()).unwrap().unwrap();
        assert!(ws.members.is_empty());
    }

    #[test]
    fn empty_members_and_exclude() {
        let dir = tempfile::tempdir().unwrap();

        let pyproject = "[tool.uv.workspace]\nmembers = []\nexclude = []\n";
        std::fs::write(dir.path().join("pyproject.toml"), pyproject).unwrap();

        let ws = parse_uv_workspace(dir.path()).unwrap().unwrap();
        assert!(ws.members.is_empty());
        assert!(ws.exclude.is_empty());
    }

    #[test]
    fn glob_skips_files_in_member_dir() {
        let dir = tempfile::tempdir().unwrap();
        let packages = dir.path().join("packages");
        std::fs::create_dir_all(packages.join("real_pkg")).unwrap();
        // Create a file (not a directory) — should be skipped
        std::fs::write(packages.join("README.md"), "# hi").unwrap();

        let pyproject = "[tool.uv.workspace]\nmembers = [\"packages/*\"]\n";
        std::fs::write(dir.path().join("pyproject.toml"), pyproject).unwrap();

        let ws = parse_uv_workspace(dir.path()).unwrap().unwrap();
        assert_eq!(ws.members.len(), 1);
        assert!(ws.members[0].ends_with("real_pkg"));
    }

    #[test]
    fn returns_none_with_tool_uv_but_no_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let pyproject = "[tool.uv]\ndev-dependencies = []\n";
        std::fs::write(dir.path().join("pyproject.toml"), pyproject).unwrap();

        let result = parse_uv_workspace(dir.path()).unwrap();
        assert!(result.is_none());
    }
}
