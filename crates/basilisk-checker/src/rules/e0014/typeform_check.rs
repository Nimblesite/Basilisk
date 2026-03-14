//! TypeForm validation for BSK-E0014.
//!
//! When the declared type is `TypeForm[T]`, the RHS must be a valid type
//! expression whose represented type is assignable to `T`.  This module
//! validates type form assignments by parsing the RHS source text as a
//! type expression rather than as a runtime value.
//!
//! Reference: <https://typing.readthedocs.io/en/latest/spec/type-forms.html>

use crate::diagnostic::{Diagnostic, Severity};
use crate::span_util::slice_span;
use crate::types::InferredType;

use basilisk_resolver::{FunctionInfo, ResolvedModule, VariableInfo};

use super::CODE;

/// Special forms that are NOT valid type expressions in a TypeForm context.
///
/// Per PEP 747, `Self`, `ClassVar`, `Final`, `Unpack`, and bare `Optional`
/// (without type argument) are not valid type form objects.
const INVALID_TYPE_FORMS: &[&str] = &["Self", "ClassVar", "Final", "Unpack"];

/// Forms that are only valid as type expressions when parameterised (with `[T]`).
const REQUIRES_PARAMETERISATION: &[&str] = &["Optional"];

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
) -> bool {
    let Some(rhs_span) = var.rhs_span else {
        return true; // No RHS to check
    };
    let Some(rhs_text) = slice_span(source, rhs_span) else {
        return true;
    };
    let rhs_text = rhs_text.trim();

    // Check the RhsKind first for obviously invalid values
    match &var.rhs_kind {
        basilisk_resolver::RhsKind::IntLiteral
        | basilisk_resolver::RhsKind::FloatLiteral
        | basilisk_resolver::RhsKind::BoolLiteral
        | basilisk_resolver::RhsKind::BytesLiteral => return false,
        basilisk_resolver::RhsKind::Tuple(_) => return false,
        basilisk_resolver::RhsKind::TypeCall => return false,
        basilisk_resolver::RhsKind::CallExpr => {
            return is_valid_call_typeform(rhs_text, inner, functions, source);
        }
        basilisk_resolver::RhsKind::StrLiteral => {
            return is_valid_string_typeform(rhs_text, inner);
        }
        basilisk_resolver::RhsKind::NoneValue => {
            // None is a valid type expression representing NoneType.
            return InferredType::None_.is_assignable_to(inner);
        }
        _ => {}
    }

    // For Other/Lambda/etc., parse the RHS text as a type expression
    is_valid_rhs_type_expression(rhs_text, inner)
}

/// Check whether a function call result is a valid TypeForm assignment.
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
) -> bool {
    // Extract the callee name (before `(`)
    let callee = rhs_text.split('(').next().unwrap_or("").trim();
    let callee_lower = callee.to_ascii_lowercase();

    // Builtin type constructors create instances, not type forms
    if BUILTIN_TYPE_CONSTRUCTORS.contains(&callee_lower.as_str()) {
        return false;
    }

    // Look up user-defined function return types
    if let Some(func) = functions.iter().find(|f| f.name == callee) {
        if let Some(ret_span) = func.return_annotation_span {
            if let Some(ret_text) = slice_span(source, ret_span) {
                let ret_type = InferredType::from_annotation(ret_text.trim());
                // If the function returns TypeForm[S], check S assignable to inner
                if let InferredType::TypeForm(ref ret_inner) = ret_type {
                    return ret_inner.is_assignable_to(inner);
                }
                // If the function returns type[S], check S assignable to inner
                // (type[T] is a subtype of TypeForm[T])
                let ret_text_trimmed = ret_text.trim().to_ascii_lowercase();
                if ret_text_trimmed.starts_with("type[") && ret_text_trimmed.ends_with(']') {
                    let type_inner = &ret_text_trimmed["type[".len()..ret_text_trimmed.len() - 1];
                    let type_inner_type = InferredType::from_annotation(type_inner);
                    return type_inner_type.is_assignable_to(inner);
                }
            }
        }
    }

    // For unknown functions, accept conservatively to avoid FPs
    true
}

/// Check if a string literal is a valid type form.
///
/// The string content (without quotes) must parse as a valid type expression,
/// and the represented type must be assignable to `inner`.
fn is_valid_string_typeform(rhs_text: &str, inner: &InferredType) -> bool {
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
    let represented = InferredType::from_annotation(content);
    represented.is_assignable_to(inner)
}

/// Check whether a text parses as a valid Python type expression.
///
/// A valid type expression contains only type names, `|`, `[]`, `.`,
/// and recognised typing constructs.  Expressions like `type(1)` or
/// `int + str` are NOT valid.
fn is_parseable_type_expression(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }

    // Reject expressions containing operators that aren't `|` (union)
    for ch in text.chars() {
        if matches!(
            ch,
            '+' | '-' | '*' | '/' | '%' | '(' | ')' | '!' | '~' | '^' | '&'
        ) {
            return false;
        }
    }

    // Validate each `|`-separated component is a plausible type name.
    // A type name consists of identifiers optionally separated by `.` and
    // optionally followed by `[...]`.  Bare words with spaces (e.g.
    // `"not a type"`) are not valid type expressions.
    for part in text.split('|') {
        let part = part.trim();
        if part.is_empty() {
            return false;
        }
        // Strip any trailing `[...]` subscript
        let base = part.split('[').next().unwrap_or(part).trim();
        if base.is_empty() {
            return false;
        }
        // A type name must be a dotted identifier (e.g. `int`, `typing.List`)
        // — no spaces allowed within a single type name
        if base.contains(' ') {
            return false;
        }
    }

    true
}

/// Check if a non-string, non-literal RHS is a valid type expression
/// assignable to the TypeForm's inner type.
fn is_valid_rhs_type_expression(rhs_text: &str, inner: &InferredType) -> bool {
    let rhs_text = rhs_text.trim();

    let base_name = rhs_text.split('[').next().unwrap_or(rhs_text).trim();

    // Reject forms that are never valid as type expressions
    if INVALID_TYPE_FORMS.contains(&base_name) {
        return false;
    }

    // Forms that need parameterisation (e.g. bare `Optional` without `[T]`)
    if REQUIRES_PARAMETERISATION.contains(&base_name) && !rhs_text.contains('[') {
        return false;
    }

    // Reject `Final[...]` and `Unpack[...]` even when parameterised
    if rhs_text.starts_with("Final[") || rhs_text.starts_with("Unpack[") {
        return false;
    }

    // Handle `Annotated[T, metadata]` — the type form is just `T`
    if rhs_text.starts_with("Annotated[") && rhs_text.ends_with(']') {
        let params = &rhs_text["Annotated[".len()..rhs_text.len() - 1];
        // First param before comma is the actual type
        let type_part = params.split(',').next().unwrap_or("").trim();
        if type_part.is_empty() {
            return false;
        }
        let represented = InferredType::from_annotation(type_part);
        return represented.is_assignable_to(inner);
    }

    // Parse the RHS as a type annotation and check assignability
    let represented = InferredType::from_annotation(rhs_text);

    // If it parsed as a known type, check assignability to inner
    represented.is_assignable_to(inner)
}

/// Check TypeForm constructor calls and function calls with TypeForm parameters.
///
/// This catches:
/// - `TypeForm("type(1)")` — invalid type expression as TypeForm constructor arg
/// - `func1("not a type")` — invalid type expression passed to TypeForm param
pub(super) fn check_typeform_calls(module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
    let source = &module.source;

    for call in &module.calls {
        // Check TypeForm() constructor calls
        if call.callee == "TypeForm" {
            check_typeform_constructor(call, source, &module.path, diagnostics);
            continue;
        }

        // Check function calls where parameters have TypeForm annotations
        check_typeform_param_args(call, &module.functions, source, &module.path, diagnostics);
    }
}

/// Validate a `TypeForm(arg)` constructor call.
fn check_typeform_constructor(
    call: &basilisk_resolver::CallSite,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // TypeForm() takes exactly one argument
    if call.args.len() != 1 {
        return;
    }

    let (ref rhs_kind, arg_span) = call.args[0];
    let Some(arg_text) = slice_span(source, arg_span) else {
        return;
    };
    let arg_text = arg_text.trim();

    let is_invalid = match rhs_kind {
        basilisk_resolver::RhsKind::StrLiteral => {
            !is_valid_string_typeform(arg_text, &InferredType::Any)
        }
        basilisk_resolver::RhsKind::CallExpr | basilisk_resolver::RhsKind::TypeCall => true,
        basilisk_resolver::RhsKind::IntLiteral
        | basilisk_resolver::RhsKind::FloatLiteral
        | basilisk_resolver::RhsKind::BoolLiteral
        | basilisk_resolver::RhsKind::BytesLiteral => true,
        basilisk_resolver::RhsKind::Tuple(_) => true,
        _ => false,
    };

    if is_invalid {
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Invalid TypeForm argument: `{arg_text}` is not a valid type expression"
            ),
            span: call.span,
            path: path.to_owned(),
            help: Some(
                "TypeForm() requires a valid type expression such as `int`, `str | None`, or `list[int]`".to_owned(),
            ),
            note: Some(
                "TypeForm acts as a function that can be called with a single valid type expression"
                    .to_owned(),
            ),
        });
    }
}

/// Check function call arguments against TypeForm parameter annotations.
fn check_typeform_param_args(
    call: &basilisk_resolver::CallSite,
    functions: &[FunctionInfo],
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Find the function definition
    let Some(func) = functions.iter().find(|f| f.name == call.callee) else {
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
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };

        let param_type = InferredType::from_annotation(ann_text.trim());
        let InferredType::TypeForm(ref inner) = param_type else {
            continue;
        };

        // This parameter expects a TypeForm — validate the argument
        let Some(arg_text) = slice_span(source, *arg_span) else {
            continue;
        };
        let arg_text = arg_text.trim();

        let is_invalid = match rhs_kind {
            basilisk_resolver::RhsKind::StrLiteral => !is_valid_string_typeform(arg_text, inner),
            basilisk_resolver::RhsKind::IntLiteral
            | basilisk_resolver::RhsKind::FloatLiteral
            | basilisk_resolver::RhsKind::BoolLiteral
            | basilisk_resolver::RhsKind::BytesLiteral => true,
            basilisk_resolver::RhsKind::Tuple(_) => true,
            _ => false,
        };

        if is_invalid {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Argument `{arg_text}` is not a valid type expression for parameter `{}` of type `{param_type}`",
                    param.name
                ),
                span: *arg_span,
                path: path.to_owned(),
                help: Some(format!(
                    "Pass a valid type expression assignable to `{inner}`"
                )),
                note: Some(
                    "TypeForm parameters require valid type expressions, not runtime values"
                        .to_owned(),
                ),
            });
        }
    }
}
