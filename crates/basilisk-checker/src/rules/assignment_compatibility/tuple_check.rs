//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! Tuple assignment checking for `assignment_compatibility`.
//!
//! Validates that re-assignments to tuple-annotated module variables are
//! compatible with the declared tuple type, checking element count and
//! element type for fixed-length tuples and homogeneous variable-length tuples.

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::span_util::slice_span;

use super::CODE;

/// Check re-assignments to tuple-annotated variables against the tuple literal RHS.
///
/// For example, `t1: tuple[int]` declared, then `t1 = (1, 2)` assigned — error because
/// `(1, 2)` has 2 elements but `tuple[int]` requires exactly 1.
pub(super) fn check_tuple_reassignments(
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;

    // Build map: var name → annotation text, for vars annotated with tuple types.
    let mut tuple_annotations: std::collections::HashMap<&str, &str> =
        std::collections::HashMap::new();
    for var in &module.module_vars {
        if !var.has_annotation {
            continue;
        }
        let Some(ann_span) = var.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let ann_trimmed = ann_text.trim();
        if is_tuple_annotation(ann_trimmed) {
            let _ = tuple_annotations.insert(var.name.as_str(), ann_trimmed);
        }
    }

    if tuple_annotations.is_empty() {
        return;
    }

    // Check unannotated re-assignments to tuple-annotated variables.
    for var in &module.module_vars {
        if var.has_annotation {
            continue;
        }
        let Some(&ann_text) = tuple_annotations.get(var.name.as_str()) else {
            continue;
        };
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };
        let Some(rhs_text) = slice_span(source, rhs_span) else {
            continue;
        };
        let rhs_trimmed = rhs_text.trim();

        if !is_tuple_literal(rhs_trimmed) {
            continue;
        }

        if let Some(msg) = check_tuple_literal_mismatch(rhs_trimmed, ann_text) {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Type mismatch: `{}` is annotated `{ann_text}` but assigned {msg}",
                    var.name
                ),
                var.name_span,
                path,
                Some("Ensure the tuple literal matches the annotated tuple type".to_owned()),
                Some(
                    "Basilisk checks that tuple literals are compatible with the declared tuple type"
                        .to_owned(),
                ),
            ));
        }
    }
}

// ##########################################################################
// # DELETED BODY — `is_tuple_annotation`. DO NOT RESTORE IT AND DO NOT     #
// # RETURN `false` IN ITS PLACE.                                           #
// #                                                                        #
// # It recognised a tuple annotation by the six characters `tuple[`:       #
// #                                                                        #
// #   if !ann.starts_with("tuple[") || !ann.ends_with(']') { return false } #
// #                                                                        #
// # Python's grammar puts no constraint on whitespace before a subscript,  #
// # so `tuple [int]` is the SAME annotation and failed the test; so did    #
// # `builtins.tuple[int]`, `typing.Tuple[int]`, and the annotation reached #
// # through `from builtins import tuple as T`. A module that defines its   #
// # own `class tuple` passed it. Tuple-ness is a question about the        #
// # resolved head of an `Expr::Subscript`, not about a prefix.             #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
pub(super) fn is_tuple_annotation(_ann: &str) -> bool {
    panic!(
        "basilisk-checker: `is_tuple_annotation` was DELETED because it recognised a \
         tuple annotation by the PREFIX of its rendered text, so `tuple [int]` and \
         `builtins.tuple[int]` were not tuples and a user-defined `class tuple` was. \
         It panics because the real implementation — resolving the head of the \
         `Expr::Subscript` through the binding table — DOES NOT EXIST YET. Do not \
         restore the prefix test and do not return `false` in its place: `false` \
         silently disables the whole rule while it still reports as implemented."
    )
}

// ##########################################################################
// # DELETED BODY — `is_tuple_literal`. DO NOT RESTORE IT.                  #
// #                                                                        #
// #   text.starts_with('(') && text.ends_with(')')                         #
// #                                                                        #
// # Parentheses neither make nor unmake a tuple. `(1)` is the integer 1,   #
// # `(a for b in c)` is a generator, `(x)` is whatever `x` is — all three  #
// # passed. `1, 2` is a tuple and failed. The parser already decided this: #
// # the node is `Expr::Tuple` or it is not.                                #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn is_tuple_literal(_text: &str) -> bool {
    panic!(
        "basilisk-checker: `is_tuple_literal` was DELETED because it decided tuple-ness \
         from the first and last CHARACTER of the source text, accepting `(1)` and \
         rejecting `1, 2`. It panics because the real implementation — matching \
         `Expr::Tuple` on the assigned expression — DOES NOT EXIST YET. Do not restore \
         the punctuation test and do not return either answer unconditionally."
    )
}

// ##########################################################################
// # DELETED BODY — `check_tuple_literal_mismatch`, AND THE TWO SPLITTERS   #
// # IT RAN ON (`split_tuple_literal_elems`, `split_type_list`) ARE GONE    #
// # ENTIRELY. DO NOT RESTORE ANY OF THEM.                                  #
// #                                                                        #
// # This was a SECOND PARSER for Python, written in string operations and  #
// # run on the output of the first one:                                    #
// #                                                                        #
// #   let inner_ann = ann.strip_prefix("tuple[")?.strip_suffix(']')?;      #
// #   let rhs_inner = rhs.strip_prefix('(')?.strip_suffix(')')?;           #
// #   let rhs_elems = split_tuple_literal_elems(rhs_inner);  // scan chars #
// #   if let Some(t) = inner_ann.strip_suffix(", ...") { … }               #
// #   if inner_ann.trim() == "()" { … }                                    #
// #   if rhs_elems.len() != ann_elems.len() { …emit a diagnostic… }        #
// #                                                                        #
// # The arity diagnostic came straight off a COMMA COUNT taken by walking  #
// # characters and tracking bracket depth by hand — a live verdict derived #
// # from punctuation. It agreed with Python only by coincidence:           #
// #                                                                        #
// #   * `tuple[T, ...]` was matched with `strip_suffix(", ...")`, so       #
// #     `tuple[T,...]` (no space) was read as a TWO-element fixed tuple    #
// #     whose second element type is the string `...`;                     #
// #   * a comma inside a string literal — `("a,b",)` — split the element   #
// #     list in two, reporting a 1-tuple as a 2-tuple, because the depth   #
// #     counter tracks brackets and knows nothing about quoting;           #
// #   * a trailing comma, a line continuation, or a comment anywhere in    #
// #     the assignment moved the count.                                    #
// #                                                                        #
// # The element counts are `Expr::Tuple::elts.len()` and the annotation's  #
// # arguments are the `Expr::Subscript::slice` — both already built.       #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn check_tuple_literal_mismatch(_rhs: &str, _ann: &str) -> Option<String> {
    panic!(
        "basilisk-checker: `check_tuple_literal_mismatch` was DELETED because it \
         RE-PARSED both the annotation and the assigned value out of source text — \
         stripping brackets by prefix, splitting elements on commas with a \
         hand-tracked depth counter that does not know about string literals — and \
         emitted an arity diagnostic from the resulting count. It panics because the \
         real implementation — comparing `Expr::Tuple::elts` against the subscript's \
         own argument nodes — DOES NOT EXIST YET. Do not restore the splitters and do \
         not return `None` in its place: `None` reports every tuple as compatible."
    )
}

// ##########################################################################
// # DELETED BODY — `literal_elem_matches`. DO NOT RESTORE IT. DO NOT       #
// # SUBSTITUTE A PLACEHOLDER THAT RETURNS `true`.                          #
// #                                                                        #
// # This was the worst offender in the crate: a HAND-WRITTEN LEXER that    #
// # classified a tuple element by re-reading its SOURCE CHARACTERS —       #
// #                                                                        #
// #   is_int_lit   = elem.chars().all(|c| c.is_ascii_digit() || …)         #
// #   is_str_lit   = elem.starts_with('"') && elem.ends_with('"')          #
// #   is_float_lit = elem.contains('.')                                    #
// #   is_bytes_lit = elem.starts_with("b\"")                               #
// #   is_bool_lit  = elem == "True" || elem == "False"                     #
// #                                                                        #
// # — and then matched it against the LOWER-CASED, bracket-split spelling  #
// # of the expected type, with `expected_base == "object"` accepting       #
// # anything. Every one of these is already an AST node the parser         #
// # produced: `Expr::NumberLiteral`, `Expr::StringLiteral`,                #
// # `Expr::BytesLiteral`, `Expr::BooleanLiteral`, `Expr::NoneLiteral`.     #
// # Re-deriving them from characters means `0x1F` in one file and `0X1F`   #
// # in another disagree, `1_000` is read as an int but `1e3` is not, and   #
// # `"""x"""` is not a string at all.                                      #
// #                                                                        #
// # Pinned by: tests/no_type_spelling_surgery_tests.rs                     #
// ##########################################################################

/// DELETED — panics. The signature survives only so its callers stay visible
/// as the rebuild map; see the banner above.
#[expect(
    dead_code,
    reason = "its only caller, `check_tuple_literal_mismatch`, was itself deleted for \
              re-parsing source text; the shell and its banner are retained as the \
              record of what the element comparison has to become"
)]
pub(super) fn literal_elem_matches(_elem: &str, _expected: &str) -> bool {
    panic!(
        "basilisk-checker: `literal_elem_matches` was DELETED because it RE-LEXED a \
         tuple element from its source characters (digit scans, quote-prefix tests, \
         `contains('.')`) and compared it against a lower-cased, bracket-split \
         spelling of the expected type. It panics because the real implementation — \
         reading the element's literal `Expr` node and asking the ordinary \
         assignability question — DOES NOT EXIST YET. Do not restore the lexer and do \
         not return `true` in its place: `true` accepts every mismatch while the rule \
         still reports itself as implemented."
    )
}
