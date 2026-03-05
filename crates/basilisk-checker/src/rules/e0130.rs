//! BSK-E0130: `TypeVar` scoping violation.
//!
//! Detects uses of `TypeVar` instances outside their valid scope:
//!
//! 1. A nested class inside a generic class using the outer class's `TypeVar`
//!    in its base classes or body (the outer class's type params don't cover
//!    the inner class scope).
//! 2. A class nested inside a generic function re-using the function's `TypeVar`
//!    in `Generic[...]`.
//! 3. A `TypeVar` used in a module-level expression (subscript call like `list[T]()`).
//!
//! Per PEP 484: "A generic class nested in another generic class cannot use
//! the same type variables."

use std::collections::HashSet;

use basilisk_resolver::{ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0130",
    docs_url: "https://basilisk-lang.org/errors/BSK-E0130",
};

/// Emits BSK-E0130 for `TypeVar` scoping violations.
pub(crate) struct TypeVarScopeViolation;

/// Represents a scope (class or function) with its indentation level and bound `TypeVars`.
struct ScopeInfo {
    /// Indentation column of the `class`/`def` keyword.
    indent: usize,
    /// `TypeVar` names bound by this scope (from `Generic[...]` params or function annotations).
    bound_typevars: HashSet<String>,
    /// Whether this is a class scope (vs function scope).
    is_class: bool,
}

/// Check if `name` appears as a whole identifier in `text` (not as part of a longer name).
fn contains_typevar_reference(text: &str, typevar_name: &str) -> bool {
    let needle = typevar_name.as_bytes();
    let haystack = text.as_bytes();
    let needle_len = needle.len();

    if needle_len > haystack.len() {
        return false;
    }

    haystack
        .windows(needle_len)
        .enumerate()
        .any(|(idx, window)| {
            if window != needle {
                return false;
            }
            let before_ok = idx == 0
                || (!haystack[idx - 1].is_ascii_alphanumeric()
                    && haystack[idx - 1] != b'_');
            let after_ok = idx + needle_len >= haystack.len()
                || (!haystack[idx + needle_len].is_ascii_alphanumeric()
                    && haystack[idx + needle_len] != b'_');
            before_ok && after_ok
        })
}

/// Extract `TypeVar` names from a `Generic[T, S, ...]` or similar parameterized base.
fn extract_typevars_from_generic_base(line: &str) -> HashSet<String> {
    let mut result = HashSet::new();
    if let Some(start) = line.find("Generic[") {
        let after = &line[start + 8..];
        if let Some(end) = after.find(']') {
            let params = &after[..end];
            for param in params.split(',') {
                let trimmed = param.trim();
                if !trimmed.is_empty()
                    && trimmed
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    result.insert(trimmed.to_owned());
                }
            }
        }
    }
    result
}

/// Extract `TypeVar` names referenced in function parameter annotations and return type.
fn extract_typevars_from_function_sig(
    line: &str,
    all_typevars: &HashSet<String>,
) -> HashSet<String> {
    let mut result = HashSet::new();
    for typevar_name in all_typevars {
        if contains_typevar_reference(line, typevar_name) {
            result.insert(typevar_name.clone());
        }
    }
    result
}

/// Compute the leading whitespace count of a line.
fn leading_indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Find the byte offset of a given 1-based line number in source text.
#[allow(clippy::cast_possible_truncation)]
fn line_to_byte_offset(source: &str, target_line: usize) -> u32 {
    let mut current_line = 1usize;
    for (byte_idx, ch) in source.char_indices() {
        if current_line == target_line {
            return byte_idx as u32;
        }
        if ch == '\n' {
            current_line += 1;
        }
    }
    source.len() as u32
}

/// Build a span covering the trimmed content of the given 1-based line.
#[allow(clippy::cast_possible_truncation)]
fn span_for_line(source: &str, line_number: usize) -> Span {
    let start = line_to_byte_offset(source, line_number) as usize;
    let line_text = source[start..]
        .lines()
        .next()
        .unwrap_or("");
    let trimmed_start = start + (line_text.len() - line_text.trim_start().len());
    let trimmed_end = start + line_text.trim_end().len();
    Span {
        start: trimmed_start as u32,
        end: trimmed_end as u32,
    }
}

impl Rule for TypeVarScopeViolation {
    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)]
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let all_typevars: HashSet<String> = module
            .typevar_calls
            .iter()
            .map(|tv| tv.name.clone())
            .collect();

        if all_typevars.is_empty() {
            return;
        }

        let lines: Vec<&str> = module.source.lines().collect();

        // Track scope stack: each entry has (indent, bound_typevars, is_class).
        let mut scope_stack: Vec<ScopeInfo> = Vec::new();

        for (line_idx, line) in lines.iter().enumerate() {
            let line_number = line_idx + 1;
            let trimmed = line.trim();

            // Skip empty lines, comments, and pure string lines.
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let indent = leading_indent(line);

            // Pop scopes that are no longer active (indentation decreased).
            while let Some(top) = scope_stack.last() {
                if indent <= top.indent {
                    scope_stack.pop();
                } else {
                    break;
                }
            }

            // Detect class definitions.
            if trimmed.starts_with("class ") {
                let bound_tvs = extract_typevars_from_generic_base(trimmed);

                // Check: does this class's base reference a TypeVar from an outer scope?
                let outer_bound: HashSet<String> = scope_stack
                    .iter()
                    .flat_map(|scope| scope.bound_typevars.iter().cloned())
                    .collect();

                if !outer_bound.is_empty() {
                    // Check if any outer TypeVar is used in the base classes.
                    // Extract the base class portion: everything between `(` and `)`.
                    if let Some(paren_start) = trimmed.find('(') {
                        if let Some(paren_end) = trimmed.rfind(')') {
                            let bases_text = &trimmed[paren_start + 1..paren_end];
                            for typevar_name in &outer_bound {
                                if contains_typevar_reference(bases_text, typevar_name) {
                                    diagnostics.push(Diagnostic {
                                        code: CODE.clone(),
                                        severity: Severity::Error,
                                        message: format!(
                                            "TypeVar `{typevar_name}` from outer scope \
                                             cannot be used in nested class definition"
                                        ),
                                        span: span_for_line(&module.source, line_number),
                                        path: module.path.clone(),
                                        help: Some(
                                            "Use a different TypeVar for the inner class, \
                                             or restructure to avoid nesting"
                                                .to_owned(),
                                        ),
                                        note: Some(
                                            "PEP 484: the scope of type variables of the \
                                             outer class doesn't cover the inner one"
                                                .to_owned(),
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }

                scope_stack.push(ScopeInfo {
                    indent,
                    bound_typevars: bound_tvs,
                    is_class: true,
                });
                continue;
            }

            // Detect function definitions.
            if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
                let bound_tvs =
                    extract_typevars_from_function_sig(trimmed, &all_typevars);
                scope_stack.push(ScopeInfo {
                    indent,
                    bound_typevars: bound_tvs,
                    is_class: false,
                });
                continue;
            }

            // For lines inside a nested class body (inner class of a generic class),
            // check if annotations reference outer class TypeVars.
            // This only applies when there are at least 2 class scopes on the stack
            // (outer generic class + inner class).
            {
                let class_scopes: Vec<&ScopeInfo> = scope_stack
                    .iter()
                    .filter(|scope| scope.is_class)
                    .collect();

                if class_scopes.len() >= 2 {
                    // Collect TypeVars from all outer class scopes (all but the last).
                    let outer_class_tvs: HashSet<String> = class_scopes
                        [..class_scopes.len() - 1]
                        .iter()
                        .flat_map(|scope| scope.bound_typevars.iter().cloned())
                        .collect();

                    let innermost_tvs = &class_scopes
                        .last()
                        .map_or_else(HashSet::new, |scope| scope.bound_typevars.clone());

                    let forbidden_tvs: HashSet<&String> = outer_class_tvs
                        .iter()
                        .filter(|tv| !innermost_tvs.contains(*tv))
                        .collect();

                    if !forbidden_tvs.is_empty() && trimmed.contains(':') {
                        // This is an annotation line — check for forbidden TypeVar refs.
                        let annotation_part = trimmed
                            .split_once(':')
                            .map_or(trimmed, |(_, rhs)| rhs);
                        for typevar_name in &forbidden_tvs {
                            if contains_typevar_reference(annotation_part, typevar_name)
                            {
                                diagnostics.push(Diagnostic {
                                    code: CODE.clone(),
                                    severity: Severity::Error,
                                    message: format!(
                                        "TypeVar `{typevar_name}` from outer class \
                                         cannot be used in nested class body"
                                    ),
                                    span: span_for_line(&module.source, line_number),
                                    path: module.path.clone(),
                                    help: Some(
                                        "Use a different TypeVar for the inner class"
                                            .to_owned(),
                                    ),
                                    note: Some(
                                        "PEP 484: the scope of type variables of the \
                                         outer class doesn't cover the inner one"
                                            .to_owned(),
                                    ),
                                });
                            }
                        }
                    }
                }
            }

            // Check for unbound TypeVar usage in function bodies and class
            // attribute annotations. A TypeVar is "unbound" if no enclosing
            // scope binds it (via Generic[...] or function signature).
            if !scope_stack.is_empty() && trimmed.contains(':') {
                // Collect all TypeVars bound by any enclosing scope.
                let all_bound: HashSet<&String> = scope_stack
                    .iter()
                    .flat_map(|scope| scope.bound_typevars.iter())
                    .collect();

                let annotation_part = trimmed
                    .split_once(':')
                    .map_or(trimmed, |(_, rhs)| rhs);
                let before_comment = annotation_part
                    .split_once('#')
                    .map_or(annotation_part, |(code, _)| code);

                for typevar_name in &all_typevars {
                    if !all_bound.contains(typevar_name)
                        && contains_typevar_reference(before_comment, typevar_name)
                    {
                        // Skip if this is a function def line (already handles its own
                        // TypeVars via scope creation).
                        let is_def_line = trimmed.starts_with("def ")
                            || trimmed.starts_with("async def ");
                        if !is_def_line {
                            diagnostics.push(Diagnostic {
                                code: CODE.clone(),
                                severity: Severity::Error,
                                message: format!(
                                    "TypeVar `{typevar_name}` is not bound in \
                                     this scope"
                                ),
                                span: span_for_line(&module.source, line_number),
                                path: module.path.clone(),
                                help: Some(
                                    "TypeVars can only be used where they are \
                                     bound by a Generic[...] base or function signature"
                                        .to_owned(),
                                ),
                                note: Some(
                                    "PEP 484: unbound type variables should not \
                                     appear in function or class bodies"
                                        .to_owned(),
                                ),
                            });
                        }
                    }
                }
            }

            // TypeAlias inside a class body: class TypeVars are not in scope
            // for type alias definitions. `alias: TypeAlias = list[T]` is invalid
            // when T comes from the enclosing class's Generic[T].
            {
                let class_scopes: Vec<&ScopeInfo> = scope_stack
                    .iter()
                    .filter(|scope| scope.is_class)
                    .collect();

                if class_scopes.len() == 1 && trimmed.contains("TypeAlias") {
                    let enclosing_tvs = &class_scopes[0].bound_typevars;
                    if !enclosing_tvs.is_empty() {
                        // Check the RHS of the TypeAlias assignment for TypeVar refs.
                        let rhs_part = trimmed
                            .split_once('=')
                            .map_or("", |(_, rhs)| rhs);
                        for typevar_name in enclosing_tvs {
                            if contains_typevar_reference(rhs_part, typevar_name) {
                                diagnostics.push(Diagnostic {
                                    code: CODE.clone(),
                                    severity: Severity::Error,
                                    message: format!(
                                        "TypeVar `{typevar_name}` from enclosing class \
                                         is not accessible in a TypeAlias definition"
                                    ),
                                    span: span_for_line(&module.source, line_number),
                                    path: module.path.clone(),
                                    help: Some(
                                        "Type aliases in class bodies cannot reference \
                                         the class's type parameters"
                                            .to_owned(),
                                    ),
                                    note: Some(
                                        "PEP 484: TypeAlias creates its own scope and \
                                         cannot capture class-level TypeVars"
                                            .to_owned(),
                                    ),
                                });
                            }
                        }
                    }
                }
            }

            // Module-level (indent == 0, no enclosing scope): check for TypeVar
            // subscript expressions like `list[T]()`.
            if indent == 0 && scope_stack.is_empty() {
                // Skip class/def definitions, imports, assignments with annotations
                // (those are already handled by other checks or are valid).
                let dominated_by_other =
                    trimmed.starts_with("class ")
                        || trimmed.starts_with("def ")
                        || trimmed.starts_with("import ")
                        || trimmed.starts_with("from ")
                        || trimmed.starts_with('@');

                if !dominated_by_other {
                    let before_comment = trimmed
                        .split_once('#')
                        .map_or(trimmed, |(code, _)| code);
                    for typevar_name in &all_typevars {
                        // Check for TypeVar in annotations (x: T, x: list[T] = ...)
                        // and in expressions (list[T]()).
                        if contains_typevar_reference(before_comment, typevar_name) {
                            // Skip lines that are TypeVar definitions themselves
                            // (e.g. `T = TypeVar('T')`)
                            let is_typevar_def = before_comment
                                .contains("TypeVar");
                            if !is_typevar_def {
                                diagnostics.push(Diagnostic {
                                    code: CODE.clone(),
                                    severity: Severity::Error,
                                    message: format!(
                                        "TypeVar `{typevar_name}` is not bound in \
                                         this scope"
                                    ),
                                    span: span_for_line(&module.source, line_number),
                                    path: module.path.clone(),
                                    help: Some(
                                        "TypeVars can only be used inside generic \
                                         functions or classes that bind them"
                                            .to_owned(),
                                    ),
                                    note: Some(
                                        "PEP 484: unbound type variables should not \
                                         appear at module scope"
                                            .to_owned(),
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}
