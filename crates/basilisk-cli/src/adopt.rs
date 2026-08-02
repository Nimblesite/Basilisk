//! Implements [AUTOFIX-ADOPTION]. See docs/specs/LSP-MASS-AUTOFIX-SPEC.md#AUTOFIX-ADOPTION
//! `basilisk adopt`, `basilisk unadopt`, and `basilisk adopt --status`.
//!
//! Adoption records current error debt as **ordinary warning-severity rule
//! entries** in the config file of the nearest folder governing each affected
//! file — plain `code -> severity` entries in the one configuration model
//! ([CHKARCH-CONFIG-MODEL]). There are no exact-file overrides, ownership
//! markers, or sidecar state: the adoption state IS the set of
//! warning-severity `[tool.basilisk.rules]` entries, `unadopt` deletes them,
//! and re-running `adopt` recomputes them so rules that no longer fire revert
//! without manual bookkeeping ([AUTOFIX-ADOPTION-FLOW]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use basilisk_config::{RuleConfigUpdate, RuleSeverity};
use tracing::{error, info};

use crate::pipeline::{
    collect_and_check, find_project_root, first_path_dir, parent_dir_of, pluralise,
    DiagnosticScope, PipelineError,
};

/// Run the adopt subcommand.
///
/// Exit codes ([CHKARCH-CLI-EXITCODES]):
/// - `0` — adoption recorded successfully
/// - `2` — invalid configuration
/// - `3` — internal error
pub(crate) fn run_adopt(paths: &[String]) -> u8 {
    match adopt_folders(paths) {
        Ok(summary) => {
            println!(
                "Adopted {} folder config{} with {} demoted rule code{}.",
                summary.folders_updated,
                pluralise(summary.folders_updated),
                summary.demoted_count,
                pluralise(summary.demoted_count),
            );
            0
        }
        Err(err) => report_failure(&err, "adopt failed"),
    }
}

/// Run the unadopt subcommand.
///
/// Exit codes: `0` on success, `2` on invalid configuration, `3` on
/// internal error.
pub(crate) fn run_unadopt(paths: &[String]) -> u8 {
    match unadopt_folders(paths) {
        Ok(removed) => {
            println!(
                "Un-adopted {} rule entr{}.",
                removed,
                if removed == 1 { "y" } else { "ies" },
            );
            0
        }
        Err(err) => report_failure(&err, "unadopt failed"),
    }
}

/// Run the adopt --status subcommand.
///
/// Reports, per governing folder config, the warning-severity rule entries
/// that constitute the adoption state ([AUTOFIX-ADOPTION]).
///
/// Exit codes: `0` on success, `3` on internal error.
pub(crate) fn run_adopt_status(paths: &[String]) -> u8 {
    let roots = match governing_roots(paths) {
        Ok(roots) => roots,
        Err(err) => return report_failure(&err, "adopt --status failed"),
    };
    let mut adopted_any = false;
    for root in roots {
        let entries = match adopted_entries(&root) {
            Ok(entries) => entries,
            Err(err) => return report_failure(&err, "adopt --status failed"),
        };
        if entries.is_empty() {
            continue;
        }
        adopted_any = true;
        println!(
            "{} ({} demoted code{}):",
            root.display(),
            entries.len(),
            pluralise(entries.len()),
        );
        for code in entries {
            println!("  {code}");
        }
    }
    if !adopted_any {
        println!("No folders are currently adopted.");
    }
    0
}

/// Log a pipeline failure and map it to its exit code.
fn report_failure(err: &PipelineError, context: &'static str) -> u8 {
    match err {
        PipelineError::Config(message) => {
            error!(%message, "{context}: configuration error");
            2
        }
        PipelineError::NoSource(message) => {
            error!(%message, "{context}");
            3
        }
        PipelineError::Internal(message) => {
            error!(%message, "{context}");
            3
        }
    }
}

/// Summary of an adopt run.
struct AdoptSummary {
    /// Number of folder configs that were rewritten.
    folders_updated: usize,
    /// Total number of rule codes demoted across all folders.
    demoted_count: usize,
}

/// Current debt for one governing folder config.
#[derive(Default)]
struct FolderDebt {
    /// Codes firing at `error`/`safety-violation` — the debt to demote.
    error_codes: BTreeSet<String>,
    /// Codes firing at any severity — existing adoption entries for codes
    /// absent here have graduated and are removed on recompute.
    firing_codes: BTreeSet<String>,
}

/// Adopt: check both command scopes at their resolved severities
/// ([CHKARCH-COMMANDS]) and rewrite each governing folder config's adoption
/// entries to exactly the current debt ([AUTOFIX-ADOPTION-FLOW]).
fn adopt_folders(paths: &[String]) -> Result<AdoptSummary, PipelineError> {
    let debt_by_root = collect_folder_debt(paths)?;
    let mut folders_updated: usize = 0;
    let mut demoted_count: usize = 0;

    for (root, debt) in debt_by_root {
        let existing = adopted_entries(&root)?;
        let mut rules: BTreeMap<String, Option<RuleSeverity>> = debt
            .error_codes
            .iter()
            .map(|code| (code.clone(), Some(RuleSeverity::Warning)))
            .collect();
        // Recompute: an adoption entry whose rule no longer fires anywhere in
        // the scanned scope has graduated — delete it ([AUTOFIX-ADOPTION-FLOW]).
        for code in existing {
            if !debt.firing_codes.contains(&code) {
                let _ = rules.entry(code).or_insert(None);
            }
        }
        if rules.is_empty() {
            continue;
        }
        write_rule_entries(&root, rules.clone())?;
        folders_updated += 1;
        demoted_count += debt.error_codes.len();
        info!(
            root = %root.display(),
            demoted = debt.error_codes.len(),
            "adopted folder config"
        );
    }

    Ok(AdoptSummary {
        folders_updated,
        demoted_count,
    })
}

/// Unadopt: delete every warning-severity rule entry — the adoption state —
/// from each governing folder config ([AUTOFIX-ADOPTION]).
fn unadopt_folders(paths: &[String]) -> Result<usize, PipelineError> {
    let mut removed: usize = 0;
    for root in governing_roots(paths)? {
        let entries = adopted_entries(&root)?;
        if entries.is_empty() {
            continue;
        }
        removed += entries.len();
        let rules: BTreeMap<String, Option<RuleSeverity>> =
            entries.into_iter().map(|code| (code, None)).collect();
        write_rule_entries(&root, rules)?;
        info!(root = %root.display(), "un-adopted folder config");
    }
    Ok(removed)
}

/// Run the shared pipeline over both scopes and group the result per
/// governing folder config. Every scanned file registers its root even when
/// clean, so recompute can graduate stale entries.
fn collect_folder_debt(paths: &[String]) -> Result<BTreeMap<PathBuf, FolderDebt>, PipelineError> {
    // Adoption rewrites the very configuration a cache entry is fingerprinted
    // against, so it always runs cold — the project's `cache` key does not
    // apply here ([CHKCACHE-CONFIG]).
    let no_cache = crate::cache_check::CacheOptions {
        enabled: crate::cache_check::CacheOverride::ForceOff,
        dir: None,
        stats: false,
    };
    let mut stats = crate::cache_check::CacheStats::default();
    let outcome = collect_and_check(paths, &no_cache, &mut stats, DiagnosticScope::Union)?;
    for failure in &outcome.failures {
        tracing::warn!(path = %failure.path, error = %failure.message, "error checking file");
    }

    let mut debt: BTreeMap<PathBuf, FolderDebt> = BTreeMap::new();
    for source in &outcome.sources {
        let _ = debt.entry(governing_root(&source.path)).or_default();
    }
    for diagnostic in &outcome.diagnostics {
        let entry = debt.entry(governing_root(&diagnostic.path)).or_default();
        let code = diagnostic.code.code.to_owned();
        if matches!(
            diagnostic.severity,
            basilisk_checker::Severity::Error | basilisk_checker::Severity::SafetyViolation
        ) {
            let _ = entry.error_codes.insert(code.clone());
        }
        let _ = entry.firing_codes.insert(code);
    }
    Ok(debt)
}

/// The unique governing folder configs for the Python files under `paths`.
fn governing_roots(paths: &[String]) -> Result<BTreeSet<PathBuf>, PipelineError> {
    let config_root = first_path_dir(paths);
    let config = basilisk_config::load_basilisk_config(&config_root);
    let excluded = crate::pipeline::excluded_dirs_and_log(&config, &config_root);
    let python_files =
        crate::pipeline::collect_python_files(paths, &excluded).map_err(PipelineError::Internal)?;
    Ok(python_files
        .iter()
        .map(|file| governing_root(file))
        .collect())
}

/// The folder whose config file governs `file`: the nearest ancestor holding
/// a `[tool.basilisk]` table, else the project root (whose `pyproject.toml`
/// becomes the creation target). [CHKARCH-CONFIG-DISCOVERY]
fn governing_root(file: &str) -> PathBuf {
    let parent = parent_dir_of(file);
    basilisk_config::discover_config_dir(&parent).unwrap_or_else(|| find_project_root(&parent))
}

/// The adoption state of one folder config: its warning-severity
/// `[tool.basilisk.rules]` entries ([AUTOFIX-ADOPTION]).
fn adopted_entries(root: &Path) -> Result<BTreeSet<String>, PipelineError> {
    let document = discover_document(root)?;
    Ok(document
        .config
        .nearest_tables()
        .map(|tables| {
            tables
                .rules
                .iter()
                .filter(|(_, severity)| **severity == RuleSeverity::Warning)
                .map(|(code, _)| code.clone())
                .collect()
        })
        .unwrap_or_default())
}

/// Apply plain rule-entry updates to the folder config at `root` through the
/// shared configuration mutation service ([AUTOFIX-ADOPTION-FLOW]).
fn write_rule_entries(
    root: &Path,
    rules: BTreeMap<String, Option<RuleSeverity>>,
) -> Result<(), PipelineError> {
    let document = discover_document(root)?;
    let update = RuleConfigUpdate {
        rules,
        rule_tags: BTreeMap::new(),
    };
    let patch = basilisk_config::build_rule_patch(&document, &update)
        .map_err(|err| PipelineError::Config(err.to_string()))?;
    basilisk_config::apply_config_patch(&patch)
        .map_err(|err| PipelineError::Internal(err.to_string()))
}

fn discover_document(root: &Path) -> Result<basilisk_config::ConfigDocument, PipelineError> {
    basilisk_config::discover_config_document(root)
        .map_err(|err| PipelineError::Config(err.to_string()))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;
    use std::fs;

    /// Python code with a missing parameter annotation (triggers BSK-0001)
    /// and a missing return type annotation (triggers BSK-0002).
    // `x` has no default to infer from (BSK-0001) and `return x` is not
    // inferable (BSK-0002) — a `pass` body would infer `-> None` and only
    // fire BSK-0001 ([TYPEINF-FUNC-RETURN]).
    const BAD_PYTHON: &str = "def foo(x):\n    return x\n";

    /// Fully typed Python code that should produce zero errors.
    const CLEAN_PYTHON: &str = "def greet(name: str) -> str:\n    return name\n";

    /// Python code with a check-scope (pep) error: wrong return type.
    const PEP_ERROR_PYTHON: &str = "def bad() -> int:\n    return \"x\"\n";

    /// Create a fresh temporary project directory (removing any leftover from
    /// a prior run) that ships a `pyproject.toml` opting into the annotation
    /// house rules. `adopt` records the diagnostics a project has enabled,
    /// and those analyze-scope rules are off by default — so the test project
    /// turns them on exactly as a real adopter would ([CHKARCH-COMMANDS]).
    fn temp_dir(name: &str) -> PathBuf {
        // Per-process dir name (same pattern as `stage_project` in
        // cli_binary_tests): a stray watcher or leftover harness process from
        // a previous run must never touch this run's fixture files.
        let dir =
            std::env::temp_dir().join(format!("bsk_adopt_test_{name}.{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n\"BSK-0002\" = \"error\"\n",
        )
        .unwrap();
        dir
    }

    /// Write a `.py` file inside `dir` and return its absolute path as a `String`.
    fn write_py(dir: &Path, filename: &str, content: &str) -> String {
        let path = dir.join(filename);
        fs::write(&path, content).unwrap();
        path.to_string_lossy().into_owned()
    }

    /// The warning-severity rule entries in `dir`'s config — the adoption
    /// state ([AUTOFIX-ADOPTION]).
    fn adoption(dir: &Path) -> BTreeSet<String> {
        adopted_entries(dir).unwrap()
    }

    /// The full `[tool.basilisk.rules]` table in `dir`'s config.
    fn rule_entries(dir: &Path) -> BTreeMap<String, RuleSeverity> {
        let document = basilisk_config::discover_config_document(dir).unwrap();
        document
            .config
            .nearest_tables()
            .map(|tables| tables.rules.clone().into_iter().collect())
            .unwrap_or_default()
    }

    // ── run_adopt ([AUTOFIX-ADOPTION]) ───────────────────────────────────

    /// [AUTOFIX-ADOPTION]: adopting a folder with analyze-scope error debt
    /// demotes the firing codes to plain warning entries in the governing
    /// folder config — no exact-file overrides, no markers.
    #[test]
    fn run_adopt_bad_code_demotes_codes_in_folder_config() {
        let dir = temp_dir("adopt_bad");
        let path = write_py(&dir, "bad.py", BAD_PYTHON);

        let exit = run_adopt(&[path]);
        assert_eq!(exit, 0, "adopt should succeed with exit code 0");

        let entries = rule_entries(&dir);
        assert_eq!(
            entries.get("BSK-0001"),
            Some(&RuleSeverity::Warning),
            "BSK-0001 must be demoted to a folder-level warning entry, got: {entries:?}"
        );
        assert_eq!(
            entries.get("BSK-0002"),
            Some(&RuleSeverity::Warning),
            "BSK-0002 must be demoted to a folder-level warning entry, got: {entries:?}"
        );
    }

    /// [AUTOFIX-ADOPTION-FLOW]: pep debt is demoted to `warning` (never below
    /// info) as an ordinary folder entry, so `check` reports it as a warning
    /// afterwards.
    #[test]
    fn run_adopt_records_pep_debt_as_warning_entry() {
        // Per-process dir name — see `temp_dir` for the rationale.
        let dir =
            std::env::temp_dir().join(format!("bsk_adopt_test_pep_debt.{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("pyproject.toml"),
            "[project]\nname = \"x\"\nversion = \"0.0.0\"\n",
        )
        .unwrap();
        let path = write_py(&dir, "bad.py", PEP_ERROR_PYTHON);

        let exit = run_adopt(&[path]);
        assert_eq!(exit, 0, "adopt should succeed");

        let entries = rule_entries(&dir);
        let demoted_pep: Vec<_> = entries
            .iter()
            .filter(|(code, severity)| {
                basilisk_checker::is_pep_rule(code) && **severity == RuleSeverity::Warning
            })
            .collect();
        assert!(
            !demoted_pep.is_empty(),
            "the firing pep code must be demoted to warning in the folder config, got: {entries:?}"
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
            "clean code should produce no adoption entries"
        );
    }

    #[test]
    fn run_adopt_nonexistent_path_returns_3() {
        let exit = run_adopt(&["/no/such/path/ever.py".to_owned()]);
        assert_eq!(exit, 3, "nonexistent path should return exit code 3");
    }

    /// [AUTOFIX-ADOPTION-RULES]: a folder entry is a plain override — two bad
    /// files in one folder produce one set of folder entries, not per-file
    /// state.
    #[test]
    fn run_adopt_directory_traversal_writes_one_folder_entry_set() {
        let dir = temp_dir("adopt_multi");
        let _ = write_py(&dir, "a.py", BAD_PYTHON);
        let _ = write_py(&dir, "b.py", BAD_PYTHON);

        let exit = run_adopt(&[dir.to_string_lossy().into_owned()]);
        assert_eq!(exit, 0);

        let adopted = adoption(&dir);
        assert_eq!(
            adopted,
            ["BSK-0001", "BSK-0002"]
                .into_iter()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>(),
            "both files' debt collapses into the one governing folder config"
        );
    }

    /// [AUTOFIX-ADOPTION]: debt in differently-governed folders is demoted in
    /// each folder's own config file (the old single-store restriction is
    /// gone).
    #[test]
    fn run_adopt_writes_each_governing_folder_config() {
        let first = temp_dir("adopt_cross_root_first");
        let second = temp_dir("adopt_cross_root_second");
        let first_path = write_py(&first, "first.py", BAD_PYTHON);
        let second_path = write_py(&second, "second.py", BAD_PYTHON);

        assert_eq!(run_adopt(&[first_path, second_path]), 0);
        assert!(
            adoption(&first).contains("BSK-0001"),
            "first root must hold its own adoption entries"
        );
        assert!(
            adoption(&second).contains("BSK-0001"),
            "second root must hold its own adoption entries"
        );
    }

    /// [AUTOFIX-ADOPTION-FLOW]: re-running adopt recomputes — entries for
    /// rules that no longer fire anywhere in the folder are deleted.
    #[test]
    fn run_adopt_rerun_graduates_fixed_rules() {
        let dir = temp_dir("adopt_rerun");
        let path = write_py(&dir, "bad.py", BAD_PYTHON);

        assert_eq!(run_adopt(std::slice::from_ref(&path)), 0);
        assert!(
            !adoption(&dir).is_empty(),
            "precondition: adoption entries exist"
        );

        // Fix the debt, re-run adopt: the entries must graduate away.
        let _ = write_py(&dir, "bad.py", CLEAN_PYTHON);
        assert_eq!(run_adopt(&[path]), 0);
        assert!(
            adoption(&dir).is_empty(),
            "re-running adopt must remove entries whose rules no longer fire, got: {:?}",
            adoption(&dir)
        );
    }

    // ── run_unadopt ([AUTOFIX-ADOPTION]) ─────────────────────────────────

    /// [AUTOFIX-ADOPTION-FLOW]: unadopt deletes the folder's warning entries,
    /// restoring the ancestor severity.
    #[test]
    fn run_unadopt_removes_adoption_entries() {
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

    /// Unadopt leaves non-warning entries (the user's own error opt-ins)
    /// untouched — only the adoption state is deleted. [AUTOFIX-ADOPTION]
    #[test]
    fn run_unadopt_preserves_error_entries() {
        let dir = temp_dir("unadopt_preserve");
        let path = write_py(&dir, "bad.py", BAD_PYTHON);
        assert_eq!(run_adopt(std::slice::from_ref(&path)), 0);
        assert_eq!(run_unadopt(&[path]), 0);

        // BSK-0001/BSK-0002 were rewritten to warning by adopt and removed by
        // unadopt; a config with only non-warning entries would keep them.
        let entries = rule_entries(&dir);
        assert!(
            entries
                .values()
                .all(|severity| *severity != RuleSeverity::Warning),
            "no warning entries may remain after unadopt, got: {entries:?}"
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

    // ── run_adopt_status ([AUTOFIX-ADOPTION]) ────────────────────────────

    #[test]
    fn run_adopt_status_empty_prints_no_folders() {
        let dir = temp_dir("status_empty");
        // Create the directory but no adoptions.
        let _ = write_py(&dir, "clean.py", CLEAN_PYTHON);
        let exit = run_adopt_status(&[dir.to_string_lossy().into_owned()]);
        assert_eq!(exit, 0);
    }

    #[test]
    fn run_adopt_status_shows_adopted_folders() {
        let dir = temp_dir("status_shows");
        let path = write_py(&dir, "bad.py", BAD_PYTHON);

        let exit = run_adopt(&[path]);
        assert_eq!(exit, 0);

        let exit = run_adopt_status(&[dir.to_string_lossy().into_owned()]);
        assert_eq!(exit, 0);
    }

    // ── governing_root ([CHKARCH-CONFIG-DISCOVERY]) ──────────────────────

    #[test]
    fn governing_root_file_returns_config_dir() {
        let dir = temp_dir("resolve_file");
        let path = write_py(&dir, "foo.py", CLEAN_PYTHON);

        // Discovery preserves the caller's path spelling (no
        // canonicalization) — a symlinked temp dir stays as given.
        assert_eq!(governing_root(&path), dir);
    }

    #[test]
    fn governing_root_nested_file_finds_project_config() {
        let dir = temp_dir("resolve_nested");
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        let path = write_py(&src, "nested.py", CLEAN_PYTHON);
        assert_eq!(governing_root(&path), dir);
    }

    /// A nested folder with its own `[tool.basilisk]` table governs its files
    /// — adoption writes there, exactly where `check` discovers.
    /// [CHKARCH-CONFIG-DISCOVERY]
    #[test]
    fn governing_root_prefers_nearest_config_table() {
        let dir = temp_dir("resolve_nearest");
        let sub = dir.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(
            sub.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n",
        )
        .unwrap();
        let path = write_py(&sub, "nested.py", CLEAN_PYTHON);
        assert_eq!(governing_root(&path), sub);
    }
}
