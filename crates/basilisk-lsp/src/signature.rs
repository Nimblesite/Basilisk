//! Implements [LSPARCH-FEATURES-SIGHELP]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-SIGHELP
//!
//! Signature Help handler: parameter hints on function calls.

use basilisk_resolver::{FunctionInfo, ResolvedModule, ReturnAnnotationKind};
use tower_lsp::lsp_types::{
    ParameterInformation, ParameterLabel, SignatureHelp, SignatureInformation,
};

/// Compute signature help at a byte offset within source text.
///
/// Detects if the cursor is inside a function call `(...)` and returns
/// the function signature with the active parameter highlighted.
#[must_use]
pub fn signature_help_at(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
) -> Option<SignatureHelp> {
    let (callee, active_param) = find_call_context(source, byte_offset)?;
    let func = find_function(resolved, &callee)?;

    let signature = build_signature(func, source);
    let active_parameter = adjust_active_param(func, active_param);

    Some(SignatureHelp {
        signatures: vec![signature],
        active_signature: Some(0),
        active_parameter: Some(active_parameter),
    })
}

/// Scan backwards from cursor to find the enclosing function call.
///
/// Returns `(callee_name, comma_count)` where `comma_count` is the number of
/// commas before the cursor (indicating which parameter is active).
fn find_call_context(source: &str, offset: usize) -> Option<(String, u32)> {
    let before = source.get(..offset)?;
    let mut depth = 0i32;
    let mut commas = 0u32;

    // Walk backwards to find the matching `(`.
    for (idx, ch) in before.char_indices().rev() {
        match ch {
            ')' | ']' | '}' => depth += 1,
            '(' => {
                if depth == 0 {
                    // Found the opening paren. Extract callee name before it.
                    let before_paren = before.get(..idx)?;
                    let callee = extract_callee(before_paren)?;
                    return Some((callee, commas));
                }
                depth -= 1;
            }
            '[' | '{' if depth > 0 => {
                depth -= 1;
            }
            ',' if depth == 0 => commas += 1,
            _ => {}
        }
    }
    None
}

/// Extract the function/method name immediately before the `(`.
fn extract_callee(text: &str) -> Option<String> {
    let trimmed = text.trim_end();
    let name: String = trimmed
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if name.is_empty() {
        None
    } else {
        // Return just the last segment for method calls (e.g. "self.greet" → "greet")
        Some(name.rsplit('.').next().unwrap_or(&name).to_owned())
    }
}

/// Find a function by name in the resolved module.
///
/// When the callee name matches a class name rather than a function name,
/// this looks up the `__init__` method of that class instead, so that
/// constructor calls like `MyClass(x, y)` show the correct signature.
fn find_function<'a>(resolved: &'a ResolvedModule, name: &str) -> Option<&'a FunctionInfo> {
    // Direct function name match.
    if let Some(func) = resolved.functions.iter().find(|f| f.name == name) {
        return Some(func);
    }

    // If the callee name matches a class, look for its __init__ method.
    let is_class = resolved.classes.iter().any(|c| c.name == name);
    if is_class {
        return resolved
            .functions
            .iter()
            .find(|f| f.name == "__init__" && f.class_name.as_deref() == Some(name));
    }

    None
}

/// Adjust active parameter index to skip `self`/`cls`.
fn adjust_active_param(func: &FunctionInfo, comma_count: u32) -> u32 {
    let has_self = func
        .parameters
        .first()
        .is_some_and(|p| p.name == "self" || p.name == "cls");
    if has_self && func.class_name.is_some() {
        // Caller doesn't pass self, so params are shifted.
        comma_count
    } else {
        comma_count
    }
}

/// Build a `SignatureInformation` from a function.
fn build_signature(func: &FunctionInfo, source: &str) -> SignatureInformation {
    let params_display: Vec<String> = display_params(func, source);
    let label = build_label(func, &params_display, source);

    let parameters: Vec<ParameterInformation> = params_display
        .into_iter()
        .map(|p| ParameterInformation {
            label: ParameterLabel::Simple(p),
            documentation: None,
        })
        .collect();

    SignatureInformation {
        label,
        documentation: None,
        parameters: Some(parameters),
        active_parameter: None,
    }
}

/// Build parameter display strings, skipping `self`/`cls` for methods.
fn display_params(func: &FunctionInfo, source: &str) -> Vec<String> {
    let skip_self = func.class_name.is_some()
        && func
            .parameters
            .first()
            .is_some_and(|p| p.name == "self" || p.name == "cls");

    let params = if skip_self {
        func.parameters.get(1..).unwrap_or(&[])
    } else {
        &func.parameters
    };

    let mut result: Vec<String> = params
        .iter()
        .map(|p| {
            if let Some(ann_span) = p.annotation_span {
                if let Some(ann_text) = ann_span.slice_source(source) {
                    return format!("{}: {}", p.name, ann_text.trim());
                }
            }
            p.name.clone()
        })
        .collect();

    if let Some(ref va) = func.vararg {
        let s = if let Some(ann_span) = va.annotation_span {
            ann_span.slice_source(source).map_or_else(
                || format!("*{}", va.name),
                |ann| format!("*{}: {}", va.name, ann.trim()),
            )
        } else {
            format!("*{}", va.name)
        };
        result.push(s);
    }

    if let Some(ref kw) = func.kwarg {
        let s = if let Some(ann_span) = kw.annotation_span {
            ann_span.slice_source(source).map_or_else(
                || format!("**{}", kw.name),
                |ann| format!("**{}: {}", kw.name, ann.trim()),
            )
        } else {
            format!("**{}", kw.name)
        };
        result.push(s);
    }

    result
}

/// Build the full signature label string.
fn build_label(func: &FunctionInfo, params: &[String], source: &str) -> String {
    let mut label = func.name.clone();
    label.push('(');
    label.push_str(&params.join(", "));
    label.push(')');

    match func.return_annotation {
        ReturnAnnotationKind::Missing => {}
        ReturnAnnotationKind::NoneType => label.push_str(" -> None"),
        ReturnAnnotationKind::Any => label.push_str(" -> Any"),
        _ => {
            if let Some(span) = func.return_annotation_span {
                if let Some(text) = span.slice_source(source) {
                    label.push_str(" -> ");
                    label.push_str(text.trim());
                }
            }
        }
    }

    label
}
