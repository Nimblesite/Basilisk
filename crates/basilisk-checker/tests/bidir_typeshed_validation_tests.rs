//! Tests for [TYPEINF-TARGET-CONSTRAINTS]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-CONSTRAINTS and
//! the Stage 0 checklist in
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CHECKLIST.
//!
//! Prototype validation of the borrowed algebraic-subtyping ideas — polar
//! type variables and constraint simplification — against **real typeshed
//! stubs** before the engine relies on them. The fixtures under
//! `tests/fixtures/typeshed/` are verbatim files from
//! `python/typeshed@main` (commit pinned in `TYPESHED_COMMIT.txt`), chosen
//! for generic/`Callable`/overload density: `functools`, `itertools`,
//! `contextlib`, `collections`, `heapq`.
//!
//! For every parameter and return annotation in those stubs the validation
//! asserts, through the public `bidir` API:
//! 1. **Reflexivity through the solver** — `T <: T` decomposes and
//!    discharges with zero errors (catches asymmetries in leaf delegation
//!    over real-world type shapes);
//! 2. **Projection stability** — lifting to the solver language and
//!    projecting back is idempotent (normalization reaches a fixpoint,
//!    no oscillation on real shapes);
//! 3. **Polar-variable resolution** — an output variable lower-bounded by
//!    the type (and an input variable upper-bounded by it) resolves to a
//!    projection compatible with the original (deferred generalization
//!    holds on real shapes, not just synthetic ones).
#![expect(
    clippy::expect_used,
    reason = "test-only parsing of pinned typeshed fixture files"
)]

use basilisk_checker::bidir::{solve, ConstraintReason, ConstraintSet, Polarity, Ty, TyVarStore};
use basilisk_checker::types::InferredType;

/// Extract every parameter/return annotation text from one stub.
fn annotation_texts(source: &str, path: &str) -> Vec<String> {
    let parsed = basilisk_parser::parse_source(source.to_owned(), path.to_owned())
        .expect("typeshed fixture must parse");
    let resolved = basilisk_resolver::resolve(&parsed).expect("typeshed fixture must resolve");
    let slice = |span: basilisk_resolver::Span| {
        let start = usize::try_from(span.start).ok()?;
        let end = usize::try_from(span.end).ok()?;
        source.get(start..end).map(str::to_owned)
    };
    let mut texts = Vec::new();
    for function in &resolved.functions {
        for parameter in &function.parameters {
            if let Some(text) = parameter.annotation_span.and_then(slice) {
                texts.push(text);
            }
        }
        if let Some(text) = function.return_annotation_span.and_then(slice) {
            texts.push(text);
        }
    }
    texts
}

/// Load all fixture stubs as `(name, source)` pairs.
fn fixture_stubs() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/typeshed");
    let entries = std::fs::read_dir(&dir).expect("typeshed fixtures directory must exist");
    let mut stubs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "pyi") {
            let source = std::fs::read_to_string(&path).expect("fixture must be readable");
            stubs.push((path.display().to_string(), source));
        }
    }
    stubs.sort();
    stubs
}

/// Reflexivity: `T <: T` must solve with zero errors for every annotation.
#[test]
fn typeshed_annotations_are_reflexive_through_the_solver() {
    let mut validated = 0_u32;
    for (name, source) in fixture_stubs() {
        for text in annotation_texts(&source, &name) {
            let lifted = Ty::from_inferred(&InferredType::from_annotation(&text));
            let mut constraints = ConstraintSet::default();
            constraints.push(
                lifted.clone(),
                lifted,
                ruff_text_size::TextRange::default(),
                ConstraintReason::ExpectedType,
            );
            let solution = solve(TyVarStore::default(), constraints.into_vec());
            assert!(
                solution.errors.is_empty(),
                "{name}: `{text}` <: `{text}` must hold, got {:?}",
                solution.errors
            );
            validated += 1;
        }
    }
    assert!(
        validated >= 300,
        "expected 300+ real annotations validated, got {validated} — fixture rot?"
    );
}

/// Projection stability: lift → project reaches a fixpoint in one step.
#[test]
fn typeshed_annotations_project_stably() {
    for (name, source) in fixture_stubs() {
        let vars = TyVarStore::default();
        for text in annotation_texts(&source, &name) {
            let once = Ty::from_inferred(&InferredType::from_annotation(&text)).to_inferred(&vars);
            let twice = Ty::from_inferred(&once).to_inferred(&vars);
            assert_eq!(
                once, twice,
                "{name}: projection of `{text}` must be idempotent"
            );
        }
    }
}

/// Polar variables: bounds from real annotations resolve compatibly.
#[test]
fn typeshed_annotations_flow_through_polar_variables() {
    for (name, source) in fixture_stubs() {
        for text in annotation_texts(&source, &name) {
            let inferred = InferredType::from_annotation(&text);
            let lifted = Ty::from_inferred(&inferred);

            // Output polarity: what flowed in is what comes out.
            let mut store = TyVarStore::default();
            let out = store.fresh(Polarity::Output);
            store.add_lower(out, lifted.clone());
            let resolved_out = store.resolve(out);
            assert!(
                resolved_out.is_assignable_to(&inferred)
                    || inferred.is_assignable_to(&resolved_out),
                "{name}: output var seeded with `{text}` resolved to incompatible {resolved_out:?}"
            );

            // Input polarity: what is demanded is what must be accepted.
            let input = store.fresh(Polarity::Input);
            store.add_upper(input, lifted);
            let resolved_in = store.resolve(input);
            assert!(
                resolved_in.is_assignable_to(&inferred) || inferred.is_assignable_to(&resolved_in),
                "{name}: input var bounded by `{text}` resolved to incompatible {resolved_in:?}"
            );
        }
    }
}

/// The fixtures really are the pinned typeshed files (guard against silent
/// truncation of the corpus).
#[test]
fn typeshed_fixture_corpus_is_present() {
    let stubs = fixture_stubs();
    assert!(
        stubs.len() >= 5,
        "expected the 5 pinned typeshed stubs, found {}",
        stubs.len()
    );
    let total_lines: usize = stubs.iter().map(|(_, s)| s.lines().count()).sum();
    assert!(
        total_lines >= 1000,
        "expected 1000+ lines of real typeshed, got {total_lines}"
    );
}
