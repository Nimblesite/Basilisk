//! Keyword-argument completion provider.
//!
//! When the cursor is inside a function call, suggests `param_name=` items
//! for parameters that have not yet been supplied.

use std::collections::HashSet;

use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind};

/// Return `param=` completion items when the cursor is inside a call, or
/// `None` if the context does not match a function call.
pub(super) fn kwarg_completions(
    resolved: &basilisk_resolver::ResolvedModule,
    text: &str,
    byte_offset: usize,
    prefix: &str,
) -> Option<Vec<CompletionItem>> {
    let callee = find_enclosing_call(text, byte_offset)?;
    let func = find_function(resolved, &callee)?;
    let already_provided = already_provided_kwargs(text, byte_offset);

    let items = func
        .parameters
        .iter()
        .filter(|p| !is_self_or_cls(&p.name))
        .filter(|p| !already_provided.contains(&p.name))
        .filter(|p| {
            let label = format!("{}=", p.name);
            prefix.is_empty() || label.starts_with(prefix) || p.name.starts_with(prefix)
        })
        .map(|p| CompletionItem {
            label: format!("{}=", p.name),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(format!("keyword argument for {callee}")),
            ..Default::default()
        })
        .collect::<Vec<_>>();

    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

/// Scan backwards from `byte_offset` to find an unmatched `(`, then extract
/// the callee identifier immediately before it.
fn find_enclosing_call(text: &str, byte_offset: usize) -> Option<String> {
    let before = text.get(..byte_offset.min(text.len())).unwrap_or(text);
    let mut depth: i32 = 0;
    let mut paren_pos = None;

    for (idx, ch) in before.char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    paren_pos = Some(idx);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    let paren_idx = paren_pos?;
    let before_paren = text.get(..paren_idx)?;
    let name: String = before_paren
        .chars()
        .rev()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Look up a `FunctionInfo` by name in the resolved module.
fn find_function<'a>(
    resolved: &'a basilisk_resolver::ResolvedModule,
    name: &str,
) -> Option<&'a basilisk_resolver::scope::FunctionInfo> {
    resolved.functions.iter().find(|f| f.name == name)
}

/// Collect keyword arguments already supplied between the enclosing `(` and
/// the cursor (`name=` but not `name==`).
fn already_provided_kwargs(text: &str, byte_offset: usize) -> HashSet<String> {
    let before = text.get(..byte_offset.min(text.len())).unwrap_or(text);
    let mut result = HashSet::new();

    let mut depth: i32 = 0;
    let mut paren_pos = None;
    for (idx, ch) in before.char_indices().rev() {
        match ch {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    paren_pos = Some(idx);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    let Some(start) = paren_pos else {
        return result;
    };

    let args_region = text
        .get(start + 1..byte_offset.min(text.len()))
        .unwrap_or("");
    let chars: Vec<char> = args_region.chars().collect();
    let len = chars.len();
    let mut idx = 0;

    while idx < len {
        let Some(&ch) = chars.get(idx) else { break };
        if ch.is_whitespace() || ch == ',' {
            idx += 1;
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            let start_idx = idx;
            while idx < len
                && chars
                    .get(idx)
                    .is_some_and(|c| c.is_alphanumeric() || *c == '_')
            {
                idx += 1;
            }
            let ident: String = chars
                .get(start_idx..idx)
                .map(|s| s.iter().collect())
                .unwrap_or_default();
            if chars.get(idx) == Some(&'=') && chars.get(idx + 1) != Some(&'=') {
                let _ = result.insert(ident);
            }
        } else {
            idx += 1;
        }
    }

    result
}

/// Returns `true` if the parameter name is `self` or `cls`.
fn is_self_or_cls(name: &str) -> bool {
    name == "self" || name == "cls"
}
