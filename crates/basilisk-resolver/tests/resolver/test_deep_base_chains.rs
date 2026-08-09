//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Stack safety and unbounded-depth correctness of the transitive base walk
//! ([`ClassGraph::ancestors`]), the shared foundation of
//! [CHKARCH-DIAG-TYPEDDICT-READONLY-INHERITANCE].
//!
//! The walk is iterative (explicit worklist, zero recursion) and carries a
//! visited set. Together those give two guarantees this file pins: a chain of
//! ANY depth cannot grow the call stack, and a self-referential or cyclic
//! `bases` list terminates instead of blowing up exponentially (GitHub #398).
//!
//! The graph is keyed on DEFINITION SITE, so the lookups here are by class
//! definition, never by rendered name; the identity half of that contract is
//! pinned in `test_resolved_class_hierarchy.rs`.

use std::fmt::Write as _;

use basilisk_resolver::{ClassGraph, ClassInfo};

use super::common::resolve_src;

/// `class C0(TypedDict)` followed by `depth` single-inheritance subclasses.
fn deep_typeddict_chain(depth: usize) -> String {
    let mut src = String::from("from typing import TypedDict\nclass C0(TypedDict):\n    x: int\n");
    for level in 1..=depth {
        let _ = writeln!(src, "class C{level}(C{}):\n    pass", level - 1);
    }
    src
}

/// The one class in `classes` declared with `name`.
///
/// Test-only: these sources declare each name exactly once, so a name picks
/// out one definition. Production lookups go through the definition site.
fn class_named<'a>(classes: &'a [ClassInfo], name: &str) -> Option<&'a ClassInfo> {
    classes.iter().find(|class| class.name == name)
}

/// A 1 000-deep chain resolves without exhausting the stack, and the deepest
/// leaf is still recognised as a `TypedDict`.
///
/// Recursion here would push one frame per level; the iterative walk pushes
/// heap entries instead, so depth costs memory rather than stack. The depth
/// also sits far past any fixed cap — a bounded walk would silently report the
/// leaf as not a `TypedDict`, which is a wrong answer, not a slow one.
#[test]
fn thousand_deep_chain_walks_without_stack_growth() -> Result<(), Box<dyn std::error::Error>> {
    let resolved = resolve_src(&deep_typeddict_chain(1_000))?;
    let graph = ClassGraph::new(&resolved.classes);

    let leaf = class_named(&resolved.classes, "C1000").ok_or("C1000 was not collected")?;
    assert!(
        graph.is_typed_dict(leaf),
        "the 1 000th subclass of a TypedDict is still a TypedDict"
    );
    assert_eq!(
        graph.ancestors(leaf).len(),
        1_001,
        "the leaf's chain is itself plus all 1 000 classes above it, each once"
    );
    assert!(
        class_named(&resolved.classes, "C0Missing").is_none(),
        "a class the module never declares has no definition to look up"
    );
    Ok(())
}

/// A class listing itself twice among its bases terminates. With a
/// depth-bounded recursive walk this input branched at every level and took
/// exponential time (GitHub #398); the visited set makes it linear.
///
/// It is also not its own base: `class C(...)` binds `C` only once the
/// statement completes, so the bases resolve to whatever `C` meant before —
/// here, nothing.
#[test]
fn self_referential_bases_terminate() -> Result<(), Box<dyn std::error::Error>> {
    let resolved = resolve_src("class C(C[int], C[bool]):\n    pass\n")?;
    let graph = ClassGraph::new(&resolved.classes);
    let class = class_named(&resolved.classes, "C").ok_or("C was not collected")?;

    assert!(
        !graph.is_typed_dict(class),
        "a self-referential class is not a TypedDict, and deciding that terminates"
    );
    assert_eq!(
        graph.ancestors(class).len(),
        1,
        "a class cannot be its own ancestor"
    );
    Ok(())
}

/// Two classes naming each other as bases — the general cycle — terminates.
#[test]
fn mutually_recursive_bases_terminate() -> Result<(), Box<dyn std::error::Error>> {
    let resolved = resolve_src("class A(B):\n    pass\nclass B(A):\n    pass\n")?;
    let graph = ClassGraph::new(&resolved.classes);
    let a = class_named(&resolved.classes, "A").ok_or("A was not collected")?;
    let b = class_named(&resolved.classes, "B").ok_or("B was not collected")?;

    assert!(!graph.is_typed_dict(a));
    assert!(!graph.is_typed_dict(b));
    // `A`'s base `B` is not yet bound where it is written, so that edge does
    // not exist; `B`'s base `A` does, because `A`'s statement has completed.
    assert_eq!(graph.ancestors(a).len(), 1);
    assert_eq!(graph.ancestors(b).len(), 2);
    Ok(())
}
