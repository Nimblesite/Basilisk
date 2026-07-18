//! Tests [STUBRES-OVERVIEW], [TYPESHEDRT-OVERVIEW], [STUBRES-TYPE-MODEL], and
//! [STUBRES-ENGINE]. See
//! docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-OVERVIEW
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions
)]
//! Integration tests for basilisk-stubs.

fn bundled_builtins_source() -> String {
    basilisk_stubs::typeshed::bundle::bundled_snapshot()
        .expect("bundled snapshot")
        .read_stub("builtins")
        .map(|(_, source)| source.to_owned())
        .expect("bundled builtins.pyi")
}

fn bundled_builtin_exists(name: &str) -> bool {
    let source = bundled_builtins_source();
    source.contains(&format!("class {name}")) || source.contains(&format!("{name}:"))
}

fn bundled_stub_distribution(module: &str) -> Option<String> {
    basilisk_stubs::typeshed::bundle::bundled_snapshot()
        .expect("bundled snapshot")
        .distribution_index
        .distribution(module)
        .map(str::to_owned)
}

// Exercises [STUBRES-TYPESHED] against the exact bundled snapshot body.
#[test]
fn bundled_builtin_str_type() {
    // Phase 5: the stubs library must know about Python built-in types.
    // Currently returns None for all names (placeholder).
    assert!(
        bundled_builtin_exists("str"),
        "str must be a known builtin type — Phase 5 stubs not yet implemented"
    );
}

#[test]
fn bundled_builtin_int_type() {
    // Phase 5: int must be a known built-in type.
    assert!(
        bundled_builtin_exists("int"),
        "int must be a known builtin type — Phase 5 stubs not yet implemented"
    );
}

#[test]
fn bundled_builtin_list_type() {
    // Phase 5: list must be a known built-in type.
    assert!(
        bundled_builtin_exists("list"),
        "list must be a known builtin type — Phase 5 stubs not yet implemented"
    );
}

#[test]
fn bundled_unknown_name_is_absent() {
    // Unknown symbols must always return None.
    assert!(
        !bundled_builtin_exists("definitely_not_a_real_builtin"),
        "unknown names must return None"
    );
}

#[test]
fn bundled_builtin_float_type() {
    assert!(bundled_builtin_exists("float"));
}

#[test]
fn bundled_builtin_bytes_type() {
    assert!(bundled_builtin_exists("bytes"));
}

#[test]
fn bundled_builtin_bool_type() {
    assert!(bundled_builtin_exists("bool"));
}

#[test]
fn bundled_builtin_dict_type() {
    assert!(bundled_builtin_exists("dict"));
}

#[test]
fn bundled_builtin_set_type() {
    assert!(bundled_builtin_exists("set"));
}

#[test]
fn bundled_builtin_tuple_type() {
    assert!(bundled_builtin_exists("tuple"));
}

#[test]
fn bundled_builtin_frozenset_type() {
    assert!(bundled_builtin_exists("frozenset"));
}

#[test]
fn bundled_builtin_type_type() {
    assert!(bundled_builtin_exists("type"));
}

#[test]
fn bundled_builtin_object_type() {
    assert!(bundled_builtin_exists("object"));
}

#[test]
fn bundled_builtin_none_type() {
    assert!(bundled_builtins_source().contains("None"));
}

#[test]
fn bundled_builtin_complex_type() {
    assert!(bundled_builtin_exists("complex"));
}

#[test]
fn bundled_builtin_range_type() {
    assert!(bundled_builtin_exists("range"));
}

#[test]
fn bundled_builtin_bytearray_type() {
    assert!(bundled_builtin_exists("bytearray"));
}

#[test]
fn bundled_builtin_memoryview_type() {
    assert!(bundled_builtin_exists("memoryview"));
}

// Regression for issue #46: BSK-0152's quick fix must offer the *real*
// typeshed distribution name, and nothing when no stub distribution exists.

#[test]
fn stub_distribution_maps_requests_to_types_requests() {
    assert_eq!(
        bundled_stub_distribution("requests").as_deref(),
        Some("types-requests")
    );
}

#[test]
fn stub_distribution_maps_import_root_not_distribution_name() {
    // The import root `yaml` is published as `types-PyYAML`, not `types-yaml`.
    assert_eq!(
        bundled_stub_distribution("yaml").as_deref(),
        Some("types-PyYAML")
    );
}

#[test]
fn stub_distribution_uses_top_level_import_root_for_dotted_modules() {
    assert_eq!(
        bundled_stub_distribution("requests.auth").as_deref(),
        Some("types-requests")
    );
}

#[test]
fn stub_distribution_is_none_for_inline_typed_package() {
    // pydantic-ai ships inline `py.typed`; there is no `types-pydantic_ai`.
    assert_eq!(bundled_stub_distribution("pydantic_ai"), None);
}

// ── Auto-stub generation: mode dispatch + hybrid fallback ───────────────────
// Tests for [STUBRES-AUTOGEN]/[STUBRES-AUTOGEN-MODES]. Exercises the
// `generate_stubs` mode dispatcher and the hybrid runtime→AST fallback without
// requiring a Python interpreter (a bogus `python_path` forces runtime failure).
// See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-AUTOGEN-MODES

#[test]
fn generate_stubs_ast_mode_dispatches_to_ast_backend() {
    use basilisk_stubs::generate::{generate_stubs, StubGenMode};

    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("acme.py");
    std::fs::write(&src, "def greet(name: str) -> str: ...\n").expect("write src");

    // AST mode never spawns a subprocess, so the python path is irrelevant.
    let stub = generate_stubs(
        "acme",
        &src,
        std::path::Path::new("python3"),
        StubGenMode::Ast,
    )
    .expect("AST generation should succeed");
    assert_eq!(stub.mode, StubGenMode::Ast);
    assert!(stub
        .pyi_content
        .contains("def greet(name: str) -> str: ..."));
    // Tier-3 marker so the provenance system reports warnings, not confidence.
    assert!(stub.pyi_content.contains("Tier 3"));
}

#[test]
fn generate_stubs_hybrid_falls_back_to_ast_when_runtime_unavailable() {
    use basilisk_stubs::generate::{generate_stubs, StubGenMode};

    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("acme.py");
    std::fs::write(&src, "def ping() -> bool: ...\n").expect("write src");

    // A python binary that cannot be spawned forces the runtime path to fail,
    // so hybrid mode must fall back to AST inference over the source file.
    let bogus_python = dir.path().join("definitely-not-python");
    let stub = generate_stubs("acme", &src, &bogus_python, StubGenMode::Hybrid)
        .expect("hybrid must succeed via AST fallback when runtime is unavailable");
    assert_eq!(stub.mode, StubGenMode::Hybrid);
    assert!(stub.pyi_content.contains("def ping() -> bool: ..."));
}
