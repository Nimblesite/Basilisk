//! Implements [STUBRES-CONFIG]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-CONFIG
//! Override types for per-module and per-path configuration.

use std::collections::HashMap;

/// Severity level for a diagnostic rule override.
///
/// Implements [CHKARCH-STRICTNESS-SEVERITY] (config side): the four modes a rule
/// can be set to — `error`, `warning`, `info`, `disabled`. The checker applies
/// them in `basilisk-checker/src/lib.rs`.
/// See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS-SEVERITY
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum RuleSeverity {
    /// Full error (default for most rules).
    Error,
    /// Downgraded to warning.
    Warning,
    /// Downgraded to informational hint.
    Info,
    /// Rule is completely disabled.
    Disabled,
}

impl RuleSeverity {
    /// Parse a severity string from config.
    ///
    /// Implements [CHKARCH-STRICTNESS-SEVERITY]: maps the spec's four mode names
    /// (plus the documented aliases `warn`/`information`/`off`/`none`) to a mode.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "error" => Some(Self::Error),
            "warning" | "warn" => Some(Self::Warning),
            "info" | "information" => Some(Self::Info),
            "disabled" | "off" | "none" => Some(Self::Disabled),
            _ => None,
        }
    }
}

/// Per-module override configuration.
///
/// Applied when the imported module name matches the override key.
/// Keys support wildcard patterns (e.g. `django.*` matches `django.db.models`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModuleOverride {
    /// When `true`, `imports_unresolved` is suppressed for this module.
    pub ignore_missing_stubs: bool,
}

/// Per-path override configuration.
///
/// Applied when the file path matches the override key pattern.
/// Keys use glob patterns (e.g. `vendor/**` matches `vendor/lib/foo.py`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct PathOverride {
    /// Rules to completely disable for files matching this path pattern.
    pub disabled_rules: Vec<String>,
    /// Rule severity overrides for files matching this path pattern.
    pub rule_overrides: HashMap<String, RuleSeverity>,
}

/// Check whether a module name matches an override pattern.
///
/// Patterns:
/// - `"fastmcp"` — matches `fastmcp` and `fastmcp.anything`
/// - `"django.*"` — matches `django.db`, `django.db.models`, etc.
/// - Exact match always wins over wildcard.
#[must_use]
pub fn module_matches_pattern(module_name: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix(".*") {
        // Wildcard: match the prefix and any dotted children.
        module_name == prefix
            || module_name.starts_with(prefix)
                && module_name
                    .as_bytes()
                    .get(prefix.len())
                    .is_some_and(|&b| b == b'.')
    } else {
        // Exact match: also match dotted children (e.g. `fastmcp` matches `fastmcp.server`).
        module_name == pattern
            || module_name.starts_with(pattern)
                && module_name
                    .as_bytes()
                    .get(pattern.len())
                    .is_some_and(|&b| b == b'.')
    }
}

/// Split a `/`-separated path or pattern into its meaningful segments,
/// dropping empty components (leading `/`, doubled `//`, trailing `/`) and `.`.
fn path_segments(value: &str) -> Vec<&str> {
    value
        .split('/')
        .filter(|seg| !seg.is_empty() && *seg != ".")
        .collect()
}

/// Glob-match a single path segment, where `*` matches any run of characters
/// (never `/`, which can't appear in a segment) and `?` matches exactly one.
///
/// Classic linear wildcard match with backtracking on the most recent `*`.
fn segment_matches(pattern: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t) = (0_usize, 0_usize);
    let (mut star, mut resume) = (None, 0_usize);
    while let Some(&tc) = text.get(t) {
        match pattern.get(p) {
            Some(b'*') => {
                star = Some(p);
                resume = t;
                p += 1;
            }
            Some(b'?') => {
                p += 1;
                t += 1;
            }
            Some(&c) if c == tc => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some(star_pos) => {
                    p = star_pos + 1;
                    resume += 1;
                    t = resume;
                }
                None => return false,
            },
        }
    }
    pattern.iter().skip(p).all(|&c| c == b'*')
}

/// Match `path` segments against `pattern` segments, where a `**` pattern
/// segment matches zero or more path segments and any other segment is matched
/// with [`segment_matches`].
fn segments_match(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| {
            path.get(skip..)
                .is_some_and(|tail| segments_match(rest, tail))
        }),
        Some((seg, rest)) => match path.split_first() {
            Some((head, tail)) if segment_matches(seg.as_bytes(), head.as_bytes()) => {
                segments_match(rest, tail)
            }
            _ => false,
        },
    }
}

/// Check whether a file path matches a glob path pattern (gitignore-style).
///
/// Implements [CHKARCH-CONFIG-EXCLUDE]. See
/// docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-EXCLUDE
///
/// Backslashes are normalised to `/`. Semantics:
/// - a bare name (no `/`) matches that name at **any** depth — `build` excludes
///   every `build` directory in the tree, `*.pb.py` every generated file;
/// - `**` matches zero or more directory segments (`**/bundled/**` matches a
///   `bundled` directory anywhere), `*`/`?` match within a single segment;
/// - an anchored pattern (one containing `/`) matches the full path or any of
///   its ancestor directories, so a directory pattern (`vendor/**`, `src/gen`)
///   also excludes everything beneath it.
#[must_use]
pub fn path_matches_pattern(file_path: &std::path::Path, pattern: &str) -> bool {
    let path_normalized = file_path.to_string_lossy().replace('\\', "/");
    let pattern_normalized = pattern.replace('\\', "/");
    let path_segs = path_segments(&path_normalized);
    let pattern_segs = path_segments(&pattern_normalized);

    let Some((&first, rest)) = pattern_segs.split_first() else {
        return false;
    };
    // Bare name (no `/`): match it at any depth, gitignore-style.
    if rest.is_empty() && first != "**" {
        return path_segs
            .iter()
            .any(|seg| segment_matches(first.as_bytes(), seg.as_bytes()));
    }
    // Anchored pattern: match the full path or any ancestor directory of it.
    (0..=path_segs.len()).any(|len| {
        path_segs
            .get(..len)
            .is_some_and(|prefix| segments_match(&pattern_segs, prefix))
    })
}

/// Find applicable module override for a given module name.
///
/// Returns the override configuration if any per-module pattern matches.
/// Implements the per-module step of [CHKARCH-STRICTNESS-PRECEDENCE] (rung 5).
#[must_use]
pub fn find_module_override<'a, S: std::hash::BuildHasher>(
    module_name: &str,
    overrides: &'a HashMap<String, ModuleOverride, S>,
) -> Option<&'a ModuleOverride> {
    // Exact match first, then wildcard patterns.
    if let Some(entry) = overrides.get(module_name) {
        return Some(entry);
    }
    overrides
        .iter()
        .find(|(pattern, _)| module_matches_pattern(module_name, pattern))
        .map(|(_, entry)| entry)
}

/// Find applicable path override for a given file path.
///
/// Returns the override configuration if any per-path pattern matches.
/// Implements the per-path step of [CHKARCH-STRICTNESS-PRECEDENCE] (rung 4,
/// above per-module and global, below inline/file directives).
/// See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-STRICTNESS-PRECEDENCE
#[must_use]
pub fn find_path_override<'a, S: std::hash::BuildHasher>(
    file_path: &std::path::Path,
    overrides: &'a HashMap<String, PathOverride, S>,
) -> Option<&'a PathOverride> {
    overrides
        .iter()
        .find(|(pattern, _)| path_matches_pattern(file_path, pattern))
        .map(|(_, entry)| entry)
}

/// Check whether a rule is disabled for a given file path.
#[must_use]
pub fn is_rule_disabled_for_path<S: std::hash::BuildHasher>(
    rule_code: &str,
    file_path: &std::path::Path,
    overrides: &HashMap<String, PathOverride, S>,
) -> bool {
    find_path_override(file_path, overrides)
        .is_some_and(|o| o.disabled_rules.iter().any(|r| r == rule_code))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::path_matches_pattern;

    fn matches(path: &str, pattern: &str) -> bool {
        path_matches_pattern(Path::new(path), pattern)
    }

    #[test]
    fn bare_name_matches_segment_at_any_depth() {
        assert!(matches("build/lib.py", "build"));
        assert!(matches("a/b/build/c.py", "build"));
        assert!(matches("build", "build"));
        // ...but only on a full segment, never a substring of one.
        assert!(!matches("buildkite/lib.py", "build"));
        assert!(!matches("src/main.py", "build"));
    }

    #[test]
    fn double_star_matches_directory_anywhere() {
        // The exact shape requested in issue #80.
        assert!(matches(
            "vscode-extension/bundled/debugpy/peb_teb.py",
            "**/bundled/**"
        ));
        assert!(matches("pkg/_vendored/pydevd/x.py", "**/_vendored/**"));
        assert!(!matches("pkg/vendored_stuff/x.py", "**/_vendored/**"));
    }

    #[test]
    fn trailing_double_star_matches_dir_and_subtree() {
        assert!(matches("vendor", "vendor/**")); // the directory itself
        assert!(matches("vendor/lib/foo.py", "vendor/**")); // and its subtree
        assert!(!matches("vendorx/foo.py", "vendor/**")); // segment, not prefix
    }

    #[test]
    fn anchored_pattern_matches_full_path_and_ancestors() {
        // Absolute prefix (how per-path-overrides are exercised in e2e tests).
        assert!(matches(
            "/abs/proj/tests/fixtures/x.py",
            "/abs/proj/tests/fixtures"
        ));
        // Multi-segment relative directory excludes its subtree.
        assert!(matches("src/generated/models.py", "src/generated"));
        assert!(!matches("src/generatedx/models.py", "src/generated"));
    }

    #[test]
    fn single_segment_wildcards_match_anywhere() {
        assert!(matches("schema.pb.py", "*.pb.py"));
        assert!(matches("api/v1/schema.pb.py", "*.pb.py"));
        assert!(!matches("api/app.py", "*.pb.py"));
    }

    #[test]
    fn star_matches_exactly_one_segment() {
        // A single `*` segment spans one directory level — unlike `**`.
        assert!(matches("a/b/c.py", "a/*/c.py"));
        assert!(!matches("a/b/d/c.py", "a/*/c.py"));
    }

    #[test]
    fn empty_pattern_never_matches() {
        assert!(!matches("anything.py", ""));
    }
}
