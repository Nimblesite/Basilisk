//! Tests [STUBRES-TYPESHED-DOWNLOAD] end to end, offline: a fake GitHub API
//! serving a synthetic-but-honest repository (real Git hashing, real zip
//! encoding, the real approved LICENSE bytes) drives the full pipeline into a
//! temp store, and `basilisk_stubs::typeshed::store::read_snapshot` — the
//! checker's own offline reader — verifies what was written. Every failure
//! phase must leave the store byte-for-byte empty (atomic download).

use std::cell::RefCell;

use basilisk_stubs::typeshed::gittree::{git_commit_oid, reconstruct_root_tree_oid, GitFile};
use basilisk_stubs::typeshed::store::read_snapshot;

use super::testing::{fake_repo, FakeApi, Faults};
use super::*;

fn store_entry_count(root: &std::path::Path) -> usize {
    std::fs::read_dir(root).map_or(0, Iterator::count)
}

#[expect(
    clippy::expect_used,
    reason = "test-only fixtures require infallible setup"
)]
mod cases {
    use super::*;

    /// The acceptance test: download latest, then read the entry back through
    /// the checker's own offline verifier — the exact code path analysis uses.
    #[test]
    fn download_latest_round_trips_through_the_offline_store_reader() {
        let root = tempfile::tempdir().expect("tempdir");
        let api = FakeApi::new(fake_repo());
        let phases = RefCell::new(Vec::new());
        let outcome = download_latest(Some(root.path().to_path_buf()), &api, &|phase| {
            phases.borrow_mut().push(phase);
        })
        .expect("download must succeed");
        assert_eq!(outcome.commit, api.repo.commit);
        assert_eq!(outcome.tree, api.repo.tree);
        assert_eq!(
            phases.into_inner(),
            vec![
                DownloadPhase::Resolving,
                DownloadPhase::FetchingTree,
                DownloadPhase::FetchingArchive,
                DownloadPhase::Verifying,
                DownloadPhase::Writing,
            ]
        );
        let snapshot =
            read_snapshot(root.path(), outcome.commit, true).expect("offline verification");
        assert_eq!(
            snapshot.read_stub("os").map(|(_, body)| body),
            Some("def getcwd() -> str: ...\n")
        );
        // Unmaterialised repo files participate in the pin, never in the VFS.
        assert!(snapshot.vfs.read("README.md").is_none());
    }

    /// `download_commit` is for materialising an existing pin; the resolved
    /// SHA must be the requested one, and any other answer is terminal.
    #[test]
    fn download_commit_verifies_the_resolved_sha_against_the_request() {
        let root = tempfile::tempdir().expect("tempdir");
        let api = FakeApi::new(fake_repo());
        let requested = api.repo.commit;
        let outcome = download_commit(
            requested,
            Some(root.path().to_path_buf()),
            &api,
            &|_phase| {},
        )
        .expect("the requested commit resolves to itself and materialises");
        assert_eq!(outcome.commit, requested);

        let other_root = tempfile::tempdir().expect("tempdir");
        let other = git_blob_oid(b"a different commit entirely");
        assert_eq!(
            download_commit(
                other,
                Some(other_root.path().to_path_buf()),
                &api,
                &|_phase| {}
            ),
            Err(DownloadError::Metadata)
        );
        assert_eq!(store_entry_count(other_root.path()), 0);
    }

    /// Every failure phase leaves the store empty — the atomic-download
    /// acceptance item ([STUBRES-TYPESHED-DOWNLOAD]).
    #[test]
    fn every_transport_failure_writes_nothing() {
        let cases = [
            (
                Faults {
                    resolve_fails: true,
                    ..Faults::default()
                },
                DownloadError::Metadata,
            ),
            (
                Faults {
                    tree_fails: true,
                    ..Faults::default()
                },
                DownloadError::Metadata,
            ),
            (
                Faults {
                    archive_fails: true,
                    ..Faults::default()
                },
                DownloadError::Download,
            ),
        ];
        for (faults, expected) in cases {
            let root = tempfile::tempdir().expect("tempdir");
            let mut api = FakeApi::new(fake_repo());
            api.faults = faults;
            assert_eq!(
                download_latest(Some(root.path().to_path_buf()), &api, &|_phase| {}),
                Err(expected)
            );
            assert_eq!(store_entry_count(root.path()), 0, "store must stay empty");
        }
    }

    /// A payload that does not hash to the reported SHA — a lying or tampered
    /// metadata response — is rejected before anything is fetched or written.
    #[test]
    fn a_commit_payload_that_misses_its_sha_is_rejected() {
        let root = tempfile::tempdir().expect("tempdir");
        let mut api = FakeApi::new(fake_repo());
        api.repo.payload = api.repo.payload.replace("fixture", "tampered");
        assert_eq!(
            download_latest(Some(root.path().to_path_buf()), &api, &|_phase| {}),
            Err(DownloadError::Validation)
        );
        assert_eq!(store_entry_count(root.path()), 0);
    }

    /// The API's tree SHA must agree with the tree named inside the verified
    /// commit object; a divergence means the tree listing cannot be trusted.
    #[test]
    fn a_tree_sha_disagreeing_with_the_commit_object_is_rejected() {
        let root = tempfile::tempdir().expect("tempdir");
        let mut api = FakeApi::new(fake_repo());
        api.repo.tree = git_blob_oid(b"some other tree");
        assert_eq!(
            download_latest(Some(root.path().to_path_buf()), &api, &|_phase| {}),
            Err(DownloadError::Validation)
        );
        assert_eq!(store_entry_count(root.path()), 0);
    }

    /// Archive bytes that differ from the trusted tree — one flipped stub —
    /// fail content binding even though the zip itself is well-formed.
    #[test]
    fn tampered_archive_bytes_fail_content_binding_and_write_nothing() {
        let root = tempfile::tempdir().expect("tempdir");
        let mut repo = fake_repo();
        for (path, data) in &mut repo.files {
            if path == "stdlib/os.pyi" {
                *data = b"def getcwd() -> bytes: ...\n".to_vec();
            }
        }
        // The tree entries still describe the ORIGINAL bytes.
        let honest = fake_repo();
        repo.tree_entries = honest.tree_entries;
        repo.tree = honest.tree;
        repo.payload = honest.payload;
        repo.commit = honest.commit;
        let api = FakeApi::new(repo);
        assert_eq!(
            download_latest(Some(root.path().to_path_buf()), &api, &|_phase| {}),
            Err(DownloadError::Validation)
        );
        assert_eq!(store_entry_count(root.path()), 0);
    }

    /// An archive with a file the tree does not list (or vice versa) fails the
    /// exact path-set binding.
    #[test]
    fn an_archive_file_missing_from_the_tree_is_rejected() {
        let root = tempfile::tempdir().expect("tempdir");
        let mut repo = fake_repo();
        repo.files.push(("smuggled.py".to_owned(), b"x\n".to_vec()));
        let api = FakeApi::new(repo);
        assert_eq!(
            download_latest(Some(root.path().to_path_buf()), &api, &|_phase| {}),
            Err(DownloadError::Validation)
        );
        assert_eq!(store_entry_count(root.path()), 0);
    }

    /// A commit whose legal identity drifted from the build-approved one is
    /// blocked as `LicenseChanged` — distinct from generic validation so the
    /// surface can say "pending legal review" ([STUBRES-TYPESHED-LICENSE]).
    #[test]
    fn license_drift_is_blocked_as_license_changed() {
        let root = tempfile::tempdir().expect("tempdir");
        let mut repo = fake_repo();
        for (path, data) in &mut repo.files {
            if path == "LICENSE" {
                *data = b"a different license\n".to_vec();
            }
        }
        // Rebuild an internally consistent identity around the drifted bytes.
        let tree_entries: Vec<TreeEntry> = repo
            .files
            .iter()
            .map(|(path, data)| TreeEntry {
                path: path.clone(),
                oid: git_blob_oid(data),
                mode: FileMode::Regular,
            })
            .collect();
        let git_files: Vec<GitFile> = tree_entries
            .iter()
            .map(|entry| GitFile {
                path: entry.path.clone(),
                oid: entry.oid,
                mode: entry.mode,
            })
            .collect();
        let tree = reconstruct_root_tree_oid(&git_files).expect("rebuilt tree");
        repo.payload =
            format!("tree {tree}\nauthor a <a@a> 0 +0000\ncommitter a <a@a> 0 +0000\n\nfixture\n");
        repo.commit = git_commit_oid(repo.payload.as_bytes());
        repo.tree = tree;
        repo.tree_entries = tree_entries;
        let api = FakeApi::new(repo);
        assert_eq!(
            download_latest(Some(root.path().to_path_buf()), &api, &|_phase| {}),
            Err(DownloadError::LicenseChanged)
        );
        assert_eq!(store_entry_count(root.path()), 0);
    }

    /// Re-downloading an existing commit is a no-op success: the store is
    /// content-addressed and immutable.
    #[test]
    fn redownloading_an_existing_commit_succeeds_without_rewriting() {
        let root = tempfile::tempdir().expect("tempdir");
        let api = FakeApi::new(fake_repo());
        let store = Some(root.path().to_path_buf());
        let first = download_latest(store.clone(), &api, &|_phase| {}).expect("first download");
        let second = download_latest(store, &api, &|_phase| {}).expect("second download");
        assert_eq!(first, second);
        assert_eq!(store_entry_count(root.path()), 1);
    }
}
