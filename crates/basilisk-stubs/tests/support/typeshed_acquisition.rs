//! Shared store-entry fixtures for black-box Typeshed acceptance tests.
//!
//! Every fixture is honestly content-addressed: the commit SHA is the real
//! hash of a real commit object naming the real root tree of the files, so
//! the offline pin verification chain ([STUBRES-TYPESHED-PIN]) is exercised
//! for real — nothing is stubbed past the gates.
#![allow(
    dead_code,
    clippy::allow_attributes,
    reason = "each acceptance test binary uses a subset of these shared fixtures"
)]

use std::path::Path;

use basilisk_stubs::typeshed::archive::ArchiveEntry;
use basilisk_stubs::typeshed::bundle::bundled_snapshot;
use basilisk_stubs::typeshed::gittree::{
    git_blob_oid, git_commit_oid, reconstruct_root_tree_oid, FileMode, GitFile, Oid,
};
use basilisk_stubs::typeshed::source::{SourceSelection, TypeshedRequest};
use basilisk_stubs::typeshed::store::{
    is_materialized, write_entry, StoreEntry, StoreManifest, StoreTreeFile,
};

/// A pin request against an explicit store root.
pub fn pinned_request(commit: Oid, store: &Path) -> TypeshedRequest {
    TypeshedRequest {
        selection: SourceSelection::Pinned {
            commit,
            explicit: true,
        },
        store_path: Some(store.to_path_buf()),
    }
}

/// The build-approved LICENSE bytes, so the legal-identity gate passes.
pub fn approved_license() -> Vec<u8> {
    bundled_snapshot()
        .expect("bundled snapshot")
        .vfs
        .read("LICENSE")
        .expect("approved license")
        .to_vec()
}

/// Build a complete, verifiable store entry from raw repository files.
pub fn entry_from_files(files: &[(String, Vec<u8>)]) -> StoreEntry {
    let git_files: Vec<GitFile> = files
        .iter()
        .map(|(path, data)| GitFile {
            path: path.clone(),
            oid: git_blob_oid(data),
            mode: FileMode::Regular,
        })
        .collect();
    let tree = reconstruct_root_tree_oid(&git_files).expect("fixture tree");
    let commit_object =
        format!("tree {tree}\nauthor a <a@a> 0 +0000\ncommitter a <a@a> 0 +0000\n\nfixture\n")
            .into_bytes();
    let commit = git_commit_oid(&commit_object);
    let manifest = StoreManifest {
        commit: commit.to_hex(),
        tree: tree.to_hex(),
        tree_files: git_files
            .iter()
            .map(|file| StoreTreeFile {
                path: file.path.clone(),
                oid: file.oid.to_hex(),
                mode: file.mode.as_str().to_owned(),
            })
            .collect(),
    };
    StoreEntry {
        commit,
        commit_object,
        manifest,
        files: files
            .iter()
            .filter(|(path, _)| is_materialized(path))
            .map(|(path, data)| ArchiveEntry {
                path: path.clone(),
                mode: FileMode::Regular,
                data: data.clone(),
            })
            .collect(),
    }
}

/// Write `entry` into `store` and return its pinned commit.
pub fn install(store: &Path, entry: &StoreEntry) -> Oid {
    write_entry(store, entry).expect("store entry write");
    entry.commit
}
