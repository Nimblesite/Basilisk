//! Tests for [CHKARCH-INCREMENTAL-SALSA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    missing_docs
)]
//! Behavioural tests for the Salsa-memoized `checked_file` diagnostics query.
//!
//! The decisive property is that salsa memoization is **transparent**: the
//! memoized query returns exactly what the direct `parse → resolve → check`
//! pipeline returns — same diagnostics, every field preserved through the
//! `CachedDiagnostic` round-trip — so wrapping the checker in salsa can never
//! corrupt a result. (This is equivalence to the *pure* pipeline, not to the
//! batch CLI, which additionally resolves imports against the venv; see
//! `checked_file`'s docs.) The remaining tests prove the query is genuinely
//! incremental — it memoizes, invalidates on edit, and isolates files from each
//! other — using a database that records salsa's `WillExecute` events.

use basilisk_checker::{checked_file, file_diagnostics, Diagnostic, SourceFile};
use basilisk_test_utils::EventDb;
use salsa::Setter;

/// Source snippets exercised by the equivalence test: a clean file, a PEP type
/// error, a multi-statement file, an import-bearing file (which drives the
/// `imports_unresolved` rule and its help/provenance fields through the
/// round-trip), and an empty file.
const SAMPLES: &[(&str, &str)] = &[
    ("clean.py", "x: int = 1\n"),
    ("bad_assign.py", "x: int = \"not an int\"\n"),
    (
        "multi.py",
        "def f(a: int) -> int:\n    return a\n\ny: str = f(1)\n",
    ),
    (
        "imports.py",
        "import nonexistent_pkg\n\nx = nonexistent_pkg.frobnicate()\n",
    ),
    ("empty.py", "\n"),
];

/// The reference pipeline the query must match exactly.
fn reference_diagnostics(path: &str, text: &str) -> Vec<Diagnostic> {
    let parsed = basilisk_parser::parse_source(text.to_owned(), path.to_owned()).expect("parse");
    let resolved = basilisk_resolver::resolve(&parsed).expect("resolve");
    basilisk_checker::check(&resolved)
}

fn assert_same(got: &[Diagnostic], want: &[Diagnostic], label: &str) {
    assert_eq!(
        got.len(),
        want.len(),
        "{label}: diagnostic count must match the direct pipeline"
    );
    // Compare EVERY field — `file_diagnostics` round-trips through
    // `CachedDiagnostic`, so any field the projection drops must surface here.
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g.code.code, w.code.code, "{label}: code");
        assert_eq!(g.code.docs_url, w.code.docs_url, "{label}: docs_url");
        assert_eq!(g.span, w.span, "{label}: span");
        assert_eq!(g.path, w.path, "{label}: path");
        assert_eq!(g.message, w.message, "{label}: message");
        assert_eq!(g.severity, w.severity, "{label}: severity");
        assert_eq!(g.help, w.help, "{label}: help");
        assert_eq!(g.note, w.note, "{label}: note");
        assert_eq!(g.provenance, w.provenance, "{label}: provenance");
    }
}

#[test]
fn checked_file_is_equivalent_to_direct_check() {
    let db = EventDb::default();
    for (path, src) in SAMPLES {
        let file = SourceFile::new(&db, (*path).to_owned(), (*src).to_owned());
        let got = file_diagnostics(&db, file);
        let want = reference_diagnostics(path, src);
        assert_same(&got, &want, path);
    }
}

#[test]
fn unparseable_file_yields_no_diagnostics() {
    // A file that fails to parse produces no diagnostics — identical to the
    // batch CLI, which skips such files. Exercises the query's parse-error path.
    let db = EventDb::default();
    let file = SourceFile::new(&db, "broken.py".to_owned(), "def (= :\n".to_owned());
    assert!(
        file_diagnostics(&db, file).is_empty(),
        "an unparseable file must yield no diagnostics"
    );
    assert!(
        checked_file(&db, file).is_empty(),
        "the memoized projection is empty too"
    );
}

#[test]
fn checked_file_memoizes_then_invalidates_on_edit() {
    let mut db = EventDb::default();
    let file = SourceFile::new(&db, "a.py".to_owned(), "x: int = 1\n".to_owned());

    let _first = checked_file(&db, file);
    assert_eq!(
        db.executions_of("checked_file"),
        1,
        "the first check executes the query once"
    );

    let _second = checked_file(&db, file);
    assert_eq!(
        db.executions_of("checked_file"),
        0,
        "re-checking an unchanged file is served from the memo — no re-execution"
    );

    let _previous = file.set_text(&mut db).to("x: int = \"oops\"\n".to_owned());
    let _third = checked_file(&db, file);
    assert_eq!(
        db.executions_of("checked_file"),
        1,
        "editing the file re-executes the query exactly once"
    );
}

#[test]
fn editing_one_file_does_not_recheck_another() {
    let mut db = EventDb::default();
    let a = SourceFile::new(&db, "a.py".to_owned(), "x: int = 1\n".to_owned());
    let b = SourceFile::new(&db, "b.py".to_owned(), "y: int = 2\n".to_owned());

    let _a = checked_file(&db, a);
    let _b = checked_file(&db, b);
    let _ = db.executions_of("checked_file"); // drain priming

    let _previous = b.set_text(&mut db).to("y: int = \"bad\"\n".to_owned());

    let _a2 = checked_file(&db, a);
    assert_eq!(
        db.executions_of("checked_file"),
        0,
        "file A must NOT be rechecked when only file B changed"
    );

    let _b2 = checked_file(&db, b);
    assert_eq!(
        db.executions_of("checked_file"),
        1,
        "file B must be rechecked after its own edit"
    );
}
