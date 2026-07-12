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

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use basilisk_resolver::scope::{ExternalMethod, ExternalSymbol, ExternalSymbolKind, ImportKind};
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
                methods: Vec::new(),
            },
        ));
    }

    for class in &resolved.classes {
        // Keep the class's methods so importers can hover inherited member
        // access (GitHub #287). `resolved.functions` holds methods too, tagged
        // with their enclosing class name.
        let methods = resolved
            .functions
            .iter()
            .filter(|func| func.class_name.as_deref() == Some(class.name.as_str()))
            .map(|func| ExternalMethod {
                name: func.name.clone(),
                signature: build_function_signature(func, &resolved.source),
            })
            .collect();
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
                methods,
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
                methods: Vec::new(),
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
///
/// `source` records **where** the stub came from so provenance stays honest: a
/// stub resolved from a custom typeshed (`typeshed-path`) carries
/// [`TypeProvenance::StubCustomTypeshed`] and hover reads `(custom typeshed)`,
/// while `*-stubs`/user stubs stay [`TypeProvenance::StubTier1`]
/// ([STUBRES-CUSTOM-TYPESHED]). All `.pyi` stubs are hand-written, verified
/// types — Tier1 — so the tier is fixed and only the source varies.
#[must_use]
pub fn extract_stub_exports(
    stub_path: &Path,
    module_name: &str,
    source: StubSource,
) -> Vec<(String, ExternalSymbol)> {
    let Ok(stub) = basilisk_stubs::parse_pyi_file(stub_path, module_name, source, StubTier::Tier1)
    else {
        return Vec::new();
    };

    let provenance = Some(TypeProvenance::from((&source, &StubTier::Tier1)));
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
                methods: Vec::new(),
            },
        ));
    }

    for class in stub.classes.values() {
        // Keep the stub class's methods so importers can hover inherited
        // member access (GitHub #287).
        let methods = class
            .methods
            .iter()
            .map(|method| ExternalMethod {
                name: method.name.clone(),
                signature: build_stub_signature(method),
            })
            .collect();
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
                methods,
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
                methods: Vec::new(),
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

/// Resolve `name` through the re-export chain of a `py.typed` module.
///
/// A package `__init__.py` often defines nothing itself and re-exports its
/// public names from submodules — pydantic v2 exposes `BaseModel` only via
/// `if TYPE_CHECKING: from .main import *` (runtime uses a lazy module
/// `__getattr__`). When the resolved file's own definitions miss `name`,
/// follow its module-level `from … import name` and `from … import *`
/// statements into the sibling module file and take the symbol — with its
/// methods — from there (GitHub #287). `visited` breaks import cycles.
fn chase_py_typed_reexport(
    py_path: &Path,
    name: &str,
    external_cache: &mut HashMap<PathBuf, Vec<(String, ExternalSymbol)>>,
    visited: &mut HashSet<PathBuf>,
) -> Option<ExternalSymbol> {
    if !visited.insert(py_path.to_path_buf()) {
        return None;
    }
    let text = std::fs::read_to_string(py_path).ok()?;
    let parsed =
        basilisk_parser::parse_source(text, py_path.to_string_lossy().into_owned()).ok()?;
    let resolved = basilisk_resolver::resolve(&parsed).ok()?;
    let dir = py_path.parent()?;
    resolved
        .imports
        .iter()
        .filter(|import| match import.kind {
            ImportKind::From => import.names.iter().any(|n| n == name),
            ImportKind::Star => true,
            ImportKind::Plain => false,
        })
        .filter_map(|import| sibling_module_file(dir, &import.module))
        .find_map(|target| {
            let direct = external_cache
                .entry(target.clone())
                .or_insert_with(|| extract_py_typed_exports(&target))
                .iter()
                .find(|(export_name, _)| export_name == name)
                .map(|(_, symbol)| symbol.clone());
            direct.or_else(|| chase_py_typed_reexport(&target, name, external_cache, visited))
        })
}

/// Map a `from X import …` module inside a package to a sibling file.
///
/// Relative dots are dropped during import collection, so `.main` arrives as
/// `main` and resolves against the importing file's directory; an absolute
/// self-import (`pydantic.main`) resolves by stripping the package's own name.
fn sibling_module_file(dir: &Path, module: &str) -> Option<PathBuf> {
    let package_prefix = dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| format!("{n}."));
    let in_package = package_prefix
        .as_deref()
        .and_then(|prefix| module.strip_prefix(prefix));
    [Some(module), in_package]
        .into_iter()
        .flatten()
        .find_map(|candidate| {
            let base = dir.join(candidate.split('.').collect::<PathBuf>());
            let file = base.with_extension("py");
            if file.is_file() {
                return Some(file);
            }
            let init = base.join("__init__.py");
            init.is_file().then_some(init)
        })
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

/// Classify an external `.pyi` stub's [`StubSource`] from its on-disk location.
///
/// A stub under the configured custom typeshed's `stdlib/` subtree is
/// [`StubSource::CustomTypeshed`] ([STUBRES-CUSTOM-TYPESHED]); every other stub
/// (`*-stubs` packages, on-demand typeshed) is [`StubSource::StubPackage`].
/// Provenance flows from here to hover via [`TypeProvenance`].
fn stub_source_for(resolved_path: &Path, custom_typeshed: Option<&Path>) -> StubSource {
    match custom_typeshed {
        Some(typeshed) if resolved_path.starts_with(typeshed.join("stdlib")) => {
            StubSource::CustomTypeshed
        }
        _ => StubSource::StubPackage,
    }
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
///
/// `custom_typeshed` is the configured `typeshed-path`, if any: a
/// stub resolved from its `stdlib/` subtree is tagged
/// [`StubSource::CustomTypeshed`] so hover reads `(custom typeshed)` and a
/// `MicroPython` signature is never reported as the bundled `CPython` one
/// ([STUBRES-CUSTOM-TYPESHED]). Pass `None` for the default bundled typeshed.
pub fn populate_imported_symbols<'a, F>(
    resolved: &mut basilisk_resolver::ResolvedModule,
    mut workspace_exports: F,
    custom_typeshed: Option<&Path>,
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
        let mut py_typed_source = false;
        let target_exports: &[(String, ExternalSymbol)] = if let Some(exports) =
            workspace_exports(resolved_path)
        {
            exports
        } else if resolved_path.extension().is_some_and(|ext| ext == "pyi") {
            // Classify the stub's provenance: a `.pyi` under the configured
            // custom typeshed's `stdlib/` is CustomTypeshed
            // ([STUBRES-CUSTOM-TYPESHED]); every other stub is StubPackage/Tier1.
            let stub_source = stub_source_for(resolved_path, custom_typeshed);
            external_cache
                .entry(resolved_path.clone())
                .or_insert_with(|| extract_stub_exports(resolved_path, &import.module, stub_source))
        } else if resolved_path.extension().is_some_and(|ext| ext == "py")
            && basilisk_stubs::has_py_typed_marker(resolved_path)
        {
            py_typed_source = true;
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
            let direct: Vec<(String, Option<ExternalSymbol>)> = import
                .names
                .iter()
                .map(|import_name| {
                    let symbol = target_exports
                        .iter()
                        .find(|(name, _)| name == import_name)
                        .map(|(_, symbol)| symbol.clone());
                    (import_name.clone(), symbol)
                })
                .collect();
            for (import_name, symbol) in direct {
                // A `py.typed` package `__init__.py` may only *re-export* the
                // name from a submodule (pydantic v2) — chase it (GitHub #287).
                let symbol = symbol.or_else(|| {
                    py_typed_source
                        .then(|| {
                            chase_py_typed_reexport(
                                resolved_path,
                                &import_name,
                                &mut external_cache,
                                &mut HashSet::new(),
                            )
                        })
                        .flatten()
                });
                if let Some(symbol) = symbol {
                    let _ = imported_symbols.insert(import_name, symbol);
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
