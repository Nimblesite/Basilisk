//! Gradual Adoption Store — manages per-file diagnostic severity overrides.
//!
//! When a user "adopts" a file, Basilisk runs autofix (safe) and then records
//! all remaining error codes per file in `.basilisk/adoptions.toml`. Those
//! codes are demoted from errors to warnings until the user fixes them.
//!
//! As issues are fixed, codes with zero remaining instances are automatically
//! removed ("auto-graduation").

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Error type for adoption operations.
#[derive(Debug)]
pub enum AdoptionError {
    /// Failed to read `adoptions.toml`.
    ReadError(std::io::Error),
    /// Failed to parse `adoptions.toml`.
    ParseError(String),
    /// Failed to write `adoptions.toml`.
    WriteError(std::io::Error),
}

impl std::fmt::Display for AdoptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadError(err) => write!(f, "failed to read adoptions.toml: {err}"),
            Self::ParseError(msg) => write!(f, "failed to parse adoptions.toml: {msg}"),
            Self::WriteError(err) => write!(f, "failed to write adoptions.toml: {err}"),
        }
    }
}

impl std::error::Error for AdoptionError {}

/// Persistent store for per-file diagnostic adoption overrides.
///
/// The store maps file paths (relative to project root) to sets of diagnostic
/// codes that are demoted from errors to warnings in those files.
///
/// # File format
///
/// The store persists to `.basilisk/adoptions.toml`:
///
/// ```toml
/// [overrides."src/utils.py"]
/// demoted = ["BSK-E0001", "BSK-E0003"]
/// ```
#[derive(Debug)]
pub struct AdoptionStore {
    /// Project root directory.
    root: PathBuf,
    /// Per-file overrides: path (relative to root) -> set of demoted codes.
    overrides: BTreeMap<PathBuf, BTreeSet<String>>,
}

impl AdoptionStore {
    /// Load adoption overrides from `.basilisk/adoptions.toml`.
    ///
    /// If the file does not exist, returns an empty store. Only returns an
    /// error if the file exists but cannot be read or parsed.
    ///
    /// # Errors
    ///
    /// Returns `AdoptionError::ReadError` if the file exists but cannot be read,
    /// or `AdoptionError::ParseError` if the TOML is malformed.
    pub fn load(project_root: &Path) -> Result<Self, AdoptionError> {
        let toml_path = project_root.join(".basilisk").join("adoptions.toml");

        if !toml_path.is_file() {
            return Ok(Self {
                root: project_root.to_path_buf(),
                overrides: BTreeMap::new(),
            });
        }

        let content = std::fs::read_to_string(&toml_path).map_err(AdoptionError::ReadError)?;
        let table: toml::Table = content
            .parse()
            .map_err(|err: toml::de::Error| AdoptionError::ParseError(err.to_string()))?;

        let overrides = parse_overrides_table(&table)?;

        Ok(Self {
            root: project_root.to_path_buf(),
            overrides,
        })
    }

    /// Persist the current adoption overrides to `.basilisk/adoptions.toml`.
    ///
    /// Creates the `.basilisk/` directory if it does not already exist. Output
    /// is deterministically ordered by file path and code.
    ///
    /// # Errors
    ///
    /// Returns `AdoptionError::WriteError` if directory creation or file write fails.
    pub fn save(&self) -> Result<(), AdoptionError> {
        let dir = self.root.join(".basilisk");
        std::fs::create_dir_all(&dir).map_err(AdoptionError::WriteError)?;

        let content = format_toml(&self.overrides);
        let toml_path = dir.join("adoptions.toml");
        std::fs::write(toml_path, content).map_err(AdoptionError::WriteError)
    }

    /// Check whether the given diagnostic code is demoted in the given file.
    ///
    /// `file` must be relative to the project root.
    #[must_use]
    pub fn is_demoted(&self, file: &Path, code: &str) -> bool {
        self.overrides
            .get(file)
            .is_some_and(|codes| codes.contains(code))
    }

    /// Record adoption overrides for a file, adding the given codes.
    ///
    /// `file` must be relative to the project root. Codes are merged with
    /// any existing overrides for the file.
    pub fn adopt_file(&mut self, file: &Path, codes: Vec<String>) {
        let entry = self.overrides.entry(file.to_path_buf()).or_default();
        for code in codes {
            let _ = entry.insert(code);
        }
    }

    /// Remove all adoption overrides for a file.
    ///
    /// `file` must be relative to the project root.
    pub fn unadopt_file(&mut self, file: &Path) {
        let _ = self.overrides.remove(file);
    }

    /// Auto-graduate codes that no longer appear in a file.
    ///
    /// For the given file, removes any demoted codes that are **not** present
    /// in `remaining_codes`. If the file has no remaining demotions after
    /// graduation, the file entry is removed entirely.
    pub fn auto_graduate(&mut self, file: &Path, remaining_codes: &BTreeSet<String>) {
        let Some(codes) = self.overrides.get_mut(file) else {
            return;
        };
        codes.retain(|code| remaining_codes.contains(code));
        if codes.is_empty() {
            let _ = self.overrides.remove(file);
        }
    }

    /// Return all files that currently have adoption overrides.
    #[must_use]
    pub fn adopted_files(&self) -> Vec<&Path> {
        self.overrides.keys().map(PathBuf::as_path).collect()
    }

    /// Return the number of demoted codes for a file.
    ///
    /// Returns `0` if the file has no adoptions.
    #[must_use]
    pub fn demoted_count(&self, file: &Path) -> usize {
        self.overrides.get(file).map_or(0, BTreeSet::len)
    }

    /// Check whether the store has any adoptions at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.overrides.is_empty()
    }
}

/// Parse the `[overrides]` section of the TOML table.
fn parse_overrides_table(
    table: &toml::Table,
) -> Result<BTreeMap<PathBuf, BTreeSet<String>>, AdoptionError> {
    let mut result = BTreeMap::new();

    let Some(overrides_val) = table.get("overrides") else {
        return Ok(result);
    };

    let overrides_table = overrides_val
        .as_table()
        .ok_or_else(|| AdoptionError::ParseError("'overrides' must be a table".to_owned()))?;

    for (file_key, file_val) in overrides_table {
        let file_table = file_val.as_table().ok_or_else(|| {
            AdoptionError::ParseError(format!("override entry for '{file_key}' must be a table"))
        })?;

        let demoted_arr = file_table
            .get("demoted")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| {
                AdoptionError::ParseError(format!(
                    "override entry for '{file_key}' must have a 'demoted' array"
                ))
            })?;

        let codes: BTreeSet<String> = demoted_arr
            .iter()
            .filter_map(|val| val.as_str().map(String::from))
            .collect();

        if !codes.is_empty() {
            let _ = result.insert(PathBuf::from(file_key), codes);
        }
    }

    Ok(result)
}

/// Format adoption overrides as TOML with a descriptive header comment.
fn format_toml(overrides: &BTreeMap<PathBuf, BTreeSet<String>>) -> String {
    let mut output = String::from(
        "# Auto-generated by Basilisk Gradual Adoption Mode\n\
         # Remove entries as you fix the underlying issues\n",
    );

    for (file, codes) in overrides {
        let file_str = file.to_string_lossy();
        let _ = write!(output, "\n[overrides.\"{file_str}\"]\n");

        let quoted_codes: Vec<String> = codes.iter().map(|c| format!("\"{c}\"")).collect();
        let _ = writeln!(output, "demoted = [{}]", quoted_codes.join(", "));
    }

    output
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bsk_adoption_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_empty_when_no_file() {
        let dir = temp_dir("no_file");
        let store = AdoptionStore::load(&dir).unwrap();
        assert!(store.is_empty());
        assert!(store.adopted_files().is_empty());
    }

    #[test]
    fn load_empty_when_file_has_no_overrides() {
        let dir = temp_dir("empty_overrides");
        let basilisk_dir = dir.join(".basilisk");
        fs::create_dir_all(&basilisk_dir).unwrap();
        fs::write(basilisk_dir.join("adoptions.toml"), "# empty file\n").unwrap();

        let store = AdoptionStore::load(&dir).unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn load_valid_toml() {
        let dir = temp_dir("valid_toml");
        let basilisk_dir = dir.join(".basilisk");
        fs::create_dir_all(&basilisk_dir).unwrap();
        fs::write(
            basilisk_dir.join("adoptions.toml"),
            r#"
[overrides."src/utils.py"]
demoted = ["BSK-E0001", "BSK-E0003"]

[overrides."src/models/user.py"]
demoted = ["BSK-E0001", "BSK-E0002"]
"#,
        )
        .unwrap();

        let store = AdoptionStore::load(&dir).unwrap();
        assert!(!store.is_empty());
        assert_eq!(store.adopted_files().len(), 2);
        assert!(store.is_demoted(Path::new("src/utils.py"), "BSK-E0001"));
        assert!(store.is_demoted(Path::new("src/utils.py"), "BSK-E0003"));
        assert!(!store.is_demoted(Path::new("src/utils.py"), "BSK-E0002"));
        assert!(store.is_demoted(Path::new("src/models/user.py"), "BSK-E0001"));
        assert!(store.is_demoted(Path::new("src/models/user.py"), "BSK-E0002"));
    }

    #[test]
    fn load_rejects_malformed_toml() {
        let dir = temp_dir("malformed");
        let basilisk_dir = dir.join(".basilisk");
        fs::create_dir_all(&basilisk_dir).unwrap();
        fs::write(
            basilisk_dir.join("adoptions.toml"),
            "this is not valid toml {{{{",
        )
        .unwrap();

        let result = AdoptionStore::load(&dir);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("failed to parse"),
            "error should mention parse failure: {err}"
        );
    }

    #[test]
    fn save_and_reload_roundtrip() {
        let dir = temp_dir("roundtrip");
        let mut store = AdoptionStore::load(&dir).unwrap();

        store.adopt_file(
            Path::new("src/app.py"),
            vec!["BSK-E0002".to_owned(), "BSK-E0001".to_owned()],
        );
        store.adopt_file(Path::new("src/lib.py"), vec!["BSK-E0003".to_owned()]);
        store.save().unwrap();

        let reloaded = AdoptionStore::load(&dir).unwrap();
        assert_eq!(reloaded.adopted_files().len(), 2);
        assert!(reloaded.is_demoted(Path::new("src/app.py"), "BSK-E0001"));
        assert!(reloaded.is_demoted(Path::new("src/app.py"), "BSK-E0002"));
        assert!(reloaded.is_demoted(Path::new("src/lib.py"), "BSK-E0003"));
    }

    #[test]
    fn is_demoted_returns_false_for_unknown_file() {
        let dir = temp_dir("unknown_file");
        let store = AdoptionStore::load(&dir).unwrap();
        assert!(!store.is_demoted(Path::new("nonexistent.py"), "BSK-E0001"));
    }

    #[test]
    fn adopt_file_merges_codes() {
        let dir = temp_dir("merge");
        let mut store = AdoptionStore::load(&dir).unwrap();

        store.adopt_file(Path::new("f.py"), vec!["BSK-E0001".to_owned()]);
        store.adopt_file(Path::new("f.py"), vec!["BSK-E0002".to_owned()]);

        assert_eq!(store.demoted_count(Path::new("f.py")), 2);
        assert!(store.is_demoted(Path::new("f.py"), "BSK-E0001"));
        assert!(store.is_demoted(Path::new("f.py"), "BSK-E0002"));
    }

    #[test]
    fn unadopt_file_removes_all_codes() {
        let dir = temp_dir("unadopt");
        let mut store = AdoptionStore::load(&dir).unwrap();

        store.adopt_file(
            Path::new("f.py"),
            vec!["BSK-E0001".to_owned(), "BSK-E0002".to_owned()],
        );
        assert!(!store.is_empty());

        store.unadopt_file(Path::new("f.py"));
        assert!(store.is_empty());
        assert!(!store.is_demoted(Path::new("f.py"), "BSK-E0001"));
    }

    #[test]
    fn unadopt_nonexistent_file_is_noop() {
        let dir = temp_dir("unadopt_noop");
        let mut store = AdoptionStore::load(&dir).unwrap();
        store.unadopt_file(Path::new("nonexistent.py"));
        assert!(store.is_empty());
    }

    #[test]
    fn auto_graduate_removes_fixed_codes() {
        let dir = temp_dir("graduate");
        let mut store = AdoptionStore::load(&dir).unwrap();

        store.adopt_file(
            Path::new("f.py"),
            vec![
                "BSK-E0001".to_owned(),
                "BSK-E0002".to_owned(),
                "BSK-E0003".to_owned(),
            ],
        );

        // Only BSK-E0002 still has remaining issues.
        let remaining = BTreeSet::from(["BSK-E0002".to_owned()]);
        store.auto_graduate(Path::new("f.py"), &remaining);

        assert_eq!(store.demoted_count(Path::new("f.py")), 1);
        assert!(!store.is_demoted(Path::new("f.py"), "BSK-E0001"));
        assert!(store.is_demoted(Path::new("f.py"), "BSK-E0002"));
        assert!(!store.is_demoted(Path::new("f.py"), "BSK-E0003"));
    }

    #[test]
    fn auto_graduate_removes_file_when_all_fixed() {
        let dir = temp_dir("graduate_all");
        let mut store = AdoptionStore::load(&dir).unwrap();

        store.adopt_file(Path::new("f.py"), vec!["BSK-E0001".to_owned()]);

        let remaining = BTreeSet::new();
        store.auto_graduate(Path::new("f.py"), &remaining);

        assert!(store.is_empty());
        assert_eq!(store.demoted_count(Path::new("f.py")), 0);
    }

    #[test]
    fn auto_graduate_noop_for_unknown_file() {
        let dir = temp_dir("graduate_noop");
        let mut store = AdoptionStore::load(&dir).unwrap();
        let remaining = BTreeSet::new();
        store.auto_graduate(Path::new("nonexistent.py"), &remaining);
        assert!(store.is_empty());
    }

    #[test]
    #[expect(
        clippy::indexing_slicing,
        reason = "test asserts — panic on OOB is the desired behavior"
    )]
    fn adopted_files_returns_sorted_paths() {
        let dir = temp_dir("adopted_files");
        let mut store = AdoptionStore::load(&dir).unwrap();

        store.adopt_file(Path::new("z.py"), vec!["BSK-E0001".to_owned()]);
        store.adopt_file(Path::new("a.py"), vec!["BSK-E0001".to_owned()]);
        store.adopt_file(Path::new("m.py"), vec!["BSK-E0001".to_owned()]);

        let files = store.adopted_files();
        assert_eq!(files.len(), 3);
        // BTreeMap guarantees sorted order.
        assert_eq!(files[0], Path::new("a.py"));
        assert_eq!(files[1], Path::new("m.py"));
        assert_eq!(files[2], Path::new("z.py"));
    }

    #[test]
    fn demoted_count_zero_for_unknown_file() {
        let dir = temp_dir("count_zero");
        let store = AdoptionStore::load(&dir).unwrap();
        assert_eq!(store.demoted_count(Path::new("nonexistent.py")), 0);
    }

    #[test]
    fn save_creates_basilisk_directory() {
        let dir = temp_dir("create_dir");
        let store = AdoptionStore::load(&dir).unwrap();
        store.save().unwrap();
        assert!(dir.join(".basilisk").join("adoptions.toml").is_file());
    }

    #[test]
    fn saved_toml_contains_header_comment() {
        let dir = temp_dir("header");
        let mut store = AdoptionStore::load(&dir).unwrap();
        store.adopt_file(Path::new("f.py"), vec!["BSK-E0001".to_owned()]);
        store.save().unwrap();

        let content = fs::read_to_string(dir.join(".basilisk").join("adoptions.toml")).unwrap();
        assert!(content.contains("Auto-generated by Basilisk"));
        assert!(content.contains("Remove entries as you fix"));
    }

    #[test]
    fn saved_toml_codes_are_sorted() {
        let dir = temp_dir("sorted_codes");
        let mut store = AdoptionStore::load(&dir).unwrap();
        store.adopt_file(
            Path::new("f.py"),
            vec![
                "BSK-E0003".to_owned(),
                "BSK-E0001".to_owned(),
                "BSK-E0002".to_owned(),
            ],
        );
        store.save().unwrap();

        let content = fs::read_to_string(dir.join(".basilisk").join("adoptions.toml")).unwrap();
        let demoted_line = content
            .lines()
            .find(|line| line.starts_with("demoted"))
            .unwrap();
        assert_eq!(
            demoted_line,
            r#"demoted = ["BSK-E0001", "BSK-E0002", "BSK-E0003"]"#
        );
    }
}
