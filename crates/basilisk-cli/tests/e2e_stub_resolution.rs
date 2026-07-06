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
//! E2E tests for `.pyi` stub file resolution through the full pipeline.
//!
//! These tests verify that stub files are correctly discovered and parsed
//! when the import resolver encounters them. They exercise:
//!
//! - PEP 561 stub resolution order (user stubs → source → stub packages)
//! - `.pyi` preference over `.py`
//! - Stub package discovery (`foopkg-stubs/`)
//! - `py.typed` marker detection
//! - `.pyi` file parsing via `basilisk_stubs::parse_pyi_source`

mod common;

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use basilisk_lsp::import_resolver::{
    has_stub_package, is_inline_typed_package, resolve_module, ImportSearchPaths,
};
use basilisk_resolver::scope::ImportResolution;
use basilisk_stubs::types::{StubSource, StubTier};
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
        typeshed_path: None,
    }
}

// ---------------------------------------------------------------------------
// .pyi preference over .py through import resolver
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// User stub-paths take highest priority
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Custom typeshed override (typing-spec import-resolution step 3)
// [STUBRES-CUSTOM-TYPESHED]
// ---------------------------------------------------------------------------

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

    let paths = ImportSearchPaths {
        roots: vec![],
        extra_paths: vec![],
        stub_paths: vec![],
        workspace_members: vec![],
        site_packages: None,
        registry: None,
        typeshed_path: Some(ts.clone()),
    };

    let result = resolve_module("os", &paths).expect("custom typeshed resolves `os`");
    assert_eq!(result.resolution, ImportResolution::StubPyi);
    assert!(result.path.starts_with(&stdlib));

    // Parse the resolved stub end-to-end and confirm the custom signatures win.
    let module = parse_pyi_file(&result.path, "os", StubSource::Typeshed, StubTier::Tier1)
        .expect("parse custom os.pyi");
    assert!(module.functions.contains_key("uname"));
    assert!(
        module.functions.contains_key("dupterm"),
        "custom MicroPython-only symbol must be visible after override"
    );

    let _ = fs::remove_dir_all(&ts);
}

// ---------------------------------------------------------------------------
// PEP 561 stub-only packages (-stubs)
// ---------------------------------------------------------------------------

#[test]
fn stub_package_resolved_before_inline_typed() {
    let root = unique_tmp("e2e_stub_pep561_root");
    let sp = unique_tmp("e2e_stub_pep561_sp");
    fs::create_dir_all(&root).unwrap();

    // Create requests-stubs package
    let stubs_dir = sp.join("requests-stubs");
    fs::create_dir_all(&stubs_dir).unwrap();
    fs::write(
        stubs_dir.join("__init__.pyi"),
        "def get(url: str) -> bytes: ...\n",
    )
    .unwrap();

    // Also create inline-typed requests package
    let inline_dir = sp.join("requests");
    fs::create_dir_all(&inline_dir).unwrap();
    fs::write(inline_dir.join("py.typed"), "").unwrap();
    fs::write(inline_dir.join("__init__.py"), "def get(url): pass\n").unwrap();

    let paths = search_paths(vec![root.clone()], vec![], Some(sp.clone()));
    let result = resolve_module("requests", &paths).expect("should resolve requests");
    // Stub package should win over inline-typed
    assert_eq!(result.resolution, ImportResolution::StubPyi);
    assert!(
        result.path.to_string_lossy().contains("requests-stubs"),
        "should come from stubs package, got: {:?}",
        result.path
    );

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&sp);
}

/// Reproduces issue: "I can't get it to pick up stubs and the auto-fix doesn't
/// fix anything." The BSK-E0152 quick-fix runs `uv add --dev types-<pkg>`, which
/// drops a `<pkg>-stubs/` package into site-packages. E0152 fires only while the
/// import resolves to `SourcePy`; after the stub package is installed the import
/// MUST resolve to `StubPyi` so the warning clears. This exercises that exact
/// before/after transition deterministically (no network / no uv).
#[test]
fn autofix_stub_install_flips_source_resolution_to_stub() {
    let sp = unique_tmp("e2e_stub_autofix_flip");

    // BEFORE the auto-fix: `requests` is installed without inline types and
    // without a stub package → resolves to plain source (E0152 fires here).
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

    // AFTER the auto-fix: `uv add --dev types-requests` installs `requests-stubs/`.
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
         package so BSK-E0152 clears, got: {:?} at {:?}",
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

// ---------------------------------------------------------------------------
// py.typed marker detection
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// .pyi file parsing end-to-end
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Full round-trip: stub file on disk → resolver → parsed stub
// ---------------------------------------------------------------------------

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
