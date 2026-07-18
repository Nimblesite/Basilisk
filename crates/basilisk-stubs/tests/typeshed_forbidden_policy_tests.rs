//! Aggregate regression guard for [TYPESHEDRT-ACCEPTANCE-GATES] forbidden policy.
#![allow(missing_docs)]

const CACHE: &str = include_str!("../src/typeshed/cache.rs");
const RUNTIME: &str = include_str!("../src/typeshed/runtime.rs");
const SOURCE: &str = include_str!("../src/typeshed/source.rs");
const TRANSPORT: &str = include_str!("../src/typeshed/transport.rs");
const BUILD: &str = include_str!("../build.rs");
const CHECK_CONTEXT: &str = include_str!("../../basilisk-checker/src/context.rs");

#[test]
fn production_sources_reject_forbidden_typeshed_policy() {
    let acquisition_sources = [
        ("cache", CACHE),
        ("runtime", RUNTIME),
        ("source", SOURCE),
        ("transport", TRANSPORT),
    ];
    let forbidden = [
        "std::process::Command",
        "Command::new",
        "git2::",
        "gix::",
        "CACHE_MAX_AGE_SECONDS",
        "load_fresh",
        "acquired_at_unix_seconds",
        "SystemTime",
        "UNIX_EPOCH",
        "python_version_to_commit",
        "python-version-to-sha",
        "stale_checkout",
    ];

    for (source_name, source) in acquisition_sources {
        for token in forbidden {
            assert!(
                !source.contains(token),
                "forbidden Typeshed policy token `{token}` appeared in {source_name}"
            );
        }
    }
    assert!(
        !BUILD.contains("python_version_to_commit")
            && !BUILD.contains("python-version-to-sha")
            && !BUILD.contains("DEFAULT_PYTHON_VERSION"),
        "generated indexes must not choose a Typeshed commit or manufacture a Python target"
    );
    assert!(
        CHECK_CONTEXT.contains("target_version: config")
            && CHECK_CONTEXT.contains("target_platform: config.python_platform.clone()"),
        "checker targets must remain evidence-derived from project/interpreter configuration"
    );
}

#[test]
fn immutable_pins_cache_and_custom_paths_remain_supported() {
    for required in ["ExactCommit", "CustomPath", "Bundled"] {
        assert!(
            SOURCE.contains(required),
            "required source selection `{required}` was removed"
        );
    }
    assert!(
        RUNTIME.contains("cache.load(&cache_key(commit))"),
        "accepted immutable cache bytes must be reused without a time-based expiry"
    );
    assert!(
        CACHE.contains("sha256_hex(&zip)") && CACHE.contains("CacheError::Mutation"),
        "cached ZIP reuse must remain mutation checked"
    );
    assert!(
        CHECK_CONTEXT.contains("config.typeshed_path.is_some()"),
        "custom Typeshed canonicality must remain represented in checker context"
    );
}
