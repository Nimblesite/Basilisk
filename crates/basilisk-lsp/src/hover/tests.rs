use super::*;

/// Parse and resolve `source` as `test.py` for hover tests.
fn parse_and_resolve(source: &str) -> ResolvedModule {
    let parsed = basilisk_parser::parse_source(source.to_owned(), "test.py".to_owned())
        .expect("test source should parse");
    basilisk_resolver::resolve(&parsed).expect("resolution should not fail")
}

/// Parse and resolve source code, then patch the first import's resolution
/// and path fields for testing hover display.
fn resolve_with_patched_import(
    source: &str,
    resolution: ImportResolution,
    resolved_path: Option<std::path::PathBuf>,
) -> ResolvedModule {
    let mut resolved = parse_and_resolve(source);
    if let Some(import) = resolved.imports.first_mut() {
        import.resolution = resolution;
        import.resolved_path = resolved_path;
    }
    resolved
}

#[test]
fn test_hover_on_resolved_import_shows_stub_info() {
    let source = "import os\n";
    let resolved = resolve_with_patched_import(
        source,
        ImportResolution::StubPyi,
        Some(std::path::PathBuf::from("/usr/lib/python3.12/os.pyi")),
    );

    let hover = hover_at(&resolved, source, 7, &[]);
    let hover = hover.expect("hover should be Some for resolved import");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup.value.contains("Type stubs"),
        "should mention type stubs: {}",
        markup.value
    );
    assert!(
        markup.value.contains("os.pyi"),
        "should show resolved path: {}",
        markup.value
    );
}

/// Stage 3 of stub type consumption: hovering a symbol imported from a stub
/// or py.typed package (one with no local definition) must show its
/// signature/type from `imported_symbols`. Before this, hover found no local
/// `hit` and rendered nothing for such symbols.
#[test]
fn test_hover_on_imported_stub_symbol_shows_signature() {
    use basilisk_resolver::scope::{ExternalSymbol, ExternalSymbolKind};
    use basilisk_resolver::Span;

    let source = "from acme import fetch\n\nx = fetch('u')\n";
    let mut resolved = parse_and_resolve(source);

    let _ = resolved.imported_symbols.insert(
        "fetch".to_owned(),
        ExternalSymbol {
            name: "fetch".to_owned(),
            kind: ExternalSymbolKind::Function,
            type_annotation: Some("bytes".to_owned()),
            source_path: std::path::PathBuf::from("/venv/.../acme-stubs/__init__.pyi"),
            source_span: Span::new(0, 0),
            signature: Some("def fetch(url: str) -> bytes".to_owned()),
            provenance: Some(basilisk_stubs::TypeProvenance::StubTier1),
            methods: Vec::new(),
        },
    );

    // Hover on the `fetch` usage in `x = fetch('u')` (not the import line).
    let offset = source.rfind("fetch").expect("usage present") + 1;
    let hover = hover_at(&resolved, source, offset, &[]);
    let hover = hover.expect("hover should be Some for an imported stub symbol");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup.value.contains("def fetch(url: str) -> bytes"),
        "hover should show the stub signature: {}",
        markup.value
    );
}

/// Regression for #287: hovering a method inherited from an *external* base
/// class (e.g. `model_validate` on a pydantic `BaseModel` subclass) must
/// show the method's stub signature. The member data exists in the stub —
/// hover showed nothing because class members were dropped during export
/// extraction and hover had no dot-access member lookup.
#[test]
fn test_hover_on_method_inherited_from_external_stub_base_shows_signature() {
    let source = "from pydantic import BaseModel\n\nclass ComposerSavePayload(BaseModel):\n    name: str\n\np = ComposerSavePayload.model_validate({})\n";
    let mut resolved = parse_and_resolve(source);

    // A real stub on disk, exactly as import resolution would find it.
    let dir = tempfile::tempdir().expect("create temp dir");
    let stub_path = dir.path().join("pydantic.pyi");
    std::fs::write(
        &stub_path,
        "class BaseModel:\n    def model_validate(cls, obj: object) -> BaseModel: ...\n",
    )
    .expect("write stub");
    if let Some(import) = resolved.imports.first_mut() {
        import.resolution = ImportResolution::StubPyi;
        import.resolved_path = Some(stub_path);
    }
    basilisk_checker::exports::populate_imported_symbols(
        &mut resolved,
        |_| None,
        basilisk_checker::exports::load_external_module,
        None,
    );
    assert!(
        resolved.imported_symbols.contains_key("BaseModel"),
        "precondition: the stub base class resolved"
    );

    // Hover on the `model_validate` call site.
    let offset = source.rfind("model_validate").expect("usage present") + 1;
    let hover = hover_at(&resolved, source, offset, &[]);
    let hover = hover.expect("hover should be Some for a method inherited from a stub base");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    // The stub pipeline skips the `self`/`cls` receiver of methods
    // (`extract_params`), matching how local method hovers render.
    assert!(
        markup
            .value
            .contains("BaseModel.model_validate(obj: object) -> BaseModel"),
        "hover should show the inherited method's stub signature: {}",
        markup.value
    );
}

/// Regression for #287, real-package shape: `pydantic/__init__.py` defines no
/// classes — it re-exports everything from `.main` via a **star import**
/// inside an `if TYPE_CHECKING:` block (`from .main import *`; runtime uses a
/// lazy module `__getattr__`). Export extraction must follow that re-export
/// into `main.py`, so hovering an inherited `model_validate` shows its
/// signature. Before this, extraction only kept symbols *defined* in the
/// resolved `__init__.py`, and the hover returned nothing against the real
/// pydantic package.
#[test]
fn test_hover_on_method_reexported_through_py_typed_package_init() {
    let source = "from pydantic import BaseModel\n\nclass ComposerSavePayload(BaseModel):\n    name: str\n\np = ComposerSavePayload.model_validate({})\n";
    let mut resolved = parse_and_resolve(source);

    // A real py.typed package on disk, shaped like pydantic v2.
    let dir = tempfile::tempdir().expect("create temp dir");
    let pkg = dir.path().join("pydantic");
    std::fs::create_dir(&pkg).expect("create package dir");
    std::fs::write(pkg.join("py.typed"), "").expect("write py.typed marker");
    std::fs::write(
        pkg.join("__init__.py"),
        "from typing import TYPE_CHECKING\n\nif TYPE_CHECKING:\n    from .main import *\n\n__all__ = ['BaseModel']\n",
    )
    .expect("write __init__.py");
    std::fs::write(
        pkg.join("main.py"),
        "class BaseModel:\n    @classmethod\n    def model_validate(cls, obj: object) -> 'BaseModel': ...\n",
    )
    .expect("write main.py");
    if let Some(import) = resolved.imports.first_mut() {
        import.resolution = ImportResolution::SourcePy;
        import.resolved_path = Some(pkg.join("__init__.py"));
    }
    basilisk_checker::exports::populate_imported_symbols(
        &mut resolved,
        |_| None,
        basilisk_checker::exports::load_external_module,
        None,
    );
    assert!(
        resolved.imported_symbols.contains_key("BaseModel"),
        "the TYPE_CHECKING re-export in the package __init__ must surface \
         `BaseModel` as an imported symbol"
    );

    // Hover on the `model_validate` call site.
    let offset = source.rfind("model_validate").expect("usage present") + 1;
    let hover = hover_at(&resolved, source, offset, &[]);
    let hover =
        hover.expect("hover should be Some for a method inherited through a re-exported base");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup.value.contains("BaseModel.model_validate"),
        "hover should show the re-exported method's signature: {}",
        markup.value
    );
}

/// Regression for #289: hovering a class name must include the
/// constructor's signature (the `__init__` hint) — `(class) Point` alone
/// doesn't tell the user how to instantiate it.
#[test]
fn test_hover_on_class_shows_init_signature() {
    let source = "class Point:\n    def __init__(self, x: int, y: int) -> None:\n        self.x = x\n        self.y = y\n\np = Point(1, 2)\n";
    let resolved = parse_and_resolve(source);

    // Hover on the `Point` reference in `p = Point(1, 2)`.
    let offset = source.rfind("Point").expect("usage present") + 1;
    let hover = hover_at(&resolved, source, offset, &[]);
    let hover = hover.expect("hover should be Some for a class reference");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup.value.contains("(class) Point"),
        "hover should show the class signature: {}",
        markup.value
    );
    assert!(
        markup
            .value
            .contains("__init__(self, x: int, y: int) -> None"),
        "hover should show the constructor signature: {}",
        markup.value
    );
}

/// Regression for #288: hovering a builtin method on a string-literal
/// receiver (e.g. `" ".join(...)`) must show the method's signature
/// instead of nothing.
#[test]
fn test_hover_on_str_literal_method_shows_signature() {
    let source = "words = [\"a\", \"b\"]\nx = \" \".join(words)\n";
    let resolved = parse_and_resolve(source);

    let offset = source.rfind("join").expect("usage present") + 1;
    let hover = hover_at(&resolved, source, offset, &[]);
    let hover = hover.expect("hover should be Some for a builtin str method");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup.value.contains("str.join"),
        "hover should show the builtin method's signature: {}",
        markup.value
    );
}

/// Regression for #200 (intermittent hover): hovering a *usage* of an
/// imported name must be deterministic. When `imported_symbols` is empty —
/// e.g. cross-file resolution has not run or not completed — hover falls back
/// to the import declaration instead of racing and returning `None`.
#[test]
fn test_hover_on_imported_name_usage_falls_back_to_import_decl() {
    let source = "from nav_helper import helper_fn, HelperClass\n\nhelper_fn()\n";
    let resolved = parse_and_resolve(source);
    // No cross-file resolution in this unit test, so imported_symbols is empty.
    assert!(
        resolved.imported_symbols.is_empty(),
        "precondition: no imported_symbols populated"
    );

    let offset = source.rfind("helper_fn").expect("usage present") + 1;
    let hover = hover_at(&resolved, source, offset, &[]);
    let hover = hover.expect("hover should be Some for an imported-name usage");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup.value.contains("helper_fn"),
        "hover should name the imported symbol: {}",
        markup.value
    );
    assert!(
        markup.value.contains("from nav_helper import"),
        "fallback should show the import declaration: {}",
        markup.value
    );
}

#[test]
fn test_hover_on_unresolved_import_shows_status() {
    let source = "import nonexistent\n";
    let resolved = resolve_with_patched_import(source, ImportResolution::Unresolved, None);

    let hover = hover_at(&resolved, source, 10, &[]);
    let hover = hover.expect("hover should be Some for unresolved import");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup.value.contains("Unresolved"),
        "should show Unresolved status: {}",
        markup.value
    );
}

#[test]
fn test_hover_on_plain_aliased_import_shows_alias_not_from() {
    // Regression (follow-up to #180): a plain `import X as Y` must hover as
    // `import X as Y`, never be mislabeled `from X import Y`. Capturing the
    // alias in `ImportInfo.names` made a names-based signature render a `from`.
    let source = "import datetime as _dt\n";
    let resolved = parse_and_resolve(source);

    // Offset 10 is inside `datetime`, within the import statement's span.
    let hover = hover_at(&resolved, source, 10, &[]);
    let hover = hover.expect("hover should be Some on an aliased import");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup.value.contains("import datetime as _dt"),
        "aliased plain import must hover as `import X as Y`: {}",
        markup.value
    );
    assert!(
        !markup.value.contains("from datetime import"),
        "aliased plain import must NOT render as a `from`-import: {}",
        markup.value
    );
}

#[test]
fn test_hover_on_source_py_import_shows_no_stubs() {
    let source = "import mymodule\n";
    let resolved = resolve_with_patched_import(
        source,
        ImportResolution::SourcePy,
        Some(std::path::PathBuf::from("/project/mymodule.py")),
    );

    let hover = hover_at(&resolved, source, 10, &[]);
    let hover = hover.expect("hover should be Some for source import");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup.value.contains("no type stubs"),
        "should indicate no type stubs: {}",
        markup.value
    );
    assert!(
        markup.value.contains("mymodule.py"),
        "should show resolved path: {}",
        markup.value
    );
}

#[test]
fn test_hover_outside_import_span_returns_none_for_import() {
    let source = "x: int = 1\nimport os\n";
    let resolved = resolve_with_patched_import(
        source,
        ImportResolution::StubPyi,
        Some(std::path::PathBuf::from("/usr/lib/python3.12/os.pyi")),
    );

    // Offset 5 is on "x: int = 1", not on the import.
    let import_hit = find_import_at_offset(&resolved, 5);
    assert!(
        import_hit.is_none(),
        "should not find import outside its span"
    );
}

#[test]
fn test_hover_shows_package_version_and_name() {
    let source = "import requests\n";
    let mut resolved = parse_and_resolve(source);
    if let Some(import) = resolved.imports.first_mut() {
        import.resolution = ImportResolution::SourcePy;
        import.resolved_path = Some(std::path::PathBuf::from(
            "/venv/lib/python3.12/site-packages/requests/__init__.py",
        ));
        import.package_name = Some("requests".to_owned());
        import.package_version = Some("2.31.0".to_owned());
        import.package_dep_kind = Some(PackageDepKind::Direct);
    }

    let hover = hover_at(&resolved, source, 10, &[]);
    let hover = hover.expect("hover should be Some");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup.value.contains("requests v2.31.0"),
        "should show package name and version: {}",
        markup.value
    );
    assert!(
        markup.value.contains("direct dependency"),
        "should show dependency kind: {}",
        markup.value
    );
}

#[test]
fn test_hover_no_package_info_for_stdlib() {
    let source = "import os\n";
    let resolved = resolve_with_patched_import(
        source,
        ImportResolution::StubPyi,
        Some(std::path::PathBuf::from("/usr/lib/python3.12/os.pyi")),
    );

    let hover = hover_at(&resolved, source, 7, &[]);
    let hover = hover.expect("hover should be Some");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    // stdlib imports should not show package version or dependency kind.
    assert!(
        !markup.value.contains("**Package**"),
        "stdlib should not show package info: {}",
        markup.value
    );
    assert!(
        !markup.value.contains("**Dependency**"),
        "stdlib should not show dependency kind: {}",
        markup.value
    );
}

#[test]
fn test_hover_on_unresolved_import_shows_provenance_annotation() {
    let source = "import nonexistent\n";
    let resolved = resolve_with_patched_import(source, ImportResolution::Unresolved, None);

    let hover = hover_at(&resolved, source, 10, &[]);
    let hover = hover.expect("hover should be Some for unresolved import");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup.value.contains("no type stubs available"),
        "unresolved import should show 'no type stubs available': {}",
        markup.value
    );
}

#[test]
fn test_hover_on_source_py_import_shows_no_stubs_annotation() {
    let source = "import mymodule\n";
    let resolved = resolve_with_patched_import(
        source,
        ImportResolution::SourcePy,
        Some(std::path::PathBuf::from("/project/mymodule.py")),
    );

    let hover = hover_at(&resolved, source, 10, &[]);
    let hover = hover.expect("hover should be Some for source import");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup.value.contains("no type stubs"),
        "source .py import should show 'no type stubs': {}",
        markup.value
    );
}

// Regression for GitHub #290: hovering a variable bound to a dict literal
// showed the bare container name (`dict`) instead of the parameterized
// generic inferred from its elements (`dict[str, str]`).
#[test]
fn test_hover_infers_generic_type_args_for_dict_literal() {
    let source = "language_timezone_mapping = {\"en\": \"UTC\", \"fr\": \"Europe/Paris\"}\n";
    let resolved = parse_and_resolve(source);

    // Cursor on `language_timezone_mapping` at its definition (offset 0).
    let hover = hover_at(&resolved, source, 0, &[]);
    let hover = hover.expect("hover should be Some for a dict-literal variable");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected Markup hover contents");
    };

    assert!(
        markup
            .value
            .contains("language_timezone_mapping: dict[str, str]"),
        "hover should show the parameterized generic dict[str, str]: {}",
        markup.value
    );
}
