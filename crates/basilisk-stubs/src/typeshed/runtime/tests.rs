//! Backend and manager wiring over real local sources: the embedded bundle, a
//! temp-dir store, and custom trees. The verification chain itself is covered
//! in `store::tests`; these tests prove the production wiring resolves only
//! from this machine and fails hard when a source is absent
//! ([STUBRES-TYPESHED-OFFLINE], [TYPESHEDRT-ACCEPTANCE]).

use std::fs;

use super::super::gittree::Oid;
use super::super::selector::{BackendError, SelectionError};
use super::super::source::{SourceKind, SourceSelection, TypeshedRequest};
use super::*;

const OTHER_SHA: &str = "0123456789012345678901234567890123456789";

fn pinned_request(commit: Oid, explicit: bool, store: &std::path::Path) -> TypeshedRequest {
    TypeshedRequest {
        selection: SourceSelection::Pinned { commit, explicit },
        store_path: Some(store.to_path_buf()),
    }
}

/// The out-of-the-box conformance path: no configuration, no store entry, no
/// network — the bundled commit resolves offline and reports UNPINNED.
#[test]
fn the_bundled_default_resolves_offline_with_unpinned() {
    let store = tempfile::tempdir().expect("tempdir");
    let commit = Oid::from_hex(super::super::bundle::bundled_commit_sha()).expect("bundled sha");
    let manager = production_manager(pinned_request(commit, false, store.path()));
    let snapshot = manager.snapshot().expect("bundled default resolves");
    assert_eq!(snapshot.status.active_source, SourceKind::Bundled);
    let codes: Vec<&str> = snapshot
        .status
        .warnings
        .iter()
        .map(|warning| warning.code.as_str())
        .collect();
    assert_eq!(codes, vec!["UNPINNED"]);
    // Real bodies, offline.
    assert!(snapshot.read_stub("os").is_some());
}

/// An explicit pin of the bundled commit is deterministic: same bytes, no
/// UNPINNED, still zero store/network involvement.
#[test]
fn an_explicit_pin_of_the_bundled_commit_suppresses_unpinned() {
    let store = tempfile::tempdir().expect("tempdir");
    let commit = Oid::from_hex(super::super::bundle::bundled_commit_sha()).expect("bundled sha");
    let manager = production_manager(pinned_request(commit, true, store.path()));
    let snapshot = manager.snapshot().expect("explicit bundled pin resolves");
    assert!(snapshot.status.warnings.is_empty());
    assert_eq!(snapshot.status.active_source, SourceKind::Bundled);
}

/// Implements the **Fails hard** acceptance item: a pin with no store entry
/// refuses to analyse, names the missing SHA and its fix, and never
/// substitutes another source ([STUBRES-TYPESHED-OFFLINE]).
#[test]
fn a_pin_absent_from_this_machine_is_terminal_no_source() {
    let store = tempfile::tempdir().expect("tempdir");
    let commit = Oid::from_hex(OTHER_SHA).expect("valid oid");
    let manager = production_manager(pinned_request(commit, true, store.path()));
    let error = manager.snapshot().expect_err("missing pin must fail");
    assert_eq!(
        error,
        SelectionError::NoSource {
            commit,
            reason: BackendError::Missing,
        }
    );
    assert!(error.to_string().contains(OTHER_SHA));
    assert!(error.to_string().contains("Download latest"));
    // The store is inert: the failed read created nothing.
    assert_eq!(
        fs::read_dir(store.path()).expect("readdir").count(),
        0,
        "resolution must never write the store"
    );
}

/// A present-but-corrupt entry is the same terminal failure with its reason.
#[test]
fn a_corrupt_store_entry_is_terminal_no_source() {
    let store = tempfile::tempdir().expect("tempdir");
    let commit = Oid::from_hex(OTHER_SHA).expect("valid oid");
    let dir = store.path().join(OTHER_SHA);
    fs::create_dir_all(&dir).expect("entry dir");
    fs::write(dir.join("commit-object"), b"not a commit object").expect("garbage");
    let manager = production_manager(pinned_request(commit, true, store.path()));
    assert_eq!(
        manager.snapshot().err(),
        Some(SelectionError::NoSource {
            commit,
            reason: BackendError::Corrupt,
        })
    );
    // The checker never repairs or evicts: the corrupt entry stays on disk
    // until a download replaces it ([STUBRES-TYPESHED-STORE]).
    assert!(dir.join("commit-object").exists());
}

/// A missing custom folder is the custom source's own hard failure; a custom
/// module miss (folder exists, module absent) is step-4 fallthrough and is
/// covered by the resolver tests, not a source substitution.
#[test]
fn a_missing_custom_folder_fails_without_fallback() {
    let manager = production_manager(TypeshedRequest {
        selection: SourceSelection::Custom {
            path: "/nonexistent/custom-typeshed".to_owned(),
        },
        store_path: None,
    });
    assert_eq!(
        manager.snapshot().err(),
        Some(SelectionError::Custom(BackendError::Custom))
    );
}

/// A real custom tree activates verbatim and reports user-managed status.
#[test]
fn a_custom_tree_activates_verbatim() {
    let root = tempfile::tempdir().expect("tempdir");
    let stdlib = root.path().join("stdlib");
    fs::create_dir(&stdlib).expect("stdlib");
    fs::write(stdlib.join("VERSIONS"), "os: 3.0-\n").expect("versions");
    fs::write(stdlib.join("os.pyi"), "name: str\n").expect("stub");
    let manager = production_manager(TypeshedRequest {
        selection: SourceSelection::Custom {
            path: root.path().to_string_lossy().into_owned(),
        },
        store_path: None,
    });
    let snapshot = manager.snapshot().expect("custom tree resolves");
    assert_eq!(snapshot.status.active_source, SourceKind::Custom);
    let codes: Vec<&str> = snapshot
        .status
        .warnings
        .iter()
        .map(|warning| warning.code.as_str())
        .collect();
    assert_eq!(codes, vec!["UNPINNED", "USER-MANAGED SOURCE"]);
    assert_eq!(
        snapshot.read_stub("os").map(|(_, body)| body),
        Some("name: str\n")
    );
}

/// The store location is honoured: an entry in a configured store resolves,
/// and the same pin against an empty default-shaped store does not.
#[test]
fn pins_resolve_from_the_configured_store_root() {
    let configured = tempfile::tempdir().expect("tempdir");
    let elsewhere = tempfile::tempdir().expect("tempdir");
    let commit = Oid::from_hex(OTHER_SHA).expect("valid oid");
    let backend = RuntimeBackend::new(Some(configured.path().to_path_buf()));
    assert_eq!(
        backend.load_pinned(commit, true).err(),
        Some(BackendError::Missing)
    );
    let other_backend = RuntimeBackend::new(Some(elsewhere.path().to_path_buf()));
    assert_eq!(
        other_backend.load_pinned(commit, true).err(),
        Some(BackendError::Missing)
    );
}
