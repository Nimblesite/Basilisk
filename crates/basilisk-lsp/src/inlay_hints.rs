//! Inlay Hints handler: inferred types and parameter names.

use basilisk_resolver::{ResolvedModule, RhsKind};
use tower_lsp::lsp_types::{InlayHint, InlayHintKind, InlayHintLabel};

use crate::util::byte_offset_to_position;

/// Compute inlay hints for a resolved module.
#[must_use]
pub fn inlay_hints(resolved: &ResolvedModule, source: &str) -> Vec<InlayHint> {
    let mut hints = Vec::new();

    // Variable type hints — unannotated variables with inferable types.
    variable_type_hints(resolved, source, &mut hints);

    // Parameter name hints at call sites.
    parameter_name_hints(resolved, source, &mut hints);

    // Return type hints for functions without explicit annotations.
    function_return_type_hints(resolved, source, &mut hints);

    // Generic type parameter hints — variance, bounds, constraints.
    generic_type_param_hints(resolved, source, &mut hints);

    hints
}

/// Add type hints for unannotated module-level variables.
fn variable_type_hints(resolved: &ResolvedModule, source: &str, hints: &mut Vec<InlayHint>) {
    for var in &resolved.module_vars {
        if var.has_annotation {
            continue;
        }
        let type_name = rhs_type_display(&var.rhs_kind);
        if type_name.is_empty() {
            continue;
        }
        hints.push(InlayHint {
            position: byte_offset_to_position(source, var.name_span.end_usize()),
            label: InlayHintLabel::String(format!(": {type_name}")),
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: None,
            padding_right: Some(true),
            data: None,
        });
    }

    // Local variables within functions.
    for func in &resolved.functions {
        for var in &func.local_vars {
            if var.has_annotation {
                continue;
            }
            let type_name = rhs_type_display(&var.rhs_kind);
            if type_name.is_empty() {
                continue;
            }
            hints.push(InlayHint {
                position: byte_offset_to_position(source, var.name_span.end_usize()),
                label: InlayHintLabel::String(format!(": {type_name}")),
                kind: Some(InlayHintKind::TYPE),
                text_edits: None,
                tooltip: None,
                padding_left: None,
                padding_right: Some(true),
                data: None,
            });
        }
    }
}

/// Add parameter name hints at call sites.
fn parameter_name_hints(resolved: &ResolvedModule, source: &str, hints: &mut Vec<InlayHint>) {
    for call in &resolved.calls {
        // Find the function definition for this call.
        let func = resolved.functions.iter().find(|f| f.name == call.callee);
        let Some(func) = func else {
            continue;
        };

        // Skip self/cls for method calls.
        let params = if func.class_name.is_some()
            && func
                .parameters
                .first()
                .is_some_and(|p| p.name == "self" || p.name == "cls")
        {
            func.parameters.get(1..).unwrap_or(&[])
        } else {
            &func.parameters
        };

        for (idx, (_, arg_span)) in call.args.iter().enumerate() {
            let Some(param) = params.get(idx) else {
                break;
            };
            // Don't add hints for args that already have keyword= syntax.
            let arg_text = source.get(arg_span.as_range());
            if let Some(text) = arg_text {
                if text.contains('=') {
                    continue;
                }
            }
            hints.push(InlayHint {
                position: byte_offset_to_position(source, arg_span.start_usize()),
                label: InlayHintLabel::String(format!("{}=", param.name)),
                kind: Some(InlayHintKind::PARAMETER),
                text_edits: None,
                tooltip: None,
                padding_left: None,
                padding_right: Some(true),
                data: None,
            });
        }
    }
}

/// Add return type hints for functions without explicit return annotations.
fn function_return_type_hints(resolved: &ResolvedModule, source: &str, hints: &mut Vec<InlayHint>) {
    for func in &resolved.functions {
        // Skip functions that already have an explicit return annotation.
        if func.return_annotation_span.is_some() {
            continue;
        }

        let inferred = infer_return_type(func);
        if inferred.is_empty() {
            continue;
        }

        // Find the closing `)` of the parameter list by scanning from after the
        // function name.  We track parenthesis nesting so nested default-value
        // expressions (e.g. `def f(x=(1,2)):`) do not fool us.
        let Some(paren_close) = find_closing_paren(source, func.name_span.end_usize()) else {
            continue;
        };

        hints.push(InlayHint {
            position: byte_offset_to_position(source, paren_close + 1),
            label: InlayHintLabel::String(format!(" -> {inferred}")),
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: None,
            padding_right: None,
            data: None,
        });
    }
}

/// Infer the return type of a function from its `return` statements.
///
/// Returns an empty string when the type cannot be determined.
fn infer_return_type(func: &basilisk_resolver::FunctionInfo) -> &'static str {
    if func.return_stmts.is_empty() {
        return "None";
    }

    // Collect the display names for every return statement.
    let mut common_type: Option<&'static str> = None;
    for ret in &func.return_stmts {
        let display = rhs_type_display(&ret.rhs_kind);
        // If any return has an uninferrable type, bail out.
        if display.is_empty() {
            return "";
        }
        match common_type {
            None => common_type = Some(display),
            Some(prev) if prev == display => {}
            Some(_) => return "", // mixed return types — cannot infer
        }
    }

    common_type.unwrap_or("None")
}

/// Find the byte offset of the closing `)` of the parameter list starting
/// from `start` (just after the function name).  Returns `None` when the
/// source is malformed.
fn find_closing_paren(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    // First, find the opening `(`.
    let mut pos = start;
    while pos < bytes.len() && bytes.get(pos).copied() != Some(b'(') {
        pos += 1;
    }
    if pos >= bytes.len() {
        return None;
    }
    // Now track nesting to find the matching `)`.
    let mut depth: u32 = 0;
    for (idx, &byte) in bytes.iter().enumerate().skip(pos) {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

/// Add generic type parameter hints: variance, bounds, and constraints for
/// `TypeVar(...)` definitions and classes with `Generic[...]` parameters.
fn generic_type_param_hints(resolved: &ResolvedModule, source: &str, hints: &mut Vec<InlayHint>) {
    // TypeVar definition hints — show variance / bound / constraints after the call.
    for tv in &resolved.typevar_calls {
        let label = typevar_hint_label(tv);
        if label.is_empty() {
            continue;
        }
        hints.push(InlayHint {
            position: byte_offset_to_position(source, tv.span.end_usize()),
            label: InlayHintLabel::String(format!("  {label}")),
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
        });
    }

    // Class generic parameter hints — show `[T, U, ...]` after the class name
    // when the params come from `Generic[...]` (not PEP 695, where they're already visible).
    for class in &resolved.classes {
        if class.generic_params.is_empty() || class.has_pep695_type_params {
            continue;
        }
        let params: Vec<&str> = basilisk_resolver::collect_names(&class.generic_params);
        let label = format!("[{}]", params.join(", "));
        hints.push(InlayHint {
            position: byte_offset_to_position(source, class.name_span.end_usize()),
            label: InlayHintLabel::String(label),
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            tooltip: Some(tower_lsp::lsp_types::InlayHintTooltip::String(
                "Generic type parameters from Generic[...] base".to_owned(),
            )),
            padding_left: None,
            padding_right: None,
            data: None,
        });
    }
}

/// Build a concise label for a `TypeVar` definition hint.
///
/// Shows variance, bound, constraints, and/or default — whichever are present.
fn typevar_hint_label(tv: &basilisk_resolver::scope::TypeVarCallInfo) -> String {
    let mut parts = Vec::new();

    // Kind prefix for non-TypeVar variants.
    if tv.is_typevartuple {
        parts.push("TypeVarTuple".to_owned());
    } else if tv.is_paramspec {
        parts.push("ParamSpec".to_owned());
    }

    // Variance.
    if tv.is_covariant {
        parts.push("covariant".to_owned());
    } else if tv.is_contravariant {
        parts.push("contravariant".to_owned());
    } else if tv.has_infer_variance {
        parts.push("infer_variance".to_owned());
    }

    // Bound.
    if let Some(ref bound) = tv.bound_type_name {
        parts.push(format!("bound: {bound}"));
    }

    // Constraints.
    if !tv.constraint_type_names.is_empty() {
        parts.push(tv.constraint_type_names.join(" | "));
    }

    // Default (PEP 696).
    if let Some(ref default) = tv.default_type_name {
        parts.push(format!("default: {default}"));
    }

    parts.join(", ")
}

/// Simple type name from `RhsKind`.
fn rhs_type_display(rhs: &RhsKind) -> &'static str {
    match rhs {
        RhsKind::IntLiteral => "int",
        RhsKind::FloatLiteral => "float",
        RhsKind::StrLiteral => "str",
        RhsKind::BoolLiteral => "bool",
        RhsKind::BytesLiteral => "bytes",
        RhsKind::NoneValue => "None",
        RhsKind::EmptyList | RhsKind::List(_) => "list",
        RhsKind::EmptyDict | RhsKind::Dict(_) => "dict",
        RhsKind::Set(_) => "set",
        RhsKind::Tuple(_) => "tuple",
        _ => "",
    }
}
