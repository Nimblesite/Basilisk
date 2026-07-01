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
//! - `# type: ignore[imports_unresolved]` — suppress specific codes
//! - `# type: warning[imports_unresolved]` — demote to warning
//! - `# type: info[imports_unresolved]` — demote to info
//! - `# type: disabled[imports_unresolved]` — disable rule on this line
//! - `# type: disabled[imports_unresolved]` ... `# type: end-disabled[imports_unresolved]` — block
//! - `# basilisk: relaxed` — per-file: all errors become warnings
//! - `# basilisk: file-disabled[CODE]` — per-file: disable specific rules
//!
//! ## Project-level configuration
//!
//! [`check_with_config`] applies project-level overrides from `pyproject.toml`
//! or `basilisk.json`:
//! - Global rule severity overrides (`rules."imports_unresolved" = "warning"`)
//! - Per-module overrides (`per-module-overrides."fastmcp".ignore-missing-stubs`)
//! - Per-path overrides (`per-path-overrides."vendor/**".rules.disabled`)

pub mod cached;
pub mod collection_inference;
pub mod context;
pub mod diagnostic;
pub mod imports;
pub mod incremental;
pub mod inference;
pub mod rule_tags;
pub mod rules;
pub mod span_util;
pub mod suppression;
pub mod types;
pub mod types_parsing;

pub use cached::CachedDiagnostic;
pub use diagnostic::{Diagnostic, ErrorCode, Severity};
pub use incremental::{
    checked_file, checked_file_resolved, file_diagnostics, file_diagnostics_resolved,
    resolved_module, ConfigInput, ConfigValue, SearchPathsInput,
};

// Re-export the incremental-database handles so consumers can drive the
// memoized `checked_file` query without depending on `basilisk-db` directly.
pub use basilisk_db::{BasiliskDatabase, Db, SourceFile};

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
/// 4. Global rule severity overrides (`rules."imports_unresolved" = "warning"`)
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
    // [CHKARCH-VERSION-TARGET] every rule sees the configured target.
    let ctx = context::CheckContext::from_config(config);
    let raw = rules::run_all(module, &ctx);

    // Build the set of symbol names imported from unresolved modules.
    // Used for cascade suppression: downstream errors referencing these names
    // are suppressed since the root cause is the missing import (imports_unresolved).
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

            // 0. Opt-in gating. Basilisk-original rules (provenance `basilisk`)
            //    are off by default; each turns on only when the configuration
            //    opts into one of its tags. PEP rules always run. Provenance and
            //    tags come from the rule itself via the tagging layer — there is
            //    no hand-maintained code list here. [CHKTAG-PROVENANCE]
            if let Some(spec) = rule_tags::opt_in_spec_for_code(code) {
                if !spec.tags.iter().any(|tag| opt_in_tag_enabled(tag, config)) {
                    return None;
                }
            }

            // 1. Per-path: check if rule is completely disabled for this file path.
            if config.is_rule_disabled_for_path(code, file_path) {
                return None;
            }

            // 2. Per-module: suppress imports_unresolved for modules with ignore-missing-stubs.
            if code == "imports_unresolved" && should_suppress_e0010_for_module(module, config) {
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
                    basilisk_config::RuleSeverity::Error => diag.severity = Severity::Error,
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
                    basilisk_config::RuleSeverity::Error => diag.severity = Severity::Error,
                }
            }

            // 7. Inline source overrides (highest priority).
            let diag_line = suppression::byte_offset_to_line_in_source(source, diag.span.start);
            suppression::apply_overrides_at_line(diag, diag_line, &inline_overrides)
        })
        .collect()
}

/// Whether a Basilisk rule's free-form `tag` is opted into by `config`.
///
/// This is the single bridge from the configuration's opt-in switches to rule
/// tags. Selection is by tag, so adding a Basilisk rule needs only a tag on the
/// rule itself — never an entry in a code list here or in the tagging layer.
/// [CHKARCH-CONFIGURATION-ONLY]
fn opt_in_tag_enabled(tag: &str, config: &basilisk_config::BasiliskConfig) -> bool {
    match tag {
        "strictness" | "style" | "redundancy" => config.strict_annotations,
        "dependencies" | "imports" => config.uv_dependency_diagnostics,
        "stubs" => config.uv_stub_suggestions,
        _ => false,
    }
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
        "calls_argument_type"  // wrong call
        | "returns_compatibility_2" // attribute not found
        | "assignment_compatibility" // type mismatch
        | "callables_annotation" // missing return type
        | "names_undefined" // undefined variable
        | "calls_argument_count" // too few arguments
        | "directives_assert_type_2" // assert_type mismatch
    )
}

/// Check whether a diagnostic should be suppressed because it references a
/// symbol from an unresolved import.
///
/// Extracts the source text covered by the diagnostic's span and checks if
/// any of the `untyped_names` appear as whole identifiers within it. This
/// avoids cascading errors from untyped imports — the root cause is already
/// reported by `imports_unresolved`.
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

/// Check whether `imports_unresolved` should be suppressed based on per-module overrides.
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

        // Should have imports_unresolved for the unresolved import.
        let e0010_count = count_code(&diagnostics, "imports_unresolved");
        assert!(
            e0010_count >= 1,
            "imports_unresolved should fire for unresolved import"
        );

        // Should NOT have any downstream errors referencing `get`.
        let downstream = diagnostics
            .iter()
            .filter(|d| d.code.code != "imports_unresolved" && d.code.code != "BSK-E0152")
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

        // os is stdlib — no imports_unresolved should fire.
        let e0010_count = count_code(&diagnostics, "imports_unresolved");
        assert_eq!(
            e0010_count, 0,
            "stdlib imports should not fire imports_unresolved"
        );
    }
}
