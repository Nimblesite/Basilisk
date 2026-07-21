//! Black-box target acceptance for one immutable Typeshed generation.
//!
//! Implements [TYPESHEDRT-ACCEPTANCE-TARGET]: target evidence filters the
//! active generation's `VERSIONS` and guarded bodies; it never chooses a
//! generation, infers a commit, or supplies an implicit Python target.

#![expect(
    clippy::expect_used,
    reason = "fixed target, archive, and JSON fixtures fail loudly"
)]

#[path = "support/typeshed_acquisition.rs"]
mod support;

use std::path::Path;
use std::sync::Arc;

use basilisk_stubs::pyi_parser::parse_pyi_source_for_target;
use basilisk_stubs::types::{StubSource, StubTarget, StubTargetPlatform, StubTier};
use basilisk_stubs::typeshed::bundle::bundled_snapshot;
use basilisk_stubs::typeshed::runtime::production_manager;
use basilisk_stubs::typeshed::store::StoreEntry;
use serde_json::Value;
use support::{approved_license, entry_from_files, install, pinned_request};

/// One immutable generation whose `VERSIONS` and guarded declarations differ
/// across explicit targets without changing the pinned commit.
fn target_entry() -> StoreEntry {
    let files = vec![
        ("LICENSE".to_owned(), approved_license()),
        (
            "stdlib/VERSIONS".to_owned(),
            b"guarded: 3.8-\nlegacy_only: 3.8-3.10\nmodern_only: 3.11-\n".to_vec(),
        ),
        (
            "stdlib/guarded.pyi".to_owned(),
            br#"import sys
if sys.version_info >= (3, 11):
    modern_name: int
else:
    legacy_name: str
if sys.platform == "win32":
    windows_name: bytes
else:
    posix_name: bytes
"#
            .to_vec(),
        ),
        (
            "stdlib/legacy_only.pyi".to_owned(),
            b"LEGACY: str\n".to_vec(),
        ),
        (
            "stdlib/modern_only.pyi".to_owned(),
            b"MODERN: int\n".to_vec(),
        ),
    ];
    entry_from_files(&files)
}

fn targets() -> (StubTarget, StubTarget) {
    (
        StubTarget {
            python_version: (3, 10),
            platform: StubTargetPlatform::Concrete("linux".to_owned()),
        },
        StubTarget {
            python_version: (3, 11),
            platform: StubTargetPlatform::Concrete("win32".to_owned()),
        },
    )
}

fn active_target_fixture() -> (
    Arc<basilisk_stubs::typeshed::snapshot::Snapshot>,
    tempfile::TempDir,
) {
    let store = tempfile::tempdir().expect("store root");
    let commit = install(store.path(), &target_entry());
    let snapshot = production_manager(pinned_request(commit, store.path()))
        .snapshot()
        .expect("target fixture resolution");
    (snapshot, store)
}

#[test]
fn same_sha_different_targets_use_the_active_versions_index() {
    let (snapshot, _store) = active_target_fixture();
    let identity = snapshot.identity.clone();
    let (legacy, modern) = targets();

    assert!(snapshot
        .read_stub_for_target("legacy_only", legacy.python_version)
        .is_some());
    assert!(snapshot
        .read_stub_for_target("modern_only", legacy.python_version)
        .is_none());
    assert!(snapshot
        .read_stub_for_target("legacy_only", modern.python_version)
        .is_none());
    assert!(snapshot
        .read_stub_for_target("modern_only", modern.python_version)
        .is_some());
    assert_eq!(snapshot.identity, identity);
    assert_eq!(snapshot.status.commit, identity.commit());
}

#[test]
fn target_changes_never_infer_or_fetch_a_different_commit() {
    let (snapshot, _store) = active_target_fixture();
    let pinned_identity = snapshot.identity.clone();
    let (legacy, modern) = targets();
    let (_, body) = snapshot.read_stub("guarded").expect("guarded body");

    let legacy_stub = parse_pyi_source_for_target(
        body,
        Path::new("guarded.pyi"),
        "guarded",
        StubSource::Typeshed,
        StubTier::Tier1,
        &legacy,
    )
    .expect("legacy target");
    assert!(legacy_stub.variables.contains_key("legacy_name"));
    assert!(legacy_stub.variables.contains_key("posix_name"));
    assert!(!legacy_stub.variables.contains_key("modern_name"));
    assert!(!legacy_stub.variables.contains_key("windows_name"));

    let modern_stub = parse_pyi_source_for_target(
        body,
        Path::new("guarded.pyi"),
        "guarded",
        StubSource::Typeshed,
        StubTier::Tier1,
        &modern,
    )
    .expect("modern target");
    assert!(modern_stub.variables.contains_key("modern_name"));
    assert!(modern_stub.variables.contains_key("windows_name"));
    assert!(!modern_stub.variables.contains_key("legacy_name"));
    assert!(!modern_stub.variables.contains_key("posix_name"));

    // The target selection changed NOTHING about the source: the identity is
    // still the one pinned commit, and there is no seam through which a
    // target could fetch or infer a different generation.
    assert_eq!(snapshot.identity, pinned_identity);
    assert_eq!(snapshot.status.commit, pinned_identity.commit());
}

#[test]
fn runtime_and_bundled_policy_manufactures_no_python_target() {
    let policy_sources = [
        include_str!("../src/lib.rs"),
        include_str!("../src/typeshed/bundle.rs"),
    ];
    assert!(!std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("build.rs")
        .exists());
    for forbidden in [
        "python_version",
        "python-version",
        "target_version",
        "target-version",
        "version_to_sha",
        "version-to-sha",
        "version_to_commit",
        "version-to-commit",
    ] {
        assert!(
            policy_sources
                .iter()
                .all(|source| !source.contains(forbidden)),
            "runtime and bundled policy must not contain target policy: {forbidden}"
        );
    }
    for minor in 0..=30 {
        assert!(
            policy_sources
                .iter()
                .all(|source| !source.contains(&format!("3.{minor}"))),
            "runtime and bundled policy must not manufacture Python 3.{minor}"
        );
    }

    let manifest: Value = serde_json::from_str(include_str!("../data/typeshed/manifest.json"))
        .expect("bundled manifest JSON");
    let source = manifest
        .get("source")
        .and_then(Value::as_object)
        .expect("source object");
    assert!(source.get("commit_sha").is_some_and(Value::is_string));
    for forbidden in [
        "python_version",
        "python-version",
        "python_platform",
        "python-platform",
        "target",
        "targets",
        "version_map",
        "commit_map",
    ] {
        assert!(!source.contains_key(forbidden));
        assert!(manifest.get(forbidden).is_none());
    }

    let bundled = bundled_snapshot().expect("bundled snapshot");
    assert_eq!(bundled.identity.commit(), bundled.status.commit);
    assert!(bundled.versions().is_some());
}
