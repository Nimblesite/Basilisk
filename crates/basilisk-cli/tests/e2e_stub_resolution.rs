//! Tests for [CHKARCH-CLI]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CLI
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    unused_results,
    dead_code
)]
//! E2E tests for `.pyi` stub resolution through the resolver and parser.

mod common;

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use basilisk_lsp::import_resolver::{
    has_stub_package, is_inline_typed_package, resolve_module, ActiveTypeshed, ImportSearchPaths,
};
use basilisk_resolver::scope::ImportResolution;
use basilisk_stubs::types::{StubSource, StubTier, TypeProvenance};
use basilisk_stubs::{parse_pyi_file, parse_pyi_source};

static TEST_CTR: AtomicU64 = AtomicU64::new(0);

/// Generate a unique temp dir to avoid races between parallel tests.
fn unique_tmp(prefix: &str) -> std::path::PathBuf {
    let ctr = TEST_CTR.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{prefix}_{ctr}_{}", std::process::id()))
}

fn search_paths(
    roots: Vec<std::path::PathBuf>,
    stub_paths: Vec<std::path::PathBuf>,
    site_packages: Option<std::path::PathBuf>,
) -> ImportSearchPaths {
    ImportSearchPaths {
        roots,
        extra_paths: vec![],
        stub_paths,
        workspace_members: vec![],
        site_packages,
        registry: None,
        typeshed_snapshot: None,
    }
}

#[test]
fn resolver_prefers_pyi_stub_over_py_source() {
    let dir = unique_tmp("e2e_stub_pyi_pref");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("mymod.py"), "x = 1\n").unwrap();
    fs::write(dir.join("mymod.pyi"), "x: int\n").unwrap();

    let paths = search_paths(vec![dir.clone()], vec![], None);
    let result = resolve_module("mymod", &paths).expect("should resolve mymod");
    assert_eq!(result.resolution, ImportResolution::StubPyi);
    assert!(
        result.path.ends_with("mymod.pyi"),
        "should prefer .pyi, got: {:?}",
        result.path
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn user_stub_paths_take_priority_over_source() {
    let root = unique_tmp("e2e_stub_user_root");
    let stubs = unique_tmp("e2e_stub_user_stubs");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&stubs).unwrap();
    fs::write(root.join("mymod.py"), "x = 1\n").unwrap();
    fs::write(stubs.join("mymod.pyi"), "x: int\n").unwrap();

    let paths = search_paths(vec![root.clone()], vec![stubs.clone()], None);
    let result = resolve_module("mymod", &paths).expect("should resolve mymod");
    assert_eq!(result.resolution, ImportResolution::StubPyi);
    assert!(
        result.path.starts_with(&stubs),
        "should come from user stubs dir, got: {:?}",
        result.path
    );

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&stubs);
}

#[test]
fn custom_typeshed_overrides_stdlib_and_parses() {
    let ts = unique_tmp("e2e_typeshed");
    let stdlib = ts.join("stdlib");
    fs::create_dir_all(&stdlib).unwrap();
    // A MicroPython-flavoured `os` whose surface differs from CPython typeshed.
    fs::write(
        stdlib.join("os.pyi"),
        "def uname() -> str: ...\ndef dupterm(stream: object) -> None: ...\n",
    )
    .unwrap();

    let config = basilisk_lsp::config::WorkspaceConfig {
        typeshed_path: Some(ts.clone()),
        typeshed_cache: false,
        ..Default::default()
    };
    let request = basilisk_lsp::config::typeshed_request(&config).unwrap();
    let snapshot = basilisk_stubs::typeshed::runtime::production_manager(request, None)
        .unwrap()
        .snapshot()
        .unwrap();
    assert_eq!(
        snapshot.status.active_source,
        basilisk_stubs::typeshed::source::SourceKind::Custom
    );

    let paths = ImportSearchPaths {
        roots: vec![],
        extra_paths: vec![],
        stub_paths: vec![],
        workspace_members: vec![],
        site_packages: None,
        registry: None,
        typeshed_snapshot: Some(ActiveTypeshed::new(Arc::clone(&snapshot), None)),
    };
    assert!(
        stdlib.join("os.pyi").is_file(),
        "precondition: custom typeshed supplies os.pyi"
    );
    assert!(
        !stdlib.join("fractions.pyi").exists(),
        "precondition: custom typeshed deliberately omits fractions.pyi"
    );
    let result = resolve_module("os", &paths).expect("custom typeshed resolves `os`");
    assert_eq!(result.resolution, ImportResolution::StubPyi);
    let logical_uri = result.path.to_string_lossy();
    assert!(logical_uri.starts_with("typeshed:custom-"));
    let source = snapshot
        .vfs
        .read_uri(&logical_uri)
        .expect("resolved URI belongs to active snapshot");

    let module = parse_pyi_source(
        source,
        &result.path,
        "os",
        StubSource::CustomTypeshed,
        StubTier::Tier1,
    )
    .expect("parse custom os.pyi");
    assert_eq!(module.source, StubSource::CustomTypeshed);
    assert_eq!(module.tier, StubTier::Tier1);
    assert_eq!(
        TypeProvenance::from((&module.source, &module.tier)),
        TypeProvenance::StubCustomTypeshed
    );
    assert!(module.functions.contains_key("uname"));
    assert!(
        module.functions.contains_key("dupterm"),
        "custom MicroPython-only symbol must be visible after override"
    );

    assert!(
        resolve_module("fractions", &paths).is_none(),
        "stdlib modules absent from a custom typeshed must fall through unresolved"
    );

    fs::write(
        stdlib.join("requests.pyi"),
        "def get(url: str) -> bytes: ...\n",
    )
    .unwrap();
    assert!(
        resolve_module("requests", &paths).is_none(),
        "the immutable snapshot must not observe later filesystem mutation"
    );

    let shadow = unique_tmp("e2e_typeshed_shadow_stubs");
    fs::create_dir_all(&shadow).unwrap();
    fs::write(shadow.join("os.pyi"), "def getcwd() -> str: ...\n").unwrap();
    let shadow_paths = ImportSearchPaths {
        stub_paths: vec![shadow.clone()],
        ..paths.clone()
    };
    let shadowed = resolve_module("os", &shadow_paths).expect("stub-path os resolves");
    assert!(
        shadowed.path.starts_with(&shadow),
        "stub-paths must shadow custom typeshed, got: {:?}",
        shadowed.path
    );

    let _ = fs::remove_dir_all(&ts);
    let _ = fs::remove_dir_all(&shadow);
}

#[test]
fn stub_package_resolved_before_inline_typed() {
    let root = unique_tmp("e2e_stub_pep561_root");
    let sp = unique_tmp("e2e_stub_pep561_sp");
    fs::create_dir_all(&root).unwrap();

    let stubs_dir = sp.join("requests-stubs");
    fs::create_dir_all(&stubs_dir).unwrap();
    fs::write(
        stubs_dir.join("__init__.pyi"),
        "def get(url: str) -> bytes: ...\n",
    )
    .unwrap();

    let inline_dir = sp.join("requests");
    fs::create_dir_all(&inline_dir).unwrap();
    fs::write(inline_dir.join("py.typed"), "").unwrap();
    fs::write(inline_dir.join("__init__.py"), "def get(url): pass\n").unwrap();

    let paths = search_paths(vec![root.clone()], vec![], Some(sp.clone()));
    let result = resolve_module("requests", &paths).expect("should resolve requests");
    assert_eq!(result.resolution, ImportResolution::StubPyi);
    assert!(
        result.path.to_string_lossy().contains("requests-stubs"),
        "should come from stubs package, got: {:?}",
        result.path
    );

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&sp);
}

#[test]
fn autofix_stub_install_flips_source_resolution_to_stub() {
    let sp = unique_tmp("e2e_stub_autofix_flip");

    let pkg = sp.join("requests");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join("__init__.py"), "def get(url): pass\n").unwrap();

    let paths = search_paths(vec![], vec![], Some(sp.clone()));
    let before = resolve_module("requests", &paths).expect("should resolve requests");
    assert_eq!(
        before.resolution,
        ImportResolution::SourcePy,
        "precondition: plain site-packages package resolves to SourcePy (E0152 fires)"
    );

    let stubs_dir = sp.join("requests-stubs");
    fs::create_dir_all(&stubs_dir).unwrap();
    fs::write(
        stubs_dir.join("__init__.pyi"),
        "def get(url: str) -> bytes: ...\n",
    )
    .unwrap();

    let after = resolve_module("requests", &paths).expect("should resolve requests");
    assert_eq!(
        after.resolution,
        ImportResolution::StubPyi,
        "after `uv add --dev types-requests` the import must resolve to the stub \
         package so BSK-0152 clears, got: {:?} at {:?}",
        after.resolution,
        after.path
    );

    let _ = fs::remove_dir_all(&sp);
}

#[test]
fn stub_package_submodule_resolution() {
    let sp = unique_tmp("e2e_stub_pep561_sub");
    let stubs_dir = sp.join("requests-stubs");
    fs::create_dir_all(&stubs_dir).unwrap();
    fs::write(stubs_dir.join("__init__.pyi"), "").unwrap();
    fs::write(
        stubs_dir.join("api.pyi"),
        "def get(url: str) -> bytes: ...\n",
    )
    .unwrap();

    let paths = search_paths(vec![], vec![], Some(sp.clone()));
    let result = resolve_module("requests.api", &paths).expect("should resolve requests.api");
    assert_eq!(result.resolution, ImportResolution::StubPyi);
    assert!(
        result.path.ends_with("api.pyi"),
        "should resolve to api.pyi, got: {:?}",
        result.path
    );

    let _ = fs::remove_dir_all(&sp);
}

#[test]
fn py_typed_marker_detected() {
    let sp = unique_tmp("e2e_stub_pytyped");
    let pkg = sp.join("rich");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join("py.typed"), "").unwrap();
    fs::write(pkg.join("__init__.py"), "").unwrap();

    assert!(is_inline_typed_package("rich", &sp));
    assert!(is_inline_typed_package("rich.console", &sp));
    assert!(!is_inline_typed_package("flask", &sp));

    let _ = fs::remove_dir_all(&sp);
}

#[test]
fn has_stub_package_detected() {
    let sp = unique_tmp("e2e_stub_has_stubs");
    let stubs = sp.join("requests-stubs");
    fs::create_dir_all(&stubs).unwrap();

    assert!(has_stub_package("requests", &sp));
    assert!(has_stub_package("requests.api", &sp));
    assert!(!has_stub_package("flask", &sp));

    let _ = fs::remove_dir_all(&sp);
}

#[test]
fn parse_pyi_file_from_disk() {
    let dir = unique_tmp("e2e_stub_parse_disk");
    fs::create_dir_all(&dir).unwrap();
    let pyi_path = dir.join("mymod.pyi");
    fs::write(
        &pyi_path,
        "def greet(name: str) -> str: ...\nVERSION: str\n",
    )
    .unwrap();

    let module = parse_pyi_file(&pyi_path, "mymod", StubSource::UserStub, StubTier::Tier1)
        .expect("should parse .pyi from disk");
    assert!(module.functions.contains_key("greet"));
    let greet = module.functions.get("greet").expect("greet should exist");
    assert_eq!(greet.return_type.as_deref(), Some("str"));
    assert_eq!(greet.params.len(), 1);

    assert!(module.variables.contains_key("VERSION"));
    let version = module
        .variables
        .get("VERSION")
        .expect("VERSION should exist");
    assert_eq!(version.annotation.as_deref(), Some("str"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn parse_pyi_source_with_overloads() {
    let source = "\
from typing import overload

@overload
def process(x: int) -> int: ...
@overload
def process(x: str) -> str: ...
def process(x: int | str) -> int | str: ...

class Parser:
    @overload
    def parse(self, data: str) -> str: ...
    @overload
    def parse(self, data: bytes) -> bytes: ...
    def parse(self, data: str | bytes) -> str | bytes: ...
";
    let module = parse_pyi_source(
        source,
        Path::new("test.pyi"),
        "test",
        StubSource::UserStub,
        StubTier::Tier1,
    )
    .expect("should parse");

    // Top-level overloads
    assert!(module.overloads.contains_key("process"));
    let overloads = module.overloads.get("process").expect("process overloads");
    assert_eq!(overloads.len(), 2);
    assert!(module.functions.contains_key("process"));
    let impl_fn = module.functions.get("process").expect("process impl");
    assert!(!impl_fn.is_overload);

    // Class method overloads
    assert!(module.classes.contains_key("Parser"));
    let parser = module.classes.get("Parser").expect("Parser class");
    assert_eq!(parser.methods.len(), 3);
    assert!(module.overloads.contains_key("Parser.parse"));
    let class_overloads = module
        .overloads
        .get("Parser.parse")
        .expect("Parser.parse overloads");
    assert_eq!(class_overloads.len(), 2);
}

#[test]
fn parse_pyi_class_with_bases_and_attributes() {
    let source = "\
class Animal:
    name: str
    age: int
    def speak(self) -> str: ...

class Dog(Animal):
    breed: str
    def fetch(self, item: str) -> bool: ...
";
    let module = parse_pyi_source(
        source,
        Path::new("animals.pyi"),
        "animals",
        StubSource::UserStub,
        StubTier::Tier1,
    )
    .expect("should parse");

    let animal = module.classes.get("Animal").expect("Animal class");
    assert_eq!(animal.attributes.len(), 2);
    assert_eq!(animal.methods.len(), 1);
    let speak = animal.methods.first().expect("speak method");
    assert_eq!(speak.name, "speak");
    assert!(speak.params.is_empty(), "self should be stripped");

    let dog = module.classes.get("Dog").expect("Dog class");
    assert_eq!(dog.bases, vec!["Animal"]);
    assert_eq!(dog.attributes.len(), 1);
    assert_eq!(dog.methods.len(), 1);
}

#[test]
fn full_roundtrip_resolve_then_parse_stub() {
    let root = unique_tmp("e2e_stub_roundtrip");
    let stubs = unique_tmp("e2e_stub_roundtrip_stubs");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&stubs).unwrap();

    // Source file (no type annotations)
    fs::write(root.join("mylib.py"), "def compute(x): return x + 1\n").unwrap();
    // Stub file (with full annotations)
    fs::write(stubs.join("mylib.pyi"), "def compute(x: int) -> int: ...\n").unwrap();

    // Step 1: Resolve — stub should win
    let paths = search_paths(vec![root.clone()], vec![stubs.clone()], None);
    let resolved = resolve_module("mylib", &paths).expect("should resolve mylib");
    assert_eq!(resolved.resolution, ImportResolution::StubPyi);
    assert!(resolved.path.ends_with("mylib.pyi"));

    // Step 2: Parse the resolved stub
    let module = parse_pyi_file(
        &resolved.path,
        "mylib",
        StubSource::UserStub,
        StubTier::Tier1,
    )
    .expect("should parse resolved stub");

    let compute = module.functions.get("compute").expect("compute function");
    assert_eq!(compute.return_type.as_deref(), Some("int"));
    assert_eq!(compute.params.len(), 1);
    let param = compute.params.first().expect("should have param");
    assert_eq!(param.name, "x");
    assert_eq!(param.annotation.as_deref(), Some("int"));

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&stubs);
}

#[test]
fn full_roundtrip_stub_package_resolve_then_parse() {
    let sp = unique_tmp("e2e_stub_roundtrip_sp");
    let stubs_dir = sp.join("mylib-stubs");
    fs::create_dir_all(&stubs_dir).unwrap();
    fs::write(
        stubs_dir.join("__init__.pyi"),
        "VERSION: str\ndef init(config: dict[str, str]) -> None: ...\n",
    )
    .unwrap();

    // Step 1: Resolve via stub package
    let paths = search_paths(vec![], vec![], Some(sp.clone()));
    let resolved = resolve_module("mylib", &paths).expect("should resolve mylib");
    assert_eq!(resolved.resolution, ImportResolution::StubPyi);

    // Step 2: Parse
    let module = parse_pyi_file(
        &resolved.path,
        "mylib",
        StubSource::StubPackage,
        StubTier::Tier1,
    )
    .expect("should parse stub package");

    assert!(module.variables.contains_key("VERSION"));
    assert!(module.functions.contains_key("init"));
    let init = module.functions.get("init").expect("init function");
    assert_eq!(init.return_type.as_deref(), Some("None"));

    let _ = fs::remove_dir_all(&sp);
}
