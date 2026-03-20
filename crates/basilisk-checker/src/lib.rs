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
pub mod constraint_solver;
pub mod diagnostic;
pub mod expr_inference;
pub mod inference;
pub mod narrowing;
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
/// 5. Default rule severity
#[must_use]
pub fn check_with_config(
    module: &basilisk_resolver::ResolvedModule,
    config: &basilisk_config::BasiliskConfig,
) -> Vec<Diagnostic> {
    let inline_overrides = suppression::parse_source_overrides(&module.source);
    let source = &module.source;
    let file_path = std::path::Path::new(&module.path);
    let raw = rules::run_all(module);

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

            // 3. Global rule severity override from config.
            if let Some(severity) = config.rule_severity(code) {
                match severity {
                    basilisk_config::RuleSeverity::Disabled => return None,
                    basilisk_config::RuleSeverity::Warning => diag.severity = Severity::Warning,
                    basilisk_config::RuleSeverity::Info => diag.severity = Severity::Info,
                    basilisk_config::RuleSeverity::Error => {} // keep default
                }
            }

            // 4. Per-path rule severity override.
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

            // 5. Inline source overrides (highest priority).
            let diag_line = suppression::byte_offset_to_line_in_source(source, diag.span.start);
            suppression::apply_overrides_at_line(diag, diag_line, &inline_overrides)
        })
        .collect()
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
