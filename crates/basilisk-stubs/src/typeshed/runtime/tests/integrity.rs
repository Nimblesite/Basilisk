use super::*;

fn approved_files() -> Vec<(String, Vec<u8>, FileMode, u32)> {
    let license = bundled_snapshot()
        .expect("bundle")
        .vfs
        .read("LICENSE")
        .expect("bundle license")
        .to_vec();
    vec![
        ("LICENSE".to_owned(), license, FileMode::Regular, 0o644),
        (
            "stdlib/VERSIONS".to_owned(),
            b"sentinel: 3.0-\n".to_vec(),
            FileMode::Regular,
            0o644,
        ),
        (
            "stdlib/sentinel.pyi".to_owned(),
            b"VALUE: str\n".to_vec(),
            FileMode::Regular,
            0o644,
        ),
    ]
}

#[test]
fn archive_encoding_is_not_the_tree_identity_and_pin_alone_is_not_attestation() {
    let stored = make_fixture_with_compression(A_SHA, approved_files(), CompressionMethod::Stored);
    let deflated =
        make_fixture_with_compression(A_SHA, approved_files(), CompressionMethod::Deflated);
    assert_ne!(stored.zip, deflated.zip, "ZIP encodings must differ");
    assert_eq!(stored.metadata.tree, deflated.metadata.tree);

    for fixture in [&stored, &deflated] {
        let transport = Arc::new(FakeTransport::new(
            None,
            std::slice::from_ref(fixture),
            SourceTransport::Codeload,
        ));
        let snapshot = manager(
            request(
                SourceSelection::ExactCommit {
                    commit: fixture.metadata.commit,
                },
                true,
            ),
            transport,
            None,
        )
        .snapshot()
        .expect("tree-attested archive");
        assert_eq!(snapshot.status.tree, Some(fixture.metadata.tree));
        assert_eq!(snapshot.status.provenance, Provenance::GithubTlsAttested);
        assert!(!snapshot.status.signed_release);
        assert_eq!(
            snapshot.read_stub("sentinel").map(|(_, body)| body),
            Some("VALUE: str\n")
        );
    }

    let mut pin_without_metadata = FakeTransport::new(
        None,
        std::slice::from_ref(&stored),
        SourceTransport::Codeload,
    );
    pin_without_metadata.commits.clear();
    pin_without_metadata.trees.clear();
    let transport = Arc::new(pin_without_metadata);
    let error = manager(
        request(
            SourceSelection::ExactCommit {
                commit: stored.metadata.commit,
            },
            true,
        ),
        Arc::clone(&transport),
        None,
    )
    .snapshot()
    .expect_err("a commit pin alone cannot authenticate an archive tree");
    assert!(matches!(
        error,
        super::super::super::selector::SelectionError::Exact {
            reason: BackendError::Metadata,
            ..
        }
    ));
    assert_eq!(transport.archive_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn root_and_nested_legal_drift_have_identical_exact_and_latest_policy() {
    let root_drift = fixture_with_license(
        A_SHA,
        "ROOT-DRIFT",
        b"changed root license identity\n".to_vec(),
    );
    let mut nested_files = approved_files();
    nested_files.push((
        "stdlib/NOTICE.runtime".to_owned(),
        b"new nested legal identity\n".to_vec(),
        FileMode::Regular,
        0o644,
    ));
    let nested_drift = make_fixture(A_SHA, nested_files);

    for drifted in [&root_drift, &nested_drift] {
        let exact = SourceSelection::ExactCommit {
            commit: drifted.metadata.commit,
        };
        for origin in [SourceTransport::Codeload, SourceTransport::Mirror] {
            let exact_transport = Arc::new(FakeTransport::new(
                None,
                std::slice::from_ref(drifted),
                origin,
            ));
            let exact_error = manager(request(exact.clone(), true), exact_transport, None)
                .snapshot()
                .expect_err("legal drift must fail an exact source");
            assert!(matches!(
                exact_error,
                super::super::super::selector::SelectionError::Exact {
                    reason: BackendError::LicenseChanged,
                    ..
                }
            ));

            let latest_transport = Arc::new(FakeTransport::new(
                Some(drifted.metadata.commit),
                std::slice::from_ref(drifted),
                origin,
            ));
            let fallback = manager(
                request(SourceSelection::Latest, true),
                latest_transport,
                None,
            )
            .snapshot()
            .expect("Latest may only fall back to the reviewed bundle");
            assert_eq!(fallback.status.active_source, SourceKind::Bundled);
            assert_eq!(
                fallback
                    .status
                    .warnings
                    .iter()
                    .map(|warning| warning.code.as_str())
                    .collect::<Vec<_>>(),
                vec!["UNPINNED", "DOWNLOAD FAILED", "LICENSE CHANGED"]
            );
        }
    }
}
