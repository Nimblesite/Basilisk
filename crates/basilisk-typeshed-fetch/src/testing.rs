//! Offline fake-GitHub fixture for [STUBRES-TYPESHED-DOWNLOAD] tests: a
//! synthetic-but-honest repository at one commit (real Git hashing, real zip
//! encoding, the real approved LICENSE bytes). Shared by this crate's own
//! pipeline tests and — behind the `test-support` feature — by consumers
//! (`basilisk-cli`) exercising their download-invoking surfaces without a
//! network. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-DOWNLOAD.

use std::io::Write as _;

use basilisk_stubs::typeshed::bundle::bundled_snapshot;
use basilisk_stubs::typeshed::gittree::{
    git_blob_oid, git_commit_oid, reconstruct_root_tree_oid, FileMode, GitFile, Oid,
};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

use crate::github::CommitInfo;
use crate::{GithubApi, TransportError, TreeEntry};

/// A complete fake repository at one commit: the trusted tree, the raw commit
/// object (unsigned, so payload == raw), and the file bytes. Fields are public
/// so tests can tamper with individual pieces and assert the pipeline rejects
/// the inconsistency.
#[derive(Debug)]
pub struct FakeRepo {
    /// The commit OID the payload hashes to.
    pub commit: Oid,
    /// The root tree OID named inside the payload.
    pub tree: Oid,
    /// The raw (unsigned) commit-object payload.
    pub payload: String,
    /// The trusted recursive tree listing.
    pub tree_entries: Vec<TreeEntry>,
    /// Repo-relative path → file bytes.
    pub files: Vec<(String, Vec<u8>)>,
}

/// Build an internally consistent [`FakeRepo`] carrying the build-approved
/// LICENSE bytes, a README, and a tiny `stdlib/` stub set.
#[must_use]
pub fn fake_repo() -> FakeRepo {
    let license = bundled_snapshot()
        .ok()
        .and_then(|bundle| bundle.vfs.read("LICENSE").map(<[u8]>::to_vec))
        .unwrap_or_default();
    let files: Vec<(String, Vec<u8>)> = vec![
        ("LICENSE".to_owned(), license),
        ("README.md".to_owned(), b"readme body\n".to_vec()),
        ("stdlib/VERSIONS".to_owned(), b"os: 3.0-\n".to_vec()),
        (
            "stdlib/os.pyi".to_owned(),
            b"def getcwd() -> str: ...\n".to_vec(),
        ),
    ];
    let tree_entries: Vec<TreeEntry> = files
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
    let tree = reconstruct_root_tree_oid(&git_files).unwrap_or_else(|_error| git_blob_oid(b""));
    let payload =
        format!("tree {tree}\nauthor a <a@a> 0 +0000\ncommitter a <a@a> 0 +0000\n\nfixture\n");
    let commit = git_commit_oid(payload.as_bytes());
    FakeRepo {
        commit,
        tree,
        payload,
        tree_entries,
        files,
    }
}

/// Encode the repo files as a codeload-style zipball (single root prefix).
fn zipball(repo: &FakeRepo) -> Vec<u8> {
    let prefix = format!("python-typeshed-{}", repo.commit);
    let mut buffer = Vec::new();
    {
        let mut writer = ZipWriter::new(std::io::Cursor::new(&mut buffer));
        for (path, data) in &repo.files {
            let options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Stored)
                .unix_permissions(0o644);
            if writer
                .start_file(format!("{prefix}/{path}"), options)
                .is_err()
            {
                return Vec::new();
            }
            if writer.write_all(data).is_err() {
                return Vec::new();
            }
        }
        if writer.finish().is_err() {
            return Vec::new();
        }
    }
    buffer
}

/// One switchable failure per pipeline phase.
#[derive(Debug, Default, Clone, Copy)]
pub struct Faults {
    /// Fail commit-metadata resolution.
    pub resolve_fails: bool,
    /// Fail the trusted-tree fetch.
    pub tree_fails: bool,
    /// Fail the archive download.
    pub archive_fails: bool,
}

/// A [`GithubApi`] serving one [`FakeRepo`], with switchable [`Faults`].
#[derive(Debug)]
pub struct FakeApi {
    /// The repository served; tests may tamper with it after construction.
    pub repo: FakeRepo,
    /// Which pipeline phases fail.
    pub faults: Faults,
    archive: Vec<u8>,
}

impl FakeApi {
    /// Serve `repo`, pre-encoding its zipball from the current file bytes.
    #[must_use]
    pub fn new(repo: FakeRepo) -> Self {
        let archive = zipball(&repo);
        Self {
            repo,
            faults: Faults::default(),
            archive,
        }
    }
}

impl GithubApi for FakeApi {
    fn resolve(&self, _reference: &str) -> Result<CommitInfo, TransportError> {
        if self.faults.resolve_fails {
            return Err(TransportError::Metadata);
        }
        Ok(CommitInfo {
            commit: self.repo.commit,
            tree: self.repo.tree,
            payload: self.repo.payload.clone(),
            signature: None,
        })
    }

    fn fetch_tree(&self, _root_tree: Oid) -> Result<Vec<TreeEntry>, TransportError> {
        if self.faults.tree_fails {
            return Err(TransportError::Metadata);
        }
        Ok(self.repo.tree_entries.clone())
    }

    fn fetch_archive(&self, _commit: Oid) -> Result<Vec<u8>, TransportError> {
        if self.faults.archive_fails {
            return Err(TransportError::Download);
        }
        Ok(self.archive.clone())
    }
}
