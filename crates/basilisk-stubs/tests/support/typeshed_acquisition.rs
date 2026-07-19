use std::collections::HashMap;
use std::io::{Cursor, Write as _};
use std::sync::{Arc, Mutex};

use basilisk_stubs::typeshed::bundle::bundled_snapshot;
use basilisk_stubs::typeshed::cache::DiskCache;
use basilisk_stubs::typeshed::gittree::{
    git_blob_oid, reconstruct_root_tree_oid, FileMode, GitFile, Oid,
};
use basilisk_stubs::typeshed::runtime::manager_for_request;
use basilisk_stubs::typeshed::source::{
    SourceSelection, Transport as SourceTransport, TypeshedRequest,
};
use basilisk_stubs::typeshed::transport::{CommitMetadata, Transport, TransportError, TreeEntry};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

pub const A_SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const B_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[derive(Clone)]
pub struct Fixture {
    pub metadata: CommitMetadata,
    tree: Vec<TreeEntry>,
    zip: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    ResolveLatest,
    ResolveCommit(Oid),
    FetchTree(Oid),
    FetchArchive(Oid),
}

#[derive(Debug)]
pub struct RecordingTransport {
    latest: Option<CommitMetadata>,
    commits: HashMap<Oid, CommitMetadata>,
    trees: HashMap<Oid, Vec<TreeEntry>>,
    archives: HashMap<Oid, Vec<u8>>,
    origin: SourceTransport,
    operations: Mutex<Vec<Operation>>,
}

impl RecordingTransport {
    pub fn new(latest: Option<Oid>, fixtures: &[Fixture], origin: SourceTransport) -> Self {
        let latest = latest.and_then(|commit| {
            fixtures
                .iter()
                .find(|fixture| fixture.metadata.commit == commit)
                .map(|fixture| fixture.metadata.clone())
        });
        Self {
            latest,
            commits: fixtures
                .iter()
                .map(|fixture| (fixture.metadata.commit, fixture.metadata.clone()))
                .collect(),
            trees: fixtures
                .iter()
                .map(|fixture| (fixture.metadata.tree, fixture.tree.clone()))
                .collect(),
            archives: fixtures
                .iter()
                .map(|fixture| (fixture.metadata.commit, fixture.zip.clone()))
                .collect(),
            origin,
            operations: Mutex::new(Vec::new()),
        }
    }

    pub fn operations(&self) -> Vec<Operation> {
        self.operations.lock().expect("operation lock").clone()
    }

    fn record(&self, operation: Operation) {
        self.operations
            .lock()
            .expect("operation lock")
            .push(operation);
    }
}

impl Transport for RecordingTransport {
    fn resolve_latest(&self) -> Result<CommitMetadata, TransportError> {
        self.record(Operation::ResolveLatest);
        self.latest.clone().ok_or(TransportError::Metadata)
    }

    fn resolve_commit(&self, commit: Oid) -> Result<CommitMetadata, TransportError> {
        self.record(Operation::ResolveCommit(commit));
        self.commits
            .get(&commit)
            .cloned()
            .ok_or(TransportError::Metadata)
    }

    fn fetch_tree(&self, root_tree: Oid) -> Result<Vec<TreeEntry>, TransportError> {
        self.record(Operation::FetchTree(root_tree));
        self.trees
            .get(&root_tree)
            .cloned()
            .ok_or(TransportError::Metadata)
    }

    fn fetch_archive(&self, commit: Oid) -> Result<Vec<u8>, TransportError> {
        self.record(Operation::FetchArchive(commit));
        self.archives
            .get(&commit)
            .cloned()
            .ok_or(TransportError::Download)
    }

    fn archive_transport(&self) -> SourceTransport {
        self.origin
    }
}

pub fn oid(sha: &str) -> Oid {
    Oid::from_hex(sha).expect("valid fixture SHA")
}

pub fn request(
    selection: SourceSelection,
    verify_content: bool,
    use_cache: bool,
) -> TypeshedRequest {
    TypeshedRequest {
        selection,
        verify_content,
        use_cache,
        url_template: None,
    }
}

pub fn fixture(commit: &str, marker: &str) -> Fixture {
    let license = bundled_snapshot()
        .expect("bundled snapshot")
        .vfs
        .read("LICENSE")
        .expect("approved license")
        .to_vec();
    let lower = marker.to_ascii_lowercase();
    let files = vec![
        ("LICENSE".to_owned(), license),
        (
            "stdlib/VERSIONS".to_owned(),
            format!("os: 3.0-\n{lower}_only: 3.0-\n").into_bytes(),
        ),
        (
            "stdlib/os.pyi".to_owned(),
            format!("GENERATION: str = \"{marker}\"\n").into_bytes(),
        ),
        (
            format!("stdlib/{lower}_only.pyi"),
            format!("GENERATION: str = \"{marker}\"\n").into_bytes(),
        ),
        (
            format!("stubs/{lower}-demo/{lower}_demo.pyi"),
            b"VALUE: int\n".to_vec(),
        ),
    ];
    fixture_from_files(commit, &files)
}

pub fn fixture_from_files(commit: &str, files: &[(String, Vec<u8>)]) -> Fixture {
    let tree: Vec<_> = files
        .iter()
        .map(|(path, data)| TreeEntry {
            path: path.clone(),
            oid: git_blob_oid(data),
            mode: FileMode::Regular,
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
    let root = reconstruct_root_tree_oid(&git_files).expect("fixture tree");
    Fixture {
        metadata: CommitMetadata {
            commit: oid(commit),
            tree: root,
        },
        tree,
        zip: zip(commit, files),
    }
}

pub fn untrusted_fixture(commit: &str, files: &[(String, Vec<u8>)]) -> Fixture {
    Fixture {
        metadata: CommitMetadata {
            commit: oid(commit),
            tree: oid(B_SHA),
        },
        tree: Vec::new(),
        zip: zip(commit, files),
    }
}

fn zip(commit: &str, files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut writer = ZipWriter::new(Cursor::new(&mut bytes));
        for (path, data) in files {
            writer
                .start_file(
                    format!("typeshed-{commit}/{path}"),
                    SimpleFileOptions::default()
                        .compression_method(CompressionMethod::Stored)
                        .unix_permissions(0o644),
                )
                .expect("ZIP entry");
            writer.write_all(data).expect("ZIP body");
        }
        let _ = writer.finish().expect("ZIP finish");
    }
    bytes
}

pub fn manager(
    acquisition: TypeshedRequest,
    transport: Arc<RecordingTransport>,
    cache: Option<DiskCache>,
) -> basilisk_stubs::typeshed::manager::TypeshedManager {
    let injected: Arc<dyn Transport> = transport;
    manager_for_request(acquisition, injected, cache)
}
