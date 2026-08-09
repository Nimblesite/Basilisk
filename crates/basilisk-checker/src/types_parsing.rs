// ############################################################################
// # DELETED IMPLEMENTATION — PANIC-ONLY SHELL. DO NOT PUT LOGIC BACK HERE.   #
// #                                                                          #
// # `InferredType::from_annotation` PARSED PYTHON TYPE EXPRESSIONS OUT OF    #
// # RAW SOURCE TEXT. It lowercased its input, string-matched the result      #
// # against `"int"`/`"str"`/`"bool"`/`"none"`, split on `|` to build unions, #
// # split on `[`/`]` and `,` to build containers, and fell back to           #
// # `Named(<the text>)` for everything else.                                 #
// #                                                                          #
// # That is a second Python parser made of string operations, sitting next   #
// # to the real one. It is the single largest source of spelling dependence  #
// # left in this crate:                                                      #
// #                                                                          #
// #   * It lowercases, so a user class `Int` became the builtin `int`.       #
// #   * `from typing import Optional as Maybe` was unrecognisable to it.     #
// #   * Whitespace and line breaks inside an annotation changed the result.  #
// #   * A shadowed or rebound builtin was invisible to it.                   #
// #                                                                          #
// # THE SIGNATURE SURVIVES ONLY AS A MAP of the callers that must be         #
// # rebuilt. It panics because the real implementation DOES NOT EXIST YET.   #
// #                                                                          #
// #   * DO NOT return `InferredType::Unknown` "for now" — that is a verdict  #
// #     nobody computed, and it makes every dependent rule silently abstain  #
// #     while still reporting itself as implemented.                         #
// #   * DO NOT reintroduce a text parser under another name, in a rule       #
// #     module, or as a "temporary" helper.                                  #
// #                                                                          #
// # The replacement already exists for annotations reachable as AST:         #
// # `crate::annotation::AnnotationResolver`, which resolves a type           #
// # EXPRESSION through the binding cascade. Callers holding only text must   #
// # be rebuilt to carry the `Expr` instead. Text is not a type.              #
// #                                                                          #
// # Pinned by:                                                               #
// #   crates/basilisk-checker/tests/legacy_annotation_text_parser_pin_tests.rs
// #   crates/basilisk-checker/tests/annotation_class_name_tests.rs           #
// ############################################################################

//! The DELETED annotation text parser, reduced to a loudly panicking signature.

use crate::types::InferredType;

impl InferredType {
    /// DELETED — panics; see the banner at the head of this file.
    ///
    /// Callers reaching this are holding an annotation as a `String` and asking
    /// what type it is. There is no honest answer from text: rebuild the caller
    /// to hold the annotation's `Expr` and resolve it through
    /// [`crate::annotation::AnnotationResolver`].
    #[must_use]
    pub fn from_annotation(_annotation: &str) -> InferredType {
        panic!(
            "basilisk-checker: `InferredType::from_annotation` was DELETED because it \
             parsed Python type expressions out of RAW SOURCE TEXT — lowercasing the \
             input, string-matching builtin names, and splitting on `|`/`[`/`,`. It \
             panics because the real implementation DOES NOT EXIST YET for callers \
             that hold only text. Do not restore the parser and do not return \
             `Unknown` in its place: rebuild this caller to carry the annotation's \
             `Expr` and resolve it through `crate::annotation::AnnotationResolver`."
        )
    }
}
