//! Tests for [CHKARCH-VERSION-TARGET] / [CHKARCH-VERSION-NARROWING].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-VERSION-TARGET
//!
//! Issue #93: the configured `python_version` must reach every rule. `directives_version_platform`
//! dead-branch analysis and the `version_target_syntax` PEP 695 syntax gate must follow the
//! *configured* target, not a hardcoded 3.12.

use super::common::*;

/// A guard whose dead branch flips with the target version: on 3.12 the
/// if-body is dead (`val`), on 3.9 the else-body is dead (`other`).
const VERSION_GUARD_SOURCE: &str = concat!(
    "import sys\n",
    "\n",
    "def f():\n",
    "    if sys.version_info < (3, 10):\n",
    "        val = \"\"\n",
    "    else:\n",
    "        other = \"\"\n",
    "    print(val)\n",
    "    print(other)\n",
);

fn run_with_python_version(source: &str, version: &str) -> Vec<Diagnostic> {
    let parsed = parse_source(source.to_owned(), "test.py".to_owned()).unwrap();
    let resolved = resolve(&parsed).unwrap();
    let config = basilisk_config::BasiliskConfig {
        python_version: Some(version.to_owned()),
        ..Default::default()
    };
    basilisk_checker::check_with_config(&resolved, &config)
}

#[test]
fn test_e0150_default_target_flags_sub_3_10_branch() {
    let diags = run(VERSION_GUARD_SOURCE).unwrap();
    let messages = messages_for(&diags, "directives_version_platform");
    assert!(
        messages.iter().any(|m| m.contains("`val`")),
        "on the default 3.12 target the `< (3, 10)` body is dead: {messages:?}"
    );
    assert!(
        !messages.iter().any(|m| m.contains("`other`")),
        "the else body is live on 3.12: {messages:?}"
    );
}

#[test]
fn test_e0150_follows_configured_3_9_target() {
    let diags = run_with_python_version(VERSION_GUARD_SOURCE, "3.9");
    let messages = messages_for(&diags, "directives_version_platform");
    assert!(
        messages.iter().any(|m| m.contains("`other`")),
        "on a 3.9 target the else body is dead: {messages:?}"
    );
    assert!(
        !messages.iter().any(|m| m.contains("`val`")),
        "the `< (3, 10)` body is live on 3.9 — flagging it is a false positive: {messages:?}"
    );
}

#[test]
fn test_e0155_pep695_type_alias_below_3_12() {
    let diags = run_with_python_version("type Alias = int\n", "3.11");
    assert!(
        has_code(&diags, "version_target_syntax"),
        "PEP 695 `type` statement requires Python 3.12+, target is 3.11"
    );
}

#[test]
fn test_e0155_pep695_generic_class_below_3_12() {
    let diags = run_with_python_version("class Box[T]:\n    pass\n", "3.10");
    assert!(
        has_code(&diags, "version_target_syntax"),
        "PEP 695 `class Box[T]` requires Python 3.12+, target is 3.10"
    );
}

#[test]
fn test_e0155_pep695_generic_function_below_3_12() {
    let diags = run_with_python_version(
        "def first[T](items: list[T]) -> T:\n    return items[0]\n",
        "3.11",
    );
    assert!(
        has_code(&diags, "version_target_syntax"),
        "PEP 695 `def first[T]` requires Python 3.12+, target is 3.11"
    );
}

#[test]
fn test_e0155_silent_on_3_12_target() {
    let diags = run_with_python_version(
        "type Alias = int\nclass Box[T]:\n    pass\ndef first[T](items: list[T]) -> T:\n    return items[0]\n",
        "3.12",
    );
    assert!(
        !has_code(&diags, "version_target_syntax"),
        "PEP 695 syntax is native on 3.12 — no diagnostic"
    );
}

#[test]
fn test_e0155_silent_without_configured_version() {
    // The centralized default target is 3.12 — PEP 695 is allowed.
    let diags = run("type Alias = int\n").unwrap();
    assert!(!has_code(&diags, "version_target_syntax"));
}

#[test]
fn test_python_version_parsed_from_pyproject() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pyproject.toml");
    std::fs::write(
        &path,
        "[tool.basilisk]\npython-version = \"3.9\"\npython-platform = \"linux\"\n",
    )
    .unwrap();
    let config = basilisk_config::load_basilisk_config(dir.path());
    assert_eq!(config.python_version.as_deref(), Some("3.9"));
    assert_eq!(config.python_platform.as_deref(), Some("linux"));
}

#[test]
fn test_malformed_python_version_falls_back_to_default() {
    // An unparsable version must behave exactly like the 3.12 default,
    // not panic and not produce spurious version gating.
    let diags = run_with_python_version("type Alias = int\n", "not-a-version");
    assert!(!has_code(&diags, "version_target_syntax"));
}
