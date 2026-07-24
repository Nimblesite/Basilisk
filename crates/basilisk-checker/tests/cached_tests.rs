//! Tests for [CHKCACHE-DIAG] / [CHKCACHE-DIAG-INTERN].
//! See docs/specs/CHECKER-CACHE-SPEC.md#CHKCACHE-ENTRY
#![allow(clippy::allow_attributes, clippy::unwrap_used, clippy::expect_used)]
//! Crate-boundary tests for the serde-friendly diagnostic projection and the
//! `&'static` code interner used on cache replay.

use basilisk_checker::cached::CachedDiagnostic;
use basilisk_checker::{Diagnostic, ErrorCode, Severity};
use basilisk_resolver::Span;
use basilisk_stubs::TypeProvenance;

fn sample(code: &'static str, with_extras: bool) -> Diagnostic {
    Diagnostic {
        code: ErrorCode {
            code,
            docs_url: "https://www.basilisk-python.dev/errors/X",
        },
        severity: Severity::Error,
        message: "boom".to_owned(),
        span: Span::new(3, 9),
        path: "m.py".to_owned(),
        help: with_extras.then(|| "try this".into()),
        note: with_extras.then(|| "PEP 484".into()),
        provenance: with_extras.then_some(TypeProvenance::StubTier1),
    }
}

#[test]
fn projection_round_trips_through_serde_preserving_every_field() {
    let original = sample("BSK-0001", true);
    let cached = CachedDiagnostic::from(&original);
    let json = serde_json::to_string(&cached).expect("serialize");
    let restored: CachedDiagnostic = serde_json::from_str(&json).expect("deserialize");
    let diagnostic = restored.into_diagnostic();

    assert_eq!(diagnostic.code.code, "BSK-0001");
    assert_eq!(diagnostic.code.docs_url, original.code.docs_url);
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.message, "boom");
    assert_eq!(diagnostic.span, Span::new(3, 9));
    assert_eq!(diagnostic.path, "m.py");
    assert_eq!(diagnostic.help.as_deref(), Some("try this"));
    assert_eq!(diagnostic.note.as_deref(), Some("PEP 484"));
    assert_eq!(diagnostic.provenance, Some(TypeProvenance::StubTier1));
}

#[test]
fn optional_fields_round_trip_as_none() {
    let cached = CachedDiagnostic::from(&sample("BSK-0002", false));
    let diagnostic = cached.into_diagnostic();
    assert_eq!(diagnostic.help, None);
    assert_eq!(diagnostic.note, None);
    assert_eq!(diagnostic.provenance, None);
}

#[test]
fn interner_reuses_static_storage_for_repeated_codes() {
    // Two diagnostics with the same code must intern to the SAME `&'static`
    // pointer — proving the interner returns the existing entry, not a fresh
    // leak per replay.
    let first = CachedDiagnostic::from(&sample("protocols_explicit_3", false)).into_diagnostic();
    let second = CachedDiagnostic::from(&sample("protocols_explicit_3", false)).into_diagnostic();
    assert_eq!(first.code.code, second.code.code);
    assert!(
        std::ptr::eq(first.code.code, second.code.code),
        "interned code must reuse the same static storage"
    );
    assert!(std::ptr::eq(first.code.docs_url, second.code.docs_url));
}
