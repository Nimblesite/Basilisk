//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-arch-pipeline
//! Type checker for Basilisk.
//!
//! The public API is [`check`] and [`check_with_config`], which take a
//! [`ResolvedModule`] and return a list of [`Diagnostic`]s.
//!
//! ## Suppression and Severity Overrides
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
//! [`check_with_config`] applies the configuration model from
//! `pyproject.toml` `[tool.basilisk]` ([CHKARCH-CONFIG-MODEL]): per-rule
//! entries (`rules."imports_unresolved" = "warning"`) and tag entries
//! (`rule-tags."basilisk" = "error"`), resolved nearest-deciding-table-first.
//! PEP rules always run and can never be disabled; every other rule runs only
//! when configuration resolves it to a non-disabled severity
//! ([CHKARCH-COMMANDS]).

pub mod bidir;
pub mod cached;
pub mod collection_inference;
pub mod context;
pub mod diagnostic;
pub mod exports;
pub mod imports;
pub mod incremental;
pub mod incremental_defs;
pub mod inference;
pub mod narrow;
pub mod param_infer;
pub mod rule_catalog;
pub mod rule_tags;
pub mod rules;
pub mod span_util;
pub mod stub_constructor;
pub mod suppression;
mod suppression_audit;
pub mod types;
pub mod types_parsing;
pub(crate) mod types_star_tuples;

pub use cached::CachedDiagnostic;
pub use diagnostic::{Diagnostic, ErrorCode, Severity};
pub use incremental::{
    checked_file, checked_file_cross, checked_file_resolved, cross_resolved_module,
    file_diagnostics, file_diagnostics_cross, file_diagnostics_resolved, module_exports,
    resolved_module, ConfigInput, ConfigValue, FileRegistry, ModuleExports, ResolvedFile,
    SearchPathsInput, WorkspaceFiles,
};
pub use rule_catalog::{rule_catalog, RuleDescriptor};

// Re-export the incremental-database handles so consumers can drive the
// memoized `checked_file` query without depending on `basilisk-db` directly.
pub use basilisk_db::{BasiliskDatabase, Db, SourceFile};

/// Run all rules and apply inline suppression and severity overrides.
///
/// Uses default configuration (no project-level overrides).
#[must_use]
pub fn check(module: &basilisk_resolver::ResolvedModule) -> Vec<Diagnostic> {
    check_with_config(module, &basilisk_config::BasiliskConfig::default())
}

/// Run all rules with project-level configuration applied.
///
/// Applies overrides in this priority order (highest to lowest) —
/// [CHKARCH-STRICTNESS-PRECEDENCE]:
/// 1. Inline source comments (`# type: ignore`, `# basilisk: relaxed`, etc.)
/// 2. Nearest deciding folder config — rule entry over tag entry, strictest
///    matching tag ([CHKARCH-CONFIG-MODEL])
/// 3. Cascade suppression (suppress downstream errors from untyped imports)
/// 4. Scope default: `pep` rules run at `error`; everything else does not run
#[must_use]
pub fn check_with_config(
    module: &basilisk_resolver::ResolvedModule,
    config: &basilisk_config::BasiliskConfig,
) -> Vec<Diagnostic> {
    let source = &module.source;
    let effective_rules = EffectiveRuleConfig::new(config);
    let audit_selected = suppression_audit_selected(effective_rules);
    let inline_overrides = suppression::parse_source_overrides_with_comments(
        &module.source,
        &module.comment_ranges,
        audit_selected,
    );
    let has_inline_overrides = !inline_overrides.is_empty();
    // [CHKARCH-VERSION-TARGET] every rule sees the configured target, plus a
    // shared line index so offset→line lookups (here and in rules) stay O(log n)
    // instead of rescanning the source per diagnostic / per function.
    // Only starred-tuple analysis and inline suppression need byte→line
    // lookups. Avoid allocating and populating an O(lines) index for the common
    // case where neither feature appears.
    let ctx = if has_inline_overrides || source.contains("*tuple[") {
        context::CheckContext::from_config_with_source(config, source)
    } else {
        context::CheckContext::from_config(config)
    };
    let raw = rules::run_all(module, &ctx);

    // Build the set of symbol names imported from unresolved modules.
    // Used for cascade suppression: downstream errors referencing these names
    // are suppressed since the root cause is the missing import (imports_unresolved).
    let untyped_names: std::collections::HashSet<String> = if raw
        .iter()
        .any(|diagnostic| is_cascade_suppressible(diagnostic.code.code))
    {
        module
            .imports
            .iter()
            .filter(|i| i.resolution == basilisk_resolver::scope::ImportResolution::Unresolved)
            .flat_map(|i| i.names.iter().cloned())
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    // Apply every project-level decision first, retaining this pre-inline view
    // for suppression auditing. Audit diagnostics are appended only after the
    // ordinary diagnostics pass through inline suppression, so a directive can
    // never hide the audit finding about itself.
    let prepared = raw
        .into_iter()
        .filter_map(|mut diag| {
            let code = diag.code.code;

            // 0. Selection ([CHKARCH-CONFIG-MODEL]). `pep` rules always run;
            //    every other rule runs only when configuration resolves it to
            //    a non-disabled severity. Provenance comes from the rule
            //    itself via the tagging layer — there is no hand-maintained
            //    code list. [CHKTAG-PROVENANCE]
            if !effective_rules.selected(code) {
                return None;
            }

            // 2. Cascade suppression: suppress downstream errors that reference
            //    symbols from unresolved imports. Only applies to type-checking
            //    rules whose results depend on resolved import types. Structural
            //    rules (Final, deprecated, Protocol, Generic params, etc.) fire
            //    independently of type resolution and must never be suppressed.
            if is_cascade_suppressible(code)
                && should_suppress_cascade(&diag, &untyped_names, source)
            {
                return None;
            }

            // 3. Tier-based severity adjustment: Tier3 (best-effort) stubs
            //    produce info-level diagnostics, not errors.
            if diag.provenance == Some(basilisk_stubs::TypeProvenance::StubTier3) {
                diag.severity = Severity::Info;
            }

            // 4. Effective configured severity from the nearest deciding
            //    table. A `disabled` resolution never applies to a `pep` rule
            //    ([CHKARCH-CONFIG-MODEL]) — such a config is invalid
            //    ([`pep_disable_violations`]) and the rule keeps its own
            //    severity here so `check` never loses a PEP diagnostic.
            if let Some(severity) = effective_rules.severity(code) {
                if severity == basilisk_config::RuleSeverity::Disabled && is_pep_rule(code) {
                    return Some(diag);
                }
                apply_configured_severity(&mut diag, severity)?;
            }

            Some(diag)
        })
        .collect::<Vec<_>>();

    // 7. Inline source overrides (highest priority). The 0-based line is the
    //    count of newlines before the span start — the shared line index answers
    //    that in O(log n) instead of rescanning the prefix for every diagnostic.
    let mut filtered = prepared
        .iter()
        .cloned()
        .filter_map(|diag| {
            finish_inline_override(diag, has_inline_overrides, &inline_overrides, &ctx)
        })
        .collect::<Vec<_>>();

    if audit_selected {
        filtered.extend(
            suppression_audit::diagnostics(source, &inline_overrides, &prepared, &module.path)
                .into_iter()
                .filter_map(|diagnostic| configure_suppression_audit(diagnostic, effective_rules)),
        );
    }
    filtered
}

/// Whether `code` is check-scope: it carries the `pep` provenance tag.
///
/// Implements [CHKARCH-COMMANDS]: a rule is check-scope iff it is `pep`;
/// everything else is analyze-scope. Every rule belongs to exactly one
/// command, and the registry's self-declared tags are the single source of
/// the partition.
#[must_use]
pub fn is_pep_rule(code: &str) -> bool {
    rule_tags::opt_in_spec_for_code(code).is_none()
}

/// PEP rules the configuration invalidly resolves to `disabled`.
///
/// Implements [CHKARCH-CONFIG-MODEL]: `disabled` never applies to a
/// `pep`-tagged rule; a configuration that resolves one to `disabled` — by
/// rule entry or tag entry — is invalid. Callers (CLI, LSP) surface the
/// returned codes as a configuration error; the checker itself defensively
/// keeps such rules running so `check` never loses a PEP diagnostic.
#[must_use]
pub fn pep_disable_violations(config: &basilisk_config::BasiliskConfig) -> Vec<&'static str> {
    rule_catalog()
        .into_iter()
        .map(|descriptor| descriptor.code)
        .filter(|code| is_pep_rule(code))
        .filter(|code| {
            let tags = rule_tags::tags_for_code(code);
            config.resolve_severity(code, &tags) == Some(basilisk_config::RuleSeverity::Disabled)
        })
        .collect()
}

/// Per-file rule selection and grading over the configuration model.
///
/// Implements [CHKARCH-CONFIG-MODEL]: the nearest deciding table wins, a rule
/// entry beats tag entries, the strictest matching tag entry wins; `pep`
/// rules bottom out at their own (error) severity, everything else at
/// disabled.
#[derive(Clone, Copy)]
struct EffectiveRuleConfig<'a> {
    config: &'a basilisk_config::BasiliskConfig,
}

impl<'a> EffectiveRuleConfig<'a> {
    const fn new(config: &'a basilisk_config::BasiliskConfig) -> Self {
        Self { config }
    }

    fn severity(self, code: &str) -> Option<basilisk_config::RuleSeverity> {
        // Bare tree fast path: with no table anywhere there is nothing to
        // resolve — the conformance hot path never allocates tag vectors.
        if !self.config.has_config_table() {
            return None;
        }
        let tags = rule_tags::tags_for_code(code);
        self.config.resolve_severity(code, &tags)
    }

    fn selected(self, code: &str) -> bool {
        match self.severity(code) {
            // No table decides the rule: pep runs (at its own severity),
            // everything else is disabled. And a disabled resolution never
            // applies to a pep rule — invalid config
            // ([`pep_disable_violations`]); keep the rule running.
            // [CHKARCH-CONFIG-MODEL]
            None | Some(basilisk_config::RuleSeverity::Disabled) => is_pep_rule(code),
            Some(_) => true,
        }
    }
}

/// Whether a rule is selected before it runs.
///
fn suppression_audit_selected(effective_rules: EffectiveRuleConfig<'_>) -> bool {
    rule_tags::opt_in_specs_with_tag("suppressions").any(|spec| effective_rules.selected(spec.code))
}

fn configure_suppression_audit(
    mut diagnostic: Diagnostic,
    effective_rules: EffectiveRuleConfig<'_>,
) -> Option<Diagnostic> {
    let code = diagnostic.code.code;
    if !effective_rules.selected(code) {
        return None;
    }
    if let Some(severity) = effective_rules.severity(code) {
        apply_configured_severity(&mut diagnostic, severity)?;
    }
    Some(diagnostic)
}

fn apply_configured_severity(
    diagnostic: &mut Diagnostic,
    severity: basilisk_config::RuleSeverity,
) -> Option<()> {
    diagnostic.severity = match severity {
        basilisk_config::RuleSeverity::Disabled => return None,
        basilisk_config::RuleSeverity::Warning => Severity::Warning,
        basilisk_config::RuleSeverity::Info => Severity::Info,
        basilisk_config::RuleSeverity::Error => Severity::Error,
    };
    Some(())
}

fn finish_inline_override(
    diag: Diagnostic,
    has_inline_overrides: bool,
    inline_overrides: &suppression::SourceOverrides,
    ctx: &context::CheckContext,
) -> Option<Diagnostic> {
    if !has_inline_overrides {
        return Some(diag);
    }
    let diag_line = ctx
        .line_index
        .line(diag.span.start_usize())
        .saturating_sub(1);
    suppression::apply_overrides_at_line(diag, diag_line, inline_overrides)
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
        // Whole-string match at both bounds (start == 0 AND end == len): kills
        // the boundary mutants on `start == 0` and `start + ident_len == len`.
        assert!(contains_identifier("x", "x"));
        assert!(contains_identifier("__init__", "__init__"));
        // ident longer than text can never match — kills `ident_len > bytes.len()`
        // becoming `<`/`==`/`>=` (the `-> false` early return must fire here).
        assert!(!contains_identifier("Fo", "Foo"));
        assert!(!contains_identifier("", "Foo"));
        // A single-char text vs a 2-char ident (len boundary at exactly len-1).
        assert!(!contains_identifier("F", "Fo"));
    }

    #[test]
    fn explicit_suppression_tagged_rule_selects_audit_family_from_live_metadata() {
        let specs = rule_tags::opt_in_specs_with_tag("suppressions").collect::<Vec<_>>();
        assert!(
            !specs.is_empty(),
            "the live catalog must expose suppression rules"
        );

        for spec in specs {
            let config = basilisk_config::BasiliskConfig {
                rule_chain: vec![basilisk_config::RuleTables {
                    rules: [(spec.code.to_owned(), basilisk_config::RuleSeverity::Info)]
                        .into_iter()
                        .collect(),
                    rule_tags: std::collections::HashMap::new(),
                }],
                ..Default::default()
            };
            let effective = EffectiveRuleConfig::new(&config);
            assert!(
                suppression_audit_selected(effective),
                "explicitly selecting live suppression rule {} must enable audit parsing",
                spec.code
            );
        }
    }

    #[test]
    fn contains_identifier_within_text() {
        assert!(contains_identifier("x = Foo()", "Foo"));
        assert!(contains_identifier("bar.Foo.baz", "Foo"));
        // Delimited by non-identifier chars on each side — proves before_ok AND
        // after_ok must BOTH hold (kills `&& -> ||` in the boundary check, and
        // the `!is_identifier_char` negation deletes).
        assert!(contains_identifier("(Foo)", "Foo"));
        assert!(contains_identifier(".Foo ", "Foo"));
        assert!(contains_identifier("Foo=1", "Foo"));
        // Match at the very end of the string (after_ok via end == len branch).
        assert!(contains_identifier("call Foo", "Foo"));
        // Match at the very start (before_ok via start == 0 branch).
        assert!(contains_identifier("Foo bar", "Foo"));
        // Second occurrence is the valid one — the loop must scan past the first
        // (substring) hit; kills `start` arithmetic (`+`/`-`) and the `0..=len`
        // range boundary.
        assert!(contains_identifier("Foobar Foo", "Foo"));
    }

    #[test]
    fn contains_identifier_rejects_substring() {
        assert!(!contains_identifier("FooBar", "Foo"));
        assert!(!contains_identifier("aFoo", "Foo"));
        assert!(!contains_identifier("Foo_bar", "Foo"));
        // Trailing digit / underscore make it part of a longer identifier:
        // is_identifier_char must return true for alphanumeric AND '_' — kills
        // `is_identifier_char -> false`, the `||`->`&&`, and the `== '_'` flip.
        assert!(!contains_identifier("Foo1", "Foo"));
        assert!(!contains_identifier("_Foo", "Foo"));
        assert!(!contains_identifier("1Foo", "Foo"));
        // Surrounded on BOTH sides — neither before_ok nor after_ok holds.
        assert!(!contains_identifier("xFooy", "Foo"));
        // A separator that IS whitespace/punct passes, but a digit does not:
        // pins is_identifier_char's alphanumeric branch precisely.
        assert!(contains_identifier("a Foo b", "Foo"));
        assert!(!contains_identifier("a9Foo9b", "Foo"));
    }

    #[test]
    fn contains_identifier_empty_ident() {
        // Empty ident => `ident_len == 0` early-returns false: kills the
        // `-> true` body-replacement and `ident_len == 0` -> `!=` flip.
        assert!(!contains_identifier("abc", ""));
        assert!(!contains_identifier("", ""));
        // Non-empty ident in empty text also false (len guard), for contrast.
        assert!(!contains_identifier("", "x"));
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
            .filter(|d| d.code.code != "imports_unresolved" && d.code.code != "BSK-0152")
            .filter(|d| d.message.contains("get"))
            .count();
        assert_eq!(
            downstream, 0,
            "downstream errors referencing 'get' should be suppressed"
        );

        // Direct assertions on `should_suppress_cascade` pin its span/boundary
        // logic (line 279 `>=`/`>`/`||` mutants) that the pipeline test above is
        // too coarse to reach. `source` is the fixture text; craft spans that hit
        // each guard branch precisely.
        let mut names = std::collections::HashSet::new();
        let _ = names.insert("get".to_owned());

        // Safe usize -> u32 for span offsets into the small fixture text (no
        // silent `as`, matching the crate's `u32::try_from(..).unwrap_or(0)` idiom).
        let to_u32 = |value: usize| u32::try_from(value).unwrap_or(0);
        let source_len = to_u32(source.len());

        // Helper: a diagnostic whose span covers [start, end) of `source`.
        let diag_over = |start: u32, end: u32, msg: &str| Diagnostic {
            code: ErrorCode {
                code: "assignment_compatibility",
                docs_url: "https://www.basilisk-python.dev/errors/X",
            },
            severity: Severity::Error,
            message: msg.to_owned(),
            span: basilisk_resolver::Span::new(start, end),
            path: "test.py".to_owned(),
            help: None,
            note: None,
            provenance: None,
        };

        // `x = get('url')` — locate the `get` occurrence in the fixture.
        // `.unwrap()` is allowed in this test module (see the module-level
        // `#[expect(clippy::unwrap_used)]`); the fixture always contains `get(`.
        let get_pos = to_u32(source.find("get(").unwrap());
        let get_end = get_pos + 3;

        // Span exactly over the whole-identifier `get` => suppressed. Kills the
        // `>= -> <`, `> -> <`, and `start >= end` boundary flips: with a valid
        // in-bounds span the guard must NOT early-return false.
        assert!(
            should_suppress_cascade(&diag_over(get_pos, get_end, "irrelevant"), &names, source),
            "span over whole-identifier `get` must suppress"
        );

        // Empty untyped set => never suppress (kills the empty-check inversion).
        let empty: std::collections::HashSet<String> = std::collections::HashSet::new();
        assert!(!should_suppress_cascade(
            &diag_over(get_pos, get_end, "get"),
            &empty,
            source
        ));

        // start == end (zero-width span) => guard returns false. Kills
        // `start >= end` -> `start < end`/`==`.
        assert!(!should_suppress_cascade(
            &diag_over(get_pos, get_pos, "unrelated message"),
            &names,
            source
        ));

        // end > source.len() (out of bounds) => guard returns false, no panic.
        // Kills `end > source.len()` -> `<`/`==` and the `||` -> `&&`.
        let over_len = source_len + 10;
        assert!(!should_suppress_cascade(
            &diag_over(0, over_len, "unrelated"),
            &names,
            source
        ));

        // start >= source.len() => false. Kills `start >= source.len()` flip.
        assert!(!should_suppress_cascade(
            &diag_over(source_len, source_len + 1, "unrelated"),
            &names,
            source
        ));

        // Match via the MESSAGE (span text unrelated) => suppressed. Kills the
        // `||` -> `&&` between the span-text and message checks (line 286).
        assert!(should_suppress_cascade(
            &diag_over(0, 4, "cannot resolve get here"),
            &names,
            source
        ));

        // Message contains `get` only as a substring (`getter`) => NOT a whole
        // identifier => not suppressed. Proves whole-identifier matching holds
        // through the message path too.
        assert!(!should_suppress_cascade(
            &diag_over(0, 4, "getter not found"),
            &names,
            source
        ));
    }

    #[test]
    fn cascade_suppression_does_not_suppress_resolved_imports() {
        let source = "from os import path\nx = path.join('a', 'b')\n";
        let parsed =
            basilisk_parser::parse_source(source.to_owned(), "test.py".to_owned()).unwrap();
        let mut module = basilisk_resolver::resolve(&parsed).unwrap();
        let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot().unwrap();
        let paths = imports::ImportSearchPaths {
            roots: Vec::new(),
            extra_paths: Vec::new(),
            stub_paths: Vec::new(),
            workspace_members: Vec::new(),
            site_packages: None,
            registry: None,
            typeshed_snapshot: Some(imports::ActiveTypeshed::new(
                std::sync::Arc::new(snapshot),
                None,
            )),
        };
        imports::resolve_module_imports(&mut module, &paths);

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
