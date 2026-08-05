//! Implements [`callables_annotation`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//! `callables_annotation`: Invalid type argument count or form.
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
//! | `dict[...]`      | exactly 2 | 0, 1, or 3+ args |

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule, Span, VariableInfo};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "callables_annotation",
    docs_url: "https://www.basilisk-python.dev/errors/callables_annotation",
};

/// Emits `callables_annotation` for function parameters whose generic annotation has the
/// wrong number of type arguments.
pub(crate) struct InvalidTypeArgCount;

impl Rule for InvalidTypeArgCount {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
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
            out.push(make_diagnostic(
                &violation,
                annotation,
                &param.name,
                param.name_span,
                path,
            ));
        }
    }
}

/// Check a module-level variable annotation.
fn check_module_var(var: &VariableInfo, source: &str, path: &str, out: &mut Vec<Diagnostic>) {
    let Some(ann_span) = var.annotation_span else {
        return;
    };
    let Some(annotation) = slice_span(source, ann_span) else {
        return;
    };
    if let Some(violation) = check_annotation(annotation.trim()) {
        out.push(make_diagnostic(
            &violation,
            annotation.trim(),
            &var.name,
            var.name_span,
            path,
        ));
    }
}

/// Describes a type-argument count violation.
enum Violation {
    /// Wrong number of type arguments for a generic type.
    ArgCount {
        generic_name: String,
        found: usize,
        expected: usize,
    },
}

/// Checks whether an annotation string has an incorrect number of type args for
/// known generic types.
fn check_annotation(annotation: &str) -> Option<Violation> {
    let trimmed = annotation.trim();

    let bracket_pos = trimmed.find('[')?;
    let generic_name = trimmed.get(..bracket_pos)?.trim().to_owned();
    let after_bracket = trimmed.get(bracket_pos + 1..)?;
    // Strip only the final `]` that closes the outermost `[`.
    let inner = after_bracket.strip_suffix(']').unwrap_or(after_bracket);

    let arg_count = count_type_args(inner);

    let expected: usize = match generic_name.as_str() {
        "list" | "set" | "frozenset" | "type" => 1,
        "dict" => 2,
        _ => return None,
    };

    if arg_count == expected {
        None
    } else {
        Some(Violation::ArgCount {
            generic_name,
            found: arg_count,
            expected,
        })
    }
}

/// Counts comma-separated type arguments at the top level.
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
fn extract_param_annotation(source: &str, name_span: Span) -> Option<&str> {
    let start = name_span.start_usize();
    let line_start = source.get(..start)?.rfind('\n').map_or(0, |p| p + 1);
    let line_end = source
        .get(start..)?
        .find('\n')
        .map_or(source.len(), |p| start + p);

    let line = source.get(line_start..line_end)?;
    let name_offset = start.checked_sub(line_start)?;

    let colon_pos = line.get(name_offset..)?.find(": ")? + name_offset;
    let after_colon = colon_pos + 2;

    let mut depth: usize = 0;
    let annotation_end = line
        .get(after_colon..)?
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

/// Format a violation into a diagnostic.
fn make_diagnostic(
    violation: &Violation,
    annotation: &str,
    name: &str,
    span: Span,
    path: &str,
) -> Diagnostic {
    let (message, help, note) = match violation {
        Violation::ArgCount {
            generic_name,
            found,
            expected,
        } => (
            format!(
                "Invalid type argument count in annotation `{annotation}` on `{name}`: \
                 `{generic_name}` takes {expected} type argument{} but {found} {} provided",
                if *expected == 1 { "" } else { "s" },
                if *found == 1 { "was" } else { "were" },
            ),
            Some(format!(
                "`{generic_name}` requires exactly {expected} type argument{}; \
                 e.g. `{generic_name}[{}]`",
                if *expected == 1 { "" } else { "s" },
                (0..*expected)
                    .map(|i| *["T", "K", "V"].get(i.min(2)).unwrap_or(&"T"))
                    .collect::<Vec<_>>()
                    .join(", "),
            )),
            Some("Provide the correct number of type arguments for this generic type".to_owned()),
        ),
    };

    error_diagnostic_owned(CODE.clone(), message, span, path, help, note)
}
