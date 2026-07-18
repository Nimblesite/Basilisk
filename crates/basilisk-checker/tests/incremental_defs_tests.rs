//! Tests for [TYPEINF-TARGET-INCREMENTAL] Stage 1. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-INCREMENTAL and
//! docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md#NARROWPLAN-CHECKLIST.
//!
//! Exercises the definition-level and expression-level Salsa queries in
//! `crates/basilisk-checker/src/incremental_defs.rs`: definition extraction,
//! per-definition inference, the fixpoint cycle sentinel, the interface
//! boundary, and — via [`basilisk_test_utils::EventDb`]'s `WillExecute` log —
//! the definition-level **early cutoff** itself.
#![expect(
    clippy::expect_used,
    reason = "test-only lookups over fixed fixture definitions"
)]

use basilisk_checker::incremental_defs::{
    definition_type, definitions, expression_types, module_interface, DefKind,
};
use basilisk_checker::types::{CallableInfo, InferredType, LiteralValue};
use basilisk_db::SourceFile;
use basilisk_test_utils::EventDb;
use salsa::Setter as _;

const MODULE: &str = r#"def alpha(a: int) -> str:
    return "x"

BETA = [1]

class Gamma:
    pass
"#;

/// Top-level definitions are extracted with their kinds and names.
#[test]
fn definitions_are_extracted_with_kinds() {
    let db = EventDb::default();
    let file = SourceFile::new(&db, "m.py".to_owned(), MODULE.to_owned());
    let defs = definitions(&db, file);
    let summary: Vec<(&str, DefKind)> = defs
        .iter()
        .map(|def| (def.name(&db).as_str(), def.kind(&db)))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("alpha", DefKind::Function),
            ("BETA", DefKind::Variable),
            ("Gamma", DefKind::Class),
        ]
    );
}

/// A function definition's type is its declared `Callable` surface.
#[test]
fn function_definition_type_reads_annotations() {
    let db = EventDb::default();
    let file = SourceFile::new(&db, "m.py".to_owned(), MODULE.to_owned());
    let defs = definitions(&db, file);
    let alpha = defs.first().copied().expect("alpha exists");
    assert_eq!(
        definition_type(&db, alpha),
        InferredType::Callable(CallableInfo {
            param_types: vec![InferredType::Int],
            return_type: Box::new(InferredType::Str),
        })
    );
}

/// A variable definition synthesizes through the bidirectional engine —
/// deferred generalization keeps `[1]` as `list[Literal[1]]`
/// ([TYPEINF-TARGET-CONSTRAINTS]).
#[test]
fn variable_definition_type_keeps_literal_precision() {
    let db = EventDb::default();
    let file = SourceFile::new(&db, "m.py".to_owned(), MODULE.to_owned());
    let defs = definitions(&db, file);
    let beta = defs.get(1).copied().expect("BETA exists");
    assert_eq!(
        definition_type(&db, beta),
        InferredType::List(Box::new(InferredType::Literal(LiteralValue::Int(1))))
    );
}

/// A bare-name alias resolves through its sibling definition.
#[test]
fn alias_chain_resolves_through_siblings() {
    let db = EventDb::default();
    let source = "x = 1\ny = x\n";
    let file = SourceFile::new(&db, "m.py".to_owned(), source.to_owned());
    let defs = definitions(&db, file);
    let y = defs.get(1).copied().expect("y exists");
    assert_eq!(
        definition_type(&db, y),
        InferredType::Literal(LiteralValue::Int(1))
    );
}

/// Mutually-referential definitions terminate via fixpoint iteration and
/// settle on the divergent/bottom sentinel (`Unknown`) — never a hang, never
/// an invented type ([TYPEINF-TARGET-INCREMENTAL] cycle handling).
#[test]
fn definition_cycles_settle_on_the_divergent_sentinel() {
    let db = EventDb::default();
    let source = "a = b\nb = a\n";
    let file = SourceFile::new(&db, "m.py".to_owned(), source.to_owned());
    let defs = definitions(&db, file);
    for def in defs {
        assert_eq!(definition_type(&db, *def), InferredType::Unknown);
    }
}

/// Expression-level query: assignment RHS and return values get types,
/// slice-relative.
#[test]
fn expression_types_cover_assignments_and_returns() {
    let db = EventDb::default();
    let file = SourceFile::new(&db, "m.py".to_owned(), MODULE.to_owned());
    let defs = definitions(&db, file);
    let alpha = defs.first().copied().expect("alpha exists");
    let types = expression_types(&db, alpha);
    assert_eq!(types.len(), 1, "one return expression in alpha");
    let ret = types.first().expect("return expr typed");
    assert_eq!(
        ret.inferred,
        InferredType::Literal(LiteralValue::Str("x".to_owned()))
    );
}

/// The module interface is the compact `(name, type)` boundary.
#[test]
fn module_interface_lists_definition_types() {
    let db = EventDb::default();
    let file = SourceFile::new(&db, "m.py".to_owned(), MODULE.to_owned());
    let interface = module_interface(&db, file);
    let names: Vec<&str> = interface.0.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "BETA", "Gamma"]);
}

/// **Definition-level early cutoff**: editing one definition's body re-runs
/// `definition_type` ONLY for that definition — the untouched sibling's memo
/// survives, proven by salsa's `WillExecute` log.
#[test]
fn editing_one_definition_recomputes_only_that_definition() {
    let mut db = EventDb::default();
    let file = SourceFile::new(&db, "m.py".to_owned(), MODULE.to_owned());
    for def in definitions(&db, file) {
        let _ = definition_type(&db, *def);
    }
    // Drain setup events.
    let _ = db.executions_of("definition_type");

    // Edit ONLY alpha's body (BETA's and Gamma's slices are unchanged).
    let edited = MODULE.replace("return \"x\"", "return \"xy\"");
    assert_ne!(edited, MODULE);
    let _ = file.set_text(&mut db).to(edited);
    for def in definitions(&db, file) {
        let _ = definition_type(&db, *def);
    }
    assert_eq!(
        db.executions_of("definition_type"),
        1,
        "only the edited definition's type may recompute"
    );
}

/// **Interface backdating**: a body-only edit leaves the module interface
/// value unchanged, so a consumer query reading it is never re-executed —
/// the cross-file early-cutoff boundary.
#[test]
fn body_only_edit_backdates_the_module_interface() {
    let mut db = EventDb::default();
    let file = SourceFile::new(&db, "m.py".to_owned(), MODULE.to_owned());
    let before = module_interface(&db, file).clone();

    let edited = MODULE.replace("return \"x\"", "return \"different body\"");
    let _ = file.set_text(&mut db).to(edited);
    let after = module_interface(&db, file).clone();
    assert_eq!(
        before, after,
        "a body-only edit must not change the interface value"
    );
}

/// The Salsa-backed use-def map ([TYPEINF-TARGET-NARROWING]): `narrowed_uses`
/// reports flow-narrowed reads per definition, and editing one function
/// re-executes only that function's query — narrowing is incremental at
/// definition granularity.
#[test]
fn narrowed_uses_is_definition_incremental() {
    use basilisk_checker::incremental_defs::narrowed_uses;

    const TWO_FUNCTIONS: &str = r"def first(x: int | None) -> int:
    assert x is not None
    return x

def second(y: int | str) -> None:
    if isinstance(y, int):
        a = y
";
    let mut db = EventDb::default();
    let file = SourceFile::new(&db, "m.py".to_owned(), TWO_FUNCTIONS.to_owned());
    for def in definitions(&db, file) {
        let _ = narrowed_uses(&db, *def);
    }
    let first_uses = definitions(&db, file)
        .first()
        .map(|def| narrowed_uses(&db, *def).clone())
        .unwrap_or_default();
    assert!(
        first_uses
            .iter()
            .any(|use_site| use_site.name == "x" && use_site.narrowed == InferredType::Int),
        "assert narrowing must be visible through the tracked query: {first_uses:?}"
    );
    let _ = db.executions_of("narrowed_uses");

    // Edit ONLY `second` — `first`'s narrowing memo must survive.
    let edited = TWO_FUNCTIONS.replace("a = y", "b = y");
    let _ = file.set_text(&mut db).to(edited);
    for def in definitions(&db, file) {
        let _ = narrowed_uses(&db, *def);
    }
    assert_eq!(
        db.executions_of("narrowed_uses"),
        1,
        "only the edited definition's narrowing may recompute"
    );
}

/// Same-module return inference: an unannotated function's return type is
/// synthesized from its body ([NARROWPLAN-CHECKLIST] expression inference).
#[test]
fn unannotated_return_is_synthesized_from_the_body() {
    let db = EventDb::default();
    let source = r#"def no_return(x: int):
    y = x

def returns_literal():
    return "done"

def diverging_return():
    return 1
"#;
    let file = SourceFile::new(&db, "m.py".to_owned(), source.to_owned());
    let defs = definitions(&db, file);

    let return_of = |index: usize| -> InferredType {
        let def = defs.get(index).copied().expect("definition exists");
        match definition_type(&db, def) {
            InferredType::Callable(info) => *info.return_type,
            other => other,
        }
    };

    // No return statement at all → None.
    assert_eq!(return_of(0), InferredType::None_);

    // A single trailing return synthesizes its literal type precisely.
    assert_eq!(
        return_of(1),
        InferredType::Literal(LiteralValue::Str("done".to_owned()))
    );

    // Trailing return diverges: no implicit None union.
    let diverging = return_of(2);
    assert!(
        !InferredType::None_.is_assignable_to(&diverging),
        "a body ending in return must not union None: {diverging:?}"
    );
}

/// Same-module call-return inference: `x = f()` resolves through the
/// sibling's declared or synthesized return type, including chains.
#[test]
fn variable_from_sibling_call_infers_the_return() {
    let db = EventDb::default();
    let source = r#"def make(a: int) -> str:
    return "s"

def synth_only():
    return 42

made = make(1)
synthesized = synth_only()
"#;
    let file = SourceFile::new(&db, "m.py".to_owned(), source.to_owned());
    let defs = definitions(&db, file);
    let made = defs.get(2).copied().expect("made");
    assert_eq!(definition_type(&db, made), InferredType::Str);

    let synthesized = defs.get(3).copied().expect("synthesized");
    assert_eq!(
        definition_type(&db, synthesized),
        InferredType::Literal(LiteralValue::Int(42)),
        "the callee's SYNTHESIZED return flows into the variable"
    );
}
