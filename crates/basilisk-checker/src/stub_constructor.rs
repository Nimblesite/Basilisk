//! Implements [STUBRES-PYI] #289 constructor-chain resolution.
//! Implements the constructor half of [TYPESHEDRT-ACCEPTANCE-HOVER].
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PYI
//!
//! Resolves, from a set of parsed `.pyi` classes, which class in a target
//! class's method-resolution order supplies each step of the pinned constructor
//! conversion: the first inherited non-`object` `__new__` and the first
//! `__init__`. This real-`.pyi` model avoids hand-authored stdlib constructor
//! tables for classes such as `unittest.mock.Mock` (GitHub #289): given
//! `mock.pyi`, `Mock`'s constructor takes `__new__` from
//! `NonCallableMock` and `__init__` from `CallableMixin`, so it accepts
//! arbitrary arguments and must never draw an "unexpected argument" diagnostic.

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

use basilisk_stubs::types::StubClass;

/// Which class in an MRO supplies each constructor step ([STUBRES-PYI] #289).
///
/// `None` means no class in the resolved order declares that step, so the
/// pinned `object.__new__` / `object.__init__` fallback applies.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConstructorChain {
    /// The class supplying the first inherited non-`object` `__new__`.
    pub new_from: Option<String>,
    /// The class supplying the first `__init__`.
    pub init_from: Option<String>,
}

/// Resolve `class_name`'s constructor chain over `classes` ([STUBRES-PYI] #289).
///
/// Walks the C3-linearized MRO and selects the first non-`object` `__new__` and
/// the first `__init__`, exactly as the pinned constructor conversion rules
/// require.
#[must_use]
pub fn resolve_constructor<S: BuildHasher>(
    classes: &HashMap<String, StubClass, S>,
    class_name: &str,
) -> ConstructorChain {
    let mro = simplified_mro(classes, class_name);
    ConstructorChain {
        new_from: mro
            .iter()
            .find(|name| class_defines(classes, name, "__new__"))
            .cloned(),
        init_from: mro
            .iter()
            .find(|name| class_defines(classes, name, "__init__"))
            .cloned(),
    }
}

/// The C3-linearized MRO of `class_name` ([STUBRES-PYI] #289).
///
/// This is the linearization Python itself computes, so a diamond hierarchy
/// (e.g. `Mock` reaching `Base` through both `CallableMixin` and
/// `NonCallableMock`) correctly orders `NonCallableMock` *before* the shared
/// `Base` — which a naive depth-first walk gets wrong. `object` and classes
/// outside `classes` terminate a branch.
#[must_use]
pub fn simplified_mro<S: BuildHasher>(
    classes: &HashMap<String, StubClass, S>,
    class_name: &str,
) -> Vec<String> {
    mro_over(class_name, &|name| resolvable_bases(classes, name))
}

/// C3-linearize `class_name` over an arbitrary hierarchy described by
/// `bases_of` ([STUBRES-PYI] #289).
///
/// `bases_of(name)` returns the resolvable direct base names of `name` (already
/// filtered of `object`/unresolved terminals as the caller sees fit). This
/// lets the same linearization serve parsed [`StubClass`] hierarchies and the
/// `ExternalSymbol` graph an importer sees, without duplicating the algorithm.
#[must_use]
pub fn mro_over(class_name: &str, bases_of: &dyn Fn(&str) -> Vec<String>) -> Vec<String> {
    linearize(class_name, bases_of, &mut HashSet::new())
}

/// C3 linearization with a cycle guard: `active` holds the ancestors currently
/// being linearized, so a cyclic base cannot recurse forever.
fn linearize(
    class_name: &str,
    bases_of: &dyn Fn(&str) -> Vec<String>,
    active: &mut HashSet<String>,
) -> Vec<String> {
    if !active.insert(class_name.to_owned()) {
        return vec![class_name.to_owned()];
    }
    let bases = bases_of(class_name);
    let mut sequences: Vec<Vec<String>> = bases
        .iter()
        .map(|base| linearize(base, bases_of, active))
        .collect();
    sequences.push(bases);
    let _ = active.remove(class_name);
    let mut result = vec![class_name.to_owned()];
    result.extend(c3_merge(sequences));
    result
}

/// Direct base heads of `class_name`, excluding `object`.
///
/// DELETED — panics. The body was `base_head(base)` (itself deleted for
/// splitting SOURCE TEXT at `[`) piped into `.filter(|head| head != "object")`
/// — the top type recognised by its builtin SPELLING. Both halves decided a
/// class's identity from characters. Ask the binding table.
fn resolvable_bases<S: BuildHasher>(
    _classes: &HashMap<String, StubClass, S>,
    _class_name: &str,
) -> Vec<String> {
    panic!(
        "basilisk-checker: `resolvable_bases` was DELETED because it took base heads \
         by splitting SOURCE TEXT at `[` and excluded the top type by comparing the \
         result to the literal `\"object\"`. It panics because the real \
         implementation — resolving each base expression through the binding table — \
         DOES NOT EXIST YET. Do not restore either half and do not return an empty \
         vector in its place."
    )
}

/// Merge linearizations per the C3 rule: repeatedly take the head of the first
/// sequence that appears in no sequence's tail. An inconsistent hierarchy has no
/// valid head; rather than loop forever, take the first remaining head so
/// resolution still terminates.
fn c3_merge(mut sequences: Vec<Vec<String>>) -> Vec<String> {
    let mut result = Vec::new();
    loop {
        sequences.retain(|seq| !seq.is_empty());
        if sequences.is_empty() {
            return result;
        }
        let head = pick_c3_head(&sequences);
        for seq in &mut sequences {
            seq.retain(|name| name != &head);
        }
        result.push(head);
    }
}

/// The next C3 head: a candidate appearing in no sequence's tail (position > 0).
/// Falls back to the first remaining head to guarantee termination.
fn pick_c3_head(sequences: &[Vec<String>]) -> String {
    sequences
        .iter()
        .filter_map(|seq| seq.first())
        .find(|candidate| !in_any_tail(sequences, candidate))
        .or_else(|| sequences.iter().find_map(|seq| seq.first()))
        .cloned()
        .unwrap_or_default()
}

/// Whether `candidate` appears in the tail (any position after the head) of any
/// sequence — the C3 "not yet safe to emit" condition.
fn in_any_tail(sequences: &[Vec<String>], candidate: &str) -> bool {
    sequences
        .iter()
        .any(|seq| seq.iter().skip(1).any(|name| name == candidate))
}

// ##########################################################################
// # DELETED BODY — `base_head`. DO NOT RESTORE IT. DO NOT SUBSTITUTE A     #
// # PLACEHOLDER THAT RETURNS THE INPUT UNCHANGED.                          #
// #                                                                        #
// # It read: `base.split('[').next().unwrap_or(base).trim()` — taking the  #
// # generic head of a base class by SPLITTING ITS SOURCE TEXT at a         #
// # bracket. A subscripted base written across lines, with a space before  #
// # the bracket, or reached through an alias produced a different head,    #
// # and any base whose name merely contained a bracket was truncated.      #
// #                                                                        #
// # A base class is an EXPRESSION. Its head is `Expr::Subscript.value`     #
// # resolved through the binding table to the symbol it denotes — never    #
// # the characters before a `[`.                                           #
// #                                                                        #
// # Pinned by: tests/no_type_spelling_surgery_tests.rs                     #
// ##########################################################################

/// DELETED — panics. The signature survives only so its callers stay visible
/// as the rebuild map; see the banner above.
#[must_use]
pub fn base_head(_base: &str) -> &str {
    panic!(
        "basilisk-checker: `base_head` was DELETED because it took a base class's \
         generic head by splitting its SOURCE TEXT at `[`. It panics because the real \
         implementation — resolving `Expr::Subscript.value` through the binding table \
         — DOES NOT EXIST YET. Do not restore the split and do not return the input \
         unchanged in its place."
    )
}

/// Whether `class_name` directly declares a method named `method`.
fn class_defines<S: BuildHasher>(
    classes: &HashMap<String, StubClass, S>,
    class_name: &str,
    method: &str,
) -> bool {
    classes
        .get(class_name)
        .is_some_and(|class| class.methods.iter().any(|func| func.name == method))
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "a malformed inline test fixture must fail loudly"
)]
mod tests {
    use std::path::Path;

    use basilisk_stubs::parse_pyi_source;
    use basilisk_stubs::types::{StubModule, StubSource, StubTier};

    use super::{resolve_constructor, simplified_mro};

    fn parse(src: &str) -> StubModule {
        parse_pyi_source(
            src,
            Path::new("mock.pyi"),
            "unittest.mock",
            StubSource::Typeshed,
            StubTier::Tier1,
        )
        .expect("fixture parses")
    }

    /// The real `unittest.mock` hierarchy shape (constructor-relevant members).
    /// `NonCallableMock` inherits the dynamic `Any` base; `Mock` itself is empty.
    const MOCK_HIERARCHY: &str = "\
from typing import Any, Self

class Base:
    def __init__(self, *args: Any, **kwargs: Any) -> None: ...

class NonCallableMock(Base, Any):
    def __new__(cls, *args: Any, **kwargs: Any) -> Self: ...
    def __init__(self, spec: Any = None, **kwargs: Any) -> None: ...

class CallableMixin(Base):
    def __init__(self, spec: Any = None, side_effect: Any = None, **kwargs: Any) -> None: ...
    def __call__(self, *args: Any, **kwargs: Any) -> Any: ...

class Mock(CallableMixin, NonCallableMock): ...
";

    /// [STUBRES-PYI] #289: `Mock`'s constructor draws `__new__` from the first
    /// inherited non-`object` `__new__` (`NonCallableMock`) and `__init__` from
    /// the first `__init__` in MRO (`CallableMixin`) — not `Base` or
    /// `NonCallableMock`. Proven against the real class shape, no hand table.
    #[test]
    fn mock_constructor_chain_matches_typeshed_mro() {
        let module = parse(MOCK_HIERARCHY);
        let chain = resolve_constructor(&module.classes, "Mock");
        assert_eq!(chain.new_from.as_deref(), Some("NonCallableMock"));
        assert_eq!(chain.init_from.as_deref(), Some("CallableMixin"));
    }

    /// The C3 linearization matches Python's own: the diamond through `Base`
    /// orders `NonCallableMock` BEFORE the shared `Base` (a naive DFS would put
    /// `Base` first via `CallableMixin`), and the dynamic `Any` base is skipped.
    #[test]
    fn mock_mro_is_c3_linearized() {
        let module = parse(MOCK_HIERARCHY);
        let mro = simplified_mro(&module.classes, "Mock");
        assert_eq!(
            mro,
            vec![
                "Mock".to_owned(),
                "CallableMixin".to_owned(),
                "NonCallableMock".to_owned(),
                "Base".to_owned(),
            ],
            "MRO must be the exact C3 linearization Python computes"
        );
        assert!(
            !mro.iter().any(|n| n == "Any"),
            "Any is not a resolvable base"
        );
    }

    /// Classic C3 diamond `D(B, C)` where `B, C` both extend `A`: the
    /// linearization is `[D, B, C, A]`, and the shared `A` appears exactly once
    /// after both `B` and `C`.
    #[test]
    fn classic_diamond_linearizes_to_d_b_c_a() {
        let module = parse("class A: ...\nclass B(A): ...\nclass C(A): ...\nclass D(B, C): ...\n");
        assert_eq!(
            simplified_mro(&module.classes, "D"),
            vec![
                "D".to_owned(),
                "B".to_owned(),
                "C".to_owned(),
                "A".to_owned(),
            ]
        );
    }

    /// A class with neither `__new__` nor `__init__` anywhere in its MRO falls
    /// back to `object` (both `None`) rather than manufacturing a constructor.
    #[test]
    fn plain_class_has_object_fallback() {
        let module = parse("class Empty: ...\n");
        let chain = resolve_constructor(&module.classes, "Empty");
        assert_eq!(chain, super::ConstructorChain::default());
    }

    /// An unknown class name resolves to no constructor steps.
    #[test]
    fn unknown_class_resolves_to_none() {
        let module = parse("class C: ...\n");
        let chain = resolve_constructor(&module.classes, "DoesNotExist");
        assert_eq!(chain.new_from, None);
        assert_eq!(chain.init_from, None);
    }
}
