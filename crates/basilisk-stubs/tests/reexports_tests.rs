//! Tests for [STUBRES-PYI-REEXPORTS]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-PYI-REEXPORTS
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic
)]
//! Re-export extraction (`__all__`, redundant aliases, `from … import *`) and
//! star-import resolution relative to the stub file (GitHub #312).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use basilisk_stubs::reexports::reexported_member_names_with_loader;
use basilisk_stubs::types::{StubSource, StubTier};
use basilisk_stubs::{parse_pyi_file, parse_pyi_source, reexported_member_names, StubModule};

static TEST_CTR: AtomicU64 = AtomicU64::new(0);

/// Create a unique temp dir so parallel tests never collide.
fn make_tmp_dir(prefix: &str) -> PathBuf {
    let n = TEST_CTR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("{prefix}_{n}_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn parse_stub(source: &str) -> StubModule {
    parse_pyi_source(
        source,
        Path::new("test.pyi"),
        "test",
        StubSource::UserStub,
        StubTier::Tier1,
    )
    .expect("stub should parse")
}

fn parse_named_stub(source: &str, path: &str, module_name: &str) -> StubModule {
    parse_pyi_source(
        source,
        Path::new(path),
        module_name,
        StubSource::Typeshed,
        StubTier::Tier1,
    )
    .expect("stub should parse")
}

fn parse_file(path: &Path) -> StubModule {
    parse_pyi_file(path, "pkg", StubSource::UserStub, StubTier::Tier1).expect("stub should parse")
}

fn names(stub: &StubModule) -> HashSet<String> {
    reexported_member_names(stub)
}

// ── Extraction (parser-level) ────────────────────────────────────────────────

#[test]
fn redundant_alias_from_import_is_reexported() {
    let stub = parse_stub("from _core import Task as Task\nfrom _core import quiet\n");
    assert_eq!(stub.reexported_names, vec!["Task".to_owned()]);
    assert!(
        names(&stub).contains("Task"),
        "`from y import x as x` re-exports `x`"
    );
    assert!(
        !names(&stub).contains("quiet"),
        "a plain `from y import z` is NOT a re-export"
    );
}

#[test]
fn redundant_alias_plain_import_is_reexported() {
    let stub = parse_stub("import machine as machine\nimport sys\nimport os.path as osp\n");
    assert_eq!(stub.reexported_names, vec!["machine".to_owned()]);
}

#[test]
fn dunder_all_list_tuple_and_augmented_entries_union() {
    let stub = parse_stub("__all__ = [\"a\", \"b\"]\n__all__ += (\"c\",)\n");
    assert_eq!(
        stub.dunder_all,
        Some(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()])
    );
    let exported = names(&stub);
    for name in ["a", "b", "c"] {
        assert!(exported.contains(name), "`{name}` listed in __all__");
    }
}

#[test]
fn dunder_all_mutation_methods_apply_in_source_order() {
    let stub = parse_stub(
        "__all__ = [\"discarded\"]\n__all__ = [\"base\"]\n__all__.append(\"appended\")\n__all__.extend((\"extended\", \"removed\"))\n__all__.remove(\"removed\")\n",
    );
    let exported = names(&stub);
    assert_eq!(
        exported,
        HashSet::from([
            "base".to_owned(),
            "appended".to_owned(),
            "extended".to_owned(),
        ]),
        "assignment replaces the prior list and append/extend/remove mutate it"
    );
}

#[test]
fn unknown_platform_intersects_guarded_dunder_all_branches() {
    let stub = parse_stub(
        "import sys\nif sys.platform == \"win32\":\n    __all__ = (\"common\", \"windows_only\")\nelse:\n    __all__ = (\"common\", \"posix_only\")\n",
    );
    assert_eq!(
        names(&stub),
        HashSet::from(["common".to_owned()]),
        "without concrete platform evidence only names valid in every branch are exposed"
    );
}

#[test]
fn non_literal_dunder_all_is_ignored() {
    let stub = parse_stub("__all__ = _compute()\n");
    assert_eq!(stub.dunder_all, None);
}

#[test]
fn star_import_levels_and_modules_are_recorded() {
    let stub = parse_stub("from .tasks import *\nfrom ..sib import *\nfrom abs_mod import *\n");
    let star: Vec<(String, u32)> = stub
        .star_reexports
        .iter()
        .map(|s| (s.module.clone(), s.level))
        .collect();
    assert_eq!(
        star,
        vec![
            ("tasks".to_owned(), 1),
            ("sib".to_owned(), 2),
            ("abs_mod".to_owned(), 0)
        ]
    );
}

// ── Resolution (filesystem-level) ────────────────────────────────────────────

#[test]
fn star_target_with_dunder_all_exports_exactly_dunder_all() {
    let pkg = make_tmp_dir("bsk_rx_all").join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join("__init__.pyi"), "from .tasks import *\n").unwrap();
    // `__all__` is authoritative: `_private` IS exported, `extra` is NOT.
    fs::write(
        pkg.join("tasks.pyi"),
        "__all__ = (\"sleep\", \"_private\")\n\ndef sleep(d: float) -> None: ...\ndef extra() -> None: ...\n",
    )
    .unwrap();

    let exported = names(&parse_file(&pkg.join("__init__.pyi")));
    assert!(exported.contains("sleep"));
    assert!(exported.contains("_private"));
    assert!(
        !exported.contains("extra"),
        "with __all__ defined, star import brings exactly __all__"
    );

    let _ = fs::remove_dir_all(pkg.parent().unwrap());
}

#[test]
fn dunder_all_can_copy_and_mutate_a_submodule_dunder_all() {
    let pkg = make_tmp_dir("bsk_rx_module_all").join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(
        pkg.join("__init__.pyi"),
        "from . import exports\n__all__ = exports.__all__\n__all__.append(\"local\")\n__all__.remove(\"removed\")\n",
    )
    .unwrap();
    fs::write(
        pkg.join("exports.pyi"),
        "__all__ = (\"copied\", \"removed\")\n",
    )
    .unwrap();

    assert_eq!(
        names(&parse_file(&pkg.join("__init__.pyi"))),
        HashSet::from(["copied".to_owned(), "local".to_owned()])
    );

    let _ = fs::remove_dir_all(pkg.parent().unwrap());
}

#[test]
fn star_target_without_dunder_all_exports_public_names_recursively() {
    let pkg = make_tmp_dir("bsk_rx_pub").join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join("__init__.pyi"), "from .core import *\n").unwrap();
    // No `__all__`: public defs + redundant-alias re-exports (public only) +
    // transitive star imports; `_hidden` stays private.
    fs::write(
        pkg.join("core.pyi"),
        "from ._impl import Task as Task\nfrom ._impl import _Guts as _Guts\nfrom .extras import *\n\nclass Widget: ...\ndef _hidden() -> None: ...\nLIMIT: int\n",
    )
    .unwrap();
    fs::write(pkg.join("_impl.pyi"), "class Task: ...\nclass _Guts: ...\n").unwrap();
    fs::write(pkg.join("extras.pyi"), "def bonus() -> None: ...\n").unwrap();

    let exported = names(&parse_file(&pkg.join("__init__.pyi")));
    for name in ["Task", "Widget", "LIMIT", "bonus"] {
        assert!(exported.contains(name), "missing `{name}`: {exported:?}");
    }
    for name in ["_hidden", "_Guts"] {
        assert!(!exported.contains(name), "`{name}` is private");
    }

    let _ = fs::remove_dir_all(pkg.parent().unwrap());
}

#[test]
fn star_target_may_be_a_package_init() {
    let pkg = make_tmp_dir("bsk_rx_subpkg").join("pkg");
    fs::create_dir_all(pkg.join("sub")).unwrap();
    fs::write(pkg.join("__init__.pyi"), "from .sub import *\n").unwrap();
    fs::write(
        pkg.join("sub").join("__init__.pyi"),
        "def nested() -> None: ...\n",
    )
    .unwrap();

    let exported = names(&parse_file(&pkg.join("__init__.pyi")));
    assert!(exported.contains("nested"));

    let _ = fs::remove_dir_all(pkg.parent().unwrap());
}

#[test]
fn dotted_and_parent_level_star_targets_resolve() {
    let root = make_tmp_dir("bsk_rx_dotted");
    let pkg = root.join("pkg");
    fs::create_dir_all(pkg.join("inner")).unwrap();
    // `from .inner.leaf import *` (dotted) and `from ..sibling import *`
    // (level 2, resolved against the stub's grandparent dir).
    fs::write(
        pkg.join("__init__.pyi"),
        "from .inner.leaf import *\nfrom ..sibling import *\n",
    )
    .unwrap();
    fs::write(
        pkg.join("inner").join("leaf.pyi"),
        "def leaf_fn() -> None: ...\n",
    )
    .unwrap();
    fs::write(root.join("sibling.pyi"), "def sib_fn() -> None: ...\n").unwrap();

    let exported = names(&parse_file(&pkg.join("__init__.pyi")));
    assert!(exported.contains("leaf_fn"));
    assert!(exported.contains("sib_fn"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn absolute_and_relative_star_targets_are_followed() {
    let root = make_tmp_dir("bsk_rx_absolute");
    let pkg = root.join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(
        pkg.join("__init__.pyi"),
        "from shared import *\nfrom .relative import *\n",
    )
    .unwrap();
    fs::write(
        root.join("shared.pyi"),
        "def absolute_name() -> None: ...\n",
    )
    .unwrap();
    fs::write(
        pkg.join("relative.pyi"),
        "def relative_name() -> None: ...\n",
    )
    .unwrap();

    let stub = parse_file(&pkg.join("__init__.pyi"));
    let exported = names(&stub);
    assert!(exported.contains("absolute_name"), "{exported:?}");
    assert!(exported.contains("relative_name"), "{exported:?}");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn star_import_cycles_terminate_and_union_names() {
    let pkg = make_tmp_dir("bsk_rx_cycle").join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join("__init__.pyi"), "from .a import *\n").unwrap();
    fs::write(
        pkg.join("a.pyi"),
        "from .b import *\n\ndef from_a() -> None: ...\n",
    )
    .unwrap();
    fs::write(
        pkg.join("b.pyi"),
        "from .a import *\n\ndef from_b() -> None: ...\n",
    )
    .unwrap();

    let exported = names(&parse_file(&pkg.join("__init__.pyi")));
    assert!(exported.contains("from_a"));
    assert!(exported.contains("from_b"));

    let _ = fs::remove_dir_all(pkg.parent().unwrap());
}

#[test]
fn star_import_chain_has_no_arbitrary_depth_cap() {
    let pkg = make_tmp_dir("bsk_rx_depth").join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    // Regression: the old implementation silently stopped after sixteen hops.
    // Cycles are bounded by the visited set, so a valid finite chain is complete.
    fs::write(pkg.join("__init__.pyi"), "from .m1 import *\n").unwrap();
    for i in 1..=20 {
        let next = if i < 20 {
            format!("from .m{} import *\n", i + 1)
        } else {
            String::new()
        };
        fs::write(
            pkg.join(format!("m{i}.pyi")),
            format!("{next}def marker_{i}() -> None: ...\n"),
        )
        .unwrap();
    }

    let exported = names(&parse_file(&pkg.join("__init__.pyi")));
    assert!(exported.contains("marker_1"));
    assert!(
        exported.contains("marker_20"),
        "a valid 20-deep star chain must be followed completely, got {exported:?}"
    );

    let _ = fs::remove_dir_all(pkg.parent().unwrap());
}

#[test]
fn archive_style_loader_follows_names_without_filesystem_paths() {
    let root = parse_named_stub(
        "from .a import *\n",
        "typeshed:snapshot/stdlib/pkg/__init__.pyi",
        "pkg",
    );
    let a = parse_named_stub(
        "from pkg.b import *\ndef from_a() -> None: ...\n",
        "typeshed:snapshot/stdlib/pkg/a.pyi",
        "pkg.a",
    );
    let b = parse_named_stub(
        "from pkg.a import *\ndef from_b() -> None: ...\n",
        "typeshed:snapshot/stdlib/pkg/b.pyi",
        "pkg.b",
    );
    let modules = HashMap::from([("pkg.a".to_owned(), a), ("pkg.b".to_owned(), b)]);
    let mut requested_modules = Vec::new();
    let mut module_loader = |module_name: &str| {
        requested_modules.push(module_name.to_owned());
        modules.get(module_name).cloned()
    };

    let exported = reexported_member_names_with_loader(&root, &mut module_loader);
    assert!(exported.contains("from_a"), "{exported:?}");
    assert!(exported.contains("from_b"), "{exported:?}");
    assert!(requested_modules.contains(&"pkg.a".to_owned()));
    assert!(requested_modules.contains(&"pkg.b".to_owned()));
}

#[test]
fn qualified_class_method_keys_are_not_module_members() {
    let pkg = make_tmp_dir("bsk_rx_qualified").join("pkg");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join("__init__.pyi"), "from .core import *\n").unwrap();
    // The extractor also stores `Widget.render` in `functions`; star exports
    // must expose `Widget` only, never the qualified method key.
    fs::write(
        pkg.join("core.pyi"),
        "class Widget:\n    def render(self) -> None: ...\n",
    )
    .unwrap();

    let exported = names(&parse_file(&pkg.join("__init__.pyi")));
    assert!(exported.contains("Widget"));
    assert!(!exported.iter().any(|n| n.contains('.')));

    let _ = fs::remove_dir_all(pkg.parent().unwrap());
}
