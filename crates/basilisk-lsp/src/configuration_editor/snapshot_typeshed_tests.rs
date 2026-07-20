//! Tests for the server-described Typeshed source projection.

use std::path::PathBuf;

use super::*;

/// [LSPCFGED-TYPESHED]: a blocked official source has no immutable license
/// document to show, while a user-managed tree always states its own terms.
#[test]
fn blocked_official_source_cannot_offer_a_stale_license() {
    let exact = BasiliskConfig {
        typeshed_commit: Some("a".repeat(40)),
        ..BasiliskConfig::default()
    };
    let exact_source = source(&exact);
    assert_eq!(
        exact_source,
        TypeshedSource::ExactCommit {
            commit: "a".repeat(40),
        }
    );
    assert!(!license_available(
        &exact,
        &exact_source,
        TypeshedLifecycle::Blocked,
        None,
    ));

    let custom = BasiliskConfig {
        typeshed_path: Some(PathBuf::from("/user/typeshed")),
        ..BasiliskConfig::default()
    };
    let custom_source = source(&custom);
    assert!(matches!(custom_source, TypeshedSource::CustomFolder { .. }));
    assert!(license_available(
        &custom,
        &custom_source,
        TypeshedLifecycle::Blocked,
        None,
    ));
    assert_eq!(
        downloads(&custom, &custom_source),
        None,
        "a user-managed folder downloads nothing"
    );
}

/// A source that cannot be reached is never offered: pinning requires an
/// unpinned download with a settled commit behind it.
#[test]
fn pinning_is_offered_only_for_an_unpinned_settled_download() {
    let latest = BasiliskConfig::default();
    let latest_source = source(&latest);
    assert_eq!(latest_source, TypeshedSource::Latest);
    assert_eq!(pinnable_commit(&latest_source, false, None), None);
    assert_eq!(
        pinnable_commit(&latest_source, true, None),
        None,
        "an in-flight acquisition offers no pin"
    );

    let pinned = BasiliskConfig {
        typeshed_commit: Some("b".repeat(40)),
        ..BasiliskConfig::default()
    };
    assert_eq!(pinnable_commit(&source(&pinned), false, None), None);

    // Defaults resolve on the wire so the client never re-derives them.
    let policy = downloads(&latest, &latest_source);
    assert_eq!(
        policy.as_ref().map(|policy| policy.reuse_downloads),
        Some(true)
    );
    assert_eq!(
        policy.as_ref().map(|policy| policy.verify_content),
        Some(true)
    );
    assert_eq!(policy.and_then(|policy| policy.archive_url), None);
}
