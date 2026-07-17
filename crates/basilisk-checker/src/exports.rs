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

use std::collections::HashSet;
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

/// A parsed external (non-workspace) type-bearing module: its exports plus
/// the re-export edges the `py.typed` chase follows.
///
/// Built by [`load_external_module`] from **one** read+parse of the on-disk
/// file, and memoized per path by the salsa layer
/// ([`crate::incremental::external_module`]) so a workspace scan parses each
/// external module once — not once per workspace file per imported name,
/// which pinned a CPU core for hours on large workspaces (GitHub #304).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExternalModule {
    /// The module's exported symbols.
    pub exports: Vec<(String, ExternalSymbol)>,
    /// Module-level `from … import` edges into sibling module files, for the
    /// `py.typed` re-export chase. Empty for `.pyi` stubs (no chase).
    pub reexports: Vec<ReexportEdge>,
}

/// One `from … import` edge of a `py.typed` module, pre-resolved to the
/// sibling file it points at.
#[derive(Debug, Clone, PartialEq)]
pub struct ReexportEdge {
    /// The binding names the edge re-exports, or `None` for `import *`.
    pub names: Option<Vec<String>>,
    /// The sibling module file the edge resolves to.
    pub target: PathBuf,
}

/// What to load from an external import target's on-disk file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExternalModuleRequest {
    /// An inline-typed (PEP 561 `py.typed`) package module. Callers MUST gate
    /// this on [`basilisk_stubs::has_py_typed_marker`] — a package without the
    /// marker has not opted in to inline type distribution.
    PyTyped,
    /// A `.pyi` stub, with the importing module name and stub provenance.
    Stub {
        /// The module name the import refers to (drives stub parsing).
        module_name: String,
        /// Where the stub came from, for honest hover provenance.
        source: StubSource,
    },
}

/// A shared handle to an external module's parsed view.
pub type SharedExternalModule = std::sync::Arc<ExternalModule>;

/// Load one external module's exports and re-export edges from disk.
///
/// This is the **uncached** producer: one read + parse + resolve per call. The
/// salsa layer memoizes it per `(path, request)`
/// ([`crate::incremental::external_module`]) so consumers share the work;
/// callers outside the engine (tests) may use it directly as the
/// `external_module` argument of [`populate_imported_symbols`]. A file that
/// cannot be read, parsed, or resolved yields an empty module. Implements
/// [ANALYSIS-CROSSLSP].
#[must_use]
pub fn load_external_module(path: &Path, request: &ExternalModuleRequest) -> SharedExternalModule {
    match request {
        ExternalModuleRequest::Stub {
            module_name,
            source,
        } => std::sync::Arc::new(ExternalModule {
            exports: extract_stub_exports(path, module_name, *source),
            reexports: Vec::new(),
        }),
        ExternalModuleRequest::PyTyped => std::sync::Arc::new(load_py_typed_module(path)),
    }
}

/// Load a `py.typed` module: exports plus chase edges, from one parse.
fn load_py_typed_module(py_path: &Path) -> ExternalModule {
    let Ok(text) = std::fs::read_to_string(py_path) else {
        return ExternalModule::default();
    };
    let path_str = py_path.to_string_lossy().into_owned();
    let Ok(parsed) = basilisk_parser::parse_source(text, path_str) else {
        return ExternalModule::default();
    };
    let Ok(resolved) = basilisk_resolver::resolve(&parsed) else {
        return ExternalModule::default();
    };
    let reexports = py_path
        .parent()
        .map(|dir| {
            resolved
                .imports
                .iter()
                .filter_map(|import| {
                    let names = match import.kind {
                        ImportKind::From => Some(Some(import.names.clone())),
                        ImportKind::Star => Some(None),
                        ImportKind::Plain => None,
                    }?;
                    let target = sibling_module_file(dir, &import.module)?;
                    Some(ReexportEdge { names, target })
                })
                .collect()
        })
        .unwrap_or_default();
    ExternalModule {
        exports: extract_exports(&resolved, py_path),
        reexports,
    }
}

/// Resolve `name` through the re-export chain of a `py.typed` module.
///
/// A package `__init__.py` often defines nothing itself and re-exports its
/// public names from submodules — pydantic v2 exposes `BaseModel` only via
/// `if TYPE_CHECKING: from .main import *` (runtime uses a lazy module
/// `__getattr__`). When the module's own exports miss `name`, follow its
/// `from … import name` and `from … import *` edges into the sibling module
/// and take the symbol — with its methods — from there (GitHub #287).
/// `visited` breaks import cycles. Every module is fetched through the shared
/// `external_module` lookup, so the chase never re-parses a file (GitHub #304).
fn chase_py_typed_reexport<E>(
    py_path: &Path,
    name: &str,
    external_module: &mut E,
    visited: &mut HashSet<PathBuf>,
) -> Option<ExternalSymbol>
where
    E: FnMut(&Path, &ExternalModuleRequest) -> SharedExternalModule,
{
    if !visited.insert(py_path.to_path_buf()) {
        return None;
    }
    let module = external_module(py_path, &ExternalModuleRequest::PyTyped);
    module
        .reexports
        .iter()
        .filter(|edge| {
            edge.names
                .as_ref()
                .is_none_or(|names| names.iter().any(|n| n == name))
        })
        .find_map(|edge| {
            let direct = external_module(&edge.target, &ExternalModuleRequest::PyTyped)
                .exports
                .iter()
                .find(|(export_name, _)| export_name == name)
                .map(|(_, symbol)| symbol.clone());
            direct.or_else(|| chase_py_typed_reexport(&edge.target, name, external_module, visited))
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
/// workspace fall back to `external_module`, the caller-supplied lookup for
/// external `.pyi` stubs and PEP 561 `py.typed` packages. The salsa engine
/// supplies the memoized [`crate::incremental::external_module`] query there,
/// so external modules are parsed once per workspace — not once per importing
/// file per name, which pinned a CPU core for hours on large workspaces
/// (GitHub #304). Non-`py.typed` `.py` packages are skipped — their
/// annotations must not be trusted (PEP 561 opt-in).
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
pub fn populate_imported_symbols<'a, F, E>(
    resolved: &mut basilisk_resolver::ResolvedModule,
    mut workspace_exports: F,
    mut external_module: E,
    custom_typeshed: Option<&Path>,
) where
    F: FnMut(&Path) -> Option<&'a [(String, ExternalSymbol)]>,
    E: FnMut(&Path, &ExternalModuleRequest) -> SharedExternalModule,
{
    let imports = &resolved.imports;
    let imported_symbols = &mut resolved.imported_symbols;
    imported_symbols.clear();

    for import in imports {
        let Some(resolved_path) = &import.resolved_path else {
            continue;
        };

        // Prefer workspace exports; otherwise load the external `.pyi` stub
        // or inline-typed `py.typed` package (PEP 561 opt-in only) through the
        // shared lookup.
        let mut py_typed_source = false;
        let external;
        let target_exports: &[(String, ExternalSymbol)] =
            if let Some(exports) = workspace_exports(resolved_path) {
                exports
            } else if resolved_path.extension().is_some_and(|ext| ext == "pyi") {
                // Classify the stub's provenance: a `.pyi` under the configured
                // custom typeshed's `stdlib/` is CustomTypeshed
                // ([STUBRES-CUSTOM-TYPESHED]); every other stub is StubPackage/Tier1.
                let request = ExternalModuleRequest::Stub {
                    module_name: import.module.clone(),
                    source: stub_source_for(resolved_path, custom_typeshed),
                };
                external = external_module(resolved_path, &request);
                &external.exports
            } else if resolved_path.extension().is_some_and(|ext| ext == "py")
                && basilisk_stubs::has_py_typed_marker(resolved_path)
            {
                py_typed_source = true;
                external = external_module(resolved_path, &ExternalModuleRequest::PyTyped);
                &external.exports
            } else {
                continue;
            };

        // Discriminate on `kind`, not `names.is_empty()`: a plain `import foo
        // as f` carries its alias in `names`, but the alias binds the module
        // object — it is not a member to look up in `foo`'s exports.
        if import.kind == ImportKind::From {
            // `from foo import bar, baz` — only import the named symbols.
            for import_name in &import.names {
                let symbol = target_exports
                    .iter()
                    .find(|(name, _)| name == import_name)
                    .map(|(_, symbol)| symbol.clone());
                // A `py.typed` package `__init__.py` may only *re-export* the
                // name from a submodule (pydantic v2) — chase it (GitHub #287).
                let symbol = symbol.or_else(|| {
                    py_typed_source
                        .then(|| {
                            chase_py_typed_reexport(
                                resolved_path,
                                import_name,
                                &mut external_module,
                                &mut HashSet::new(),
                            )
                        })
                        .flatten()
                });
                if let Some(symbol) = symbol {
                    let _ = imported_symbols.insert(import_name.clone(), symbol);
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
