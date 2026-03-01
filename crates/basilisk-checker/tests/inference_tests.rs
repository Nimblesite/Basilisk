//! End-to-end tests for Basilisk's type inference engine.

use basilisk_checker::inference::{infer_flow_union_types, infer_rhs, infer_variable_type};
use basilisk_checker::types::{InferredType, LiteralValue};
use basilisk_resolver::{RhsKind, Span, VariableInfo};

#[test]
fn test_infer_rhs_literals() {
    assert_eq!(infer_rhs(&RhsKind::IntLiteral), InferredType::Int);
    assert_eq!(infer_rhs(&RhsKind::FloatLiteral), InferredType::Float);
    assert_eq!(infer_rhs(&RhsKind::StrLiteral), InferredType::Str);
    assert_eq!(infer_rhs(&RhsKind::BoolLiteral), InferredType::Bool);
    assert_eq!(infer_rhs(&RhsKind::BytesLiteral), InferredType::Bytes);
    assert_eq!(infer_rhs(&RhsKind::NoneValue), InferredType::None_);
    assert_eq!(
        infer_rhs(&RhsKind::EmptyList),
        InferredType::List(Box::new(InferredType::Never))
    );
    assert_eq!(
        infer_rhs(&RhsKind::EmptyDict),
        InferredType::Dict(Box::new(InferredType::Never), Box::new(InferredType::Never))
    );
}

#[test]
fn test_infer_variable_type_basic() {
    let var_info = VariableInfo {
        name: "x".to_string(),
        name_span: Span { start: 0, end: 1 },
        has_annotation: false,
        rhs_kind: RhsKind::IntLiteral,
        annotation_span: None,
        rhs_span: Some(Span { start: 4, end: 6 }),
    };

    assert_eq!(infer_variable_type(&var_info), InferredType::Int);
}

#[test]
fn test_flow_union_inference_single_type() {
    let assignments = vec![("x".to_string(), InferredType::Int)];

    let result = infer_flow_union_types(&assignments);
    assert_eq!(result.get("x"), Some(&InferredType::Int));
}

#[test]
fn test_flow_union_inference_nested_unions() {
    let assignments = vec![
        ("x".to_string(), InferredType::Int),
        (
            "x".to_string(),
            InferredType::Union(vec![InferredType::Str, InferredType::Float]),
        ),
        ("x".to_string(), InferredType::Bool),
    ];

    let result = infer_flow_union_types(&assignments);

    // Should flatten nested unions
    assert!(
        matches!(result.get("x"), Some(InferredType::Union(types)) if types.len() == 4),
        "x should be a flat Union of 4 types"
    );
}

#[test]
fn test_flow_union_inference_empty() {
    let assignments: Vec<(String, InferredType)> = vec![];
    let result = infer_flow_union_types(&assignments);
    assert!(result.is_empty());
}

#[test]
fn test_flow_union_inference_duplicate_types() {
    let assignments = vec![
        ("x".to_string(), InferredType::Int),
        ("x".to_string(), InferredType::Int), // Duplicate
        ("x".to_string(), InferredType::Str),
    ];

    let result = infer_flow_union_types(&assignments);

    // Fixed: infer_flow_union_types now deduplicates — Union contains only 2 unique entries (Int, Str)
    assert!(
        matches!(result.get("x"), Some(InferredType::Union(types)) if types.len() == 2),
        "x should be Union of 2 unique types (duplicates removed)"
    );
}

#[test]
fn test_flow_union_inference_complex_types() {
    let assignments = vec![
        (
            "x".to_string(),
            InferredType::List(Box::new(InferredType::Int)),
        ),
        (
            "x".to_string(),
            InferredType::List(Box::new(InferredType::Str)),
        ),
        (
            "y".to_string(),
            InferredType::Dict(Box::new(InferredType::Str), Box::new(InferredType::Int)),
        ),
    ];

    let result = infer_flow_union_types(&assignments);

    assert!(
        matches!(result.get("x"), Some(InferredType::Union(types)) if types.len() == 2),
        "x should be Union of 2 list types"
    );
    assert!(
        matches!(result.get("y"), Some(InferredType::Dict(_, _))),
        "y should be Dict"
    );
}

#[test]
fn test_flow_union_inference_optional_types() {
    let assignments = vec![
        ("x".to_string(), InferredType::Int),
        (
            "x".to_string(),
            InferredType::Optional(Box::new(InferredType::Str)),
        ),
        ("x".to_string(), InferredType::None_),
    ];

    let result = infer_flow_union_types(&assignments);

    // Should handle optional types correctly
    assert!(
        matches!(result.get("x"), Some(InferredType::Union(types)) if types.len() == 3),
        "x should be Union of 3 types"
    );
}

#[test]
fn test_flow_union_inference_literal_types() {
    let assignments = vec![
        (
            "x".to_string(),
            InferredType::Literal(LiteralValue::Int(42)),
        ),
        (
            "x".to_string(),
            InferredType::Literal(LiteralValue::Str("hello".to_string())),
        ),
        ("x".to_string(), InferredType::Int), // Base type
    ];

    let result = infer_flow_union_types(&assignments);

    // Should include both literals and base types
    assert!(
        matches!(result.get("x"), Some(InferredType::Union(types)) if types.len() == 3),
        "x should be Union of 3 types including literals"
    );
}

#[test]
fn test_flow_union_inference_any_and_never() {
    let assignments = vec![
        ("x".to_string(), InferredType::Any),
        ("x".to_string(), InferredType::Never),
        ("x".to_string(), InferredType::Int),
    ];

    let result = infer_flow_union_types(&assignments);

    // Any should dominate, Never should be included but doesn't affect assignability
    assert!(
        matches!(result.get("x"), Some(InferredType::Union(types)) if types.len() == 3),
        "x should be Union of 3 types"
    );
}

#[test]
fn test_flow_union_inference_multiple_variables() {
    let assignments = vec![
        ("x".to_string(), InferredType::Int),
        ("y".to_string(), InferredType::Str),
        ("x".to_string(), InferredType::Float),
        ("z".to_string(), InferredType::Bool),
        ("y".to_string(), InferredType::Int),
    ];

    let result = infer_flow_union_types(&assignments);

    assert_eq!(result.len(), 3);
    assert!(result.contains_key("x"));
    assert!(result.contains_key("y"));
    assert!(result.contains_key("z"));

    assert!(
        matches!(result.get("x"), Some(InferredType::Union(types)) if types.len() == 2),
        "x should be Union of 2 types"
    );
    assert!(
        matches!(result.get("y"), Some(InferredType::Union(types)) if types.len() == 2),
        "y should be Union of 2 types"
    );
    assert_eq!(result.get("z"), Some(&InferredType::Bool));
}

// ---------------------------------------------------------------------------
// E2E Tests using real Python code through the full pipeline
// ---------------------------------------------------------------------------

use basilisk_checker::check;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

fn run_e2e(src: &str) -> Result<Vec<basilisk_checker::Diagnostic>, Box<dyn std::error::Error>> {
    let parsed = parse_source(src.to_owned(), "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    Ok(check(&resolved))
}

#[test]
fn test_e0014_int_assigned_to_str() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_e2e("x: str = 42\n")?;
    let e0014: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")?;
    assert!(!e0014.is_empty(), "int assigned to str should fire E0014");
    Ok(())
}

#[test]
fn test_e0014_no_error_compatible() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_e2e("x: float = 42\n")?;
    let e0014: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0014")
        .collect();
    assert!(e0014.is_empty(), "int assigned to float should be clean");
    Ok(())
}

#[test]
fn test_flow_union_if_else() -> Result<(), Box<dyn std::error::Error>> {
    let src = r#"
def f(cond: bool) -> None:
    if cond:
        x = 1
    else:
        x = "hi"
"#;
    let diags = run_e2e(src)?;
    // This should not produce type errors since union types are allowed
    // The test verifies the inference system handles flow union correctly
    if !diags.is_empty() {
        println!("Unexpected diagnostics:");
        for diag in &diags {
            println!("  {}: {}", diag.code.code, diag.message);
        }
    }
    assert!(diags.is_empty(), "flow union if-else should be clean");
    Ok(())
}

#[test]
fn test_flow_union_no_else() -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
def process(flag: bool):
    if flag:
        x = 42
    return x
";
    let diags = run_e2e(src)?;
    // Variable x may be unbound if flag is False
    let e0019: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0019")
        .collect();
    assert!(!e0019.is_empty(), "unbound variable should fire E0019");
    Ok(())
}

#[test]
fn test_self_no_e0001() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_e2e("def method(self): pass\n")?;
    let e0001: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    assert!(e0001.is_empty(), "self parameter should not fire E0001");
    Ok(())
}

#[test]
fn test_cls_no_e0001() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_e2e("def method(cls): pass\n")?;
    let e0001: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    assert!(e0001.is_empty(), "cls parameter should not fire E0001");
    Ok(())
}

#[test]
fn test_unannotated_param_fires_e0001() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_e2e("def process(data): pass\n")?;
    let e0001: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0001")
        .collect();
    assert!(!e0001.is_empty(), "unannotated parameter should fire E0001");
    Ok(())
}

#[test]
fn test_missing_return_fires_e0002() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_e2e("def process(data: str): pass\n")?;
    let e0002: Vec<_> = diags
        .iter()
        .filter(|d| d.code.code == "BSK-E0002")
        .collect();
    assert!(!e0002.is_empty(), "missing return annotation should fire E0002");
    Ok(())
}

#[test]
fn test_fully_annotated_clean() -> Result<(), Box<dyn std::error::Error>> {
    let diags = run_e2e("def process(data: str) -> None: pass\n")?;
    assert!(diags.is_empty(), "fully annotated function should be clean");
    Ok(())
}
