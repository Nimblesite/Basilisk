//! Tests for resolver: test_annotations.

mod common;

use common::resolve_src;

#[test]
fn annotation_attribute_detected_as_any() -> Result<(), Box<dyn std::error::Error>> {
    let src = "import typing\ndef foo(x: typing.Any) -> typing.Any:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.functions.len(), 1);
    let func = &resolved.functions[0];
    assert!(
        func.parameters[0].annotation_is_any,
        "typing.Any parameter annotation must be detected as Any"
    );
    Ok(())
}

#[test]
fn numeric_return_annotation_detected() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ReturnAnnotationKind;
    let src = "def foo() -> 1:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(
        resolved.functions[0].return_annotation,
        ReturnAnnotationKind::NumericLiteral
    );
    Ok(())
}

#[test]
fn boolean_return_annotation_detected_as_numeric_literal() -> Result<(), Box<dyn std::error::Error>>
{
    use basilisk_resolver::ReturnAnnotationKind;
    let src = "def foo() -> True:\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(
        resolved.functions[0].return_annotation,
        ReturnAnnotationKind::NumericLiteral
    );
    Ok(())
}
