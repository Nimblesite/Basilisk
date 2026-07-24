//! Tests for [STUBRES-PYI] bound-method signature synthesis (GitHub #288).
//! Covers real `.pyi` constructor/signature behavior for [TYPESHEDRT-ACCEPTANCE-HOVER].
//! See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PYI
#![allow(clippy::allow_attributes, clippy::unwrap_used, clippy::expect_used)]
//! `basilisk_checker::exports::bound_method_signatures` renders a bound
//! method's call-visible overloads from a parsed `.pyi` — the real-`.pyi`
//! replacement for the curated builtin-method table. These tests parse real
//! typeshed source through the production `.pyi` parser and assert the rendered
//! signatures, so a `.pyi` change (not an edit to a hand table) is what would
//! move them.

use std::path::Path;
use std::sync::Arc;

use basilisk_checker::exports::bound_method_signatures;
use basilisk_stubs::parse_pyi_source;
use basilisk_stubs::types::{StubSource, StubTier};

fn resolve_with_snapshot(
    source: &str,
    snapshot: Arc<basilisk_stubs::typeshed::snapshot::Snapshot>,
) -> basilisk_resolver::ResolvedModule {
    let parsed = basilisk_parser::parse_source(source.to_owned(), "/workspace/main.py".to_owned())
        .expect("Python fixture parses");
    let mut resolved = basilisk_resolver::resolve(&parsed).expect("fixture resolves");
    let paths = basilisk_checker::imports::ImportSearchPaths {
        roots: vec![std::path::PathBuf::from("/workspace")],
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: None,
        registry: None,
        typeshed_snapshot: Some(basilisk_checker::imports::ActiveTypeshed::new(
            snapshot, None,
        )),
    };
    basilisk_checker::imports::resolve_module_imports(&mut resolved, &paths);
    resolved
}

fn parse(src: &str) -> basilisk_stubs::types::StubModule {
    parse_pyi_source(
        src,
        Path::new("builtins.pyi"),
        "builtins",
        StubSource::Typeshed,
        StubTier::Tier1,
    )
    .expect("fixture parses")
}

/// [STUBRES-PYI] #288: the real typeshed `str.join` overload pair renders with
/// the `self` receiver removed, the positional-only `/` preserved, and
/// `LiteralString` intact — proving the signatures come from the parsed `.pyi`,
/// never a hand-maintained table.
#[test]
fn str_join_overloads_render_from_real_pyi() {
    let snapshot =
        basilisk_stubs::typeshed::bundle::bundled_snapshot().expect("release bundle must activate");
    let (uri, src) = snapshot
        .read_stub("builtins")
        .expect("release bundle contains builtins.pyi");
    let module = parse_pyi_source(
        src,
        Path::new(&uri),
        "builtins",
        StubSource::Typeshed,
        StubTier::Tier1,
    )
    .expect("real bundled builtins.pyi parses");
    let str_class = module.classes.get("str").expect("str class parsed");
    let sigs = bound_method_signatures(str_class, "join");
    assert_eq!(
        sigs,
        vec![
            "def join(iterable: Iterable[LiteralString], /) -> LiteralString".to_owned(),
            "def join(iterable: Iterable[str], /) -> str".to_owned(),
        ],
        "str.join must render both overloads: receiver removed, / preserved, LiteralString intact"
    );
}

/// Mutating the `.pyi` return type changes the rendered signature — there is no
/// hand table shadowing the parsed declaration ([STUBRES-PYI] #288).
#[test]
fn pyi_mutation_flows_through_no_hand_table() {
    let root = tempfile::tempdir().expect("temporary custom typeshed");
    std::fs::create_dir(root.path().join("stdlib")).expect("stdlib directory");
    std::fs::write(
        root.path().join("stdlib/builtins.pyi"),
        "class str:\n    def join(self, iterable: int, /) -> bytes: ...\n",
    )
    .expect("custom builtins.pyi");
    let request = basilisk_stubs::typeshed::source::TypeshedRequest {
        selection: basilisk_stubs::typeshed::source::SourceSelection::Custom {
            path: root.path().to_string_lossy().into_owned(),
        },
        store_path: None,
    };
    let snapshot = basilisk_stubs::typeshed::runtime::production_manager(request)
        .snapshot()
        .expect("custom generation activates");
    let resolved = resolve_with_snapshot("value = ''.join('wrong')\n", snapshot);
    let str_class = &resolved
        .builtin_classes
        .get("str")
        .expect("custom str indexed")
        .declaration;
    let sigs = bound_method_signatures(str_class, "join");
    assert_eq!(sigs, vec!["def join(iterable: int, /) -> bytes".to_owned()]);
    assert!(
        basilisk_checker::check(&resolved)
            .iter()
            .any(|diagnostic| diagnostic.code.code == "calls_argument_type"),
        "call checking must consume the mutated custom declaration"
    );
}

/// The real bundled declaration drives overload-aware call checking as well as
/// rendering: valid string iterables pass, missing arguments and an integer
/// iterable are rejected from the same indexed `StubFunction` values.
#[test]
fn real_bundled_str_join_drives_call_checking() {
    let snapshot =
        basilisk_stubs::typeshed::bundle::bundled_snapshot().expect("release bundle must activate");
    let identity = snapshot.identity.uri_component();
    let resolved = resolve_with_snapshot(
        "ok = ''.join(['a', 'b'])\nmissing = ''.join()\nbad = ''.join([1])\n",
        Arc::new(snapshot),
    );
    let indexed = resolved
        .builtin_classes
        .get("str")
        .expect("active builtins.pyi str class indexed");
    assert_eq!(indexed.source_identity, identity);
    assert!(indexed
        .source_path
        .to_string_lossy()
        .ends_with("stdlib/builtins.pyi"));
    let diagnostics = basilisk_checker::check(&resolved);
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.code == "calls_argument_count")
            .count(),
        1,
        "only the zero-argument call violates the real join overload arity: {diagnostics:?}"
    );
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.code == "calls_argument_type")
            .count(),
        1,
        "only the integer iterable violates both real join overloads: {diagnostics:?}"
    );
}

/// A method the class does not declare yields no signatures.
#[test]
fn unknown_method_yields_no_signatures() {
    let module = parse("class str:\n    def upper(self) -> str: ...\n");
    let str_class = module.classes.get("str").expect("str class parsed");
    assert!(bound_method_signatures(str_class, "join").is_empty());
}

/// `*args`/`**kwargs`, positional-only `/`, and keyword-only markers all render
/// faithfully, and a `*args` suppresses a redundant bare `*` before the
/// keyword-only parameters ([STUBRES-PYI]).
#[test]
fn vararg_kwarg_and_keyword_only_markers() {
    let src =
        "class C:\n    def f(self, a: int, /, b: str, *args: int, key: bool, **kw: str) -> None: ...\n";
    let module = parse(src);
    let c = module.classes.get("C").expect("C parsed");
    let sigs = bound_method_signatures(c, "f");
    assert_eq!(
        sigs,
        vec!["def f(a: int, /, b: str, *args: int, key: bool, **kw: str) -> None".to_owned()]
    );
}

/// A keyword-only parameter with no preceding `*args` gets a bare `*` separator.
#[test]
fn bare_star_before_keyword_only_without_vararg() {
    let src = "class C:\n    def g(self, a: int, *, b: str) -> None: ...\n";
    let module = parse(src);
    let c = module.classes.get("C").expect("C parsed");
    assert_eq!(
        bound_method_signatures(c, "g"),
        vec!["def g(a: int, *, b: str) -> None".to_owned()]
    );
}

/// [STUBRES-PYI] #289 prerequisite: a stub class's declared bases flow into its
/// [`ExternalSymbol`] so the constructor/MRO machinery can walk an external
/// class's hierarchy (previously `ExternalSymbol` dropped bases, making
/// `unittest.mock.Mock`'s cross-class `__new__`/`__init__` chain impossible).
#[test]
fn stub_class_bases_flow_into_external_symbol() {
    use basilisk_checker::exports::extract_stub_exports;

    let dir = std::env::temp_dir().join("bsk_ext_bases_xm");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let path = dir.join("mock.pyi");
    std::fs::write(
        &path,
        "class Base: ...\nclass Mock(Base, CallableMixin): ...\n",
    )
    .expect("write .pyi");

    let exports = extract_stub_exports(&path, "unittest.mock", StubSource::Typeshed);
    let mock = exports
        .iter()
        .find(|(name, _)| name == "Mock")
        .map(|(_, symbol)| symbol)
        .expect("Mock class exported");
    assert_eq!(
        mock.bases,
        vec!["Base".to_owned(), "CallableMixin".to_owned()],
        "external Mock must carry its declared bases for MRO resolution"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// [STUBRES-PYI] #289: a `from unittest.mock import Mock`-style binding must
/// surface `Mock`'s INHERITED constructor — `__new__` from `NonCallableMock`
/// and `__init__` from `CallableMixin` (not `Base`) — flattened over the real
/// C3 MRO, so hover can render the constructor. Proven against the real class
/// shape via the production parser/extractor, never a hand table.
#[test]
fn mock_flattens_inherited_constructor_from_mro() {
    use basilisk_checker::exports::{extract_stub_exports, flattened_class_methods};

    let dir = std::env::temp_dir().join("bsk_flatten_mock_xm");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let path = dir.join("mock.pyi");
    std::fs::write(
        &path,
        "from typing import Any, Self\n\
class Base:\n    def __init__(self, *args: Any, **kwargs: Any) -> None: ...\n\
class NonCallableMock(Base, Any):\n    def __new__(cls, *args: Any, **kwargs: Any) -> Self: ...\n    def __init__(self, spec: Any = None) -> None: ...\n\
class CallableMixin(Base):\n    def __init__(self, spec: Any = None, side_effect: Any = None) -> None: ...\n    def __call__(self, *args: Any, **kwargs: Any) -> Any: ...\n\
class Mock(CallableMixin, NonCallableMock): ...\n",
    )
    .expect("write .pyi");

    let exports = extract_stub_exports(&path, "unittest.mock", StubSource::Typeshed);
    let flattened = flattened_class_methods("Mock", &exports);
    let sig = |name: &str| {
        flattened
            .iter()
            .find(|m| m.name == name)
            .map(|m| m.signature.clone())
    };
    // __init__ resolves to CallableMixin's (has `side_effect`), NOT Base's or
    // NonCallableMock's — MRO override semantics. Defaults remain visible in
    // the shared structured declaration renderer.
    assert_eq!(
        sig("__init__").as_deref(),
        Some("def __init__(spec: Any = ..., side_effect: Any = ...) -> None"),
        "__init__ must come from CallableMixin per the C3 MRO"
    );
    // __new__ resolves to NonCallableMock's (the first inherited non-object one).
    assert!(
        sig("__new__").is_some(),
        "__new__ must be inherited from NonCallableMock"
    );
    // __call__ is inherited from CallableMixin (makes the mock callable).
    assert!(
        sig("__call__").is_some(),
        "__call__ inherited from CallableMixin"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
