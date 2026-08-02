//! Implements [LSPUV]. See docs/specs/LSP-UV-INTEGRATION-SPEC.md#LSPUV
//! uv package manager integration for Basilisk LSP.
//!
//! Provides detection, lock file parsing, package registry construction, and
//! workspace resolution for Python projects managed by
//! [uv](https://github.com/astral-sh/uv).

pub mod binary;
pub mod detect;
pub mod error;
pub mod import_map;
pub mod lockfile;
pub mod pyproject;
pub mod python_version;
pub mod registry;
pub mod workspace;

pub use binary::{find_uv_binary, is_uv_available};
pub use detect::{detect_uv_project, UvProjectInfo};
pub use error::UvError;
pub use lockfile::{
    find_typeshed_package_pin, parse_lock_file, resolve_typeshed_package_pin, LockFile, LockWheel,
};
pub use pyproject::extract_pyproject_deps;
pub use registry::{DepKind, PackageInfo, PackageRegistry};
pub use workspace::{discover_workspace_members, parse_uv_workspace, UvWorkspace};
