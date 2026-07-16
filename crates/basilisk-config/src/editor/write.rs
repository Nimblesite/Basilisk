//! Revision-checked atomic persistence for non-LSP callers.

use std::io::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{content_revision, ConfigDocumentError, ConfigPatch};

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(1);

/// Persist a validated patch atomically after verifying its base revision.
///
/// LSP callers should ask the editor client to apply a workspace edit instead;
/// this API exists for the CLI and other direct filesystem frontends.
///
/// # Errors
///
/// Returns [`ConfigDocumentError`] if the source revision changed or the
/// validated replacement cannot be written and atomically renamed.
pub fn apply_config_patch(patch: &ConfigPatch) -> Result<(), ConfigDocumentError> {
    let current = if patch.path.is_file() {
        std::fs::read_to_string(&patch.path).map_err(|error| ConfigDocumentError::Read {
            path: patch.path.clone(),
            message: error.to_string(),
        })?
    } else {
        String::new()
    };
    let actual = content_revision(&current);
    if actual != patch.base_revision {
        return Err(ConfigDocumentError::RevisionConflict {
            expected: patch.base_revision.clone(),
            actual,
        });
    }
    let Some(parent) = patch.path.parent() else {
        return Err(ConfigDocumentError::Read {
            path: patch.path.clone(),
            message: "configuration path has no parent directory".to_owned(),
        });
    };
    std::fs::create_dir_all(parent).map_err(|error| ConfigDocumentError::Read {
        path: parent.to_path_buf(),
        message: error.to_string(),
    })?;
    let temp_name = format!(
        ".basilisk-config-{}-{}.tmp",
        std::process::id(),
        NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed)
    );
    let temp_path = parent.join(temp_name);
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| ConfigDocumentError::Read {
                path: temp_path.clone(),
                message: error.to_string(),
            })?;
        file.write_all(patch.content.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| ConfigDocumentError::Read {
                path: temp_path.clone(),
                message: error.to_string(),
            })?;
        std::fs::rename(&temp_path, &patch.path).map_err(|error| ConfigDocumentError::Read {
            path: patch.path.clone(),
            message: error.to_string(),
        })
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp_path);
    }
    result
}
