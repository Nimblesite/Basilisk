//! Parser and target admission for typeshed's `stdlib/VERSIONS` file.

use std::collections::BTreeMap;

/// An inclusive Python-version range from `stdlib/VERSIONS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionRange {
    /// First supported `(major, minor)` version.
    pub minimum: (u32, u32),
    /// Last supported version, or `None` when the range is open-ended.
    pub maximum: Option<(u32, u32)>,
}

impl VersionRange {
    /// Whether the concrete target is inside this inclusive range.
    #[must_use]
    pub fn contains(self, target: (u32, u32)) -> bool {
        target >= self.minimum && self.maximum.is_none_or(|maximum| target <= maximum)
    }
}

/// Parsed, immutable target-admission index for one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionsIndex {
    ranges: BTreeMap<String, VersionRange>,
}

impl VersionsIndex {
    /// Admit every supplied module for every target. Custom user-managed trees
    /// use this when they omit typeshed's optional `stdlib/VERSIONS` convention.
    pub fn from_modules<'a>(modules: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            ranges: modules
                .into_iter()
                .map(|module| {
                    (
                        module.to_owned(),
                        VersionRange {
                            minimum: (0, 0),
                            maximum: None,
                        },
                    )
                })
                .collect(),
        }
    }

    /// Parse the exact `stdlib/VERSIONS` text carried by a snapshot.
    ///
    /// # Errors
    ///
    /// Rejects malformed or duplicate module rows and invalid ranges.
    pub fn parse(source: &str) -> Result<Self, VersionsError> {
        let mut ranges = BTreeMap::new();
        for (offset, raw) in source.lines().enumerate() {
            let line_number = offset + 1;
            let line = raw.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            let (module, raw_range) =
                line.split_once(':')
                    .ok_or_else(|| VersionsError::MalformedLine {
                        line: line_number,
                        text: line.to_owned(),
                    })?;
            let module = module.trim();
            if !valid_module_name(module) {
                return Err(VersionsError::InvalidModule {
                    line: line_number,
                    module: module.to_owned(),
                });
            }
            let range = parse_range(raw_range.trim(), line_number)?;
            if ranges.insert(module.to_owned(), range).is_some() {
                return Err(VersionsError::DuplicateModule(module.to_owned()));
            }
        }
        if ranges.is_empty() {
            return Err(VersionsError::Empty);
        }
        Ok(Self { ranges })
    }

    /// The range for a module. Unlisted submodules inherit the nearest listed
    /// parent's lifetime, exactly as documented by `stdlib/VERSIONS`.
    #[must_use]
    pub fn range_for(&self, module: &str) -> Option<VersionRange> {
        let mut candidate = module;
        loop {
            if let Some(range) = self.ranges.get(candidate) {
                return Some(*range);
            }
            candidate = candidate.rsplit_once('.').map(|(parent, _)| parent)?;
        }
    }

    /// Whether `module` is admitted for a concrete Python target.
    #[must_use]
    pub fn admits(&self, module: &str, target: (u32, u32)) -> bool {
        self.range_for(module)
            .is_some_and(|range| range.contains(target))
    }

    /// Iterate every explicitly listed module and its range.
    pub fn iter(&self) -> impl Iterator<Item = (&str, VersionRange)> {
        self.ranges
            .iter()
            .map(|(module, range)| (module.as_str(), *range))
    }
}

/// Invalid `stdlib/VERSIONS` input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VersionsError {
    /// No data rows were present.
    #[error("stdlib/VERSIONS contains no module rows")]
    Empty,
    /// A row did not have the required `module: range` form.
    #[error("malformed stdlib/VERSIONS line {line}: {text}")]
    MalformedLine {
        /// One-based source line.
        line: usize,
        /// The non-comment source text.
        text: String,
    },
    /// A module name was not a dotted Python identifier.
    #[error("invalid module name on stdlib/VERSIONS line {line}: {module}")]
    InvalidModule {
        /// One-based source line.
        line: usize,
        /// Invalid module name.
        module: String,
    },
    /// A module appeared more than once.
    #[error("duplicate stdlib/VERSIONS module: {0}")]
    DuplicateModule(String),
    /// A version or range was malformed.
    #[error("invalid version range on stdlib/VERSIONS line {line}: {range}")]
    InvalidRange {
        /// One-based source line.
        line: usize,
        /// Invalid range text.
        range: String,
    },
}

fn parse_range(source: &str, line: usize) -> Result<VersionRange, VersionsError> {
    let invalid = || VersionsError::InvalidRange {
        line,
        range: source.to_owned(),
    };
    let (minimum, maximum) = source.split_once('-').ok_or_else(&invalid)?;
    let minimum = parse_version(minimum).ok_or_else(&invalid)?;
    let maximum = if maximum.is_empty() {
        None
    } else {
        Some(parse_version(maximum).ok_or_else(&invalid)?)
    };
    if maximum.is_some_and(|value| value < minimum) {
        return Err(invalid());
    }
    Ok(VersionRange { minimum, maximum })
}

fn parse_version(source: &str) -> Option<(u32, u32)> {
    let (major, minor) = source.trim().split_once('.')?;
    if major.is_empty() || minor.is_empty() || minor.contains('.') {
        return None;
    }
    Some((major.parse().ok()?, minor.parse().ok()?))
}

fn valid_module_name(module: &str) -> bool {
    !module.is_empty()
        && module.split('.').all(|segment| {
            let mut chars = segment.chars();
            chars
                .next()
                .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
                && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
        })
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test-only fixed VERSIONS fixture must fail loudly"
)]
mod tests {
    use super::*;

    #[test]
    fn parses_comments_bounds_and_parent_inheritance() {
        let index =
            VersionsIndex::parse("# comment\nasyncio: 3.4-\nasyncio.tasks: 3.8-3.12 # inline\n")
                .expect("valid fixture");
        assert!(index.admits("asyncio.runners", (3, 7)));
        assert!(!index.admits("asyncio.tasks", (3, 7)));
        assert!(index.admits("asyncio.tasks", (3, 12)));
        assert!(!index.admits("asyncio.tasks", (3, 13)));
    }

    #[test]
    fn rejects_empty_duplicate_and_reversed_ranges() {
        assert_eq!(
            VersionsIndex::parse("# only a comment\n"),
            Err(VersionsError::Empty)
        );
        assert!(matches!(
            VersionsIndex::parse("os: 3.0-\nos: 3.1-\n"),
            Err(VersionsError::DuplicateModule(module)) if module == "os"
        ));
        assert!(matches!(
            VersionsIndex::parse("os: 3.12-3.8\n"),
            Err(VersionsError::InvalidRange { .. })
        ));
    }

    #[test]
    fn custom_module_index_can_admit_present_paths_without_versions_file() {
        let index = VersionsIndex::from_modules(["micropython", "uasyncio"]);
        assert!(index.admits("micropython", (3, 4)));
        assert!(index.admits("uasyncio", (99, 0)));
        assert!(!index.admits("os", (3, 12)));
    }
}
