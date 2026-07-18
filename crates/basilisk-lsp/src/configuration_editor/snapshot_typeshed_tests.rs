//! Tests for server-described Typeshed controls.

use std::path::PathBuf;

use super::*;

#[test]
fn blocked_official_source_cannot_offer_a_stale_license() {
    let exact = BasiliskConfig {
        typeshed_commit: Some("a".repeat(40)),
        ..BasiliskConfig::default()
    };
    assert!(!view_license_enabled(
        &exact,
        TypeshedSourceMode::ExactCommit,
        TypeshedLifecycle::Blocked,
        None,
    ));

    let custom = BasiliskConfig {
        typeshed_path: Some(PathBuf::from("/user/typeshed")),
        ..BasiliskConfig::default()
    };
    assert!(view_license_enabled(
        &custom,
        TypeshedSourceMode::CustomFolder,
        TypeshedLifecycle::Blocked,
        None,
    ));
}
