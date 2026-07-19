//! Black-box acceptance coverage for runtime Typeshed source acquisition.
//!
//! Implements the parent [TYPESHEDRT-MODEL], [TYPESHEDRT-WORK], and
//! [TYPESHEDRT-ACCEPTANCE] contracts through the following independent
//! [TYPESHEDRT-ACCEPTANCE-SOURCE],
//! [TYPESHEDRT-ACCEPTANCE-VERIFY], and
//! [TYPESHEDRT-ACCEPTANCE-OVERRIDES].

#![expect(
    clippy::expect_used,
    reason = "test-only archive and temporary-directory fixtures fail loudly"
)]

#[path = "support/typeshed_acquisition.rs"]
mod support;

use std::sync::Arc;

use basilisk_stubs::typeshed::bundle::{bundled_commit_sha, bundled_snapshot};
use basilisk_stubs::typeshed::cache::DiskCache;
use basilisk_stubs::typeshed::selector::{BackendError, SelectionError};
use basilisk_stubs::typeshed::source::{
    Provenance, SourceIdentity, SourceKind, SourceSelection, Transport as SourceTransport,
};
use basilisk_stubs::typeshed::transport::{HttpsTransport, TransportError};
use support::{
    fixture, manager, oid, request, untrusted_fixture, Operation, RecordingTransport, A_SHA, B_SHA,
};

#[test]
fn production_acquisition_is_https_only_and_has_no_process_transport() {
    assert!(HttpsTransport::new(Some("https://mirror.example/{sha}.zip".to_owned())).is_ok());
    assert_eq!(
        HttpsTransport::new(Some("http://mirror.example/{sha}.zip".to_owned())).err(),
        Some(TransportError::InvalidMirror)
    );
    let runtime_source = include_str!("../src/typeshed/runtime.rs");
    let http_source = include_str!("../src/typeshed/transport/http.rs");
    for source in [runtime_source, http_source] {
        assert!(!source.contains("std::process::Command"));
        assert!(!source.contains("Command::new"));
        assert!(!source.contains("git clone"));
        assert!(!source.contains("git://"));
    }

    let a = fixture(A_SHA, "A");
    let transport = Arc::new(RecordingTransport::new(
        Some(a.metadata.commit),
        std::slice::from_ref(&a),
        SourceTransport::Codeload,
    ));
    let snapshot = manager(
        request(SourceSelection::Latest, true, false),
        Arc::clone(&transport),
        None,
    )
    .snapshot()
    .expect("HTTPS-seam acquisition");
    assert_eq!(snapshot.status.transport, SourceTransport::Codeload);
    assert_eq!(
        transport.operations(),
        vec![
            Operation::ResolveLatest,
            Operation::FetchTree(a.metadata.tree),
            Operation::FetchArchive(a.metadata.commit),
        ]
    );
}

#[test]
fn one_manager_exposes_only_the_selected_b_generation() {
    let a = fixture(A_SHA, "A");
    let b = fixture(B_SHA, "B");
    let transport = Arc::new(RecordingTransport::new(
        Some(b.metadata.commit),
        &[a, b.clone()],
        SourceTransport::Codeload,
    ));
    let acquisition = manager(
        request(SourceSelection::Latest, true, false),
        Arc::clone(&transport),
        None,
    );
    let first = acquisition.snapshot().expect("B snapshot");
    let second = acquisition.snapshot().expect("same B snapshot");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.identity.commit(), Some(b.metadata.commit));
    assert!(first
        .versions()
        .is_some_and(|versions| versions.contains("b_only")));
    assert!(first.module_index.path("b_only").is_some());
    assert!(first.module_index.path("a_only").is_none());
    assert_eq!(
        first.read_stub("os").map(|(_, body)| body),
        Some("GENERATION: str = \"B\"\n")
    );
    assert_eq!(
        first.distribution_index.distribution("b_demo"),
        Some("types-b-demo")
    );
    assert_eq!(transport.operations().len(), 3);
}

#[test]
fn failed_latest_never_uses_cached_a_and_falls_back_to_bundle() {
    let cache_dir = tempfile::tempdir().expect("cache directory");
    let cache = DiskCache::new(cache_dir.path());
    let a = fixture(A_SHA, "A");
    let seed = Arc::new(RecordingTransport::new(
        Some(a.metadata.commit),
        std::slice::from_ref(&a),
        SourceTransport::Codeload,
    ));
    let _ = manager(
        request(SourceSelection::Latest, true, true),
        seed,
        Some(cache.clone()),
    )
    .snapshot()
    .expect("seed A cache");

    let unavailable = Arc::new(RecordingTransport::new(None, &[], SourceTransport::Mirror));
    let fallback = manager(
        request(SourceSelection::Latest, true, true),
        Arc::clone(&unavailable),
        Some(cache),
    )
    .snapshot()
    .expect("bundled fallback");
    assert_eq!(fallback.status.active_source, SourceKind::Bundled);
    assert_eq!(fallback.status.commit, Some(oid(bundled_commit_sha())));
    assert_ne!(fallback.status.commit, Some(a.metadata.commit));
    assert!(fallback.module_index.path("a_only").is_none());
    assert_eq!(unavailable.operations(), vec![Operation::ResolveLatest]);
    assert_eq!(
        fallback
            .status
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect::<Vec<_>>(),
        vec!["UNPINNED", "DOWNLOAD FAILED"]
    );
}

#[test]
fn verification_off_waives_only_tree_binding() {
    let approved_license = bundled_snapshot()
        .expect("bundle")
        .vfs
        .read("LICENSE")
        .expect("approved license")
        .to_vec();
    let cases = [
        untrusted_fixture(
            A_SHA,
            &[
                ("../escape.pyi".to_owned(), b"VALUE: int\n".to_vec()),
                ("LICENSE".to_owned(), approved_license.clone()),
                ("stdlib/VERSIONS".to_owned(), b"os: 3.0-\n".to_vec()),
                ("stdlib/os.pyi".to_owned(), b"VALUE: int\n".to_vec()),
            ],
        ),
        untrusted_fixture(
            A_SHA,
            &[
                ("LICENSE".to_owned(), approved_license),
                ("stdlib/os.pyi".to_owned(), b"VALUE: int\n".to_vec()),
            ],
        ),
        untrusted_fixture(
            A_SHA,
            &[
                ("LICENSE".to_owned(), b"changed license\n".to_vec()),
                ("stdlib/VERSIONS".to_owned(), b"os: 3.0-\n".to_vec()),
                ("stdlib/os.pyi".to_owned(), b"VALUE: int\n".to_vec()),
            ],
        ),
    ];
    for (index, fixture) in cases.into_iter().enumerate() {
        let transport = Arc::new(RecordingTransport::new(
            None,
            std::slice::from_ref(&fixture),
            SourceTransport::Codeload,
        ));
        let result = manager(
            request(
                SourceSelection::ExactCommit {
                    commit: fixture.metadata.commit,
                },
                false,
                false,
            ),
            Arc::clone(&transport),
            None,
        )
        .snapshot();
        let expected_reason = if index == 2 {
            BackendError::LicenseChanged
        } else {
            BackendError::Validation
        };
        assert!(matches!(
            result,
            Err(SelectionError::Exact { reason, .. }) if reason == expected_reason
        ));
        assert!(!transport
            .operations()
            .iter()
            .any(|operation| matches!(operation, Operation::FetchTree(_))));
    }

    let valid = fixture(A_SHA, "A");
    let transport = Arc::new(RecordingTransport::new(
        None,
        std::slice::from_ref(&valid),
        SourceTransport::Codeload,
    ));
    let snapshot = manager(
        request(
            SourceSelection::ExactCommit {
                commit: valid.metadata.commit,
            },
            false,
            false,
        ),
        Arc::clone(&transport),
        None,
    )
    .snapshot()
    .expect("unverified but otherwise gated snapshot");
    assert_eq!(snapshot.status.provenance, Provenance::Unverified);
    assert!(snapshot.status.tree.is_none());
    assert_eq!(
        snapshot
            .status
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect::<Vec<_>>(),
        vec!["UNVERIFIED"]
    );
    assert!(!transport
        .operations()
        .iter()
        .any(|operation| matches!(operation, Operation::FetchTree(_))));
}

#[test]
fn mirror_fetches_known_sha_and_exact_a_ignores_moved_main_b() {
    let a = fixture(A_SHA, "A");
    let b = fixture(B_SHA, "B");
    let transport = Arc::new(RecordingTransport::new(
        Some(b.metadata.commit),
        &[a.clone(), b],
        SourceTransport::Mirror,
    ));
    let snapshot = manager(
        request(
            SourceSelection::ExactCommit {
                commit: a.metadata.commit,
            },
            true,
            false,
        ),
        Arc::clone(&transport),
        None,
    )
    .snapshot()
    .expect("exact mirror snapshot");
    assert_eq!(snapshot.status.transport, SourceTransport::Mirror);
    assert_eq!(snapshot.status.commit, Some(a.metadata.commit));
    assert_eq!(
        snapshot.read_stub("os").map(|(_, body)| body),
        Some("GENERATION: str = \"A\"\n")
    );
    assert_eq!(
        transport.operations(),
        vec![
            Operation::ResolveCommit(a.metadata.commit),
            Operation::FetchTree(a.metadata.tree),
            Operation::FetchArchive(a.metadata.commit),
        ]
    );

    let unavailable = Arc::new(RecordingTransport::new(
        Some(oid(B_SHA)),
        &[fixture(B_SHA, "B")],
        SourceTransport::Mirror,
    ));
    let error = manager(
        request(
            SourceSelection::ExactCommit {
                commit: a.metadata.commit,
            },
            true,
            false,
        ),
        Arc::clone(&unavailable),
        None,
    )
    .snapshot()
    .expect_err("unavailable A cannot substitute B or a different bundle SHA");
    assert!(matches!(
        error,
        SelectionError::Exact { commit, .. } if commit == a.metadata.commit
    ));
    assert_eq!(
        unavailable.operations(),
        vec![Operation::ResolveCommit(a.metadata.commit)]
    );
}

#[test]
fn custom_tree_is_canonical_and_never_rescued_by_download_or_bundle() {
    let custom = tempfile::tempdir().expect("custom root");
    let stdlib = custom.path().join("stdlib");
    std::fs::create_dir(&stdlib).expect("stdlib");
    std::fs::write(stdlib.join("VERSIONS"), "os: 3.0-\ncustom_only: 3.0-\n").expect("VERSIONS");
    std::fs::write(stdlib.join("os.pyi"), "GENERATION: str = \"CUSTOM\"\n").expect("custom os");
    std::fs::write(stdlib.join("custom_only.pyi"), "VALUE: int\n").expect("custom module");

    let remote = fixture(A_SHA, "A");
    let transport = Arc::new(RecordingTransport::new(
        Some(remote.metadata.commit),
        &[remote],
        SourceTransport::Codeload,
    ));
    let snapshot = manager(
        request(
            SourceSelection::Custom {
                path: custom.path().to_string_lossy().into_owned(),
            },
            true,
            true,
        ),
        Arc::clone(&transport),
        None,
    )
    .snapshot()
    .expect("custom snapshot");
    assert!(matches!(snapshot.identity, SourceIdentity::Custom { .. }));
    assert_eq!(snapshot.status.provenance, Provenance::UserManaged);
    assert_eq!(
        snapshot.status.license_status,
        basilisk_stubs::typeshed::source::LicenseStatus::NotSupplied
    );
    assert!(snapshot.status.license_reference.is_none());
    assert_eq!(
        snapshot.read_stub("os").map(|(_, body)| body),
        Some("GENERATION: str = \"CUSTOM\"\n")
    );
    assert!(snapshot.read_stub("a_only").is_none());
    assert!(transport.operations().is_empty());
    assert_eq!(
        snapshot
            .status
            .warnings
            .iter()
            .map(|warning| warning.code.as_str())
            .collect::<Vec<_>>(),
        vec!["UNPINNED", "USER-MANAGED SOURCE"]
    );
}

#[test]
fn custom_path_validation_is_typed_and_deterministic() {
    let transport = Arc::new(RecordingTransport::new(
        None,
        &[],
        SourceTransport::Codeload,
    ));
    let relative = manager(
        request(
            SourceSelection::Custom {
                path: "workspace-relative-typeshed".to_owned(),
            },
            true,
            false,
        ),
        Arc::clone(&transport),
        None,
    )
    .snapshot()
    .expect_err("runtime requires config-resolved absolute path");
    assert_eq!(
        relative,
        SelectionError::Custom(BackendError::InvalidConfiguration)
    );

    let absent = tempfile::tempdir().expect("parent").path().join("absent");
    let absolute_request = request(
        SourceSelection::Custom {
            path: absent.to_string_lossy().into_owned(),
        },
        true,
        false,
    );
    let first = manager(absolute_request.clone(), Arc::clone(&transport), None)
        .snapshot()
        .expect_err("nonexistent path");
    let second = manager(absolute_request, Arc::clone(&transport), None)
        .snapshot()
        .expect_err("same nonexistent path");
    assert_eq!(first, SelectionError::Custom(BackendError::Custom));
    assert_eq!(first, second);

    let malformed = tempfile::tempdir().expect("malformed root");
    std::fs::write(malformed.path().join("os.pyi"), "VALUE: int\n").expect("misplaced stub");
    let error = manager(
        request(
            SourceSelection::Custom {
                path: malformed.path().to_string_lossy().into_owned(),
            },
            true,
            false,
        ),
        transport,
        None,
    )
    .snapshot()
    .expect_err("required top-level stdlib directory");
    assert_eq!(error, SelectionError::Custom(BackendError::Custom));

    let malformed = tempfile::tempdir().expect("malformed stdlib root");
    let stdlib = malformed.path().join("stdlib");
    std::fs::create_dir(&stdlib).expect("stdlib");
    std::fs::write(stdlib.join("VERSIONS"), "this is not a version row\n").expect("VERSIONS");
    std::fs::write(stdlib.join("os.pyi"), "VALUE: int\n").expect("stub");
    let error = manager(
        request(
            SourceSelection::Custom {
                path: malformed.path().to_string_lossy().into_owned(),
            },
            true,
            false,
        ),
        Arc::new(RecordingTransport::new(
            None,
            &[],
            SourceTransport::Codeload,
        )),
        None,
    )
    .snapshot()
    .expect_err("malformed custom index");
    assert_eq!(error, SelectionError::Custom(BackendError::Custom));
}
