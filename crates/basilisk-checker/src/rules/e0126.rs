//! BSK-E0126: `LiteralString` and `Literal` assignment incompatibilities.
//!
//! Detects annotated local variables inside function bodies where the declared
//! type is incompatible with the assigned value, specifically for `LiteralString`
//! and `Literal[...]` types.
//!
//! Covered cases:
//!
//! 1. Assigning a `Literal["X"]`-typed parameter to a `Literal["Y"]` variable
//!    where the literal values differ.
//! 2. Assigning an f-string containing non-`LiteralString` interpolations to
//!    a `LiteralString`-annotated variable.
//! 3. Assigning a generic parameterised with `str` where `LiteralString` is
//!    required (invariant generics like `list`, `Container`).
//! 4. Assigning a `list[LiteralString]` to `list[str]` — lists are invariant.
//!
//! ```python
//! def func(b: Literal["two"], non_literal: str):
//!     x1: Literal[""] = b                          # E — different literal values
//!     x2: LiteralString = f"{non_literal}"          # E — non-literal in f-string
//!     x3: Container[LiteralString] = Container(s)   # E — str ≠ LiteralString
//!     x4: list[str] = val                            # E — invariant mismatch
//! ```

use std::collections::HashMap;

use basilisk_resolver::{FunctionInfo, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0126",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0126",
};

/// Emits BSK-E0126 for `LiteralString` / `Literal[...]` assignment
/// incompatibilities found inside function bodies.
pub(crate) struct LiteralStringAssignment;

impl Rule for LiteralStringAssignment {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        for func in &module.functions {
            check_function_body(func, source, path, diagnostics);
        }
    }
}

/// Build a map from parameter name → annotation text for a function.
fn param_annotations<'a>(func: &'a FunctionInfo, source: &'a str) -> HashMap<&'a str, &'a str> {
    let mut map = HashMap::new();
    for param in func
        .parameters
        .iter()
        .chain(func.vararg.iter())
        .chain(func.kwarg.iter())
    {
        if let Some(ann_span) = param.annotation_span {
            if let Some(ann_text) = ann_span.slice_source(source) {
                let _ = map.insert(param.name.as_str(), ann_text.trim());
            }
        }
    }
    map
}

/// Locate the function body in the source by finding the `:` after the
/// signature and returning everything from the next line to the end of
/// the function (approximated by the next `def ` or `class ` at the same
/// indentation, or end of file).
fn function_body_range(func: &FunctionInfo, source: &str) -> Option<(usize, usize)> {
    // Start from the def keyword span.
    let def_start = usize::try_from(func.def_span.start).ok()?;

    // Find the colon that ends the function signature.
    let after_def = source.get(def_start..)?;
    let colon_pos = after_def
        .find(":\n")
        .or_else(|| after_def.find("):\n").map(|p| p + 1))?;
    let body_start = def_start + colon_pos + 1; // after ':'

    // Determine the indentation of the `def` line.
    let line_start = source.get(..def_start)?.rfind('\n').map_or(0, |p| p + 1);
    let def_indent = def_start - line_start;

    // Scan forward to find the end: a line at the same or lesser indentation
    // that starts with `def `, `class `, or `@` (decorator), or EOF.
    let mut body_end = source.len();
    let mut pos = body_start;
    let mut first_line = true;
    let body_source = source.get(body_start..)?;
    for line in body_source.lines() {
        if first_line {
            first_line = false;
            pos += line.len() + 1; // +1 for '\n'
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            pos += line.len() + 1;
            continue;
        }

        // Count leading spaces.
        let line_indent = line.len() - line.trim_start().len();
        if line_indent <= def_indent
            && (trimmed.starts_with("def ")
                || trimmed.starts_with("class ")
                || trimmed.starts_with('@'))
        {
            body_end = pos;
            break;
        }
        pos += line.len() + 1;
    }

    Some((body_start, body_end))
}

/// A parsed annotated local assignment: `name: annotation = rhs`.
struct LocalAssign<'a> {
    /// The annotation text (e.g. `Literal[""]`, `LiteralString`).
    annotation: &'a str,
    /// The right-hand side expression text (e.g. `b`, `f"{a} {non_literal}"`).
    rhs: &'a str,
    /// Byte offset into the source where the variable name starts.
    name_offset: usize,
    /// Length of the variable name.
    name_len: usize,
}

/// Parse annotated assignments from function body source text.
///
/// Looks for lines matching the pattern `name: Type = expr`.
fn parse_annotated_assigns(body: &str, body_offset: usize) -> Vec<LocalAssign<'_>> {
    let mut results = Vec::new();

    for line in body.lines() {
        let trimmed = line.trim();
        // Skip comments, empty lines, decorators, def/class, etc.
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("def ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with('@')
            || trimmed.starts_with("return ")
            || trimmed.starts_with("assert_type")
            || trimmed.starts_with("if ")
            || trimmed.starts_with("else")
            || trimmed.starts_with("for ")
            || trimmed.starts_with("while ")
        {
            continue;
        }

        // Look for `name: Type = expr` pattern.
        if let Some(assign) = parse_single_annotated_assign(trimmed, line, body, body_offset) {
            results.push(assign);
        }
    }

    results
}

/// Try to parse a single `name: Type = expr` from a trimmed line.
fn parse_single_annotated_assign<'a>(
    trimmed: &'a str,
    raw_line: &'a str,
    body: &'a str,
    body_offset: usize,
) -> Option<LocalAssign<'a>> {
    // Find `: ` — annotation separator.
    let colon_pos = trimmed.find(": ")?;
    let name = trimmed[..colon_pos].trim();

    // Name must be a simple identifier.
    if !is_simple_identifier(name) {
        return None;
    }

    let after_colon = &trimmed[colon_pos + 2..];

    // Find `=` at depth 0 (skip `=` inside brackets).
    let eq_pos = find_top_level_eq(after_colon)?;
    let annotation = after_colon[..eq_pos].trim();
    let rhs = after_colon[eq_pos + 1..].trim();

    // Strip trailing comments from RHS.
    let rhs = strip_trailing_comment(rhs);

    if annotation.is_empty() || rhs.is_empty() {
        return None;
    }

    // Calculate source offset for the name.
    #[allow(clippy::as_conversions)]
    let line_offset_in_body = raw_line.as_ptr() as usize - body.as_ptr() as usize;
    let name_start_in_line = raw_line.len() - raw_line.trim_start().len();
    let name_offset = body_offset + line_offset_in_body + name_start_in_line;

    Some(LocalAssign {
        annotation,
        rhs,
        name_offset,
        name_len: name.len(),
    })
}

/// Check if a string is a simple Python identifier.
fn is_simple_identifier(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap_or('0');
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

/// Find the position of `=` at bracket depth 0 in `s`.
/// Skips `==`, `!=`, `<=`, `>=`.
fn find_top_level_eq(source: &str) -> Option<usize> {
    let mut depth = 0i32;
    let bytes = source.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        match bytes[idx] {
            b'[' | b'(' | b'{' => depth += 1,
            b']' | b')' | b'}' => depth -= 1,
            b'=' if depth == 0 => {
                // Skip `==`
                if bytes.get(idx + 1) == Some(&b'=') {
                    idx += 2;
                    continue;
                }
                // Skip `!=`, `<=`, `>=` — the `=` is part of a comparison
                if idx > 0 && matches!(bytes[idx - 1], b'!' | b'<' | b'>') {
                    idx += 1;
                    continue;
                }
                return Some(idx);
            }
            _ => {}
        }
        idx += 1;
    }
    None
}

/// Strip trailing `# comment` from RHS text.
fn strip_trailing_comment(rhs: &str) -> &str {
    // Simple heuristic: find `#` not inside quotes.
    let mut in_single = false;
    let mut in_double = false;
    for (idx, ch) in rhs.char_indices() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '#' if !in_single && !in_double => return rhs[..idx].trim(),
            _ => {}
        }
    }
    rhs
}

/// Check all annotated local assignments in a function body for
/// `LiteralString` / `Literal[...]` violations.
fn check_function_body(
    func: &FunctionInfo,
    source: &str,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((body_start, body_end)) = function_body_range(func, source) else {
        return;
    };
    let Some(body) = source.get(body_start..body_end) else {
        return;
    };

    let param_anns = param_annotations(func, source);
    let assigns = parse_annotated_assigns(body, body_start);

    for assign in &assigns {
        check_literal_value_mismatch(assign, &param_anns, path, diagnostics);
        check_literal_string_fstring(assign, &param_anns, path, diagnostics);
        check_invariant_generic_literal_string(assign, &param_anns, path, diagnostics);
    }
}

/// Case 1: `x: Literal["X"] = param` where param is `Literal["Y"]` and X ≠ Y.
///
/// Also covers: `x: Literal[""] = param` where param is `Literal["two"]`.
fn check_literal_value_mismatch(
    assign: &LocalAssign<'_>,
    param_anns: &HashMap<&str, &str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Annotation must be Literal["..."] (string literal).
    let Some(target_value) = extract_literal_string_value(assign.annotation) else {
        return;
    };

    // RHS must be a simple name referring to a parameter.
    let rhs_name = assign.rhs.trim();
    if !is_simple_identifier(rhs_name) {
        return;
    }

    let Some(param_ann) = param_anns.get(rhs_name) else {
        return;
    };

    // Parameter must also be Literal["..."].
    let Some(source_value) = extract_literal_string_value(param_ann) else {
        return;
    };

    if target_value != source_value {
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Cannot assign `Literal[\"{source_value}\"]` to `Literal[\"{target_value}\"]` \
                 — literal values are incompatible"
            ),
            span: Span {
                start: u32::try_from(assign.name_offset).unwrap_or(u32::MAX),
                end: u32::try_from(assign.name_offset + assign.name_len).unwrap_or(u32::MAX),
            },
            path: path.to_owned(),
            help: Some(format!(
                "The variable expects exactly `Literal[\"{target_value}\"]`, \
                 but the parameter has type `Literal[\"{source_value}\"]`"
            )),
            note: Some(
                "PEP 586: Literal types are only compatible when their values match exactly"
                    .to_owned(),
            ),
        });
    }
}

/// Case 2: `x: LiteralString = f"... {non_literal} ..."` where an
/// interpolated variable has type `str` (not `LiteralString`).
fn check_literal_string_fstring(
    assign: &LocalAssign<'_>,
    param_anns: &HashMap<&str, &str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Annotation must be LiteralString.
    if assign.annotation.trim() != "LiteralString" {
        return;
    }

    // RHS must be an f-string.
    let rhs = assign.rhs.trim();
    if !rhs.starts_with("f\"") && !rhs.starts_with("f'") {
        return;
    }

    // Extract interpolated names from f-string: `{name}` patterns.
    let interpolated = extract_fstring_names(rhs);

    // If any interpolated name is a parameter with type `str` (not
    // `LiteralString` and not `Literal[...]`), emit a diagnostic.
    for name in &interpolated {
        if let Some(param_ann) = param_anns.get(name.as_str()) {
            if is_plain_str_type(param_ann) {
                diagnostics.push(Diagnostic {
                    code: CODE.clone(),
                    severity: Severity::Error,
                    message: format!(
                        "Cannot assign f-string to `LiteralString` — interpolated variable \
                         `{name}` has type `{param_ann}`, which is not `LiteralString`"
                    ),
                    span: Span {
                        start: u32::try_from(assign.name_offset).unwrap_or(u32::MAX),
                        end: u32::try_from(assign.name_offset + assign.name_len)
                            .unwrap_or(u32::MAX),
                    },
                    path: path.to_owned(),
                    help: Some(format!(
                        "Change `{name}` to `LiteralString` or use `str` as the target type"
                    )),
                    note: Some(
                        "PEP 675: an f-string is `LiteralString` only if all interpolated \
                         expressions are compatible with `LiteralString`"
                            .to_owned(),
                    ),
                });
                return; // one diagnostic per assignment is enough
            }
        }
    }
}

/// Case 3 & 4: Invariant generic mismatches involving `LiteralString`.
///
/// - `x: Container[LiteralString] = Container(s)` where `s: str`
/// - `x: list[str] = val` where `val: list[LiteralString]`
fn check_invariant_generic_literal_string(
    assign: &LocalAssign<'_>,
    param_anns: &HashMap<&str, &str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ann = assign.annotation.trim();
    let rhs = assign.rhs.trim();

    // Case 4: `list[str] = param` where param is `list[LiteralString]`.
    // Lists (and other mutable containers) are invariant, so
    // `list[LiteralString]` is NOT assignable to `list[str]`.
    if let Some((ann_container, ann_inner)) = split_generic(ann) {
        if is_invariant_container(ann_container)
            && is_plain_str_type(ann_inner)
            && is_simple_identifier(rhs)
        {
            if let Some(param_ann) = param_anns.get(rhs) {
                if let Some((param_container, param_inner)) = split_generic(param_ann) {
                    if param_container == ann_container && param_inner.trim() == "LiteralString" {
                        diagnostics.push(Diagnostic {
                            code: CODE.clone(),
                            severity: Severity::Error,
                            message: format!(
                                "Cannot assign `{param_ann}` to `{ann}` — \
                                 `{ann_container}` is invariant in its type parameter"
                            ),
                            span: Span {
                                start: u32::try_from(assign.name_offset).unwrap_or(u32::MAX),
                                end: u32::try_from(assign.name_offset + assign.name_len)
                                    .unwrap_or(u32::MAX),
                            },
                            path: path.to_owned(),
                            help: Some(format!(
                                "Use `Sequence[str]` (covariant) instead of `{ann}` if you \
                                 need to accept `{param_ann}`"
                            )),
                            note: Some(
                                "PEP 484: mutable generic containers like `list` are invariant — \
                                 `list[LiteralString]` is not a subtype of `list[str]`"
                                    .to_owned(),
                            ),
                        });
                        return;
                    }
                }
            }
        }
    }

    // Case 3: `Container[LiteralString] = Container(s)` where `s: str`.
    // The RHS is a constructor call like `Container(s)`, and `s` has type `str`
    // which is not assignable to `LiteralString`.
    if let Some((_ann_container, ann_inner)) = split_generic(ann) {
        if ann_inner.trim() == "LiteralString" {
            // RHS is a call like `SomeClass(arg)`.
            if let Some((callee, call_args)) = parse_simple_call(rhs) {
                // Check if any argument is a parameter with `str` type.
                let _ = callee; // we only care about the args
                for arg in &call_args {
                    if let Some(param_ann) = param_anns.get(arg.as_str()) {
                        if is_plain_str_type(param_ann) {
                            diagnostics.push(Diagnostic {
                                code: CODE.clone(),
                                severity: Severity::Error,
                                message: format!(
                                    "Cannot assign `{rhs}` to `{ann}` — argument `{arg}` \
                                     has type `{param_ann}`, not `LiteralString`"
                                ),
                                span: Span {
                                    start: u32::try_from(assign.name_offset).unwrap_or(u32::MAX),
                                    end: u32::try_from(assign.name_offset + assign.name_len).unwrap_or(u32::MAX),
                                },
                                path: path.to_owned(),
                                help: Some(format!(
                                    "Change `{arg}` to `LiteralString` or relax the target annotation"
                                )),
                                note: Some(
                                    "PEP 675: `str` is not assignable to `LiteralString` — \
                                     `LiteralString` is a strict subtype of `str`"
                                        .to_owned(),
                                ),
                            });
                            return;
                        }
                    }
                }
            }
        }
    }
}

/// Extract the string value from a `Literal["value"]` annotation.
/// Returns `None` if the annotation is not of this form.
fn extract_literal_string_value(ann: &str) -> Option<&str> {
    let ann = ann.trim();
    let inner = ann.strip_prefix("Literal[")?;
    let inner = inner.strip_suffix(']')?;
    let inner = inner.trim();

    // Must be a quoted string: "..." or '...'
    if (inner.starts_with('"') && inner.ends_with('"'))
        || (inner.starts_with('\'') && inner.ends_with('\''))
    {
        Some(&inner[1..inner.len() - 1])
    } else {
        None
    }
}

/// Extract interpolated variable names from an f-string.
/// For `f"{a} {non_literal}"`, returns `["a", "non_literal"]`.
fn extract_fstring_names(fstring: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut chars = fstring.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            // Collect until '}' or non-identifier char.
            let mut name = String::new();
            for inner in chars.by_ref() {
                if inner == '}' || inner == '!' || inner == ':' || inner == '.' {
                    break;
                }
                name.push(inner);
            }
            let name = name.trim().to_owned();
            if !name.is_empty() && is_simple_identifier(&name) {
                names.push(name);
            }
        }
    }
    names
}

/// Returns `true` when the annotation text is plain `str` (not `LiteralString`,
/// not `Literal[...]`, not a generic like `list[str]`).
fn is_plain_str_type(ann: &str) -> bool {
    let ann = ann.trim();
    ann == "str"
}

/// Split a generic annotation like `list[str]` into `("list", "str")`.
/// Returns `None` for non-generic annotations.
fn split_generic(ann: &str) -> Option<(&str, &str)> {
    let ann = ann.trim();
    let bracket = ann.find('[')?;
    let container = ann[..bracket].trim();
    let rest = &ann[bracket + 1..];
    let inner = rest.strip_suffix(']')?.trim();
    Some((container, inner))
}

/// Returns `true` for invariant container types (mutable generics).
fn is_invariant_container(name: &str) -> bool {
    matches!(
        name,
        "list" | "List" | "dict" | "Dict" | "set" | "Set" | "deque" | "Deque"
    )
}

/// Parse a simple function call `Name(arg1, arg2)` into the callee name
/// and the list of argument names.
fn parse_simple_call(expr: &str) -> Option<(&str, Vec<String>)> {
    let paren = expr.find('(')?;
    let callee = expr[..paren].trim();
    if !is_simple_identifier(callee) {
        return None;
    }
    let rest = expr[paren + 1..].strip_suffix(')')?.trim();
    let args: Vec<String> = rest
        .split(',')
        .map(|a| a.trim().to_owned())
        .filter(|a| !a.is_empty())
        .collect();
    Some((callee, args))
}
