//! Implements helpers for [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Shared helper functions used across multiple type checking rules.
//!
//! Consolidated from duplicated implementations in individual rule modules
//! to eliminate code duplication and improve maintainability.

use std::collections::{HashMap, HashSet};

use basilisk_parser::ParsedModule;
use basilisk_resolver::{ClassInfo, FunctionInfo, ResolvedModule, Span, TypeVarCallInfo};
use ruff_python_ast::{self as ast, Expr};

// ---------------------------------------------------------------------------
// Source-text geometry
// ---------------------------------------------------------------------------

/// Number of leading whitespace bytes on `line`. Identical to what every rule
/// re-implemented as `line.len() - line.trim_start().len()`.
pub(crate) fn leading_indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Return the byte offset (as `u32`) of the start of the given 1-based line.
/// If `target_line` is past the end of `source`, returns `source.len()`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::as_conversions,
    reason = "byte offsets fit u32 for source files"
)]
pub(crate) fn line_to_byte_offset(source: &str, target_line: usize) -> u32 {
    let mut current = 1usize;
    for (byte_idx, ch) in source.char_indices() {
        if current == target_line {
            return byte_idx as u32;
        }
        if ch == '\n' {
            current += 1;
        }
    }
    source.len() as u32
}

/// Returns `true` when `inner` contains a comma at bracket-depth zero.
///
/// Bracket-depth tracks `[`/`(`/`{` openers and their matching closers. Used
/// by rules that need to decide whether a parenthesised expression like
/// `(a, b)` is a tuple at top level versus a single bracketed group.
pub(crate) fn contains_top_level_comma(inner: &str) -> bool {
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

/// Returns `true` when `s` is a `(...)` parenthesised expression whose
/// contents contain a top-level comma (i.e. a tuple expression).
pub(crate) fn paren_has_top_level_comma(s: &str) -> bool {
    if s.len() < 2 || !s.starts_with('(') || !s.ends_with(')') {
        return false;
    }
    contains_top_level_comma(&s[1..s.len() - 1])
}

/// Build a `Span` covering the trimmed content of a given 1-based line.
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    reason = "u32<->usize safe on 32-bit+"
)]
pub(crate) fn span_for_line(source: &str, line_number: usize) -> Span {
    let start = line_to_byte_offset(source, line_number) as usize;
    let line_text = source
        .get(start..)
        .and_then(|s| s.lines().next())
        .unwrap_or("");
    let trimmed_start = start + (line_text.len() - line_text.trim_start().len());
    let trimmed_end = start + line_text.trim_end().len();
    Span {
        start: trimmed_start as u32,
        end: trimmed_end as u32,
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse the resolved module's source into an AST, returning `None` on parse failure.
///
/// Every `Rule::check` implementation needs the AST and silently bails on parse
/// errors (those are reported separately as `BSK-E0000`). This collapses that
/// boilerplate into a single line at the call site.
pub(crate) fn parse_module(module: &ResolvedModule) -> Option<ParsedModule> {
    basilisk_parser::parse_source(module.source.clone(), module.path.clone()).ok()
}

// ---------------------------------------------------------------------------
// Class lookup
// ---------------------------------------------------------------------------

/// Build a `&str -> &ClassInfo` lookup map for every class in the module.
///
/// The returned map borrows from the slice; both must outlive the map.
pub(crate) fn class_name_map(classes: &[ClassInfo]) -> HashMap<&str, &ClassInfo> {
    classes.iter().map(|c| (c.name.as_str(), c)).collect()
}

/// Build a `(class_name, method_name) -> Vec<&FunctionInfo>` lookup for every
/// method in the module (functions carrying a `class_name`).
///
/// Multiple definitions sharing a key (e.g. `@overload` signatures plus the
/// implementation) are preserved in declaration order. The returned map borrows
/// from the slice; both must outlive the map.
pub(crate) fn method_name_map(
    functions: &[FunctionInfo],
) -> HashMap<(&str, &str), Vec<&FunctionInfo>> {
    let mut map: HashMap<(&str, &str), Vec<&FunctionInfo>> = HashMap::new();
    for func in functions {
        if let Some(ref class_name) = func.class_name {
            map.entry((class_name.as_str(), func.name.as_str()))
                .or_default()
                .push(func);
        }
    }
    map
}

// ---------------------------------------------------------------------------
// TypeVar helpers
// ---------------------------------------------------------------------------

/// Collect the names of every `TypeVarTuple` declared in the module.
///
/// The returned set borrows from the slice; both must outlive the set.
pub(crate) fn typevar_tuple_names(typevar_calls: &[TypeVarCallInfo]) -> HashSet<&str> {
    typevar_calls
        .iter()
        .filter(|tv| tv.is_typevartuple)
        .map(|tv| tv.name.as_str())
        .collect()
}

// ---------------------------------------------------------------------------
// String splitting
// ---------------------------------------------------------------------------

/// Split `s` at every top-level comma, respecting bracket nesting.
///
/// Returns slices into the original string (no allocation for the parts
/// themselves). Callers that need trimmed/owned values can chain
/// `.iter().map(|p| p.trim().to_owned())`.
pub(crate) fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: usize = 0;
    let mut start = 0;
    for (idx, ch) in s.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&s[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

// ---------------------------------------------------------------------------
// Annotation parsing
// ---------------------------------------------------------------------------

/// Parse a subscript annotation like `Name[A, B]` into `(name, [A, B])`.
///
/// Returns a borrowed name and owned, trimmed type argument strings.
/// Returns `None` when the annotation is not a valid subscript form.
pub(crate) fn parse_subscript_annotation(text: &str) -> Option<(&str, Vec<String>)> {
    let bracket_pos = text.find('[')?;
    let name = text[..bracket_pos].trim();
    if name.is_empty() {
        return None;
    }
    let inner = text.get(bracket_pos + 1..text.rfind(']')?)?;
    let args: Vec<String> = split_top_level_commas(inner)
        .iter()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    if args.is_empty() {
        return None;
    }
    Some((name, args))
}

// ---------------------------------------------------------------------------
// Expression helpers
// ---------------------------------------------------------------------------

/// Extract the simple name from a `Name` expression.
pub(crate) fn expr_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Name(n) => Some(n.id.as_str()),
        _ => None,
    }
}

/// Convert an annotation expression to a readable string.
pub(crate) fn ann_str(expr: &Expr) -> String {
    match expr {
        Expr::Name(n) => n.id.to_string(),
        Expr::Subscript(s) => format!("{}[{}]", ann_str(&s.value), ann_str(&s.slice)),
        Expr::Attribute(a) => format!("{}.{}", ann_str(&a.value), a.attr),
        Expr::Tuple(t) => t.elts.iter().map(ann_str).collect::<Vec<_>>().join(", "),
        Expr::BinOp(b) => format!("{} | {}", ann_str(&b.left), ann_str(&b.right)),
        Expr::NoneLiteral(_) => "None".to_owned(),
        Expr::StringLiteral(s) => s.value.to_str().to_owned(),
        Expr::List(l) => format!(
            "[{}]",
            l.elts.iter().map(ann_str).collect::<Vec<_>>().join(", ")
        ),
        Expr::NumberLiteral(n) => format!("{:?}", n.value),
        _ => "...".to_owned(),
    }
}

/// Infer the concrete type of a literal expression.
pub(crate) fn infer_expr_literal_type(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::NumberLiteral(n) => match &n.value {
            ast::Number::Int(_) => Some("int"),
            ast::Number::Float(_) => Some("float"),
            ast::Number::Complex { .. } => Some("complex"),
        },
        Expr::StringLiteral(_) => Some("str"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::NoneLiteral(_) => Some("None"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Type compatibility
// ---------------------------------------------------------------------------

/// Check numeric subtype relationship: `bool → int → float → complex`.
pub(crate) fn is_numeric_subtype(child: &str, parent: &str) -> bool {
    if child == parent {
        return true;
    }
    matches!(
        (child, parent),
        ("bool", "int" | "float" | "complex") | ("int", "float" | "complex") | ("float", "complex")
    )
}

/// Check if `actual` is assignable to `expected`.
///
/// Handles `Any`, `object`, the numeric tower (`bool → int → float → complex`),
/// and union types (`X | Y`).
pub(crate) fn is_type_compatible(actual: &str, expected: &str) -> bool {
    if actual == expected {
        return true;
    }
    if expected == "Any" || actual == "Any" || expected == "object" {
        return true;
    }
    if is_numeric_subtype(actual, expected) {
        return true;
    }
    if expected.contains('|') {
        return expected
            .split('|')
            .any(|part| is_type_compatible(actual, part.trim()));
    }
    false
}

// ---------------------------------------------------------------------------
// Identifier / typevar matching
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Literal helpers
// ---------------------------------------------------------------------------

/// Extract the content between `Literal[` (or `L[`) and the matching `]`.
pub(crate) fn extract_literal_inner(ann: &str) -> Option<&str> {
    // Support both `Literal[` and `L[`.
    let start_bracket = if let Some(pos) = ann.find("Literal[") {
        pos + "Literal[".len()
    } else if ann.starts_with("L[") {
        2
    } else {
        return None;
    };

    let mut depth = 1i32;
    let bytes = ann.as_bytes();
    let mut idx = start_bracket;
    while idx < bytes.len() {
        match bytes.get(idx) {
            Some(b'[') => depth += 1,
            Some(b']') => {
                depth -= 1;
                if depth == 0 {
                    return ann.get(start_bracket..idx);
                }
            }
            Some(_) | None => {}
        }
        idx += 1;
    }
    None
}

/// Extract the callee name from a RHS text like `ClassName(...)` or `ClassName[T](...)`.
pub(crate) fn extract_callee_name(rhs_text: &str) -> Option<&str> {
    // Handle `ClassName[T](...)` by stripping everything from `[` onwards first.
    let before_bracket = rhs_text.split('[').next()?;
    let before_paren = before_bracket.split('(').next()?;
    let name = before_paren.trim();
    if name.is_empty() {
        return None;
    }
    // Class names start with uppercase (heuristic).
    if !name.starts_with(|c: char| c.is_ascii_uppercase()) {
        return None;
    }
    Some(name)
}

// ---------------------------------------------------------------------------
// Identifier / typevar matching
// ---------------------------------------------------------------------------

pub(crate) fn contains_typevar_reference(text: &str, name: &str) -> bool {
    let name_bytes = name.as_bytes();
    let text_bytes = text.as_bytes();
    let name_len = name_bytes.len();

    if name_len > text_bytes.len() {
        return false;
    }

    for start in 0..=(text_bytes.len() - name_len) {
        let Some(slice) = text_bytes.get(start..start + name_len) else {
            continue;
        };
        if slice != name_bytes {
            continue;
        }
        if start > 0
            && text_bytes
                .get(start - 1)
                .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
        {
            continue;
        }
        let end = start + name_len;
        if end < text_bytes.len()
            && text_bytes
                .get(end)
                .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_')
        {
            continue;
        }
        return true;
    }
    false
}
