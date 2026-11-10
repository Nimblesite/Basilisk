//! Implements [CHKARCH-DIAG-CATEGORIES]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CATEGORIES
//!
//! Which registered rules actually implement their PEP obligation.
//!
//! Registration is not implementation. A rule can be listed in the registry,
//! carry a documented error code, and still be incapable of ever producing a
//! verdict — because its verdict path was deleted as text-matched logic, or
//! because the resolver input it reads is permanently empty. Publishing such a
//! rule as part of the checker's coverage is the same category of dishonesty
//! as a text-matched verdict: it looks like analysis and is not.
//!
//! This table names them. It is evidence-based, not aspirational:
//!
//! * [`RuleStatus::Invalid`] — **proven incapable of emitting**. Either the
//!   module contains no `diagnostics.push`/`extend` at all
//!   ([`InvalidReason::NoVerdictPath`]), or every field it reads from
//!   [`basilisk_resolver::ResolvedModule`] is hardcoded empty by the visitor
//!   ([`InvalidReason::StarvedInput`]). Both are static, checkable facts.
//! * [`RuleStatus::Unproven`] — reachable, but NOT known correct. No attributed
//!   spec obligation proves its verdict. This is the honest default: until the
//!   permutation oracle proves an obligation with `assert_rejected_by`
//!   ([PERMTEST-FAMILY-B]), a rule that fires has demonstrated only that its
//!   code path runs.
//! * [`RuleStatus::Proven`] — an attributed obligation proves the verdict, and
//!   the obligation survives semantic mutation. **Nothing holds this status
//!   yet**, and nothing may be moved into it except by adding such a test.
//!
//! `Unproven` is deliberately not a compliment, and `Proven` is deliberately
//! empty. Moving a rule up this scale requires evidence, never opinion.

/// Why a rule cannot produce a verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidReason {
    /// The rule's module never pushes a diagnostic on any path.
    NoVerdictPath,
    /// Every resolver field the rule reads is hardcoded empty by the visitor,
    /// so the rule's own logic can never see an input. The field named is the
    /// starved input.
    StarvedInput(&'static str),
}

/// Whether a registered rule implements the obligation it claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleStatus {
    /// An attributed obligation proves this rule's verdict.
    Proven,
    /// Reachable, but no attributed obligation proves the verdict.
    Unproven,
    /// INVALID: proven incapable of emitting a diagnostic.
    Invalid(InvalidReason),
}

impl RuleStatus {
    /// Whether this rule is incapable of producing a verdict.
    #[must_use]
    pub const fn is_invalid(self) -> bool {
        matches!(self, Self::Invalid(_))
    }
}

/// Every registered diagnostic code and its evidence-backed status.
///
/// Regenerate the evidence with
/// `cargo test -p basilisk-checker --test rule_liveness_census -- --ignored`.
pub const RULE_STATUS: &[(&str, RuleStatus)] = &[
    ("BSK-0001", RuleStatus::Unproven),
    ("BSK-0002", RuleStatus::Unproven),
    ("BSK-0003", RuleStatus::Unproven),
    ("BSK-0004", RuleStatus::Unproven),
    ("BSK-0005", RuleStatus::Unproven),
    ("BSK-0011", RuleStatus::Unproven),
    ("BSK-0012", RuleStatus::Unproven),
    ("BSK-0013", RuleStatus::Unproven),
    ("BSK-0014", RuleStatus::Unproven),
    ("BSK-0025", RuleStatus::Unproven),
    ("BSK-0040", RuleStatus::Unproven),
    ("BSK-0050", RuleStatus::Unproven),
    ("BSK-0060", RuleStatus::Unproven),
    ("BSK-0061", RuleStatus::Unproven),
    ("BSK-0062", RuleStatus::Unproven),
    ("BSK-0063", RuleStatus::Unproven),
    ("BSK-0152", RuleStatus::Unproven),
    ("aliases_implicit", RuleStatus::Unproven),
    (
        "aliases_newtype",
        RuleStatus::Invalid(InvalidReason::StarvedInput("newtype_calls")),
    ),
    ("aliases_recursive", RuleStatus::Unproven),
    ("aliases_type_statement", RuleStatus::Unproven),
    (
        "aliases_typealiastype",
        RuleStatus::Invalid(InvalidReason::StarvedInput("type_alias_type_violations")),
    ),
    ("annotations_forward_refs", RuleStatus::Unproven),
    ("annotations_generators", RuleStatus::Unproven),
    (
        "annotations_generators_2",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("annotations_typeexpr", RuleStatus::Unproven),
    ("assignment_compatibility", RuleStatus::Unproven),
    ("callables_protocol", RuleStatus::Unproven),
    ("callables_protocol_2", RuleStatus::Unproven),
    ("calls_argument_count", RuleStatus::Unproven),
    ("calls_argument_type", RuleStatus::Unproven),
    (
        "classes_override",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("classes_override_2", RuleStatus::Unproven),
    (
        "classes_override_3",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("constructors_call_init", RuleStatus::Unproven),
    ("constructors_call_new", RuleStatus::Unproven),
    ("constructors_call_type", RuleStatus::Unproven),
    ("dataclasses_frozen", RuleStatus::Unproven),
    ("dataclasses_hash", RuleStatus::Unproven),
    ("dataclasses_inheritance", RuleStatus::Unproven),
    ("dataclasses_kwonly", RuleStatus::Unproven),
    ("dataclasses_match_args", RuleStatus::Unproven),
    ("dataclasses_postinit", RuleStatus::Unproven),
    ("dataclasses_slots", RuleStatus::Unproven),
    (
        "dataclasses_transform_meta",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("dataclasses_usage", RuleStatus::Unproven),
    ("dict_key_hashable", RuleStatus::Unproven),
    ("directives_assert_type", RuleStatus::Unproven),
    ("directives_assert_type_2", RuleStatus::Unproven),
    (
        "directives_cast",
        RuleStatus::Invalid(InvalidReason::StarvedInput("cast_calls")),
    ),
    ("directives_deprecated", RuleStatus::Unproven),
    ("directives_disjoint_base", RuleStatus::Unproven),
    ("directives_reveal_type", RuleStatus::Unproven),
    ("enums_behaviors", RuleStatus::Unproven),
    ("enums_definition", RuleStatus::Unproven),
    (
        "enums_expansion",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    (
        "enums_member_values",
        RuleStatus::Invalid(InvalidReason::StarvedInput("enum_value_type_violations")),
    ),
    ("enums_members", RuleStatus::Unproven),
    (
        "enums_members_2",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("generics_base_class", RuleStatus::Unproven),
    ("generics_base_class_2", RuleStatus::Unproven),
    ("generics_base_class_3", RuleStatus::Unproven),
    ("generics_basic", RuleStatus::Unproven),
    ("generics_basic_2", RuleStatus::Unproven),
    ("generics_basic_3", RuleStatus::Unproven),
    ("generics_defaults", RuleStatus::Unproven),
    ("generics_defaults_2", RuleStatus::Unproven),
    ("generics_defaults_referential", RuleStatus::Unproven),
    ("generics_defaults_specialization", RuleStatus::Unproven),
    (
        "generics_self_attributes",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    (
        "generics_self_basic",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("generics_self_protocols", RuleStatus::Unproven),
    (
        "generics_self_usage",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("generics_syntax_compatibility", RuleStatus::Unproven),
    ("generics_syntax_declarations", RuleStatus::Unproven),
    ("generics_syntax_declarations_2", RuleStatus::Unproven),
    ("generics_syntax_scoping", RuleStatus::Unproven),
    ("generics_type_erasure", RuleStatus::Unproven),
    ("generics_typevartuple_args", RuleStatus::Unproven),
    ("generics_typevartuple_basic", RuleStatus::Unproven),
    ("generics_typevartuple_basic_2", RuleStatus::Unproven),
    ("generics_typevartuple_specialization", RuleStatus::Unproven),
    (
        "generics_typevartuple_specialization_2",
        RuleStatus::Unproven,
    ),
    ("generics_typevartuple_unpack", RuleStatus::Unproven),
    (
        "generics_upper_bound_2",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("generics_variance", RuleStatus::Unproven),
    ("historical_positional", RuleStatus::Unproven),
    (
        "imports_missing_name",
        RuleStatus::Invalid(InvalidReason::StarvedInput("imported_symbols")),
    ),
    ("imports_module_attribute", RuleStatus::Unproven),
    ("imports_unresolved", RuleStatus::Unproven),
    (
        "literals_parameterizations",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    (
        "literals_parameterizations_2",
        RuleStatus::Invalid(InvalidReason::StarvedInput(
            "literal_string_enum_mismatches",
        )),
    ),
    (
        "literals_semantics",
        RuleStatus::Invalid(InvalidReason::StarvedInput(
            "literal_augmented_assign_violations",
        )),
    ),
    ("match_exhaustiveness", RuleStatus::Unproven),
    (
        "namedtuples_define_class",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("namedtuples_define_functional", RuleStatus::Unproven),
    (
        "namedtuples_type_compat",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    (
        "namedtuples_usage",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("names_unbound", RuleStatus::Unproven),
    ("names_undefined", RuleStatus::Unproven),
    ("narrowing_typeguard", RuleStatus::Unproven),
    ("narrowing_typeis_2", RuleStatus::Unproven),
    ("overloads_basic", RuleStatus::Unproven),
    ("overloads_consistency", RuleStatus::Unproven),
    ("overloads_consistency_2", RuleStatus::Unproven),
    (
        "overloads_consistency_3",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("overloads_definitions", RuleStatus::Unproven),
    ("overloads_evaluation", RuleStatus::Unproven),
    (
        "protocols_class_objects",
        RuleStatus::Invalid(InvalidReason::StarvedInput(
            "protocol_class_object_violations",
        )),
    ),
    (
        "protocols_definition_2",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    (
        "protocols_explicit",
        RuleStatus::Invalid(InvalidReason::StarvedInput(
            "protocol_instantiation_violations",
        )),
    ),
    (
        "protocols_generic",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("protocols_runtime_checkable", RuleStatus::Unproven),
    ("protocols_variance", RuleStatus::Unproven),
    (
        "qualifiers_annotated_2",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("qualifiers_final_annotation_2", RuleStatus::Unproven),
    ("qualifiers_final_decorator", RuleStatus::Unproven),
    ("returns_compatibility", RuleStatus::Unproven),
    ("returns_compatibility_2", RuleStatus::Unproven),
    (
        "specialtypes_never",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    (
        "specialtypes_promotions",
        RuleStatus::Invalid(InvalidReason::StarvedInput("float_param_int_attr_accesses")),
    ),
    ("specialtypes_type", RuleStatus::Unproven),
    (
        "tuples_index",
        RuleStatus::Invalid(InvalidReason::StarvedInput("tuple_index_violations")),
    ),
    ("tuples_type_form", RuleStatus::Unproven),
    ("tuples_type_form_2", RuleStatus::Unproven),
    (
        "typeddicts_alt_syntax",
        RuleStatus::Invalid(InvalidReason::StarvedInput("typeddict_calls")),
    ),
    ("typeddicts_class_syntax", RuleStatus::Unproven),
    ("typeddicts_class_syntax_2", RuleStatus::Unproven),
    ("typeddicts_inheritance", RuleStatus::Unproven),
    ("typeddicts_operations", RuleStatus::Unproven),
    ("typeddicts_readonly", RuleStatus::Unproven),
    (
        "typeddicts_required",
        RuleStatus::Invalid(InvalidReason::NoVerdictPath),
    ),
    ("typeddicts_usage", RuleStatus::Unproven),
    ("version_target_syntax", RuleStatus::Unproven),
];

/// The status recorded for `code`, or `None` if the code is not registered.
#[must_use]
pub fn status_of(code: &str) -> Option<RuleStatus> {
    RULE_STATUS
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .map(|(_, status)| *status)
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test-only: these assertions must abort loudly when a label stops matching the code"
)]
mod tests {
    use super::{InvalidReason, RuleStatus, RULE_STATUS};
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    fn rules_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/rules")
    }

    fn read_all(path: &Path, out: &mut String) {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                read_all(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }

    /// The module directory or file that declares `code`.
    fn module_source_of(code: &str) -> Option<String> {
        fn walk(dir: &Path, needle: &str) -> Option<PathBuf> {
            for entry in std::fs::read_dir(dir).ok()?.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(found) = walk(&path, needle) {
                        return Some(found);
                    }
                } else if path.extension().is_some_and(|ext| ext == "rs")
                    && std::fs::read_to_string(&path)
                        .unwrap_or_default()
                        .contains(needle)
                {
                    return Some(path);
                }
            }
            None
        }
        let declaration = format!("code: \"{code}\"");
        let file = walk(&rules_dir(), &declaration)?;
        let mut source = String::new();
        if file.file_name().is_some_and(|name| name == "mod.rs") {
            read_all(file.parent()?, &mut source);
        } else {
            source = std::fs::read_to_string(&file).unwrap_or_default();
        }
        Some(source)
    }

    /// Every code the rule sources declare must be classified — a new rule
    /// cannot slip in unlabelled and be presented as working coverage.
    #[test]
    fn every_declared_rule_code_is_classified() {
        let mut sources = String::new();
        read_all(&rules_dir(), &mut sources);
        let declared: HashSet<String> = sources
            .match_indices("code: \"")
            .filter_map(|(index, marker)| {
                let rest = sources.get(index + marker.len()..)?;
                let end = rest.find('"')?;
                rest.get(..end).map(str::to_owned)
            })
            .collect();
        let classified: HashSet<String> = RULE_STATUS
            .iter()
            .map(|(code, _)| (*code).to_owned())
            .collect();
        let missing: Vec<&String> = declared.difference(&classified).collect();
        assert!(
            missing.is_empty(),
            "these rule codes are declared but carry no status: {missing:?}. \
             Every registered rule must be classified in RULE_STATUS."
        );
    }

    #[test]
    fn no_rule_is_classified_twice() {
        let mut seen = HashSet::new();
        for (code, _) in RULE_STATUS {
            assert!(seen.insert(*code), "`{code}` is classified more than once");
        }
    }

    /// A rule labelled `NoVerdictPath` must genuinely contain no diagnostic
    /// push. If someone implements it, this test fails and forces the label up.
    #[test]
    fn no_verdict_path_rules_really_have_no_verdict_path() {
        for (code, status) in RULE_STATUS {
            if *status != RuleStatus::Invalid(InvalidReason::NoVerdictPath) {
                continue;
            }
            let source = module_source_of(code)
                .unwrap_or_else(|| panic!("no source declares rule code `{code}`"));
            assert!(
                !source.contains("diagnostics.push") && !source.contains("diagnostics.extend"),
                "`{code}` is labelled Invalid(NoVerdictPath) but its module now emits a \
                 diagnostic. If it genuinely implements its obligation, relabel it — with a \
                 test that proves the verdict."
            );
        }
    }

    /// Whether the resolver hardcodes `field` empty — directly, or through the
    /// one level of renaming the module builder uses
    /// (`violations_field: results.issues_field,`).
    fn field_is_hardcoded_empty(source: &str, field: &str) -> bool {
        let direct = [
            format!("{field}: Vec::new()"),
            format!("{field}: std::collections::HashMap::new()"),
            format!("{field}: HashMap::new()"),
            format!("let {field} = Vec::new()"),
            format!("let {field}: Vec<"),
        ];
        if direct.iter().any(|pattern| source.contains(pattern)) {
            return true;
        }
        // `field: results.other,` — follow the rename to the collected field.
        let marker = format!("{field}: results.");
        let Some(index) = source.find(&marker) else {
            return false;
        };
        let Some(rest) = source.get(index + marker.len()..) else {
            return false;
        };
        let Some(end) = rest.find(',') else {
            return false;
        };
        rest.get(..end)
            .is_some_and(|aliased| field_is_hardcoded_empty(source, aliased.trim()))
    }

    /// A rule labelled `StarvedInput` must name a resolver field the visitor
    /// really does hardcode empty. If the collector is rebuilt, this fails.
    #[test]
    fn starved_inputs_are_really_empty_in_the_resolver() {
        let visitor =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../basilisk-resolver/src/visitor/mod.rs");
        let source = std::fs::read_to_string(&visitor).expect("resolver visitor must be readable");
        for (code, status) in RULE_STATUS {
            let RuleStatus::Invalid(InvalidReason::StarvedInput(field)) = status else {
                continue;
            };
            assert!(
                field_is_hardcoded_empty(&source, field),
                "`{code}` is labelled Invalid(StarvedInput(\"{field}\")) but the resolver no \
                 longer hardcodes that field empty. The collector was rebuilt — relabel the \
                 rule and prove its verdict with a test."
            );
        }
    }

    /// Codes attributed by an obligation in the golden permutation suite: every
    /// string literal appearing near an `assert_by(` / `assert_rejected_by(`
    /// call site. Proximity to the attribution call is required so a code
    /// merely *mentioned* in a fixture cannot count as attributed.
    fn attributed_codes() -> HashSet<String> {
        let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
        let mut sources = String::new();
        read_all(&golden, &mut sources);
        let mut codes = HashSet::new();
        for marker in ["assert_by(", "assert_rejected_by("] {
            for (index, _) in sources.match_indices(marker) {
                let start = index + marker.len();
                let window = sources
                    .get(start..(start + 400).min(sources.len()))
                    .unwrap_or("");
                let mut rest = window;
                while let Some(open) = rest.find('"') {
                    let Some(tail) = rest.get(open + 1..) else {
                        break;
                    };
                    let Some(close) = tail.find('"') else { break };
                    if let Some(literal) = tail.get(..close) {
                        let _ = codes.insert(literal.to_owned());
                    }
                    rest = tail.get(close + 1..).unwrap_or("");
                }
            }
        }
        codes
    }

    /// `Proven` requires an attributed obligation: a golden permutation test
    /// that names the rule's code in `assert_by`/`assert_rejected_by`
    /// ([PERMTEST-FAMILY-B]). Pass-ness is enforced by those obligations being
    /// tests themselves — a Proven rule whose obligation regresses fails the
    /// golden suite, and a Proven label without any attribution fails here.
    #[test]
    fn proven_status_requires_an_attributed_obligation() {
        let attributed = attributed_codes();
        let unattributed: Vec<&str> = RULE_STATUS
            .iter()
            .filter(|(_, status)| *status == RuleStatus::Proven)
            .map(|(code, _)| *code)
            .filter(|code| !attributed.contains(*code))
            .collect();
        assert!(
            unattributed.is_empty(),
            "these rules claim Proven but no golden obligation attributes a diagnostic to \
             them via `assert_by`/`assert_rejected_by` ([PERMTEST-FAMILY-B]): \
             {unattributed:?}. Add the attributed obligation (and make it pass) before \
             claiming Proven."
        );
    }

    /// The inverse direction: an INVALID rule may not simultaneously be the
    /// subject of an attributed obligation someone expects to pass — the two
    /// claims contradict each other, and whichever is stale must be fixed.
    #[test]
    fn invalid_rules_are_not_attributed_in_the_golden_suite() {
        let attributed = attributed_codes();
        let contradictions: Vec<&str> = RULE_STATUS
            .iter()
            .filter(|(_, status)| status.is_invalid())
            .map(|(code, _)| *code)
            .filter(|code| attributed.contains(*code))
            .collect();
        assert!(
            contradictions.is_empty(),
            "these rules are labelled Invalid yet a golden obligation attributes a \
             diagnostic to them: {contradictions:?}. Either the rule was rebuilt (relabel \
             it) or the obligation is aspirational (it must be failing — check it is)."
        );
    }
}
