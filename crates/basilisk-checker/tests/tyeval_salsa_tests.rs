//! External tests for [TYPEINF-TARGET-TYPELEVEL] Stage 3 — the memoized
//! Salsa queries returning whnf types
//! (`crates/basilisk-checker/src/tyeval/queries.rs`). See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-TYPELEVEL and
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CHECKLIST.
//!
//! Proves, via [`basilisk_test_utils::EventDb`]'s `WillExecute` log, that
//! normalized results are memoized across revisions: an edit that leaves
//! the alias environment unchanged **backdates** and the whnf memo
//! survives untouched.

use basilisk_checker::tyeval::{alias_whnf, type_alias_env};
use basilisk_checker::types::InferredType;
use basilisk_db::SourceFile;
use basilisk_test_utils::EventDb;
use salsa::Setter as _;

const MODULE: &str = r"type Json = None | bool | int | float | str | list[Json] | dict[str, Json]
type Pair[T] = tuple[T, T]

def unrelated() -> int:
    return 1
";

/// The guarded recursive `Json` alias normalizes to a whnf union with the
/// recursive interiors projected gradually — never a diagnostic-shaped
/// failure ([TYPEINF-TARGET-GRADUAL], the #371 boundary).
#[test]
fn recursive_alias_normalizes_through_the_query() {
    let db = EventDb::default();
    let file = SourceFile::new(&db, "m.py".to_owned(), MODULE.to_owned());
    let whnf = alias_whnf(&db, file, "Json".to_owned());
    assert!(
        InferredType::Int.is_assignable_to(&whnf),
        "int arm must survive normalization: {whnf:?}"
    );
    assert!(
        InferredType::List(Box::new(InferredType::Unknown)).is_assignable_to(&whnf),
        "recursive list arm must be present: {whnf:?}"
    );
}

/// Unguarded definitions are kept OUT of the environment by the acceptance
/// front door, so normalizing them projects to the gradual `Unknown`.
#[test]
fn unguarded_alias_projects_to_unknown_through_the_query() {
    let db = EventDb::default();
    let file = SourceFile::new(&db, "m.py".to_owned(), "type X = X\n".to_owned());
    assert!(type_alias_env(&db, file).get("X").is_none());
    assert_eq!(
        alias_whnf(&db, file, "X".to_owned()),
        InferredType::Unknown
    );
}

/// **Memoization of normalized results across revisions**: an edit outside
/// every `type` statement re-runs the (cheap) env lowering, which
/// backdates as unchanged — and the `alias_whnf` memo survives, proven by
/// the `WillExecute` log showing zero re-executions.
#[test]
fn whnf_memo_survives_unrelated_edits() {
    let mut db = EventDb::default();
    let file = SourceFile::new(&db, "m.py".to_owned(), MODULE.to_owned());
    let before = alias_whnf(&db, file, "Json".to_owned());
    // Drain setup events.
    let _ = db.executions_of("alias_whnf");

    // Edit ONLY the unrelated function's body: alias definitions unchanged.
    let edited = MODULE.replace("return 1", "return 2");
    assert_ne!(edited, MODULE);
    let _ = file.set_text(&mut db).to(edited);
    let after = alias_whnf(&db, file, "Json".to_owned());

    assert_eq!(before, after);
    assert_eq!(
        db.executions_of("alias_whnf"),
        0,
        "the normalized result must be memoized across the backdated env"
    );
}

/// Editing an alias definition DOES re-normalize — memoization must never
/// serve stale results.
#[test]
fn editing_an_alias_recomputes_its_whnf() {
    let mut db = EventDb::default();
    let file = SourceFile::new(&db, "m.py".to_owned(), MODULE.to_owned());
    let _ = alias_whnf(&db, file, "Pair".to_owned());
    let _ = db.executions_of("alias_whnf");

    let edited = MODULE.replace("tuple[T, T]", "list[T]");
    assert_ne!(edited, MODULE);
    let _ = file.set_text(&mut db).to(edited);
    let _ = alias_whnf(&db, file, "Pair".to_owned());
    assert_eq!(
        db.executions_of("alias_whnf"),
        1,
        "a changed definition must re-normalize"
    );
}
