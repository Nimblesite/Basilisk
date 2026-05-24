//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-arch-pipeline
//! Type checker for Basilisk.
//!
//! The public API is [`check`] and [`check_with_config`], which take a
//! [`ResolvedModule`] and return a list of [`Diagnostic`]s.
//!
//! ## Suppression and Mode Override
//!
//! Basilisk supports a rich set of inline directives for controlling diagnostic
//! severity. See CHECKER-ARCHITECTURE-SPEC.md Section 4.1.3 for the full specification.
//!
//! - `# type: ignore` — suppress all diagnostics (PEP 484 compatible)
//! - `# type: ignore[BSK-E0010]` — suppress specific codes
//! - `# type: warning[BSK-E0010]` — demote to warning
//! - `# type: info[BSK-E0010]` — demote to info
//! - `# type: disabled[BSK-E0010]` — disable rule on this line
//! - `# type: disabled[BSK-E0010]` ... `# type: end-disabled[BSK-E0010]` — block
//! - `# basilisk: relaxed` — per-file: all errors become warnings
//! - `# basilisk: file-disabled[CODE]` — per-file: disable specific rules
//!
//! ## Project-level configuration
//!
//! [`check_with_config`] applies project-level overrides from `pyproject.toml`
//! or `basilisk.json`:
//! - Global rule severity overrides (`rules."BSK-E0010" = "warning"`)
//! - Per-module overrides (`per-module-overrides."fastmcp".ignore-missing-stubs`)
//! - Per-path overrides (`per-path-overrides."vendor/**".rules.disabled`)

pub mod collection_inference;
pub mod diagnostic;
pub mod inference;
pub mod rules;
pub mod span_util;
pub mod suppression;
pub mod types;
pub mod types_parsing;

pub use diagnostic::{Diagnostic, ErrorCode, Severity};

/// Run all rules and apply inline suppression / mode overrides.
///
/// Uses default configuration (no project-level overrides).
#[must_use]
pub fn check(module: &basilisk_resolver::ResolvedModule) -> Vec<Diagnostic> {
    check_with_config(module, &basilisk_config::BasiliskConfig::default())
}

/// Run all rules with project-level configuration applied.
///
/// Applies overrides in this priority order (highest to lowest):
/// 1. Inline source comments (`# type: ignore`, `# basilisk: relaxed`, etc.)
/// 2. Per-path overrides (`per-path-overrides."vendor/**"`)
/// 3. Per-module overrides (`per-module-overrides."fastmcp"`)
/// 4. Global rule severity overrides (`rules."BSK-E0010" = "warning"`)
/// 5. Cascade suppression (suppress downstream errors from untyped imports)
/// 6. Default rule severity
#[must_use]
pub fn check_with_config(
    module: &basilisk_resolver::ResolvedModule,
    config: &basilisk_config::BasiliskConfig,
) -> Vec<Diagnostic> {
    let inline_overrides = suppression::parse_source_overrides(&module.source);
    let source = &module.source;
    let file_path = std::path::Path::new(&module.path);
    let raw = rules::run_all(module);

    // Build the set of symbol names imported from unresolved modules.
    // Used for cascade suppression: downstream errors referencing these names
    // are suppressed since the root cause is the missing import (BSK-E0010).
    let untyped_names: std::collections::HashSet<String> = module
        .imports
        .iter()
        .filter(|i| {
            i.resolution == basilisk_resolver::scope::ImportResolution::Unresolved
                && !basilisk_stubs::is_stdlib_module(&i.module)
        })
        .flat_map(|i| i.names.iter().cloned())
        .collect();

    raw.into_iter()
        .filter_map(|mut diag| {
            let code = diag.code.code;

            // 0. Config gating for uv diagnostics.
            if code == "BSK-W0010" && !config.uv_stub_suggestions {
                return None;
            }
            if matches!(code, "BSK-W0011" | "BSK-W0012" | "BSK-W0013")
                && !config.uv_dependency_diagnostics
            {
                return None;
            }

            // 1. Per-path: check if rule is completely disabled for this file path.
            if config.is_rule_disabled_for_path(code, file_path) {
                return None;
            }

            // 2. Per-module: suppress BSK-E0010 for modules with ignore-missing-stubs.
            if code == "BSK-E0010" && should_suppress_e0010_for_module(module, config) {
                return None;
            }

            // 3. Cascade suppression: suppress downstream errors that reference
            //    symbols from unresolved imports. Only applies to type-checking
            //    rules whose results depend on resolved import types. Structural
            //    rules (Final, deprecated, Protocol, Generic params, etc.) fire
            //    independently of type resolution and must never be suppressed.
            if is_cascade_suppressible(code)
                && should_suppress_cascade(&diag, &untyped_names, source)
            {
                return None;
            }

            // 4. Tier-based severity adjustment: Tier3 (best-effort) stubs
            //    produce info-level diagnostics, not errors.
            if diag.provenance == Some(basilisk_stubs::TypeProvenance::StubTier3) {
                diag.severity = Severity::Info;
            }

            // 5. Global rule severity override from config.
            if let Some(severity) = config.rule_severity(code) {
                match severity {
                    basilisk_config::RuleSeverity::Disabled => return None,
                    basilisk_config::RuleSeverity::Warning => diag.severity = Severity::Warning,
                    basilisk_config::RuleSeverity::Info => diag.severity = Severity::Info,
                    basilisk_config::RuleSeverity::Error => {} // keep default
                }
            }

            // 6. Per-path rule severity override.
            if let Some(path_severity) =
                find_path_rule_severity(code, file_path, &config.per_path_overrides)
            {
                match path_severity {
                    basilisk_config::RuleSeverity::Disabled => return None,
                    basilisk_config::RuleSeverity::Warning => diag.severity = Severity::Warning,
                    basilisk_config::RuleSeverity::Info => diag.severity = Severity::Info,
                    basilisk_config::RuleSeverity::Error => {}
                }
            }

            // 7. Inline source overrides (highest priority).
            let diag_line = suppression::byte_offset_to_line_in_source(source, diag.span.start);
            suppression::apply_overrides_at_line(diag, diag_line, &inline_overrides)
        })
        .collect()
}

/// Returns `true` if this diagnostic code can be cascade-suppressed.
///
/// Only type-checking rules whose results depend on knowing the resolved type
/// of an imported symbol are eligible.  Structural / semantic rules (Final
/// re-assignment, deprecated usage, Protocol member checks, Generic param
/// validation, etc.) fire independently of import resolution.
fn is_cascade_suppressible(code: &str) -> bool {
    matches!(
        code,
        "BSK-E0012"  // wrong call
        | "BSK-E0013" // attribute not found
        | "BSK-E0014" // type mismatch
        | "BSK-E0015" // missing return type
        | "BSK-E0018" // undefined variable
        | "BSK-E0041" // too few arguments
        | "BSK-E0053" // assert_type mismatch
    )
}

/// Check whether a diagnostic should be suppressed because it references a
/// symbol from an unresolved import.
///
/// Extracts the source text covered by the diagnostic's span and checks if
/// any of the `untyped_names` appear as whole identifiers within it. This
/// avoids cascading errors from untyped imports — the root cause is already
/// reported by BSK-E0010.
fn should_suppress_cascade(
    diag: &Diagnostic,
    untyped_names: &std::collections::HashSet<String>,
    source: &str,
) -> bool {
    if untyped_names.is_empty() {
        return false;
    }

    // Extract the source text at the diagnostic's span.
    let start = diag.span.start_usize();
    let end = diag.span.end_usize();
    if start >= source.len() || end > source.len() || start >= end {
        return false;
    }
    let span_text = &source[start..end];

    // Also check the message — rules often include the symbol name.
    for name in untyped_names {
        if contains_identifier(span_text, name) || contains_identifier(&diag.message, name) {
            return true;
        }
    }
    false
}

/// Check if `text` contains `ident` as a whole identifier (not a substring
/// of a longer identifier).
fn contains_identifier(text: &str, ident: &str) -> bool {
    let bytes = text.as_bytes();
    let ident_bytes = ident.as_bytes();
    let ident_len = ident_bytes.len();

    if ident_len == 0 || ident_len > bytes.len() {
        return false;
    }

    for start in 0..=(bytes.len() - ident_len) {
        let Some(slice) = bytes.get(start..start + ident_len) else {
            continue;
        };
        if slice != ident_bytes {
            continue;
        }
        // Check that the character before (if any) is not an identifier char.
        let before_ok = start == 0 || bytes.get(start - 1).is_none_or(|b| !is_identifier_char(*b));
        // Check that the character after (if any) is not an identifier char.
        let after_ok = start + ident_len == bytes.len()
            || bytes
                .get(start + ident_len)
                .is_none_or(|b| !is_identifier_char(*b));
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

/// ASCII-only identifier character check (sufficient for Python identifiers
/// in diagnostic messages).
const fn is_identifier_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Check whether BSK-E0010 should be suppressed based on per-module overrides.
///
/// Iterates the module's imports to find which module triggered E0010,
/// then checks if that module has `ignore-missing-stubs = true`.
fn should_suppress_e0010_for_module(
    module: &basilisk_resolver::ResolvedModule,
    config: &basilisk_config::BasiliskConfig,
) -> bool {
    module.imports.iter().any(|import| {
        import.resolution == basilisk_resolver::scope::ImportResolution::Unresolved
            && config.should_ignore_missing_stubs(&import.module)
    })
}

/// Look up per-path rule severity override for a specific rule code.
fn find_path_rule_severity(
    rule_code: &str,
    file_path: &std::path::Path,
    overrides: &std::collections::HashMap<String, basilisk_config::PathOverride>,
) -> Option<basilisk_config::RuleSeverity> {
    basilisk_config::overrides::find_path_override(file_path, overrides)
        .and_then(|o| o.rule_overrides.get(rule_code).copied())
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;

    /// Count diagnostics with code `code` in `diagnostics`.
    fn count_code(diagnostics: &[Diagnostic], code: &str) -> usize {
        diagnostics.iter().filter(|d| d.code.code == code).count()
    }

    #[test]
    fn contains_identifier_exact_match() {
        assert!(contains_identifier("Foo", "Foo"));
    }

    #[test]
    fn contains_identifier_within_text() {
        assert!(contains_identifier("x = Foo()", "Foo"));
        assert!(contains_identifier("bar.Foo.baz", "Foo"));
    }

    #[test]
    fn contains_identifier_rejects_substring() {
        assert!(!contains_identifier("FooBar", "Foo"));
        assert!(!contains_identifier("aFoo", "Foo"));
        assert!(!contains_identifier("Foo_bar", "Foo"));
    }

    #[test]
    fn contains_identifier_empty_ident() {
        assert!(!contains_identifier("abc", ""));
    }

    #[test]
    fn cascade_suppression_suppresses_downstream() {
        let source = "from requests import get\nx = get('url')\n";
        let parsed =
            basilisk_parser::parse_source(source.to_owned(), "test.py".to_owned()).unwrap();
        let mut module = basilisk_resolver::resolve(&parsed).unwrap();
        // Mark the import as unresolved.
        if let Some(import) = module.imports.first_mut() {
            import.resolution = basilisk_resolver::scope::ImportResolution::Unresolved;
        }

        let config = basilisk_config::BasiliskConfig::default();
        let diagnostics = check_with_config(&module, &config);

        // Should have BSK-E0010 for the unresolved import.
        let e0010_count = count_code(&diagnostics, "BSK-E0010");
        assert!(
            e0010_count >= 1,
            "BSK-E0010 should fire for unresolved import"
        );

        // Should NOT have any downstream errors referencing `get`.
        let downstream = diagnostics
            .iter()
            .filter(|d| d.code.code != "BSK-E0010" && d.code.code != "BSK-W0010")
            .filter(|d| d.message.contains("get"))
            .count();
        assert_eq!(
            downstream, 0,
            "downstream errors referencing 'get' should be suppressed"
        );
    }

    #[test]
    fn cascade_suppression_does_not_suppress_resolved_imports() {
        let source = "from os import path\nx = path.join('a', 'b')\n";
        let parsed =
            basilisk_parser::parse_source(source.to_owned(), "test.py".to_owned()).unwrap();
        let module = basilisk_resolver::resolve(&parsed).unwrap();

        let config = basilisk_config::BasiliskConfig::default();
        let diagnostics = check_with_config(&module, &config);

        // os is stdlib — no BSK-E0010 should fire.
        let e0010_count = count_code(&diagnostics, "BSK-E0010");
        assert_eq!(e0010_count, 0, "stdlib imports should not fire BSK-E0010");
    }
}
