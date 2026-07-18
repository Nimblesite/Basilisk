//! Implements [STUBRES-TYPESHED-ACQUIRE] Content gate. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-ACQUIRE
//!
//! Git object-ID reconstruction.
//!
//! The Content gate binds the bytes the checker actually reads to a **trusted
//! root-tree SHA** obtained from GitHub's commit→tree metadata. GitHub source
//! archives are not byte-stable (compression and prefixing vary), so the gate
//! never hashes archive bytes; it reconstructs Git object IDs the way Git itself
//! does and compares against the trusted tree SHA.
//!
//! A Git blob ID is `SHA-1("blob " + len + "\0" + content)`; a tree ID is
//! `SHA-1("tree " + len + "\0" + entries)` where each entry is
//! `mode + " " + name + "\0" + raw-20-byte-oid`, and entries are sorted by Git's
//! canonical name order (a directory sorts as if its name ended in `/`). These
//! definitions are pinned points: the empty blob is
//! `e69de29bb2d1d6434b8b29ae775ad8c2e48c5391` and the empty tree is
//! `4b825dc642cb6eb9a060e54bf8d69288fbee4904`, both asserted in the tests.
//!
//! Reconstruction is exact and order-independent: any content change flips the
//! root ID, and re-encoding the same tree differently does not. This supports
//! both verification strategies — recomputing the root tree, or verifying each
//! archived blob against a trusted recursive tree listing.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

use sha1::{Digest, Sha1};

/// A 20-byte Git object identifier (SHA-1), rendered as 40 lowercase hex chars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Oid([u8; 20]);

/// Error parsing a hex object ID.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OidParseError {
    /// The hex string was not exactly 40 characters.
    #[error("object id must be 40 hex chars, got {0}")]
    Length(usize),
    /// The hex string contained a non-hex byte.
    #[error("object id contains a non-hex character")]
    NotHex,
}

impl Oid {
    /// Wrap 20 raw bytes as an object ID.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    /// The raw 20 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    /// Render as 40 lowercase hex characters.
    #[must_use]
    pub fn to_hex(&self) -> String {
        use fmt::Write as _;
        let mut hex = String::with_capacity(40);
        for byte in self.0 {
            // Writing to a `String` is infallible; the discard is deliberate.
            let _ = write!(hex, "{byte:02x}");
        }
        hex
    }

    /// Parse 40 lowercase-or-uppercase hex characters into an object ID.
    ///
    /// # Errors
    ///
    /// Returns [`OidParseError::Length`] if the input is not 40 characters, or
    /// [`OidParseError::NotHex`] if it contains a non-hex byte.
    pub fn from_hex(hex: &str) -> Result<Self, OidParseError> {
        if hex.len() != 40 {
            return Err(OidParseError::Length(hex.len()));
        }
        let mut bytes: Vec<u8> = Vec::with_capacity(20);
        for pair in hex.as_bytes().chunks_exact(2) {
            let hi = hex_nibble(pair.first().copied())?;
            let lo = hex_nibble(pair.get(1).copied())?;
            bytes.push((hi << 4) | lo);
        }
        let bytes: [u8; 20] = bytes
            .try_into()
            .map_err(|got: Vec<u8>| OidParseError::Length(got.len()))?;
        Ok(Self(bytes))
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl serde::Serialize for Oid {
    /// Serializes as the 40-char lowercase hex string, never a byte array, so
    /// every surface reports a full, canonical SHA.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

/// Decode one hex nibble, or fail.
fn hex_nibble(byte: Option<u8>) -> Result<u8, OidParseError> {
    match byte {
        Some(b @ b'0'..=b'9') => Ok(b - b'0'),
        Some(b @ b'a'..=b'f') => Ok(b - b'a' + 10),
        Some(b @ b'A'..=b'F') => Ok(b - b'A' + 10),
        _ => Err(OidParseError::NotHex),
    }
}

/// Git file mode for a leaf tree entry (never a subtree — subtrees are
/// synthesized during reconstruction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileMode {
    /// A regular, non-executable file (`100644`).
    Regular,
    /// An executable file (`100755`).
    Executable,
    /// A symbolic link (`120000`); its blob content is the link target.
    Symlink,
    /// A gitlink / submodule commit reference (`160000`).
    Submodule,
}

impl FileMode {
    /// The canonical Git mode string used in tree-entry encoding.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "100644",
            Self::Executable => "100755",
            Self::Symlink => "120000",
            Self::Submodule => "160000",
        }
    }
}

/// One archived file to feed into root-tree reconstruction.
#[derive(Debug, Clone)]
pub struct GitFile {
    /// Slash-separated repo-relative path (archive prefix already stripped).
    pub path: String,
    /// The file's Git blob ID (for a submodule, its recorded commit ID).
    pub oid: Oid,
    /// The file's Git mode.
    pub mode: FileMode,
}

/// Error reconstructing a Git tree from a flat file list.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TreeError {
    /// A file entry had an empty path (no components).
    #[error("archived file has an empty path")]
    EmptyPath,
    /// The same path appeared twice.
    #[error("duplicate archived path: {0}")]
    DuplicatePath(String),
    /// A name was used as both a file and a directory.
    #[error("path used as both file and directory: {0}")]
    PathConflict(String),
    /// A path contained a `.` or `..` component (never silently normalized).
    #[error("path has an unsafe '.'/'..' component: {0}")]
    UnsafeComponent(String),
}

/// Compute a Git blob ID for raw file content.
#[must_use]
pub fn git_blob_oid(content: &[u8]) -> Oid {
    git_object_oid(b"blob", content)
}

/// Reconstruct the Git **root-tree** object ID for a flat set of archived files.
///
/// An empty set yields Git's canonical empty-tree ID. Reconstruction is exact
/// and independent of input order.
///
/// # Errors
///
/// Returns [`TreeError`] if any path is empty, duplicated, or used as both a
/// file and a directory.
pub fn reconstruct_root_tree_oid(files: &[GitFile]) -> Result<Oid, TreeError> {
    let mut leaves: Vec<Leaf> = Vec::with_capacity(files.len());
    for file in files {
        let mut components: Vec<String> = Vec::new();
        for segment in file.path.split('/') {
            match segment {
                "" => return Err(TreeError::EmptyPath),
                "." | ".." => return Err(TreeError::UnsafeComponent(file.path.clone())),
                other => components.push(other.to_owned()),
            }
        }
        if components.is_empty() {
            return Err(TreeError::EmptyPath);
        }
        leaves.push(Leaf {
            components,
            oid: file.oid,
            mode: file.mode.as_str(),
        });
    }
    build_tree(leaves)
}

/// A file flattened to its remaining path components at the current tree depth.
struct Leaf {
    components: Vec<String>,
    oid: Oid,
    mode: &'static str,
}

/// One entry in a single tree level, ready to be encoded.
struct TreeEntry {
    name: String,
    oid: Oid,
    mode: &'static str,
    is_tree: bool,
}

/// Build the tree ID for one directory level, recursing into subdirectories.
fn build_tree(leaves: Vec<Leaf>) -> Result<Oid, TreeError> {
    let mut files: BTreeMap<String, TreeEntry> = BTreeMap::new();
    let mut dirs: BTreeMap<String, Vec<Leaf>> = BTreeMap::new();
    for leaf in leaves {
        let (head, rest) = leaf.components.split_first().ok_or(TreeError::EmptyPath)?;
        let head = head.clone();
        if rest.is_empty() {
            if dirs.contains_key(&head) {
                return Err(TreeError::PathConflict(head));
            }
            let entry = TreeEntry {
                name: head.clone(),
                oid: leaf.oid,
                mode: leaf.mode,
                is_tree: false,
            };
            if files.insert(head.clone(), entry).is_some() {
                return Err(TreeError::DuplicatePath(head));
            }
        } else {
            if files.contains_key(&head) {
                return Err(TreeError::PathConflict(head));
            }
            dirs.entry(head).or_default().push(Leaf {
                components: rest.to_vec(),
                oid: leaf.oid,
                mode: leaf.mode,
            });
        }
    }
    let mut entries: Vec<TreeEntry> = files.into_values().collect();
    for (name, children) in dirs {
        let oid = build_tree(children)?;
        entries.push(TreeEntry {
            name,
            oid,
            mode: "40000",
            is_tree: true,
        });
    }
    Ok(git_tree_oid(&entries))
}

/// Encode and hash one tree level's entries into its Git tree ID.
fn git_tree_oid(entries: &[TreeEntry]) -> Oid {
    let mut ordered: Vec<&TreeEntry> = entries.iter().collect();
    ordered.sort_by(|left, right| entry_order(left, right));
    let mut body: Vec<u8> = Vec::new();
    for entry in ordered {
        body.extend_from_slice(entry.mode.as_bytes());
        body.push(b' ');
        body.extend_from_slice(entry.name.as_bytes());
        body.push(0);
        body.extend_from_slice(entry.oid.as_bytes());
    }
    git_object_oid(b"tree", &body)
}

/// Git's canonical tree-entry ordering: byte order by name, but a directory
/// sorts as though its name ended in `/`.
fn entry_order(left: &TreeEntry, right: &TreeEntry) -> Ordering {
    let (an, bn) = (left.name.as_bytes(), right.name.as_bytes());
    let mut index = 0usize;
    loop {
        match (an.get(index), bn.get(index)) {
            (Some(a), Some(b)) if a == b => index = index.saturating_add(1),
            (Some(a), Some(b)) => return a.cmp(b),
            (Some(a), None) => return a.cmp(&trailing(right.is_tree)),
            (None, Some(b)) => return trailing(left.is_tree).cmp(b),
            (None, None) => return Ordering::Equal,
        }
    }
}

/// The virtual trailing byte for a name: `/` for a directory, `0` otherwise.
const fn trailing(is_tree: bool) -> u8 {
    if is_tree {
        b'/'
    } else {
        0
    }
}

/// Hash a Git object: `SHA-1(kind + " " + len + "\0" + body)`.
fn git_object_oid(kind: &[u8], body: &[u8]) -> Oid {
    let mut hasher = Sha1::new();
    hasher.update(kind);
    hasher.update(b" ");
    hasher.update(body.len().to_string().as_bytes());
    hasher.update([0u8]);
    hasher.update(body);
    let digest = hasher.finalize();
    let mut bytes = [0u8; 20];
    bytes.copy_from_slice(digest.as_slice());
    Oid(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned Git constant: the empty blob.
    const EMPTY_BLOB: &str = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";
    /// Pinned Git constant: the empty tree.
    const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

    fn file(path: &str, content: &[u8]) -> GitFile {
        GitFile {
            path: path.to_owned(),
            oid: git_blob_oid(content),
            mode: FileMode::Regular,
        }
    }

    #[test]
    fn empty_blob_matches_git() {
        assert_eq!(git_blob_oid(b"").to_hex(), EMPTY_BLOB);
    }

    #[test]
    fn hello_blob_matches_git() {
        // `printf 'hello\n' | git hash-object --stdin`
        assert_eq!(
            git_blob_oid(b"hello\n").to_hex(),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
    }

    #[test]
    fn empty_tree_matches_git() {
        assert_eq!(
            reconstruct_root_tree_oid(&[]).map(|oid| oid.to_hex()),
            Ok(EMPTY_TREE.to_owned())
        );
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(
            Oid::from_hex(EMPTY_BLOB).map(|oid| oid.to_hex()),
            Ok(EMPTY_BLOB.to_owned())
        );
    }

    #[test]
    fn hex_rejects_bad_input() {
        assert_eq!(Oid::from_hex("abcd"), Err(OidParseError::Length(4)));
        assert!(matches!(
            Oid::from_hex("zz13625030ba8dba906f756967f9e9ca394464a0"),
            Err(OidParseError::NotHex)
        ));
    }

    #[test]
    fn root_tree_is_order_independent() {
        let forward = [
            file("stdlib/os.pyi", b"class A: ...\n"),
            file("stdlib/sys.pyi", b"x: int\n"),
            file("stdlib/asyncio/__init__.pyi", b"def run(): ...\n"),
        ];
        let reversed = [
            file("stdlib/asyncio/__init__.pyi", b"def run(): ...\n"),
            file("stdlib/sys.pyi", b"x: int\n"),
            file("stdlib/os.pyi", b"class A: ...\n"),
        ];
        let a = reconstruct_root_tree_oid(&forward).map(|oid| oid.to_hex());
        let b = reconstruct_root_tree_oid(&reversed).map(|oid| oid.to_hex());
        assert!(a.is_ok());
        assert_eq!(a, b, "re-encoding the same tree must give the same root id");
    }

    #[test]
    fn any_content_change_flips_the_root() {
        let original = [
            file("stdlib/os.pyi", b"class A: ...\n"),
            file("stdlib/asyncio/__init__.pyi", b"def run(): ...\n"),
        ];
        let mutated = [
            file("stdlib/os.pyi", b"class A: ...\n"),
            file("stdlib/asyncio/__init__.pyi", b"def run(x): ...\n"),
        ];
        let a = reconstruct_root_tree_oid(&original).map(|oid| oid.to_hex());
        let b = reconstruct_root_tree_oid(&mutated).map(|oid| oid.to_hex());
        assert!(a.is_ok() && b.is_ok());
        assert_ne!(a, b, "a single byte change must change the root tree id");
    }

    #[test]
    fn file_and_directory_name_collision_is_rejected() {
        let files = [
            file("stdlib/os", b"x\n"),
            file("stdlib/os/path.pyi", b"y\n"),
        ];
        assert!(matches!(
            reconstruct_root_tree_oid(&files),
            Err(TreeError::PathConflict(_))
        ));
    }

    #[test]
    fn duplicate_path_is_rejected() {
        let files = [file("stdlib/os.pyi", b"a\n"), file("stdlib/os.pyi", b"b\n")];
        assert!(matches!(
            reconstruct_root_tree_oid(&files),
            Err(TreeError::DuplicatePath(_))
        ));
    }
}
