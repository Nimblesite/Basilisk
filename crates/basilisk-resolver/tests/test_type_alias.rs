//! Tests for resolver: `test_type_alias`.

mod common;

use common::resolve_src;

#[test]
fn type_alias_def_explicit_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeAlias\n",
        "Vector: TypeAlias = list[float]\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.type_alias_defs.len(), 1);
    assert_eq!(resolved.type_alias_defs[0].name, "Vector");
    assert_eq!(
        resolved.type_alias_defs[0].rhs_base_name,
        Some("list".to_owned())
    );
    Ok(())
}

#[test]
fn type_alias_def_implicit_subscript() -> Result<(), Box<dyn std::error::Error>> {
    let src = "IntList = list[int]\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.type_alias_defs.len(), 1);
    assert_eq!(resolved.type_alias_defs[0].name, "IntList");
    Ok(())
}

#[test]
fn type_alias_string_refs_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeAlias, Optional\n",
        "MyType: TypeAlias = Optional['ForwardRef']\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.type_alias_defs.len(), 1);
    assert!(
        resolved.type_alias_defs[0]
            .rhs_string_refs
            .contains(&"ForwardRef".to_owned()),
        "string refs in type alias RHS must be collected"
    );
    Ok(())
}

#[test]
fn type_alias_with_binop_rhs() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeAlias\n",
        "MyType: TypeAlias = int | str\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.type_alias_defs.len(), 1);
    assert_eq!(resolved.type_alias_defs[0].name, "MyType");
    Ok(())
}

#[test]
fn type_alias_with_subscript_rhs() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeAlias, List\n",
        "MyList: TypeAlias = List[int]\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.type_alias_defs.is_empty());
    Ok(())
}
