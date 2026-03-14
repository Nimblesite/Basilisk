//! Cross-module symbol resolution.
//!
//! Populates `ResolvedModule.imported_symbols` by looking up each import's
//! `resolved_path` in the workspace index and extracting exported symbols
//! from the target module.

use std::path::PathBuf;
use std::sync::Arc;

use basilisk_resolver::scope::{ExternalSymbol, ExternalSymbolKind};

use crate::workspace::WorkspaceIndex;

/// Extract exported symbols from a `ResolvedModule`.
///
/// Returns all public functions, classes, and variables as `ExternalSymbol`
/// entries keyed by their name.
fn extract_exports(
    resolved: &basilisk_resolver::ResolvedModule,
    source_path: &std::path::Path,
) -> Vec<(String, ExternalSymbol)> {
    let mut exports = Vec::new();

    // Functions
    for func in &resolved.functions {
        let signature = build_function_signature(func, &resolved.source);
        let return_type = func
            .return_annotation_span
            .as_ref()
            .and_then(|span| span.slice_source(&resolved.source))
            .map(String::from);
        exports.push((
            func.name.clone(),
            ExternalSymbol {
                name: func.name.clone(),
                kind: ExternalSymbolKind::Function,
                type_annotation: return_type,
                source_path: source_path.to_path_buf(),
                source_span: func.name_span,
                signature: Some(signature),
            },
        ));
    }

    // Classes
    for class in &resolved.classes {
        exports.push((
            class.name.clone(),
            ExternalSymbol {
                name: class.name.clone(),
                kind: ExternalSymbolKind::Class,
                type_annotation: None,
                source_path: source_path.to_path_buf(),
                source_span: class.name_span,
                signature: Some(format!("class {}", class.name)),
            },
        ));
    }

    // Module-level variables
    for var in &resolved.module_vars {
        let type_text = var
            .annotation_span
            .as_ref()
            .and_then(|span| span.slice_source(&resolved.source))
            .map(String::from);
        exports.push((
            var.name.clone(),
            ExternalSymbol {
                name: var.name.clone(),
                kind: ExternalSymbolKind::Variable,
                type_annotation: type_text,
                source_path: source_path.to_path_buf(),
                source_span: var.name_span,
                signature: None,
            },
        ));
    }

    exports
}

/// Build a function signature string for hover display.
fn build_function_signature(func: &basilisk_resolver::scope::FunctionInfo, source: &str) -> String {
    let mut sig = format!("def {}(", func.name);
    for (idx, param) in func.parameters.iter().enumerate() {
        if idx > 0 {
            sig.push_str(", ");
        }
        sig.push_str(&param.name);
        if let Some(ann_span) = &param.annotation_span {
            if let Some(ann_text) = ann_span.slice_source(source) {
                sig.push_str(": ");
                sig.push_str(ann_text);
            }
        }
    }
    sig.push(')');
    if let Some(ret_span) = &func.return_annotation_span {
        if let Some(ret_text) = ret_span.slice_source(source) {
            sig.push_str(" -> ");
            sig.push_str(ret_text);
        }
    }
    sig
}

/// Populate `imported_symbols` for all files in the workspace index.
///
/// For each file, walks its `imports`, looks up the `resolved_path` in the
/// index, and extracts the target module's exported symbols. Only imports
/// that resolved to files present in the index are populated.
pub fn populate_cross_module_symbols(index: &WorkspaceIndex) {
    // First pass: collect all exports keyed by path.
    let mut all_exports: std::collections::HashMap<PathBuf, Vec<(String, ExternalSymbol)>> =
        std::collections::HashMap::new();

    for entry in index.files.iter() {
        let path = entry.key().clone();
        if let Some(resolved) = &entry.resolved {
            let exports = extract_exports(resolved, &path);
            let _ = all_exports.insert(path, exports);
        }
    }

    // Second pass: for each file, populate imported_symbols from resolved imports.
    for mut entry in index.files.iter_mut() {
        let Some(resolved_arc) = entry.value_mut().resolved.take() else {
            continue;
        };

        let mut resolved = Arc::try_unwrap(resolved_arc).unwrap_or_else(|arc| (*arc).clone());
        let mut changed = false;

        for import in &resolved.imports {
            let Some(resolved_path) = &import.resolved_path else {
                continue;
            };

            let Some(target_exports) = all_exports.get(resolved_path) else {
                continue;
            };

            if import.names.is_empty() {
                // `import foo` — make all exports available under the module name.
                // We store them individually for now.
                for (name, symbol) in target_exports {
                    let _ = resolved
                        .imported_symbols
                        .insert(name.clone(), symbol.clone());
                    changed = true;
                }
            } else {
                // `from foo import bar, baz` — only import named symbols.
                for import_name in &import.names {
                    if let Some((_, symbol)) =
                        target_exports.iter().find(|(name, _)| name == import_name)
                    {
                        let _ = resolved
                            .imported_symbols
                            .insert(import_name.clone(), symbol.clone());
                        changed = true;
                    }
                }
            }
        }

        // Only create a new Arc if we actually added symbols.
        if changed {
            entry.value_mut().resolved = Some(Arc::new(resolved));
        } else {
            // Reconstruct the Arc without modification.
            entry.value_mut().resolved = Some(Arc::new(resolved));
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only code: unwrap acceptable in unit tests"
)]
mod tests {
    use super::*;
    use crate::config::AnalysisMode;
    use crate::workspace::WorkspaceIndex;

    fn make_uri(path: &str) -> tower_lsp::lsp_types::Url {
        tower_lsp::lsp_types::Url::parse(&format!("file://{path}")).unwrap()
    }

    #[test]
    fn cross_module_symbol_population() {
        let index = WorkspaceIndex::new(vec![], AnalysisMode::CrossModule);

        // File A: defines a function
        let uri_a = make_uri("/tmp/cross_a.py");
        let src_a = "def greet(name: str) -> str:\n    return f'Hello {name}'\n";
        let _ = index.set_open(&uri_a, src_a, 1);

        // File B: imports from A
        let uri_b = make_uri("/tmp/cross_b.py");
        let src_b = "from cross_a import greet\n\nx: str = greet('world')\n";
        let _ = index.set_open(&uri_b, src_b, 1);

        // Manually set up the resolved_path for B's import to point to A
        let path_a = uri_a.to_file_path().unwrap();
        let path_b = uri_b.to_file_path().unwrap();

        if let Some(mut entry) = index.files.get_mut(&path_b) {
            if let Some(resolved_arc) = entry.resolved.take() {
                let mut resolved =
                    Arc::try_unwrap(resolved_arc).unwrap_or_else(|arc| (*arc).clone());
                for import in &mut resolved.imports {
                    if import.module == "cross_a" {
                        import.resolved_path = Some(path_a.clone());
                    }
                }
                entry.resolved = Some(Arc::new(resolved));
            }
        }

        // Run cross-module population
        populate_cross_module_symbols(&index);

        // Verify: B should now have `greet` in its imported_symbols
        let entry_b = index.files.get(&path_b).unwrap();
        let resolved_b = entry_b.resolved.as_ref().unwrap();
        assert!(
            resolved_b.imported_symbols.contains_key("greet"),
            "greet should be in imported_symbols"
        );
        let greet_sym = &resolved_b.imported_symbols["greet"];
        assert_eq!(greet_sym.kind, ExternalSymbolKind::Function);
        assert_eq!(greet_sym.source_path, path_a);
    }

    #[test]
    fn cross_module_class_symbol() {
        let index = WorkspaceIndex::new(vec![], AnalysisMode::CrossModule);

        let uri_a = make_uri("/tmp/cross_cls_a.py");
        let src_a =
            "class Dog:\n    name: str\n    def bark(self) -> str:\n        return 'woof'\n";
        let _ = index.set_open(&uri_a, src_a, 1);

        let uri_b = make_uri("/tmp/cross_cls_b.py");
        let src_b = "from cross_cls_a import Dog\n\nd: Dog = Dog()\n";
        let _ = index.set_open(&uri_b, src_b, 1);

        let path_a = uri_a.to_file_path().unwrap();
        let path_b = uri_b.to_file_path().unwrap();

        if let Some(mut entry) = index.files.get_mut(&path_b) {
            if let Some(resolved_arc) = entry.resolved.take() {
                let mut resolved =
                    Arc::try_unwrap(resolved_arc).unwrap_or_else(|arc| (*arc).clone());
                for import in &mut resolved.imports {
                    if import.module == "cross_cls_a" {
                        import.resolved_path = Some(path_a.clone());
                    }
                }
                entry.resolved = Some(Arc::new(resolved));
            }
        }

        populate_cross_module_symbols(&index);

        let entry_b = index.files.get(&path_b).unwrap();
        let resolved_b = entry_b.resolved.as_ref().unwrap();
        assert!(resolved_b.imported_symbols.contains_key("Dog"));
        let dog_sym = &resolved_b.imported_symbols["Dog"];
        assert_eq!(dog_sym.kind, ExternalSymbolKind::Class);
    }
}
