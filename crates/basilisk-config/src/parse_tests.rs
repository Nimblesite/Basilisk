//! Validation and merge tests for [CHKARCH-CONFIG-MODEL] / [CHKCACHE-CONFIG]
//! parsing in `parse.rs`. Split out to keep that file under the repository
//! size ceiling; `#[path]`-included, so it stays the same `validation_tests`
//! module it always was.

use std::collections::HashMap;
use std::path::PathBuf;

use super::{
    is_full_commit_sha, is_valid_distribution_name, parse_typeshed_package, BasiliskConfig,
    RuleSeverity,
};

/// [STUBRES-TYPESHED-CONFIG]: only a full 40-char hex SHA is a valid pin.
#[test]
fn full_sha_is_accepted_short_and_nonhex_rejected() {
    // The pinned typing-authority SHA from the plan — exactly 40 hex chars.
    assert!(is_full_commit_sha(
        "6ef9f7719ecfff09dad8724ef42b621fd994fb5e"
    ));
    // Upper-case identifies the same immutable commit.
    assert!(is_full_commit_sha(
        "6EF9F7719ECFFF09DAD8724EF42B621FD994FB5E"
    ));
    // Abbreviated (7-char) SHA — ambiguous, rejected so a pin fails closed.
    assert!(!is_full_commit_sha("6ef9f77"));
    // 39 and 41 chars — off-by-one lengths rejected.
    assert!(!is_full_commit_sha(
        "6ef9f7719ecfff09dad8724ef42b621fd994fb5"
    ));
    assert!(!is_full_commit_sha(
        "6ef9f7719ecfff09dad8724ef42b621fd994fb5ee"
    ));
    // Non-hex character (`g`) at full length — rejected.
    assert!(!is_full_commit_sha(
        "6ef9f7719ecfff09dad8724ef42b621fd994fb5g"
    ));
    // Empty — rejected.
    assert!(!is_full_commit_sha(""));
}

#[test]
fn nearer_typeshed_selection_replaces_inherited_selection_atomically() {
    let ancestor_pin = BasiliskConfig {
        typeshed_commit: Some("83c2518a9e6abbda0c44592c3483de459198f887".to_owned()),
        ..Default::default()
    };
    let child_path = BasiliskConfig {
        typeshed_path: Some(PathBuf::from("custom-typeshed")),
        ..Default::default()
    };
    let path_result = ancestor_pin.merged_with(child_path);
    assert_eq!(
        path_result.typeshed_path,
        Some(PathBuf::from("custom-typeshed"))
    );
    assert!(path_result.typeshed_commit.is_none());

    let ancestor_path = BasiliskConfig {
        typeshed_path: Some(PathBuf::from("parent-typeshed")),
        ..Default::default()
    };
    let child_pin = BasiliskConfig {
        typeshed_commit: Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
        ..Default::default()
    };
    let pin_result = ancestor_path.merged_with(child_pin);
    assert!(pin_result.typeshed_path.is_none());
    assert_eq!(
        pin_result.typeshed_commit.as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );
}

#[test]
fn malformed_same_table_path_and_pin_remain_visible_to_fail_closed() {
    let child = BasiliskConfig {
        typeshed_path: Some(PathBuf::from("custom-typeshed")),
        typeshed_commit: Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
        ..Default::default()
    };
    let merged = BasiliskConfig::default().merged_with(child);
    assert!(merged.typeshed_path.is_some());
    assert!(merged.typeshed_commit.is_some());
}

/// [STUBRES-TYPESHED-PYPI] (issue #312): `typeshed-package` is a third,
/// mutually-exclusive source selector — a child that sets it replaces an
/// inherited `typeshed-path`/`typeshed-commit` as a unit, and vice-versa, so a
/// merge can never manufacture a path+package or commit+package combination
/// that appeared in no source file.
#[test]
fn typeshed_package_replaces_inherited_selection_as_a_unit() {
    const PACKAGE_SPEC: &str =
        "micropython-stdlib-stubs@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let ancestor_pin = BasiliskConfig {
        typeshed_commit: Some("83c2518a9e6abbda0c44592c3483de459198f887".to_owned()),
        ..Default::default()
    };
    let child_package = BasiliskConfig {
        typeshed_package: Some(PACKAGE_SPEC.to_owned()),
        ..Default::default()
    };
    let result = ancestor_pin.merged_with(child_package);
    assert!(result.typeshed_commit.is_none());
    assert_eq!(result.typeshed_package.as_deref(), Some(PACKAGE_SPEC));

    // And the reverse: a child commit clears an inherited package.
    let ancestor_package = BasiliskConfig {
        typeshed_package: Some(PACKAGE_SPEC.to_owned()),
        ..Default::default()
    };
    let child_pin = BasiliskConfig {
        typeshed_commit: Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
        ..Default::default()
    };
    let result = ancestor_package.merged_with(child_pin);
    assert!(result.typeshed_package.is_none());
    assert!(result.typeshed_commit.is_some());
}

/// The 64-hex digest used across the pin-parsing tests, in upper case so the
/// accepting case also proves the parser canonicalises to lower.
const PACKAGE_DIGEST_UPPER: &str =
    "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855";

#[test]
fn typeshed_package_spec_shape_is_validated() {
    let spec = format!("micropython-stdlib-stubs@sha256:{PACKAGE_DIGEST_UPPER}");
    assert_eq!(
        parse_typeshed_package(&spec),
        Ok((
            "micropython-stdlib-stubs".to_owned(),
            PACKAGE_DIGEST_UPPER.to_ascii_lowercase(),
        )),
        "a well-formed pin yields the name plus the digest canonicalised to lower case"
    );
}

#[test]
fn typeshed_package_spec_rejects_malformed_pins() {
    // Each case pairs a malformed spec with the exact reason the user is shown,
    // so a rejection that stops explaining itself fails here too.
    const MALFORMED: &str = "typeshed-package must be of the form `name@sha256:<64-hex>`";
    const BAD_DIGEST: &str = "typeshed-package sha256 must be 64 hex characters";
    const NO_NAME: &str = "typeshed-package distribution name is empty";
    const BAD_NAME: &str =
        "typeshed-package distribution name must be a PEP 508 name: ASCII letters, digits, \
         `.`, `_`, or `-`, beginning and ending with a letter or digit";
    let cases = [
        ("micropython-stdlib-stubs", MALFORMED),
        (
            "name@md5:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            MALFORMED,
        ),
        ("name@sha256:abc", BAD_DIGEST),
        // 63 hex digits plus a non-hex `g` — right length, wrong alphabet.
        (
            "name@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b85g",
            BAD_DIGEST,
        ),
        (
            "@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            NO_NAME,
        ),
        // A spec with a second `@sha256:` in the name is rejected, not parsed as
        // a distribution literally named `name@sha256:x` — the two old twin
        // parsers disagreed here; the single parser closes that gap.
        (
            "name@sha256:x@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            BAD_DIGEST,
        ),
        // [STUBRES-TYPESHED-PYPI]: the name becomes a path segment of the PyPI
        // index URL, so anything outside the PEP 508 alphabet — a separator, a
        // query/fragment introducer, an escape, or a traversal — is refused at
        // the parser, before a request could ever be built from it.
        (
            "../typeshed@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            BAD_NAME,
        ),
        (
            "stubs/json@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            BAD_NAME,
        ),
        (
            "stubs?x=1@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            BAD_NAME,
        ),
        (
            "stubs%2f@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            BAD_NAME,
        ),
        // PEP 508 requires the first and last character to be alphanumeric, so
        // a leading or trailing separator is not a name either.
        (
            "-stubs@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            BAD_NAME,
        ),
        (
            "stubs.@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            BAD_NAME,
        ),
    ];
    for (spec, expected_reason) in cases {
        assert_eq!(
            parse_typeshed_package(spec),
            Err(expected_reason.to_owned()),
            "`{spec}` must be rejected with the reason that explains why"
        );
    }
}

/// [STUBRES-TYPESHED-PYPI]: the name check exists to keep a hostile value out
/// of the index URL, so it must not also reject the ordinary names real stub
/// distributions publish. Every separator PEP 508 allows is legal *between*
/// alphanumerics, in any case, including a single-character name.
#[test]
fn valid_distribution_names_cover_the_real_pep_508_alphabet() {
    for name in [
        "micropython-stdlib-stubs",
        "types_requests",
        "zope.interface",
        "Django",
        "ruamel.yaml.clib",
        "a",
        "A1",
        "backports.tarfile",
    ] {
        assert!(
            is_valid_distribution_name(name),
            "`{name}` is a PEP 508 distribution name and must be accepted"
        );
    }
}

/// Build a `[tool.basilisk.rules]`-shaped table directly, so the fixture
/// carries no `Result` to unwrap and can hold non-string values a TOML
/// severity table must still tolerate.
fn severity_table(entries: &[(&str, toml::Value)]) -> toml::Table {
    let mut table = toml::Table::new();
    for (key, value) in entries {
        let _ = table.insert((*key).to_owned(), value.clone());
    }
    table
}

/// [CHKARCH-STRICTNESS-SEVERITY]: every documented spelling — the four
/// canonical names and the four aliases — must reach the rule map, so a
/// config the docs sanction is never quietly a no-op.
#[test]
fn every_documented_severity_spelling_is_accepted() {
    let table = severity_table(&[
        ("a", toml::Value::from("error")),
        ("b", toml::Value::from("warning")),
        ("c", toml::Value::from("warn")),
        ("d", toml::Value::from("info")),
        ("e", toml::Value::from("information")),
        ("f", toml::Value::from("disabled")),
        ("g", toml::Value::from("off")),
        ("h", toml::Value::from("none")),
    ]);
    let mut parsed = HashMap::new();
    super::parse_severity_map(&table, &mut parsed);

    assert_eq!(parsed.get("a"), Some(&RuleSeverity::Error));
    assert_eq!(parsed.get("b"), Some(&RuleSeverity::Warning));
    assert_eq!(parsed.get("c"), Some(&RuleSeverity::Warning));
    assert_eq!(parsed.get("d"), Some(&RuleSeverity::Info));
    assert_eq!(parsed.get("e"), Some(&RuleSeverity::Info));
    assert_eq!(parsed.get("f"), Some(&RuleSeverity::Disabled));
    assert_eq!(parsed.get("g"), Some(&RuleSeverity::Disabled));
    assert_eq!(parsed.get("h"), Some(&RuleSeverity::Disabled));
    assert_eq!(parsed.len(), 8, "no documented spelling may be dropped");
}

/// [CHKARCH-CONFIG-MODEL]: a value that is not a severity is dropped rather
/// than coerced — a typo must never silently become `error`, and it must
/// never take a neighbouring valid entry down with it. The drop is
/// announced through `tracing::warn!` in `parse_severity_map`, because the
/// configuration editor rejects the same value outright and a silent run
/// would disagree with the editor about one file.
#[test]
fn unparseable_severities_are_dropped_without_disturbing_valid_entries() {
    let table = severity_table(&[
        ("typo", toml::Value::from("eror")),
        ("wrong_case", toml::Value::from("ERROR")),
        ("empty", toml::Value::from("")),
        ("numeric", toml::Value::from(3)),
        ("boolean", toml::Value::from(true)),
        ("listy", toml::Value::from(vec!["error"])),
        ("good", toml::Value::from("warning")),
    ]);
    let mut parsed = HashMap::new();
    super::parse_severity_map(&table, &mut parsed);

    for dropped in ["typo", "wrong_case", "empty", "numeric", "boolean", "listy"] {
        assert!(
            !parsed.contains_key(dropped),
            "`{dropped}` is not a severity and must not enter the rule map"
        );
    }
    assert_eq!(
        parsed.get("good"),
        Some(&RuleSeverity::Warning),
        "a malformed neighbour must not suppress a valid entry"
    );
    assert_eq!(parsed.len(), 1);
}

/// [CHKCACHE-CONFIG]: both persistent-cache keys parse from `[tool.basilisk]`
/// with their documented TOML types.
#[test]
fn cache_keys_parse_from_pyproject() {
    let cfg = super::parse_pyproject_content(
        "[tool.basilisk]\ncache = true\ncache-dir = \"build/bsk-cache\"\n",
    )
    .expect("a [tool.basilisk] table must parse");
    assert_eq!(cfg.cache_enabled, Some(true));
    assert_eq!(cfg.cache_dir, Some(PathBuf::from("build/bsk-cache")));
    assert!(cfg.cache_is_enabled());
}

/// [CHKCACHE-CONFIG]: unwritten keys stay `None` so the pre-existing default
/// (cache off, default folder) is untouched by the key's mere existence.
#[test]
fn cache_keys_default_to_unset_and_off() {
    let cfg =
        super::parse_pyproject_content("[tool.basilisk]\n").expect("an empty table must parse");
    assert!(cfg.cache_enabled.is_none());
    assert!(cfg.cache_dir.is_none());
    assert!(!cfg.cache_is_enabled(), "an unwritten `cache` key is off");
}

/// [CHKCACHE-CONFIG]: `cache = false` is a real, explicit opt-out — it must
/// reach the config as `Some(false)`, not collapse into "unset".
#[test]
fn explicit_cache_false_is_recorded() {
    let cfg = super::parse_pyproject_content("[tool.basilisk]\ncache = false\n")
        .expect("a [tool.basilisk] table must parse");
    assert_eq!(cfg.cache_enabled, Some(false));
    assert!(!cfg.cache_is_enabled());
}

/// [CHKCACHE-CONFIG]: a wrongly-typed value is dropped rather than coerced —
/// `cache = "yes"` must never read as `true`.
#[test]
fn wrongly_typed_cache_values_are_not_coerced() {
    let cfg = super::parse_pyproject_content("[tool.basilisk]\ncache = \"yes\"\ncache-dir = 7\n")
        .expect("a [tool.basilisk] table must parse");
    assert!(cfg.cache_enabled.is_none());
    assert!(cfg.cache_dir.is_none());
}

/// [CHKCACHE-CONFIG]: the nearer directory wins per key, exactly like the
/// other non-rule fields ([CHKARCH-CONFIG-DISCOVERY]).
#[test]
fn nearer_cache_settings_win_per_key() {
    let ancestor = BasiliskConfig {
        cache_enabled: Some(false),
        cache_dir: Some(PathBuf::from("outer")),
        ..Default::default()
    };
    let child = BasiliskConfig {
        cache_enabled: Some(true),
        ..Default::default()
    };
    let merged = ancestor.merged_with(child);
    assert_eq!(merged.cache_enabled, Some(true), "the child key wins");
    assert_eq!(
        merged.cache_dir,
        Some(PathBuf::from("outer")),
        "a key the child does not state inherits the ancestor"
    );
}

/// [CHKCACHE-CONFIG]: the default location is `.basilisk/cache/check` under
/// the project root — the same folder the CLI has always used.
#[test]
fn default_cache_directory_is_under_the_project_root() {
    let root = PathBuf::from("/projects/demo");
    let resolved = BasiliskConfig::default().cache_directory(&root);
    assert_eq!(resolved, root.join(".basilisk").join("cache").join("check"));
}

/// [CHKCACHE-CONFIG]: a relative `cache-dir` anchors to the project root, so
/// the folder does not move with the caller's working directory.
#[test]
fn relative_cache_directory_resolves_against_the_project_root() {
    let root = PathBuf::from("/projects/demo");
    let cfg = BasiliskConfig {
        cache_dir: Some(PathBuf::from("build/cache")),
        ..Default::default()
    };
    assert_eq!(cfg.cache_directory(&root), root.join("build").join("cache"));
}

/// [CHKCACHE-CONFIG]: an absolute `cache-dir` is used verbatim — a shared
/// cache outside the project is the whole point of setting one.
#[test]
fn absolute_cache_directory_is_used_verbatim() {
    let absolute = if cfg!(windows) {
        PathBuf::from(r"C:\shared\bsk")
    } else {
        PathBuf::from("/shared/bsk")
    };
    let cfg = BasiliskConfig {
        cache_dir: Some(absolute.clone()),
        ..Default::default()
    };
    assert_eq!(
        cfg.cache_directory(&PathBuf::from("/projects/demo")),
        absolute
    );
}
