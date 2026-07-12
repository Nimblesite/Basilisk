//! Implements [LSPUV]. See docs/specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV
//! uv workspace configuration parsing.
//!
//! Reads `[tool.uv.workspace]` from `pyproject.toml` and resolves glob
//! patterns to actual member directories.

use std::path::{Path, PathBuf};

use crate::error::UvError;

/// Parsed uv workspace configuration.
//
// Implements [LSPUV-WORKSPACE-MODEL] in a slimmed form: holds the resolved
// member directory paths (the spec's `WorkspaceInfo.members`). The spec's
// richer per-member shape (`WorkspaceMember { name, path, pyproject,
// src_roots }`) is not modelled as a struct here — member src-roots are
// derived later in `discover_workspace_members` / `add_source_root`. See
// conformance audit (DEVIATION: simplified model).
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
//
// Implements [LSPUV-WORKSPACE-DETECTION] — reads `[tool.uv.workspace] members`
// (and `exclude`) from `pyproject.toml` and resolves glob patterns to member
// directories. NOTE: the spec table shows `exclude` being honoured in spirit,
// but `resolve_member_patterns` does NOT subtract `exclude` from the resolved
// members (the field is parsed and surfaced, not applied). See conformance
// audit (DEVIATION).
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

    /// Write `src` as the temp dir's `pyproject.toml`, then parse it and unwrap
    /// the workspace — for success-path tests that expect a workspace to exist.
    fn write_and_parse(dir: &tempfile::TempDir, src: &str) -> UvWorkspace {
        std::fs::write(dir.path().join("pyproject.toml"), src).unwrap();
        parse_uv_workspace(dir.path()).unwrap().unwrap()
    }

    // [LSPUV-WORKSPACE-DETECTION]: parsing `[tool.uv.workspace]` members/glob
    // patterns/excludes, and the no-workspace / malformed / literal-member
    // edge cases.
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
        let ws = write_and_parse(&dir, pyproject);
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
        let ws = write_and_parse(&dir, pyproject);
        assert_eq!(ws.members, vec![member]);
    }

    #[test]
    fn skips_nonexistent_literal_member() {
        let dir = tempfile::tempdir().unwrap();

        let pyproject = "[tool.uv.workspace]\nmembers = [\"nonexistent\"]\n";
        let ws = write_and_parse(&dir, pyproject);
        assert!(ws.members.is_empty());
    }

    #[test]
    fn empty_members_and_exclude() {
        let dir = tempfile::tempdir().unwrap();

        let pyproject = "[tool.uv.workspace]\nmembers = []\nexclude = []\n";
        let ws = write_and_parse(&dir, pyproject);
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
        let ws = write_and_parse(&dir, pyproject);
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

/// Discover uv workspace member source directories.
///
/// Implements [LSPUV-WORKSPACE-IMPORT-RESOLUTION]. Parses
/// `[tool.uv.workspace]` from `pyproject.toml` at each workspace root and
/// returns the resolved member directory paths. For each member, looks for a
/// `src/` subdirectory (common Python project layout) and adds it; otherwise
/// adds the member directory itself.
///
/// In monorepo layouts (e.g. `ai_cms/agent-backend/`), `pyproject.toml` may
/// not be at the workspace root itself. If no uv workspace is found at a root,
/// we search one level of subdirectories for `pyproject.toml` with `src/`
/// layouts and add those as source roots.
///
/// Returns an empty vec if no workspace members are found.
#[must_use]
pub fn discover_workspace_members(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut members = Vec::new();

    for root in roots {
        // Try uv workspace at this root first.
        if let Ok(Some(workspace)) = parse_uv_workspace(root) {
            for member_dir in &workspace.members {
                add_source_root(&mut members, member_dir);
            }

            if !workspace.members.is_empty() {
                tracing::info!(
                    root = %root.display(),
                    member_count = workspace.members.len(),
                    "discovered uv workspace members"
                );
                continue;
            }
        }

        // Root itself is a Python project (pyproject.toml at the workspace
        // root, not inside a uv workspace). Add its source root so first-party
        // imports resolve under a `src/` layout.
        if root.join("pyproject.toml").is_file() {
            add_source_root(&mut members, root);
            tracing::info!(
                root = %root.display(),
                "discovered project at workspace root"
            );
        }

        // Also search subdirectories for projects. This handles monorepos
        // where the IDE root (e.g. `ai_cms/`) is a parent of the actual
        // Python project (e.g. `ai_cms/agent-backend/`).
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // `DirEntry::file_type` uses directory-enumeration metadata on
            // normal filesystems, avoiding one `stat` syscall for every child
            // of every workspace root. Preserve the old symlink-following
            // behaviour explicitly; only real directories take the fast path.
            let is_directory = entry.file_type().map_or_else(
                |_| path.is_dir(),
                |file_type| file_type.is_dir() || (file_type.is_symlink() && path.is_dir()),
            );
            if !is_directory || !path.join("pyproject.toml").is_file() {
                continue;
            }

            // Try uv workspace in the subdirectory.
            if let Ok(Some(workspace)) = parse_uv_workspace(&path) {
                for member_dir in &workspace.members {
                    add_source_root(&mut members, member_dir);
                }
                if !workspace.members.is_empty() {
                    tracing::info!(
                        root = %path.display(),
                        member_count = workspace.members.len(),
                        "discovered uv workspace members in subdirectory"
                    );
                    continue;
                }
            }

            // No uv workspace, but has pyproject.toml — treat as a project.
            add_source_root(&mut members, &path);
            tracing::info!(
                path = %path.display(),
                "discovered project subdirectory"
            );
        }
    }

    members
}

/// Add a project directory's source root to the member list.
///
/// Prefers `src/` layout if it exists, otherwise adds the directory itself.
fn add_source_root(members: &mut Vec<PathBuf>, project_dir: &Path) {
    let src_dir = project_dir.join("src");
    if src_dir.is_dir() {
        members.push(src_dir);
    } else {
        members.push(project_dir.to_path_buf());
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod discovery_tests {
    use super::discover_workspace_members;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_CTR: AtomicU64 = AtomicU64::new(0);

    fn unique_tmp(prefix: &str) -> std::path::PathBuf {
        let n = TEST_CTR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("{prefix}_{n}_{}", std::process::id()))
    }

    /// Regression: when the workspace root itself is a Python project with a
    /// `src/` layout (no uv workspace, no subdirectory projects), the `src/`
    /// directory must be added to workspace members so first-party imports
    /// like `from agent_backend.config import settings` resolve.
    ///
    /// Exercises [LSPUV-WORKSPACE-IMPORT-RESOLUTION] — member source-root
    /// discovery (the `src/` layout preference in `add_source_root`).
    #[test]
    fn root_src_layout_project_is_discovered() {
        let root = unique_tmp("bsk_uv_root_src");
        let src = root.join("src");
        let pkg = src.join("mypkg");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(pkg.join("__init__.py"), "").unwrap();
        std::fs::write(pkg.join("config.py"), "settings = 1\n").unwrap();
        std::fs::write(
            root.join("pyproject.toml"),
            "[project]\nname = \"mypkg\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let members = discover_workspace_members(std::slice::from_ref(&root));

        assert!(
            members.iter().any(|m| m == &src),
            "expected src/ to be in workspace_members, got: {members:?}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
