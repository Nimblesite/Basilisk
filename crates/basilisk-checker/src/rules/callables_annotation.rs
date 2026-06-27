//! Implements [BSK-E0015] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag-typesafety
//! BSK-E0015: Invalid type argument count or form.
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
//! | `Callable[...]`  | exactly 2 | wrong count or invalid form |
//!
//! For `Callable`, the first argument must be a parameter list `[int, str]`,
//! bare ellipsis `...`, a `ParamSpec`, or `Concatenate[...]`.  The second
//! argument (return type) must not be a list literal.

use basilisk_resolver::{FunctionInfo, ParameterInfo, ResolvedModule, Span, VariableInfo};

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0015",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0015",
};

/// Emits BSK-E0015 for function parameters whose generic annotation has the
/// wrong number of type arguments or invalid Callable form.
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

/// Describes a type-argument count or form violation.
enum Violation {
    /// Wrong number of type arguments for a generic type.
    ArgCount {
        generic_name: String,
        found: usize,
        expected: usize,
    },
    /// Invalid Callable first argument.
    CallableFirstArg { first_arg: String },
    /// Invalid Callable return type (list literal).
    CallableReturnType { return_type: String },
    /// Ellipsis inside brackets `Callable[[...], int]`.
    CallableEllipsisInBrackets,
}

/// Checks whether an annotation string has an incorrect number of type args for
/// known generic types, or has an invalid Callable form.
fn check_annotation(annotation: &str) -> Option<Violation> {
    let trimmed = annotation.trim();

    let bracket_pos = trimmed.find('[')?;
    let generic_name = trimmed.get(..bracket_pos)?.trim().to_ascii_lowercase();
    let after_bracket = trimmed.get(bracket_pos + 1..)?;
    // Strip only the final `]` that closes the outermost `[`.
    let inner = after_bracket.strip_suffix(']').unwrap_or(after_bracket);

    let arg_count = count_type_args(inner);

    let expected: usize = match generic_name.as_str() {
        "list" | "set" | "frozenset" | "type" | "optional" => 1,
        "dict" => 2,
        "callable" => {
            if arg_count != 2 {
                return Some(Violation::ArgCount {
                    generic_name,
                    found: arg_count,
                    expected: 2,
                });
            }
            return check_callable_form(inner);
        }
        "union" if arg_count < 2 => 2,
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

/// Validates the form of a `Callable[``first_arg``, ``return_type``]` annotation.
fn check_callable_form(inner: &str) -> Option<Violation> {
    let trimmed = inner.trim();

    let (first_arg, return_type) = split_callable_args(trimmed)?;
    let first = first_arg.trim();
    let ret = return_type.trim();

    // Return type must not be a list literal like `[int]`.
    if ret.starts_with('[') && ret.ends_with(']') {
        return Some(Violation::CallableReturnType {
            return_type: ret.to_owned(),
        });
    }

    // Bare ellipsis — valid.
    if first == "..." {
        return None;
    }

    // List form — check for `[...]` (ellipsis inside brackets is invalid).
    if first.starts_with('[') && first.ends_with(']') {
        let list_inner = first.get(1..first.len().saturating_sub(1))?;
        let list_inner = list_inner.trim();
        if list_inner == "..." {
            return Some(Violation::CallableEllipsisInBrackets);
        }
        return None;
    }

    // Concatenate[...] form — valid.
    if first.starts_with("Concatenate[") {
        return None;
    }

    // Known builtin types used as first arg — always invalid (not a ParamSpec).
    let lower = first.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "int" | "str" | "float" | "bool" | "bytes" | "none" | "object" | "type"
    ) {
        return Some(Violation::CallableFirstArg {
            first_arg: first.to_owned(),
        });
    }

    // Single identifier — could be a ParamSpec. Accept it.
    None
}

/// Splits `Callable[...]` inner text into (``first_arg``, ``return_type``) at the
/// top-level comma separating the two arguments.
fn split_callable_args(inner: &str) -> Option<(&str, &str)> {
    let mut depth: usize = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                return Some((inner.get(..idx)?, inner.get(idx + 1..)?));
            }
            _ => {}
        }
    }
    None
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
        Violation::CallableFirstArg { first_arg } => (
            format!(
                "Invalid `Callable` annotation on `{name}`: \
                 first argument `{first_arg}` is not a valid parameter specification; \
                 expected `[arg_types]`, `...`, a `ParamSpec`, or `Concatenate[...]`"
            ),
            Some(
                "Use `Callable[[int, str], ReturnType]` or `Callable[..., ReturnType]`".to_owned(),
            ),
            Some("The first argument to `Callable` defines the parameter types".to_owned()),
        ),
        Violation::CallableReturnType { return_type } => (
            format!(
                "Invalid `Callable` annotation on `{name}`: \
                 return type `{return_type}` is a list literal; use a plain type instead"
            ),
            Some(
                "Use `Callable[[arg_types], int]` instead of `Callable[[arg_types], [int]]`"
                    .to_owned(),
            ),
            Some("The return type in `Callable` must be a single type, not a list".to_owned()),
        ),
        Violation::CallableEllipsisInBrackets => (
            format!(
                "Invalid `Callable` annotation on `{name}`: \
                 `[...]` is not valid; use bare `...` for an arbitrary parameter list"
            ),
            Some(
                "Use `Callable[..., ReturnType]` instead of `Callable[[...], ReturnType]`"
                    .to_owned(),
            ),
            Some("Ellipsis must appear directly, not inside brackets".to_owned()),
        ),
    };

    error_diagnostic_owned(CODE.clone(), message, span, path, help, note)
}
