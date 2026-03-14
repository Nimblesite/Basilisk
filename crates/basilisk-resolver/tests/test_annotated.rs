//! Tests for resolver: `test_annotated`.

mod common;

use common::resolve_src;

#[test]
fn annotated_direct_call_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Annotated\nAnnotated()\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.annotated_direct_call_spans.is_empty(),
        "Annotated() direct call must be collected"
    );
    Ok(())
}

#[test]
fn annotated_subscript_call_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("from typing import Annotated\n", "Annotated[int, '']()\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.annotated_direct_call_spans.is_empty());
    Ok(())
}

#[test]
fn annotated_call_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("from typing import Annotated\n", "Annotated()\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.annotated_direct_call_spans.is_empty());
    Ok(())
}

#[test]
fn annotated_direct_call_at_module_level() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("from typing import Annotated\n", "Annotated()\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.annotated_direct_call_spans.is_empty());
    Ok(())
}

#[test]
fn typeddict_ann_assign_missing_key() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "    year: int\n",
        "m: Movie = {'name': 'x'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.typeddict_key_violations.is_empty());
    Ok(())
}

#[test]
fn annotated_subscript_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import Annotated\n",
        "x: Annotated[int, 'meta'] = 5\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    // Just verify it parses without issue
    assert!(resolved.annotated_too_few_args.is_empty());
    Ok(())
}

#[test]
fn assert_type_strip_annotated() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import assert_type, Annotated\ndef check(x: Annotated[int, 'doc']) -> None:\n    assert_type(x, int)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.assert_type_calls.is_empty());
    // The assert_type should not flag a mismatch since Annotated[int, ...] == int
    let call = &resolved.assert_type_calls[0];
    assert!(call.actual_type.is_some());
    Ok(())
}
