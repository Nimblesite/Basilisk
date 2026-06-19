//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Typeddict visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtAssign};
use ruff_text_size::Ranged;

use crate::scope::{
    ClassInfo, TypedDictCallInfo, TypedDictKeyViolation, TypedDictKeyViolationKind,
    TypedDictSecondArgKind,
};

use super::annotations::strip_annotated_wrapper;
use super::class_info_ext::expr_simple_name;
use super::core::{check_td_stmts, text_range_to_span};
use super::final_readonly::TYPING_FORMS;
use super::typeddict_ext::{expr_literal_type_name, typeddict_field_type_compatible};

pub(super) fn collect_typeddict_calls(stmts: &[Stmt]) -> Vec<TypedDictCallInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::Assign(node) = stmt else { continue };
        let Expr::Call(call) = node.value.as_ref() else {
            continue;
        };
        // Callee must be `TypedDict` or `typing.TypedDict`.
        let is_typeddict = if let Some(name) = expr_simple_name(&call.func) {
            name == "TypedDict"
        } else if let Expr::Attribute(attr) = call.func.as_ref() {
            attr.attr.as_str() == "TypedDict"
        } else {
            false
        };
        if !is_typeddict {
            continue;
        }
        // Determine the LHS name.
        let Some(lhs_name) = node.targets.first().and_then(expr_simple_name) else {
            continue;
        };
        // First positional arg: the declared name (must be a string literal).
        let declared_name = call.arguments.args.first().and_then(|arg| {
            if let Expr::StringLiteral(s) = arg {
                Some(s.value.to_string())
            } else {
                None
            }
        });
        // Second positional arg: expected to be a dict literal.
        let has_positional_dict;
        let (second_arg_kind, has_non_string_key) =
            if let Some(second_arg) = call.arguments.args.get(1) {
                has_positional_dict = true;
                if let Expr::Dict(dict) = second_arg {
                    // Check if every key is a string literal.
                    let non_string = dict.items.iter().any(|item| {
                        item.key
                            .as_ref()
                            .is_some_and(|k| !matches!(k, Expr::StringLiteral(_)))
                    });
                    (TypedDictSecondArgKind::DictLiteral, non_string)
                } else {
                    (TypedDictSecondArgKind::NotDictLiteral, false)
                }
            } else {
                // No second arg — keyword syntax or zero args; treat as dict literal
                // variant since we don't flag keyword-only syntax here.
                has_positional_dict = false;
                (TypedDictSecondArgKind::DictLiteral, false)
            };
        let keyword_names: Vec<String> = call
            .arguments
            .keywords
            .iter()
            .filter_map(|kw| kw.arg.as_ref().map(std::string::ToString::to_string))
            .collect();
        out.push(TypedDictCallInfo {
            lhs_name,
            declared_name,
            second_arg_kind,
            has_non_string_key,
            has_positional_dict,
            keyword_names,
            span: text_range_to_span(call.range()),
        });
    }
    out
}

/// Collect module-level `NewType(...)` call sites.
///
/// Matches assignments of the form `Name = NewType("Name", BaseType)`.
pub(super) fn expr_is_parameterized(expr: &Expr) -> bool {
    match expr {
        Expr::Subscript(sub) => {
            // Skip well-known typing forms: Literal["x"], Optional[T], etc.
            let base_name = expr_simple_name(&sub.value);
            if base_name
                .as_deref()
                .is_some_and(|n| TYPING_FORMS.contains(&n))
            {
                return false;
            }
            true
        }
        Expr::BinOp(bin) => expr_is_parameterized(&bin.left) || expr_is_parameterized(&bin.right),
        Expr::Tuple(tup) => tup.elts.iter().any(expr_is_parameterized),
        _ => false,
    }
}

/// Extracts the name from a PEP 695 `TypeParam` (`TypeVar`, `TypeVarTuple`, or `ParamSpec`).
pub(super) fn resolve_actual_type(
    expr: &Expr,
    params: &std::collections::HashMap<String, String>,
    _source: &str,
) -> Option<String> {
    match expr {
        Expr::Name(name) => params.get(name.id.as_str()).and_then(|ann| {
            let normalized = normalize_type_str(ann);
            // If the normalized annotation still contains quotes, it has forward
            // references inside subscripts (e.g. `list["ClassA"]`) that we cannot
            // resolve textually — skip the check to avoid false positives.
            if normalized.contains('"') || normalized.contains('\'') {
                None
            } else {
                Some(normalized)
            }
        }),
        Expr::StringLiteral(_) => Some("str".to_owned()),
        Expr::NumberLiteral(n) => {
            if matches!(n.value, ruff_python_ast::Number::Float(_)) {
                Some("float".to_owned())
            } else {
                Some("int".to_owned())
            }
        }
        Expr::BooleanLiteral(_) => Some("bool".to_owned()),
        Expr::BytesLiteral(_) => Some("bytes".to_owned()),
        Expr::NoneLiteral(_) => Some("None".to_owned()),
        // Complex expressions (attribute access, subscripts, calls, binary ops, etc.)
        // cannot be typed without full type inference — returning source text produces
        // false positives when compared against expected types textually.
        _ => None,
    }
}

/// Extract the text of a type expression (the second argument to `assert_type`).
pub(super) fn normalize_type_str(ann: &str) -> String {
    let trimmed = ann.trim();
    // Strip Annotated[T, ...] → take first argument only.
    if let Some(inner) = strip_annotated_wrapper(trimmed) {
        return normalize_type_str(inner);
    }
    // Strip outer string quotes (forward references like `"list[int]"` or `'MyClass'`).
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        return normalize_type_str(&trimmed[1..trimmed.len() - 1]);
    }
    trimmed.to_owned()
}

/// Split `s` at every top-level comma, respecting `[](){}` nesting.
///
/// Returns trimmed slices into the original string.
pub(super) fn split_top_level_args(inner: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth: usize = 0;
    let mut start = 0;
    for (idx, ch) in inner.char_indices() {
        match ch {
            '[' | '(' | '{' => depth += 1,
            ']' | ')' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(inner[start..idx].trim());
                start = idx + 1;
            }
            _ => {}
        }
    }
    parts.push(inner[start..].trim());
    parts
}

/// Split `name[inner]` into `(name, inner)` when `text` is a single subscript
/// whose `[` opens at top level and whose matching `]` is the final character.
pub(super) fn split_subscript(text: &str) -> Option<(&str, &str)> {
    let open = text.find('[')?;
    if !text.ends_with(']') {
        return None;
    }
    let name = text[..open].trim();
    if name.is_empty() {
        return None;
    }
    let inner = &text[open + 1..text.len() - 1];
    Some((name, inner))
}

/// Strip a leading unpacked-tuple marker (`*tuple[...]` or `Unpack[tuple[...]]`),
/// returning the inner element text of the wrapped tuple. Other unpacks (e.g.
/// `*Ts` for a `TypeVarTuple`) yield `None` — they cannot be expanded textually.
fn unpacked_tuple_inner(arg: &str) -> Option<&str> {
    let arg = arg.trim();
    let tuple_expr = if let Some(star) = arg.strip_prefix('*') {
        star.trim()
    } else {
        arg.strip_prefix("Unpack[")
            .and_then(|rest| rest.strip_suffix(']'))?
            .trim()
    };
    split_subscript(tuple_expr).and_then(|(name, inner)| (name == "tuple").then_some(inner))
}

/// Canonicalize a type-expression string so that equivalent spellings compare
/// equal. Two rewrites are applied recursively:
///
/// - `Union[a, b, ...]` → `a | b | ...` (PEP 604 form), preserving member order.
/// - Fixed unpacked tuples inside `tuple[...]` are spliced in place, e.g.
///   `tuple[int, *tuple[bool, bool], str]` → `tuple[int, bool, bool, str]`.
///   Unbounded unpacks (`*tuple[x, ...]`, `Unpack[tuple[x, ...]]`) are left
///   intact, matching the typing-spec rule that an unbounded tuple is preserved.
///
/// Order is preserved (no member sorting), so genuinely different types such as
/// `int` vs `int | str` stay distinct. Implements part of [BSK-E0053].
pub(super) fn canonicalize_type_str(ann: &str) -> String {
    let text = normalize_type_str(ann);
    let trimmed = text.trim();
    match split_subscript(trimmed) {
        Some(("Union", inner)) => split_top_level_args(inner)
            .iter()
            .map(|a| canonicalize_type_str(a))
            .collect::<Vec<_>>()
            .join(" | "),
        Some(("tuple", inner)) => format!("tuple[{}]", canonicalize_tuple_args(inner)),
        Some((name, inner)) => {
            let args = split_top_level_args(inner)
                .iter()
                .map(|a| canonicalize_type_str(a))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}[{args}]")
        }
        None => trimmed.to_owned(),
    }
}

/// Canonicalize the comma-separated argument list inside a `tuple[...]`,
/// splicing fixed unpacked tuples and preserving unbounded ones.
fn canonicalize_tuple_args(inner: &str) -> String {
    split_top_level_args(inner)
        .iter()
        .map(|arg| canonicalize_tuple_member(arg))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Canonicalize a single `tuple[...]` member, returning its flattened spelling.
///
/// A fixed unpacked tuple is replaced by its (recursively canonicalized) members;
/// every other member is canonicalized on its own.
fn canonicalize_tuple_member(arg: &str) -> String {
    match unpacked_tuple_inner(arg) {
        // Unbounded unpacks contain a top-level `...` element and are preserved.
        Some(elem_inner) if !split_top_level_args(elem_inner).contains(&"...") => {
            canonicalize_tuple_args(elem_inner)
        }
        _ => canonicalize_type_str(arg),
    }
}

/// If `ann` starts with `Annotated[`, return the first type argument (the actual type).
pub(super) fn build_var_type_map<'a>(
    stmts: &[Stmt],
    td_readonly_fields: &'a std::collections::HashMap<String, std::collections::HashSet<String>>,
) -> std::collections::HashMap<String, &'a str> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Some(var_name) = expr_simple_name(&ann.target) else {
            continue;
        };
        let Expr::Name(type_name) = ann.annotation.as_ref() else {
            continue;
        };
        if let Some((key, _)) = td_readonly_fields.get_key_value(type_name.id.as_str()) {
            let _ = map.insert(var_name, key.as_str());
        }
    }
    map
}

/// Collect `TypedDict` key/value violations from module-level statements and function bodies.
///
/// Detects:
/// - Subscript assignments with invalid keys: `movie["director"] = "Ridley Scott"`
/// - Subscript assignments with wrong value type: `movie["year"] = "1982"`
/// - Annotated dict literal assignments with invalid or missing keys
/// - Regular dict assignments to `TypedDict` variables with wrong keys/types
/// - Subscript read access with invalid keys: `print(movie["unknown"])`
/// - Disallowed method calls: `movie.clear()`
/// - Delete operations on required `TypedDict` keys: `del movie["name"]`
pub(super) fn collect_typeddict_key_violations<'a>(
    stmts: &[Stmt],
    classes: &'a [ClassInfo],
    source: &'a str,
) -> Vec<TypedDictKeyViolation> {
    use std::collections::HashMap;
    // (all_fields, field_types, is_total, has_extra_items)
    type FieldMap<'x> = HashMap<&'x str, (Vec<&'x str>, HashMap<&'x str, String>, bool, bool)>;

    let class_map = crate::scope::class_by_name(classes);

    let typeddict_fields: FieldMap<'a> = classes
        .iter()
        .filter(|c| crate::scope::is_transitive_typeddict(c.name.as_str(), &class_map))
        .map(|c| {
            // Merge own + inherited fields so transitive subclasses
            // (`class Album(NamedDict): ...`) carry the full schema and the
            // most-derived declaration of each redeclared field.
            let effective = super::typeddict_schema::effective_fields(c, &class_map, source);
            let all_fields: Vec<&str> = effective.iter().map(|f| f.name).collect();
            let field_types: HashMap<&str, String> = effective
                .iter()
                .filter_map(|f| f.annotation.map(|ann| (f.name, ann.to_owned())))
                .collect();
            let has_extra_items =
                crate::scope::has_extra_items_transitive(c.name.as_str(), &class_map);
            (
                c.name.as_str(),
                (
                    all_fields,
                    field_types,
                    c.is_typeddict_total,
                    has_extra_items,
                ),
            )
        })
        .collect();

    if typeddict_fields.is_empty() {
        return Vec::new();
    }

    let var_type = td_var_type_from_stmts(stmts, &typeddict_fields);
    let final_consts = collect_final_str_consts(stmts);
    let mut out = Vec::new();
    check_td_stmts(&typeddict_fields, &var_type, &final_consts, stmts, &mut out);
    out
}

/// Returns `true` when `ann` is a `Final` / `Final[...]` / `typing.Final[...]` annotation.
fn ann_is_final(ann: &Expr) -> bool {
    match ann {
        Expr::Name(n) => n.id.as_str() == "Final",
        Expr::Attribute(a) => a.attr.as_str() == "Final",
        Expr::Subscript(s) => ann_is_final(&s.value),
        _ => false,
    }
}

/// Collect module-level `NAME: Final = "literal"` string constants as a
/// name → value map. Used so a `Final` string name can stand in for a string
/// literal in `TypedDict` subscript operations (PEP 591).
fn collect_final_str_consts(stmts: &[Stmt]) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        if !ann_is_final(&ann.annotation) {
            continue;
        }
        let Some(name) = expr_simple_name(&ann.target) else {
            continue;
        };
        if let Some(Expr::StringLiteral(value)) = ann.value.as_deref() {
            let _ = map.insert(name, value.value.to_string());
        }
    }
    map
}

/// `(all_fields, field_types, is_total, has_extra_items)` map keyed by class name.
pub(super) type TdFieldMap<'a> = std::collections::HashMap<
    &'a str,
    (
        Vec<&'a str>,
        std::collections::HashMap<&'a str, String>,
        bool,
        bool,
    ),
>;

/// Build a variable-name → TypedDict-class-name map from annotated assignments in `stmts`.
pub(super) fn td_var_type_from_stmts(
    stmts: &[Stmt],
    fields: &TdFieldMap<'_>,
) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for stmt in stmts {
        let Stmt::AnnAssign(ann) = stmt else { continue };
        let Some(var_name) = expr_simple_name(&ann.target) else {
            continue;
        };
        let Expr::Name(type_name) = ann.annotation.as_ref() else {
            continue;
        };
        let class_name = type_name.id.as_str();
        if fields.contains_key(class_name) {
            let _ = map.insert(var_name, class_name.to_owned());
        }
    }
    map
}

/// Recursively check statements for `TypedDict` violations.
pub(super) fn td_check_subscript_assign(
    node: &StmtAssign,
    var_type: &std::collections::HashMap<String, String>,
    fields: &TdFieldMap<'_>,
    out: &mut Vec<TypedDictKeyViolation>,
) {
    use ruff_text_size::Ranged as _;
    for target in &node.targets {
        let Expr::Subscript(sub) = target else {
            continue;
        };
        let Some(var_name) = expr_simple_name(&sub.value) else {
            continue;
        };
        let Some(class_name) = var_type.get(&var_name) else {
            continue;
        };
        let Some((all_fields, field_types, _, has_extra_items)) = fields.get(class_name.as_str())
        else {
            continue;
        };
        let Expr::StringLiteral(key_str) = sub.slice.as_ref() else {
            continue;
        };
        let key = key_str.value.to_string();
        if !all_fields.contains(&key.as_str()) && !has_extra_items {
            out.push(TypedDictKeyViolation {
                span: text_range_to_span(node.range()),
                class_name: class_name.clone(),
                kind: TypedDictKeyViolationKind::InvalidSubscriptKey { key },
            });
        } else if let Some(expected) = field_types.get(key.as_str()) {
            if let Some(actual) = expr_literal_type_name(&node.value) {
                if !typeddict_field_type_compatible(actual, expected) {
                    out.push(TypedDictKeyViolation {
                        span: text_range_to_span(node.range()),
                        class_name: class_name.clone(),
                        kind: TypedDictKeyViolationKind::WrongSubscriptValueType {
                            key,
                            expected: expected.clone(),
                        },
                    });
                }
            }
        }
    }
}
