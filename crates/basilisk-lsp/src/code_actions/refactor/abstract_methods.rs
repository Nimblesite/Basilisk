//! Implement abstract methods refactoring action.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, Position, Range, TextEdit, Url, WorkspaceEdit,
};

use super::helpers::{byte_offset_to_line, leading_indent_of_line};

/// Offer to implement all abstract methods from base classes.
///
/// Scans the resolved module for the class at the cursor position, finds
/// abstract methods in its bases, and generates stub implementations.
#[must_use]
pub(in crate::code_actions) fn implement_abstract_methods(
    uri: &Url,
    source: &str,
    resolved: &basilisk_resolver::ResolvedModule,
    position_offset: usize,
) -> Option<CodeAction> {
    let offset_u32 = u32::try_from(position_offset).unwrap_or(u32::MAX);

    // Find the class at the cursor position.
    let current_class = resolved
        .classes
        .iter()
        .find(|cls| cls.def_span.start <= offset_u32 && offset_u32 <= cls.def_span.end)?;

    // Collect abstract methods from base classes.
    let mut abstract_methods: Vec<&basilisk_resolver::FunctionInfo> = Vec::new();

    for base_name in &current_class.bases {
        // Find the base class in the same module.
        let Some(base_class) = resolved.classes.iter().find(|cls| &cls.name == base_name) else {
            continue;
        };

        // Find methods in the base that are decorated with `abstractmethod`.
        for func in &resolved.functions {
            let Some(ref fn_class) = func.class_name else {
                continue;
            };
            if fn_class != &base_class.name {
                continue;
            }
            if !func.decorators.iter().any(|d| d == "abstractmethod") {
                continue;
            }
            // Skip if already implemented in the current class.
            if current_class.method_names.contains(&func.name) {
                continue;
            }
            abstract_methods.push(func);
        }
    }

    if abstract_methods.is_empty() {
        return None;
    }

    // Generate stub method implementations.
    let indent = leading_indent_of_line(
        source,
        byte_offset_to_line(source, current_class.def_span.start),
    );
    let method_indent = format!("{indent}    ");
    let body_indent = format!("{indent}        ");

    let mut stubs = String::new();
    for method in &abstract_methods {
        stubs.push('\n');
        stubs.push_str(&method_indent);
        stubs.push_str("def ");
        stubs.push_str(&method.name);
        stubs.push('(');
        stubs.push_str(&format_parameters(method));
        stubs.push_str("):");
        stubs.push('\n');
        stubs.push_str(&body_indent);
        stubs.push_str("raise NotImplementedError\n");
    }

    // Insert at the end of the class body (before the closing line).
    let insert_line = byte_offset_to_line(source, current_class.def_span.end);
    let insert_pos = Position {
        line: insert_line,
        character: 0,
    };

    let mut changes = HashMap::new();
    let _ = changes.insert(
        uri.clone(),
        vec![TextEdit {
            range: Range {
                start: insert_pos,
                end: insert_pos,
            },
            new_text: stubs,
        }],
    );

    let method_names: Vec<&str> = abstract_methods.iter().map(|m| m.name.as_str()).collect();
    let title = format!(
        "Implement abstract method{} {} (basilisk)",
        if method_names.len() > 1 { "s" } else { "" },
        method_names.join(", ")
    );

    Some(CodeAction {
        title,
        kind: Some(CodeActionKind::new("refactor.rewrite.implement")),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        }),
        is_preferred: Some(false),
        ..Default::default()
    })
}

/// Format function parameters for a stub implementation.
fn format_parameters(func: &basilisk_resolver::FunctionInfo) -> String {
    let mut parts: Vec<String> = Vec::new();
    for param in &func.parameters {
        if let Some(ref ann) = param.annotation_text {
            parts.push(format!("{}: {ann}", param.name));
        } else {
            parts.push(param.name.clone());
        }
    }
    if let Some(ref vararg) = func.vararg {
        if let Some(ref ann) = vararg.annotation_text {
            parts.push(format!("*{}: {ann}", vararg.name));
        } else {
            parts.push(format!("*{}", vararg.name));
        }
    }
    if let Some(ref kwarg) = func.kwarg {
        if let Some(ref ann) = kwarg.annotation_text {
            parts.push(format!("**{}: {ann}", kwarg.name));
        } else {
            parts.push(format!("**{}", kwarg.name));
        }
    }
    parts.join(", ")
}
