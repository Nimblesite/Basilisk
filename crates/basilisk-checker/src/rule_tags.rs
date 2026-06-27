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

/// The opt-in tag declaration a Basilisk-original rule attaches to itself.
///
/// A rule returns `Some(OptInSpec { .. })` from [`crate::rules::Rule::opt_in_spec`]
/// to declare that it is a [`BASILISK`] rule (off by default, opt-in only) and to
/// list the free-form tags its diagnostic carries. Core [`PEP`] rules return
/// `None` and are always selected by the default configuration.
///
/// This is the **single source of rule provenance**: there is no central rule
/// list. A rule's provenance and tags live on the rule itself, and both the
/// selection in `check_with_config` and the classification in [`tags_for_code`]
/// read them from here. To add a Basilisk rule, tag the rule — nothing else.
/// [CHKTAG-PROVENANCE]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptInSpec {
    /// The diagnostic code this opt-in rule emits (e.g. `"BSK-W0050"`).
    pub code: &'static str,
    /// The free-form tags the rule carries, beyond its [`BASILISK`] provenance.
    pub tags: &'static [&'static str],
}

/// Every Basilisk-original rule's self-declared [`OptInSpec`], gathered once from
/// the live rule registry — never a hand-maintained list. [CHKTAG-PROVENANCE]
fn opt_in_specs() -> &'static [OptInSpec] {
    static CACHE: std::sync::OnceLock<Vec<OptInSpec>> = std::sync::OnceLock::new();
    CACHE.get_or_init(crate::rules::opt_in_specs).as_slice()
}

/// The [`OptInSpec`] a diagnostic `code` was declared with, if it is a Basilisk
/// (opt-in) rule. Returns `None` for core [`PEP`] rules — the answer a caller
/// uses to gate selection without ever consulting a code list. [CHKTAG-PROVENANCE]
#[must_use]
pub fn opt_in_spec_for_code(code: &str) -> Option<OptInSpec> {
    opt_in_specs()
        .iter()
        .copied()
        .find(|spec| spec.code == code)
}

/// The full tag set for a diagnostic `code`: exactly one provenance tag followed
/// by its PEP category and/or free-form tags. Never panics and never returns an
/// empty set — an unknown code resolves to a bare [`PEP`] rule. [CHKTAG-MODEL]
#[must_use]
pub fn tags_for_code(code: &str) -> Vec<&'static str> {
    if let Some(spec) = opt_in_spec_for_code(code) {
        let mut tags = Vec::with_capacity(1 + spec.tags.len());
        tags.push(BASILISK);
        tags.extend_from_slice(spec.tags);
        return tags;
    }
    match pep_category_of(code) {
        Some(category) => vec![PEP, category],
        None => vec![PEP],
    }
}

/// The diagnostic codes currently classified as Basilisk-original (the opt-in
/// set), gathered from the rules' own [`OptInSpec`] declarations. Exposed so
/// tests can assert provenance never drifts from the live rule registry — a
/// declared code that no live rule emits is a rename that escaped its tag.
/// [CHKTAG-PROVENANCE]
#[must_use]
pub fn basilisk_rule_codes() -> Vec<&'static str> {
    opt_in_specs().iter().map(|spec| spec.code).collect()
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
