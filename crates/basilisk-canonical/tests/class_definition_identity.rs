//! Implements [RESOLV-CANONICAL-BINDING]: a class is identified by the
//! DEFINITION SITE it was bound from, not by the spelling of the name that
//! reached it. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#RESOLV-CANONICAL-BINDING
//!
//! Pins the 2026-08-09 review finding against `src/binding.rs`: the DEFERRED
//! follower resolved an assignment alias by looking its target name up in the
//! module's FINAL namespace, discarding the offset the alias captured. An
//! assignment binds an OBJECT, not a name:
//!
//! ```python
//! class Old: ...
//! Alias = Old        # `Alias` is now the first `Old` object, forever
//! class Old: ...     # rebinds the NAME `Old`; `Alias` is unaffected
//! ```
//!
//! Following `Alias` through the final binding of `Old` reaches the SECOND
//! class — a class the alias has never referred to at any point in the
//! program's life. The positional follower already keeps the captured object;
//! these tests require the deferred one to agree, because the two differ only
//! in WHEN the alias name itself is looked up, never in what its assignment
//! captured.
//!
//! Every fixture here is authored in vocabulary the python/typing conformance
//! suite does not contain.

use basilisk_canonical::BindingTable;
use ruff_python_ast::{ModModule, Stmt};
use ruff_text_size::{Ranged as _, TextRange};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Parse Python source into a module AST.
fn parsed(source: &str) -> Result<ModModule, ruff_python_parser::ParseError> {
    Ok(ruff_python_parser::parse_module(source)?.into_syntax())
}

/// The name-token ranges of every module-level `class <name>` statement, in
/// declaration order. Two classes spelled the same yield two distinct ranges —
/// which is the whole point: these are the identities under test.
fn class_sites(body: &[Stmt], name: &str) -> Vec<TextRange> {
    body.iter()
        .filter_map(|stmt| match stmt {
            Stmt::ClassDef(class) if class.name.as_str() == name => Some(class.name.range()),
            _ => None,
        })
        .collect()
}

/// The definition site `expr_source` denotes, resolved in the module's FINAL
/// namespace — the resolution a PEP 484 lazily evaluated annotation sees.
fn deferred_site(source: &str, expr_source: &str) -> Result<Option<TextRange>, String> {
    let module = parsed(source).map_err(|error| error.to_string())?;
    let table = BindingTable::from_module(&module.body);
    let expr = ruff_python_parser::parse_expression(expr_source).map_err(|e| e.to_string())?;
    Ok(table.deferred_local_class(expr.expr()))
}

// ---------------------------------------------------------------------------
// An alias captures an object, so a later rebinding of its target cannot move it
// ---------------------------------------------------------------------------

/// `Alias = Old` captures the class object bound to `Old` AT THAT MOMENT. A
/// later `class Old` rebinds only the name. Resolving `Alias` must reach the
/// FIRST definition; reaching the second attributes to the alias a class it
/// never referred to.
#[test]
fn a_deferred_alias_keeps_the_class_it_captured_across_a_later_rebinding() -> TestResult {
    let source = "
class Trellis: ...

Espalier = Trellis

class Trellis: ...
";
    let module = parsed(source)?;
    let sites = class_sites(&module.body, "Trellis");
    assert_eq!(sites.len(), 2, "fixture must define `Trellis` twice");

    let resolved = deferred_site(source, "Espalier")?;
    assert_eq!(
        resolved,
        Some(sites[0]),
        "`Espalier = Trellis` captured the FIRST `Trellis`; a later rebinding \
         of the NAME cannot retarget the alias"
    );
    Ok(())
}

/// The same program with the alias assigned AFTER the redefinition captures
/// the second class. Together with the test above this pins that the answer
/// tracks the assignment's position, not a fixed choice of first-or-last.
#[test]
fn a_deferred_alias_assigned_after_a_rebinding_captures_the_later_class() -> TestResult {
    let source = "
class Trellis: ...

class Trellis: ...

Espalier = Trellis
";
    let module = parsed(source)?;
    let sites = class_sites(&module.body, "Trellis");
    assert_eq!(sites.len(), 2, "fixture must define `Trellis` twice");

    let resolved = deferred_site(source, "Espalier")?;
    assert_eq!(
        resolved,
        Some(sites[1]),
        "the alias was assigned after the second `class Trellis`, so it \
         captured that one"
    );
    Ok(())
}

/// A chain of aliases: each link captures its target at ITS OWN assignment.
/// `Pergola = Espalier` runs before the redefinition, so both links still
/// reach the first class.
#[test]
fn a_deferred_alias_chain_resolves_each_link_at_its_own_assignment() -> TestResult {
    let source = "
class Trellis: ...

Espalier = Trellis
Pergola = Espalier

class Trellis: ...
";
    let module = parsed(source)?;
    let sites = class_sites(&module.body, "Trellis");
    assert_eq!(sites.len(), 2, "fixture must define `Trellis` twice");

    assert_eq!(
        deferred_site(source, "Pergola")?,
        Some(sites[0]),
        "every link of the chain was assigned before the redefinition"
    );
    Ok(())
}

/// The mid-chain rebinding case: `Pergola = Espalier` captures whatever
/// `Espalier` was, and `Espalier` is later re-pointed at a different class.
/// `Pergola` must not follow it.
#[test]
fn a_deferred_alias_does_not_follow_its_target_alias_being_repointed() -> TestResult {
    let source = "
class Trellis: ...
class Arbour: ...

Espalier = Trellis
Pergola = Espalier
Espalier = Arbour
";
    let module = parsed(source)?;
    let trellis = class_sites(&module.body, "Trellis");
    let arbour = class_sites(&module.body, "Arbour");
    assert_eq!(trellis.len(), 1, "fixture defines `Trellis` once");
    assert_eq!(arbour.len(), 1, "fixture defines `Arbour` once");

    assert_eq!(
        deferred_site(source, "Pergola")?,
        Some(trellis[0]),
        "`Pergola` captured `Espalier`'s value, not the name `Espalier`"
    );
    assert_eq!(
        deferred_site(source, "Espalier")?,
        Some(arbour[0]),
        "`Espalier` itself WAS re-pointed, and its final binding is the one a \
         lazily evaluated annotation sees"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Positional resolution: the already-correct path, kept honest
// ---------------------------------------------------------------------------

/// The positional follower resolves a use at its own offset. A base list
/// written between the two definitions sees the first class, whether it names
/// it directly or through the alias.
#[test]
fn a_positional_use_of_an_alias_resolves_to_the_captured_class() -> TestResult {
    let source = "
class Trellis: ...

Espalier = Trellis

class Vine(Espalier): ...

class Trellis: ...
";
    let module = parsed(source)?;
    let sites = class_sites(&module.body, "Trellis");
    assert_eq!(sites.len(), 2, "fixture must define `Trellis` twice");

    let table = BindingTable::from_module(&module.body);
    let base = module
        .body
        .iter()
        .find_map(|stmt| match stmt {
            Stmt::ClassDef(class) if class.name.as_str() == "Vine" => {
                class.arguments.as_ref()?.args.first()
            }
            _ => None,
        })
        .ok_or("fixture must give `Vine` a base")?;

    assert_eq!(
        table.local_class_definition(base),
        Some(sites[0]),
        "`Vine`'s base resolves through the alias to the first `Trellis`"
    );
    Ok(())
}

/// A name never bound to a class in this module resolves to nothing — an
/// abstention, never a guess at a same-spelled local class.
#[test]
fn a_name_bound_to_a_non_class_resolves_to_no_definition() -> TestResult {
    let source = "
class Trellis: ...

Espalier = 3
";
    assert_eq!(
        deferred_site(source, "Espalier")?,
        None,
        "`Espalier` is an int, not the class that happens to be nearby"
    );
    Ok(())
}
