use std::sync::Mutex;

use super::super::archive::{Archive, ArchiveEntry, ArchiveVfs};
use super::super::gittree::FileMode;
use super::*;

const OTHER_SHA: &str = "0123456789012345678901234567890123456789";
const BUNDLE_SHA: &str = "83c2518a9e6abbda0c44592c3483de459198f887";

#[derive(Default)]
struct FakeBackend {
    custom: Mutex<Option<Result<Snapshot, BackendError>>>,
    pinned: Mutex<Option<Result<Snapshot, BackendError>>>,
    bundle: Mutex<Option<Result<Snapshot, BackendError>>>,
    calls: Mutex<Vec<&'static str>>,
}

impl FakeBackend {
    fn take(
        slot: &Mutex<Option<Result<Snapshot, BackendError>>>,
    ) -> Result<Snapshot, BackendError> {
        let Ok(mut value) = slot.lock() else {
            return Err(BackendError::Corrupt);
        };
        value.take().unwrap_or(Err(BackendError::Corrupt))
    }

    fn record(&self, call: &'static str) {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(call);
        }
    }
}

impl SourceBackend for FakeBackend {
    fn load_custom(&self, _path: &str) -> Result<Snapshot, BackendError> {
        self.record("custom");
        Self::take(&self.custom)
    }

    fn load_pinned(&self, _commit: Oid, _explicit: bool) -> Result<Snapshot, BackendError> {
        self.record("pinned");
        Self::take(&self.pinned)
    }

    fn load_bundled(&self) -> Result<Snapshot, BackendError> {
        self.record("bundle");
        Self::take(&self.bundle)
    }
}

fn request(selection: SourceSelection) -> TypeshedRequest {
    TypeshedRequest {
        selection,
        store_path: None,
    }
}

fn bundle() -> Snapshot {
    let commit = Oid::from_hex(BUNDLE_SHA).expect("valid bundle oid");
    fixture_snapshot(SourceIdentity::Bundled { commit }, SourceKind::Bundled)
}

fn stored(commit: Oid, explicit: bool) -> Snapshot {
    let identity = SourceIdentity::Commit {
        commit,
        pinned: explicit,
    };
    fixture_snapshot(identity, SourceKind::ExactCommit)
}

fn fixture_snapshot(identity: SourceIdentity, active_source: SourceKind) -> Snapshot {
    let commit = identity.commit();
    let archive = Archive::new(vec![
        ArchiveEntry {
            path: "stdlib/VERSIONS".to_owned(),
            mode: FileMode::Regular,
            data: b"os: 3.0-\n".to_vec().into(),
        },
        ArchiveEntry {
            path: "stdlib/os.pyi".to_owned(),
            mode: FileMode::Regular,
            data: b"name: str\n".to_vec().into(),
        },
    ]);
    let status = super::super::source::TypeshedStatus {
        active_source,
        commit,
        tree: commit,
        license_status: LicenseStatus::Approved,
        license_reference: commit
            .map(|oid| format!("https://github.com/python/typeshed/blob/{oid}/LICENSE")),
        warnings: Vec::new(),
    };
    let uri_identity = identity.uri_component();
    Snapshot::build(
        identity,
        status,
        ArchiveVfs::new(uri_identity, archive),
        None,
    )
    .expect("valid fixture snapshot")
}

#[test]
fn custom_failure_never_consults_another_source() {
    let backend = FakeBackend {
        custom: Mutex::new(Some(Err(BackendError::Custom))),
        bundle: Mutex::new(Some(Ok(bundle()))),
        ..FakeBackend::default()
    };
    let result = select_snapshot(
        &request(SourceSelection::Custom {
            path: "/tmp/custom".to_owned(),
        }),
        &backend,
    );
    assert_eq!(
        result.err(),
        Some(SelectionError::Custom(BackendError::Custom))
    );
    assert_eq!(
        backend.calls.lock().ok().map(|calls| calls.clone()),
        Some(vec!["custom"])
    );
}

/// Implements [STUBRES-TYPESHED]: a pin naming the bundled commit is already
/// complete inside the binary — content-addressed identity makes the embedded
/// bytes exact — so selection activates the bundle without touching the store,
/// and an explicit pin of it suppresses `typeshed_source_unpinned`.
#[test]
fn exact_pin_of_the_bundled_commit_is_served_from_the_binary() {
    let matching = bundle();
    let commit = matching.identity.commit().expect("bundle commit");
    let backend = FakeBackend {
        bundle: Mutex::new(Some(Ok(matching))),
        ..FakeBackend::default()
    };
    let selected = select_snapshot(
        &request(SourceSelection::Pinned {
            commit,
            explicit: true,
        }),
        &backend,
    )
    .expect("embedded bundle satisfies its own pinned commit");
    assert_eq!(selected.status.active_source, SourceKind::Bundled);
    assert_eq!(selected.status.commit, Some(commit));
    assert!(selected.status.warnings.is_empty());
    assert_eq!(
        backend.calls.lock().ok().map(|calls| calls.clone()),
        Some(vec!["bundle"])
    );
}

/// Implements [STUBRES-TYPESHED-WARN]: the bundled default (no explicit
/// `typeshed-commit`) serves the same bytes but stays `typeshed_source_unpinned` — a build-time
/// pin is not a user pin.
#[test]
fn the_bundled_default_reports_unpinned() {
    let matching = bundle();
    let commit = matching.identity.commit().expect("bundle commit");
    let backend = FakeBackend {
        bundle: Mutex::new(Some(Ok(matching))),
        ..FakeBackend::default()
    };
    let selected = select_snapshot(
        &request(SourceSelection::Pinned {
            commit,
            explicit: false,
        }),
        &backend,
    )
    .expect("bundled default");
    let codes: Vec<&str> = selected
        .status
        .warnings
        .iter()
        .map(|warning| warning.code.as_str())
        .collect();
    assert_eq!(codes, vec!["typeshed_source_unpinned"]);
}

/// Implements [STUBRES-TYPESHED-OFFLINE]: a pin that is not on this machine is
/// terminal NO SOURCE — no bundle substitution, no network, no degraded mode.
#[test]
fn a_missing_pin_fails_hard_with_the_no_source_message() {
    let commit = Oid::from_hex(OTHER_SHA).expect("valid test oid");
    let backend = FakeBackend {
        pinned: Mutex::new(Some(Err(BackendError::Missing))),
        bundle: Mutex::new(Some(Ok(bundle()))),
        ..FakeBackend::default()
    };
    let error = select_snapshot(
        &request(SourceSelection::Pinned {
            commit,
            explicit: true,
        }),
        &backend,
    )
    .expect_err("missing pin must fail");
    assert_eq!(
        error,
        SelectionError::NoSource {
            commit,
            reason: BackendError::Missing,
        }
    );
    // The message is the spec's NO SOURCE status line verbatim, naming the fix.
    let message = error.to_string();
    assert_eq!(
        message,
        format!(
            "NO SOURCE — {OTHER_SHA} is not on this machine; run Download latest or basilisk typeshed download --commit {OTHER_SHA}"
        )
    );
    // The bundle is a different commit and must never be consulted.
    assert_eq!(
        backend.calls.lock().ok().map(|calls| calls.clone()),
        Some(vec!["pinned"])
    );
}

/// A corrupt store entry (failed offline verification) is the same terminal
/// failure with its reason preserved for status classification.
#[test]
fn a_corrupt_store_entry_fails_hard_and_keeps_its_reason() {
    let commit = Oid::from_hex(OTHER_SHA).expect("valid test oid");
    for reason in [BackendError::Corrupt, BackendError::LicenseChanged] {
        let backend = FakeBackend {
            pinned: Mutex::new(Some(Err(reason))),
            ..FakeBackend::default()
        };
        assert_eq!(
            select_snapshot(
                &request(SourceSelection::Pinned {
                    commit,
                    explicit: true,
                }),
                &backend,
            )
            .err(),
            Some(SelectionError::NoSource { commit, reason })
        );
    }
}

#[test]
fn a_verified_store_entry_activates_for_an_explicit_pin() {
    let commit = Oid::from_hex(OTHER_SHA).expect("valid test oid");
    let backend = FakeBackend {
        pinned: Mutex::new(Some(Ok(stored(commit, true)))),
        ..FakeBackend::default()
    };
    let selected = select_snapshot(
        &request(SourceSelection::Pinned {
            commit,
            explicit: true,
        }),
        &backend,
    )
    .expect("verified store entry");
    assert_eq!(selected.status.active_source, SourceKind::ExactCommit);
    assert_eq!(selected.status.commit, Some(commit));
    assert!(selected.status.warnings.is_empty());
}

/// The backend must return exactly the requested identity; anything else is a
/// wiring bug and fails closed rather than mislabeling the active source.
#[test]
fn an_inconsistent_backend_identity_fails_closed() {
    let commit = Oid::from_hex(OTHER_SHA).expect("valid test oid");
    let other = Oid::from_hex(BUNDLE_SHA).expect("valid bundle oid");
    let backend = FakeBackend {
        pinned: Mutex::new(Some(Ok(stored(other, true)))),
        ..FakeBackend::default()
    };
    assert_eq!(
        select_snapshot(
            &request(SourceSelection::Pinned {
                commit,
                explicit: true,
            }),
            &backend,
        )
        .err(),
        Some(SelectionError::InconsistentIdentity)
    );
}
