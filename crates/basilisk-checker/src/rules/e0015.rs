//! BSK-E0015: Invalid type argument count.
//!
//! Certain generic types accept a fixed number of type arguments.  This rule
//! catches the most common violations detectable from source text alone:
//!
//! | Annotation pattern | Expected args | Error condition |
//! |---|---|---|
//! | `list[...]`      | exactly 1 | 0 or 2+ args |
//! | `set[...]`       | exactly 1 | 0 or 2+ args |
//! | `frozenset[...]` | exactly 1 | 0 or 2+ args |
//! | `type[...]`      | exactly 1 | 0 or 2+ args |
//! | `Type[...]`      | exactly 1 | 0 or 2+ args |
//! | `dict[...]`      | exactly 2 | 0, 1, or 3+ args |
//!
//! The check is text-based: the annotation string is extracted from the source
//! around each annotated parameter's name span.  Module-level variable
//! annotations are also checked.

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule, Span, VariableInfo};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0015",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0015",
};

/// Emits BSK-E0015 for function parameters whose generic annotation has the
/// wrong number of type arguments.
pub(crate) struct InvalidTypeArgCount;

impl Rule for InvalidTypeArgCount {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        module
            .functions
            .iter()
            .for_each(|func| check_function(func, &module.source, &module.path, diagnostics));

        module
            .module_vars
            .iter()
            .for_each(|var| check_module_var(var, &module.source, &module.path, diagnostics));
    }
}

fn check_function(func: &FunctionInfo, source: &str, path: &str, out: &mut Vec<Diagnostic>) {
    func.parameters
        .iter()
        .for_each(|p| check_param(p, source, path, out));

    if let Some(vararg) = &func.vararg {
        check_param(vararg, source, path, out);
    }
    if let Some(kwarg) = &func.kwarg {
        check_param(kwarg, source, path, out);
    }
}

fn check_param(param: &ParameterInfo, source: &str, path: &str, out: &mut Vec<Diagnostic>) {
    if !param.has_annotation {
        return;
    }
    if let Some(annotation) = extract_param_annotation(source, param.name_span) {
        if let Some(violation) = check_annotation(annotation) {
            out.push(make_param_diagnostic(param, annotation, &violation, path));
        }
    }
}

/// Check a module-level variable annotation (e.g. `bad_type1: type[int, str]`).
fn check_module_var(var: &VariableInfo, source: &str, path: &str, out: &mut Vec<Diagnostic>) {
    let Some(ann_span) = var.annotation_span else {
        return;
    };
    let Some(annotation) = source.get(ann_span.start as usize..ann_span.end as usize) else {
        return;
    };
    if let Some(violation) = check_annotation(annotation.trim()) {
        out.push(make_var_diagnostic(var, annotation.trim(), &violation, path));
    }
}

/// Describes a type-argument count violation.
struct Violation {
    generic_name: String,
    found: usize,
    expected: usize,
}

/// Checks whether an annotation string has an incorrect number of type args for
/// known generic types.  Returns `None` when no violation is detected.
fn check_annotation(annotation: &str) -> Option<Violation> {
    let trimmed = annotation.trim();

    // Extract `name` and `[...]` from `name[...]`.
    let bracket_pos = trimmed.find('[')?;
    let generic_name = trimmed[..bracket_pos].trim().to_ascii_lowercase();
    let inner = trimmed.get(bracket_pos + 1..)?.trim_end_matches(']');

    let arg_count = count_type_args(inner);

    let expected: usize = match generic_name.as_str() {
        "list" | "set" | "frozenset" | "type" | "optional" => 1,
        "dict" => 2,
        "callable" => {
            // Callable has special syntax: Callable[[arg_types], return_type]
            // We need to handle this differently
            if inner.starts_with('[') && inner.contains("],") {
                2 // Callable[[args], return_type] - 2 arguments
            } else {
                1 // Malformed callable
            }
        }
        "union" => {
            // Union can have any number of arguments, but at least 2
            if arg_count < 2 {
                2 // Union needs at least 2 arguments
            } else {
                return None; // Valid union
            }
        }
        _ => return None,
    };

    if arg_count == expected {
        None
    } else {
        Some(Violation {
            generic_name,
            found: arg_count,
            expected,
        })
    }
}

/// Counts the number of comma-separated type arguments at the top level of an
/// annotation fragment (i.e. not inside nested brackets).
///
/// An empty string yields 0.  A non-empty string with no commas at depth 0
/// yields 1.
fn count_type_args(inner: &str) -> usize {
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        return 0;
    }

    let mut depth: usize = 0;
    let mut count = 1usize;

    for ch in trimmed.chars() {
        match ch {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }

    count
}

/// Extracts the annotation text for a parameter from the source.
///
/// Looks for `: <annotation>` on the same line as the parameter name, ending
/// at the first `,` or `)` at bracket depth 0.
fn extract_param_annotation(source: &str, name_span: Span) -> Option<&str> {
    let start = name_span.start as usize;
    let line_start = source[..start].rfind('\n').map_or(0, |p| p + 1);
    let line_end = source[start..]
        .find('\n')
        .map_or(source.len(), |p| start + p);

    let line = source.get(line_start..line_end)?;
    let name_offset = start.checked_sub(line_start)?;

    // Find `: ` starting at name_offset.
    let colon_pos = line[name_offset..].find(": ")? + name_offset;
    let after_colon = colon_pos + 2;

    // Scan forward to find end of annotation (`,` or `)` at depth 0 or `=`).
    let mut depth: usize = 0;
    let annotation_end = line[after_colon..]
        .char_indices()
        .find_map(|(idx, ch)| match ch {
            '[' => {
                depth += 1;
                None
            }
            ']' => {
                depth = depth.saturating_sub(1);
                None
            }
            ',' | ')' | '=' if depth == 0 => Some(after_colon + idx),
            _ => None,
        })
        .unwrap_or(line.len());

    let annotation = line.get(after_colon..annotation_end)?.trim();
    if annotation.is_empty() {
        None
    } else {
        Some(annotation)
    }
}

fn make_param_diagnostic(
    param: &ParameterInfo,
    annotation: &str,
    violation: &Violation,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Invalid type argument count in annotation `{}` on `{}`: \
             `{}` takes {} type argument{} but {} {} provided",
            annotation,
            param.name,
            violation.generic_name,
            violation.expected,
            if violation.expected == 1 { "" } else { "s" },
            violation.found,
            if violation.found == 1 { "was" } else { "were" },
        ),
        span: param.name_span,
        path: path.to_owned(),
        help: Some(format!(
            "`{}` requires exactly {} type argument{}; e.g. `{}[{}]`",
            violation.generic_name,
            violation.expected,
            if violation.expected == 1 { "" } else { "s" },
            violation.generic_name,
            (0..violation.expected)
                .map(|i| ["T", "K", "V"][i.min(2)])
                .collect::<Vec<_>>()
                .join(", "),
        )),
        note: Some("Provide the correct number of type arguments for this generic type".to_owned()),
    }
}

fn make_var_diagnostic(
    var: &VariableInfo,
    annotation: &str,
    violation: &Violation,
    path: &str,
) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message: format!(
            "Invalid type argument count in annotation `{}` on `{}`: \
             `{}` takes {} type argument{} but {} {} provided",
            annotation,
            var.name,
            violation.generic_name,
            violation.expected,
            if violation.expected == 1 { "" } else { "s" },
            violation.found,
            if violation.found == 1 { "was" } else { "were" },
        ),
        span: var.name_span,
        path: path.to_owned(),
        help: Some(format!(
            "`{}` requires exactly {} type argument{}; e.g. `{}[{}]`",
            violation.generic_name,
            violation.expected,
            if violation.expected == 1 { "" } else { "s" },
            violation.generic_name,
            (0..violation.expected)
                .map(|i| ["T", "K", "V"][i.min(2)])
                .collect::<Vec<_>>()
                .join(", "),
        )),
        note: Some("Provide the correct number of type arguments for this generic type".to_owned()),
    }
}
