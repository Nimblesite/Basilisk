//! Heavy end-to-end coverage for the custom-typeshed (`typeshed-path`) feature —
//! GitHub #271, spec [STUBRES-CUSTOM-TYPESHED].
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CUSTOM-TYPESHED
//!
//! These tests drive the REAL `basilisk` binary the way a user would, with many
//! sequential interactions per test (edit config, edit the typeshed on disk,
//! re-run `check`) and many assertions per interaction (exit code, every
//! diagnostic that must appear, and every diagnostic that must NOT). They pin the
//! load-bearing consequence of a custom typeshed being *canonical for stdlib
//! resolution* (typing-spec import-resolution step 3): a stdlib module absent
//! from the configured typeshed is surfaced as unresolved instead of being
//! silently rescued by the bundled name-set.
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// A unique temp directory per call so parallel tests never collide.
fn unique_dir(prefix: &str) -> PathBuf {
    static CTR: AtomicU64 = AtomicU64::new(0);
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("bsk_ts_heavy_{prefix}_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write(dir: &Path, rel: &str, contents: &str) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dir");
    }
    std::fs::write(path, contents).expect("write file");
}

/// Create a `<dir>/<typeshed>/stdlib/` tree seeded with the given `.pyi` files.
fn seed_typeshed(dir: &Path, typeshed: &str, stubs: &[(&str, &str)]) {
    let stdlib = dir.join(typeshed).join("stdlib");
    std::fs::create_dir_all(&stdlib).expect("create stdlib dir");
    for (name, body) in stubs {
        std::fs::write(stdlib.join(name), body).expect("write stub");
    }
}

/// Run `basilisk check app.py` in `dir` with the ambient venv scrubbed so the
/// result depends only on the on-disk config + typeshed.
fn check(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_basilisk"))
        .arg("check")
        .arg("app.py")
        .current_dir(dir)
        .env_remove("VIRTUAL_ENV")
        .output()
        .expect("spawn basilisk")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Assert the run is clean: exit 0 and the "no issues" banner, with none of the
/// forbidden markers present.
#[track_caller]
fn assert_clean(output: &Output, forbidden: &[&str]) {
    let out = stdout_of(output);
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected a clean exit, got {:?}; stdout: {out}",
        output.status.code()
    );
    assert!(
        out.contains("No issues found"),
        "expected the no-issues banner; stdout: {out}"
    );
    assert!(
        !out.contains("imports_unresolved"),
        "clean run must emit no imports_unresolved; stdout: {out}"
    );
    for marker in forbidden {
        assert!(
            !out.contains(marker),
            "clean run must not mention `{marker}`; stdout: {out}"
        );
    }
}

/// Assert the run flags `module` as unresolved: non-zero exit, the
/// `imports_unresolved` code, the module name, and none of the `resolved`
/// modules named.
#[track_caller]
fn assert_flags_unresolved(output: &Output, module: &str, resolved: &[&str]) {
    let out = stdout_of(output);
    assert_ne!(
        output.status.code(),
        Some(0),
        "expected a non-zero exit because `{module}` is unresolved; stdout: {out}"
    );
    assert!(
        out.contains("imports_unresolved"),
        "expected the imports_unresolved diagnostic code; stdout: {out}"
    );
    assert!(
        out.contains(module),
        "diagnostic must name the unresolved module `{module}`; stdout: {out}"
    );
    for ok in resolved {
        assert!(
            !out.contains(&format!("`{ok}`")),
            "module `{ok}` resolves from the custom typeshed and must NOT be \
             flagged; stdout: {out}"
        );
    }
}

const APP_OS_AND_FRACTIONS: &str =
    "from os import uname\nfrom fractions import Fraction\n\nname: str = uname()\nvalue = Fraction(1, 2)\n";

/// The full custom-typeshed lifecycle through `pyproject.toml`, exercised as one
/// continuous user session: five `check` interactions as the config and the
/// typeshed contents change on disk. Pins that canonicality is evaluated LIVE on
/// every run — not cached from a previous state.
#[test]
fn typeshed_path_full_lifecycle_through_pyproject() {
    let dir = unique_dir("lifecycle");
    seed_typeshed(&dir, "ts", &[("os.pyi", "def uname() -> str: ...\n")]);
    write(&dir, "app.py", APP_OS_AND_FRACTIONS);

    // ── Interaction 1: NO typeshed-path → bundled name-set rescues both stdlib
    // modules, so the project is clean even though nothing resolves on disk. ──
    write(
        &dir,
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n",
    );
    assert_clean(&check(&dir), &["imports_unresolved"]);

    // ── Interaction 2: add typeshed-path. The custom typeshed is now canonical
    // for step 3: `os` resolves from its stdlib/, but `fractions` — absent from
    // it — is no longer rescued by the bundled name-set and surfaces unresolved. ──
    write(
        &dir,
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ntypeshed-path = \"ts\"\n",
    );
    let out2 = check(&dir);
    assert_flags_unresolved(&out2, "fractions", &["os", "uname"]);
    let out2_text = stdout_of(&out2);
    assert_eq!(
        // Count the bracketed diagnostic HEADER, emitted exactly once per
        // diagnostic. The bare code `imports_unresolved` also appears in the
        // `see:` docs URL, so a raw substring count would double-report.
        out2_text.matches("error[imports_unresolved]").count(),
        1,
        "exactly one import (`fractions`) must be unresolved; stdout: {out2_text}"
    );
    assert!(
        out2_text.contains("Found 1 diagnostic (1 error)."),
        "the summary must report exactly one diagnostic; stdout: {out2_text}"
    );

    // ── Interaction 3: supply `fractions.pyi` in the custom typeshed → clean. ──
    seed_typeshed(
        &dir,
        "ts",
        &[(
            "fractions.pyi",
            "class Fraction:\n    def __init__(self, a: int, b: int) -> None: ...\n",
        )],
    );
    assert_clean(&check(&dir), &["fractions", "imports_unresolved"]);

    // ── Interaction 4: delete `fractions.pyi` again → it must flip back to
    // unresolved on the very next run (canonicality is live, not stale-cached). ──
    std::fs::remove_file(dir.join("ts").join("stdlib").join("fractions.pyi"))
        .expect("remove fractions stub");
    assert_flags_unresolved(&check(&dir), "fractions", &["os", "uname"]);

    // ── Interaction 5: remove typeshed-path entirely → the bundled name-set
    // rescues stdlib modules again, so the project is clean once more. ──
    write(
        &dir,
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n",
    );
    assert_clean(&check(&dir), &["imports_unresolved"]);

    let _ = std::fs::remove_dir_all(&dir);
}

/// `basilisk.json` `typeshedPath` (camelCase) and `pyproject.toml` `typeshed-path`
/// (kebab-case) are the SAME setting in two casings: given identical typesheds
/// and sources they must produce byte-for-byte identical diagnostics.
#[test]
fn typeshed_path_camelcase_json_matches_kebab_pyproject() {
    let run = |dir: &Path| -> Output {
        seed_typeshed(dir, "ts", &[("os.pyi", "def uname() -> str: ...\n")]);
        write(dir, "app.py", APP_OS_AND_FRACTIONS);
        check(dir)
    };

    // pyproject.toml, kebab-case.
    let toml_dir = unique_dir("json_vs_toml_toml");
    write(
        &toml_dir,
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ntypeshed-path = \"ts\"\n",
    );
    let toml_out = run(&toml_dir);

    // basilisk.json, camelCase (takes priority over pyproject, per config loader).
    let json_dir = unique_dir("json_vs_toml_json");
    write(&json_dir, "basilisk.json", "{ \"typeshedPath\": \"ts\" }\n");
    let json_out = run(&json_dir);

    // Both must flag `fractions` (absent) while resolving `os` (present).
    assert_flags_unresolved(&toml_out, "fractions", &["os", "uname"]);
    assert_flags_unresolved(&json_out, "fractions", &["os", "uname"]);

    // …and the two config surfaces must agree exactly on exit code and on the
    // diagnostic body (temp-dir path prefixes are the only permitted difference,
    // and app.py is relative so the diagnostic text itself matches).
    assert_eq!(
        toml_out.status.code(),
        json_out.status.code(),
        "camelCase JSON and kebab TOML must share an exit code"
    );
    assert_eq!(
        stdout_of(&toml_out),
        stdout_of(&json_out),
        "camelCase JSON and kebab TOML must produce identical diagnostics"
    );

    let _ = std::fs::remove_dir_all(&toml_dir);
    let _ = std::fs::remove_dir_all(&json_dir);
}

/// `stub-paths` (import-resolution step 1) is consulted BEFORE the custom
/// typeshed (step 3), so a user stub shadows a stdlib module even when a custom
/// typeshed is canonical — and a stdlib module absent from the typeshed but
/// present in `stub-paths` still resolves.
#[test]
fn stub_paths_shadow_custom_typeshed_end_to_end() {
    let dir = unique_dir("shadow");
    // Custom typeshed ships `os` only; `fractions` is deliberately absent.
    seed_typeshed(&dir, "ts", &[("os.pyi", "def uname() -> str: ...\n")]);
    // A user stub dir supplies `fractions` (the module the typeshed lacks).
    write(
        &dir,
        "mystubs/fractions.pyi",
        "class Fraction:\n    def __init__(self, a: int, b: int) -> None: ...\n",
    );
    write(
        &dir,
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ntypeshed-path = \"ts\"\nstub-paths = [\"mystubs\"]\n",
    );
    write(&dir, "app.py", APP_OS_AND_FRACTIONS);

    // `os` resolves from the custom typeshed, `fractions` from stub-paths →
    // the whole project is clean despite `fractions` being absent from typeshed.
    assert_clean(&check(&dir), &["imports_unresolved", "fractions"]);

    let _ = std::fs::remove_dir_all(&dir);
}

/// A configured custom typeshed governs ONLY the standard library. Third-party
/// imports are unaffected: they resolve (or fail) exactly as they would without
/// any `typeshed-path`.
#[test]
fn typeshed_path_leaves_third_party_imports_untouched() {
    let dir = unique_dir("thirdparty");
    seed_typeshed(&dir, "ts", &[("os.pyi", "def uname() -> str: ...\n")]);
    write(
        &dir,
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ntypeshed-path = \"ts\"\n",
    );
    // `os` resolves from the custom typeshed; `requests` is a genuinely missing
    // third-party package and must still be flagged.
    write(
        &dir,
        "app.py",
        "import requests\nfrom os import uname\n\nname: str = uname()\n",
    );

    let out = check(&dir);
    assert_flags_unresolved(&out, "requests", &["os", "uname"]);
    // The stdlib resolution must not leak into the third-party diagnostic.
    assert!(
        !stdout_of(&out).contains("fractions"),
        "unrelated stdlib names must not appear; stdout: {}",
        stdout_of(&out)
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A relative `typeshed-path` (resolved against the project root) and the
/// equivalent absolute path must behave identically.
#[test]
fn relative_and_absolute_typeshed_path_are_equivalent() {
    // Relative form.
    let rel_dir = unique_dir("rel");
    seed_typeshed(&rel_dir, "ts", &[("os.pyi", "def uname() -> str: ...\n")]);
    write(&rel_dir, "app.py", APP_OS_AND_FRACTIONS);
    write(
        &rel_dir,
        "pyproject.toml",
        "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ntypeshed-path = \"ts\"\n",
    );
    let rel_out = check(&rel_dir);

    // Absolute form: point at the SAME on-disk typeshed by absolute path.
    let abs_dir = unique_dir("abs");
    seed_typeshed(&abs_dir, "ts", &[("os.pyi", "def uname() -> str: ...\n")]);
    write(&abs_dir, "app.py", APP_OS_AND_FRACTIONS);
    let abs_typeshed = abs_dir.join("ts");
    write(
        &abs_dir,
        "pyproject.toml",
        &format!(
            "[project]\nname = \"x\"\nversion = \"0.1.0\"\n\n[tool.basilisk]\ntypeshed-path = \"{}\"\n",
            abs_typeshed.display()
        ),
    );
    let abs_out = check(&abs_dir);

    // Both resolve `os` and flag `fractions`, with the same exit code.
    assert_flags_unresolved(&rel_out, "fractions", &["os", "uname"]);
    assert_flags_unresolved(&abs_out, "fractions", &["os", "uname"]);
    assert_eq!(
        rel_out.status.code(),
        abs_out.status.code(),
        "relative and absolute typeshed-path must share an exit code"
    );

    let _ = std::fs::remove_dir_all(&rel_dir);
    let _ = std::fs::remove_dir_all(&abs_dir);
}
