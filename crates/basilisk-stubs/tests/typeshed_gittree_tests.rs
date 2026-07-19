//! Acceptance tests for [STUBRES-TYPESHED-ACQUIRE] Git-tree binding.
#![expect(clippy::expect_used, reason = "acceptance test: expect is acceptable")]

use basilisk_stubs::typeshed::gittree::{
    git_blob_oid, reconstruct_root_tree_oid, FileMode, GitFile,
};

fn file(path: &str, mode: FileMode, bytes: &[u8]) -> GitFile {
    GitFile {
        path: path.to_owned(),
        oid: git_blob_oid(bytes),
        mode,
    }
}

#[test]
fn consumed_bytes_reconstruct_the_known_git_tree() {
    let files = vec![file("hello.txt", FileMode::Regular, b"hello\n")];
    let root = reconstruct_root_tree_oid(&files).expect("known Git tree must reconstruct");
    assert_eq!(root.to_hex(), "aaa96ced2d9a1c8e72c56b253a0e2fe78393feb7");
}

#[test]
fn content_mutation_changes_the_reconstructed_tree() {
    let accepted = vec![file("hello.txt", FileMode::Regular, b"hello\n")];
    let changed = vec![file("hello.txt", FileMode::Regular, b"tampered\n")];
    assert_ne!(
        reconstruct_root_tree_oid(&accepted),
        reconstruct_root_tree_oid(&changed)
    );
}

#[test]
fn trusted_mode_contributes_to_the_tree_identity() {
    let regular = vec![file("script.py", FileMode::Regular, b"pass\n")];
    let executable = vec![file("script.py", FileMode::Executable, b"pass\n")];
    assert_ne!(
        reconstruct_root_tree_oid(&regular),
        reconstruct_root_tree_oid(&executable)
    );
}

#[test]
fn extra_or_missing_paths_change_the_tree_identity() {
    let expected = vec![file("hello.txt", FileMode::Regular, b"hello\n")];
    let extra = vec![
        file("hello.txt", FileMode::Regular, b"hello\n"),
        file("extra.txt", FileMode::Regular, b""),
    ];
    assert_ne!(
        reconstruct_root_tree_oid(&expected),
        reconstruct_root_tree_oid(&extra)
    );
    assert_ne!(
        reconstruct_root_tree_oid(&expected),
        reconstruct_root_tree_oid(&[])
    );
}
