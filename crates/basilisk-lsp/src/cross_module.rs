//! Implements [ANALYSIS-CROSSLSP]. See docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-CROSSLSP
//!
//! Cross-module symbol resolution.
//!
//! Populates `ResolvedModule.imported_symbols` by looking up each import's
//! `resolved_path` in the workspace index and extracting exported symbols
//! from the target module.

use std::path::PathBuf;
use std::sync::Arc;

use basilisk_resolver::scope::{ExternalSymbol, ExternalSymbolKind};
use basilisk_resolver::Span;
use basilisk_stubs::types::{StubFunction, StubSource, StubTier};
use basilisk_stubs::TypeProvenance;

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
                provenance: Some(TypeProvenance::Source),
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
                provenance: Some(TypeProvenance::Source),
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
                provenance: Some(TypeProvenance::Source),
            },
        ));
    }

    exports
}

/// Extract exported symbols from a `.pyi` stub file.
///
/// Parses the stub and converts its functions, classes, and variables into
/// `ExternalSymbol` entries with stub provenance, so imports that resolve to a
/// stub (typeshed, `*-stubs` packages, or user stubs) carry real type
/// information instead of nothing. Returns an empty vec if the stub cannot be
/// parsed. Implements [ANALYSIS-CROSSLSP].
fn extract_stub_exports(
    stub_path: &std::path::Path,
    module_name: &str,
) -> Vec<(String, ExternalSymbol)> {
    // `.pyi` stubs (typeshed, `*-stubs`, user stubs) are hand-written, verified
    // types — Tier1. Source/tier only affect provenance via the Tier mapping.
    let Ok(stub) = basilisk_stubs::parse_pyi_file(
        stub_path,
        module_name,
        StubSource::StubPackage,
        StubTier::Tier1,
    ) else {
        return Vec::new();
    };

    let provenance = Some(TypeProvenance::StubTier1);
    let mut exports = Vec::new();

    for func in stub.functions.values() {
        exports.push((
            func.name.clone(),
            ExternalSymbol {
                name: func.name.clone(),
                kind: ExternalSymbolKind::Function,
                type_annotation: func.return_type.clone(),
                source_path: stub_path.to_path_buf(),
                source_span: Span::new(0, 0),
                signature: Some(build_stub_signature(func)),
                provenance,
            },
        ));
    }

    for class in stub.classes.values() {
        exports.push((
            class.name.clone(),
            ExternalSymbol {
                name: class.name.clone(),
                kind: ExternalSymbolKind::Class,
                type_annotation: None,
                source_path: stub_path.to_path_buf(),
                source_span: Span::new(0, 0),
                signature: Some(format!("class {}", class.name)),
                provenance,
            },
        ));
    }

    for var in stub.variables.values() {
        exports.push((
            var.name.clone(),
            ExternalSymbol {
                name: var.name.clone(),
                kind: ExternalSymbolKind::Variable,
                type_annotation: var.annotation.clone(),
                source_path: stub_path.to_path_buf(),
                source_span: Span::new(0, 0),
                signature: None,
                provenance,
            },
        ));
    }

    exports
}

/// Extract exported symbols from an external `.py` package that ships inline
/// PEP 561 types (a `py.typed` marker).
///
/// Parses and resolves the file, then reuses [`extract_exports`]. Callers MUST
/// gate this on [`basilisk_stubs::has_py_typed_marker`] — a package without the
/// marker has not opted in to inline type distribution, so its annotations must
/// not be trusted. Returns an empty vec if the file cannot be read or parsed.
/// Implements [ANALYSIS-CROSSLSP].
fn extract_py_typed_exports(py_path: &std::path::Path) -> Vec<(String, ExternalSymbol)> {
    let Ok(text) = std::fs::read_to_string(py_path) else {
        return Vec::new();
    };
    let path_str = py_path.to_string_lossy().into_owned();
    let Ok(parsed) = basilisk_parser::parse_source(text, path_str) else {
        return Vec::new();
    };
    let Ok(resolved) = basilisk_resolver::resolve(&parsed) else {
        return Vec::new();
    };
    extract_exports(&resolved, py_path)
}

/// Build a function signature string from a parsed stub function.
fn build_stub_signature(func: &StubFunction) -> String {
    let mut sig = format!("def {}(", func.name);
    for (idx, param) in func.params.iter().enumerate() {
        if idx > 0 {
            sig.push_str(", ");
        }
        sig.push_str(&param.name);
        if let Some(annotation) = &param.annotation {
            sig.push_str(": ");
            sig.push_str(annotation);
        }
    }
    sig.push(')');
    if let Some(return_type) = &func.return_type {
        sig.push_str(" -> ");
        sig.push_str(return_type);
    }
    sig
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

    for entry in &index.files {
        let path = entry.key().clone();
        if let Some(resolved) = &entry.resolved {
            let exports = extract_exports(resolved, &path);
            let _ = all_exports.insert(path, exports);
        }
    }

    // Third-party type sources resolved outside the workspace — `.pyi` stubs
    // (typeshed, `*-stubs`, user stubs) and inline-typed `py.typed` packages —
    // are not in `index.files`, so they never appear in `all_exports`. Parse
    // them on demand and cache by path to avoid re-parsing per importer.
    let mut external_exports: std::collections::HashMap<PathBuf, Vec<(String, ExternalSymbol)>> =
        std::collections::HashMap::new();

    // Second pass: for each file, populate imported_symbols from resolved imports.
    for mut entry in index.files.iter_mut() {
        let Some(resolved_arc) = entry.value_mut().resolved.take() else {
            continue;
        };

        let mut resolved = Arc::try_unwrap(resolved_arc).unwrap_or_else(|arc| (*arc).clone());

        for import in &resolved.imports {
            let Some(resolved_path) = &import.resolved_path else {
                continue;
            };

            // Prefer workspace exports; otherwise parse an external `.pyi` stub
            // or an inline-typed `py.typed` package (PEP 561 opt-in only).
            let target_exports = if let Some(exports) = all_exports.get(resolved_path) {
                exports
            } else if resolved_path.extension().is_some_and(|ext| ext == "pyi") {
                external_exports
                    .entry(resolved_path.clone())
                    .or_insert_with(|| extract_stub_exports(resolved_path, &import.module))
            } else if resolved_path.extension().is_some_and(|ext| ext == "py")
                && basilisk_stubs::has_py_typed_marker(resolved_path)
            {
                external_exports
                    .entry(resolved_path.clone())
                    .or_insert_with(|| extract_py_typed_exports(resolved_path))
            } else {
                continue;
            };

            if import.names.is_empty() {
                // `import foo` — make all exports available under the module name.
                // We store them individually for now.
                for (name, symbol) in target_exports {
                    let _ = resolved
                        .imported_symbols
                        .insert(name.clone(), symbol.clone());
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
                    }
                }
            }
        }

        entry.value_mut().resolved = Some(Arc::new(resolved));
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test-only code: unwrap/expect acceptable in unit tests"
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
        let index = WorkspaceIndex::new(
            vec![],
            AnalysisMode::CrossModule,
            basilisk_config::BasiliskConfig::default(),
        );

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
        let greet_sym = resolved_b
            .imported_symbols
            .get("greet")
            .expect("greet should exist");
        assert_eq!(greet_sym.kind, ExternalSymbolKind::Function);
        assert_eq!(greet_sym.source_path, path_a);
    }

    /// Stage 1 of stub type consumption (issue: "can't get it to pick up stubs"):
    /// when an import resolves to a `.pyi` stub that is NOT a workspace file, the
    /// importing module must still receive the stub's symbols (with stub
    /// provenance) so hover/type-checking get real types. Before this, stubs were
    /// discovered (`StubPyi`) but never parsed, so `imported_symbols` stayed empty.
    #[test]
    fn cross_module_imports_symbols_from_pyi_stub() {
        let dir = std::env::temp_dir().join(format!("bsk_xmod_stub_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let stub_path = dir.join("acme.pyi");
        std::fs::write(
            &stub_path,
            "def fetch(url: str) -> bytes: ...\nVERSION: str\n",
        )
        .unwrap();

        let index = WorkspaceIndex::new(
            vec![],
            AnalysisMode::CrossModule,
            basilisk_config::BasiliskConfig::default(),
        );

        // File B imports from `acme`, which resolves to an external `.pyi` stub
        // (e.g. an installed `acme-stubs` package) — NOT a workspace file.
        let uri_b = make_uri("/tmp/xmod_stub_b.py");
        let src_b = "from acme import fetch\n\nx = fetch('u')\n";
        let _ = index.set_open(&uri_b, src_b, 1);
        let path_b = uri_b.to_file_path().unwrap();

        if let Some(mut entry) = index.files.get_mut(&path_b) {
            if let Some(resolved_arc) = entry.resolved.take() {
                let mut resolved =
                    Arc::try_unwrap(resolved_arc).unwrap_or_else(|arc| (*arc).clone());
                for import in &mut resolved.imports {
                    if import.module == "acme" {
                        import.resolved_path = Some(stub_path.clone());
                        import.resolution = basilisk_resolver::scope::ImportResolution::StubPyi;
                    }
                }
                entry.resolved = Some(Arc::new(resolved));
            }
        }

        populate_cross_module_symbols(&index);

        let entry_b = index.files.get(&path_b).unwrap();
        let resolved_b = entry_b.resolved.as_ref().unwrap();
        let fetch_sym = resolved_b
            .imported_symbols
            .get("fetch")
            .expect("`fetch` from the .pyi stub should be in imported_symbols");
        assert_eq!(fetch_sym.kind, ExternalSymbolKind::Function);
        assert_eq!(
            fetch_sym.type_annotation.as_deref(),
            Some("bytes"),
            "stub return type must be picked up"
        );
        assert_eq!(
            fetch_sym.provenance,
            Some(TypeProvenance::StubTier1),
            "symbols from a .pyi stub must carry stub provenance, not Source"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Stage 2 of stub type consumption: a third-party package that ships inline
    /// types via a PEP 561 `py.typed` marker (its code is `.py`, not `.pyi`) must
    /// have its symbols consumed too. Most modern libraries (pydantic, httpx,
    /// rich) are this case. Gated on `py.typed` — an untyped package's
    /// annotations must NOT be trusted (PEP 561).
    #[test]
    fn cross_module_imports_symbols_from_py_typed_package() {
        let sp = std::env::temp_dir().join(format!("bsk_xmod_pytyped_{}", std::process::id()));
        let pkg = sp.join("site-packages").join("widgets");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(pkg.join("py.typed"), "").unwrap();
        let init_py = pkg.join("__init__.py");
        std::fs::write(
            &init_py,
            "def render(node: int) -> str:\n    return str(node)\n",
        )
        .unwrap();

        let index = WorkspaceIndex::new(
            vec![],
            AnalysisMode::CrossModule,
            basilisk_config::BasiliskConfig::default(),
        );

        let uri_b = make_uri("/tmp/xmod_pytyped_b.py");
        let src_b = "from widgets import render\n\ny = render(1)\n";
        let _ = index.set_open(&uri_b, src_b, 1);
        let path_b = uri_b.to_file_path().unwrap();

        if let Some(mut entry) = index.files.get_mut(&path_b) {
            if let Some(resolved_arc) = entry.resolved.take() {
                let mut resolved =
                    Arc::try_unwrap(resolved_arc).unwrap_or_else(|arc| (*arc).clone());
                for import in &mut resolved.imports {
                    if import.module == "widgets" {
                        import.resolved_path = Some(init_py.clone());
                        import.resolution = basilisk_resolver::scope::ImportResolution::SourcePy;
                    }
                }
                entry.resolved = Some(Arc::new(resolved));
            }
        }

        populate_cross_module_symbols(&index);

        let entry_b = index.files.get(&path_b).unwrap();
        let resolved_b = entry_b.resolved.as_ref().unwrap();
        let render_sym = resolved_b
            .imported_symbols
            .get("render")
            .expect("`render` from the py.typed package should be in imported_symbols");
        assert_eq!(render_sym.kind, ExternalSymbolKind::Function);
        assert_eq!(
            render_sym.type_annotation.as_deref(),
            Some("str"),
            "py.typed package's inline return type must be picked up"
        );

        let _ = std::fs::remove_dir_all(&sp);
    }

    /// A site-packages `.py` package WITHOUT a `py.typed` marker must NOT have its
    /// annotations consumed (PEP 561: inline types are opt-in). This keeps
    /// untyped third-party packages untyped so BSK-W0010 still applies.
    #[test]
    fn cross_module_skips_non_py_typed_package() {
        let sp = std::env::temp_dir().join(format!("bsk_xmod_untyped_{}", std::process::id()));
        let pkg = sp.join("site-packages").join("legacy");
        std::fs::create_dir_all(&pkg).unwrap();
        // No py.typed marker.
        let init_py = pkg.join("__init__.py");
        std::fs::write(&init_py, "def helper(x: int) -> int:\n    return x\n").unwrap();

        let index = WorkspaceIndex::new(
            vec![],
            AnalysisMode::CrossModule,
            basilisk_config::BasiliskConfig::default(),
        );

        let uri_b = make_uri("/tmp/xmod_untyped_b.py");
        let src_b = "from legacy import helper\n\nz = helper(1)\n";
        let _ = index.set_open(&uri_b, src_b, 1);
        let path_b = uri_b.to_file_path().unwrap();

        if let Some(mut entry) = index.files.get_mut(&path_b) {
            if let Some(resolved_arc) = entry.resolved.take() {
                let mut resolved =
                    Arc::try_unwrap(resolved_arc).unwrap_or_else(|arc| (*arc).clone());
                for import in &mut resolved.imports {
                    if import.module == "legacy" {
                        import.resolved_path = Some(init_py.clone());
                        import.resolution = basilisk_resolver::scope::ImportResolution::SourcePy;
                    }
                }
                entry.resolved = Some(Arc::new(resolved));
            }
        }

        populate_cross_module_symbols(&index);

        let entry_b = index.files.get(&path_b).unwrap();
        let resolved_b = entry_b.resolved.as_ref().unwrap();
        assert!(
            !resolved_b.imported_symbols.contains_key("helper"),
            "symbols from a non-py.typed package must NOT be consumed (PEP 561)"
        );

        let _ = std::fs::remove_dir_all(&sp);
    }

    #[test]
    fn cross_module_class_symbol() {
        let index = WorkspaceIndex::new(
            vec![],
            AnalysisMode::CrossModule,
            basilisk_config::BasiliskConfig::default(),
        );

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
        let dog_sym = resolved_b
            .imported_symbols
            .get("Dog")
            .expect("Dog should exist");
        assert_eq!(dog_sym.kind, ExternalSymbolKind::Class);
    }
}
