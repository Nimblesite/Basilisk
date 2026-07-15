//! Implements [CHKARCH-CONFIG-EXCLUDE]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFIG-EXCLUDE
//!
//! Gitignore-style glob matching for `exclude` entries. Globs select files;
//! they never scope rules ([CHKARCH-CONFIG-MODEL]).

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
    // Multi-segment patterns are anchored to the project-relative path. Try
    // each ancestor prefix so a directory entry selects its subtree, while an
    // exact file entry cannot match the same suffix under another directory.
    (1..=path_segs.len()).any(|end| {
        path_segs
            .get(..end)
            .is_some_and(|candidate| segments_match(&pattern_segs, candidate))
    })
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
        assert!(matches(
            "/abs/proj/tests/fixtures/x.py",
            "/abs/proj/tests/fixtures"
        ));
        // Multi-segment relative directory excludes its subtree.
        assert!(matches("src/generated/models.py", "src/generated"));
        assert!(!matches("src/generatedx/models.py", "src/generated"));
    }

    #[test]
    fn root_relative_exact_file_is_anchored() {
        assert!(matches("src/app.py", "src/app.py"));
        assert!(!matches("src/other.py", "src/app.py"));
        assert!(!matches("vendor/src/app.py", "src/app.py"));
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

    #[test]
    fn question_mark_matches_exactly_one_character() {
        assert!(matches("cache_a/x.py", "cache_?"));
        assert!(matches("v1/data.py", "v?"));
        // `?` requires exactly one character — never zero, never two.
        assert!(!matches("cache_/x.py", "cache_?"));
        assert!(!matches("cache_ab/x.py", "cache_?"));
        // `?` composes with `*` backtracking inside one segment.
        assert!(matches("report_2024_final.py", "report_?024*.py"));
    }
}
