//! Implements [STUBRES-TYPESHED-DOWNLOAD] ZIP decode. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-TYPESHED-DOWNLOAD
//!
//! ZIP → [`Archive`] decoding with streaming zip-bomb caps.
//!
//! Both the downloaded GitHub zipball and the bundled snapshot arrive as a ZIP.
//! The decoder enforces entry-count, per-entry, **total**, and compression-ratio
//! caps *while reading each entry*, so an inflated entry is bounded before it is
//! fully allocated — a later gate over a fully-inflated archive would be too late
//! ([STUBRES-TYPESHED-DOWNLOAD]). Encrypted entries are rejected outright. When a
//! top-level prefix is stripped (GitHub zipballs nest under `typeshed-<sha>/`),
//! the decoder requires a single coherent non-`..` common root, so a mixed-root
//! or `../evil`-prefixed archive can never normalize into a clean path. Path
//! safety and content attestation still run afterwards as their own gates.

use std::io::{Cursor, Read};

use zip::read::ZipFile;
use zip::ZipArchive;

use super::archive::{Archive, ArchiveEntry};
use super::gittree::FileMode;

/// Caps enforced during decompression.
#[derive(Debug, Clone, Copy)]
pub struct DecodeLimits {
    /// Maximum number of ZIP entries (including directories).
    pub max_entries: usize,
    /// Maximum decompressed size of any single entry, in bytes.
    pub max_entry_bytes: u64,
    /// Maximum decompressed size across all entries, in bytes.
    pub max_total_bytes: u64,
    /// Maximum per-entry decompressed/compressed ratio (zip-bomb defence).
    pub max_ratio: u64,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_entries: 200_000,
            max_entry_bytes: 64 * 1024 * 1024,
            max_total_bytes: 512 * 1024 * 1024,
            max_ratio: 1_000,
        }
    }
}

/// How the archive's paths are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipLayout {
    /// A GitHub zipball: everything nests under one `typeshed-<sha>/` root.
    CodeloadPrefixed,
    /// The bundled snapshot: entries are rootless (`stdlib/…`, `LICENSE`).
    BundledRootless,
}

/// A ZIP decode failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// The ZIP container could not be opened or an entry could not be read.
    #[error("zip error: {0}")]
    Zip(String),
    /// An entry was encrypted; encrypted typeshed archives are never expected.
    #[error("encrypted entry is not permitted: {0}")]
    Encrypted(String),
    /// The ZIP had more entries than allowed.
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
        /// Its (declared or read) decompressed size.
        size: u64,
        /// Allowed maximum.
        limit: u64,
    },
    /// The total decompressed size exceeded the limit.
    #[error("archive too large: exceeds {limit} bytes")]
    TotalTooLarge {
        /// Allowed maximum.
        limit: u64,
    },
    /// One entry's compression ratio exceeded the limit.
    #[error("entry {path} compression ratio {ratio} exceeds {limit}")]
    RatioExceeded {
        /// The offending entry.
        path: String,
        /// Its decompressed/compressed ratio.
        ratio: u64,
        /// Allowed maximum.
        limit: u64,
    },
    /// A prefixed archive's top-level root was empty, `.`, or `..`.
    #[error("unsafe top-level prefix: {0}")]
    BadPrefix(String),
    /// A prefixed archive did not share one common top-level root.
    #[error("archive entries do not share one common top-level root")]
    MixedRoots,
    /// A prefixed archive had no file entries to derive a root from.
    #[error("archive has no file entries")]
    EmptyArchive,
}

/// Decode ZIP bytes into an [`Archive`] under the given [`ZipLayout`].
///
/// Directories are dropped; only file entries are kept.
///
/// # Errors
///
/// Returns a [`DecodeError`] if the container is invalid, an entry is encrypted,
/// any cap is breached, or a prefixed archive lacks one coherent common root.
pub fn decode_zip(
    bytes: &[u8],
    layout: ZipLayout,
    limits: &DecodeLimits,
) -> Result<Archive, DecodeError> {
    let mut zip =
        ZipArchive::new(Cursor::new(bytes)).map_err(|err| DecodeError::Zip(err.to_string()))?;
    if zip.len() > limits.max_entries {
        return Err(DecodeError::TooManyEntries {
            count: zip.len(),
            limit: limits.max_entries,
        });
    }
    let mut raw: Vec<(String, FileMode, Vec<u8>)> = Vec::new();
    let mut total: u64 = 0;
    for index in 0..zip.len() {
        let mut file = zip
            .by_index(index)
            .map_err(|err| DecodeError::Zip(err.to_string()))?;
        if file.is_dir() {
            continue;
        }
        if file.encrypted() {
            return Err(DecodeError::Encrypted(file.name().to_owned()));
        }
        let raw_name = file.name().to_owned();
        let mode = mode_from_unix(file.unix_mode());
        let remaining = limits.max_total_bytes.saturating_sub(total);
        let data = read_entry_data(&mut file, &raw_name, limits, remaining)?;
        total = total.saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
        raw.push((raw_name, mode, data));
    }
    let prefix = match layout {
        ZipLayout::CodeloadPrefixed => {
            Some(common_root(raw.iter().map(|(name, _, _)| name.as_str()))?)
        }
        ZipLayout::BundledRootless => None,
    };
    let entries = raw
        .into_iter()
        .map(|(name, mode, data)| ArchiveEntry {
            path: prefix
                .as_deref()
                .map_or_else(|| name.clone(), |root| strip_root(&name, root)),
            mode,
            data,
        })
        .collect();
    Ok(Archive::new(entries))
}

/// Read one entry's bytes with per-entry, total, and ratio caps applied during
/// the read, plus a cheap declared-size ratio preflight before inflating.
fn read_entry_data<R: Read>(
    file: &mut ZipFile<'_, R>,
    path: &str,
    limits: &DecodeLimits,
    remaining_total: u64,
) -> Result<Vec<u8>, DecodeError> {
    let declared = file.size();
    if declared > limits.max_entry_bytes {
        return Err(DecodeError::EntryTooLarge {
            path: path.to_owned(),
            size: declared,
            limit: limits.max_entry_bytes,
        });
    }
    let compressed = file.compressed_size();
    if declared > compressed.saturating_mul(limits.max_ratio) {
        return Err(DecodeError::RatioExceeded {
            path: path.to_owned(),
            ratio: declared / compressed.max(1),
            limit: limits.max_ratio,
        });
    }
    // Read at most one byte past the smaller of the per-entry and remaining-total
    // caps, so an over-cap entry is detected without allocating its full size.
    let read_cap = limits
        .max_entry_bytes
        .min(remaining_total)
        .saturating_add(1);
    let mut data = Vec::new();
    let _ = file
        .by_ref()
        .take(read_cap)
        .read_to_end(&mut data)
        .map_err(|err| DecodeError::Zip(err.to_string()))?;
    let size = u64::try_from(data.len()).unwrap_or(u64::MAX);
    if size > limits.max_entry_bytes {
        return Err(DecodeError::EntryTooLarge {
            path: path.to_owned(),
            size,
            limit: limits.max_entry_bytes,
        });
    }
    if size > remaining_total {
        return Err(DecodeError::TotalTooLarge {
            limit: limits.max_total_bytes,
        });
    }
    Ok(data)
}

/// Require and return the single common top-level root across every entry.
fn common_root<'a>(names: impl Iterator<Item = &'a str>) -> Result<String, DecodeError> {
    let mut root: Option<String> = None;
    let mut any = false;
    for name in names {
        any = true;
        let first = name.split('/').next().unwrap_or("");
        if first.is_empty() || first == "." || first == ".." {
            return Err(DecodeError::BadPrefix(name.to_owned()));
        }
        match &root {
            None => root = Some(first.to_owned()),
            Some(existing) if existing == first => {}
            Some(_) => return Err(DecodeError::MixedRoots),
        }
    }
    if !any {
        return Err(DecodeError::EmptyArchive);
    }
    root.ok_or(DecodeError::EmptyArchive)
}

/// Strip exactly `root/` from the front of a name.
fn strip_root(name: &str, root: &str) -> String {
    name.strip_prefix(root)
        .and_then(|rest| rest.strip_prefix('/'))
        .map_or_else(|| name.to_owned(), str::to_owned)
}

/// Map a ZIP entry's unix mode to a [`FileMode`]. Symlinks and gitlinks are
/// classified so the Safety gate can reject them.
fn mode_from_unix(mode: Option<u32>) -> FileMode {
    match mode {
        Some(bits) if bits & 0o170_000 == 0o120_000 => FileMode::Symlink,
        Some(bits) if bits & 0o170_000 == 0o160_000 => FileMode::Submodule,
        Some(bits) if bits & 0o111 != 0 => FileMode::Executable,
        _ => FileMode::Regular,
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use zip::write::{SimpleFileOptions, ZipWriter};
    use zip::CompressionMethod;

    fn zip_with(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut writer = ZipWriter::new(Cursor::new(&mut buf));
            for (name, data, mode) in entries {
                let options = SimpleFileOptions::default()
                    .compression_method(CompressionMethod::Stored)
                    .unix_permissions(*mode);
                writer.start_file(*name, options).unwrap();
                writer.write_all(data).unwrap();
            }
            let _ = writer.finish().unwrap();
        }
        buf
    }

    #[test]
    fn decodes_and_strips_common_prefix() {
        let bytes = zip_with(&[
            (
                "typeshed-abc/stdlib/os.pyi",
                b"def getcwd() -> str: ...\n",
                0o644,
            ),
            ("typeshed-abc/stdlib/VERSIONS", b"os: 3.0-\n", 0o644),
            ("typeshed-abc/LICENSE", b"composite\n", 0o644),
        ]);
        let archive = decode_zip(
            &bytes,
            ZipLayout::CodeloadPrefixed,
            &DecodeLimits::default(),
        )
        .unwrap();
        let mut paths: Vec<&str> = archive.entries().iter().map(|e| e.path.as_str()).collect();
        paths.sort_unstable();
        assert_eq!(paths, vec!["LICENSE", "stdlib/VERSIONS", "stdlib/os.pyi"]);
    }

    #[test]
    fn mixed_roots_and_dotdot_prefix_are_rejected() {
        let mixed = zip_with(&[("a/x.pyi", b"1", 0o644), ("b/y.pyi", b"2", 0o644)]);
        assert!(matches!(
            decode_zip(
                &mixed,
                ZipLayout::CodeloadPrefixed,
                &DecodeLimits::default()
            ),
            Err(DecodeError::MixedRoots)
        ));
        let evil = zip_with(&[("../evil.pyi", b"1", 0o644)]);
        assert!(matches!(
            decode_zip(&evil, ZipLayout::CodeloadPrefixed, &DecodeLimits::default()),
            Err(DecodeError::BadPrefix(_))
        ));
    }

    #[test]
    fn round_trip_regular_mode() {
        let bytes = zip_with(&[("plain.pyi", b"x\n", 0o644)]);
        let archive =
            decode_zip(&bytes, ZipLayout::BundledRootless, &DecodeLimits::default()).unwrap();
        assert_eq!(
            archive.get("plain.pyi").map(|e| e.mode),
            Some(FileMode::Regular)
        );
    }

    #[test]
    fn mode_from_unix_classifies_every_kind() {
        // The ZIP writer masks the file-type bits, so classify the helper directly.
        assert_eq!(mode_from_unix(Some(0o120_777)), FileMode::Symlink);
        assert_eq!(mode_from_unix(Some(0o160_000)), FileMode::Submodule);
        assert_eq!(mode_from_unix(Some(0o100_755)), FileMode::Executable);
        assert_eq!(mode_from_unix(Some(0o100_644)), FileMode::Regular);
        assert_eq!(mode_from_unix(None), FileMode::Regular);
    }

    #[test]
    fn enforces_per_entry_cap_during_read() {
        let bytes = zip_with(&[("big.pyi", &vec![b'x'; 4096], 0o644)]);
        let limits = DecodeLimits {
            max_entry_bytes: 1024,
            ..DecodeLimits::default()
        };
        assert!(matches!(
            decode_zip(&bytes, ZipLayout::BundledRootless, &limits),
            Err(DecodeError::EntryTooLarge { .. })
        ));
    }

    #[test]
    fn enforces_total_cap_during_read() {
        let bytes = zip_with(&[("a", b"xxxx", 0o644), ("b", b"yyyy", 0o644)]);
        let limits = DecodeLimits {
            max_total_bytes: 5,
            ..DecodeLimits::default()
        };
        assert!(matches!(
            decode_zip(&bytes, ZipLayout::BundledRootless, &limits),
            Err(DecodeError::TotalTooLarge { .. })
        ));
    }

    #[test]
    fn enforces_entry_count_cap() {
        let bytes = zip_with(&[("a", b"x", 0o644), ("b", b"y", 0o644)]);
        let limits = DecodeLimits {
            max_entries: 1,
            ..DecodeLimits::default()
        };
        assert!(matches!(
            decode_zip(&bytes, ZipLayout::BundledRootless, &limits),
            Err(DecodeError::TooManyEntries { .. })
        ));
    }
}
