//! Pure helpers for ordered `__all__` extraction.

use std::collections::HashMap;

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

pub(super) fn literal_dunder_all(mutations: &[DunderAllMutation]) -> Option<Vec<String>> {
    if mutations.is_empty() {
        return None;
    }
    let mut names = Vec::new();
    apply_literal_mutations(&mut names, mutations);
    Some(names)
}

fn apply_literal_mutations(names: &mut Vec<String>, mutations: &[DunderAllMutation]) {
    for mutation in mutations {
        apply_literal_mutation(names, mutation);
    }
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
        DunderAllMutation::Choice(branches) => {
            let alternatives = branches.iter().map(|branch| {
                let mut alternative = names.clone();
                apply_literal_mutations(&mut alternative, branch);
                alternative
            });
            *names = intersect_name_lists(alternatives);
        }
    }
}

pub(crate) fn intersect_name_lists(
    mut alternatives: impl Iterator<Item = Vec<String>>,
) -> Vec<String> {
    let mut intersection = alternatives.next().unwrap_or_default();
    for alternative in alternatives {
        let mut counts = HashMap::new();
        for name in alternative {
            let count = counts.entry(name).or_insert(0_usize);
            *count = count.saturating_add(1);
        }
        intersection.retain(|name| {
            let Some(remaining) = counts.get_mut(name) else {
                return false;
            };
            if *remaining == 0 {
                return false;
            }
            *remaining = remaining.saturating_sub(1);
            true
        });
    }
    intersection
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
