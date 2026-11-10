//! Implements [`assignment_compatibility`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `TypeForm` validation for `assignment_compatibility`.
//!
//! When the declared type is `TypeForm[T]`, the RHS must be a valid type
//! expression whose represented type is assignable to `T`.  This module
//! validates type form assignments by parsing the RHS source text as a
//! type expression rather than as a runtime value.
//!
//! Reference: <https://typing.readthedocs.io/en/latest/spec/type-forms.html>

use crate::annotation::AnnotationResolver;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic};
use crate::span_util::slice_span;
use crate::types::InferredType;

use basilisk_resolver::{FunctionInfo, ResolvedModule, VariableInfo};

use super::CODE;

// DELETED — `BUILTIN_TYPE_CONSTRUCTORS`, a table of builtin SPELLINGS matched
// against a lowercased callee. Its only reader panics; see the banner below.
// DO NOT RECREATE IT.

// ##########################################################################
// # DELETED BODY — `is_valid_typeform_assignment`.                         #
// #                                                                         #
// # Although `RhsKind` correctly identified literal/call node kinds, this  #
// # function then sliced the RHS back out of SOURCE TEXT and delegated the #
// # actual TypeForm verdict to helpers that stripped quotes, lowercased    #
// # callees, reparsed renderings, and scanned characters for operators.    #
// # That made formatting and spelling decide whether the RHS was a type.   #
// #                                                                         #
// # The rebuild must pass the original RHS `Expr` and resolve it through   #
// # the binding table/type-expression lowering.                            #
// ##########################################################################

/// DELETED — panics; callers must provide the RHS expression node.
pub(super) fn is_valid_typeform_assignment(
    _var: &VariableInfo,
    _source: &str,
    _inner: &InferredType,
    _functions: &[FunctionInfo],
    _resolver: &AnnotationResolver<'_>,
) -> bool {
    panic!(
        "basilisk-checker: `is_valid_typeform_assignment` was DELETED because it \
         sliced the RHS from SOURCE TEXT and delegated semantic validity to quote, \
         callee-name, character-scan, and text-reparse helpers. It panics because the \
         real implementation — validating the original RHS `Expr` as a resolved type \
         expression — DOES NOT EXIST YET. Do not restore the source slice and do not \
         choose a default TypeForm verdict."
    )
}

// ##########################################################################
// # DELETED BODY — `is_valid_call_typeform`. DO NOT RESTORE IT AND DO NOT RETURN A DEFAULT.
// #
// # It split the RHS SOURCE TEXT at `(` to get a callee, LOWERCASED it, and tested membership in `BUILTIN_TYPE_CONSTRUCTORS` — so `List(...)` and `list(...)` were the same callee and a user class named `int` was a builtin constructor.
// #
// # CLAUDE.md: "a whitelist of `int`/`str`/`isinstance` names. Builtins are
// # not an exception — Python lets any name be shadowed, rebound, or
// # aliased, so builtin uses resolve through the binding table like
// # everything else." The replacement is `BindingTable::form_of_with_builtins`,
// # which already models builtin scope AND rebinding.
// #
// # Pinned by: tests/string_keyed_class_hierarchy_pin_tests.rs
// ##########################################################################
fn is_valid_call_typeform(
    _rhs_text: &str,
    _inner: &InferredType,
    _functions: &[FunctionInfo],
    _source: &str,
    _resolver: &AnnotationResolver<'_>,
) -> bool {
    panic!(
        "basilisk-checker: `is_valid_call_typeform` was DELETED because it recognised builtin types by \
         matching their SPELLINGS against a hard-coded table, so a shadowed or \
         aliased builtin was misjudged and any user symbol spelled like a builtin \
         inherited the table's verdict. It panics because the real implementation \
         — resolution through the binding table — DOES NOT EXIST YET. Do not \
         restore the table and do not substitute a default answer."
    )
}

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
/// Whether `func`'s declared return type makes it a valid `TypeForm[inner]`
/// producer. `None` when the annotation is missing or the cascade cannot
/// resolve it — the caller then accepts conservatively.
fn callee_return_typeform(
    _func: &FunctionInfo,
    _inner: &InferredType,
    _source: &str,
    _resolver: &AnnotationResolver<'_>,
) -> Option<bool> {
    panic!(
        "basilisk-checker: `callee_return_typeform` was DELETED because it fell back \
         from an annotation span to RE-PARSING its source rendering, then extracted \
         `type[S]` from characters and reparsed `S` again. It panics because the real \
         implementation — resolving the return annotation's original subscript `Expr` \
         — DOES NOT EXIST YET. Do not restore either text round-trip and do not return \
         `None` in its place."
    )
}

// ##########################################################################
// # DELETED BODY — `is_valid_string_typeform`.                             #
// #                                                                         #
// # It removed the first and last quote characters by hand, guessed        #
// # parseability with a character scanner, then handed the resulting text  #
// # back to `AnnotationResolver::resolve_text`. Prefixes (`r`, `u`), triple #
// # quotes, escapes, and concatenated string literals all changed the       #
// # answer despite already having structured string-literal AST nodes.      #
// ##########################################################################

/// DELETED — panics; quoted type expressions must come from the AST node.
fn is_valid_string_typeform(
    _rhs_text: &str,
    _inner: &InferredType,
    _resolver: &AnnotationResolver<'_>,
) -> bool {
    panic!(
        "basilisk-checker: `is_valid_string_typeform` was DELETED because it stripped \
         quote CHARACTERS from RHS source and reparsed the remaining rendering as a \
         type. It panics because the real implementation — decoding the original \
         string-literal AST node and resolving its quoted type expression in scope — \
         DOES NOT EXIST YET. Do not restore quote slicing and do not return `false`."
    )
}

#[expect(
    dead_code,
    reason = "caller deleted for spelling dependence; retained for the rebuild — see \
              tests/string_keyed_class_hierarchy_pin_tests.rs"
)]
/// The argument text of a `type[...]` annotation, if that is its form.
///
/// The cascade collapses `type[X]` to the nominal `type` leaf (a class
/// object is not its instance), so a caller that needs `X` — here, because
/// PEP 747 makes `type[T]` a subtype of `TypeForm[T]` — reads the subscript
/// and evaluates THAT through the cascade.
/// DELETED — panics. The body recognised `type[X]` with
/// `trimmed.strip_prefix("type[")?.strip_suffix(']')?` — class-object-ness and
/// the subscript both taken from the annotation's CHARACTERS. `builtins.type`
/// under an alias was invisible, a user class named `type` was mistaken for
/// it, and `type [X]` with a space did not match at all. `type[X]` is an
/// `Expr::Subscript` whose `value` resolves to `TypingForm::TypeClass` and
/// whose `slice` IS the inner type expression.
fn type_subscript_inner(_annotation: &str) -> Option<&str> {
    panic!(
        "basilisk-checker: `type_subscript_inner` was DELETED because it recognised \
         `type[X]` by `strip_prefix(\"type[\")` on annotation TEXT. It panics because \
         the real implementation — resolving an `Expr::Subscript` whose value denotes \
         `TypingForm::TypeClass` and reading its slice — DOES NOT EXIST YET. Do not \
         restore the prefix test and do not return `None` in its place."
    )
}

/// Check whether a text parses as a valid Python type expression.
///
/// A valid type expression contains only type names, `|`, `[]`, `.`,
/// and recognised typing constructs.  Expressions like `type(1)` or
/// `int + str` are NOT valid.
// ##########################################################################
// # DELETED BODY — `is_parseable_type_expression`. DO NOT RESTORE IT. DO   #
// # NOT SUBSTITUTE A PLACEHOLDER THAT RETURNS `true` OR `false`.           #
// #                                                                        #
// # It decided whether text is a valid PEP 747 type expression by          #
// # SCANNING FOR CHARACTERS:                                               #
// #                                                                        #
// #   text.contains(['+','-','*','/','%','(',')','!','~','^','&'])         #
// #   for part in text.split('|') { … part.split('[').next() … }           #
// #   if base.contains(' ') { return false }                               #
// #                                                                        #
// # So `Callable[[int], str]` was rejected for its parentheses-free but    #
// # space-bearing spelling, `dict[str, int]` for the space after the       #
// # comma, and `-1` inside a `Literal` for the minus sign — while          #
// # `not a type` written without spaces would have passed. Validity moved  #
// # with the FORMATTER.                                                    #
// #                                                                        #
// # `ruff_python_parser` already answers this: a type expression either    #
// # parses and resolves through the cascade, or it does not.               #
// #                                                                        #
// # Pinned by: tests/no_type_spelling_surgery_tests.rs                     #
// ##########################################################################

/// DELETED — panics. The signature survives only so its callers stay visible
/// as the rebuild map; see the banner above.
fn is_parseable_type_expression(_text: &str) -> bool {
    panic!(
        "basilisk-checker: `is_parseable_type_expression` was DELETED because it \
         judged type-expression validity by scanning source CHARACTERS for operators, \
         splitting on `|` and `[`, and rejecting anything containing a space. It \
         panics because the real implementation — parsing the expression and resolving \
         it through the annotation cascade — DOES NOT EXIST YET. Do not restore the \
         character scan and do not answer `true`/`false` in its place."
    )
}

/// Check if a non-string, non-literal RHS is a valid type expression
/// assignable to the `TypeForm`'s inner type.
fn is_valid_rhs_type_expression(
    _rhs_text: &str,
    _inner: &InferredType,
    _resolver: &AnnotationResolver<'_>,
) -> bool {
    panic!(
        "basilisk-checker: `is_valid_rhs_type_expression` was DELETED because it \
         decided a TypeForm assignment by trimming and RE-PARSING RHS SOURCE TEXT. The \
         reconstructed expression has no original offset or scope. It panics because \
         the real implementation — resolving the RHS's original `Expr` — DOES NOT \
         EXIST YET. Do not restore `resolve_text` and do not choose a default verdict."
    )
}

/// Check function calls with `TypeForm` parameters.
///
/// This catches `func1("not a type")` — an invalid type expression passed to a
/// parameter whose annotation resolves to `TypeForm`.
pub(super) fn check_typeform_calls(
    module: &ResolvedModule,
    resolver: &AnnotationResolver<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;

    for call in &module.calls {
        check_typeform_param_args(
            call,
            &module.functions,
            source,
            &module.path,
            resolver,
            diagnostics,
        );
    }
}

// ##########################################################################
// # DELETED BODY — `check_typeform_param_args`. KEEP ITS CALL SITE.         #
// #                                                                         #
// # The function definition was selected with                             #
// #                                                                         #
// #   functions.iter().find(|func| func.name == call.callee)               #
// #                                                                         #
// # which joins a call to a declaration by RENDERED NAME, ignoring alias,  #
// # rebinding, scope, receiver, and definition position. It then sliced    #
// # each argument from source and sent string literals through the deleted #
// # quote-strip/reparse path. Both the callee and argument already have     #
// # resolved AST identities; neither may be reconstructed from text.       #
// ##########################################################################

/// DELETED — panics; see the banner above.
fn check_typeform_param_args(
    _call: &basilisk_resolver::CallSite,
    _functions: &[FunctionInfo],
    _source: &str,
    _path: &str,
    _resolver: &AnnotationResolver<'_>,
    _diagnostics: &mut Vec<Diagnostic>,
) {
    panic!(
        "basilisk-checker: `check_typeform_param_args` was DELETED because it joined \
         calls to functions by RENDERED NAME and validated arguments by slicing and \
         reparsing SOURCE TEXT. It panics because the real implementation — the call's \
         resolved function definition plus each argument's original `Expr` — DOES NOT \
         EXIST YET. Do not restore the name join and do not return no diagnostics."
    )
}
