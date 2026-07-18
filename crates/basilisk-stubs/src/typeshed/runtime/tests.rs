use std::collections::HashMap;
use std::io::{Cursor, Write as _};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

use super::super::bundle::{bundled_commit_sha, bundled_snapshot};
use super::super::gittree::{git_blob_oid, reconstruct_root_tree_oid, GitFile};
use super::super::source::{SourceIdentity, SourceSelection};
use super::*;

mod integrity;

const A_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[derive(Debug, Clone)]
struct Fixture {
    metadata: CommitMetadata,
    tree: Vec<TreeEntry>,
    zip: Vec<u8>,
}

#[derive(Debug)]
struct FakeTransport {
    latest: Option<CommitMetadata>,
    commits: HashMap<Oid, CommitMetadata>,
    trees: HashMap<Oid, Vec<TreeEntry>>,
    archives: HashMap<Oid, Vec<u8>>,
    origin: SourceTransport,
    latest_calls: AtomicUsize,
    commit_calls: AtomicUsize,
    tree_calls: AtomicUsize,
    archive_calls: AtomicUsize,
    fetched: Mutex<Vec<Oid>>,
}

impl FakeTransport {
    fn new(latest: Option<Oid>, fixtures: &[Fixture], origin: SourceTransport) -> Self {
        let commits = fixtures
            .iter()
            .map(|fixture| (fixture.metadata.commit, fixture.metadata.clone()))
            .collect();
        let trees = fixtures
            .iter()
            .map(|fixture| (fixture.metadata.tree, fixture.tree.clone()))
            .collect();
        let archives = fixtures
            .iter()
            .map(|fixture| (fixture.metadata.commit, fixture.zip.clone()))
            .collect();
        let latest = latest.and_then(|commit| {
            fixtures
                .iter()
                .find(|fixture| fixture.metadata.commit == commit)
                .map(|fixture| fixture.metadata.clone())
        });
        Self {
            latest,
            commits,
            trees,
            archives,
            origin,
            latest_calls: AtomicUsize::new(0),
            commit_calls: AtomicUsize::new(0),
            tree_calls: AtomicUsize::new(0),
            archive_calls: AtomicUsize::new(0),
            fetched: Mutex::new(Vec::new()),
        }
    }

    fn offline() -> Self {
        Self::new(None, &[], SourceTransport::Codeload)
    }
}

impl Transport for FakeTransport {
    fn resolve_latest(&self) -> Result<CommitMetadata, TransportError> {
        let _ = self.latest_calls.fetch_add(1, Ordering::SeqCst);
        self.latest.clone().ok_or(TransportError::Metadata)
    }

    fn resolve_commit(&self, commit: Oid) -> Result<CommitMetadata, TransportError> {
        let _ = self.commit_calls.fetch_add(1, Ordering::SeqCst);
        self.commits
            .get(&commit)
            .cloned()
            .ok_or(TransportError::Metadata)
    }

    fn fetch_tree(&self, root_tree: Oid) -> Result<Vec<TreeEntry>, TransportError> {
        let _ = self.tree_calls.fetch_add(1, Ordering::SeqCst);
        self.trees
            .get(&root_tree)
            .cloned()
            .ok_or(TransportError::Metadata)
    }

    fn fetch_archive(&self, commit: Oid) -> Result<Vec<u8>, TransportError> {
        let _ = self.archive_calls.fetch_add(1, Ordering::SeqCst);
        self.fetched.lock().expect("fetched lock").push(commit);
        self.archives
            .get(&commit)
            .cloned()
            .ok_or(TransportError::Download)
    }

    fn archive_transport(&self) -> SourceTransport {
        self.origin
    }
}

fn oid(value: &str) -> Oid {
    Oid::from_hex(value).expect("fixture oid")
}

fn request(selection: SourceSelection, verify_content: bool) -> TypeshedRequest {
    TypeshedRequest {
        selection,
        verify_content,
        use_cache: true,
        url_template: None,
    }
}

fn fixture(commit: &str, marker: &str) -> Fixture {
    let license = bundled_snapshot()
        .expect("bundle")
        .vfs
        .read("LICENSE")
        .expect("bundle license")
        .to_vec();
    fixture_with_license(commit, marker, license)
}

fn fixture_with_license(commit: &str, marker: &str, license: Vec<u8>) -> Fixture {
    let lowercase = marker.to_ascii_lowercase();
    let files = vec![
        ("LICENSE".to_owned(), license, FileMode::Regular, 0o644),
        (
            "stdlib/VERSIONS".to_owned(),
            format!("sentinel: 3.0-\n# {marker}\n").into_bytes(),
            FileMode::Regular,
            0o644,
        ),
        (
            "stdlib/sentinel.pyi".to_owned(),
            format!("VALUE: str  # {marker}\n").into_bytes(),
            FileMode::Regular,
            0o644,
        ),
        (
            format!("stdlib/{lowercase}_only.pyi"),
            b"ONLY: int\n".to_vec(),
            FileMode::Regular,
            0o644,
        ),
        (
            format!("stubs/{lowercase}_demo/demo.pyi"),
            b"VALUE: int\n".to_vec(),
            FileMode::Regular,
            0o644,
        ),
    ];
    make_fixture(commit, files)
}

fn make_fixture(commit: &str, files: Vec<(String, Vec<u8>, FileMode, u32)>) -> Fixture {
    make_fixture_with_compression(commit, files, CompressionMethod::Stored)
}

fn make_fixture_with_compression(
    commit: &str,
    files: Vec<(String, Vec<u8>, FileMode, u32)>,
    compression: CompressionMethod,
) -> Fixture {
    let tree: Vec<_> = files
        .iter()
        .map(|(path, data, mode, _zip_mode)| TreeEntry {
            path: path.clone(),
            oid: git_blob_oid(data),
            mode: *mode,
        })
        .collect();
    let git_files: Vec<_> = tree
        .iter()
        .map(|entry| GitFile {
            path: entry.path.clone(),
            oid: entry.oid,
            mode: entry.mode,
        })
        .collect();
    let root = reconstruct_root_tree_oid(&git_files).expect("fixture root");
    let mut zip = Vec::new();
    {
        let mut writer = ZipWriter::new(Cursor::new(&mut zip));
        for (path, data, _trusted_mode, zip_mode) in files {
            let options = SimpleFileOptions::default()
                .compression_method(compression)
                .unix_permissions(zip_mode);
            writer
                .start_file(format!("typeshed-{commit}/{path}"), options)
                .expect("zip entry");
            writer.write_all(&data).expect("zip bytes");
        }
        let _ = writer.finish().expect("zip finish");
    }
    Fixture {
        metadata: CommitMetadata {
            commit: oid(commit),
            tree: root,
        },
        tree,
        zip,
    }
}

fn manager(
    request: TypeshedRequest,
    fake: Arc<FakeTransport>,
    cache: Option<DiskCache>,
) -> TypeshedManager {
    let transport: Arc<dyn Transport> = fake;
    manager_for_request(request, transport, cache)
}

#[test]
fn latest_resolves_b_before_cache_and_all_views_come_from_b() {
    let cache_dir = tempfile::tempdir().expect("cache dir");
    let cache = DiskCache::new(cache_dir.path());
    let a = fixture(A_SHA, "A");
    let seed = Arc::new(FakeTransport::new(
        Some(a.metadata.commit),
        std::slice::from_ref(&a),
        SourceTransport::Codeload,
    ));
    let _ = manager(
        request(SourceSelection::Latest, true),
        seed,
        Some(cache.clone()),
    )
    .snapshot()
    .expect("seed A");

    let b = fixture(B_SHA, "B");
    let remote = Arc::new(FakeTransport::new(
        Some(b.metadata.commit),
        std::slice::from_ref(&b),
        SourceTransport::Codeload,
    ));
    let snapshot = manager(
        request(SourceSelection::Latest, true),
        Arc::clone(&remote),
        Some(cache),
    )
    .snapshot()
    .expect("activate B");
    assert_eq!(snapshot.status.commit, Some(b.metadata.commit));
    assert!(snapshot.versions().is_some_and(|text| text.contains("# B")));
    assert!(snapshot.module_index.path("b_only").is_some());
    assert!(snapshot.module_index.path("a_only").is_none());
    assert_eq!(
        snapshot.read_stub("sentinel").map(|(_, body)| body),
        Some("VALUE: str  # B\n")
    );
    assert_eq!(
        snapshot.distribution_index.distribution("demo"),
        Some("types-b_demo")
    );
    assert_eq!(remote.latest_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        *remote.fetched.lock().expect("fetched"),
        vec![b.metadata.commit]
    );
}

#[test]
fn exact_pin_reuses_cache_offline_and_remains_pinned() {
    let cache_dir = tempfile::tempdir().expect("cache dir");
    let cache = DiskCache::new(cache_dir.path());
    let a = fixture(A_SHA, "A");
    let online = Arc::new(FakeTransport::new(
        None,
        std::slice::from_ref(&a),
        SourceTransport::Codeload,
    ));
    let exact = SourceSelection::ExactCommit {
        commit: a.metadata.commit,
    };
    let _ = manager(
        request(exact.clone(), true),
        Arc::clone(&online),
        Some(cache.clone()),
    )
    .snapshot()
    .expect("online pin");
    let offline = Arc::new(FakeTransport::offline());
    let snapshot = manager(request(exact, true), Arc::clone(&offline), Some(cache))
        .snapshot()
        .expect("offline cached pin");
    assert!(matches!(
        snapshot.identity,
        SourceIdentity::Commit { pinned: true, .. }
    ));
    assert_eq!(offline.commit_calls.load(Ordering::SeqCst), 0);
    assert_eq!(offline.tree_calls.load(Ordering::SeqCst), 0);
    assert_eq!(offline.archive_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn exact_bundle_commit_restarts_offline_without_a_download_cache() {
    let commit = oid(bundled_commit_sha());
    let offline = Arc::new(FakeTransport::offline());
    let snapshot = manager(
        request(SourceSelection::ExactCommit { commit }, true),
        Arc::clone(&offline),
        None,
    )
    .snapshot()
    .expect("the embedded ZIP is the exact pinned source after restart");
    assert_eq!(snapshot.status.active_source, SourceKind::Bundled);
    assert_eq!(snapshot.status.commit, Some(commit));
    assert!(snapshot.status.warnings.is_empty());
    assert_eq!(offline.commit_calls.load(Ordering::SeqCst), 1);
    assert_eq!(offline.archive_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn cache_off_writes_nothing_and_eviction_redownloads_the_same_pin() {
    let cache_dir = tempfile::tempdir().expect("cache dir");
    let cache = DiskCache::new(cache_dir.path());
    let a = fixture(A_SHA, "A");
    let exact = SourceSelection::ExactCommit {
        commit: a.metadata.commit,
    };

    let mut no_cache_request = request(exact.clone(), true);
    no_cache_request.use_cache = false;
    let no_cache_transport = Arc::new(FakeTransport::new(
        None,
        std::slice::from_ref(&a),
        SourceTransport::Codeload,
    ));
    let _ = manager(no_cache_request, no_cache_transport, Some(cache.clone()))
        .snapshot()
        .expect("cache-off acquisition");
    assert_eq!(
        std::fs::read_dir(cache_dir.path())
            .expect("cache root")
            .count(),
        0,
        "cache-off must validate and discard without a generation directory"
    );

    let seed = Arc::new(FakeTransport::new(
        None,
        std::slice::from_ref(&a),
        SourceTransport::Codeload,
    ));
    let _ = manager(request(exact.clone(), true), seed, Some(cache.clone()))
        .snapshot()
        .expect("seed exact cache");
    std::fs::remove_dir_all(cache_dir.path().join(A_SHA)).expect("explicit cache eviction");

    let retry = Arc::new(FakeTransport::new(
        None,
        std::slice::from_ref(&a),
        SourceTransport::Codeload,
    ));
    let snapshot = manager(request(exact, true), Arc::clone(&retry), Some(cache))
        .snapshot()
        .expect("same pin reacquired");
    assert_eq!(snapshot.status.commit, Some(a.metadata.commit));
    assert_eq!(retry.commit_calls.load(Ordering::SeqCst), 1);
    assert_eq!(retry.archive_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn license_drift_blocks_exact_and_mirror_and_latest_falls_back_loudly() {
    let drifted = fixture_with_license(A_SHA, "DRIFT", b"changed license identity\n".to_vec());
    let exact = SourceSelection::ExactCommit {
        commit: drifted.metadata.commit,
    };

    for origin in [SourceTransport::Codeload, SourceTransport::Mirror] {
        let transport = Arc::new(FakeTransport::new(
            None,
            std::slice::from_ref(&drifted),
            origin,
        ));
        let error = manager(request(exact.clone(), true), transport, None)
            .snapshot()
            .expect_err("license drift must fail an exact source");
        assert!(matches!(
            error,
            super::super::selector::SelectionError::Exact {
                reason: BackendError::LicenseChanged,
                ..
            }
        ));
    }

    let latest_transport = Arc::new(FakeTransport::new(
        Some(drifted.metadata.commit),
        std::slice::from_ref(&drifted),
        SourceTransport::Codeload,
    ));
    let fallback = manager(
        request(SourceSelection::Latest, true),
        latest_transport,
        None,
    )
    .snapshot()
    .expect("Latest may use only the vetted bundle after drift");
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

#[test]
fn cache_mutation_is_rejected_and_reacquired() {
    let cache_dir = tempfile::tempdir().expect("cache dir");
    let cache = DiskCache::new(cache_dir.path());
    let a = fixture(A_SHA, "A");
    let online = Arc::new(FakeTransport::new(
        None,
        std::slice::from_ref(&a),
        SourceTransport::Codeload,
    ));
    let exact = SourceSelection::ExactCommit {
        commit: a.metadata.commit,
    };
    let _ = manager(request(exact.clone(), true), online, Some(cache.clone()))
        .snapshot()
        .expect("seed cache");
    let cached_zip = cache_dir
        .path()
        .join(A_SHA)
        .join("generations")
        .join(sha256_hex(&a.zip))
        .join("archive.zip");
    std::fs::write(cached_zip, b"mutated").expect("mutate cache");
    let retry = Arc::new(FakeTransport::new(
        None,
        std::slice::from_ref(&a),
        SourceTransport::Codeload,
    ));
    let transport: Arc<dyn Transport> = Arc::<FakeTransport>::clone(&retry);
    let backend = RuntimeBackend::new(transport, Some(cache));
    let snapshot = backend
        .load_commit(a.metadata.commit, &request(exact, true))
        .expect("reacquired valid archive");
    assert_eq!(
        snapshot.read_stub("sentinel").map(|(_, body)| body),
        Some("VALUE: str  # A\n")
    );
    assert_eq!(retry.archive_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn verification_on_rehashes_cached_archive_offline() {
    let cache_dir = tempfile::tempdir().expect("cache dir");
    let cache = DiskCache::new(cache_dir.path());
    let a = fixture(A_SHA, "A");
    let online = Arc::new(FakeTransport::new(
        None,
        std::slice::from_ref(&a),
        SourceTransport::Codeload,
    ));
    let exact = SourceSelection::ExactCommit {
        commit: a.metadata.commit,
    };
    let unverified = manager(request(exact.clone(), false), online, Some(cache.clone()))
        .snapshot()
        .expect("unverified");
    assert_eq!(unverified.status.provenance, Provenance::Unverified);
    assert!(unverified.status.tree.is_none());
    let offline = Arc::new(FakeTransport::offline());
    let verified_snapshot = manager(request(exact, true), Arc::clone(&offline), Some(cache))
        .snapshot()
        .expect("verify cached bytes");
    assert_eq!(
        verified_snapshot.status.provenance,
        Provenance::GithubTlsAttested
    );
    assert_eq!(verified_snapshot.status.tree, Some(a.metadata.tree));
    assert_eq!(offline.commit_calls.load(Ordering::SeqCst), 0);
    assert_eq!(offline.archive_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn cache_preserves_mirror_origin_across_configuration_changes() {
    let cache_dir = tempfile::tempdir().expect("cache dir");
    let cache = DiskCache::new(cache_dir.path());
    let a = fixture(A_SHA, "A");
    let mirror = Arc::new(FakeTransport::new(
        None,
        std::slice::from_ref(&a),
        SourceTransport::Mirror,
    ));
    let exact = SourceSelection::ExactCommit {
        commit: a.metadata.commit,
    };
    let _ = manager(request(exact.clone(), true), mirror, Some(cache.clone()))
        .snapshot()
        .expect("mirror");
    let authenticated = Arc::new(FakeTransport::new(
        None,
        std::slice::from_ref(&a),
        SourceTransport::Codeload,
    ));
    let reused = manager(request(exact, true), authenticated, Some(cache))
        .snapshot()
        .expect("cached mirror");
    assert_eq!(reused.status.transport, SourceTransport::Mirror);
}

#[test]
fn trusted_git_modes_override_zip_modes_and_blob_mutation_fails() {
    let license = bundled_snapshot()
        .expect("bundle")
        .vfs
        .read("LICENSE")
        .expect("license")
        .to_vec();
    let files = vec![
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
            FileMode::Executable,
            0o644,
        ),
    ];
    let valid = make_fixture(A_SHA, files);
    let fake = Arc::new(FakeTransport::new(
        None,
        std::slice::from_ref(&valid),
        SourceTransport::Codeload,
    ));
    let cache_dir = tempfile::tempdir().expect("cache dir");
    let cache = DiskCache::new(cache_dir.path());
    let cached_request = request(
        SourceSelection::ExactCommit {
            commit: valid.metadata.commit,
        },
        true,
    );
    let backend = RuntimeBackend::new(fake, Some(cache.clone()));
    assert!(backend
        .load_commit(valid.metadata.commit, &cached_request)
        .is_ok());
    let authenticated = RuntimeBackend::new(
        Arc::new(FakeTransport::new(
            None,
            std::slice::from_ref(&valid),
            SourceTransport::Codeload,
        )),
        Some(cache),
    );
    assert!(authenticated
        .load_commit(valid.metadata.commit, &cached_request)
        .is_ok());

    let mut no_cache = cached_request;
    no_cache.use_cache = false;

    let mut mutated = valid.clone();
    let needle = b"VALUE: str";
    let position = mutated
        .zip
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("stored body");
    let byte = mutated.zip.get_mut(position).expect("stored body position");
    *byte = b'X';
    let bad = RuntimeBackend::new(
        Arc::new(FakeTransport::new(
            None,
            &[mutated],
            SourceTransport::Codeload,
        )),
        None,
    );
    assert_eq!(
        bad.load_commit(valid.metadata.commit, &no_cache).err(),
        Some(BackendError::Validation)
    );
}
