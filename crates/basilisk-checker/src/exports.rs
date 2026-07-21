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

use basilisk_resolver::scope::{
    ExternalMethod, ExternalSymbol, ExternalSymbolKind, ImportKind, ImportedModuleApi,
};
use basilisk_resolver::Span;
use basilisk_stubs::types::{StubClass, StubModule, StubSource, StubTarget, StubTier};
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
                bases: Vec::new(),
                metaclass: None,
                metaclass_calls: Vec::new(),
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
                bases: class.bases.clone(),
                metaclass: class.metaclass_name.clone(),
                metaclass_calls: Vec::new(),
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
                bases: Vec::new(),
                metaclass: None,
                metaclass_calls: Vec::new(),
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
/// while user stubs stay [`TypeProvenance::StubUser`]
/// ([STUBRES-CUSTOM-TYPESHED]). Generated user stubs carry their stable header
/// marker and remain Tier3 through both disk and unsaved-source paths.
#[must_use]
pub fn extract_stub_exports(
    stub_path: &Path,
    module_name: &str,
    source: StubSource,
) -> Vec<(String, ExternalSymbol)> {
    let Ok(source_text) = std::fs::read_to_string(stub_path) else {
        return Vec::new();
    };
    extract_stub_exports_from_source(&source_text, stub_path, module_name, source, None)
}

/// Extract exports from immutable VFS source using the same declaration model
/// as disk-backed stubs. A concrete target selects guarded declarations; absent
/// evidence keeps only declarations valid across feasible branches.
#[must_use]
pub fn extract_stub_exports_from_source(
    source_text: &str,
    logical_path: &Path,
    module_name: &str,
    source: StubSource,
    target: Option<&StubTarget>,
) -> Vec<(String, ExternalSymbol)> {
    let tier = stub_tier(source_text, source);
    let Some(stub) =
        parse_stub_source(source_text, logical_path, module_name, source, tier, target)
    else {
        return Vec::new();
    };
    stub_module_exports(&stub, logical_path, source, tier)
}

fn stub_tier(source_text: &str, source: StubSource) -> StubTier {
    if source == StubSource::UserStub {
        basilisk_stubs::user_stub_tier(source_text)
    } else {
        StubTier::Tier1
    }
}

fn parse_stub_source(
    source_text: &str,
    logical_path: &Path,
    module_name: &str,
    source: StubSource,
    tier: StubTier,
    target: Option<&StubTarget>,
) -> Option<StubModule> {
    match target {
        Some(target) => basilisk_stubs::pyi_parser::parse_pyi_source_for_target(
            source_text,
            logical_path,
            module_name,
            source,
            tier,
            target,
        ),
        None => {
            basilisk_stubs::parse_pyi_source(source_text, logical_path, module_name, source, tier)
        }
    }
    .ok()
}

fn stub_module_exports(
    stub: &StubModule,
    stub_path: &Path,
    source: StubSource,
    tier: StubTier,
) -> Vec<(String, ExternalSymbol)> {
    let provenance = Some(TypeProvenance::from((&source, &tier)));
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
                signature: Some(basilisk_stubs::render_stub_signature(func)),
                provenance,
                methods: Vec::new(),
                bases: Vec::new(),
                metaclass: None,
                metaclass_calls: Vec::new(),
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
                signature: basilisk_stubs::render_stub_signature(method),
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
                bases: class.bases.clone(),
                metaclass: class.metaclass.clone(),
                metaclass_calls: Vec::new(),
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
                bases: Vec::new(),
                metaclass: None,
                metaclass_calls: Vec::new(),
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

/// Load one **active-snapshot** stub module's exports, following its re-export
/// graph through that same snapshot.
///
/// The single place that knows how to turn `(snapshot, target, stub source
/// text)` into an export set. Shared by the salsa `external_module` query and
/// by the single-file import pipeline the CLI runs, so a step-3 Typeshed
/// module yields the identical member set on both paths — the divergence that
/// silently dropped `imports_module_attribute` from `basilisk check`
/// (GitHub #330).
#[must_use]
pub fn load_snapshot_stub_module(
    logical_path: &Path,
    source_text: &str,
    request: &ExternalModuleRequest,
    snapshot: &basilisk_stubs::typeshed::snapshot::Snapshot,
    target: Option<&StubTarget>,
) -> SharedExternalModule {
    let ExternalModuleRequest::Stub { source, .. } = request else {
        // `py.typed` modules stay filesystem sources at resolution step 5.
        return load_external_module_from_source(logical_path, source_text, request, target);
    };
    let stub_source = *source;
    load_external_module_from_source_with_loader(
        logical_path,
        source_text,
        request,
        target,
        |module_name| {
            let (logical_uri, body) = match target {
                Some(target) => snapshot.read_stub_for_target(module_name, target.python_version),
                None => snapshot.read_stub(module_name),
            }?;
            match target {
                Some(target) => basilisk_stubs::pyi_parser::parse_pyi_source_for_target(
                    body,
                    Path::new(&logical_uri),
                    module_name,
                    stub_source,
                    basilisk_stubs::StubTier::Tier1,
                    target,
                )
                .ok(),
                None => basilisk_stubs::parse_pyi_source(
                    body,
                    Path::new(&logical_uri),
                    module_name,
                    stub_source,
                    basilisk_stubs::StubTier::Tier1,
                )
                .ok(),
            }
        },
    )
}

/// Load a `.pyi` external module from immutable VFS text rather than disk.
/// Non-stub requests are rejected because `py.typed` modules remain filesystem
/// sources at resolution step 5.
#[must_use]
pub fn load_external_module_from_source(
    logical_path: &Path,
    source_text: &str,
    request: &ExternalModuleRequest,
    target: Option<&StubTarget>,
) -> SharedExternalModule {
    load_external_module_from_source_with_loader(logical_path, source_text, request, target, |_| {
        None
    })
}

/// Load one VFS-backed stub and follow its re-export graph through the same
/// active snapshot. Re-exported names that have no local declaration are
/// represented as honest, signature-free symbols: this keeps typing-spec
/// imports resolvable without manufacturing a declaration that the stub did
/// not contain.
#[must_use]
pub fn load_external_module_from_source_with_loader(
    logical_path: &Path,
    source_text: &str,
    request: &ExternalModuleRequest,
    target: Option<&StubTarget>,
    mut loader: impl FnMut(&str) -> Option<StubModule>,
) -> SharedExternalModule {
    let ExternalModuleRequest::Stub {
        module_name,
        source,
    } = request
    else {
        return std::sync::Arc::new(ExternalModule::default());
    };
    let tier = stub_tier(source_text, *source);
    let Some(stub) = parse_stub_source(
        source_text,
        logical_path,
        module_name,
        *source,
        tier,
        target,
    ) else {
        return std::sync::Arc::new(ExternalModule::default());
    };
    let mut exports = stub_module_exports(&stub, logical_path, *source, tier);
    let reexported =
        basilisk_stubs::reexports::reexported_member_names_with_loader(&stub, &mut loader);
    let provenance = Some(TypeProvenance::from((source, &tier)));
    for name in reexported {
        if exports.iter().any(|(existing, _)| existing == &name) {
            continue;
        }
        exports.push((
            name.clone(),
            ExternalSymbol {
                name,
                kind: ExternalSymbolKind::Variable,
                type_annotation: None,
                source_path: logical_path.to_path_buf(),
                source_span: Span::new(0, 0),
                signature: None,
                provenance,
                methods: Vec::new(),
                bases: Vec::new(),
                metaclass: None,
                metaclass_calls: Vec::new(),
            },
        ));
    }
    std::sync::Arc::new(ExternalModule {
        exports,
        reexports: Vec::new(),
    })
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
///
/// Parameter-kind markers are rendered faithfully — positional-only `/`,
/// keyword-only `*`, `*args`, and `**kwargs` — so a `.pyi` signature such as
/// Call-visible signatures of a bound method, one per `@overload` variant,
/// in declaration order ([STUBRES-PYI], GitHub #288).
///
/// Each overload's `self`/`cls` receiver is removed (it lives in
/// [`StubFunction::receiver`], where its annotation — e.g. `LiteralString` for
/// `str.join` — drives specialization at the call site), while positional-only
/// `/` and the exact return type are preserved. This is the real-`.pyi`
/// replacement for the curated builtin-method table: hover and call checking
/// read the same parsed declaration, never a hand-maintained string. An empty
/// result means the class declares no method with that name.
#[must_use]
pub fn bound_method_signatures(class: &StubClass, method: &str) -> Vec<String> {
    class
        .methods
        .iter()
        .filter(|func| func.name == method)
        .map(basilisk_stubs::render_stub_signature)
        .collect()
}

/// Flatten an imported class's inherited methods over its module's C3 MRO
/// ([STUBRES-PYI] #289, GitHub #287).
///
/// A `from … import Class` binding carries only the named class, not its bases
/// (`from unittest.mock import Mock` binds `Mock` but not `NonCallableMock` /
/// `CallableMixin`). Without flattening, hover on `Mock`'s constructor cannot
/// reach the inherited `__new__`/`__init__`. Method names are resolved in MRO
/// order — the first defining class wins, and all its overloads are kept — so a
/// subclass override shadows a base definition exactly as Python does.
///
/// This enriches ONLY the hover-consumed `methods` (no diagnostic rule reads
/// `ExternalSymbol::methods`), and callers apply it solely to the bounded set of
/// named `From` imports, so lazy extraction and the benchmark stay unaffected.
#[must_use]
pub fn flattened_class_methods(
    class_name: &str,
    module_exports: &[(String, ExternalSymbol)],
) -> Vec<ExternalMethod> {
    let by_name: std::collections::HashMap<&str, &ExternalSymbol> = module_exports
        .iter()
        .map(|(name, symbol)| (name.as_str(), symbol))
        .collect();
    let mro = crate::stub_constructor::mro_over(class_name, &|name| {
        by_name.get(name).map_or_else(Vec::new, |symbol| {
            symbol
                .bases
                .iter()
                .map(|base| crate::stub_constructor::base_head(base).to_owned())
                .filter(|head| head != "object" && head != "Any")
                .collect()
        })
    });
    let mut methods = Vec::new();
    let mut claimed: HashSet<String> = HashSet::new();
    for class in &mro {
        let Some(symbol) = by_name.get(class.as_str()) else {
            continue;
        };
        for method in &symbol.methods {
            if !claimed.contains(&method.name) {
                methods.push(method.clone());
            }
        }
        for method in &symbol.methods {
            let _ = claimed.insert(method.name.clone());
        }
    }
    methods
}

/// Resolve the bound `__call__` overloads declared by an external class's
/// metaclass, including an inherited definition over the metaclass C3 MRO.
fn flattened_metaclass_calls(
    symbol: &ExternalSymbol,
    module_exports: &[(String, ExternalSymbol)],
) -> Vec<ExternalMethod> {
    let Some(metaclass) = symbol.metaclass.as_deref() else {
        return Vec::new();
    };
    let head = crate::stub_constructor::base_head(metaclass);
    let simple_name = head.rsplit('.').next().unwrap_or(head);
    flattened_class_methods(simple_name, module_exports)
        .into_iter()
        .filter(|method| method.name == "__call__")
        .collect()
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
/// A logical custom-snapshot URI is [`StubSource::CustomTypeshed`]
/// ([STUBRES-CUSTOM-TYPESHED]); a stub under a configured `stub-paths` root is
/// [`StubSource::UserStub`]; every other stub is [`StubSource::StubPackage`].
/// Provenance flows from here to hover via [`TypeProvenance`].
fn stub_source_for(resolved_path: &Path, stub_paths: &[PathBuf]) -> StubSource {
    let logical = resolved_path.to_string_lossy();
    if logical.starts_with("typeshed:custom-") {
        return StubSource::CustomTypeshed;
    }
    if logical.starts_with("typeshed:") {
        return StubSource::Typeshed;
    }
    if stub_paths
        .iter()
        .any(|root| resolved_path.starts_with(root))
    {
        return StubSource::UserStub;
    }
    StubSource::StubPackage
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
/// `stub_paths` are the configured user-stub roots. Custom Typeshed provenance
/// is carried by the active snapshot's logical URI rather than by re-reading a
/// mutable configuration path.
pub fn populate_imported_symbols<'a, F, E>(
    resolved: &mut basilisk_resolver::ResolvedModule,
    mut workspace_exports: F,
    mut external_module: E,
    stub_paths: &[PathBuf],
) where
    F: FnMut(&Path) -> Option<&'a [(String, ExternalSymbol)]>,
    E: FnMut(&Path, &ExternalModuleRequest) -> SharedExternalModule,
{
    let imports = &resolved.imports;
    let imported_symbols = &mut resolved.imported_symbols;
    let imported_modules = &mut resolved.imported_modules;
    imported_symbols.clear();

    for import in imports {
        let Some(resolved_path) = &import.resolved_path else {
            continue;
        };

        // Prefer workspace exports; otherwise load the external `.pyi` stub
        // or inline-typed `py.typed` package (PEP 561 opt-in only) through the
        // shared lookup.
        let mut py_typed_source = false;
        let mut authoritative_stub = false;
        let external;
        let target_exports: &[(String, ExternalSymbol)] =
            if let Some(exports) = workspace_exports(resolved_path) {
                exports
            } else if resolved_path.extension().is_some_and(|ext| ext == "pyi") {
                let source = stub_source_for(resolved_path, stub_paths);
                authoritative_stub =
                    matches!(source, StubSource::Typeshed | StubSource::CustomTypeshed);
                let request = ExternalModuleRequest::Stub {
                    module_name: import.module.clone(),
                    source,
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

        if import.kind == ImportKind::Plain && authoritative_stub {
            let binding = import.names.first().cloned().unwrap_or_else(|| {
                import
                    .module
                    .split('.')
                    .next()
                    .unwrap_or(&import.module)
                    .to_owned()
            });
            let member_names = target_exports
                .iter()
                .map(|(name, _)| name.clone())
                .collect();
            let _ = imported_modules.insert(
                binding,
                ImportedModuleApi {
                    has_getattr: target_exports.iter().any(|(name, _)| name == "__getattr__"),
                    member_names,
                    stub_path: resolved_path.clone(),
                },
            );
        }

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
                if let Some(mut symbol) = symbol {
                    // [STUBRES-PYI] #289: give a named-imported class its
                    // inherited methods (over the module's C3 MRO) so hover can
                    // reach an inherited constructor. Only when the class is in
                    // this module's own exports, so a re-export keeps its
                    // already-resolved methods.
                    if symbol.kind == ExternalSymbolKind::Class
                        && target_exports.iter().any(|(name, _)| name == &symbol.name)
                    {
                        symbol.metaclass_calls = flattened_metaclass_calls(&symbol, target_exports);
                        symbol.methods = flattened_class_methods(&symbol.name, target_exports);
                    }
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
