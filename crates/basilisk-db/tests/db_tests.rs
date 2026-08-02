//! Tests for [CHKARCH-INCREMENTAL-SALSA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for the Salsa incremental database.
//!
//! These prove the three properties that make the engine incremental, using a
//! database that records salsa's `WillExecute` events so we can observe exactly
//! which derived queries re-ran:
//!
//! 1. **Memoization** — re-querying an unchanged input does not re-execute.
//! 2. **Invalidation** — editing an input re-executes the queries that read it.
//! 3. **Isolation** — editing one file does not re-execute another file's query.

use basilisk_db::db::{BasiliskDatabase, Db, SourceFile};
use basilisk_test_utils::EventDb;
use salsa::{Database, Setter};

/// A derived query whose only input is the file text. We assert on its
/// execution count by name.
#[salsa::tracked(returns(copy))]
fn observed_len(db: &dyn Db, file: SourceFile) -> usize {
    file.text(db).len()
}

#[test]
fn unchanged_input_is_memoized_not_recomputed() {
    let db = EventDb::default();
    let file = SourceFile::new(&db, "a.py".to_owned(), "x = 1\n".to_owned());

    assert_eq!(
        observed_len(&db, file),
        6,
        "first query computes the length"
    );
    assert_eq!(
        db.executions_of("observed_len"),
        1,
        "the first query must execute exactly once"
    );

    assert_eq!(
        observed_len(&db, file),
        6,
        "second query returns the same value"
    );
    assert_eq!(
        db.executions_of("observed_len"),
        0,
        "re-querying an unchanged input must NOT re-execute — it is served from the memo"
    );
}

#[test]
fn editing_input_invalidates_and_recomputes() {
    let mut db = EventDb::default();
    let file = SourceFile::new(&db, "a.py".to_owned(), "x = 1\n".to_owned());

    assert_eq!(observed_len(&db, file), 6);
    let _ = db.executions_of("observed_len"); // drain

    let _previous = file.set_text(&mut db).to("x = 4242\n".to_owned());
    assert_eq!(observed_len(&db, file), 9, "value reflects the edited text");
    assert_eq!(
        db.executions_of("observed_len"),
        1,
        "editing the input must re-execute the dependent query exactly once"
    );
}

#[test]
fn editing_one_file_does_not_recompute_another() {
    let mut db = EventDb::default();
    let a = SourceFile::new(&db, "a.py".to_owned(), "aaaa\n".to_owned());
    let b = SourceFile::new(&db, "b.py".to_owned(), "bb\n".to_owned());

    // Prime both memos.
    assert_eq!(observed_len(&db, a), 5);
    assert_eq!(observed_len(&db, b), 3);
    let _ = db.executions_of("observed_len"); // drain priming executions

    // Edit only file B.
    let _previous = b.set_text(&mut db).to("bbbbbbbb\n".to_owned());

    // Re-querying A must be served from the memo — A read nothing that changed.
    assert_eq!(observed_len(&db, a), 5);
    assert_eq!(
        db.executions_of("observed_len"),
        0,
        "an unrelated edit must not invalidate file A's query"
    );

    // Re-querying B must recompute — its input changed.
    assert_eq!(observed_len(&db, b), 9);
    assert_eq!(
        db.executions_of("observed_len"),
        1,
        "file B's query must recompute after its own text changed"
    );
}

#[test]
fn cancellation_unwinds_in_flight_work() {
    // [CHKARCH-INCREMENTAL-CANCEL]: when a new keystroke arrives mid-check, the
    // in-flight computation must be abandoned rather than run to completion and
    // waste work — this is what keeps an editor responsive under fast typing.
    // Salsa implements it by raising its cancellation flag; the next query
    // checkpoint unwinds with the `Cancelled` sentinel. We drive that flag
    // explicitly so the test is fully deterministic (no thread race).
    let db = BasiliskDatabase::default();
    let token = db.cancellation_token();
    assert!(!token.is_cancelled(), "a live revision starts un-cancelled");

    token.cancel();
    assert!(token.is_cancelled(), "cancelling the token raises the flag");

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        db.unwind_if_revision_cancelled();
    }));
    let payload = outcome.expect_err("a cancelled revision must unwind, not return a stale result");
    assert!(
        payload.downcast_ref::<salsa::Cancelled>().is_some(),
        "the unwind payload must be salsa's `Cancelled` sentinel"
    );
}

#[test]
fn database_debug_does_not_leak_source_text() {
    // The Debug impl summarises the storage rather than dumping it, so the query
    // graph — which holds every file's source — never leaks into logs (no PII).
    let db = BasiliskDatabase::default();
    let _file = SourceFile::new(
        &db,
        "secret.py".to_owned(),
        "API_TOKEN = \"hunter2\"\n".to_owned(),
    );
    let shown = format!("{db:?}");
    assert!(shown.contains("BasiliskDatabase"), "Debug names the type");
    assert!(
        !shown.contains("hunter2"),
        "Debug must never leak source text into logs"
    );
}

#[test]
fn different_sources_produce_different_hashes() {
    // The content hash underpinning the cross-session cache must distinguish
    // different source strings.
    let hash_a = basilisk_db::hash_source("x = 1\n");
    let hash_b = basilisk_db::hash_source("x = 2\n");
    assert_ne!(
        hash_a, hash_b,
        "different sources must produce different hashes"
    );
}
