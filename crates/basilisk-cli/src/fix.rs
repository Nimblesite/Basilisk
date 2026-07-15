//! Implements [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
//! `basilisk fix` subcommand — apply autofixes to Python files.
//!
//! For each Python file: parse → resolve → check → generate fixes → apply.
//! Writes the fixed source back to disk.

use basilisk_lsp::code_actions::mass_fix::{ALL_FIXABLE_RULES, SAFE_FIXABLE_RULES};
use tower_lsp::lsp_types::{TextEdit, Url};
use tracing::{info, warn};

use crate::pipeline::pluralise;

/// Run the fix subcommand.
///
/// Exit codes:
/// - `0` — fixes applied successfully
/// - `1` — some files had errors that couldn't be fixed
/// - `3` — internal error
pub(crate) fn run_fix(paths: &[String], include_unsafe: bool, rules: &[String]) -> u8 {
    let allowed_rules = resolve_rules(include_unsafe, rules);
    let allowed_refs: Vec<&str> = allowed_rules.iter().map(String::as_str).collect();
    match collect_and_fix(paths, &allowed_refs) {
        Ok(summary) => {
            println!(
                "Fixed {} diagnostic{} in {} file{}.",
                summary.fixed_count,
                pluralise(summary.fixed_count),
                summary.files_fixed,
                pluralise(summary.files_fixed),
            );
            u8::from(summary.had_unfixable_errors)
        }
        Err(err) => {
            tracing::error!(%err, "internal error");
            3
        }
    }
}

/// Summary of a fix run.
struct FixSummary {
    /// Total number of diagnostics that were fixed.
    fixed_count: usize,
    /// Number of files that had at least one fix applied.
    files_fixed: usize,
    /// Whether any file had errors that could not be auto-fixed.
    had_unfixable_errors: bool,
}

/// Resolve which rules to apply based on CLI flags.
///
/// - Empty `rules` + `include_unsafe` false → safe rules only.
/// - Empty `rules` + `include_unsafe` true → all fixable rules.
/// - Single entry `"all"` (case-insensitive) → all fixable rules.
/// - Otherwise → the provided rules verbatim.
fn resolve_rules(include_unsafe: bool, rules: &[String]) -> Vec<String> {
    if rules.is_empty() {
        if include_unsafe {
            ALL_FIXABLE_RULES.iter().map(|s| (*s).to_owned()).collect()
        } else {
            SAFE_FIXABLE_RULES.iter().map(|s| (*s).to_owned()).collect()
        }
    } else if rules.len() == 1 && rules.first().is_some_and(|r| r.eq_ignore_ascii_case("all")) {
        ALL_FIXABLE_RULES.iter().map(|s| (*s).to_owned()).collect()
    } else {
        rules.to_vec()
    }
}

/// Collect Python files, analyse them, apply fixes, and write back.
fn collect_and_fix(paths: &[String], allowed_rules: &[&str]) -> Result<FixSummary, String> {
    // [CHKARCH-CONFIG-DISCOVERY] Rule config resolves per file, exactly like
    // `basilisk check` (GitHub #311).
    let config_root = crate::pipeline::first_path_dir(paths);
    let config = basilisk_config::load_basilisk_config(&config_root);

    let excluded = crate::pipeline::excluded_dirs_and_log(&config, &config_root);

    let python_files = crate::pipeline::collect_python_files(paths, &excluded)?;
    let dir_configs = crate::pipeline::resolve_dir_configs(&python_files, &config);

    let mut fixed_count: usize = 0;
    let mut files_fixed: usize = 0;
    let mut had_unfixable_errors = false;

    for path in python_files {
        let file_config = crate::pipeline::config_for_path(&dir_configs, &path, &config);
        match fix_single_file(&path, allowed_rules, &file_config) {
            Ok(count) => {
                fixed_count += count;
                if count > 0 {
                    files_fixed += 1;
                }
            }
            Err(err) => {
                warn!(path, %err, "error processing file");
                had_unfixable_errors = true;
            }
        }
    }

    Ok(FixSummary {
        fixed_count,
        files_fixed,
        had_unfixable_errors,
    })
}

/// Analyse a single file and apply fixes matching the allowed rules.
///
/// Returns the number of fixes applied.
fn fix_single_file(
    path: &str,
    allowed_rules: &[&str],
    config: &basilisk_config::BasiliskConfig,
) -> Result<usize, String> {
    let source = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let uri = Url::from_file_path(
        std::path::Path::new(path)
            .canonicalize()
            .map_err(|e| format!("{path}: {e}"))?,
    )
    .map_err(|()| format!("{path}: cannot convert to file URI"))?;

    let parsed = basilisk_parser::parse_source(source.clone(), path.to_owned())
        .map_err(|e| e.to_string())?;
    let resolved = basilisk_resolver::resolve(&parsed).map_err(|e| e.to_string())?;
    let checker_diags = basilisk_checker::check_with_config(&resolved, config);

    let lsp_diags: Vec<_> = checker_diags
        .iter()
        .map(|d| basilisk_lsp::workspace_analysis::bsk_to_lsp(d, &source))
        .collect();

    let Some(action) = basilisk_lsp::code_actions::mass_fix::fix_filtered_in_file(
        &uri,
        &lsp_diags,
        &source,
        allowed_rules,
    ) else {
        return Ok(0);
    };

    let edits = action
        .edit
        .and_then(|ws| ws.changes)
        .and_then(|mut map| map.remove(&uri))
        .unwrap_or_default();

    if edits.is_empty() {
        return Ok(0);
    }

    let edit_count = edits.len();
    let fixed_source = apply_text_edits(&source, &edits);
    std::fs::write(path, fixed_source).map_err(|e| format!("{path}: {e}"))?;

    info!(path, edit_count, "applied fixes");
    Ok(edit_count)
}

/// Apply LSP text edits to source text.
///
/// Edits are sorted by position descending (bottom-to-top) so that earlier
/// offsets remain valid as later text is modified.
fn apply_text_edits(source: &str, edits: &[TextEdit]) -> String {
    let mut indexed: Vec<_> = edits
        .iter()
        .map(|edit| {
            let start = basilisk_lsp::util::position_to_byte_offset(source, edit.range.start);
            let end = basilisk_lsp::util::position_to_byte_offset(source, edit.range.end);
            (start, end, &edit.new_text)
        })
        .collect();

    // Sort descending by start offset so we can apply from the end.
    indexed.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));

    let mut result = source.to_owned();
    for (start, end, new_text) in indexed {
        let clamped_start = start.min(result.len());
        let clamped_end = end.min(result.len());
        result.replace_range(clamped_start..clamped_end, new_text);
    }

    result
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only code: expect acceptable in unit tests"
)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::{Position, Range};

    /// Write `source` to a uniquely-named temp `.py` file inside an isolated
    /// project dir that ships a `pyproject.toml` opting into the annotation
    /// house rules. `fix` targets those rules (`BSK-0001`/`BSK-0002`/
    /// `BSK-0005`/`BSK-0050`), which are OFF by default — a real user enables
    /// them in configuration, so the test project does too. The command loads
    /// that config from disk exactly as it would in production. No modes; this
    /// is configuration. See [CHKARCH-CONFIGURATION-ONLY].
    fn write_temp(name: &str, source: &str) -> (std::path::PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("{name}.proj"));
        std::fs::create_dir_all(&dir).expect("create temp project dir");
        std::fs::write(
            dir.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n\"BSK-0002\" = \"error\"\n\"BSK-0005\" = \"error\"\n\"BSK-0050\" = \"warning\"\n",
        )
        .expect("write pyproject.toml");
        let py = dir.join(name);
        std::fs::write(&py, source).expect("write temp file");
        let path = py.to_string_lossy().into_owned();
        (py, path)
    }

    /// Remove the isolated project dir created by [`write_temp`].
    fn cleanup(py: &std::path::Path) {
        if let Some(dir) = py.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    /// Run fix, read back the file, clean up, and return `(exit_code, content)`.
    fn fix_and_read(
        path_str: &str,
        py: &std::path::Path,
        include_unsafe: bool,
        rules: &[String],
    ) -> (u8, String) {
        let code = run_fix(&[path_str.to_owned()], include_unsafe, rules);
        let content = std::fs::read_to_string(py).expect("read back");
        cleanup(py);
        (code, content)
    }

    #[test]
    fn apply_text_edits_empty_edits() {
        let source = "hello world";
        let result = apply_text_edits(source, &[]);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn apply_text_edits_single_insert() {
        let source = "x = 42\n";
        let edits = vec![TextEdit {
            range: Range::new(Position::new(0, 1), Position::new(0, 1)),
            new_text: ": int".to_owned(),
        }];
        assert_eq!(apply_text_edits(source, &edits), "x: int = 42\n");
    }

    #[test]
    fn apply_text_edits_single_delete() {
        let source = "x: int = 42\n";
        let edits = vec![TextEdit {
            range: Range::new(Position::new(0, 1), Position::new(0, 7)),
            new_text: String::new(),
        }];
        assert_eq!(apply_text_edits(source, &edits), "x= 42\n");
    }

    #[test]
    fn apply_text_edits_multiple_non_overlapping() {
        let source = "x = 1\ny = 2\n";
        let edits = vec![
            TextEdit {
                range: Range::new(Position::new(0, 1), Position::new(0, 1)),
                new_text: ": int".to_owned(),
            },
            TextEdit {
                range: Range::new(Position::new(1, 1), Position::new(1, 1)),
                new_text: ": int".to_owned(),
            },
        ];
        assert_eq!(apply_text_edits(source, &edits), "x: int = 1\ny: int = 2\n");
    }

    #[test]
    fn pluralise_returns_empty_for_one() {
        assert_eq!(pluralise(1), "");
    }

    #[test]
    fn pluralise_returns_s_for_zero() {
        assert_eq!(pluralise(0), "s");
    }

    #[test]
    fn pluralise_returns_s_for_many() {
        assert_eq!(pluralise(5), "s");
    }

    #[test]
    fn run_fix_nonexistent_path_returns_three() {
        assert_eq!(run_fix(&["/no/such/path.py".to_owned()], false, &[]), 3);
    }

    #[test]
    fn run_fix_clean_code_returns_zero() {
        let (py, path) = write_temp(
            "basilisk_test_fix_clean.py",
            "def greet(name: str) -> str:\n    return name\n",
        );
        let code = run_fix(&[path], false, &[]);
        cleanup(&py);
        assert_eq!(code, 0, "clean code must return 0");
    }

    #[test]
    fn run_fix_applies_fixes_to_file() {
        let (py, path) = write_temp("basilisk_test_fix_apply.py", "x: int = 42\n");
        let (code, fixed) = fix_and_read(&path, &py, false, &[]);
        assert_eq!(code, 0, "fixable code must return 0");
        assert_eq!(fixed, "x = 42\n", "redundant annotation should be removed");
    }

    #[test]
    fn run_fix_with_specific_rule_only_fixes_that_rule() {
        let (py, path) = write_temp("basilisk_test_fix_specific_rule.py", "x: int = 42\n");
        let (code, fixed) = fix_and_read(&path, &py, false, &["BSK-0050".to_owned()]);
        assert_eq!(code, 0);
        assert_eq!(
            fixed, "x = 42\n",
            "BSK-0050 fix should be applied when specified"
        );
    }

    #[test]
    fn run_fix_with_unmatched_rule_does_not_fix() {
        let (py, path) = write_temp("basilisk_test_fix_unmatched_rule.py", "x: int = 42\n");
        let (code, fixed) = fix_and_read(&path, &py, false, &["BSK-0001".to_owned()]);
        assert_eq!(code, 0);
        assert_eq!(
            fixed, "x: int = 42\n",
            "file unchanged when rule does not match"
        );
    }

    #[test]
    fn run_fix_with_rules_all_applies_all_rules() {
        let (py, path) = write_temp("basilisk_test_fix_rules_all.py", "x: int = 42\n");
        let (code, fixed) = fix_and_read(&path, &py, false, &["all".to_owned()]);
        assert_eq!(code, 0);
        assert_eq!(fixed, "x = 42\n", "--rules all should apply all fixes");
    }

    #[test]
    fn run_fix_empty_rules_applies_all_safe_rules() {
        let (py, path) = write_temp("basilisk_test_fix_default_safe.py", "x: int = 42\n");
        let (code, fixed) = fix_and_read(&path, &py, false, &[]);
        assert_eq!(code, 0);
        assert_eq!(
            fixed, "x = 42\n",
            "default (safe) rules should fix BSK-0050"
        );
    }

    #[test]
    fn resolve_rules_empty_safe() {
        let result = resolve_rules(false, &[]);
        let expected: Vec<String> = SAFE_FIXABLE_RULES
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn resolve_rules_empty_unsafe() {
        let result = resolve_rules(true, &[]);
        let expected: Vec<String> = ALL_FIXABLE_RULES.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn resolve_rules_all_keyword() {
        let result = resolve_rules(false, &["ALL".to_owned()]);
        let expected: Vec<String> = ALL_FIXABLE_RULES.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn resolve_rules_specific_list() {
        let input = vec!["BSK-0001".to_owned(), "BSK-0050".to_owned()];
        assert_eq!(resolve_rules(false, &input), input);
    }

    // ── New e2e tests ────────────────────────────────────────────────────

    #[test]
    fn run_fix_applies_e0001_missing_param_annotation() {
        let (py, path) = write_temp("basilisk_test_fix_e0001.py", "def foo(x):\n    pass\n");
        let (code, fixed) = fix_and_read(&path, &py, false, &["BSK-0001".to_owned()]);
        assert_eq!(code, 0, "BSK-0001 fix must return 0");
        assert!(fixed.contains("def foo(x: Any)"), "got: {fixed}");
    }

    #[test]
    fn run_fix_applies_e0002_missing_return_annotation() {
        let (py, path) = write_temp("basilisk_test_fix_e0002.py", "def foo(x: int):\n    pass\n");
        let (code, fixed) = fix_and_read(&path, &py, false, &["BSK-0002".to_owned()]);
        assert_eq!(code, 0, "BSK-0002 fix must return 0");
        assert_eq!(fixed, "def foo(x: int) -> None:\n    pass\n");
    }

    #[test]
    fn run_fix_applies_e0005_missing_attribute_annotation() {
        let (py, path) = write_temp("basilisk_test_fix_e0005.py", "class Foo:\n    bar = []\n");
        let (code, fixed) = fix_and_read(&path, &py, false, &["BSK-0005".to_owned()]);
        assert_eq!(code, 0, "BSK-0005 fix must return 0");
        assert!(fixed.contains("bar: Any = []"), "got: {fixed}");
    }

    #[test]
    fn run_fix_applies_multiple_rules_in_one_file() {
        let (py, path) = write_temp(
            "basilisk_test_fix_multi_rules.py",
            "def foo(x):\n    pass\n\ny: int = 42\n",
        );
        let (code, fixed) = fix_and_read(&path, &py, false, &[]);
        assert_eq!(code, 0);
        assert!(
            fixed.contains("x: Any"),
            "BSK-0001 not applied, got: {fixed}"
        );
        assert!(
            fixed.contains("-> None"),
            "BSK-0002 not applied, got: {fixed}"
        );
        assert!(
            fixed.contains("y = 42"),
            "BSK-0050 not applied, got: {fixed}"
        );
    }

    #[test]
    fn run_fix_directory_traversal() {
        let dir = std::env::temp_dir().join("basilisk_test_fix_dir_traversal");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(
            dir.join("pyproject.toml"),
            "[tool.basilisk.rules]\n\"BSK-0050\" = \"warning\"\n",
        )
        .expect("write pyproject.toml");
        let file_a = dir.join("a_fix.py");
        let file_b = dir.join("b_fix.py");
        std::fs::write(&file_a, "x: int = 42\n").expect("write a");
        std::fs::write(&file_b, "y: str = \"hello\"\n").expect("write b");
        let code = run_fix(
            &[dir.to_string_lossy().into_owned()],
            false,
            &["BSK-0050".to_owned()],
        );
        let fixed_a = std::fs::read_to_string(&file_a).expect("read a");
        let fixed_b = std::fs::read_to_string(&file_b).expect("read b");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(code, 0);
        assert_eq!(fixed_a, "x = 42\n", "BSK-0050 not applied to first file");
        assert_eq!(
            fixed_b, "y = \"hello\"\n",
            "BSK-0050 not applied to second file"
        );
    }

    #[test]
    fn run_fix_is_idempotent() {
        let (py, path) = write_temp("basilisk_test_fix_idempotent.py", "x: int = 42\n");
        let first = run_fix(std::slice::from_ref(&path), false, &[]);
        let after_first = std::fs::read_to_string(&py).expect("read after first");
        assert_eq!(first, 0);
        assert_eq!(after_first, "x = 42\n");

        let second = run_fix(&[path], false, &[]);
        let after_second = std::fs::read_to_string(&py).expect("read after second");
        cleanup(&py);
        assert_eq!(second, 0);
        assert_eq!(after_second, "x = 42\n", "second pass should be a no-op");
    }

    #[test]
    fn run_fix_with_unsafe_flag() {
        let (py, path) = write_temp("basilisk_test_fix_unsafe.py", "x: int = 42\n");
        let (code, fixed) = fix_and_read(&path, &py, true, &[]);
        assert_eq!(code, 0);
        assert_eq!(
            fixed, "x = 42\n",
            "include_unsafe=true should apply BSK-0050"
        );
    }

    #[test]
    fn run_fix_preserves_surrounding_content() {
        let source = "# This is a comment\n\nx: int = 42\n\n\
                       # Another comment\ndef greet(name: str) -> str:\n\
                       \x20   \"\"\"Say hello.\"\"\"\n    return f\"Hello, {name}\"\n";
        let (py, path) = write_temp("basilisk_test_fix_preserves.py", source);
        let (code, fixed) = fix_and_read(&path, &py, false, &[]);
        assert_eq!(code, 0);
        assert!(
            fixed.contains("# This is a comment"),
            "leading comment lost"
        );
        assert!(fixed.contains("# Another comment"), "middle comment lost");
        assert!(fixed.contains("\"\"\"Say hello.\"\"\""), "docstring lost");
        assert!(
            fixed.contains("def greet(name: str) -> str:"),
            "clean fn changed"
        );
        assert!(fixed.contains("x = 42"), "BSK-0050 fix not applied");
    }

    #[test]
    fn run_fix_no_fixable_diagnostics_leaves_file_unchanged() {
        let source = "def foo(x: int) -> int:\n    return \"hello\"\n";
        let (py, path) = write_temp("basilisk_test_fix_unfixable_diags.py", source);
        let (code, fixed) = fix_and_read(&path, &py, false, &[]);
        assert_eq!(code, 0, "unfixable diagnostics should not cause errors");
        assert_eq!(fixed, source, "file must be unchanged when no fixes apply");
    }
}
