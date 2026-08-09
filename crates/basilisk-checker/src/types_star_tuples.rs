// ############################################################################
// # DELETED IMPLEMENTATION — PANIC-ONLY SHELL. DO NOT PUT LOGIC BACK HERE.   #
// #                                                                          #
// # These helpers recognised PEP 646 unpacked tuple forms and the PEP 484    #
// # homogeneous `tuple[X, ...]` form by READING THE RENDERED SPELLING of a   #
// # type: `Named(name)` where `name.starts_with('*')`, `name == "..."` for   #
// # the ellipsis element, and `starts_with("*tuple[")` / `split('[')` to     #
// # take apart an unpacked segment.                                          #
// #                                                                          #
// # `...` and `*Ts` are AST nodes — `Expr::EllipsisLiteral` and              #
// # `Expr::Starred` — and `tuple` is a resolvable symbol. Deciding any of    #
// # them from a rendered string means the answer moves when the source is    #
// # respelled: `*tuple[int, ...]` written across two lines, `Unpack[Ts]`     #
// # spelled instead of `*Ts`, or `tuple` imported under an alias.            #
// #                                                                          #
// # THE SIGNATURES SURVIVE ONLY AS A MAP. Each body panics because the real  #
// # implementation DOES NOT EXIST YET.                                       #
// #                                                                          #
// #   * DO NOT return `None`/`false` "for now" — a tuple check that answers  #
// #     "not a star tuple" to everything reports coverage it does not have.  #
// #   * DO NOT vendor these tests into `types.rs` or a rule module.          #
// #                                                                          #
// # The replacement carries tuple shape structurally — an element list with  #
// # an explicit unpacked/homogeneous marker set when the AST is lowered, so  #
// # no consumer ever has to re-read a rendering.                             #
// #                                                                          #
// # Pinned by: crates/basilisk-checker/tests/no_type_spelling_surgery_tests.rs
// ############################################################################

//! The DELETED star-tuple text helpers, reduced to panicking signatures.

use crate::types::InferredType;

/// Panic message shared by every deleted body in this module.
macro_rules! deleted {
    ($what:literal) => {
        panic!(concat!(
            "basilisk-checker: `",
            $what,
            "` was DELETED because it recognised tuple shape from the RENDERED \
             SPELLING of a type (`starts_with('*')`, `== \"...\"`, \
             `split('[')`) instead of from its structure. It panics because the \
             real implementation — tuple shape carried structurally from the \
             AST — DOES NOT EXIST YET. Do not restore it and do not answer \
             `None`/`false` in its place."
        ))
    };
}

/// DELETED — panics. Returned the element of a homogeneous `tuple[X, ...]` by
/// testing whether the last element RENDERED as `"..."`.
pub(crate) fn homogeneous_tuple_elem(_elems: &[InferredType]) -> Option<&InferredType> {
    deleted!("homogeneous_tuple_elem")
}

/// DELETED — panics. Recognised a PEP 646 unpacked element by a leading `*` in
/// its rendered name.
pub(crate) fn is_unpacked_tuple_elem(_elem: &InferredType) -> bool {
    deleted!("is_unpacked_tuple_elem")
}

/// DELETED — panics. Matched prefix/middle/suffix around an unpacked segment
/// located by string surgery on the rendering.
pub(crate) fn tuple_assignable_with_star(
    _source: &[InferredType],
    _target: &[InferredType],
) -> bool {
    deleted!("tuple_assignable_with_star")
}
