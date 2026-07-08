//! Pins salsa 0.27's input-`set` semantics for [CHKARCH-INCREMENTAL-SALSA].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
#![allow(clippy::allow_attributes, missing_docs)]
//! Salsa treats **every** input `set` as a new revision — there is no
//! same-value equality shortcut — so a dependent query re-executes even when
//! the value written is identical to the stored one. This is why the LSP's
//! `SalsaAnalysisEngine` compares each input against the stored value before
//! setting it (`source_for` / `config_for` / `search_paths_for`): without the
//! guard, syncing inputs on every analysis would silently discard the whole
//! database's memos, turning the incremental engine into a full recompute.
//!
//! If this test ever FAILS (0 re-executions), salsa has gained same-value
//! backdating and the engine's compare-before-set guards have become optional.

use basilisk_checker::{checked_file, ConfigInput, ConfigValue, SourceFile};
use basilisk_config::BasiliskConfig;
use basilisk_test_utils::EventDb;
use salsa::Setter;

#[test]
fn same_value_input_set_reexecutes_dependents() {
    let mut db = EventDb::default();
    let config = ConfigInput::new(&db, ConfigValue(BasiliskConfig::default()));
    let file = SourceFile::new(&db, "a.py".to_owned(), "x: int = 1\n".to_owned());

    let _ = checked_file(&db, file, config);
    let _ = db.executions_of("checked_file"); // drain priming

    // Re-query without any set: served from the memo.
    let _ = checked_file(&db, file, config);
    assert_eq!(
        db.executions_of("checked_file"),
        0,
        "an untouched input is served from the memo"
    );

    // Set the text to the IDENTICAL value: salsa still re-executes.
    let _ = file.set_text(&mut db).to("x: int = 1\n".to_owned());
    let _ = checked_file(&db, file, config);
    assert_eq!(
        db.executions_of("checked_file"),
        1,
        "salsa re-executes a dependent after a same-value input set — the \
         engine MUST compare-before-set; if this now reports 0, salsa gained \
         same-value backdating and the engine guards are optional"
    );
}
