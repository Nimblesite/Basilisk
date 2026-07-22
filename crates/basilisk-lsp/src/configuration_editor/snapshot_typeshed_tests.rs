//! Tests for the server-described Typeshed source projection.

use std::path::PathBuf;
use std::sync::Arc;

use basilisk_stubs::typeshed::bundle::{bundled_commit_sha, bundled_snapshot};

use super::*;

/// [STUBRES-TYPESHED]: an unset pin IS the bundled commit — the picker shows
/// the effective SHA, never a third "latest" source.
#[test]
fn unset_pin_reports_the_bundled_commit_as_the_effective_source() {
    let unset = BasiliskConfig::default();
    assert_eq!(
        source(&unset),
        TypeshedSource::ExactCommit {
            commit: bundled_commit_sha().to_owned(),
        }
    );

    let pinned = BasiliskConfig {
        typeshed_commit: Some("a".repeat(40)),
        ..BasiliskConfig::default()
    };
    assert_eq!(
        source(&pinned),
        TypeshedSource::ExactCommit {
            commit: "a".repeat(40),
        }
    );

    let custom = BasiliskConfig {
        typeshed_path: Some(PathBuf::from("/user/typeshed")),
        ..BasiliskConfig::default()
    };
    assert!(matches!(
        source(&custom),
        TypeshedSource::CustomFolder { .. }
    ));
}

/// [STUBRES-TYPESHED-STORE]: pins resolve from a store folder (configured or
/// canonical); a custom folder resolves nothing from the store.
#[test]
fn store_folder_exists_only_for_pinned_sources() {
    let configured = BasiliskConfig {
        typeshed_store_path: Some(PathBuf::from("/stores/typeshed")),
        ..BasiliskConfig::default()
    };
    assert_eq!(
        store_folder(&configured, &source(&configured)).as_deref(),
        Some("/stores/typeshed")
    );

    let custom = BasiliskConfig {
        typeshed_path: Some(PathBuf::from("/user/typeshed")),
        typeshed_store_path: Some(PathBuf::from("/stores/typeshed")),
        ..BasiliskConfig::default()
    };
    assert_eq!(store_folder(&custom, &source(&custom)), None);
}

/// [LSPCFGED-TYPESHED]: a pin without a matching active generation has no
/// immutable license document to show, while a user-managed tree always
/// answers `ViewLicense` with its own terms.
#[test]
fn license_availability_tracks_the_matching_active_generation() {
    let custom_source = TypeshedSource::CustomFolder {
        path: "/user/typeshed".to_owned(),
    };
    assert!(license_available(&custom_source, None));

    let bundled_pin = TypeshedSource::ExactCommit {
        commit: bundled_commit_sha().to_owned(),
    };
    assert!(!license_available(&bundled_pin, None));

    let Ok(snapshot) = bundled_snapshot() else {
        unreachable!("release bundle must activate");
    };
    let ready = TypeshedGeneration::Ready(Arc::new(snapshot));
    assert!(license_available(&bundled_pin, Some(&ready)));

    let other_pin = TypeshedSource::ExactCommit {
        commit: "a".repeat(40),
    };
    assert!(
        !license_available(&other_pin, Some(&ready)),
        "a pin must not surface a different commit's license"
    );
}

/// [LSPCFGED-TYPESHED]: every projection is terminal. A snapshot is always
/// `Ready`; an unresolved root is `NoSource` with its reason — there is no
/// intermediate state for a client to render as a blocking overlay.
#[test]
fn projections_are_always_terminal() {
    let Ok(snapshot) = bundled_snapshot() else {
        unreachable!("release bundle must activate");
    };
    let ready = ready_projection(&snapshot.status);
    assert_eq!(ready.lifecycle, TypeshedLifecycle::Ready);
    assert!(ready.no_source_reason.is_none());
    assert_eq!(ready.active_source, Some(TypeshedActiveSource::Bundled));

    let unresolved = typeshed_configuration(&BasiliskConfig::default(), None);
    assert_eq!(unresolved.status.lifecycle, TypeshedLifecycle::NoSource);
    assert_eq!(
        unresolved.status.no_source_reason.as_deref(),
        Some("typeshed resolution has not run for this root")
    );
    assert_eq!(
        unresolved.status.license_status,
        TypeshedLicenseStatus::Unavailable
    );
}
