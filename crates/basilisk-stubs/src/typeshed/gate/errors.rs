//! Implements [STUBRES-TYPESHED-ACQUIRE] gate errors. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-ACQUIRE
//!
//! The rejection taxonomy for the four activation gates.

use crate::typeshed::gittree::{Oid, TreeError};

use super::manifest::LicenseManifest;

/// A Safety-gate rejection.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SafetyViolation {
    /// A path had no components.
    #[error("empty archive path")]
    EmptyPath,
    /// A path was absolute.
    #[error("absolute archive path: {0}")]
    AbsolutePath(String),
    /// A path contained a `..` component.
    #[error("path escapes with '..': {0}")]
    ParentTraversal(String),
    /// A path contained a `.` component.
    #[error("path has a '.' segment: {0}")]
    CurrentDirSegment(String),
    /// The same path appeared more than once.
    #[error("duplicate archive path: {0}")]
    DuplicatePath(String),
    /// A symlink entry was present. Typeshed contains none, so every symlink is
    /// rejected outright rather than resolved — link-target bytes are never
    /// parsed as `.pyi`, and no cycle logic is needed.
    #[error("symlink entries are not permitted: {0}")]
    DisallowedSymlink(String),
    /// A submodule / gitlink entry was present. Typeshed's stdlib contains none,
    /// and its recorded OID is a commit, not a blob, so all are rejected.
    #[error("submodule entries are not permitted: {0}")]
    DisallowedSubmodule(String),
    /// A path had malformed syntax: a backslash, a NUL byte, an empty segment
    /// (`a//b`, trailing `/`), or a Windows drive prefix.
    #[error("malformed archive path: {0}")]
    MalformedPath(String),
    /// The archive had more entries than allowed.
    #[error("too many entries: {count} > {limit}")]
    TooManyEntries {
        /// Actual entry count.
        count: usize,
        /// Allowed maximum.
        limit: usize,
    },
    /// One entry exceeded the per-entry size limit.
    #[error("entry {path} too large: {size} > {limit} bytes")]
    EntryTooLarge {
        /// The offending entry.
        path: String,
        /// Its decompressed size.
        size: u64,
        /// Allowed maximum.
        limit: u64,
    },
    /// The archive's total decompressed size exceeded the limit.
    #[error("archive too large: {size} > {limit} bytes")]
    TotalTooLarge {
        /// Total decompressed size.
        size: u64,
        /// Allowed maximum.
        limit: u64,
    },
}

/// A Shape-gate rejection.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShapeViolation {
    /// `stdlib/VERSIONS` was absent.
    #[error("missing stdlib/VERSIONS")]
    MissingVersions,
    /// `stdlib/VERSIONS` was empty.
    #[error("empty stdlib/VERSIONS")]
    EmptyVersions,
    /// `stdlib/VERSIONS` had a malformed line.
    #[error("malformed stdlib/VERSIONS line: {0}")]
    MalformedVersions(String),
    /// No `stdlib/*.pyi` files were present.
    #[error("no stdlib .pyi stubs present")]
    NoStdlibStubs,
    /// The root `LICENSE` was absent.
    #[error("missing root LICENSE")]
    MissingLicense,
}

/// A License-gate rejection: the approved legal-file identity drifted
/// ([STUBRES-TYPESHED-LICENSE]).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("license identity drift: found {found:?} does not match approved {approved:?}")]
pub struct LicenseViolation {
    /// The build-approved legal-file manifest.
    pub approved: LicenseManifest,
    /// The manifest discovered in the archive.
    pub found: LicenseManifest,
}

/// A Content-gate rejection.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContentViolation {
    /// The archive tree could not be reconstructed.
    #[error("tree reconstruction failed: {0}")]
    Tree(TreeError),
    /// The reconstructed root tree did not match the trusted SHA.
    #[error("tree mismatch: reconstructed {actual}, expected {expected}")]
    TreeMismatch {
        /// The trusted root-tree SHA.
        expected: Oid,
        /// The reconstructed root-tree SHA.
        actual: Oid,
    },
}

/// A failure at any activation gate.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateError {
    /// Safety gate.
    #[error("safety gate: {0}")]
    Safety(SafetyViolation),
    /// Shape gate.
    #[error("shape gate: {0}")]
    Shape(ShapeViolation),
    /// License gate.
    #[error("license gate: {0}")]
    License(LicenseViolation),
    /// Content gate.
    #[error("content gate: {0}")]
    Content(ContentViolation),
}
