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
    (
        "!= \"object\"",
        "the same top-type-by-spelling test as `== \"object\"`, negated",
    ),
    (
        "strip_prefix(\"type[\")",
        "decides class-object-ness, and extracts its argument, by string surgery \
         on a rendered generic; `type[X]` is an `Expr::Subscript`",
    ),
    (
        ".find('[')",
        "locates a type argument list inside RENDERED text; a subscript's slice \
         is an AST node, not a character offset",
    ),
    (
        ".rfind(']')",
        "locates the end of a type argument list inside RENDERED text; see \
         `.find('[')`",
    ),
    // ---------------------------------------------------------------------
    // Resolver fields that carry a RENDERED SPELLING rather than a reference.
    // Reading any of them to decide something is the string-keyed hierarchy
    // again: the resolver records "simple names only; complex expressions
    // ignored", so an aliased or dotted form is already lost by the time a
    // rule sees it. They may appear ONLY in diagnostic message text.
    // ---------------------------------------------------------------------
    (
        "base_expression_names",
        "matches TypeVar/base identity against RENDERED names harvested from \
         base-class expressions; resolve the base expression instead",
    ),
    (
        "metaclass_name",
        "identifies a metaclass by the RENDERED TEXT of its `metaclass=` value, \
         so an imported or dotted metaclass never resolves",
    ),
    (
        "constraint_type_names",
        "compares PEP 696 constraints as STRINGS; constraint membership is type \
         equivalence, a `TypeNode` relation",
    ),
    (
        "bound_type_name",
        "compares a PEP 696 bound as a STRING, and is recorded only when the \
         bound `is a simple name` — `bound=list[int]` never arrives at all",
    ),
    (
        "default_type_name",
        "compares a PEP 696 default as a STRING; see `bound_type_name`",
    ),
    (
        "class_name_map",
        "the DELETED name-keyed class hierarchy; do not reintroduce it or vendor \
         a copy under another name",
    ),
    (
        "is_transitive_typeddict",
        "the DELETED name-keyed TypedDict base walk (basilisk-resolver)",
    ),
    // ---------------------------------------------------------------------
    // Re-parsing Python out of its own source text. `ruff_python_parser` has
    // already produced the AST; every construct re-derived from characters
    // disagrees with it somewhere, and the disagreement is always a
    // respelling the tests do not contain.
    // ---------------------------------------------------------------------
    (
        ".split('|')",
        "decomposes a UNION by splitting rendered text on a character; `|` \
         occurs inside `Literal[\"a|b\"]` and inside nested generics, and \
         `Optional[X]` / `Union[X, Y]` contain no `|` at all",
    ),
    (
        "starts_with(\"tuple[\")",
        "recognises a tuple annotation by a six-character PREFIX, so \
         `tuple [int]` and `builtins.tuple[int]` are not tuples",
    ),
    (
        ".split('(')",
        "reads a call's callee out of RAW SOURCE by cutting at a parenthesis; \
         the callee is `ExprCall::func`, an AST node",
    ),
    (
        "rsplit('.')",
        "reduces a dotted expression to its TRAILING WORD, so `models.User` \
         collides with every other `User` in the program; resolve the \
         expression instead",
    ),
    (
        "is_ascii_uppercase",
        "decides that a callee is a CLASS from its capitalisation — a naming \
         convention, not a Python rule",
    ),
    (
        "fn leaf_name",
        "renders a resolved type to a `String` so nominal subtyping can be \
         settled between two spellings; this is `judge::nominal_leaf`, \
         already deleted once",
    ),
    (
        "fn classify_literal",
        "decides a literal's TYPE from its leading characters; the parser \
         already built `Expr::NumberLiteral` / `StringLiteral` / …",
    ),
    (
        "builtin_call_return",
        "the DELETED table mapping a BARE CALLEE NAME to a builtin's return \
         type; builtins are not an exception to binding resolution",
    ),
    (
        "== \"staticmethod\"",
        "identifies the `staticmethod` builtin by its spelling on an \
         `Expr::Name` decorator, so `@builtins.staticmethod` is not one",
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

/// Lines of real code paired with their TRUE 1-based line number.
///
/// Two kinds of line are dropped, because both exist to NAME the forbidden
/// constructs rather than to use them, and neither can produce a verdict:
///
/// * comment lines — the DELETED banners quote what they forbid;
/// * the body of a `panic!(…)` — a deletion's panic message must say which
///   construct it replaced, and that message is the only remaining record of
///   it.
///
/// Numbering still points at the file as it is on disk.
fn code_lines(source: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut in_panic = false;
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if in_panic {
            // A `panic!` message is a multi-line string literal, so counting
            // parens across it is wrong — prose and escapes inside the string
            // unbalance the count, and a single unbalanced message would
            // silently exempt the whole rest of the file. Terminate on the
            // closing line instead, which is how every one of these is
            // formatted.
            if trimmed == ")" || trimmed == ");" || trimmed == ")," {
                in_panic = false;
            }
            continue;
        }
        if trimmed.starts_with("//") {
            continue;
        }
        // A single-line `panic!("…")` closes on its own line and is skipped
        // outright; a multi-line one opens the skip above.
        if trimmed.starts_with("panic!(") {
            if !trimmed.ends_with(")") && !trimmed.ends_with(");") {
                in_panic = true;
            }
            continue;
        }
        out.push((index + 1, line));
    }
    out
}

/// The skip above must never run past the end of one `panic!`. A message that
/// failed to terminate would exempt every line after it — coverage that isn't.
#[test]
fn panic_skipping_does_not_swallow_following_code() {
    let source = "\
fn shell() {
    panic!(
        \"was DELETED because it read metaclass_name (and other things)\"
    )
}

fn live() {
    let x = thing.metaclass_name;
}
";
    let kept: Vec<usize> = code_lines(source).into_iter().map(|(n, _)| n).collect();
    assert!(
        kept.contains(&9),
        "line 9 (`thing.metaclass_name`) is live code AFTER a panic block and must \
         still be scanned; kept lines were {kept:?}"
    );
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
        let lines = code_lines(&raw);
        for (pattern, why) in FORBIDDEN {
            for (number, line) in &lines {
                if line.contains(pattern) {
                    let name = file.strip_prefix(&src).unwrap_or(file).display();
                    offences.push(format!(
                        "  {name}:{number}\n      found: {pattern}\n      why:   {why}"
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
