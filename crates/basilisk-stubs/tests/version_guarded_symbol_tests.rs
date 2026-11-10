//! Pins for version-guarded stub symbols surviving extraction.
//!
//! `typing.TypeIs` ([PEP 742](https://peps.python.org/pep-0742/)) is declared
//! in typeshed as `TypeIs: _SpecialForm` inside `if sys.version_info >=
//! (3, 13):`. A parse with **no version target** keeps every feasible branch,
//! and a parse targeting 3.13 selects that branch — in both cases the symbol
//! must come out of extraction, or every `typing.TypeIs` use in user code is
//! flagged as a missing module attribute (`imports_module_attribute`), which
//! poisons unrelated diagnostics on lawful code.
#![allow(
    clippy::allow_attributes,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    missing_docs
)]

use std::path::Path;

use basilisk_stubs::types::{StubTarget, StubTargetPlatform};
use basilisk_stubs::typeshed::bundle::bundled_snapshot;
use basilisk_stubs::{StubSource, StubTier};

fn bundled_typing_source() -> (String, String) {
    let snapshot = bundled_snapshot().expect("bundled snapshot must load");
    let (uri, body) = snapshot
        .read_stub("typing")
        .expect("bundled typeshed must contain typing.pyi");
    (uri, body.to_owned())
}

#[test]
fn untargeted_parse_keeps_version_guarded_special_forms() {
    let (uri, body) = bundled_typing_source();
    let stub = basilisk_stubs::parse_pyi_source(
        &body,
        Path::new(&uri),
        "typing",
        StubSource::Typeshed,
        StubTier::Tier1,
    )
    .expect("typing.pyi must parse");
    for symbol in ["TypeIs", "ReadOnly", "NoDefault"] {
        assert!(
            stub.variables.contains_key(symbol),
            "`typing.{symbol}` is declared under `if sys.version_info >= (3, 13):` and an \
             untargeted parse keeps all feasible branches — it must be extracted"
        );
    }
    assert!(
        stub.functions.contains_key("is_protocol"),
        "`typing.is_protocol` is a guarded function declaration and must be extracted"
    );
}

#[test]
fn py313_targeted_parse_keeps_version_guarded_special_forms() {
    let (uri, body) = bundled_typing_source();
    let stub = basilisk_stubs::pyi_parser::parse_pyi_source_for_target(
        &body,
        Path::new(&uri),
        "typing",
        StubSource::Typeshed,
        StubTier::Tier1,
        &StubTarget {
            python_version: (3, 13),
            platform: StubTargetPlatform::All,
        },
    )
    .expect("typing.pyi must parse");
    assert!(
        stub.variables.contains_key("TypeIs"),
        "targeting 3.13 selects the `sys.version_info >= (3, 13)` branch, so `TypeIs` \
         must be extracted"
    );
}
