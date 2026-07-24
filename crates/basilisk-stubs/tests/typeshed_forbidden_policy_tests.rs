//! Aggregate regression guard for [TYPESHEDRT-ACCEPTANCE-GATES] forbidden policy.
#![allow(missing_docs)]

const RUNTIME: &str = include_str!("../src/typeshed/runtime.rs");
const SOURCE: &str = include_str!("../src/typeshed/source.rs");
const STORE: &str = include_str!("../src/typeshed/store.rs");
const SELECTOR: &str = include_str!("../src/typeshed/selector.rs");
const STUBS_LIB: &str = include_str!("../src/lib.rs");
const STUBS_MANIFEST: &str = include_str!("../Cargo.toml");
const CHECK_CONTEXT: &str = include_str!("../../basilisk-checker/src/context.rs");
const CONSTRUCTOR_HELPERS: &str =
    include_str!("../../basilisk-checker/src/rules/constructors_call_init/helpers.rs");

#[test]
fn production_sources_reject_forbidden_typeshed_policy() {
    let resolution_sources = [
        ("runtime", RUNTIME),
        ("source", SOURCE),
        ("store", STORE),
        ("selector", SELECTOR),
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

    for (source_name, source) in resolution_sources {
        for token in forbidden {
            assert!(
                !source.contains(token),
                "forbidden Typeshed policy token `{token}` appeared in {source_name}"
            );
        }
    }
    // The checker links this crate, and "the checker never downloads" is a
    // property of the build ([TYPESHEDRT-SEGREGATION]): no HTTP client may
    // ever appear in this crate's manifest — downloading lives in
    // `basilisk-typeshed-fetch`.
    for client in ["ureq", "reqwest", "hyper"] {
        assert!(
            !STUBS_MANIFEST.contains(client),
            "HTTP client `{client}` must never appear in basilisk-stubs"
        );
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
fn immutable_pins_store_and_custom_paths_remain_supported() {
    for required in ["ExactCommit", "Custom", "Bundled"] {
        assert!(
            SOURCE.contains(required),
            "required source kind `{required}` was removed"
        );
    }
    // Resolution is offline by construction ([STUBRES-TYPESHED-OFFLINE]): the
    // moving-main freshness window died with in-checker downloads, so nothing
    // in the resolution path may resurrect a TTL or wall-clock gate.
    for (source_name, source) in [
        ("runtime", RUNTIME),
        ("store", STORE),
        ("selector", SELECTOR),
    ] {
        for token in ["CACHE_MAX_AGE", "load_fresh", "unix_seconds_now"] {
            assert!(
                !source.contains(token),
                "TTL machinery `{token}` reappeared in {source_name}"
            );
        }
    }
    assert!(
        RUNTIME.contains("load_pinned"),
        "an explicitly pinned commit must resolve from the local store"
    );
    assert!(
        !CHECK_CONTEXT.contains("typeshed_path"),
        "the checker must consume the selected snapshot, not run a legacy path lookup"
    );
}
