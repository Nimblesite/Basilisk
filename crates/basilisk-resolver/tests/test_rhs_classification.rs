//! Tests for resolver: test_rhs_classification.

mod common;

use common::resolve_src;

#[test]
fn classifies_various_literal_rhs_at_module_level() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = concat!(
        "a = b\"bytes\"\n",
        "b = 1.0\n",
        "c = \"str\"\n",
        "d = True\n",
        "e = None\n",
        "f = something_else\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_vars.len(), 6);
    let kinds: Vec<&RhsKind> = resolved.module_vars.iter().map(|v| &v.rhs_kind).collect();
    assert!(kinds.contains(&&RhsKind::BytesLiteral), "bytes literal");
    assert!(kinds.contains(&&RhsKind::FloatLiteral), "float literal");
    assert!(kinds.contains(&&RhsKind::StrLiteral), "str literal");
    assert!(kinds.contains(&&RhsKind::BoolLiteral), "bool literal");
    assert!(kinds.contains(&&RhsKind::NoneValue), "None literal");
    assert!(kinds.contains(&&RhsKind::Other), "other expr");
    Ok(())
}

#[test]
fn classifies_call_expr_rhs() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = "result = some_func()\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_vars.len(), 1);
    assert_eq!(resolved.module_vars[0].rhs_kind, RhsKind::CallExpr);
    Ok(())
}

#[test]
fn classifies_fstring_as_str_literal() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = "name = f\"hello\"\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_vars.len(), 1);
    assert_eq!(resolved.module_vars[0].rhs_kind, RhsKind::StrLiteral);
    Ok(())
}

#[test]
fn classifies_int_literal_rhs() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = "count = 42\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_vars[0].rhs_kind, RhsKind::IntLiteral);
    Ok(())
}

#[test]
fn classifies_empty_list_rhs() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = "items = []\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_vars[0].rhs_kind, RhsKind::EmptyList);
    Ok(())
}

#[test]
fn classifies_empty_dict_rhs() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = "mapping = {}\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_vars[0].rhs_kind, RhsKind::EmptyDict);
    Ok(())
}
