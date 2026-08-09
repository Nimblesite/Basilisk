//! Implements [CHKARCH-CONFORMANCE-MODE]: an answer derived from evidence the
//! module does not have is an abstention, never a negative.
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE-MODE
//!
//! Pins the 2026-08-09 review finding against `scope/class_graph.rs`: the
//! ancestry walk silently DROPPED every base it could not resolve to a class
//! this module defines, and returned the surviving chain as if it were the
//! whole one. A caller asking "is `Sapling` a subclass of `Orchard`?" then read
//! the absence of `Orchard` from a truncated chain as proof of no relationship:
//!
//! ```python
//! from nursery import Rootstock     # a subclass of `Orchard` — unknowable here
//!
//! class Orchard: ...
//! class Sapling(Rootstock): ...     # chain walks to [Sapling] and stops
//! ```
//!
//! `Sapling` may well be an `Orchard`. The walk has one edge it cannot follow,
//! and the honest report is "unknown". A `false` here becomes a diagnostic the
//! user cannot act on — the checker asserting a negative about code it never
//! saw.
//!
//! The fixtures are ordinary horticulture, not conformance-suite vocabulary.

use basilisk_resolver::{ClassGraph, ClassInfo};

use super::common::resolve_src;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The one class in `classes` declared with `name`.
///
/// Test-only: these sources declare each name exactly once. Production lookups
/// go through the definition site.
fn class_named<'a>(classes: &'a [ClassInfo], name: &str) -> Option<&'a ClassInfo> {
    classes.iter().find(|class| class.name == name)
}

// ---------------------------------------------------------------------------
// An edge the module cannot follow makes the ancestry incomplete
// ---------------------------------------------------------------------------

/// A base imported from another module is an edge this graph cannot follow.
/// The chain it yields is a LOWER BOUND, and must say so.
#[test]
fn an_imported_base_leaves_the_ancestry_incomplete() -> TestResult {
    let resolved = resolve_src(
        "\
from nursery import Rootstock

class Orchard: ...

class Sapling(Rootstock): ...
",
    )?;
    let graph = ClassGraph::new(&resolved.classes);
    let sapling = class_named(&resolved.classes, "Sapling").ok_or("Sapling was not collected")?;

    let ancestry = graph.ancestry(sapling);
    assert_eq!(
        ancestry.classes.len(),
        1,
        "only `Sapling` itself is reachable — `Rootstock` is another module's"
    );
    assert!(
        !ancestry.complete,
        "an unfollowable base must mark the walk INCOMPLETE: `Sapling` may \
         still be an `Orchard` through `Rootstock`"
    );
    Ok(())
}

/// An unfollowable edge ANYWHERE in the chain, not just on the class asked
/// about, taints the whole walk.
#[test]
fn an_imported_base_higher_up_the_chain_still_taints_the_ancestry() -> TestResult {
    let resolved = resolve_src(
        "\
from nursery import Rootstock

class Sapling(Rootstock): ...
class Cutting(Sapling): ...
class Graft(Cutting): ...
",
    )?;
    let graph = ClassGraph::new(&resolved.classes);
    let graft = class_named(&resolved.classes, "Graft").ok_or("Graft was not collected")?;

    let ancestry = graph.ancestry(graft);
    assert_eq!(
        ancestry.classes.len(),
        3,
        "`Graft`, `Cutting`, `Sapling` are local; `Rootstock` is not"
    );
    assert!(
        !ancestry.complete,
        "the unfollowable edge is two levels up, and still bounds what this \
         module can claim about `Graft`"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// A fully local hierarchy is complete, and so is one rooted at a known form
// ---------------------------------------------------------------------------

/// Every base resolved to a class this module defines: the walk saw the whole
/// hierarchy and a negative answer from it is real evidence.
#[test]
fn a_wholly_local_hierarchy_is_complete() -> TestResult {
    let resolved = resolve_src(
        "\
class Orchard: ...
class Sapling(Orchard): ...
class Cutting(Sapling): ...
",
    )?;
    let graph = ClassGraph::new(&resolved.classes);
    let cutting = class_named(&resolved.classes, "Cutting").ok_or("Cutting was not collected")?;

    let ancestry = graph.ancestry(cutting);
    assert_eq!(
        ancestry.classes.len(),
        3,
        "the whole chain is in this module"
    );
    assert!(
        ancestry.complete,
        "no edge was left unfollowed, so `Cutting`'s ancestry is known exactly"
    );
    Ok(())
}

/// A class with no bases at all has nothing left unfollowed.
#[test]
fn a_class_with_no_bases_is_complete() -> TestResult {
    let resolved = resolve_src("class Orchard: ...\n")?;
    let graph = ClassGraph::new(&resolved.classes);
    let orchard = class_named(&resolved.classes, "Orchard").ok_or("Orchard was not collected")?;

    assert!(
        graph.ancestry(orchard).complete,
        "a base list with nothing in it hides nothing"
    );
    Ok(())
}

/// `object` is a RESOLVED base, not an unknown one. It contributes no local
/// edge — there is no `class object` here to walk into — but the graph knows
/// exactly what it is, so the ancestry stays complete.
#[test]
fn an_explicit_object_base_is_resolved_not_unknown() -> TestResult {
    let resolved = resolve_src(
        "\
class Orchard(object): ...
class Sapling(Orchard): ...
",
    )?;
    let graph = ClassGraph::new(&resolved.classes);
    let sapling = class_named(&resolved.classes, "Sapling").ok_or("Sapling was not collected")?;

    let ancestry = graph.ancestry(sapling);
    assert_eq!(
        ancestry.classes.len(),
        2,
        "`object` is not one of this module's classes"
    );
    assert!(
        ancestry.complete,
        "`object` resolves to a known form; knowing a base is `object` is \
         knowledge, not a gap"
    );
    Ok(())
}

/// The alias case, again: `Espalier = Orchard` is followed to `Orchard`, so
/// nothing is unfollowed and the walk is complete. A graph that lost the alias
/// edge would report both a shorter chain AND — wrongly — completeness.
#[test]
fn a_base_reached_through_an_alias_is_a_followed_edge() -> TestResult {
    let resolved = resolve_src(
        "\
class Orchard: ...

Espalier = Orchard

class Sapling(Espalier): ...
",
    )?;
    let graph = ClassGraph::new(&resolved.classes);
    let sapling = class_named(&resolved.classes, "Sapling").ok_or("Sapling was not collected")?;

    let ancestry = graph.ancestry(sapling);
    assert_eq!(
        ancestry.classes.len(),
        2,
        "the alias resolves to `Orchard`, so the edge exists"
    );
    assert!(
        ancestry.complete,
        "the alias edge was followed, not dropped"
    );
    Ok(())
}
