//! Implements [ANALYSIS-SYMBOLS-EXT] / [ANALYSIS-SYMBOLS-POP]. See
//! docs/specs/LSP-ANALYSIS-MODES-SPEC.md#ANALYSIS-SYMBOLS-EXT
//!
//! Cross-module export extraction and `imported_symbols` population.
//!
//! Pure functions shared by the memoized cross-module salsa queries
//! ([`crate::incremental::module_exports`], [`crate::incremental::cross_resolved_module`])
//! — hoisted from the LSP's former per-index population pass so the checker owns
//! a single implementation of "what does a module export" and "which of those
//! exports does an import bind".

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use basilisk_resolver::scope::{ExternalSymbol, ExternalSymbolKind, ImportKind};
use basilisk_resolver::Span;
use basilisk_stubs::types::{StubFunction, StubSource, StubTier};
use basilisk_stubs::TypeProvenance;

/// Extract exported symbols from a `ResolvedModule`.
///
/// Returns all public functions, classes, and variables as `ExternalSymbol`
/// entries keyed by their name.
#[must_use]
pub fn extract_exports(
    resolved: &basilisk_resolver::ResolvedModule,
    source_path: &Path,
) -> Vec<(String, ExternalSymbol)> {
    let mut exports = Vec::new();

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

/// Extract exported symbols from a `.pyi` stub file on disk.
///
/// Parses the stub and converts its functions, classes, and variables into
/// `ExternalSymbol` entries with stub provenance, so imports that resolve to a
/// stub (typeshed, `*-stubs` packages, or user stubs) carry real type
/// information instead of nothing. Returns an empty vec if the stub cannot be
/// parsed. Implements [ANALYSIS-CROSSLSP].
#[must_use]
pub fn extract_stub_exports(
    stub_path: &Path,
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
#[must_use]
pub fn extract_py_typed_exports(py_path: &Path) -> Vec<(String, ExternalSymbol)> {
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

/// Repopulate `resolved.imported_symbols` from its resolved imports.
///
/// `workspace_exports` supplies the exports of a **workspace-tracked** file
/// (the caller decides the lookup — the salsa query resolves through the
/// memoized [`crate::incremental::module_exports`]); imports outside the
/// workspace fall back to on-demand parsing of external `.pyi` stubs and
/// PEP 561 `py.typed` packages from disk. Non-`py.typed` `.py` packages are
/// skipped — their annotations must not be trusted (PEP 561 opt-in).
///
/// Stale entries are cleared first so a renamed or removed export disappears
/// from importers — without this the old name lingers and keeps suppressing its
/// now-undefined references, leaving dependents green after an export edit
/// (GitHub #56).
pub fn populate_imported_symbols<'a, F>(
    resolved: &mut basilisk_resolver::ResolvedModule,
    mut workspace_exports: F,
) where
    F: FnMut(&Path) -> Option<&'a [(String, ExternalSymbol)]>,
{
    // External type sources (`.pyi` stubs, `py.typed` packages) are parsed on
    // demand and cached per path so multiple imports of one module parse once.
    let mut external_cache: HashMap<PathBuf, Vec<(String, ExternalSymbol)>> = HashMap::new();

    let imports = &resolved.imports;
    let imported_symbols = &mut resolved.imported_symbols;
    imported_symbols.clear();

    for import in imports {
        let Some(resolved_path) = &import.resolved_path else {
            continue;
        };

        // Prefer workspace exports; otherwise parse an external `.pyi` stub
        // or an inline-typed `py.typed` package (PEP 561 opt-in only).
        let target_exports: &[(String, ExternalSymbol)] =
            if let Some(exports) = workspace_exports(resolved_path) {
                exports
            } else if resolved_path.extension().is_some_and(|ext| ext == "pyi") {
                external_cache
                    .entry(resolved_path.clone())
                    .or_insert_with(|| extract_stub_exports(resolved_path, &import.module))
            } else if resolved_path.extension().is_some_and(|ext| ext == "py")
                && basilisk_stubs::has_py_typed_marker(resolved_path)
            {
                external_cache
                    .entry(resolved_path.clone())
                    .or_insert_with(|| extract_py_typed_exports(resolved_path))
            } else {
                continue;
            };

        // Discriminate on `kind`, not `names.is_empty()`: a plain `import foo
        // as f` carries its alias in `names`, but the alias binds the module
        // object — it is not a member to look up in `foo`'s exports.
        if import.kind == ImportKind::From {
            // `from foo import bar, baz` — only import the named symbols.
            for import_name in &import.names {
                if let Some((_, symbol)) =
                    target_exports.iter().find(|(name, _)| name == import_name)
                {
                    let _ = imported_symbols.insert(import_name.clone(), symbol.clone());
                }
            }
        } else {
            // `import foo`, `import foo as f`, or `from foo import *` — make all
            // of the module's exports available under their own names.
            for (name, symbol) in target_exports {
                let _ = imported_symbols.insert(name.clone(), symbol.clone());
            }
        }
    }
}
