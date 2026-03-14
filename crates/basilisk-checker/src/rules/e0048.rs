//! BSK-E0048: Invalid right-hand side for a `TypeAlias` annotation.
//!
//! PEP 613 requires that the RHS of an explicit `TypeAlias` annotation must be
//! a valid type expression. The following are errors:
//!
//! - List literals: `x: TypeAlias = [int, str]`
//! - Tuple literals: `x: TypeAlias = ((int, str),)`
//! - Dict literals: `x: TypeAlias = {"a": "b"}`
//! - List comprehensions: `x: TypeAlias = [int for i in range(1)]`
//! - Lambda calls: `x: TypeAlias = (lambda: int)()`
//! - Conditional expressions: `x: TypeAlias = int if cond else str`
//! - Boolean literals: `x: TypeAlias = True`
//! - Integer literals: `x: TypeAlias = 1`
//! - Binary boolean operators: `x: TypeAlias = list or set`
//! - F-strings: `x: TypeAlias = f"..."`
//! - Subscript-into-subscript: `x: TypeAlias = [int][0]`
//! - Runtime calls: `x: TypeAlias = eval("int")`
//!
//! ```python
//! from typing import TypeAlias
//! BadTypeAlias2: TypeAlias = [int, str]   # E — list literal
//! BadTypeAlias10: TypeAlias = True         # E — bool literal
//! ```

use basilisk_resolver::{ImportKind, ResolvedModule, Span};

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};
use crate::span_util::slice_span;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0048",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0048",
};

fn span_text(source: &str, span: Option<Span>) -> Option<&str> {
    slice_span(source, span?)
}

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    Diagnostic {
        code: CODE.clone(),
        severity: Severity::Error,
        message,
        span,
        path: path.to_owned(),
        help: Some(
            "The RHS of a `TypeAlias` annotation must be a valid type expression".to_owned(),
        ),
        note: Some(
            "PEP 613: `x: TypeAlias = T` requires T to be a type, not a literal or expression"
                .to_owned(),
        ),
    }
}

/// Collect all local names that refer to `typing.TypeAlias` in this module.
///
/// Handles:
/// - `from typing import TypeAlias`
/// - `from typing import TypeAlias as TA`
/// - `import typing` (used as `typing.TypeAlias`)
fn collect_type_alias_names(module: &ResolvedModule) -> Vec<String> {
    let mut names = vec!["TypeAlias".to_owned()];
    for import in &module.imports {
        if import.kind != ImportKind::From {
            continue;
        }
        if import.module != "typing" && import.module != "typing_extensions" {
            continue;
        }
        // Scan the raw import source text for `TypeAlias as <alias>` patterns.
        let Some(import_text) = slice_span(&module.source, import.span) else {
            continue;
        };
        // Find all occurrences of `TypeAlias as <identifier>`
        let mut search = import_text;
        while let Some(pos) = search.find("TypeAlias as ") {
            let after = &search[pos + "TypeAlias as ".len()..];
            // Extract the identifier following `TypeAlias as `
            let alias: String = after
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !alias.is_empty() && alias != "TypeAlias" {
                names.push(alias);
            }
            search = &search[pos + 1..];
        }
    }
    names
}

/// Returns `true` when the annotation text matches one of the known `TypeAlias` names.
fn is_type_alias_annotation(ann: &str, type_alias_names: &[String]) -> bool {
    let ann = ann.trim();
    type_alias_names.iter().any(|n| ann == n) || ann.ends_with(".TypeAlias")
}

/// Returns `true` when the RHS text is an invalid type expression for a `TypeAlias`.
fn is_invalid_rhs(rhs: &str) -> bool {
    let rhs = rhs.trim();

    // Boolean literals
    if rhs == "True" || rhs == "False" {
        return true;
    }

    // Integer or float literals: starts with a digit
    if rhs.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return true;
    }

    // Negative numeric literals
    if rhs.starts_with('-')
        && rhs[1..]
            .trim()
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
    {
        return true;
    }

    // F-string
    if rhs.starts_with("f\"") || rhs.starts_with("f'") {
        return true;
    }

    // List literal (starts with `[`) — also catches list comprehensions
    if rhs.starts_with('[') {
        return true;
    }

    // Dict literal
    if rhs.starts_with('{') {
        return true;
    }

    // Tuple literal: starts with `(` and has a comma at depth 0 inside
    if rhs.starts_with('(') && paren_has_top_level_comma(rhs) {
        return true;
    }

    // Conditional expression: has ` if ` at depth 0
    if has_top_level_token(rhs, " if ") {
        return true;
    }

    // Boolean binary operator `or` / `and` at depth 0
    if has_top_level_token(rhs, " or ") || has_top_level_token(rhs, " and ") {
        return true;
    }

    // Lambda (possibly called)
    if rhs.contains("lambda") {
        return true;
    }

    // Runtime call: eval(...)
    if rhs.starts_with("eval(") {
        return true;
    }

    false
}

/// Returns `true` when the text contains `token` at bracket depth 0.
fn has_top_level_token(s: &str, token: &str) -> bool {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let tok = token.as_bytes();
    let tok_len = tok.len();
    let mut i = 0;
    while i < bytes.len() {
        match bytes.get(i).copied() {
            Some(b'[' | b'(' | b'{') => depth += 1,
            Some(b']' | b')' | b'}') => depth -= 1,
            Some(_) if depth == 0 => {
                if bytes.get(i..i + tok_len) == Some(tok) {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// Returns `true` when `(...)` contains a comma at depth 0 inside the parens.
fn paren_has_top_level_comma(s: &str) -> bool {
    if s.len() < 2 {
        return false;
    }
    let inner = &s[1..s.len() - 1];
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

/// Emits BSK-E0048 when a `TypeAlias`-annotated variable has an invalid RHS type expression.
pub(crate) struct TypeAliasInvalidRhs;

impl Rule for TypeAliasInvalidRhs {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;
        let type_alias_names = collect_type_alias_names(module);

        // Collect names of module-level variables that hold runtime values
        // (not types). These cannot be used as `TypeAlias` RHS values.
        let runtime_var_names = collect_runtime_var_names(module);

        for var in &module.module_vars {
            let Some(ann) = span_text(source, var.annotation_span) else {
                continue;
            };
            if !is_type_alias_annotation(ann.trim(), &type_alias_names) {
                continue;
            }
            let Some(rhs_span) = var.rhs_span else {
                continue;
            };
            let Some(rhs) = span_text(source, Some(rhs_span)) else {
                continue;
            };
            let rhs_trimmed = rhs.trim();
            if is_invalid_rhs(rhs_trimmed)
                || is_runtime_variable_ref(rhs_trimmed, &runtime_var_names)
            {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Invalid type expression as right-hand side of `TypeAlias` for `{}`",
                        var.name
                    ),
                    var.name_span,
                    path,
                ));
            }
        }

        // Check type alias parameterization (wrong number of type args)
        let alias_map = build_alias_info_map(module, &type_alias_names);
        check_alias_parameterization(module, &alias_map, diagnostics);
    }
}

/// Collect names of module-level variables that hold runtime values (not types).
///
/// A name is considered a runtime variable if its RHS is a literal value
/// (int, str, float, bool, etc.) or a call expression that isn't a type
/// constructor. This detects patterns like `var1 = 3` so that
/// `BadAlias: TypeAlias = var1` can be flagged.
fn collect_runtime_var_names(module: &ResolvedModule) -> std::collections::HashSet<String> {
    use basilisk_resolver::RhsKind;
    let mut names = std::collections::HashSet::new();
    for var in &module.module_vars {
        match &var.rhs_kind {
            RhsKind::IntLiteral
            | RhsKind::FloatLiteral
            | RhsKind::StrLiteral
            | RhsKind::BoolLiteral
            | RhsKind::BytesLiteral
            | RhsKind::NoneValue
            | RhsKind::EmptyList
            | RhsKind::EmptyDict
            | RhsKind::Lambda => {
                let _ = names.insert(var.name.clone());
            }
            RhsKind::List(_) | RhsKind::Dict(_) | RhsKind::Set(_) | RhsKind::Tuple(_) => {
                let _ = names.insert(var.name.clone());
            }
            _ => {}
        }
    }
    names
}

/// Returns `true` when the RHS is a simple name reference to a runtime variable.
fn is_runtime_variable_ref(
    rhs: &str,
    runtime_var_names: &std::collections::HashSet<String>,
) -> bool {
    // Only match simple identifiers (no brackets, dots, pipes)
    if rhs.contains(['[', ']', '|', '.', '(', ')']) {
        return false;
    }
    runtime_var_names.contains(rhs)
}

// ---------------------------------------------------------------------------
// Type alias parameterization checking
// ---------------------------------------------------------------------------

/// Information about a type alias definition.
struct AliasInfo {
    /// Number of `TypeVar`/`ParamSpec`/`TypeVarTuple` parameters the alias accepts.
    typevar_count: usize,
    /// Whether the alias RHS contains a top-level `|` (union alias).
    is_union: bool,
}

/// Build a map from alias name to its `AliasInfo`.
///
/// Scans `TypeAlias`-annotated module variables, counts how many distinct
/// `TypeVar` names appear in their RHS, and detects union aliases.
fn build_alias_info_map(
    module: &ResolvedModule,
    type_alias_names: &[String],
) -> std::collections::HashMap<String, AliasInfo> {
    let source = &module.source;

    // Collect known TypeVar/ParamSpec/TypeVarTuple names
    let typevar_names: std::collections::HashSet<&str> = module
        .typevar_calls
        .iter()
        .map(|tv| tv.name.as_str())
        .collect();

    let mut map = std::collections::HashMap::new();

    for var in &module.module_vars {
        let Some(ann) = span_text(source, var.annotation_span) else {
            continue;
        };
        if !is_type_alias_annotation(ann.trim(), type_alias_names) {
            continue;
        }
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };
        let Some(rhs) = span_text(source, Some(rhs_span)) else {
            continue;
        };
        let rhs = rhs.trim();

        // Count unique TypeVar references in the RHS
        let typevar_count = count_typevar_refs(rhs, &typevar_names);

        let is_union = has_top_level_token(rhs, " | ");
        let _ = map.insert(var.name.clone(), AliasInfo { typevar_count, is_union });
    }

    // Also handle implicit aliases (no TypeAlias annotation):
    // `ListAlias = list` or `ListOrSetAlias = list | set`
    for var in &module.module_vars {
        if var.has_annotation {
            continue; // Already handled above or not an alias
        }
        if map.contains_key(&var.name) {
            continue;
        }
        // Heuristic: if the name starts with uppercase and the RHS is a type
        // expression (name, subscript, or union), treat it as an implicit alias
        let first_char = var.name.chars().next().unwrap_or('a');
        if !first_char.is_ascii_uppercase() {
            continue;
        }
        let Some(rhs_span) = var.rhs_span else {
            continue;
        };
        let Some(rhs) = span_text(source, Some(rhs_span)) else {
            continue;
        };
        let rhs = rhs.trim();

        // Only treat simple type-expression-like RHS as aliases
        if matches!(var.rhs_kind, basilisk_resolver::RhsKind::Other)
            && looks_like_type_expression(rhs)
        {
            let typevar_count = count_typevar_refs(rhs, &typevar_names);
            let is_union = has_top_level_token(rhs, " | ");
        let _ = map.insert(var.name.clone(), AliasInfo { typevar_count, is_union });
        }
    }

    map
}

/// Count unique `TypeVar` name references in a type expression string.
fn count_typevar_refs(rhs: &str, typevar_names: &std::collections::HashSet<&str>) -> usize {
    let mut seen = std::collections::HashSet::new();
    // Split on non-identifier chars to extract names
    let mut current = String::new();
    for ch in rhs.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            if !current.is_empty() && typevar_names.contains(current.as_str()) {
                let _ = seen.insert(current.clone());
            }
            current.clear();
        }
    }
    if !current.is_empty() && typevar_names.contains(current.as_str()) {
        let _ = seen.insert(current);
    }
    seen.len()
}

/// Returns `true` if the text looks like a type expression (for implicit alias detection).
fn looks_like_type_expression(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() {
        return false;
    }
    // Must start with an uppercase letter or be a builtin type
    let first = text.chars().next().unwrap_or(' ');
    if !first.is_ascii_uppercase() && !first.is_ascii_lowercase() {
        return false;
    }
    // Must not contain invalid chars for type expressions
    !text.contains(['=', '+', '-', '*', '/', '%', '!', '~', '^', '&', '{', '}'])
}

/// Count type arguments in a `Name[arg1, arg2, ...]` annotation.
///
/// Returns `None` if the annotation is not a subscript.
fn count_type_args(annotation: &str) -> Option<usize> {
    let bracket_start = annotation.find('[')?;
    if !annotation.ends_with(']') {
        return None;
    }
    let inner = &annotation[bracket_start + 1..annotation.len() - 1];
    if inner.trim().is_empty() {
        return Some(0);
    }
    // Count top-level commas
    let mut depth = 0i32;
    let mut count = 1usize;
    for ch in inner.chars() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    Some(count)
}

/// Extract the base name from an annotation like `Name[args]`.
fn annotation_base_name(annotation: &str) -> &str {
    annotation.split('[').next().unwrap_or(annotation).trim()
}

/// Check type alias parameterization across all function parameter annotations.
fn check_alias_parameterization(
    module: &ResolvedModule,
    alias_map: &std::collections::HashMap<String, AliasInfo>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let source = &module.source;
    let path = &module.path;

    // Check function parameter annotations
    for func in &module.functions {
        for param in &func.parameters {
            if !param.has_annotation {
                continue;
            }
            let Some(ann_span) = param.annotation_span else {
                continue;
            };
            let Some(ann_text) = slice_span(source, ann_span) else {
                continue;
            };
            let ann_text = ann_text.trim();
            check_single_annotation(ann_text, ann_span, alias_map, path, diagnostics);
        }
    }

    // Check module-level variable annotations
    for var in &module.module_vars {
        let Some(ann_span) = var.annotation_span else {
            continue;
        };
        let Some(ann_text) = slice_span(source, ann_span) else {
            continue;
        };
        let ann_text = ann_text.trim();
        check_single_annotation(ann_text, ann_span, alias_map, path, diagnostics);
    }
}

/// Check a single annotation for alias parameterization errors.
fn check_single_annotation(
    ann_text: &str,
    ann_span: Span,
    alias_map: &std::collections::HashMap<String, AliasInfo>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let base = annotation_base_name(ann_text);
    let Some(info) = alias_map.get(base) else {
        return;
    };

    // Check if annotation uses `[...]` subscript
    if let Some(arg_count) = count_type_args(ann_text) {
        if info.typevar_count == 0 {
            // Alias is not generic — cannot be parameterized
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!("Type alias `{base}` is not generic and cannot be parameterized"),
                span: ann_span,
                path: path.to_owned(),
                help: Some(format!("Remove the type arguments from `{ann_text}`")),
                note: Some(format!(
                    "`{base}` does not use any TypeVar parameters in its definition"
                )),
            });
        } else if arg_count > info.typevar_count {
            diagnostics.push(Diagnostic {
                code: CODE.clone(),
                severity: Severity::Error,
                message: format!(
                    "Too many type arguments for `{base}`: expected {}, got {arg_count}",
                    info.typevar_count
                ),
                span: ann_span,
                path: path.to_owned(),
                help: Some(format!(
                    "`{base}` accepts {} type parameter(s)",
                    info.typevar_count
                )),
                note: None,
            });
        }
    }
}
