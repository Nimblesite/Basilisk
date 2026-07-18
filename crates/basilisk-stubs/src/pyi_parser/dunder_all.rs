//! Pure helpers for ordered `__all__` extraction.

use ruff_python_ast::Expr;

use crate::types::{DunderAllItem, DunderAllMutation};

pub(super) fn literal_dunder_all_items(elements: &[Expr]) -> Option<Vec<DunderAllItem>> {
    elements
        .iter()
        .map(|element| string_literal(element).map(DunderAllItem::Name))
        .collect()
}

pub(super) fn string_literal(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StringLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

pub(super) fn dotted_expression(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attribute) => Some(format!(
            "{}.{}",
            dotted_expression(&attribute.value)?,
            attribute.attr
        )),
        _ => None,
    }
}

pub(super) fn literal_dunder_all_intersection(
    variants: &[Vec<DunderAllMutation>],
) -> Option<Vec<String>> {
    if variants.iter().all(Vec::is_empty) {
        return None;
    }
    let mut alternatives = variants.iter().map(|variant| {
        let mut names = Vec::new();
        for mutation in variant {
            apply_literal_mutation(&mut names, mutation);
        }
        names
    });
    let mut intersection = alternatives.next().unwrap_or_default();
    for alternative in alternatives {
        intersection.retain(|name| alternative.contains(name));
    }
    Some(intersection)
}

fn apply_literal_mutation(names: &mut Vec<String>, mutation: &DunderAllMutation) {
    match mutation {
        DunderAllMutation::Assign(items) => {
            *names = literal_item_names(items);
        }
        DunderAllMutation::Extend(items) => names.extend(literal_item_names(items)),
        DunderAllMutation::Append(name) => names.push(name.clone()),
        DunderAllMutation::Remove(name) => {
            if let Some(position) = names.iter().position(|entry| entry == name) {
                let _ = names.remove(position);
            }
        }
    }
}

fn literal_item_names(items: &[DunderAllItem]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| match item {
            DunderAllItem::Name(name) => Some(name.clone()),
            DunderAllItem::ModuleAll(_) => None,
        })
        .collect()
}
