//! Tests for [CHKARCH-DIAG-IMPORT-MEMBER] (`imports_missing_name`, GitHub #55).
//! `from M import name` where the resolved workspace module defines no `name`
//! must produce a diagnostic — module-path resolution alone is not enough.
#![allow(
    clippy::allow_attributes,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    missing_docs
)]

use std::fs;
use std::path::Path;

use basilisk_checker::imports::resolve_module_imports;

mod import_support;
use import_support::{make_search_paths, make_tmp_dir};

/// Parse + resolve `source` as `<root>/consumer.py`, resolve its imports
/// against `root`, and run the full default-config rule set over it.
fn check_consumer(root: &Path, source: &str) -> Vec<basilisk_checker::Diagnostic> {
    let consumer = root.join("consumer.py");
    fs::write(&consumer, source).unwrap();
    let parsed =
        basilisk_parser::parse_source(source.to_owned(), consumer.to_string_lossy().into_owned())
            .unwrap();
    let mut resolved = basilisk_resolver::resolve(&parsed).unwrap();
    let paths = make_search_paths(vec![root.to_path_buf()]);
    resolve_module_imports(&mut resolved, &paths);
    basilisk_checker::check_with_config(&resolved, &basilisk_config::BasiliskConfig::default())
}

fn codes(diagnostics: &[basilisk_checker::Diagnostic]) -> Vec<&str> {
    diagnostics.iter().map(|d| d.code.code).collect()
}

/// GitHub #55, the exact filed repro: the target module resolves but is empty,
/// so the imported name cannot exist — a diagnostic is required.
#[test]
fn from_import_of_name_missing_from_resolved_module_is_flagged() {
    let root = make_tmp_dir("missing_name_empty_module");
    fs::create_dir_all(root.join("demo")).unwrap();
    fs::write(root.join("demo/__init__.py"), "").unwrap();
    fs::write(root.join("demo/late_module.py"), "").unwrap();

    let diagnostics = check_consumer(
        &root,
        "from demo.late_module import provide_value\n\n\
         def use() -> int:\n    return provide_value()\n",
    );

    let missing: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "imports_missing_name")
        .collect();
    assert_eq!(
        missing.len(),
        1,
        "importing `provide_value` from an empty module must be flagged \
         (GitHub #55); got diagnostics: {:?}",
        codes(&diagnostics)
    );
    assert!(
        missing[0].message.contains("provide_value")
            && missing[0].message.contains("demo.late_module"),
        "the diagnostic must name both the missing symbol and the module: {}",
        missing[0].message
    );

    fs::remove_dir_all(&root).unwrap();
}

/// The gap is not an empty-file special case: a module that defines OTHER
/// names still draws the diagnostic for the one it doesn't define — and no
/// diagnostic for the one it does.
#[test]
fn from_import_flags_only_the_undefined_name_in_a_nonempty_module() {
    let root = make_tmp_dir("missing_name_nonempty_module");
    fs::create_dir_all(root.join("demo")).unwrap();
    fs::write(root.join("demo/__init__.py"), "").unwrap();
    fs::write(root.join("demo/other.py"), "DEFINED_NAME: int = 1\n").unwrap();

    let diagnostics = check_consumer(&root, "from demo.other import DEFINED_NAME, missing_name\n");

    let missing: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "imports_missing_name")
        .collect();
    assert_eq!(
        missing.len(),
        1,
        "exactly `missing_name` must be flagged, never `DEFINED_NAME`; got: {:?}",
        codes(&diagnostics)
    );
    assert!(missing[0].message.contains("missing_name"));

    fs::remove_dir_all(&root).unwrap();
}

/// `from m import real as alias` checks the IMPORTED name (`real`), not the
/// local alias — aliasing an existing name is fine.
#[test]
fn aliased_from_import_checks_the_imported_name_not_the_alias() {
    let root = make_tmp_dir("missing_name_alias");
    fs::write(root.join("target.py"), "value = 1\n").unwrap();

    let diagnostics = check_consumer(&root, "from target import value as v\n");
    assert!(
        !codes(&diagnostics).contains(&"imports_missing_name"),
        "aliasing an existing name must not be flagged; got: {:?}",
        codes(&diagnostics)
    );

    fs::remove_dir_all(&root).unwrap();
}

/// `from pkg import mod` where `pkg/mod.py` exists is a SUBMODULE import — the
/// package `__init__.py` need not bind the name. Must never be flagged.
#[test]
fn from_package_import_of_existing_submodule_is_not_flagged() {
    let root = make_tmp_dir("missing_name_submodule");
    fs::create_dir_all(root.join("pkg")).unwrap();
    fs::write(root.join("pkg/__init__.py"), "").unwrap();
    fs::write(root.join("pkg/mod.py"), "x = 1\n").unwrap();

    let diagnostics = check_consumer(&root, "from pkg import mod\n");
    assert!(
        !codes(&diagnostics).contains(&"imports_missing_name"),
        "an existing submodule satisfies `from pkg import mod`; got: {:?}",
        codes(&diagnostics)
    );

    fs::remove_dir_all(&root).unwrap();
}

/// A module-level `__getattr__` (PEP 562) makes every attribute importable —
/// the rule must stand down entirely for such modules.
#[test]
fn module_level_getattr_suppresses_the_rule() {
    let root = make_tmp_dir("missing_name_getattr");
    fs::write(
        root.join("dynamic.py"),
        "def __getattr__(name: str) -> object: ...\n",
    )
    .unwrap();

    let diagnostics = check_consumer(&root, "from dynamic import anything_at_all\n");
    assert!(
        !codes(&diagnostics).contains(&"imports_missing_name"),
        "PEP 562 `__getattr__` permits any name; got: {:?}",
        codes(&diagnostics)
    );

    fs::remove_dir_all(&root).unwrap();
}

/// A target module containing `from x import *` has a statically unknowable
/// member set — the rule must stay silent rather than guess.
#[test]
fn target_module_with_star_import_suppresses_the_rule() {
    let root = make_tmp_dir("missing_name_star_target");
    fs::write(root.join("base.py"), "hidden = 1\n").unwrap();
    fs::write(root.join("reexporter.py"), "from base import *\n").unwrap();

    let diagnostics = check_consumer(&root, "from reexporter import hidden\n");
    assert!(
        !codes(&diagnostics).contains(&"imports_missing_name"),
        "a star-importing target has an unknowable member set; got: {:?}",
        codes(&diagnostics)
    );

    fs::remove_dir_all(&root).unwrap();
}

/// Names bound by non-def statements — imports (re-exports), `for` targets,
/// walrus, `with ... as`, try/except fallbacks — are all legitimate exports.
#[test]
fn names_bound_by_any_module_level_statement_count_as_defined() {
    let root = make_tmp_dir("missing_name_binding_forms");
    fs::write(root.join("helper.py"), "thing = 1\n").unwrap();
    fs::write(
        root.join("bindings.py"),
        "from helper import thing\n\
         import helper as helper_alias\n\
         for loop_var in range(3):\n    pass\n\
         if (walrus := 1):\n    pass\n\
         try:\n    fallback = 1\nexcept ValueError:\n    fallback = 2\n",
    )
    .unwrap();

    let diagnostics = check_consumer(
        &root,
        "from bindings import thing, helper_alias, loop_var, walrus, fallback\n",
    );
    assert!(
        !codes(&diagnostics).contains(&"imports_missing_name"),
        "every module-level binding form must count as defined; got: {:?}",
        codes(&diagnostics)
    );

    fs::remove_dir_all(&root).unwrap();
}

/// The remaining binding statement forms — `while`/`with`/`match` bodies,
/// `except ... as`, `type` aliases, augmented assignment, unpacking — all
/// count as defined, and a genuinely missing name still surfaces beside them.
#[test]
fn structured_binding_forms_count_as_defined_without_masking_real_misses() {
    let root = make_tmp_dir("missing_name_structured_forms");
    fs::write(
        root.join("forms.py"),
        "type Alias = int\n\
         total = 0\n\
         total += 1\n\
         first, *rest = [1, 2, 3]\n\
         while False:\n    inside_while = 1\n\
         with open(__file__) as handle:\n    pass\n\
         try:\n    pass\nexcept ValueError as caught:\n    pass\n\
         match [1]:\n    case [head, *tail]:\n        pass\n    case {**mapping_rest}:\n        pass\n    case str() as bound:\n        pass\n",
    )
    .unwrap();

    let diagnostics = check_consumer(
        &root,
        "from forms import Alias, total, first, rest, inside_while, handle, \
         caught, head, tail, mapping_rest, bound, definitely_missing\n",
    );

    let missing: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code.code == "imports_missing_name")
        .collect();
    assert_eq!(
        missing.len(),
        1,
        "only `definitely_missing` may be flagged; got: {:?}",
        missing.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
    assert!(missing[0].message.contains("definitely_missing"));

    fs::remove_dir_all(&root).unwrap();
}

/// A target that does not parse has an unknowable surface — the rule must stay
/// silent (the target's own check reports the syntax error).
#[test]
fn unparseable_target_module_suppresses_the_rule() {
    let root = make_tmp_dir("missing_name_unparseable_target");
    fs::write(root.join("broken.py"), "def broken(:\n").unwrap();

    let diagnostics = check_consumer(&root, "from broken import anything\n");
    assert!(
        !codes(&diagnostics).contains(&"imports_missing_name"),
        "an unparseable target must suppress the rule; got: {:?}",
        codes(&diagnostics)
    );

    fs::remove_dir_all(&root).unwrap();
}

/// Submodules satisfy the import as `.pyi` stubs and as package directories,
/// not only as `.py` files.
#[test]
fn pyi_and_directory_submodules_satisfy_a_package_import() {
    let root = make_tmp_dir("missing_name_submodule_kinds");
    fs::create_dir_all(root.join("pkg/subpkg")).unwrap();
    fs::write(root.join("pkg/__init__.py"), "").unwrap();
    fs::write(root.join("pkg/stubbed.pyi"), "x: int\n").unwrap();
    fs::write(root.join("pkg/subpkg/__init__.py"), "").unwrap();

    let diagnostics = check_consumer(&root, "from pkg import stubbed, subpkg\n");
    assert!(
        !codes(&diagnostics).contains(&"imports_missing_name"),
        "`.pyi` and directory submodules must satisfy the import; got: {:?}",
        codes(&diagnostics)
    );

    fs::remove_dir_all(&root).unwrap();
}
