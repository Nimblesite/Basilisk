//! Implements [STUBRES-ENGINE]. See docs/specs/CHECKER-STUB-RESOLUTION-SPEC.md#STUBRES-ENGINE
//! Standard-library Typeshed resolution for Basilisk. The active runtime
//! snapshot—custom, downloaded, or bundled—is the sole source of step-3 facts.
//!
//! Provides:
//! - Stub resolution data model ([`StubResolution`], [`StubSource`], [`StubTier`])

pub mod generate;
pub mod pyi_parser;
pub mod reexports;
pub mod types;
pub mod typeshed;

pub use pyi_parser::{parse_pyi_file, parse_pyi_source, StubParseError};
pub use reexports::reexported_member_names;
pub use types::{
    render_stub_signature, user_stub_tier, StarReexport, StubClass, StubFunction, StubModule,
    StubParam, StubParamKind, StubResolution, StubSource, StubSpan, StubTier, StubVariable,
    TypeProvenance, GENERATED_STUB_HEADER_PREFIX,
};

/// Whether the package containing `resolved_path` ships a PEP 561 `py.typed`
/// marker (i.e. opts in to inline type distribution).
///
/// Walks up from the resolved file looking for a `py.typed` file, stopping at
/// the `site-packages` boundary — installed packages are its direct children,
/// so the marker never lives at or above that level. Implements [STUBRES-ENGINE].
// Implements [STUBRES-PEP561] step 5 (inline-typed packages) — detects the
// PEP 561 `py.typed` opt-in marker that distinguishes an inline-typed package
// from an untyped one.
#[must_use]
pub fn has_py_typed_marker(resolved_path: &std::path::Path) -> bool {
    let mut dir = resolved_path.parent();
    while let Some(current) = dir {
        if current.file_name() == Some(std::ffi::OsStr::new("site-packages")) {
            return false;
        }
        if current.join("py.typed").is_file() {
            return true;
        }
        dir = current.parent();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled_has_module(module: &str) -> bool {
        typeshed::bundle::bundled_snapshot()
            .is_ok_and(|snapshot| snapshot.read_stub(module).is_some())
    }

    fn bundled_builtins_source() -> String {
        typeshed::bundle::bundled_snapshot()
            .ok()
            .and_then(|snapshot| {
                snapshot
                    .read_stub("builtins")
                    .map(|(_, source)| source.to_owned())
            })
            .unwrap_or_default()
    }

    #[test]
    fn stdlib_known_modules_detected() {
        for module in [
            "os",
            "sys",
            "typing",
            "collections",
            "asyncio",
            "json",
            "pathlib",
            "dataclasses",
            "functools",
            "itertools",
            "__future__",
            "tomllib",
            "zoneinfo",
            "graphlib",
        ] {
            assert!(
                bundled_has_module(module),
                "missing bundled module {module}"
            );
        }
    }

    #[test]
    fn stdlib_dotted_names_resolved() {
        for module in [
            "collections.abc",
            "http.server",
            "email.mime.text",
            "concurrent.futures",
        ] {
            assert!(
                bundled_has_module(module),
                "missing bundled module {module}"
            );
        }
    }

    #[test]
    fn third_party_not_in_stdlib() {
        for module in [
            "requests", "flask", "django", "numpy", "pandas", "fastmcp", "pydantic",
        ] {
            assert!(!bundled_has_module(module));
        }
    }

    #[test]
    fn empty_module_not_stdlib() {
        assert!(!bundled_has_module(""));
    }

    #[test]
    fn builtin_lookup_works() {
        let source = bundled_builtins_source();
        assert!(source.contains("class int"));
        assert!(source.contains("class str"));
        assert!(source.contains("None"));
        assert!(!source.contains("definitely_not_a_real_builtin"));
    }

    #[test]
    fn stub_types_constructible() {
        let resolution = StubResolution {
            module: "os".to_owned(),
            source: StubSource::Typeshed,
            pyi_path: None,
            tier: StubTier::Tier1,
        };
        assert_eq!(resolution.source, StubSource::Typeshed);
        assert_eq!(resolution.tier, StubTier::Tier1);
    }

    #[test]
    fn stub_tier_ordering() {
        assert!(StubTier::Tier1 < StubTier::Tier2);
        assert!(StubTier::Tier2 < StubTier::Tier3);
    }

    #[test]
    fn internal_modules_recognized() {
        assert!(bundled_has_module("_thread"));
        assert!(bundled_has_module("_io"));
        assert!(bundled_has_module("_collections_abc"));
    }

    #[test]
    fn basilisk_is_not_manufactured_as_a_typeshed_module() {
        assert!(!bundled_has_module("basilisk"));
        assert!(!bundled_has_module("basilisk.types"));
    }

    #[test]
    fn type_provenance_from_stub_source_and_tier() {
        use types::{StubSource, StubTier, TypeProvenance};

        assert_eq!(
            TypeProvenance::from((&StubSource::Typeshed, &StubTier::Tier1)),
            TypeProvenance::StubTier1
        );
        assert_eq!(
            TypeProvenance::from((&StubSource::StubPackage, &StubTier::Tier1)),
            TypeProvenance::StubTier1
        );
        assert_eq!(
            TypeProvenance::from((&StubSource::UserStub, &StubTier::Tier1)).hover_label(),
            None,
            "a user-authored step-1 stub must never be labelled typeshed"
        );
        assert_eq!(
            TypeProvenance::from((&StubSource::UserStub, &StubTier::Tier1)),
            TypeProvenance::StubUser
        );
        assert_eq!(
            TypeProvenance::from((&StubSource::UserStub, &StubTier::Tier2)),
            TypeProvenance::StubTier2
        );
        assert_eq!(
            TypeProvenance::from((&StubSource::InlineTyped, &StubTier::Tier3)),
            TypeProvenance::StubTier3
        );
        // A custom typeshed (`typeshed-path`) keeps Tier-1 trust but its OWN
        // provenance so hover can distinguish it from the default typeshed
        // (downloaded archive or bundled snapshot)
        // ([STUBRES-CUSTOM-TYPESHED]). Only
        // the `(CustomTypeshed,
        // Tier1)` pair special-cases — every other tier for the same source falls
        // through to the generic tier mapping, never to `StubCustomTypeshed`.
        assert_eq!(
            TypeProvenance::from((&StubSource::CustomTypeshed, &StubTier::Tier1)),
            TypeProvenance::StubCustomTypeshed,
            "a custom-typeshed Tier-1 stub must map to StubCustomTypeshed"
        );
        assert_eq!(
            TypeProvenance::from((&StubSource::CustomTypeshed, &StubTier::Tier2)),
            TypeProvenance::StubTier2,
            "only Tier1 special-cases custom typeshed; Tier2 stays StubTier2"
        );
        assert_eq!(
            TypeProvenance::from((&StubSource::CustomTypeshed, &StubTier::Tier3)),
            TypeProvenance::StubTier3,
            "only Tier1 special-cases custom typeshed; Tier3 stays StubTier3"
        );
    }

    #[test]
    fn type_provenance_hover_labels() {
        use types::TypeProvenance;

        // Exact labels — every provenance renders a distinct suffix, so a mutant
        // that swaps a match arm or edits a string is caught. `(custom typeshed)`
        // is deliberately distinct from `(typeshed)` so a MicroPython signature is
        // never misreported as the bundled CPython one ([STUBRES-CUSTOM-TYPESHED]).
        assert_eq!(TypeProvenance::Source.hover_label(), None);
        assert_eq!(TypeProvenance::StubTier1.hover_label(), Some("(typeshed)"));
        assert_eq!(TypeProvenance::StubUser.hover_label(), None);
        assert_eq!(
            TypeProvenance::StubCustomTypeshed.hover_label(),
            Some("(custom typeshed)")
        );
        assert_eq!(
            TypeProvenance::StubTier2.hover_label(),
            Some("(community stub)")
        );
        assert_eq!(
            TypeProvenance::StubTier3.hover_label(),
            Some("(best-effort stub, may be inaccurate)")
        );
        assert_eq!(
            TypeProvenance::Untyped.hover_label(),
            Some("(no type stubs available)")
        );
        // `(custom typeshed)` must NOT be confused with the bundled `(typeshed)`:
        // the two labels are different strings even though one contains the other.
        assert_ne!(
            TypeProvenance::StubCustomTypeshed.hover_label(),
            TypeProvenance::StubTier1.hover_label()
        );
    }
}
