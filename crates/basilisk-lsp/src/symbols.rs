//! Document Symbols handler (Outline view).

#[allow(deprecated)]
use tower_lsp::lsp_types::{DocumentSymbol, SymbolKind};

use basilisk_resolver::ResolvedModule;

use crate::util::span_to_range;

/// Build a hierarchical document symbol tree from a resolved module.
#[allow(deprecated)] // DocumentSymbol.deprecated field is deprecated but required by the struct
#[must_use] pub fn document_symbols(resolved: &ResolvedModule, source: &str) -> Vec<DocumentSymbol> {
    let mut top_level: Vec<DocumentSymbol> = Vec::new();

    // Classes with nested methods and attributes.
    for class in &resolved.classes {
        let mut children: Vec<DocumentSymbol> = Vec::new();

        // Attributes as fields.
        for attr in &class.attributes {
            children.push(DocumentSymbol {
                name: attr.name.clone(),
                detail: annotation_detail(attr.annotation_span, source),
                kind: SymbolKind::FIELD,
                range: span_to_range(source, attr.name_span),
                selection_range: span_to_range(source, attr.name_span),
                children: None,
                tags: None,
                deprecated: None,
            });
        }

        // Methods: functions where class_name matches this class.
        for func in &resolved.functions {
            if func.class_name.as_deref() == Some(&class.name) {
                children.push(DocumentSymbol {
                    name: func.name.clone(),
                    detail: Some(method_detail(func, source)),
                    kind: SymbolKind::METHOD,
                    range: span_to_range(source, func.def_span),
                    selection_range: span_to_range(source, func.name_span),
                    children: None,
                    tags: None,
                    deprecated: None,
                });
            }
        }

        top_level.push(DocumentSymbol {
            name: class.name.clone(),
            detail: if class.bases.is_empty() {
                None
            } else {
                Some(class.bases.join(", "))
            },
            kind: SymbolKind::CLASS,
            range: span_to_range(source, class.def_span),
            selection_range: span_to_range(source, class.name_span),
            children: if children.is_empty() {
                None
            } else {
                Some(children)
            },
            tags: None,
            deprecated: None,
        });
    }

    // Standalone functions (not methods).
    for func in &resolved.functions {
        if func.class_name.is_some() {
            continue; // Methods are nested under their class.
        }
        top_level.push(DocumentSymbol {
            name: func.name.clone(),
            detail: Some(function_detail(func, source)),
            kind: SymbolKind::FUNCTION,
            range: span_to_range(source, func.def_span),
            selection_range: span_to_range(source, func.name_span),
            children: None,
            tags: None,
            deprecated: None,
        });
    }

    // Module-level variables.
    for var in &resolved.module_vars {
        top_level.push(DocumentSymbol {
            name: var.name.clone(),
            detail: annotation_detail(var.annotation_span, source),
            kind: SymbolKind::VARIABLE,
            range: span_to_range(source, var.name_span),
            selection_range: span_to_range(source, var.name_span),
            children: None,
            tags: None,
            deprecated: None,
        });
    }

    top_level
}

/// Build a detail string for a function's parameters.
fn function_detail(
    func: &basilisk_resolver::FunctionInfo,
    source: &str,
) -> String {
    let params = param_list(func, source);
    let ret = return_annotation(func, source);
    format!("({params}){ret}")
}

/// Build a detail string for a method's parameters (skip self).
fn method_detail(
    func: &basilisk_resolver::FunctionInfo,
    source: &str,
) -> String {
    let params = param_list_skip_self(func, source);
    let ret = return_annotation(func, source);
    format!("({params}){ret}")
}

fn param_list(func: &basilisk_resolver::FunctionInfo, source: &str) -> String {
    func.parameters
        .iter()
        .map(|p| param_display(p, source))
        .collect::<Vec<_>>()
        .join(", ")
}

fn param_list_skip_self(func: &basilisk_resolver::FunctionInfo, source: &str) -> String {
    func.parameters
        .iter()
        .filter(|p| p.name != "self" && p.name != "cls")
        .map(|p| param_display(p, source))
        .collect::<Vec<_>>()
        .join(", ")
}

fn param_display(param: &basilisk_resolver::ParameterInfo, source: &str) -> String {
    if let Some(ann) = annotation_detail(param.annotation_span, source) {
        format!("{}: {ann}", param.name)
    } else {
        param.name.clone()
    }
}

fn return_annotation(func: &basilisk_resolver::FunctionInfo, source: &str) -> String {
    match func.return_annotation {
        basilisk_resolver::ReturnAnnotationKind::Missing => String::new(),
        basilisk_resolver::ReturnAnnotationKind::NoneType => " -> None".to_owned(),
        basilisk_resolver::ReturnAnnotationKind::Any => " -> Any".to_owned(),
        _ => {
            if let Some(ann) = annotation_detail(func.return_annotation_span, source) {
                format!(" -> {ann}")
            } else {
                String::new()
            }
        }
    }
}

fn annotation_detail(
    span: Option<basilisk_resolver::Span>,
    source: &str,
) -> Option<String> {
    let span = span?;
    let text = source.get(span.start as usize..span.end as usize)?;
    Some(text.trim().to_owned())
}
