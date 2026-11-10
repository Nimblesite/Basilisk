//! Pins for [RESOLV-CANONICAL-REGISTRY] fail-closed behaviour.
//!
//! A registry that fails to load must not fail OPEN: a process that keeps
//! running with an empty index silently answers `None` for every canonical
//! lookup, which disables every recognition-based rule while appearing
//! healthy. The load result must be observable so drivers can refuse to
//! produce verdicts from a checker that recognises nothing.
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

#[test]
fn bundled_registry_reports_healthy() {
    assert_eq!(
        basilisk_canonical::registry_health(),
        Ok(()),
        "the bundled registry must load; a driver consults this before \
         trusting any canonical lookup"
    );
}

#[test]
fn health_failure_carries_the_load_error() {
    // The bundled data is valid, so the error path is exercised through the
    // crate's own unit tests over `build_index`; this pin fixes the public
    // CONTRACT: health is a `Result`, not a log line — a failed load is a
    // value a caller can branch on, not a side effect the process outlives.
    let health: Result<(), String> = basilisk_canonical::registry_health();
    assert!(health.is_ok());
}
