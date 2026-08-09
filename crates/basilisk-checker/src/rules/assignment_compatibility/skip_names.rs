//! Implements the false-positive skip environment of [TYPEINF-VARS-ANNOTATED].
//! See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-VARS-ANNOTATED
//!
//! Everything `assignment_compatibility` must NOT flag: name sets whose
//! declared types it cannot evaluate (`TypedDict`s, aliases) and the
//! alias/schema environments used by rescue checks.

use basilisk_resolver::ResolvedModule;

use crate::types::InferredType;

use super::{alias_match, enum_expand};

/// Names that E0014 must skip to avoid false positives.
///
/// ORPHANED BY DELETIONS, NOT UNUSED. Every field is read by an
/// `assignment_compatibility` path that was deleted for joining semantic
/// objects by spelling, and every field is itself keyed by a spelling. The
/// struct stays because it is the shape the rebuild has to replace: the same
/// five questions, keyed on definition sites.
#[expect(
    dead_code,
    reason = "every reader was deleted for joining by spelling; the struct is the map of \
              what the identity-keyed rebuild must replace"
)]
pub(super) struct SkipNames {
    /// `TypedDict` class names (lowercase).
    pub(super) typeddict: std::collections::HashSet<String>,
    /// `TypedDict` classes declaring `extra_items=` (PEP 728, lowercase).
    pub(super) typeddict_extra_items: std::collections::HashSet<String>,
    /// Legacy value aliases — `Name = Union[...]` or a concrete container such
    /// as `Name = dict[K, V]` (lowercase → definition), used for alias-expanded
    /// value matching.
    pub(super) value_aliases: std::collections::HashMap<String, InferredType>,
    /// Generic (`TypeVar`-parameterised) value aliases such as
    /// `G = list["G[T]" | T]`, keyed by lowercase name. Used to validate
    /// literal assignments against a specialised recursive alias (`G[str]`).
    pub(super) generic_aliases: std::collections::HashMap<String, alias_match::GenericAlias>,
    /// Enum class name → member names (lowercase), for the enum literal
    /// expansion equivalence ([TYPEINF-SUBTYPING-UNION]).
    pub(super) enum_members: enum_expand::EnumMembers,
}

impl SkipNames {
    /// Build the full skip environment for a module.
    pub(super) fn collect(module: &ResolvedModule) -> Self {
        Self {
            typeddict: collect_typeddict_names(module),
            typeddict_extra_items: collect_extra_items_typeddict_names(module),
            value_aliases: alias_match::collect_value_aliases(module),
            generic_aliases: alias_match::collect_generic_aliases(module),
            enum_members: enum_expand::collect_enum_member_sets(module),
        }
    }
}

// ##########################################################################
// # DELETED BODIES — `collect_typeddict_names` and                          #
// # `collect_extra_items_typeddict_names`. DO NOT RESTORE THEM AND DO NOT   #
// # RETURN AN EMPTY SET.                                                    #
// #                                                                         #
// #   typed_dict_class_names(&module.classes)                               #
// #       .map(str::to_ascii_lowercase)                                     #
// #   names.insert(td_call.lhs_name.to_ascii_lowercase())                   #
// #   classes.filter(|c| graph.has_extra_items(c))                          #
// #       .map(|c| c.name.to_ascii_lowercase())                             #
// #                                                                         #
// # BOTH RESOLVED THE QUESTION AND THEN THREW THE ANSWER AWAY. The          #
// # `TypedDict` chain and the PEP 728 `extra_items=` chain were both walked #
// # on the definition-site class graph — correctly — and the resolved class #
// # was then reduced to its CASE-FOLDED SPELLING so an                      #
// # `InferredType::Named(String)` consumer could join on it. That join      #
// # decides whether E0014 fires:                                            #
// #                                                                         #
// #   * two classes spelled alike in one module collapse to one entry;      #
// #   * `class movie:` — an ordinary class — inherits the skip belonging to #
// #     a `TypedDict` named `Movie`, because the key is lowercased;         #
// #   * a `TypedDict` reached under an alias renders differently and is     #
// #     missed, so a valid `album: Alias = {...}` is reported.              #
// #                                                                         #
// # "It only suppresses, so both errors fail toward silence" is not a       #
// # defence: a suppression IS a verdict, and the third case above is a      #
// # FALSE POSITIVE, not silence.                                            #
// #                                                                         #
// # The rebuild is not in this file. It needs the nominal leaf to carry the #
// # definition site the annotation cascade resolved it to ([TYPEINF-LEGACY])#
// # so `SkipNames` can hold `HashSet<Span>` and the consumer can join on    #
// # identity. `SkipNames::collect` is kept as the map of what reads this.   #
// #                                                                         #
// # Pinned by: tests/nominal_leaf_identity_tests.rs                         #
// ##########################################################################

/// DELETED — panics; see the banner above.
fn collect_typeddict_names(_module: &ResolvedModule) -> std::collections::HashSet<String> {
    panic!(
        "basilisk-checker: `assignment_compatibility`'s `TypedDict` skip set was DELETED \
         because it resolved the `TypedDict` chain on the class graph and then keyed the \
         answer by a LOWERCASED CLASS SPELLING for a consumer holding a rendering, so two \
         classes spelled alike collapsed, a class named `movie` inherited the skip of a \
         `TypedDict` named `Movie`, and a `TypedDict` reached under an alias was reported \
         as an error. It panics because the real implementation DOES NOT EXIST YET: the \
         nominal leaf must carry its definition site so this set can be keyed on identity. \
         Do not restore the lowercasing and do not return an empty set in its place."
    )
}

/// DELETED — panics; see the banner above.
fn collect_extra_items_typeddict_names(
    _module: &ResolvedModule,
) -> std::collections::HashSet<String> {
    panic!(
        "basilisk-checker: `assignment_compatibility`'s PEP 728 `extra_items=` skip set was \
         DELETED because it walked the resolved hierarchy and then keyed the answer by a \
         LOWERCASED CLASS SPELLING, so the skip landed on whatever class happened to be \
         spelled that way rather than on the class the chain actually reached. It panics \
         because the real implementation DOES NOT EXIST YET: the nominal leaf must carry \
         its definition site so this set can be keyed on identity. Do not restore the \
         lowercasing and do not return an empty set in its place."
    )
}
