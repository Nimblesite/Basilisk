//! Implements [CHKTAG-TESTS] from [CHKTAG]. See docs/specs/CHECKER-RULE-TAGGING-SPEC.md#chktag-tests
//!
//! Coarse e2e for the rule tagging system: every shipping rule code resolves to
//! a valid, conflict-free tag set, and the user-facing invariants hold —
//! exactly one provenance tag per rule, PEP-category tags only on `pep` rules,
//! and free-form tags never colliding with a reserved PEP-category name.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use basilisk_checker::rule_tags::{
    basilisk_rule_codes, is_pep_category, is_provenance, is_valid_free_form, tags_for_code,
    BASILISK, FREE_FORM_TAGS, PEP, PEP_CATEGORIES,
};

fn rules_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("rules")
}

/// Every `code: "X"` literal under `src/rules` — the codes the registry emits.
fn all_rule_codes() -> BTreeSet<String> {
    let mut codes = BTreeSet::new();
    collect_codes(&rules_dir(), &mut codes);
    codes
}

fn collect_codes(dir: &Path, out: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_codes(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            if let Ok(text) = fs::read_to_string(&path) {
                for code in extract_codes(&text) {
                    let _ = out.insert(code);
                }
            }
        }
    }
}

/// Pull `code: "..."` occurrences without a regex dependency.
fn extract_codes(text: &str) -> Vec<String> {
    const NEEDLE: &str = "code: \"";
    let mut codes = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find(NEEDLE) {
        rest = &rest[pos + NEEDLE.len()..];
        if let Some(end) = rest.find('"') {
            codes.push(rest[..end].to_owned());
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
    codes
}

#[test]
fn finds_the_whole_rule_set() {
    // Ratcheted near the real count (159 at time of writing) so a parser
    // regression that quietly drops a chunk of rules — shrinking the reach of
    // every `for code in all_rule_codes()` invariant test — fails loudly.
    assert!(
        all_rule_codes().len() >= 150,
        "expected to scan the full rule registry"
    );
}

/// [CHKTAG-INVARIANTS] #1 / [CHKTAG-PROVENANCE]: exactly one provenance tag.
#[test]
fn every_rule_has_exactly_one_provenance_tag() {
    for code in all_rule_codes() {
        let tags = tags_for_code(&code);
        let provenance = tags.iter().filter(|tag| is_provenance(tag)).count();
        assert_eq!(
            provenance, 1,
            "rule `{code}` must carry exactly one provenance tag, got {tags:?}"
        );
    }
}

/// [CHKTAG-INVARIANTS] #2 / [CHKTAG-PEP-CATEGORIES]: category only on `pep` rules.
#[test]
fn pep_category_tags_appear_only_on_pep_rules() {
    for code in all_rule_codes() {
        let tags = tags_for_code(&code);
        if tags.iter().any(|tag| is_pep_category(tag)) {
            assert!(
                tags.contains(&PEP),
                "rule `{code}` carries a PEP-category tag without `pep` provenance: {tags:?}"
            );
        }
    }
}

/// [CHKTAG-INVARIANTS] #3 / [CHKTAG-FREEFORM]: emitted non-reserved tags are valid.
#[test]
fn no_emitted_tag_is_an_invalid_free_form() {
    // The user's core safety rule: a tag that is neither provenance nor a PEP
    // category is a free-form tag, and free-form tags must never collide with a
    // reserved PEP-category name.
    for code in all_rule_codes() {
        for tag in tags_for_code(&code) {
            if !is_provenance(tag) && !is_pep_category(tag) {
                assert!(
                    is_valid_free_form(tag),
                    "free-form tag `{tag}` on `{code}` collides with the reserved vocabulary"
                );
            }
        }
    }
}

/// [CHKTAG-INVARIANTS] #6 / [CHKTAG-FREEFORM]: every declared `FREE_FORM_TAGS` valid.
#[test]
fn declared_free_form_vocabulary_never_collides() {
    for tag in FREE_FORM_TAGS {
        assert!(
            is_valid_free_form(tag),
            "declared free-form tag `{tag}` collides with a PEP category or provenance tag"
        );
    }
}

/// [CHKTAG-INVARIANTS] #5 / [CHKTAG-PEP-CATEGORIES]: unique, lowercase, non-provenance.
#[test]
fn pep_categories_are_unique_lowercase_and_distinct_from_provenance() {
    let unique: BTreeSet<_> = PEP_CATEGORIES.iter().collect();
    assert_eq!(unique.len(), PEP_CATEGORIES.len());
    for category in PEP_CATEGORIES {
        assert!(!category.is_empty());
        assert_eq!(category, category.to_ascii_lowercase());
        assert!(!is_provenance(category));
        assert!(is_pep_category(category));
    }
}

/// [CHKTAG-MODEL] / [CHKTAG-PEP-CATEGORIES]: category derived from the code prefix.
#[test]
fn pep_rules_derive_their_category_from_the_conformance_name_prefix() {
    assert_eq!(tags_for_code("aliases_newtype"), vec![PEP, "aliases"]);
    assert_eq!(tags_for_code("narrowing_typeguard"), vec![PEP, "narrowing"]);
    assert_eq!(
        tags_for_code("generics_typevartuple_basic_2"),
        vec![PEP, "generics"]
    );
    assert_eq!(
        tags_for_code("typeddicts_required"),
        vec![PEP, "typeddicts"]
    );
}

/// [CHKTAG-MODEL] / [CHKTAG-PEP-CATEGORIES]: cross-cutting core checks are bare `pep`.
#[test]
fn cross_cutting_core_checks_are_pep_without_a_category() {
    // Checks with no single home category (return/call/assignment/name checks)
    // are still `pep`, just uncategorised.
    assert_eq!(tags_for_code("returns_compatibility"), vec![PEP]);
    assert_eq!(tags_for_code("calls_argument_type"), vec![PEP]);
    assert_eq!(tags_for_code("names_undefined"), vec![PEP]);
}

/// [CHKTAG-INVARIANTS] #4 / [CHKTAG-PROVENANCE]: `basilisk` rules carry no category.
#[test]
fn basilisk_rules_are_tagged_basilisk_and_never_carry_a_pep_category() {
    for code in [
        "BSK-E0001",
        "BSK-E0025",
        "BSK-W0014",
        "BSK-W0050",
        "BSK-E0152",
    ] {
        let tags = tags_for_code(code);
        assert!(
            tags.contains(&BASILISK),
            "`{code}` should be a Basilisk rule, got {tags:?}"
        );
        assert!(!tags.contains(&PEP), "`{code}` must not also be `pep`");
        for tag in &tags {
            assert!(
                !is_pep_category(tag),
                "Basilisk rule `{code}` must not carry PEP category `{tag}`"
            );
        }
    }
}

/// [CHKTAG-INVARIANTS] #9 / [CHKTAG-PROVENANCE]: default-on Basilisk-authored = `pep`.
#[test]
fn default_on_core_checks_are_pep_not_basilisk() {
    // Per the config-only model: rule selection is via `check_with_config`, and
    // the default config selects exactly the core PEP set. These checks have no
    // gate in `check_with_config`, so they run by default → they are core PEP,
    // NOT `basilisk` — even though `version_target_syntax` is Basilisk-authored.
    for code in [
        "imports_unresolved",
        "imports_module_attribute",
        "version_target_syntax",
    ] {
        let tags = tags_for_code(code);
        assert!(
            tags.contains(&PEP),
            "default-on `{code}` must be `pep`, got {tags:?}"
        );
        assert!(
            !tags.contains(&BASILISK),
            "default-on `{code}` must not be `basilisk`"
        );
    }
}

/// [CHKTAG-INVARIANTS] #7 / [CHKTAG-IMPL]: every `opt_in_spec()` code is a live code.
#[test]
fn no_basilisk_rule_key_is_stale() {
    // Provenance is self-declared by each rule's `opt_in_spec()`. If a rule's
    // diagnostic code is renamed but its `opt_in_spec().code` is not (or vice
    // versa), the declared code no longer matches any emitted code — catch it
    // here, since that rule would silently fall through to on-by-default `pep`.
    let live = all_rule_codes();
    for code in basilisk_rule_codes() {
        assert!(
            live.contains(code),
            "opt_in_spec() code `{code}` is not a live rule code — provenance drifted (a rename?)"
        );
    }
}

/// [CHKTAG-INVARIANTS] #10 / [CHKTAG-BSK-PREFIX]: opt-in set == `BSK-`-prefixed set.
#[test]
fn basilisk_provenance_matches_the_bsk_naming_convention() {
    // The `BSK-` prefix is cosmetic — provenance is decided by each rule's
    // self-declared `opt_in_spec()`, never the prefix. But the convention must
    // hold both ways, which catches the two drift bugs the deleted hand-lists
    // used to hide: a new `BSK-` rule that forgot to tag itself (would silently
    // become on-by-default `pep`), and a non-`BSK-` rule wrongly tagged opt-in
    // (would silently vanish from the default PEP set).
    let declared_basilisk: BTreeSet<String> = basilisk_rule_codes()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let bsk_prefixed: BTreeSet<String> = all_rule_codes()
        .into_iter()
        .filter(|code| code.starts_with("BSK-"))
        .collect();
    assert_eq!(
        declared_basilisk, bsk_prefixed,
        "every BSK-prefixed rule must self-declare opt_in_spec(), and only those"
    );
}

/// [CHKTAG-INVARIANTS] #8 / [CHKTAG-PEP-CATEGORIES]: categories are real test prefixes.
#[test]
fn pep_categories_match_conformance_test_prefixes() {
    // [CHKTAG-PEP-CATEGORIES]: the category vocabulary is taken verbatim from the
    // `python/typing` conformance suite. Assert (read-only) that every category
    // is a real test-file prefix so the list cannot silently drift from source.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("conformance")
        .join("tests");
    let prefixes: BTreeSet<String> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            Path::new(name)
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("py") || ext.eq_ignore_ascii_case("pyi")
                })
        })
        .filter_map(|name| {
            name.trim_start_matches('_')
                .split('_')
                .next()
                .map(str::to_owned)
        })
        .collect();
    assert!(
        !prefixes.is_empty(),
        "found no conformance test files under {dir:?}"
    );
    for category in PEP_CATEGORIES {
        assert!(
            prefixes.contains(category),
            "PEP category `{category}` is not a conformance test-file prefix — vocabulary drifted from its source"
        );
    }
}

/// [CHKTAG-MODEL] / [CHKTAG-FREEFORM]: worked example `BSK-W0050 -> redundancy + style`.
#[test]
fn redundant_annotation_carries_redundancy_and_style() {
    let tags = tags_for_code("BSK-W0050");
    assert!(tags.contains(&"redundancy"));
    assert!(tags.contains(&"style"));
}

/// [CHKTAG-FREEFORM] / [CHKTAG-PROVENANCE] / [CHKTAG-PEP-CATEGORIES]: vocab predicates.
#[test]
fn predicates_reject_reserved_names_as_free_form() {
    assert!(!is_valid_free_form("pep"));
    assert!(!is_valid_free_form("basilisk"));
    assert!(!is_valid_free_form("aliases"));
    assert!(!is_valid_free_form(""));
    assert!(is_valid_free_form("style"));
    assert!(is_provenance("pep"));
    assert!(is_provenance("basilisk"));
    assert!(!is_provenance("aliases"));
    assert!(!is_pep_category("style"));
}
