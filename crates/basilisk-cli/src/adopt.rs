//! Implements [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
//! `basilisk adopt` and `basilisk unadopt` subcommands.
//!
//! `adopt` records remaining errors as exact-file warning overrides in the
//! active project configuration; `unadopt` removes those editor-owned entries.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use basilisk_config::{RuleConfigScope, RuleConfigUpdate, RuleSeverity};

use tracing::{error, info, warn};

use crate::pluralise;

/// Run the adopt subcommand.
///
/// Exit codes:
/// - `0` — adoption recorded successfully
/// - `3` — internal error
pub(crate) fn run_adopt(paths: &[String]) -> u8 {
    match adopt_files(paths) {
        Ok(summary) => {
            println!(
                "Adopted {} file{} with {} demoted error{}.",
                summary.files_adopted,
                pluralise(summary.files_adopted),
                summary.demoted_count,
                pluralise(summary.demoted_count),
            );
            0
        }
        Err(err) => {
            error!(%err, "adopt failed");
            3
        }
    }
}

/// Run the unadopt subcommand.
///
/// Exit codes:
/// - `0` — un-adoption recorded successfully
/// - `3` — internal error
pub(crate) fn run_unadopt(paths: &[String]) -> u8 {
    match unadopt_files(paths) {
        Ok(count) => {
            println!("Un-adopted {} file{}.", count, pluralise(count));
            0
        }
        Err(err) => {
            error!(%err, "unadopt failed");
            3
        }
    }
}

/// Run the adopt --status subcommand.
///
/// Exit codes:
/// - `0` — always
/// - `3` — internal error
pub(crate) fn run_adopt_status(paths: &[String]) -> u8 {
    let config_root = resolve_config_root(paths);
    let document = match basilisk_config::discover_config_document(&config_root) {
        Ok(document) => document,
        Err(err) => {
            error!(%err, "failed to load active configuration");
            return 3;
        }
    };
    let adoption = basilisk_config::adoption_rule_overrides(&document);
    if adoption.is_empty() {
        println!("No files are currently adopted.");
        return 0;
    }
    println!(
        "{} adopted file{}:",
        adoption.len(),
        pluralise(adoption.len()),
    );
    for (file, rules) in adoption {
        let count = rules.len();
        println!("  {} ({} demoted code{})", file, count, pluralise(count));
    }

    0
}

/// Summary of an adopt run.
struct AdoptSummary {
    /// Number of files that were adopted.
    files_adopted: usize,
    /// Total number of error codes demoted across all files.
    demoted_count: usize,
}

/// Adopt files: check each file and persist exact-path warning overrides.
fn adopt_files(paths: &[String]) -> Result<AdoptSummary, String> {
    let config_root = resolve_config_root(paths);
    let document = basilisk_config::discover_config_document(&config_root)
        .map_err(|error| error.to_string())?;
    let config = document.config.clone();
    let excluded = crate::excluded_dirs_and_log(&config, &config_root);
    let python_files = crate::collect_python_files(paths, &excluded)?;
    let mut files_adopted: usize = 0;
    let mut demoted_count: usize = 0;
    let mut updates = Vec::new();

    for path_str in python_files {
        match check_file_errors(&path_str, &config) {
            Ok(error_codes) => {
                if error_codes.is_empty() {
                    continue;
                }
                let path = Path::new(&path_str);
                let count = error_codes.len();
                updates.push(RuleConfigUpdate {
                    scope: RuleConfigScope::Path {
                        pattern: relative_pattern(&config_root, path)?,
                        adoption: true,
                    },
                    rules: error_codes
                        .into_iter()
                        .map(|code| (code, Some(RuleSeverity::Warning)))
                        .collect(),
                });
                files_adopted += 1;
                demoted_count += count;
                info!(path = path_str, count, "adopted file");
            }
            Err(err) => {
                warn!(path = path_str, %err, "error checking file");
            }
        }
    }
    if !updates.is_empty() {
        let patch = basilisk_config::build_rule_patch(&document, &updates)
            .map_err(|error| error.to_string())?;
        basilisk_config::apply_config_patch(&patch).map_err(|error| error.to_string())?;
    }

    Ok(AdoptSummary {
        files_adopted,
        demoted_count,
    })
}

/// Un-adopt files: remove adoption overrides for each path.
fn unadopt_files(paths: &[String]) -> Result<usize, String> {
    let config_root = resolve_config_root(paths);
    let document = basilisk_config::discover_config_document(&config_root)
        .map_err(|error| error.to_string())?;
    let config = document.config.clone();
    let excluded: HashSet<&str> = config.exclude.iter().map(String::as_str).collect();
    let python_files = crate::collect_python_files(paths, &excluded)?;
    let adoption = basilisk_config::adoption_rule_overrides(&document);
    let mut count: usize = 0;
    let mut updates = Vec::new();
    for path_str in python_files {
        let path = Path::new(&path_str);
        let pattern = relative_pattern(&config_root, path)?;
        if let Some(rules) = adoption.get(&pattern) {
            updates.push(RuleConfigUpdate {
                scope: RuleConfigScope::Path {
                    pattern,
                    adoption: false,
                },
                rules: rules.keys().map(|code| (code.clone(), None)).collect(),
            });
            count += 1;
            info!(path = path_str, "un-adopted file");
        }
    }
    if !updates.is_empty() {
        let patch = basilisk_config::build_rule_patch(&document, &updates)
            .map_err(|error| error.to_string())?;
        basilisk_config::apply_config_patch(&patch).map_err(|error| error.to_string())?;
    }

    Ok(count)
}

/// Check a single file under the project configuration and return all error
/// codes found. Honors config so adoption records exactly the diagnostics the
/// project has enabled (house rules are off by default). See
/// [CHKARCH-CONFIGURATION-ONLY].
fn check_file_errors(
    path: &str,
    config: &basilisk_config::BasiliskConfig,
) -> Result<Vec<String>, String> {
    let parsed = basilisk_parser::parse_file(path).map_err(|e| e.to_string())?;
    let resolved = basilisk_resolver::resolve(&parsed).map_err(|e| e.to_string())?;
    let diags = basilisk_checker::check_with_config(&resolved, config);

    let codes: BTreeSet<String> = diags
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.severity,
                basilisk_checker::Severity::Error | basilisk_checker::Severity::SafetyViolation
            )
        })
        .map(|d| d.code.code.to_owned())
        .collect();

    Ok(codes.into_iter().collect())
}

fn relative_pattern(root: &Path, path: &Path) -> Result<String, String> {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    canonical_path
        .strip_prefix(&canonical_root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|error| {
            format!(
                "{} is outside the selected project root {}: {error}",
                canonical_path.display(),
                canonical_root.display()
            )
        })
}

/// Resolve the config root directory from the provided paths.
fn resolve_config_root(paths: &[String]) -> std::path::PathBuf {
    let candidate = paths
        .first()
        .map(Path::new)
        .and_then(|p| {
            if p.is_dir() {
                Some(p.to_path_buf())
            } else {
                p.parent().map(Path::to_path_buf)
            }
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    crate::find_project_root(&candidate)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// Python code with a missing parameter annotation (triggers BSK-E0001)
    /// and a missing return type annotation (triggers BSK-E0002).
    const BAD_PYTHON: &str = "def foo(x):\n    pass\n";

    /// Fully typed Python code that should produce zero errors.
    const CLEAN_PYTHON: &str = "def greet(name: str) -> str:\n    return name\n";

    /// Create a fresh temporary project directory (removing any leftover from a
    /// prior run) that ships a `basilisk.json` opting into the annotation house
    /// rules. `adopt` records the diagnostics a project has enabled, and those
    /// rules (`BSK-E0001`/`BSK-E0002`/…) are off by default — so the test project
    /// turns them on exactly as a real adopter would. No modes; this is
    /// configuration. See [CHKARCH-CONFIGURATION-ONLY].
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bsk_adopt_test_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("basilisk.json"),
            "{\"rules\":{\"BSK-E0001\":\"error\",\"BSK-E0002\":\"error\"}}\n",
        )
        .unwrap();
        dir
    }

    /// Config that opts into the annotation house rules — the in-memory mirror of
    /// the `basilisk.json` [`temp_dir`] writes, for tests that call
    /// `check_file_errors` directly. See [CHKARCH-CONFIGURATION-ONLY].
    fn annotations_on() -> basilisk_config::BasiliskConfig {
        basilisk_config::BasiliskConfig {
            rules: ["BSK-E0001", "BSK-E0002"]
                .into_iter()
                .map(|code| (code.to_owned(), basilisk_config::RuleSeverity::Error))
                .collect(),
            ..Default::default()
        }
    }

    /// Write a `.py` file inside `dir` and return its absolute path as a `String`.
    fn write_py(dir: &Path, filename: &str, content: &str) -> String {
        let path = dir.join(filename);
        fs::write(&path, content).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn adoption(
        dir: &Path,
    ) -> std::collections::BTreeMap<String, std::collections::BTreeMap<String, RuleSeverity>> {
        let document = basilisk_config::discover_config_document(dir).unwrap();
        basilisk_config::adoption_rule_overrides(&document)
    }

    // ── run_adopt ────────────────────────────────────────────────────────────

    #[test]
    fn run_adopt_bad_code_produces_adoptions() {
        let dir = temp_dir("adopt_bad");
        let path = write_py(&dir, "bad.py", BAD_PYTHON);

        let exit = run_adopt(&[path]);
        assert_eq!(exit, 0, "adopt should succeed with exit code 0");

        let adopted = adoption(&dir);
        assert_eq!(adopted.len(), 1);
        assert!(
            adopted
                .get("bad.py")
                .is_some_and(|rules| rules.contains_key("BSK-E0001")),
            "bad.py should have BSK-E0001 demoted"
        );
    }

    #[test]
    fn run_adopt_clean_code_produces_no_adoptions() {
        let dir = temp_dir("adopt_clean");
        let path = write_py(&dir, "clean.py", CLEAN_PYTHON);

        let exit = run_adopt(&[path]);
        assert_eq!(exit, 0);

        assert!(
            adoption(&dir).is_empty(),
            "clean code should produce no adoptions"
        );
    }

    #[test]
    fn run_adopt_nonexistent_path_returns_3() {
        let exit = run_adopt(&["/no/such/path/ever.py".to_owned()]);
        assert_eq!(exit, 3, "nonexistent path should return exit code 3");
    }

    #[test]
    fn run_adopt_directory_traversal_adopts_multiple_files() {
        let dir = temp_dir("adopt_multi");
        let _ = write_py(&dir, "a.py", BAD_PYTHON);
        let _ = write_py(&dir, "b.py", BAD_PYTHON);

        let exit = run_adopt(&[dir.to_string_lossy().into_owned()]);
        assert_eq!(exit, 0);

        let adopted = adoption(&dir);
        assert_eq!(
            adopted.len(),
            2,
            "both bad files should be adopted, got: {adopted:?}"
        );
    }

    #[test]
    fn run_adopt_rejects_files_from_different_project_roots() {
        let first = temp_dir("adopt_cross_root_first");
        let second = temp_dir("adopt_cross_root_second");
        let first_path = write_py(&first, "first.py", BAD_PYTHON);
        let second_path = write_py(&second, "second.py", BAD_PYTHON);

        assert_eq!(run_adopt(&[first_path, second_path]), 3);
        assert!(adoption(&first).is_empty());
        assert!(adoption(&second).is_empty());
    }

    // ── run_unadopt ──────────────────────────────────────────────────────────

    #[test]
    fn run_unadopt_removes_adoptions() {
        let dir = temp_dir("unadopt_remove");
        let path = write_py(&dir, "bad.py", BAD_PYTHON);

        // First adopt.
        let exit = run_adopt(std::slice::from_ref(&path));
        assert_eq!(exit, 0);
        assert!(
            !adoption(&dir).is_empty(),
            "precondition: adoption must exist"
        );

        // Then unadopt.
        let exit = run_unadopt(&[path]);
        assert_eq!(exit, 0);

        assert!(
            adoption(&dir).is_empty(),
            "active config must have no adoption entries after unadopt"
        );
    }

    #[test]
    fn run_unadopt_on_clean_dir_returns_0() {
        let dir = temp_dir("unadopt_clean");
        let _ = write_py(&dir, "clean.py", CLEAN_PYTHON);

        let exit = run_unadopt(&[dir.to_string_lossy().into_owned()]);
        assert_eq!(exit, 0);
    }

    #[test]
    fn run_unadopt_nonexistent_path_returns_3() {
        let exit = run_unadopt(&["/no/such/path/ever.py".to_owned()]);
        assert_eq!(exit, 3);
    }

    // ── run_adopt_status ─────────────────────────────────────────────────────

    #[test]
    fn run_adopt_status_empty_prints_no_files() {
        let dir = temp_dir("status_empty");
        // Create the directory but no adoptions.
        let exit = run_adopt_status(&[dir.to_string_lossy().into_owned()]);
        assert_eq!(exit, 0);
    }

    #[test]
    fn run_adopt_status_shows_adopted_files() {
        let dir = temp_dir("status_shows");
        let path = write_py(&dir, "bad.py", BAD_PYTHON);

        let exit = run_adopt(&[path]);
        assert_eq!(exit, 0);

        let exit = run_adopt_status(&[dir.to_string_lossy().into_owned()]);
        assert_eq!(exit, 0);
    }

    // ── check_file_errors ────────────────────────────────────────────────────

    #[test]
    fn check_file_errors_bad_code_returns_error_codes() {
        let dir = temp_dir("check_bad");
        let path = write_py(&dir, "bad.py", BAD_PYTHON);

        let codes = check_file_errors(&path, &annotations_on()).unwrap();
        assert!(
            !codes.is_empty(),
            "bad code must produce at least one error"
        );
        assert!(
            codes.contains(&"BSK-E0001".to_owned()),
            "expected BSK-E0001 in {codes:?}"
        );
    }

    #[test]
    fn check_file_errors_clean_code_returns_empty() {
        let dir = temp_dir("check_clean");
        let path = write_py(&dir, "clean.py", CLEAN_PYTHON);

        let codes = check_file_errors(&path, &annotations_on()).unwrap();
        assert!(
            codes.is_empty(),
            "clean code should produce no errors, got: {codes:?}"
        );
    }

    #[test]
    fn check_file_errors_nonexistent_file_returns_err() {
        let result = check_file_errors("/no/such/file.py", &annotations_on());
        assert!(result.is_err(), "nonexistent file must return Err");
    }

    // ── resolve_config_root ──────────────────────────────────────────────────

    #[test]
    fn resolve_config_root_file_returns_parent() {
        let dir = temp_dir("resolve_file");
        let path = write_py(&dir, "foo.py", CLEAN_PYTHON);

        let root = resolve_config_root(&[path]);
        assert_eq!(root, dir.canonicalize().unwrap());
    }

    #[test]
    fn resolve_config_root_directory_returns_itself() {
        let dir = temp_dir("resolve_dir");
        let root = resolve_config_root(&[dir.to_string_lossy().into_owned()]);
        assert_eq!(root, dir.canonicalize().unwrap());
    }

    #[test]
    fn resolve_config_root_nested_file_finds_project_config() {
        let dir = temp_dir("resolve_nested");
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        let path = write_py(&src, "nested.py", CLEAN_PYTHON);
        assert_eq!(resolve_config_root(&[path]), dir.canonicalize().unwrap());
    }

    #[test]
    fn resolve_config_root_empty_returns_cwd() {
        let root = resolve_config_root(&[]);
        assert_eq!(root, crate::find_project_root(Path::new(".")));
    }

    // ── pluralise ────────────────────────────────────────────────────────────

    #[test]
    fn pluralise_zero_returns_s() {
        assert_eq!(pluralise(0), "s");
    }

    #[test]
    fn pluralise_one_returns_empty() {
        assert_eq!(pluralise(1), "");
    }

    #[test]
    fn pluralise_many_returns_s() {
        assert_eq!(pluralise(5), "s");
    }
}
