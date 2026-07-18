//! User-managed custom-tree activation.

use std::fs;
use std::path::Path;

use sha2::{Digest as _, Sha256};

use super::super::archive::{Archive, ArchiveEntry, ArchiveVfs};
use super::super::gate::{safety_gate, SafetyLimits};
use super::super::gittree::FileMode;
use super::super::selector::BackendError;
use super::super::snapshot::Snapshot;
use super::super::source::{
    LicenseStatus, Provenance, SourceIdentity, SourceKind, Transport, TypeshedStatus,
};

/// Load a custom tree into one immutable, content-identified snapshot.
pub(super) fn load_custom_snapshot(path: &str) -> Result<Snapshot, BackendError> {
    let configured = Path::new(path);
    if !configured.is_absolute() {
        return Err(BackendError::InvalidConfiguration);
    }
    let root = fs::canonicalize(configured).map_err(|_error| BackendError::Custom)?;
    if !root.is_dir() || !root.join("stdlib").is_dir() {
        return Err(BackendError::Custom);
    }
    let limits = SafetyLimits::default();
    let mut entries = Vec::new();
    let mut total = 0u64;
    let stdlib = root.join("stdlib");
    // Step 3 consumes only the custom stdlib subtree. Unrelated repository
    // files must neither leak into the VFS nor perturb its semantic identity.
    collect_files(&root, &stdlib, &stdlib, &limits, &mut total, &mut entries)?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    let archive = Archive::new(entries);
    safety_gate(&archive, &limits).map_err(|_error| BackendError::Custom)?;
    let digest = content_digest(&archive);
    let identity = SourceIdentity::Custom { digest };
    let status = TypeshedStatus {
        active_source: SourceKind::Custom,
        commit: None,
        tree: None,
        transport: Transport::CustomPath,
        license_status: LicenseStatus::NotSupplied,
        license_reference: None,
        provenance: Provenance::UserManaged,
        signed_release: false,
        warnings: Vec::new(),
    };
    let uri_identity = identity.uri_component();
    Snapshot::build(
        identity,
        status,
        ArchiveVfs::new(uri_identity, archive),
        None,
    )
    .map_err(|_error| BackendError::Custom)
}

fn collect_files(
    root: &Path,
    physical_dir: &Path,
    logical_dir: &Path,
    limits: &SafetyLimits,
    total: &mut u64,
    entries: &mut Vec<ArchiveEntry>,
) -> Result<(), BackendError> {
    let mut children: Vec<_> = fs::read_dir(physical_dir)
        .map_err(|_error| BackendError::Custom)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_error| BackendError::Custom)?;
    children.sort_by_key(std::fs::DirEntry::file_name);
    for child in children {
        let physical = child.path();
        let logical = logical_dir.join(child.file_name());
        let metadata = fs::symlink_metadata(&physical).map_err(|_error| BackendError::Custom)?;
        if metadata.is_dir() {
            collect_files(root, &physical, &logical, limits, total, entries)?;
            continue;
        }
        let (read_path, file_metadata) = if metadata.file_type().is_symlink() {
            let target = fs::canonicalize(&physical).map_err(|_error| BackendError::Custom)?;
            if !target.starts_with(root) {
                return Err(BackendError::Custom);
            }
            let target_metadata = fs::metadata(&target).map_err(|_error| BackendError::Custom)?;
            // Directory symlinks can form cycles and aliases. File symlinks are
            // safe after containment validation and become immutable bytes.
            if !target_metadata.is_file() {
                return Err(BackendError::Custom);
            }
            (target, target_metadata)
        } else if metadata.is_file() {
            (physical, metadata)
        } else {
            return Err(BackendError::Custom);
        };
        let declared = file_metadata.len();
        if declared > limits.max_entry_bytes {
            return Err(BackendError::Custom);
        }
        let data = fs::read(read_path).map_err(|_error| BackendError::Custom)?;
        let size = u64::try_from(data.len()).unwrap_or(u64::MAX);
        if size > limits.max_entry_bytes {
            return Err(BackendError::Custom);
        }
        *total = total.saturating_add(size);
        if *total > limits.max_total_bytes || entries.len() >= limits.max_entries {
            return Err(BackendError::Custom);
        }
        let path = logical
            .strip_prefix(root)
            .map_err(|_error| BackendError::Custom)?
            .to_str()
            .ok_or(BackendError::Custom)?
            .replace(std::path::MAIN_SEPARATOR, "/");
        entries.push(ArchiveEntry {
            path,
            mode: file_mode(&file_metadata),
            data,
        });
    }
    Ok(())
}

#[cfg(unix)]
fn file_mode(metadata: &fs::Metadata) -> FileMode {
    use std::os::unix::fs::PermissionsExt as _;
    if metadata.permissions().mode() & 0o111 == 0 {
        FileMode::Regular
    } else {
        FileMode::Executable
    }
}

#[cfg(not(unix))]
fn file_mode(_metadata: &fs::Metadata) -> FileMode {
    FileMode::Regular
}

fn content_digest(archive: &Archive) -> String {
    use std::fmt::Write as _;

    let mut hasher = Sha256::new();
    for entry in archive.entries() {
        let path_len = u64::try_from(entry.path.len()).unwrap_or(u64::MAX);
        let data_len = u64::try_from(entry.data.len()).unwrap_or(u64::MAX);
        hasher.update(path_len.to_be_bytes());
        hasher.update(entry.path.as_bytes());
        hasher.update(data_len.to_be_bytes());
        hasher.update(&entry.data);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only temporary filesystem fixtures require setup"
)]
mod tests {
    use super::*;

    #[test]
    fn same_path_mutation_changes_custom_content_identity() {
        let root = tempfile::tempdir().expect("tempdir");
        let stdlib = root.path().join("stdlib");
        fs::create_dir(&stdlib).expect("stdlib");
        fs::write(stdlib.join("VERSIONS"), "os: 3.0-\n").expect("versions");
        fs::write(stdlib.join("os.pyi"), "name: str\n").expect("stub");
        let first = load_custom_snapshot(root.path().to_str().expect("utf8")).expect("first");
        fs::write(stdlib.join("os.pyi"), "name: bytes\n").expect("mutate");
        let second = load_custom_snapshot(root.path().to_str().expect("utf8")).expect("second");
        assert_ne!(first.identity, second.identity);
        assert_eq!(
            second.read_stub("os").map(|(_, body)| body),
            Some("name: bytes\n")
        );
    }

    #[test]
    fn unrelated_files_and_modes_do_not_change_custom_identity() {
        let root = tempfile::tempdir().expect("tempdir");
        let stdlib = root.path().join("stdlib");
        fs::create_dir(&stdlib).expect("stdlib");
        fs::write(stdlib.join("os.pyi"), "name: str\n").expect("stub");
        let first = load_custom_snapshot(root.path().to_str().expect("utf8")).expect("first");
        fs::create_dir(root.path().join("stubs")).expect("unrelated dir");
        fs::write(root.path().join("stubs/demo.pyi"), "value: int\n").expect("unrelated");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(stdlib.join("os.pyi"), fs::Permissions::from_mode(0o755))
                .expect("chmod");
        }
        let second = load_custom_snapshot(root.path().to_str().expect("utf8")).expect("second");
        assert_eq!(first.identity, second.identity);
        assert!(second.vfs.read("stubs/demo.pyi").is_none());
        assert_eq!(
            second.read_stub("os").map(|(_, body)| body),
            Some("name: str\n")
        );
    }

    #[cfg(unix)]
    #[test]
    fn escaping_symlink_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::NamedTempFile::new().expect("outside");
        let stdlib = root.path().join("stdlib");
        fs::create_dir(&stdlib).expect("stdlib");
        fs::write(stdlib.join("VERSIONS"), "os: 3.0-\n").expect("versions");
        symlink(outside.path(), stdlib.join("os.pyi")).expect("symlink");
        assert_eq!(
            load_custom_snapshot(root.path().to_str().expect("utf8")).err(),
            Some(BackendError::Custom)
        );
    }
}
