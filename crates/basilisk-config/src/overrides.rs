//! Override types for per-module and per-path configuration.

use std::collections::HashMap;

/// Severity level for a diagnostic rule override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone)]
pub struct ModuleOverride {
    /// When `true`, BSK-E0010 is suppressed for this module.
    pub ignore_missing_stubs: bool,
}

/// Per-path override configuration.
///
/// Applied when the file path matches the override key pattern.
/// Keys use glob patterns (e.g. `vendor/**` matches `vendor/lib/foo.py`).
#[derive(Debug, Clone)]
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

/// Check whether a file path matches a glob-like path pattern.
///
/// Supports `**` for recursive directory matching and `*` for single-segment matching.
#[must_use]
pub fn path_matches_pattern(file_path: &std::path::Path, pattern: &str) -> bool {
    let path_str = file_path.to_string_lossy();
    let pattern_normalized = pattern.replace('\\', "/");
    let path_normalized = path_str.replace('\\', "/");

    if pattern_normalized.contains("**") {
        // Simple recursive glob: `vendor/**` matches anything under vendor/
        let prefix = pattern_normalized.trim_end_matches("/**");
        path_normalized.starts_with(prefix)
    } else {
        path_normalized.starts_with(&pattern_normalized)
    }
}

/// Find applicable module override for a given module name.
///
/// Returns the override configuration if any per-module pattern matches.
#[must_use]
pub fn find_module_override<'a>(
    module_name: &str,
    overrides: &'a HashMap<String, ModuleOverride>,
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
#[must_use]
pub fn find_path_override<'a>(
    file_path: &std::path::Path,
    overrides: &'a HashMap<String, PathOverride>,
) -> Option<&'a PathOverride> {
    overrides
        .iter()
        .find(|(pattern, _)| path_matches_pattern(file_path, pattern))
        .map(|(_, entry)| entry)
}

/// Check whether a rule is disabled for a given file path.
#[must_use]
pub fn is_rule_disabled_for_path(
    rule_code: &str,
    file_path: &std::path::Path,
    overrides: &HashMap<String, PathOverride>,
) -> bool {
    find_path_override(file_path, overrides)
        .is_some_and(|o| o.disabled_rules.iter().any(|r| r == rule_code))
}
