//! Implements [TYPEINF-SUBTYPING-NOMINAL]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-SUBTYPING-NOMINAL
//!
//! Pins the 2026-08-09 review finding §3, "nominal identity is lost at the
//! checker boundary".
//!
//! `InferredType::Named` carried a `String` — a RENDERING. Every consumer that
//! needed to know *which class* a leaf denoted had two options and both were
//! wrong:
//!
//! * compare the characters (`types.rs`'s `a == b` arm, `judge.rs`), so two
//!   unrelated classes that happen to be spelled the same are one type; or
//! * re-parse the rendering with `ruff_python_parser` and resolve the
//!   reconstructed expression (`nominal.rs::definition_site`,
//!   `AnnotationResolver::is_grounded_name`). Re-parsing recovers a *syntax
//!   tree*, never the original node — its use-site offset, its enclosing
//!   scope, and the binding it actually resolved through are all gone.
//!
//! The leaf must carry the definition site it was resolved to, taken from the
//! annotation's own AST node at the moment the cascade resolved it. Identity
//! is then a `Span` comparison and the rendering is left for diagnostic
//! MESSAGES, where spelling is the point.
//!
//! Fixtures use vocabulary the python/typing conformance suite does not
//! contain, and every typing import is aliased.
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

use basilisk_checker::annotation::resolve_annotation;
use basilisk_checker::types::InferredType;
use basilisk_resolver::ResolvedModule;
use ruff_python_ast::Stmt;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A parsed module together with its resolution — the pair every annotation
/// question needs, because the annotation's AST node is the input and the
/// resolved module is the namespace.
struct Fixture {
    parsed: basilisk_parser::ParsedModule,
    module: ResolvedModule,
}

/// Parse and resolve a module the same way the checker does.
fn fixture(source: &str) -> Result<Fixture, Box<dyn std::error::Error>> {
    let parsed = basilisk_parser::parse_source(source.to_owned(), "test.py".to_owned())?;
    let module = basilisk_resolver::resolve(&parsed)?;
    Ok(Fixture { parsed, module })
}

impl Fixture {
    /// The type of the annotation on the module-level `name: <annotation>`
    /// statement, resolved from that statement's OWN AST node.
    fn annotated(&self, name: &str) -> Result<InferredType, String> {
        self.parsed
            .ast
            .body
            .iter()
            .find_map(|stmt| {
                let Stmt::AnnAssign(ann) = stmt else {
                    return None;
                };
                let target = ann.target.as_name_expr()?;
                (target.id.as_str() == name)
                    .then(|| resolve_annotation(&self.module, &ann.annotation))
            })
            .ok_or_else(|| format!("no module-level annotation for `{name}`"))
    }
}

// ---------------------------------------------------------------------------
// Two different classes that RENDER the same are two different types
// ---------------------------------------------------------------------------

/// A module that declares its own `Sequence` and also imports `typing`
/// contains TWO classes reachable under that spelling. The rendering cannot
/// tell them apart; the definition site can.
///
/// This is the defect in its most direct form. `AnnotationResolver::attribute`
/// rewrote `tp.Sequence` to the bare member name `Sequence` and then pushed it
/// through the SAME name cascade a bare spelling takes — where the module's
/// own class table answers first. The annotation `tp.Sequence`, which names a
/// class in another module entirely, came back as this module's class.
#[test]
fn a_typing_qualified_name_is_not_the_local_class_spelled_the_same() -> TestResult {
    let fixture = fixture(
        "\
import typing as tp

class Sequence: ...

mine: Sequence
theirs: tp.Sequence
",
    )?;
    let mine = fixture.annotated("mine")?;
    let theirs = fixture.annotated("theirs")?;

    assert_ne!(
        mine, theirs,
        "`Sequence` declared here and `tp.Sequence` are different classes; a \
         leaf that cannot tell them apart is carrying a spelling, not an \
         identity"
    );
    Ok(())
}

/// The same defect through a `from`-import, which reaches the leaf by a
/// different route (`imported_leaf`) and must give the same answer.
#[test]
fn a_from_imported_symbol_is_not_the_local_class_spelled_the_same() -> TestResult {
    let fixture = fixture(
        "\
from typing import Sized as Sequence

class Sequence: ...

mine: Sequence
",
    )?;
    // The class statement runs after the import, so `Sequence` at module end
    // is the CLASS. What must never happen is the leaf being indistinguishable
    // from the imported symbol's leaf.
    let mine = fixture.annotated("mine")?;
    assert_ne!(
        mine,
        InferredType::Named("Sized".to_owned()),
        "a rendering-only leaf lets an import alias and a class collide"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// One class reached by two spellings is one type
// ---------------------------------------------------------------------------

/// The converse obligation, and the reason "make everything unequal" is not a
/// fix: an assignment alias denotes the very same class, so the two leaves
/// must compare EQUAL however differently they are spelled.
#[test]
fn an_assignment_alias_denotes_the_same_class_as_its_target() -> TestResult {
    let fixture = fixture(
        "\
class Beacon: ...

Lighthouse = Beacon

direct: Beacon
aliased: Lighthouse
",
    )?;
    let direct = fixture.annotated("direct")?;
    let aliased = fixture.annotated("aliased")?;

    assert_eq!(
        direct, aliased,
        "`Lighthouse = Beacon` binds the Beacon CLASS; both annotations name \
         one class and must resolve to one type"
    );
    Ok(())
}

/// Reformatting an annotation changes no identity. A leaf keyed on rendering
/// is at the mercy of whitespace the moment anything reconstructs it.
#[test]
fn whitespace_inside_an_annotation_changes_no_identity() -> TestResult {
    let fixture = fixture(
        "\
class Trellis: ...

tight: Trellis
loose: (
    Trellis
)
",
    )?;
    assert_eq!(
        fixture.annotated("tight")?,
        fixture.annotated("loose")?,
        "a parenthesised annotation denotes the same class"
    );
    Ok(())
}

/// A quoted forward reference is the same class as the bare spelling
/// ([PEP 484](https://peps.python.org/pep-0484/#forward-references)).
#[test]
fn a_quoted_forward_reference_denotes_the_same_class() -> TestResult {
    let fixture = fixture(
        "\
class Pergola: ...

bare: Pergola
quoted: \"Pergola\"
",
    )?;
    assert_eq!(
        fixture.annotated("bare")?,
        fixture.annotated("quoted")?,
        "a forward reference resolves to the class it names"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The rendering survives, for messages only
// ---------------------------------------------------------------------------

/// Identity is not allowed to cost the diagnostic text. A nominal leaf still
/// renders as the class's name, because that is what a user reads in a
/// message.
#[test]
fn a_nominal_leaf_still_renders_as_its_class_name() -> TestResult {
    let fixture = fixture(
        "\
class Espalier: ...

plant: Espalier
",
    )?;
    assert_eq!(
        fixture.annotated("plant")?.to_string(),
        "Espalier",
        "the rendering is what a diagnostic message shows"
    );
    Ok(())
}
