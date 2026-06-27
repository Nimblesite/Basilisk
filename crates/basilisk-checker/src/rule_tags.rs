//! Implements [CHKTAG] from [CHKARCH-DIAG]. See docs/specs/CHECKER-RULE-TAGGING-SPEC.md#chktag
//!
//! Rule tagging. Basilisk classifies every rule with a flat set of string
//! *tags*, not a hierarchical category system. Each rule carries exactly one
//! provenance tag ([`PEP`] or [`BASILISK`]); PEP rules additionally carry the
//! single PEP *category* they belong to — the only "category" axis Basilisk
//! keeps, taken verbatim from the `python/typing` conformance suite; and any
//! rule may carry free-form descriptive tags (`style`, `redundancy`, …) which
//! must never collide with a reserved PEP-category name.
//!
//! The `BSK-` prefix some rule codes still carry is a cosmetic naming
//! convention for Basilisk-original rules — it is *semantically meaningless* to
//! the checker, which identifies a Basilisk rule solely by its [`BASILISK`] tag
//! via [`tags_for_code`], never by inspecting the prefix.

/// Provenance tag: a core rule selected by the **default** configuration — the
/// `python/typing` conformance rules plus the core checks that run by default.
/// The default config is exactly this "core PEP" set and nothing else.
/// [CHKTAG-PROVENANCE]
pub const PEP: &str = "pep";

/// Provenance tag: a Basilisk-original rule that is **off by default** and turns
/// on only via opt-in configuration. There is no "strict mode"
/// ([CHKARCH-CONFIGURATION-ONLY]); rule selection is config-only, and the default
/// config selects no `basilisk` rule. [CHKTAG-PROVENANCE]
pub const BASILISK: &str = "basilisk";

/// The canonical PEP *categories* — the only category axis Basilisk keeps, and
/// the reserved tag vocabulary that free-form tags must never collide with.
///
/// Sourced verbatim from the `python/typing` conformance suite: the file-name
/// prefixes under `conformance/tests/` and the `category` column of
/// `conformance/conformance_status.csv`. [CHKTAG-PEP-CATEGORIES]
pub const PEP_CATEGORIES: [&str; 21] = [
    "aliases",
    "annotations",
    "callables",
    "classes",
    "constructors",
    "dataclasses",
    "directives",
    "enums",
    "exceptions",
    "generics",
    "historical",
    "literals",
    "namedtuples",
    "narrowing",
    "overloads",
    "protocols",
    "qualifiers",
    "specialtypes",
    "tuples",
    "typeddicts",
    "typeforms",
];

/// The free-form descriptive tags Basilisk currently uses. Each is carefully
/// named to avoid colliding with a reserved PEP-category name; the tagging test
/// ([CHKTAG-TESTS]) asserts this for every entry. [CHKTAG-FREEFORM]
pub const FREE_FORM_TAGS: [&str; 6] = [
    "style",
    "redundancy",
    "strictness",
    "dependencies",
    "imports",
    "stubs",
];

/// The Basilisk-original rules, keyed by current diagnostic code, with the
/// free-form tags each carries. Membership here is what makes a rule
/// [`BASILISK`] — the curated source of truth for provenance, independent of
/// the cosmetic `BSK-` code prefix. Every other rule is a [`PEP`] rule.
///
/// This list **mirrors the opt-in set** gated in `check_with_config`: rule
/// selection is config-only and the default config selects no rule listed here.
/// A rule that runs under the default config is core PEP, not `basilisk` — even
/// if it is a Basilisk-authored check (e.g. unresolved-import and
/// version-target syntax checks run by default, so they are `pep`).
/// [CHKTAG-PROVENANCE]
const BASILISK_RULES: &[(&str, &[&str])] = &[
    // Annotation requirements beyond the spec (the spec never mandates these).
    ("BSK-E0001", &["strictness"]), // missing parameter annotation
    ("BSK-E0002", &["strictness"]), // missing return annotation
    ("BSK-E0003", &["strictness"]), // missing variable type
    ("BSK-E0004", &["strictness"]), // missing *args/**kwargs annotation
    ("BSK-E0005", &["strictness"]), // missing attribute annotation
    ("BSK-E0025", &["strictness"]), // missing @override decorator (PEP 698 keeps it optional)
    ("BSK-W0040", &["strictness"]), // lambda missing annotations
    // Style / redundancy nudges.
    ("BSK-W0014", &["style", "strictness"]), // explicit `Any`
    ("BSK-W0050", &["redundancy", "style"]), // redundant annotation (the headline differentiator)
    // Dependency & lock-file hygiene.
    ("BSK-W0011", &["dependencies", "imports"]), // undeclared dependency import
    ("BSK-W0012", &["dependencies"]),            // unused dependency
    ("BSK-W0013", &["dependencies"]),            // stale lock file
    // Stub hygiene.
    ("BSK-E0152", &["stubs"]), // missing type stubs
];

/// The full tag set for a diagnostic `code`: exactly one provenance tag followed
/// by its PEP category and/or free-form tags. Never panics and never returns an
/// empty set — an unknown code resolves to a bare [`PEP`] rule. [CHKTAG-MODEL]
#[must_use]
pub fn tags_for_code(code: &str) -> Vec<&'static str> {
    if let Some((_, extra)) = BASILISK_RULES.iter().find(|(known, _)| *known == code) {
        let mut tags = Vec::with_capacity(1 + extra.len());
        tags.push(BASILISK);
        tags.extend_from_slice(extra);
        return tags;
    }
    match pep_category_of(code) {
        Some(category) => vec![PEP, category],
        None => vec![PEP],
    }
}

/// The diagnostic codes currently classified as Basilisk-original (the opt-in
/// set). Exposed so tests can assert this table never drifts from the live rule
/// registry — a renamed code here would otherwise silently fall through to a
/// `pep` classification. [CHKTAG-PROVENANCE]
#[must_use]
pub fn basilisk_rule_codes() -> Vec<&'static str> {
    BASILISK_RULES.iter().map(|(code, _)| *code).collect()
}

/// The PEP category a conformance-named rule belongs to, derived from the
/// portion of its code before the first `_`. Returns `None` for cross-cutting
/// core checks (e.g. `returns_compatibility`) that have no single home category.
fn pep_category_of(code: &str) -> Option<&'static str> {
    let prefix = code.split('_').next().unwrap_or(code);
    PEP_CATEGORIES
        .into_iter()
        .find(|category| *category == prefix)
}

/// Whether `tag` is one of the reserved PEP-category names. [CHKTAG-PEP-CATEGORIES]
#[must_use]
pub fn is_pep_category(tag: &str) -> bool {
    PEP_CATEGORIES.contains(&tag)
}

/// Whether `tag` is a provenance tag ([`PEP`] or [`BASILISK`]). [CHKTAG-PROVENANCE]
#[must_use]
pub fn is_provenance(tag: &str) -> bool {
    tag == PEP || tag == BASILISK
}

/// Whether `tag` is admissible as a free-form descriptive tag: non-empty and
/// colliding with neither a provenance tag nor a reserved PEP-category name.
/// This is the guard behind the user's "must not conflict" rule. [CHKTAG-FREEFORM]
#[must_use]
pub fn is_valid_free_form(tag: &str) -> bool {
    !tag.is_empty() && !is_provenance(tag) && !is_pep_category(tag)
}
