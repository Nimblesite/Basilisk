//! Document Symbols handler (Outline view) and workspace symbol search.

use std::sync::Arc;

#[allow(deprecated)]
use tower_lsp::lsp_types::{DocumentSymbol, Location, SymbolInformation, SymbolKind, Url};

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

/// Aggregate symbols across all open documents, filtered by a query string.
///
/// Returns a flat `Vec<SymbolInformation>` for `workspace/symbol`.
/// Each entry in `documents` is `(uri, resolved_module, source_text)`.
/// Symbols whose names contain `query` (case-insensitive) are included.
/// An empty `query` returns every symbol from every document.
#[allow(deprecated)]
#[must_use]
pub fn workspace_symbols(
    documents: &[(Url, Arc<ResolvedModule>, String)],
    query: &str,
) -> Vec<SymbolInformation> {
    let query_lower = query.to_lowercase();
    let mut result = Vec::new();
    for (uri, resolved, source) in documents {
        collect_for_doc(uri, resolved, source, &query_lower, &mut result);
    }
    result
}

/// Collect flat `SymbolInformation` entries for one document.
#[allow(deprecated)]
fn collect_for_doc(
    uri: &Url,
    resolved: &ResolvedModule,
    source: &str,
    query_lower: &str,
    out: &mut Vec<SymbolInformation>,
) {
    for class in &resolved.classes {
        if matches_query(&class.name, query_lower) {
            out.push(SymbolInformation {
                name: class.name.clone(),
                kind: SymbolKind::CLASS,
                location: Location {
                    uri: uri.clone(),
                    range: span_to_range(source, class.name_span),
                },
                container_name: None,
                tags: None,
                deprecated: None,
            });
        }
        for attr in &class.attributes {
            if matches_query(&attr.name, query_lower) {
                out.push(SymbolInformation {
                    name: attr.name.clone(),
                    kind: SymbolKind::FIELD,
                    location: Location {
                        uri: uri.clone(),
                        range: span_to_range(source, attr.name_span),
                    },
                    container_name: Some(class.name.clone()),
                    tags: None,
                    deprecated: None,
                });
            }
        }
    }

    for func in &resolved.functions {
        if matches_query(&func.name, query_lower) {
            let (kind, container_name) = match &func.class_name {
                Some(class_name) => (SymbolKind::METHOD, Some(class_name.clone())),
                None => (SymbolKind::FUNCTION, None),
            };
            out.push(SymbolInformation {
                name: func.name.clone(),
                kind,
                location: Location {
                    uri: uri.clone(),
                    range: span_to_range(source, func.name_span),
                },
                container_name,
                tags: None,
                deprecated: None,
            });
        }
    }

    for var in &resolved.module_vars {
        if matches_query(&var.name, query_lower) {
            out.push(SymbolInformation {
                name: var.name.clone(),
                kind: SymbolKind::VARIABLE,
                location: Location {
                    uri: uri.clone(),
                    range: span_to_range(source, var.name_span),
                },
                container_name: None,
                tags: None,
                deprecated: None,
            });
        }
    }
}

/// Returns `true` if `name` contains `query_lower` (case-insensitive).
/// Always returns `true` when `query_lower` is empty.
fn matches_query(name: &str, query_lower: &str) -> bool {
    query_lower.is_empty() || name.to_lowercase().contains(query_lower)
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
