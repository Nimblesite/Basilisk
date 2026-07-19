//! Acceptance tests for [STUBRES-TYPESHED-WARN].
#![expect(
    clippy::indexing_slicing,
    clippy::expect_used,
    reason = "acceptance test: direct indexing and expect are acceptable"
)]

use basilisk_stubs::typeshed::warning::{
    canonicalize, TypeshedWarning, UnpinnedKind, WarningSeverity,
};

#[test]
fn fallback_verification_and_unpinned_warnings_compose() {
    let mut warnings = vec![
        TypeshedWarning::Unverified,
        TypeshedWarning::DownloadFailed {
            bundled_sha: "83c2518a9e6abbda0c44592c3483de459198f887".to_owned(),
        },
        TypeshedWarning::Unpinned(UnpinnedKind::LatestOrBundled),
    ];
    canonicalize(&mut warnings);

    assert_eq!(
        warnings
            .iter()
            .map(TypeshedWarning::code)
            .collect::<Vec<_>>(),
        vec!["UNPINNED", "DOWNLOAD FAILED", "UNVERIFIED"]
    );
    assert_eq!(warnings[0].severity(), WarningSeverity::Advisory);
    assert_eq!(warnings[2].severity(), WarningSeverity::High);
}

#[test]
fn exact_verified_commit_has_no_source_warning() {
    let mut warnings = Vec::new();
    canonicalize(&mut warnings);
    assert!(warnings.is_empty());
}

#[test]
fn custom_source_is_unpinned_and_user_managed() {
    let mut warnings = vec![
        TypeshedWarning::UserManaged,
        TypeshedWarning::Unpinned(UnpinnedKind::CustomFolder),
    ];
    canonicalize(&mut warnings);
    assert_eq!(
        warnings
            .iter()
            .map(TypeshedWarning::code)
            .collect::<Vec<_>>(),
        vec!["UNPINNED", "USER-MANAGED SOURCE"]
    );
    assert!(warnings
        .iter()
        .all(|warning| warning.severity() == WarningSeverity::Advisory));
}

#[test]
fn warnings_serialize_without_losing_their_variant() {
    let warning = TypeshedWarning::DownloadFailed {
        bundled_sha: "83c2518a9e6abbda0c44592c3483de459198f887".to_owned(),
    };
    let value = serde_json::to_value(&warning).expect("warning serializes");
    assert!(value.get("DownloadFailed").is_some());
}
