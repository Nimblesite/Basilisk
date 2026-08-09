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

/// Builtin type constructors — calling these creates an instance, not a type form.
const BUILTIN_TYPE_CONSTRUCTORS: &[&str] = &[
    "tuple",
    "list",
    "dict",
    "set",
    "frozenset",
    "int",
    "str",
    "float",
    "bool",
    "bytes",
    "complex",
    "bytearray",
    "memoryview",
    "object",
    "range",
    "slice",
    "type",
];

/// Check whether a RHS expression is a valid type form assignable to `inner`.
///
/// Returns `true` if the assignment is valid (no diagnostic needed),
/// `false` if a diagnostic should be emitted.
pub(super) fn is_valid_typeform_assignment(
    var: &VariableInfo,
    source: &str,
    inner: &InferredType,
    functions: &[FunctionInfo],
    resolver: &AnnotationResolver<'_>,
) -> bool {
    let Some(rhs_span) = var.rhs_span else {
        return true; // No RHS to check
    };
    let Some(rhs_text) = slice_span(source, rhs_span) else {
        return true;
    };
    let rhs_text = rhs_text.trim();

    // Check the `RhsKind` first for obviously invalid values
    match &var.rhs_kind {
        basilisk_resolver::RhsKind::IntLiteral
        | basilisk_resolver::RhsKind::FloatLiteral
        | basilisk_resolver::RhsKind::BoolLiteral
        | basilisk_resolver::RhsKind::BytesLiteral
        | basilisk_resolver::RhsKind::Tuple(_)
        | basilisk_resolver::RhsKind::TypeCall => return false,
        basilisk_resolver::RhsKind::CallExpr => {
            return is_valid_call_typeform(rhs_text, inner, functions, source, resolver);
        }
        basilisk_resolver::RhsKind::StrLiteral => {
            return is_valid_string_typeform(rhs_text, inner, resolver);
        }
        basilisk_resolver::RhsKind::NoneValue => {
            // `None` is a valid type expression representing `NoneType`.
            return InferredType::None_.is_assignable_to(inner);
        }
        _ => {}
    }

    // For `Other`/`Lambda`/etc., parse the RHS text as a type expression
    is_valid_rhs_type_expression(rhs_text, inner, resolver)
}

/// Check whether a function call result is a valid `TypeForm` assignment.
///
/// Builtin type constructors (`tuple()`, `list()`) create instances, not type
/// forms. User-defined functions are checked by looking up their return
/// annotation — if it returns `TypeForm[S]` or `type[S]`, we verify `S` is
/// assignable to the inner type.
fn is_valid_call_typeform(
    rhs_text: &str,
    inner: &InferredType,
    functions: &[FunctionInfo],
    source: &str,
    resolver: &AnnotationResolver<'_>,
) -> bool {
    // Extract the callee name (before `(`)
    let callee = rhs_text.split('(').next().unwrap_or("").trim();
    let callee_lower = callee.to_ascii_lowercase();

    // Builtin type constructors create instances, not type forms
    if BUILTIN_TYPE_CONSTRUCTORS.contains(&callee_lower.as_str()) {
        return false;
    }

    // Look up user-defined function return types; anything the cascade
    // cannot answer falls through to the conservative acceptance below.
    functions
        .iter()
        .find(|func| func.name == callee)
        .and_then(|func| callee_return_typeform(func, inner, source, resolver))
        .unwrap_or(true)
}

/// Whether `func`'s declared return type makes it a valid `TypeForm[inner]`
/// producer. `None` when the annotation is missing or the cascade cannot
/// resolve it — the caller then accepts conservatively.
fn callee_return_typeform(
    func: &FunctionInfo,
    inner: &InferredType,
    source: &str,
    resolver: &AnnotationResolver<'_>,
) -> Option<bool> {
    let ret_span = func.return_annotation_span?;
    let ret_text = slice_span(source, ret_span)?.trim();
    // The return annotation is a type expression the cascade evaluates
    // ([NARROWPLAN-INTEGRATION] Step 7).
    let ret_type = resolver
        .resolve_span(ret_span)
        .or_else(|| resolver.resolve_text(ret_text))?;
    // Returning `TypeForm[S]`: check S assignable to inner.
    if let InferredType::TypeForm(ref ret_inner) = ret_type {
        return Some(ret_inner.is_assignable_to(inner));
    }
    // `type[S]` is a subtype of `TypeForm[S]` (PEP 747), but the cascade
    // collapses `type[..]` to the nominal `type` leaf, so `S` is resolved
    // from the annotation's own subscript.
    let type_inner = type_subscript_inner(ret_text)?;
    Some(resolver.resolve_text(type_inner)?.is_assignable_to(inner))
}

/// Check if a string literal is a valid type form.
///
/// The string content (without quotes) must parse as a valid type expression,
/// and the represented type must be assignable to `inner`.
fn is_valid_string_typeform(
    rhs_text: &str,
    inner: &InferredType,
    resolver: &AnnotationResolver<'_>,
) -> bool {
    // Strip quotes
    let content = if (rhs_text.starts_with('"') && rhs_text.ends_with('"'))
        || (rhs_text.starts_with('\'') && rhs_text.ends_with('\''))
    {
        &rhs_text[1..rhs_text.len() - 1]
    } else {
        return false;
    };

    // Must parse as a valid type expression
    if !is_parseable_type_expression(content) {
        return false;
    }

    // Check assignability of the represented type to inner
    resolver
        .resolve_text(content)
        .is_some_and(|represented| represented.is_assignable_to(inner))
}

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
    rhs_text: &str,
    inner: &InferredType,
    resolver: &AnnotationResolver<'_>,
) -> bool {
    // The RHS *is* a type expression — evaluate it through the cascade.
    resolver
        .resolve_text(rhs_text.trim())
        .is_some_and(|represented| represented.is_assignable_to(inner))
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

/// Check function call arguments against `TypeForm` parameter annotations.
fn check_typeform_param_args(
    call: &basilisk_resolver::CallSite,
    functions: &[FunctionInfo],
    source: &str,
    path: &str,
    resolver: &AnnotationResolver<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Find the function definition
    let Some(func) = functions.iter().find(|func| func.name == call.callee) else {
        return;
    };

    // Check each positional argument against its parameter annotation
    for (idx, (ref rhs_kind, arg_span)) in call.args.iter().enumerate() {
        let Some(param) = func.parameters.get(idx) else {
            continue;
        };

        if !param.has_annotation {
            continue;
        }

        let Some(ann_span) = param.annotation_span else {
            continue;
        };
        let Some(param_type) = resolver.resolve_span(ann_span) else {
            continue;
        };
        let InferredType::TypeForm(ref inner) = param_type else {
            continue;
        };

        // This parameter expects a `TypeForm` — validate the argument
        let Some(arg_text) = slice_span(source, *arg_span) else {
            continue;
        };
        let arg_text = arg_text.trim();

        let is_invalid = match rhs_kind {
            basilisk_resolver::RhsKind::StrLiteral => {
                !is_valid_string_typeform(arg_text, inner, resolver)
            }
            basilisk_resolver::RhsKind::IntLiteral
            | basilisk_resolver::RhsKind::FloatLiteral
            | basilisk_resolver::RhsKind::BoolLiteral
            | basilisk_resolver::RhsKind::BytesLiteral
            | basilisk_resolver::RhsKind::Tuple(_) => true,
            _ => false,
        };

        if is_invalid {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Argument `{arg_text}` is not a valid type expression for \
                     parameter `{}` of type `{param_type}`",
                    param.name
                ),
                *arg_span,
                path,
                Some(format!(
                    "Pass a valid type expression assignable to `{inner}`"
                )),
                Some(
                    "TypeForm parameters require valid type expressions, not runtime values"
                        .to_owned(),
                ),
            ));
        }
    }
}
