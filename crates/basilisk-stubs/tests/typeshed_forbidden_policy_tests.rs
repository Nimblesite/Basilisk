//! Aggregate regression guard for [TYPESHEDRT-ACCEPTANCE-GATES] forbidden policy.
#![allow(missing_docs)]

const CACHE: &str = include_str!("../src/typeshed/cache.rs");
const RUNTIME: &str = include_str!("../src/typeshed/runtime.rs");
const SOURCE: &str = include_str!("../src/typeshed/source.rs");
const TRANSPORT: &str = include_str!("../src/typeshed/transport.rs");
const STUBS_LIB: &str = include_str!("../src/lib.rs");
const CHECK_CONTEXT: &str = include_str!("../../basilisk-checker/src/context.rs");
const CONSTRUCTOR_HELPERS: &str =
    include_str!("../../basilisk-checker/src/rules/constructors_call_init/helpers.rs");

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
    assert!(!std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("build.rs")
        .exists());
    for forbidden in ["STDLIB_MODULES", "STUB_DISTRIBUTIONS", "lookup_builtin"] {
        assert!(
            !STUBS_LIB.contains(forbidden),
            "compiled legacy Typeshed token `{forbidden}` was restored"
        );
    }
    assert!(
        !CONSTRUCTOR_HELPERS.contains("BUILTINS_WITH_INIT"),
        "the obsolete constructor hand table was restored"
    );
    assert!(
        CHECK_CONTEXT.contains("target_version: config")
            && CHECK_CONTEXT.contains("target_platform: config")
            && CHECK_CONTEXT.contains(".python_platform")
            && CHECK_CONTEXT.contains("filter(|platform| !platform.eq_ignore_ascii_case(\"all\"))"),
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
        RUNTIME.contains("cache.load_fresh(&cache_key(commit), unix_seconds_now())"),
        "downloaded cached bytes must use the freshness gate"
    );
    assert!(
        CACHE.contains("CACHE_MAX_AGE_SECONDS")
            && CACHE.contains("acquired_at_unix_seconds")
            && CACHE.contains("sha256_hex(&zip)")
            && CACHE.contains("CacheError::Mutation"),
        "cached ZIP reuse must remain 24-hour limited and mutation checked"
    );
    assert!(
        !CHECK_CONTEXT.contains("typeshed_path"),
        "the checker must consume the selected snapshot, not run a legacy path lookup"
    );
}
