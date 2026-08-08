//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Typeddict visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtAssign};

use crate::scope::{ClassInfo, TypedDictKeyViolation, TypedDictKeyViolationKind};

use super::class_info_ext::expr_simple_name;
use super::core::{check_td_stmts, text_range_to_span};

/// Collect `TypedDict` key/value violations from module-level statements and function bodies.
///
/// Detects:
/// - Subscript assignments with invalid keys: `movie["director"] = "Ridley Scott"`
/// - Annotated dict literal assignments with invalid or missing keys
/// - Regular dict assignments to `TypedDict` variables with wrong keys
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
    let mut out = Vec::new();
    check_td_stmts(&typeddict_fields, &var_type, stmts, &mut out);
    out
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
        let Some((all_fields, _, _, has_extra_items)) = fields.get(class_name.as_str()) else {
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
        }
    }
}
