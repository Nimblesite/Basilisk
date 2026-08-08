//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! Structural callable subtyping for classes with `__call__` methods.
//!
//! The name-based assignment comparison cannot evaluate structural
//! compatibility between classes defining `__call__`.  This module resolves
//! annotation NODES to signature sets (see [`super::sig_model`]) and applies
//! the typing spec's subtyping rules (see [`super::sig_subtype`]) so the rule
//! can suppress assignments that are structurally valid.
//!
//! Resolution is by AST identity ([ASTREBUILD-LAW]): a `Name` or
//! `Name[...]` annotation node is matched against the module's class
//! definitions, and every parameter type inside a signature is a lowered
//! [`basilisk_resolver::TypeNode`] — never rendered source text.

use std::collections::HashMap;

use ruff_python_ast::{Expr, Stmt};

use basilisk_resolver::ResolvedModule;

use crate::rules::shared::parse_module;

use super::sig_model::{class_entry, ClassEntry, Sig, TypeSigs};
use super::sig_subtype::sig_subtype;

// ---------------------------------------------------------------------------
// Module index
// ---------------------------------------------------------------------------

/// Per-module index of classes.
pub(super) struct CallIndex {
    /// Class name → structural entry (methods, attributes, bases).
    pub(super) classes: HashMap<String, ClassEntry>,
    /// Module-seeded nominal context — the ONE subtyping implementation
    /// every signature verdict routes through ([NARROWPLAN-SUBTYPING]).
    pub(super) subtyping: crate::subtyping::SubtypingContext,
}

/// Build the [`CallIndex`] for a module.
pub(super) fn build_index(module: &ResolvedModule) -> CallIndex {
    let mut index = CallIndex {
        classes: HashMap::new(),
        subtyping: crate::subtyping::module_context(module),
    };
    let Some(parsed) = parse_module(module) else {
        return index;
    };
    for stmt in &parsed.ast.body {
        if let Stmt::ClassDef(cls) = stmt {
            let _ = index
                .classes
                .insert(cls.name.to_string(), class_entry(cls, &module.bindings));
        }
    }
    index
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// `true` when an assignment of a value of the callable type denoted by the
/// `rhs` annotation node to a variable annotated with the `declared` node is
/// structurally acceptable.  `false` means "provably incompatible, or not a
/// callable form this module resolves" — the caller keeps the diagnostic its
/// own evidence produced.  Both sides are judged as AST nodes; no verdict
/// depends on how either annotation is spelled ([ASTREBUILD-LAW]).
pub(super) fn assignment_compatible(declared: &Expr, rhs: &Expr, index: &CallIndex) -> bool {
    if index.classes.is_empty() {
        return false;
    }
    let Some(target) = resolve(declared, index) else {
        return false;
    };
    let Some(source) = resolve(rhs, index) else {
        return false;
    };
    sigs_compatible(&index.subtyping, &source, &target)
}

/// Overload-set compatibility: incompatible only when some target signature
/// is PROVABLY unsatisfied by every source signature ([ASTREBUILD-LAW]: a
/// kept diagnostic needs `Some(false)`; abstention counts as compatible).
/// `Unknown` on either side is compatible.
///
/// The subtyping context is unused for now: signature relations that need
/// nominal user-class verdicts abstain until the relation layer models them
/// ([ASTREBUILD-PHASE-RESOLVER]); the parameter is kept for the
/// protocol-member call sites.
pub(super) fn sigs_compatible(
    _subtyping: &crate::subtyping::SubtypingContext,
    source: &TypeSigs,
    target: &TypeSigs,
) -> bool {
    match (source, target) {
        (TypeSigs::Unknown, _) | (_, TypeSigs::Unknown) => true,
        (TypeSigs::Sigs(src), TypeSigs::Sigs(tgt)) => tgt
            .iter()
            .all(|b| src.is_empty() || src.iter().any(|a| sig_subtype(a, b) != Some(false))),
    }
}

// ---------------------------------------------------------------------------
// Type-expression resolution
// ---------------------------------------------------------------------------

/// Resolve a type-expression NODE into the callable signatures of a
/// same-module class defining `__call__`.  `None` means "not such a form" —
/// the caller keeps its own judgment.  The class is found by its AST
/// identifier, never by slicing or re-parsing source text
/// ([ASTREBUILD-LAW]).
fn resolve(expr: &Expr, index: &CallIndex) -> Option<TypeSigs> {
    let (base, subscripted) = match expr {
        Expr::Name(name) => (name.id.as_str(), false),
        Expr::Subscript(sub) => match sub.value.as_ref() {
            Expr::Name(name) => (name.id.as_str(), true),
            _ => return None,
        },
        _ => return None,
    };
    let entry = index.classes.get(base)?;
    let call_sigs = entry.methods.get("__call__")?;
    if subscripted && !entry.generic_params.is_empty() {
        // [ASTREBUILD-PHASE-RESOLVER]: specializing a generic class's
        // signatures requires TypeVar substitution over resolved nodes,
        // which this layer does not model.  Abstain rather than guess.
        return Some(TypeSigs::Unknown);
    }
    Some(specialize_class_sigs(
        call_sigs,
        &entry.generic_params,
        None,
        index,
    ))
}

// ---------------------------------------------------------------------------
// Specialization
// ---------------------------------------------------------------------------

/// Specialize a class's method signatures with subscript arguments.
///
/// Substituting type arguments into lowered signatures requires a resolved
/// `TypeVar` model this layer does not have, and rendered-text arguments are
/// never parsed ([ASTREBUILD-LAW]); whenever substitution would be required,
/// the whole set abstains as [`TypeSigs::Unknown`] instead of guessing
/// ([ASTREBUILD-PHASE-RESOLVER]).
pub(super) fn specialize_class_sigs(
    sigs: &[Sig],
    generic_params: &[String],
    args: Option<&[String]>,
    _index: &CallIndex,
) -> TypeSigs {
    if args.is_some_and(|args| !args.is_empty()) && !generic_params.is_empty() {
        return TypeSigs::Unknown;
    }
    TypeSigs::Sigs(sigs.to_vec())
}
