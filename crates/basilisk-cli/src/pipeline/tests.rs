//! Unit tests for the shared collect-and-check pipeline.
//!
//! Cross-references [CHKARCH-CLI], [CHKARCH-COMMANDS], [CHKARCH-CONFIG-MODEL],
//! and [CHKARCH-CONFIG-DISCOVERY] (GitHub #311); the code under test is
//! `crate::pipeline`.

use super::*;
use crate::cache_check;

/// Default excludes for test helpers.
fn test_excludes() -> HashSet<&'static str> {
    basilisk_config::DEFAULT_EXCLUDES.iter().copied().collect()
}

/// Disabled cache options for tests that exercise the plain check pipeline.
fn no_cache() -> cache_check::CacheOptions {
    cache_check::CacheOptions {
        enabled: false,
        dir: None,
        stats: false,
    }
}

/// Run `collect_and_check` with the cache disabled and a throwaway tally.
fn collect_uncached(
    paths: &[String],
    scope: DiagnosticScope,
) -> Result<CheckOutcome, PipelineError> {
    collect_and_check(
        paths,
        &no_cache(),
        &mut cache_check::CacheStats::default(),
        scope,
    )
}

/// Unique temp dir for tests that need an isolated project root.
fn unique_project_dir(prefix: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}_{}_{n}", std::process::id()))
}

// ── DiagnosticScope ([CHKARCH-COMMANDS]) ──────────────────────────────────

/// [CHKARCH-COMMANDS]: the partition is exact — `check` keeps only
/// `pep`-tagged codes, `analyze` keeps only the rest, `union` keeps both.
#[test]
fn diagnostic_scope_partitions_by_pep_tag() {
    // `imports_unresolved` is a conformance (pep) rule; `BSK-0001` is a
    // Basilisk-original opt-in rule ([CHKTAG-PROVENANCE]).
    assert!(basilisk_checker::is_pep_rule("imports_unresolved"));
    assert!(!basilisk_checker::is_pep_rule("BSK-0001"));

    assert!(DiagnosticScope::Check.retains("imports_unresolved"));
    assert!(!DiagnosticScope::Check.retains("BSK-0001"));
    assert!(!DiagnosticScope::Analyze.retains("imports_unresolved"));
    assert!(DiagnosticScope::Analyze.retains("BSK-0001"));
    assert!(DiagnosticScope::Union.retains("imports_unresolved"));
    assert!(DiagnosticScope::Union.retains("BSK-0001"));
}

/// [CHKARCH-COMMANDS]: every catalog code lands in exactly one command scope.
#[test]
fn every_rule_belongs_to_exactly_one_command() {
    for descriptor in basilisk_checker::rule_catalog() {
        let check = DiagnosticScope::Check.retains(descriptor.code);
        let analyze = DiagnosticScope::Analyze.retains(descriptor.code);
        assert!(
            check ^ analyze,
            "rule {} must belong to exactly one command scope",
            descriptor.code
        );
    }
}

// ── collect_python_files ──────────────────────────────────────────────────

#[test]
fn collect_python_files_returns_err_for_nonexistent_path() {
    let result = collect_python_files(&["/no/such/path/ever.py".to_owned()], &test_excludes());
    assert!(result.is_err(), "nonexistent path must return Err");
}

#[test]
fn collect_python_files_skips_non_py_file() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir();
    let txt = dir.join("basilisk_test_skip.txt");
    std::fs::write(&txt, b"hello")?;
    let path = txt.to_string_lossy().into_owned();
    let files = collect_python_files(&[path], &test_excludes())?;
    assert!(files.is_empty(), "non-.py file must be skipped");
    let _ = std::fs::remove_file(&txt);
    Ok(())
}

#[test]
fn collect_python_files_includes_py_file() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir();
    let py = dir.join("basilisk_test_include.py");
    std::fs::write(&py, b"x = 1")?;
    let path = py.to_string_lossy().into_owned();
    let files = collect_python_files(&[path], &test_excludes())?;
    let _ = std::fs::remove_file(&py);
    assert_eq!(files.len(), 1, ".py file must be included");
    Ok(())
}

#[test]
fn collect_python_files_walks_directory() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::temp_dir().join("basilisk_test_walk_dir");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base)?;
    std::fs::write(base.join("a.py"), b"x = 1")?;
    std::fs::write(base.join("b.txt"), b"ignored")?;
    let path = base.to_string_lossy().into_owned();
    let files = collect_python_files(&[path], &test_excludes())?;
    let _ = std::fs::remove_dir_all(&base);
    assert_eq!(
        files.len(),
        1,
        "directory walk must find exactly one .py file"
    );
    Ok(())
}

/// `collect_python_files` — `MatchArmGuard → true` mutant: the `NotFound`
/// guard distinguishes "not found" from other I/O errors. The `NotFound`
/// path specifically returns Err (not Ok with an empty list).
#[test]
fn collect_python_files_not_found_returns_err() {
    let result = collect_python_files(
        &["/absolutely/does/not/exist/file.py".to_owned()],
        &test_excludes(),
    );
    assert!(result.is_err(), "NotFound path must return Err, not Ok");
}

/// Complement: a path that exists but is not .py returns Ok with empty list.
/// This kills the `true` guard mutant: if all errors → Err, this would fail.
#[test]
fn collect_python_files_non_py_existing_file_returns_ok_empty(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir();
    let txt = dir.join("basilisk_test_guard_complement.txt");
    std::fs::write(&txt, b"hello")?;
    let path = txt.to_string_lossy().into_owned();
    let result = collect_python_files(&[path], &test_excludes());
    let _ = std::fs::remove_file(&txt);
    assert!(result.is_ok(), "existing non-py file must return Ok");
    assert!(result?.is_empty(), "non-py file must produce empty list");
    Ok(())
}

#[test]
fn collect_python_files_skips_excluded_directories() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::temp_dir().join("basilisk_test_exclude_dirs");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base)?;

    // File in root — should be found.
    std::fs::write(base.join("app.py"), b"x = 1")?;

    // Files in default-excluded directories — should be skipped.
    for excluded in &["__pycache__", "venv", "site-packages", "node_modules"] {
        let sub = base.join(excluded);
        std::fs::create_dir_all(&sub)?;
        std::fs::write(sub.join("hidden.py"), b"x = 1")?;
    }

    // File in a hidden directory — should be skipped.
    let hidden = base.join(".hidden");
    std::fs::create_dir_all(&hidden)?;
    std::fs::write(hidden.join("secret.py"), b"x = 1")?;

    let path = base.to_string_lossy().into_owned();
    let files = collect_python_files(&[path], &test_excludes())?;
    let _ = std::fs::remove_dir_all(&base);

    assert_eq!(
        files.len(),
        1,
        "only root app.py should be found, got: {files:?}"
    );
    Ok(())
}

#[test]
fn collect_python_files_respects_custom_excludes() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::temp_dir().join("basilisk_test_custom_exclude");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base)?;

    std::fs::write(base.join("app.py"), b"x = 1")?;
    let sub = base.join("vendor");
    std::fs::create_dir_all(&sub)?;
    std::fs::write(sub.join("lib.py"), b"x = 1")?;

    // Custom exclude: only "vendor", not the defaults.
    let custom: HashSet<&str> = ["vendor"].into_iter().collect();
    let path = base.to_string_lossy().into_owned();
    let files = collect_python_files(&[path], &custom)?;
    let _ = std::fs::remove_dir_all(&base);

    assert_eq!(
        files.len(),
        1,
        "vendor should be excluded, only app.py found"
    );
    Ok(())
}

/// Regression: the `basilisk check` CLI ignored user **glob** excludes.
/// `collect_python_files` must honour gitignore-style globs via
/// `basilisk_config::path_matches_pattern`, agreeing with the LSP scan.
/// Implements [CHKARCH-CONFIG-EXCLUDE].
#[test]
fn collect_python_files_honors_user_glob_excludes() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::temp_dir().join(format!(
        "basilisk_test_cli_glob_exclude_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    let gen = base.join("src").join("generated");
    std::fs::create_dir_all(&gen)?;
    std::fs::write(base.join("app.py"), b"x = 1")?; // real code — must survive
    std::fs::write(gen.join("models.py"), b"y = 2")?; // excluded by **/generated/**
    std::fs::write(base.join("schema.pb.py"), b"z = 3")?; // excluded by *.pb.py

    let excludes: HashSet<&str> = ["**/generated/**", "*.pb.py"].into_iter().collect();
    let path = base.to_string_lossy().into_owned();
    let files = collect_python_files(&[path], &excludes)?;
    let _ = std::fs::remove_dir_all(&base);

    let names: Vec<String> = files.iter().map(|f| f.replace('\\', "/")).collect();
    assert_eq!(
        files.len(),
        1,
        "only app.py should survive the glob excludes, got: {names:?}"
    );
    assert!(
        names.iter().any(|f| f.ends_with("/app.py")),
        "app.py must still be collected: {names:?}"
    );
    assert!(
        !names.iter().any(|f| f.contains("generated")),
        "**/generated/** must exclude the nested directory: {names:?}"
    );
    assert!(
        !names.iter().any(|f| f.contains("schema.pb.py")),
        "*.pb.py glob must exclude the file: {names:?}"
    );
    Ok(())
}

/// Regression: `basilisk check .` found zero files because the root
/// entry `.` starts with `.` and was rejected by the hidden-dir filter.
#[test]
fn collect_python_files_hidden_root_dir_still_walked() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::temp_dir().join("basilisk_test_hidden_root");
    let _ = std::fs::remove_dir_all(&base);

    let hidden = base.join(".myproject");
    std::fs::create_dir_all(&hidden)?;
    std::fs::write(hidden.join("app.py"), b"x = 1")?;
    let sub = hidden.join("pkg");
    std::fs::create_dir_all(&sub)?;
    std::fs::write(sub.join("mod.py"), b"y = 2")?;

    let path = hidden.to_string_lossy().into_owned();
    let files = collect_python_files(&[path], &test_excludes())?;
    let _ = std::fs::remove_dir_all(&base);

    assert_eq!(
        files.len(),
        2,
        "user-supplied root starting with '.' must still be walked, got: {files:?}"
    );
    Ok(())
}

// ── collect_and_check ─────────────────────────────────────────────────────

#[test]
#[cfg(unix)]
fn collect_and_check_handles_unreadable_py_file() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir();
    let py = dir.join("basilisk_test_locked.py");
    std::fs::write(&py, b"def foo(): pass")?;
    std::fs::set_permissions(&py, std::fs::Permissions::from_mode(0o000))?;

    let path = py.to_string_lossy().into_owned();
    let result = collect_uncached(&[path], DiagnosticScope::Union);
    std::fs::set_permissions(&py, std::fs::Permissions::from_mode(0o644))?;
    let _ = std::fs::remove_file(&py);

    let outcome = result.map_err(|err| err.to_string())?;
    assert!(
        outcome.diagnostics.is_empty(),
        "unreadable file produces no diagnostics, got: {:#?}",
        outcome.diagnostics
    );
    assert_eq!(
        outcome.failures.len(),
        1,
        "unreadable file must be a failure"
    );
    Ok(())
}

/// [CHKARCH-COMMANDS]: `def foo(x)` violates the annotation house rules
/// (BSK-0001/BSK-0002), which are analyze-scope opt-ins. An opted-in project
/// sees them under the analyze scope — and never under the check scope.
#[test]
fn collect_and_check_scopes_house_rules_to_analyze() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_project_dir("basilisk_test_bad_code");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("pyproject.toml"),
        b"[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n\"BSK-0002\" = \"error\"\n",
    )?;
    let py = dir.join("bad.py");
    std::fs::write(&py, b"def foo(x):\n    pass\n")?;
    let path = py.to_string_lossy().into_owned();
    let analyze = collect_uncached(std::slice::from_ref(&path), DiagnosticScope::Analyze)
        .map_err(|err| err.to_string())?;
    let check = collect_uncached(&[path], DiagnosticScope::Check).map_err(|err| err.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        analyze
            .diagnostics
            .iter()
            .any(|d| d.code.code == "BSK-0001"),
        "analyze scope must fire the configured house rule"
    );
    assert!(
        check.diagnostics.iter().all(|d| d.code.code != "BSK-0001"),
        "check scope must never emit a house rule, even when configured \
         ([CHKARCH-COMMANDS]); got: {:#?}",
        check.diagnostics
    );
    Ok(())
}

/// Regression: `basilisk analyze` must honor `[tool.basilisk.rules]` severity
/// grades from `pyproject.toml`. A project that escalates BSK-0050 to "error"
/// must see it surface as a hard error through the real pipeline.
/// [CHKARCH-CONFIG-MODEL]
#[test]
fn collect_and_check_applies_project_rule_severity_override(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_project_dir("basilisk_cli_cfg_promote");
    std::fs::create_dir_all(&dir)?;
    // An explicit non-disabled severity both selects and grades an
    // off-by-default rule. See [CHKARCH-CONFIGURATION-ONLY].
    std::fs::write(
        dir.join("pyproject.toml"),
        b"[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n\
          [tool.basilisk.rules]\n\"BSK-0050\" = \"error\"\n",
    )?;
    let py = dir.join("m.py");
    std::fs::write(&py, b"x: int = 42\n")?;

    let path = py.to_string_lossy().into_owned();
    let outcome =
        collect_uncached(&[path], DiagnosticScope::Analyze).map_err(|err| err.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);

    let w0050: Vec<_> = outcome
        .diagnostics
        .iter()
        .filter(|d| d.code.code == "BSK-0050")
        .collect();
    assert!(!w0050.is_empty(), "BSK-0050 must fire under analyze");
    assert!(
        w0050
            .iter()
            .all(|d| d.severity == basilisk_checker::Severity::Error),
        "project config `BSK-0050 = \"error\"` must promote BSK-0050 to error; got {:?}",
        w0050.iter().map(|d| d.severity).collect::<Vec<_>>()
    );
    Ok(())
}

// ── pep-disable violations ([CHKARCH-CONFIG-MODEL]) ───────────────────────

/// [CHKARCH-CONFIG-MODEL]: a config that resolves a `pep` rule to `disabled`
/// is invalid — the pipeline fails with a configuration error before checking.
#[test]
fn pep_disable_config_fails_the_run() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_project_dir("basilisk_cli_pep_disable");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("pyproject.toml"),
        b"[tool.basilisk.rules]\n\"imports_unresolved\" = \"disabled\"\n",
    )?;
    let py = dir.join("m.py");
    std::fs::write(&py, b"x: int = 1\n")?;

    let path = py.to_string_lossy().into_owned();
    let check = collect_uncached(std::slice::from_ref(&path), DiagnosticScope::Check);
    let analyze = collect_uncached(&[path], DiagnosticScope::Analyze);
    let _ = std::fs::remove_dir_all(&dir);

    for (command, result) in [("check", check), ("analyze", analyze)] {
        let message = match result {
            Err(PipelineError::Config(message)) => message,
            Err(PipelineError::Internal(message)) => {
                return Err(format!(
                    "`{command}` must fail with a Config error, got Internal: {message}"
                )
                .into())
            }
            Ok(outcome) => {
                return Err(format!(
                    "`{command}` must fail with a Config error, got Ok with {} diagnostics",
                    outcome.diagnostics.len()
                )
                .into())
            }
        };
        assert!(
            message.contains("imports_unresolved"),
            "`{command}` config error must name the offending code, got: {message}"
        );
    }
    Ok(())
}

/// Grading (not disabling) a pep rule remains valid configuration.
/// [CHKARCH-CONFIG-MODEL]
#[test]
fn pep_grade_config_is_valid() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_project_dir("basilisk_cli_pep_grade");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        dir.join("pyproject.toml"),
        b"[tool.basilisk.rules]\n\"imports_unresolved\" = \"warning\"\n",
    )?;
    let py = dir.join("m.py");
    std::fs::write(&py, b"x: int = 1\n")?;

    let path = py.to_string_lossy().into_owned();
    let outcome = collect_uncached(&[path], DiagnosticScope::Check);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        outcome.is_ok(),
        "grading a pep rule to warning must be valid config"
    );
    Ok(())
}

// ── config discovery (GitHub #311, [CHKARCH-CONFIG-DISCOVERY]) ────────────

/// Source that violates the annotation house rules (BSK-0001 on the
/// parameter, BSK-0002 on the return) once those opt-in rules are enabled.
const UNANNOTATED_FN: &[u8] = b"def foo(x):\n    pass\n";

/// A `[tool.basilisk.rules]` table enabling the opt-in annotation rules.
const ANNOTATION_RULES_TOML: &[u8] =
    b"[tool.basilisk.rules]\n\"BSK-0001\" = \"error\"\n\"BSK-0002\" = \"error\"\n";

/// GitHub #311 (headline): `basilisk analyze path/to/file.py` must discover
/// rule config from ancestor directories, so a repo-root `pyproject.toml`
/// governs a file checked by path.
#[test]
fn analyze_file_arg_discovers_config_from_ancestor_directories(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_project_dir("basilisk_cli_cfg_ancestor");
    let child = root.join("child");
    std::fs::create_dir_all(&child)?;
    std::fs::write(root.join("pyproject.toml"), ANNOTATION_RULES_TOML)?;
    let py = child.join("bad.py");
    std::fs::write(&py, UNANNOTATED_FN)?;

    let outcome = collect_uncached(
        &[py.to_string_lossy().into_owned()],
        DiagnosticScope::Analyze,
    );
    let _ = std::fs::remove_dir_all(&root);
    let outcome = outcome.map_err(|err| err.to_string())?;

    let codes: Vec<&str> = outcome.diagnostics.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-0001"),
        "checking child/bad.py by file path must apply the root pyproject.toml \
         (ancestor walk, GitHub #311); got codes: {codes:?}"
    );
    Ok(())
}

/// GitHub #311 (consequence 3): results must not depend on argument order.
/// With rules in `p/pyproject.toml` and none in `q`, both `analyze p q` and
/// `analyze q p` must flag `p/bad.py` — and never flag `q/bad.py`.
#[test]
fn results_are_independent_of_argument_order() -> Result<(), Box<dyn std::error::Error>> {
    let base = unique_project_dir("basilisk_cli_cfg_order");
    let p = base.join("p");
    let q = base.join("q");
    std::fs::create_dir_all(&p)?;
    std::fs::create_dir_all(&q)?;
    std::fs::write(p.join("pyproject.toml"), ANNOTATION_RULES_TOML)?;
    std::fs::write(p.join("bad.py"), UNANNOTATED_FN)?;
    std::fs::write(q.join("bad.py"), UNANNOTATED_FN)?;

    let p_arg = p.to_string_lossy().into_owned();
    let q_arg = q.to_string_lossy().into_owned();
    let p_first = collect_uncached(&[p_arg.clone(), q_arg.clone()], DiagnosticScope::Analyze);
    let q_first = collect_uncached(&[q_arg, p_arg], DiagnosticScope::Analyze);
    let _ = std::fs::remove_dir_all(&base);

    for (order, outcome) in [
        ("analyze p q", p_first.map_err(|err| err.to_string())?),
        ("analyze q p", q_first.map_err(|err| err.to_string())?),
    ] {
        let e0001_paths: Vec<&str> = outcome
            .diagnostics
            .iter()
            .filter(|d| d.code.code == "BSK-0001")
            .map(|d| d.path.as_str())
            .collect();
        assert!(
            e0001_paths
                .iter()
                .any(|path| std::path::Path::new(path).starts_with(&p)),
            "`{order}` must apply p's own config to p/bad.py regardless of \
             argument order (GitHub #311); BSK-0001 paths: {e0001_paths:?}"
        );
        assert!(
            e0001_paths
                .iter()
                .all(|path| !std::path::Path::new(path).starts_with(&q)),
            "`{order}` must NOT leak p's config onto q/bad.py, which has no \
             config anywhere above it (GitHub #311); BSK-0001 paths: {e0001_paths:?}"
        );
    }
    Ok(())
}

/// [CHKARCH-CONFIG-MODEL]: the nearest table that decides a rule wins, per
/// rule. The root enables both annotation rules; the child only disables
/// BSK-0001, so BSK-0002 is still decided by the root and must fire.
#[test]
fn nearest_deciding_table_wins_per_rule() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_project_dir("basilisk_cli_cfg_nearest");
    let child = root.join("child");
    std::fs::create_dir_all(&child)?;
    std::fs::write(root.join("pyproject.toml"), ANNOTATION_RULES_TOML)?;
    std::fs::write(
        child.join("pyproject.toml"),
        b"[tool.basilisk.rules]\n\"BSK-0001\" = \"disabled\"\n",
    )?;
    let py = child.join("bad.py");
    std::fs::write(&py, UNANNOTATED_FN)?;

    let outcome = collect_uncached(
        &[py.to_string_lossy().into_owned()],
        DiagnosticScope::Analyze,
    );
    let _ = std::fs::remove_dir_all(&root);
    let outcome = outcome.map_err(|err| err.to_string())?;

    let codes: Vec<&str> = outcome.diagnostics.iter().map(|d| d.code.code).collect();
    assert!(
        codes.contains(&"BSK-0002"),
        "the root's rule opt-ins must survive a child table that only decides \
         one rule (nearest-deciding-table, [CHKARCH-CONFIG-MODEL]); got: {codes:?}"
    );
    assert!(
        !codes.contains(&"BSK-0001"),
        "the child table's `BSK-0001 = disabled` must be honored; got: {codes:?}"
    );
    Ok(())
}

#[test]
fn collect_and_check_returns_no_diagnostics_for_clean_code(
) -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir();
    let py = dir.join("basilisk_test_clean_code.py");
    std::fs::write(&py, b"def greet(name: str) -> str:\n    return name\n")?;
    let path = py.to_string_lossy().into_owned();
    let outcome =
        collect_uncached(&[path], DiagnosticScope::Union).map_err(|err| err.to_string())?;
    let _ = std::fs::remove_file(&py);
    assert!(
        outcome.diagnostics.is_empty(),
        "fully annotated code must produce no diagnostics"
    );
    Ok(())
}

// ── Self-named external bases (issues #278/#299 family) ──────────────────

/// `class EnvironBuilder(werkzeug.test.EnvironBuilder)` — flask 3.1.1
/// `src/flask/testing.py`, hit by the [VSIX-REALWORLD-JOURNEY] flask corpus —
/// records its unresolved external base under the class's own terminal name.
/// On the FULL CLI pipeline (which, unlike the bare parse→resolve→check test
/// harness, first runs `resolve_module_imports`) the base walk must not
/// recurse through the self-referential class-map entry: before the fix this
/// stack-overflowed and aborted the whole `basilisk check` process (and
/// crash-looped the LSP on the same code path).
#[test]
fn self_named_attribute_base_check_does_not_overflow() -> Result<(), Box<dyn std::error::Error>> {
    let dir = unique_project_dir("bsk_selfnamed_attr_base");
    std::fs::create_dir_all(&dir)?;
    let py = dir.join("repro.py");
    std::fs::write(
        &py,
        b"import werkzeug.test\n\n\nclass EnvironBuilder(werkzeug.test.EnvironBuilder):\n    pass\n",
    )?;
    let path = py.to_string_lossy().into_owned();
    let outcome =
        collect_uncached(&[path], DiagnosticScope::Check).map_err(|err| err.to_string())?;
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        outcome.failures.is_empty(),
        "self-named external base must analyse cleanly, got failures: {:?}",
        outcome.failures
    );
    assert_eq!(
        outcome.sources.len(),
        1,
        "exactly the repro file is checked"
    );
    Ok(())
}

// ── pluralise ─────────────────────────────────────────────────────────────

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
