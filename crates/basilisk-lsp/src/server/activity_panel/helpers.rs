//! Implements [LSPARCH-ARCH-MODSTRUCT]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-ARCH-MODSTRUCT
//!
//! Shared utilities for the activity panel handlers.

use std::path::Path;

/// Derive a dotted Python module name from a file path relative to workspace roots.
pub(crate) fn module_name_from_path(path: &Path, roots: &[std::path::PathBuf]) -> String {
    let relative = roots
        .iter()
        .find_map(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);

    let mut parts: Vec<&str> = relative
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    // Strip the file extension from the last component.
    if let Some(last) = parts.last_mut() {
        if let Some(stem) = last
            .strip_suffix(".py")
            .or_else(|| last.strip_suffix(".pyi"))
        {
            // __init__.py → use the package name (drop __init__).
            if stem == "__init__" {
                let _ = parts.pop();
            } else {
                *last = stem;
            }
        }
    }

    parts.join(".")
}

/// Convert a byte offset to a 0-based line number.
pub(crate) fn byte_offset_to_line(text: &str, offset: u32) -> usize {
    let offset = usize::try_from(offset).unwrap_or(0);
    text[..offset.min(text.len())]
        .chars()
        .filter(|&c| c == '\n')
        .count()
}

/// Compute coverage percentage without raw `as` casts.
///
/// Symbol counts are far below 2^52 so f64 precision loss is negligible.
/// The result is always 0..=100 so truncation/sign-loss is impossible.
#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "symbol counts < 2^52 and result is 0..=100"
)]
pub(crate) fn coverage_percent(annotated: usize, total: usize) -> u64 {
    if total == 0 {
        return 100;
    }
    (annotated as f64 / total as f64 * 100.0).round() as u64
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    // ── module_name_from_path ─────────────────────────────────────────────

    #[test]
    fn test_module_name_regular_py_file() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/pkg/sub/module.py");
        assert_eq!(module_name_from_path(&path, &roots), "pkg.sub.module");
    }

    #[test]
    fn test_module_name_init_py_becomes_package() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/pkg/sub/__init__.py");
        assert_eq!(module_name_from_path(&path, &roots), "pkg.sub");
    }

    #[test]
    fn test_module_name_pyi_stub_file() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/pkg/types.pyi");
        assert_eq!(module_name_from_path(&path, &roots), "pkg.types");
    }

    #[test]
    fn test_module_name_outside_roots_uses_full_components() {
        let roots = vec![PathBuf::from("/other")];
        let path = PathBuf::from("/workspace/pkg/module.py");
        // Path cannot be stripped from any root, so all path components
        // (including the root `/`) are joined with dots.
        let name = module_name_from_path(&path, &roots);
        assert!(
            name.contains("workspace"),
            "expected 'workspace' in module name, got: {name}"
        );
        assert!(
            name.contains("pkg"),
            "expected 'pkg' in module name, got: {name}"
        );
        assert!(
            name.ends_with("module"),
            "expected module name to end with 'module', got: {name}"
        );
    }

    #[test]
    fn test_module_name_init_pyi_becomes_package() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/pkg/__init__.pyi");
        assert_eq!(module_name_from_path(&path, &roots), "pkg");
    }

    #[test]
    fn test_module_name_top_level_file() {
        let roots = vec![PathBuf::from("/workspace")];
        let path = PathBuf::from("/workspace/main.py");
        assert_eq!(module_name_from_path(&path, &roots), "main");
    }

    // ── byte_offset_to_line ───────────────────────────────────────────────

    #[test]
    fn test_byte_offset_zero_is_line_zero() {
        assert_eq!(byte_offset_to_line("hello\nworld\n", 0), 0);
    }

    #[test]
    fn test_byte_offset_past_first_newline_is_line_one() {
        assert_eq!(byte_offset_to_line("hello\nworld\n", 6), 1);
    }

    #[test]
    fn test_byte_offset_at_end_of_file() {
        let text = "line0\nline1\nline2\n";
        let offset = u32::try_from(text.len()).unwrap();
        assert_eq!(byte_offset_to_line(text, offset), 3);
    }

    #[test]
    fn test_byte_offset_within_second_line() {
        assert_eq!(byte_offset_to_line("ab\ncd\nef\n", 4), 1);
    }

    // ── coverage_percent ──────────────────────────────────────────────────

    #[test]
    fn test_coverage_zero_total_is_100() {
        assert_eq!(coverage_percent(0, 0), 100);
    }

    #[test]
    fn test_coverage_half() {
        assert_eq!(coverage_percent(5, 10), 50);
    }

    #[test]
    fn test_coverage_full() {
        assert_eq!(coverage_percent(10, 10), 100);
    }

    #[test]
    fn test_coverage_thirty_percent() {
        assert_eq!(coverage_percent(3, 10), 30);
    }

    #[test]
    fn test_coverage_zero_annotated() {
        assert_eq!(coverage_percent(0, 10), 0);
    }
}
