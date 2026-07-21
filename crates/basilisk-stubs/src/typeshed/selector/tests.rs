use std::sync::Mutex;

use super::super::archive::{Archive, ArchiveEntry, ArchiveVfs};
use super::super::gittree::FileMode;
use super::*;

const OTHER_SHA: &str = "0123456789012345678901234567890123456789";
const BUNDLE_SHA: &str = "83c2518a9e6abbda0c44592c3483de459198f887";

#[derive(Default)]
struct FakeBackend {
    custom: Mutex<Option<Result<Snapshot, BackendError>>>,
    commit: Mutex<Option<Result<Snapshot, BackendError>>>,
    latest: Mutex<Option<Result<Snapshot, BackendError>>>,
    bundle: Mutex<Option<Result<Snapshot, BackendError>>>,
    calls: Mutex<Vec<&'static str>>,
}

impl FakeBackend {
    fn take(
        slot: &Mutex<Option<Result<Snapshot, BackendError>>>,
    ) -> Result<Snapshot, BackendError> {
        let Ok(mut value) = slot.lock() else {
            return Err(BackendError::Validation);
        };
        value.take().unwrap_or(Err(BackendError::Validation))
    }

    fn record(&self, call: &'static str) {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(call);
        }
    }
}

impl AcquisitionBackend for FakeBackend {
    fn load_custom(&self, _path: &str) -> Result<Snapshot, BackendError> {
        self.record("custom");
        Self::take(&self.custom)
    }

    fn load_commit(
        &self,
        _commit: Oid,
        _request: &TypeshedRequest,
    ) -> Result<Snapshot, BackendError> {
        self.record("commit");
        Self::take(&self.commit)
    }

    fn load_latest(&self, _request: &TypeshedRequest) -> Result<Snapshot, BackendError> {
        self.record("latest");
        Self::take(&self.latest)
    }

    fn load_bundled(&self) -> Result<Snapshot, BackendError> {
        self.record("bundle");
        Self::take(&self.bundle)
    }
}

fn request(selection: SourceSelection) -> TypeshedRequest {
    TypeshedRequest {
        selection,
        verify_content: true,
        use_cache: true,
        url_template: None,
    }
}

fn bundle() -> Snapshot {
    let commit = Oid::from_hex(BUNDLE_SHA).expect("valid bundle oid");
    fixture_snapshot(
        SourceIdentity::Bundled { commit },
        SourceKind::Bundled,
        Provenance::BundleVetted,
    )
}

fn downloaded(commit: Oid) -> Snapshot {
    let identity = SourceIdentity::Commit {
        commit,
        pinned: false,
    };
    fixture_snapshot(identity, SourceKind::Latest, Provenance::GithubTlsAttested)
}

fn fixture_snapshot(
    identity: SourceIdentity,
    active_source: SourceKind,
    provenance: Provenance,
) -> Snapshot {
    let commit = identity.commit();
    let archive = Archive::new(vec![
        ArchiveEntry {
            path: "stdlib/VERSIONS".to_owned(),
            mode: FileMode::Regular,
            data: b"os: 3.0-\n".to_vec(),
        },
        ArchiveEntry {
            path: "stdlib/os.pyi".to_owned(),
            mode: FileMode::Regular,
            data: b"name: str\n".to_vec(),
        },
    ]);
    let status = super::super::source::TypeshedStatus {
        active_source,
        commit,
        tree: commit,
        transport: if active_source == SourceKind::Bundled {
            Transport::EmbeddedZip
        } else if active_source == SourceKind::Custom {
            Transport::CustomPath
        } else {
            Transport::Codeload
        },
        license_status: LicenseStatus::Approved,
        license_reference: commit
            .map(|oid| format!("https://github.com/python/typeshed/blob/{oid}/LICENSE")),
        provenance,
        signed_release: false,
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
fn custom_failure_never_consults_bundle() {
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

#[test]
fn exact_failure_accepts_only_equal_bundle_and_suppresses_unpinned() {
    let matching = bundle();
    let commit = matching.identity.commit().expect("bundle commit");
    let backend = FakeBackend {
        commit: Mutex::new(Some(Err(BackendError::Download))),
        bundle: Mutex::new(Some(Ok(matching))),
        ..FakeBackend::default()
    };
    let selected = select_snapshot(&request(SourceSelection::ExactCommit { commit }), &backend)
        .expect("matching bundle is eligible");
    assert_eq!(selected.status.active_source, SourceKind::Bundled);
    assert!(selected.status.warnings.is_empty());

    let other = Oid::from_hex(OTHER_SHA).expect("valid test oid");
    let mismatch = FakeBackend {
        commit: Mutex::new(Some(Err(BackendError::Download))),
        bundle: Mutex::new(Some(Ok(bundle()))),
        ..FakeBackend::default()
    };
    assert!(matches!(
        select_snapshot(&request(SourceSelection::ExactCommit { commit: other }), &mismatch),
        Err(SelectionError::Exact { commit, .. }) if commit == other
    ));
}

#[test]
fn exact_pin_of_the_bundled_commit_never_consults_the_network() {
    // Implements [STUBRES-TYPESHED-ACQUIRE]: a pin naming the bundled commit
    // is already complete inside the binary — content-addressed identity makes
    // the embedded bytes exact — so selection must activate the bundle without
    // consulting the network-backed commit loader. Reaching for rate-limited
    // metadata first is what let a 403 block a root whose pinned stdlib was
    // sitting embedded in the very binary that refused to activate it.
    let matching = bundle();
    let commit = matching.identity.commit().expect("bundle commit");
    let backend = FakeBackend {
        bundle: Mutex::new(Some(Ok(matching))),
        ..FakeBackend::default()
    };
    let selected = select_snapshot(&request(SourceSelection::ExactCommit { commit }), &backend)
        .expect("embedded bundle satisfies its own pinned commit offline");
    assert_eq!(selected.status.active_source, SourceKind::Bundled);
    assert_eq!(selected.status.commit, Some(commit));
    assert_eq!(selected.status.transport, Transport::EmbeddedZip);
    assert!(selected.status.warnings.is_empty());
    assert_eq!(
        backend.calls.lock().ok().map(|calls| calls.clone()),
        Some(vec!["bundle"])
    );
}

#[test]
fn exact_license_drift_survives_an_unavailable_bundle_fallback() {
    let commit = Oid::from_hex(OTHER_SHA).expect("valid test oid");
    let backend = FakeBackend {
        commit: Mutex::new(Some(Err(BackendError::LicenseChanged))),
        bundle: Mutex::new(Some(Err(BackendError::Bundle))),
        ..FakeBackend::default()
    };
    assert_eq!(
        select_snapshot(&request(SourceSelection::ExactCommit { commit }), &backend).err(),
        Some(SelectionError::Exact {
            commit,
            reason: BackendError::LicenseChanged,
        })
    );
}

#[test]
fn latest_failure_uses_bundle_with_ordered_composable_warnings() {
    let backend = FakeBackend {
        latest: Mutex::new(Some(Err(BackendError::LicenseChanged))),
        bundle: Mutex::new(Some(Ok(bundle()))),
        ..FakeBackend::default()
    };
    let selected =
        select_snapshot(&request(SourceSelection::Latest), &backend).expect("bundle fallback");
    let codes: Vec<&str> = selected
        .status
        .warnings
        .iter()
        .map(|warning| warning.code.as_str())
        .collect();
    assert_eq!(
        codes,
        vec!["UNPINNED", "DOWNLOAD FAILED", "LICENSE CHANGED"]
    );
    assert_eq!(
        backend.calls.lock().ok().map(|calls| calls.clone()),
        Some(vec!["latest", "bundle"])
    );
}

#[test]
fn verification_off_waives_only_download_content_status() {
    let commit = Oid::from_hex(OTHER_SHA).expect("valid test oid");
    let backend = FakeBackend {
        latest: Mutex::new(Some(Ok(downloaded(commit)))),
        ..FakeBackend::default()
    };
    let mut request = request(SourceSelection::Latest);
    request.verify_content = false;
    let selected = select_snapshot(&request, &backend).expect("latest snapshot");
    let codes: Vec<&str> = selected
        .status
        .warnings
        .iter()
        .map(|warning| warning.code.as_str())
        .collect();
    assert_eq!(codes, vec!["UNPINNED", "UNVERIFIED"]);
    assert_eq!(selected.status.provenance, Provenance::Unverified);
    assert!(selected.status.tree.is_none());
}
