// Tests for resolver: `test_typevar_calls`.

use super::common::resolve_src;

#[test]
fn typevar_bound_typeddict_violation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar, TypedDict\n",
        "T = TypeVar('T', bound=TypedDict)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.isinstance_typeddict_violations.is_empty(),
        "TypeVar with bound=TypedDict must produce a violation"
    );
    Ok(())
}

#[test]
fn typevar_call_with_constraints_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar\n",
        "T = TypeVar('T', int, str)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.typevar_calls.len(), 1);
    Ok(())
}

#[test]
fn typevar_call_with_bound_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar\n",
        "T = TypeVar('T', bound=int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.typevar_calls.len(), 1);
    Ok(())
}

#[test]
fn paramspec_call_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("from typing import ParamSpec\n", "P = ParamSpec('P')\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.typevar_calls.len(), 1);
    Ok(())
}

#[test]
fn typevartuple_call_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVarTuple\n",
        "Ts = TypeVarTuple('Ts')\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.typevar_calls.len(), 1);
    Ok(())
}
