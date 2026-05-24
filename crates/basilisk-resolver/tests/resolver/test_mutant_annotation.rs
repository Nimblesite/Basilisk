//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_mutant_annotation`.

use super::common::resolve_src;
use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

#[test]
fn annotation_flags_none_is_not_any() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ReturnAnnotationKind;
    let src = "def f() -> None: pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        matches!(func.return_annotation, ReturnAnnotationKind::NoneType),
        "None annotation must be NoneType, not Any — got {:?}",
        func.return_annotation
    );
    Ok(())
}

#[test]
fn annotation_flags_any_is_any() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ReturnAnnotationKind;
    let src = "from typing import Any\ndef f() -> Any: pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = resolved
        .functions
        .first()
        .expect("expected at least one function");
    assert!(
        matches!(func.return_annotation, ReturnAnnotationKind::Any),
        "Any annotation must be ReturnAnnotationKind::Any — got {:?}",
        func.return_annotation
    );
    Ok(())
}

#[test]
fn annotation_flags_none_name_is_none_not_other() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ReturnAnnotationKind;
    // "None" → NoneType
    let src_none = "def f() -> None: pass\n".to_owned();
    let parsed_none = parse_source(src_none, "test.py".to_owned())?;
    let resolved_none = resolve(&parsed_none)?;
    assert!(
        matches!(
            resolved_none
                .functions
                .first()
                .expect("expected at least one function")
                .return_annotation,
            ReturnAnnotationKind::NoneType
        ),
        "-> None must be NoneType"
    );
    // "int" → Other (not NoneType, not Any)
    let src_int = "def g() -> int: pass\n".to_owned();
    let parsed_int = parse_source(src_int, "test.py".to_owned())?;
    let resolved_int = resolve(&parsed_int)?;
    assert!(
        matches!(
            resolved_int
                .functions
                .first()
                .expect("expected at least one function")
                .return_annotation,
            ReturnAnnotationKind::Other
        ),
        "-> int must be Other, not NoneType"
    );
    Ok(())
}
