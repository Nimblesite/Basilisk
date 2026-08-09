//! Architectural guard: **a type may never be decided from its rendering.**
//!
//! [CHKARCH] / [ASTREBUILD-LAW]: recognition is a question about definitions,
//! answered from the AST and the binding table. Rendering a resolved type back
//! into a `String` and then doing string surgery on it — splitting at `[`,
//! comparing against `"int"`/`"object"`/`"type"`, `starts_with`/`strip_prefix`
//! on a type name — is the same defect as matching raw source text, one level
//! down. It is what got Basilisk withdrawn from the conformance results, and
//! it is what the deleted `crate::subtyping`, `judge::nominal_leaf`, and
//! `generics_basic_3::is_subtype_of` all did.
//!
//! This test FAILS while any such site remains, and NAMES each one. It exists
//! so the deleted layer cannot come back — not under its old name, not
//! vendored into a rule, and not as a placeholder that answers every query the
//! same way to make the build go green.
//!
//! **The only lawful way to satisfy this test is to derive the verdict from
//! resolved bindings / canonical `TypeNode`s, or to abstain.** Never by
//! deleting a case from the forbidden list, never by adding a broad allowance,
//! and never by renaming a helper to dodge the pattern.
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

use std::path::{Path, PathBuf};

/// Constructs that decide a type from its spelling. Each entry is
/// (pattern, why it is forbidden).
const FORBIDDEN: &[(&str, &str)] = &[
    (
        ".split('[')",
        "parses a type out of a RENDERED string by splitting at a bracket; \
         a type's structure comes from the AST, never from its rendering",
    ),
    (
        "== \"object\"",
        "recognises the top type by its builtin SPELLING, so a user class named \
         `object` is treated as `builtins.object` and an aliased import is not; \
         resolve to TypingForm::ObjectClass instead",
    ),
    (
        "starts_with(\"type[\")",
        "decides class-object-ness with a substring test on a rendered generic",
    ),
    (
        "SubtypingContext",
        "the string-keyed nominal hierarchy, DELETED; do not reintroduce it or \
         vendor a copy under another name",
    ),
    (
        "fn name_subtype",
        "settles subtyping between two NAME STRINGS; subtyping is a relation \
         between resolved types",
    ),
];

/// Files exempt for a stated reason — NOT a general escape hatch. A new entry
/// here needs a reason that survives review; "it was easier" does not.
fn is_exempt(path: &Path) -> bool {
    // This file necessarily contains every forbidden pattern as a literal.
    path.file_name()
        .is_some_and(|name| name == "no_type_spelling_surgery_tests.rs")
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") && !is_exempt(&path) {
            out.push(path);
        }
    }
}

/// Strip line comments so the DELETED banners — which quote the forbidden
/// constructs in order to forbid them — do not count as violations.
fn code_only(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn no_verdict_is_derived_from_a_types_spelling() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_sources(&src, &mut files);
    assert!(!files.is_empty(), "no checker sources found");

    let mut offences: Vec<String> = Vec::new();
    for file in &files {
        let Ok(raw) = std::fs::read_to_string(file) else {
            continue;
        };
        let source = code_only(&raw);
        for (pattern, why) in FORBIDDEN {
            for (index, line) in source.lines().enumerate() {
                if line.contains(pattern) {
                    let name = file.strip_prefix(&src).unwrap_or(file).display();
                    offences.push(format!(
                        "  {name}:{}\n      found: {pattern}\n      why:   {why}",
                        index + 1
                    ));
                }
            }
        }
    }

    assert!(
        offences.is_empty(),
        "\n{} site(s) still decide a type from its SPELLING rather than from resolved \
         bindings:\n\n{}\n\n\
         Each is a verdict derived from how a type happens to be written, so it moves \
         when the source is respelled and stays wrong when it is not. Fix by resolving \
         through the binding table / canonical `TypeNode`, or by abstaining. NEVER by \
         removing a case from FORBIDDEN, broadening the exemption list, or renaming the \
         helper to dodge the pattern.\n",
        offences.len(),
        offences.join("\n")
    );
}
