//! Implements [CHKARCH-CONFORMANCE-MODE]. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-CONFORMANCE-MODE
//!
//! Pins the 2026-08-09 review finding §2, "the builtin-scope loader reverses
//! source and URI, then fails open".
//!
//! `Snapshot::read_stub` returns `(logical_uri, source_text)`. The loader bound
//! that pair the other way round, so it handed the URI to the parser as Python
//! and the entire `builtins.pyi` body to the filesystem as a path. The parse
//! failed on every run, `unwrap_or_default()` turned the failure into an EMPTY
//! set, and the empty set was cached as though it were the answer.
//!
//! Downstream, `names_undefined::is_builtin_name` reads an empty builtin scope
//! as "unknown — suppress", which is the right call for a genuinely unknown
//! scope and catastrophic for a scope that is merely broken: every
//! undefined-name diagnostic in every module disappears, silently, with the
//! checker reporting success.
//!
//! The two obligations here are therefore separate and both required: the
//! builtin scope must actually LOAD, and "did not load" must be a state the
//! type can express rather than a value indistinguishable from a real answer.
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

use std::sync::Arc;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Resolve a module through the real import pipeline, with the bundled
/// typeshed generation active — exactly what the CLI does.
fn resolve_with_typeshed(
    source: &str,
) -> Result<basilisk_resolver::ResolvedModule, Box<dyn std::error::Error>> {
    let parsed = basilisk_parser::parse_source(source.to_owned(), "test.py".to_owned())?;
    let mut resolved = basilisk_resolver::resolve(&parsed)?;
    let snapshot = basilisk_stubs::typeshed::bundle::bundled_snapshot()?;
    let paths = basilisk_checker::imports::ImportSearchPaths {
        roots: Vec::new(),
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: None,
        registry: None,
        typeshed_snapshot: Some(basilisk_checker::imports::ActiveTypeshed::new(
            Arc::new(snapshot),
            None,
        )),
    };
    basilisk_checker::imports::resolve_module_imports(&mut resolved, &paths);
    Ok(resolved)
}

/// The builtin scope must be POPULATED when a typeshed generation is active.
///
/// This is the direct pin on the reversed tuple: with the arguments swapped the
/// parse cannot succeed, so the scope is empty for every module regardless of
/// what `builtins.pyi` contains.
#[test]
fn an_active_typeshed_generation_yields_a_populated_builtin_scope() -> TestResult {
    let resolved = resolve_with_typeshed("pass\n")?;
    let scope = resolved
        .builtin_names
        .as_ref()
        .ok_or("the builtin scope is UNAVAILABLE with a live typeshed generation active")?;

    assert!(
        !scope.is_empty(),
        "`builtins.pyi` binds hundreds of names; an empty scope means the \
         loader never parsed it"
    );
    for name in ["len", "print", "int", "str", "isinstance", "object"] {
        assert!(
            scope.contains(name),
            "`{name}` is bound by `builtins.pyi` and must appear in the \
             builtin scope"
        );
    }
    Ok(())
}

/// A missing typeshed generation is UNAVAILABLE, not an empty namespace.
///
/// Without the state distinction, "no builtins at all" and "we could not find
/// out" are the same value, and the rule reading it has to guess which one it
/// got. The rule's obligation is to suppress on unknown and to report on a
/// name a KNOWN scope does not contain — it can only honour that if the two
/// are different values.
#[test]
fn no_typeshed_generation_leaves_the_builtin_scope_unavailable() -> TestResult {
    let parsed = basilisk_parser::parse_source("pass\n".to_owned(), "test.py".to_owned())?;
    let mut resolved = basilisk_resolver::resolve(&parsed)?;
    let paths = basilisk_checker::imports::ImportSearchPaths {
        roots: Vec::new(),
        extra_paths: Vec::new(),
        stub_paths: Vec::new(),
        workspace_members: Vec::new(),
        site_packages: None,
        registry: None,
        typeshed_snapshot: None,
    };
    basilisk_checker::imports::resolve_module_imports(&mut resolved, &paths);

    assert!(
        resolved.builtin_names.is_none(),
        "with no generation active the builtin scope is unknown, and saying so \
         is the only honest answer"
    );
    Ok(())
}

/// The user-visible consequence: a name that is NOT a builtin and NOT defined
/// is reported. A silently empty scope suppresses this, and the suppression is
/// invisible — the run reports success.
#[test]
fn an_undefined_name_is_still_reported_with_the_builtin_scope_loaded() -> TestResult {
    let resolved = resolve_with_typeshed(
        "\
def survey() -> None:
    print(cairn_count)
",
    )?;
    let diagnostics = basilisk_checker::check(&resolved);
    let codes: Vec<String> = diagnostics
        .iter()
        .map(|diag| diag.code.code.to_string())
        .collect();

    assert!(
        codes.iter().any(|code| code.contains("undefined")),
        "`cairn_count` is neither defined here nor bound by `builtins.pyi`; \
         got {codes:?}"
    );
    Ok(())
}
