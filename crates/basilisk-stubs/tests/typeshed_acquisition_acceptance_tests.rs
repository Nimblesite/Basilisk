//! Black-box acceptance coverage for offline Typeshed source resolution.
//!
//! Implements the parent [TYPESHEDRT-MODEL], [TYPESHEDRT-WORK], and
//! [TYPESHEDRT-ACCEPTANCE] contracts through the production manager only:
//! resolution is a LOCAL read of the store, the bundle, or a custom folder
//! ([STUBRES-TYPESHED-OFFLINE]) — a missing source tanks hard with the
//! spec's `NO SOURCE` line, and nothing ever downloads
//! ([TYPESHEDRT-SEGREGATION]).

#![expect(
    clippy::expect_used,
    reason = "test-only store and temporary-directory fixtures fail loudly"
)]

#[path = "support/typeshed_acquisition.rs"]
mod support;

use std::sync::Arc;

use basilisk_stubs::typeshed::runtime::production_manager;
use basilisk_stubs::typeshed::selector::{BackendError, SelectionError};
use basilisk_stubs::typeshed::source::{
    LicenseStatus, SourceIdentity, SourceKind, SourceSelection, TypeshedRequest,
};
use basilisk_stubs::typeshed::store::{entry_dir, StoreEntry};
use support::{approved_license, entry_from_files, install, pinned_request};

/// A custom-folder request; a custom source never touches the store.
fn custom_request(path: String) -> TypeshedRequest {
    TypeshedRequest {
        selection: SourceSelection::Custom { path },
        store_path: None,
    }
}

/// One complete generation whose stdlib carries `marker` so tests can prove
/// exactly which generation is being served.
fn generation(marker: &str) -> StoreEntry {
    let lower = marker.to_ascii_lowercase();
    let files = vec![
        ("LICENSE".to_owned(), approved_license()),
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
        // Repository files outside stdlib/ are bound through the manifest but
        // never materialised ([STUBRES-TYPESHED-STORE]).
        (
            format!("stubs/{lower}-demo/{lower}_demo.pyi"),
            b"VALUE: int\n".to_vec(),
        ),
    ];
    entry_from_files(&files)
}

/// [TYPESHEDRT-SEGREGATION]: the analysis crates hold no network or process
/// seam — downloading is a different crate that nothing on this path can
/// reach even by mistake.
#[test]
fn resolution_sources_carry_no_network_or_process_seams() {
    let sources = [
        ("runtime", include_str!("../src/typeshed/runtime.rs")),
        ("selector", include_str!("../src/typeshed/selector.rs")),
        ("store", include_str!("../src/typeshed/store.rs")),
        ("manager", include_str!("../src/typeshed/manager.rs")),
    ];
    let forbidden = [
        "ureq",
        "TcpStream",
        "std::process::Command",
        "Command::new",
        "git2::",
        "gix::",
        "http://",
        "https://api.",
        "codeload",
        "fetch_archive",
        "resolve_latest",
    ];
    for (name, source) in sources {
        for token in forbidden {
            assert!(
                !source.contains(token),
                "forbidden network/process token `{token}` appeared in {name}"
            );
        }
    }
}

/// [TYPESHEDRT-ACCEPTANCE-SOURCE]: one manager exposes exactly the pinned
/// generation — memoized, complete, and never mixed with a sibling entry.
#[test]
fn one_manager_exposes_only_the_pinned_b_generation() {
    let store = tempfile::tempdir().expect("store root");
    let _a = install(store.path(), &generation("A"));
    let b = install(store.path(), &generation("B"));

    let manager = production_manager(pinned_request(b, store.path()));
    let first = manager.snapshot().expect("B snapshot");
    let second = manager.snapshot().expect("same B snapshot");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.identity.commit(), Some(b));
    assert_eq!(first.status.active_source, SourceKind::ExactCommit);
    assert!(first
        .versions()
        .is_some_and(|versions| versions.contains("b_only")));
    assert!(first.module_index.path("b_only").is_some());
    assert!(first.module_index.path("a_only").is_none());
    assert_eq!(
        first.read_stub("os").map(|(_, body)| body),
        Some("GENERATION: str = \"B\"\n")
    );
    assert!(
        first.status.warnings.is_empty(),
        "an explicit verified pin carries no warnings: {:?}",
        first.status.warnings
    );
}

/// [STUBRES-TYPESHED-PIN]: a pin that is not on this machine is terminal
/// `NO SOURCE` — the message names the recovery commands, no sibling entry or
/// bundle is substituted, and the store is left byte-for-byte untouched.
#[test]
fn a_missing_pin_is_terminal_no_source_and_never_downloads() {
    let store = tempfile::tempdir().expect("store root");
    let _a = install(store.path(), &generation("A"));
    let absent = generation("GHOST").commit;

    let listing_before = store_listing(store.path());
    let error = production_manager(pinned_request(absent, store.path()))
        .snapshot()
        .expect_err("an absent pin has no source");
    assert_eq!(
        error,
        SelectionError::NoSource {
            commit: absent,
            reason: BackendError::Missing,
        }
    );
    let message = error.to_string();
    assert!(message.contains("NO SOURCE"), "loud status line: {message}");
    assert!(
        message.contains(&absent.to_hex()),
        "names the pin: {message}"
    );
    assert!(
        message.contains("basilisk typeshed download"),
        "names the recovery command: {message}"
    );
    assert_eq!(
        store_listing(store.path()),
        listing_before,
        "resolution must never write or fetch"
    );
}

/// [STUBRES-TYPESHED-PIN]: any mutated byte in a stored entry fails the
/// offline verification chain — the pin is corrupt, never silently served.
#[test]
fn a_tampered_store_entry_is_terminal_no_source() {
    let store = tempfile::tempdir().expect("store root");
    let entry = generation("A");
    let commit = install(store.path(), &entry);

    let stub = entry_dir(store.path(), commit).join("stdlib/os.pyi");
    std::fs::write(&stub, b"GENERATION: str = \"EVIL\"\n").expect("tamper fixture");

    let error = production_manager(pinned_request(commit, store.path()))
        .snapshot()
        .expect_err("a tampered entry must not activate");
    assert_eq!(
        error,
        SelectionError::NoSource {
            commit,
            reason: BackendError::Corrupt,
        }
    );
}

/// [STUBRES-CUSTOM-TYPESHED]: a custom tree is canonical — never rescued by
/// the store or the bundle, never assigned typeshed's license, and loudly
/// user-managed.
#[test]
fn custom_tree_is_canonical_and_never_rescued_by_store_or_bundle() {
    let custom = tempfile::tempdir().expect("custom root");
    let stdlib = custom.path().join("stdlib");
    std::fs::create_dir(&stdlib).expect("stdlib");
    std::fs::write(stdlib.join("VERSIONS"), "os: 3.0-\ncustom_only: 3.0-\n").expect("VERSIONS");
    std::fs::write(stdlib.join("os.pyi"), "GENERATION: str = \"CUSTOM\"\n").expect("custom os");
    std::fs::write(stdlib.join("custom_only.pyi"), "VALUE: int\n").expect("custom module");

    // A fully populated store must not bleed into the custom source.
    let store = tempfile::tempdir().expect("store root");
    let _a = install(store.path(), &generation("A"));

    let snapshot = production_manager(custom_request(custom.path().to_string_lossy().into_owned()))
        .snapshot()
        .expect("custom snapshot");
    assert!(matches!(snapshot.identity, SourceIdentity::Custom { .. }));
    assert_eq!(snapshot.status.active_source, SourceKind::Custom);
    assert_eq!(snapshot.status.commit, None);
    assert_eq!(snapshot.status.license_status, LicenseStatus::NotSupplied);
    assert!(snapshot.status.license_reference.is_none());
    assert_eq!(
        snapshot.read_stub("os").map(|(_, body)| body),
        Some("GENERATION: str = \"CUSTOM\"\n")
    );
    assert!(snapshot.read_stub("a_only").is_none());
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

/// [TYPESHEDRT-ACCEPTANCE-OVERRIDES]: custom-path failures are typed and
/// deterministic; there is no fallback and no second attempt with different
/// policy.
#[test]
fn custom_path_validation_is_typed_and_deterministic() {
    let relative = production_manager(custom_request("workspace-relative-typeshed".to_owned()))
        .snapshot()
        .expect_err("runtime requires config-resolved absolute path");
    assert_eq!(
        relative,
        SelectionError::Custom(BackendError::InvalidConfiguration)
    );

    let absent = tempfile::tempdir()
        .expect("parent")
        .path()
        .join("absent")
        .to_string_lossy()
        .into_owned();
    let first = production_manager(custom_request(absent.clone()))
        .snapshot()
        .expect_err("nonexistent path");
    let second = production_manager(custom_request(absent))
        .snapshot()
        .expect_err("same nonexistent path");
    assert_eq!(first, SelectionError::Custom(BackendError::Custom));
    assert_eq!(first, second);

    let missing_stdlib = tempfile::tempdir().expect("malformed root");
    std::fs::write(missing_stdlib.path().join("os.pyi"), "VALUE: int\n").expect("misplaced stub");
    let error = production_manager(custom_request(
        missing_stdlib.path().to_string_lossy().into_owned(),
    ))
    .snapshot()
    .expect_err("required top-level stdlib directory");
    assert_eq!(error, SelectionError::Custom(BackendError::Custom));

    let malformed = tempfile::tempdir().expect("malformed stdlib root");
    let stdlib = malformed.path().join("stdlib");
    std::fs::create_dir(&stdlib).expect("stdlib");
    std::fs::write(stdlib.join("VERSIONS"), "this is not a version row\n").expect("VERSIONS");
    std::fs::write(stdlib.join("os.pyi"), "VALUE: int\n").expect("stub");
    let error = production_manager(custom_request(
        malformed.path().to_string_lossy().into_owned(),
    ))
    .snapshot()
    .expect_err("malformed custom index");
    assert_eq!(error, SelectionError::Custom(BackendError::Custom));
}

/// Every file under the store root, relative and sorted, with sizes — a
/// byte-level "nothing was written" witness.
fn store_listing(root: &std::path::Path) -> Vec<(String, u64)> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, into: &mut Vec<(String, u64)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, into);
            } else if let (Ok(relative), Ok(metadata)) = (path.strip_prefix(root), entry.metadata())
            {
                into.push((relative.to_string_lossy().into_owned(), metadata.len()));
            }
        }
    }
    let mut listing = Vec::new();
    walk(root, root, &mut listing);
    listing.sort();
    listing
}
