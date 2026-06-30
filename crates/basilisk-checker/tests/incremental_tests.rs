//! Tests for [CHKARCH-INCREMENTAL-SALSA]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-INCREMENTAL-SALSA
#![allow(
    clippy::allow_attributes,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::as_conversions,
    missing_docs
)]
//! Behavioural tests for the Salsa-memoized `checked_file` diagnostics query.
//!
//! The decisive property is that salsa memoization is **transparent**: the
//! memoized query returns exactly what the direct `parse → resolve → check`
//! pipeline returns — same diagnostics, every field preserved through the
//! `CachedDiagnostic` round-trip — so wrapping the checker in salsa can never
//! corrupt a result. (This is equivalence to the *pure* pipeline, not to the
//! batch CLI, which additionally resolves imports against the venv; see
//! `checked_file`'s docs.) The remaining tests prove the query is genuinely
//! incremental — it memoizes, invalidates on a source OR config edit, honours
//! the configuration, and isolates files from each other — using a database
//! that records salsa's `WillExecute` events.

use std::collections::HashMap;

use basilisk_checker::{
    checked_file, file_diagnostics, ConfigInput, ConfigValue, Diagnostic, SourceFile,
};
use basilisk_config::{BasiliskConfig, ModuleOverride, PathOverride};
use basilisk_test_utils::EventDb;
use salsa::Setter;

/// Source snippets exercised by the equivalence test: a clean file, a PEP type
/// error, a multi-statement file, an import-bearing file (which drives the
/// `imports_unresolved` rule and its help/provenance fields through the
/// round-trip), and an empty file.
const SAMPLES: &[(&str, &str)] = &[
    ("clean.py", "x: int = 1\n"),
    ("bad_assign.py", "x: int = \"not an int\"\n"),
    (
        "multi.py",
        "def f(a: int) -> int:\n    return a\n\ny: str = f(1)\n",
    ),
    (
        "imports.py",
        "import nonexistent_pkg\n\nx = nonexistent_pkg.frobnicate()\n",
    ),
    ("empty.py", "\n"),
];

/// The default-configuration `ConfigInput`, the equivalent of [`basilisk_checker::check`].
fn default_config(db: &EventDb) -> ConfigInput {
    ConfigInput::new(db, ConfigValue(BasiliskConfig::default()))
}

/// A `ConfigInput` with the opt-in strict-annotation rules enabled.
fn strict_config(db: &EventDb) -> ConfigInput {
    ConfigInput::new(
        db,
        ConfigValue(BasiliskConfig {
            strict_annotations: true,
            ..BasiliskConfig::default()
        }),
    )
}

/// The reference pipeline the query must match exactly.
fn reference_diagnostics(path: &str, text: &str) -> Vec<Diagnostic> {
    let parsed = basilisk_parser::parse_source(text.to_owned(), path.to_owned()).expect("parse");
    let resolved = basilisk_resolver::resolve(&parsed).expect("resolve");
    basilisk_checker::check(&resolved)
}

fn assert_same(got: &[Diagnostic], want: &[Diagnostic], label: &str) {
    assert_eq!(
        got.len(),
        want.len(),
        "{label}: diagnostic count must match the direct pipeline"
    );
    // Compare EVERY field — `file_diagnostics` round-trips through
    // `CachedDiagnostic`, so any field the projection drops must surface here.
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g.code.code, w.code.code, "{label}: code");
        assert_eq!(g.code.docs_url, w.code.docs_url, "{label}: docs_url");
        assert_eq!(g.span, w.span, "{label}: span");
        assert_eq!(g.path, w.path, "{label}: path");
        assert_eq!(g.message, w.message, "{label}: message");
        assert_eq!(g.severity, w.severity, "{label}: severity");
        assert_eq!(g.help, w.help, "{label}: help");
        assert_eq!(g.note, w.note, "{label}: note");
        assert_eq!(g.provenance, w.provenance, "{label}: provenance");
    }
}

#[test]
fn checked_file_is_equivalent_to_direct_check() {
    let db = EventDb::default();
    let config = default_config(&db);
    for (path, src) in SAMPLES {
        let file = SourceFile::new(&db, (*path).to_owned(), (*src).to_owned());
        let got = file_diagnostics(&db, file, config);
        let want = reference_diagnostics(path, src);
        assert_same(&got, &want, path);
    }
}

#[test]
fn unparseable_file_yields_no_diagnostics() {
    // A file that fails to parse produces no diagnostics — identical to the
    // batch CLI, which skips such files. Exercises the query's parse-error path.
    let db = EventDb::default();
    let config = default_config(&db);
    let file = SourceFile::new(&db, "broken.py".to_owned(), "def (= :\n".to_owned());
    assert!(
        file_diagnostics(&db, file, config).is_empty(),
        "an unparseable file must yield no diagnostics"
    );
    assert!(
        checked_file(&db, file, config).is_empty(),
        "the memoized projection is empty too"
    );
}

#[test]
fn checked_file_memoizes_then_invalidates_on_edit() {
    let mut db = EventDb::default();
    let file = SourceFile::new(&db, "a.py".to_owned(), "x: int = 1\n".to_owned());
    let config = default_config(&db);

    let _first = checked_file(&db, file, config);
    assert_eq!(
        db.executions_of("checked_file"),
        1,
        "the first check executes the query once"
    );

    let _second = checked_file(&db, file, config);
    assert_eq!(
        db.executions_of("checked_file"),
        0,
        "re-checking an unchanged file is served from the memo — no re-execution"
    );

    let _previous = file.set_text(&mut db).to("x: int = \"oops\"\n".to_owned());
    let _third = checked_file(&db, file, config);
    assert_eq!(
        db.executions_of("checked_file"),
        1,
        "editing the file re-executes the query exactly once"
    );
}

#[test]
fn editing_one_file_does_not_recheck_another() {
    let mut db = EventDb::default();
    let a = SourceFile::new(&db, "a.py".to_owned(), "x: int = 1\n".to_owned());
    let b = SourceFile::new(&db, "b.py".to_owned(), "y: int = 2\n".to_owned());
    let config = default_config(&db);

    let _a = checked_file(&db, a, config);
    let _b = checked_file(&db, b, config);
    let _ = db.executions_of("checked_file"); // drain priming

    let _previous = b.set_text(&mut db).to("y: int = \"bad\"\n".to_owned());

    let _a2 = checked_file(&db, a, config);
    assert_eq!(
        db.executions_of("checked_file"),
        0,
        "file A must NOT be rechecked when only file B changed"
    );

    let _b2 = checked_file(&db, b, config);
    assert_eq!(
        db.executions_of("checked_file"),
        1,
        "file B must be rechecked after its own edit"
    );
}

/// The query must honour the configuration, not a hard-wired default: the same
/// source yields BSK-E0001 (an opt-in strict-annotation rule) only when the
/// `ConfigInput` enables `strict_annotations`. Kills the `check_with_config` →
/// `check` mutant. [CHKARCH-CONFIGURATION-ONLY]
#[test]
fn checked_file_honours_strict_annotations() {
    let db = EventDb::default();
    // An unannotated parameter: spec-valid under default config, BSK-E0001 under
    // the opt-in strict-annotation rules.
    let file = SourceFile::new(
        &db,
        "f.py".to_owned(),
        "def f(x):\n    return x\n".to_owned(),
    );

    let default_diags = file_diagnostics(&db, file, default_config(&db));
    assert!(
        !default_diags.iter().any(|d| d.code.code == "BSK-E0001"),
        "with the default config the opt-in rule BSK-E0001 must NOT fire"
    );

    let strict_diags = file_diagnostics(&db, file, strict_config(&db));
    assert!(
        strict_diags.iter().any(|d| d.code.code == "BSK-E0001"),
        "with strict_annotations the engine must surface BSK-E0001 for the unannotated parameter"
    );
}

/// Editing the `ConfigInput` invalidates and re-runs the query exactly once, and
/// the new diagnostics reflect the new config. Kills the mutant where `config`
/// is a key but never read (so salsa records no dependency edge on it).
#[test]
fn editing_config_input_invalidates_checked_file() {
    let mut db = EventDb::default();
    let file = SourceFile::new(
        &db,
        "f.py".to_owned(),
        "def f(x):\n    return x\n".to_owned(),
    );
    let config = default_config(&db);

    let _first = checked_file(&db, file, config);
    let _ = db.executions_of("checked_file"); // drain priming

    let _previous = config.set_value(&mut db).to(ConfigValue(BasiliskConfig {
        strict_annotations: true,
        ..BasiliskConfig::default()
    }));

    let after = file_diagnostics(&db, file, config);
    assert_eq!(
        db.executions_of("checked_file"),
        1,
        "editing the config input must re-execute the query exactly once"
    );
    assert!(
        after.iter().any(|d| d.code.code == "BSK-E0001"),
        "after enabling strict_annotations the re-checked file must surface BSK-E0001"
    );
}

/// Editing nested override maps (not just scalar fields) invalidates the query,
/// proving the config's *value identity* — down through the `ModuleOverride` and
/// `PathOverride` maps — drives salsa invalidation.
#[test]
fn editing_override_maps_reinvalidates() {
    let mut db = EventDb::default();
    let file = SourceFile::new(&db, "f.py".to_owned(), "x: int = 1\n".to_owned());

    let base = BasiliskConfig {
        per_module_overrides: HashMap::from([(
            "m".to_owned(),
            ModuleOverride {
                ignore_missing_stubs: false,
            },
        )]),
        per_path_overrides: HashMap::from([(
            "/p".to_owned(),
            PathOverride {
                disabled_rules: Vec::new(),
                rule_overrides: HashMap::new(),
            },
        )]),
        ..BasiliskConfig::default()
    };
    let config = ConfigInput::new(&db, ConfigValue(base.clone()));

    let _first = checked_file(&db, file, config);
    let _ = db.executions_of("checked_file"); // drain priming

    // Edit A: flip the per-module override value (same key) — drives ModuleOverride::eq.
    let cfg_a = BasiliskConfig {
        per_module_overrides: HashMap::from([(
            "m".to_owned(),
            ModuleOverride {
                ignore_missing_stubs: true,
            },
        )]),
        ..base.clone()
    };
    let _previous_a = config.set_value(&mut db).to(ConfigValue(cfg_a.clone()));
    let _a = checked_file(&db, file, config);
    assert_eq!(
        db.executions_of("checked_file"),
        1,
        "changing a per-module override must re-execute the query (drives ModuleOverride::eq)"
    );

    // Edit B: change a per-path override's disabled_rules, module map held
    // identical — forces PathOverride::eq past the equal ModuleOverride map.
    let cfg_b = BasiliskConfig {
        per_path_overrides: HashMap::from([(
            "/p".to_owned(),
            PathOverride {
                disabled_rules: vec!["imports_unresolved".to_owned()],
                rule_overrides: HashMap::new(),
            },
        )]),
        ..cfg_a
    };
    let _previous_b = config.set_value(&mut db).to(ConfigValue(cfg_b));
    let _b = checked_file(&db, file, config);
    assert_eq!(
        db.executions_of("checked_file"),
        1,
        "changing a per-path override must re-execute the query (drives PathOverride::eq)"
    );
}

/// Direct coverage of the new `PartialEq`/`Eq` derives on `BasiliskConfig` and
/// its override structs: equal configs compare equal, and a difference in either
/// override map (or a scalar field) compares unequal. This pins value identity
/// independently of salsa's internal use of it.
#[test]
fn config_value_equality_distinguishes_overrides() {
    let base = BasiliskConfig {
        per_module_overrides: HashMap::from([(
            "m".to_owned(),
            ModuleOverride {
                ignore_missing_stubs: false,
            },
        )]),
        per_path_overrides: HashMap::from([(
            "/p".to_owned(),
            PathOverride {
                disabled_rules: Vec::new(),
                rule_overrides: HashMap::new(),
            },
        )]),
        ..BasiliskConfig::default()
    };

    assert_eq!(
        ConfigValue(base.clone()),
        ConfigValue(base.clone()),
        "structurally identical configs must compare equal"
    );

    let module_diff = BasiliskConfig {
        per_module_overrides: HashMap::from([(
            "m".to_owned(),
            ModuleOverride {
                ignore_missing_stubs: true,
            },
        )]),
        ..base.clone()
    };
    assert_ne!(
        ConfigValue(base.clone()),
        ConfigValue(module_diff),
        "a per-module override difference must compare unequal (ModuleOverride::eq)"
    );

    let path_diff = BasiliskConfig {
        per_path_overrides: HashMap::from([(
            "/p".to_owned(),
            PathOverride {
                disabled_rules: vec!["imports_unresolved".to_owned()],
                rule_overrides: HashMap::new(),
            },
        )]),
        ..base.clone()
    };
    assert_ne!(
        ConfigValue(base.clone()),
        ConfigValue(path_diff),
        "a per-path override difference must compare unequal (PathOverride::eq)"
    );

    let scalar_diff = BasiliskConfig {
        strict_annotations: true,
        ..base.clone()
    };
    assert_ne!(
        ConfigValue(base),
        ConfigValue(scalar_diff),
        "a scalar field difference must compare unequal"
    );
}
