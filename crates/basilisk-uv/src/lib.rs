//! uv package manager integration for Basilisk LSP.
//!
//! Provides detection, lock file parsing, package registry construction, and
//! workspace resolution for Python projects managed by
//! [uv](https://github.com/astral-sh/uv).

pub mod detect;
pub mod error;
pub mod import_map;
pub mod lockfile;
pub mod pyproject;
pub mod python_version;
pub mod registry;
pub mod workspace;

pub use detect::{detect_uv_project, UvProjectInfo};
pub use error::UvError;
pub use lockfile::{parse_lock_file, LockFile};
pub use pyproject::extract_pyproject_deps;
pub use registry::{DepKind, PackageInfo, PackageRegistry};
pub use workspace::{parse_uv_workspace, UvWorkspace};
