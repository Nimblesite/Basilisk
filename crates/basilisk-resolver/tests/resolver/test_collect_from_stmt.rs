//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_collect_from_stmt`.

use super::common::resolve_src;

#[test]
fn function_defined_inside_try_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "try:\n",
        "    def foo(x: int) -> int:\n",
        "        return x\n",
        "except:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions.iter().any(|f| f.name == "foo"));
    Ok(())
}

#[test]
fn function_defined_inside_except_handler() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "try:\n",
        "    pass\n",
        "except Exception:\n",
        "    def bar(x: int) -> int:\n",
        "        return x\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions.iter().any(|f| f.name == "bar"));
    Ok(())
}

#[test]
fn function_defined_inside_with_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "with open('f') as fh:\n",
        "    def baz(x: int) -> int:\n",
        "        return x\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions.iter().any(|f| f.name == "baz"));
    Ok(())
}

#[test]
fn function_defined_inside_while_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "while True:\n",
        "    def wfunc(x: int) -> int:\n",
        "        return x\n",
        "    break\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions.iter().any(|f| f.name == "wfunc"));
    Ok(())
}

#[test]
fn function_defined_inside_for_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "for i in range(3):\n",
        "    def ffunc(x: int) -> int:\n",
        "        return x\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions.iter().any(|f| f.name == "ffunc"));
    Ok(())
}

#[test]
fn function_defined_inside_match_case() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x = 1\n",
        "match x:\n",
        "    case 1:\n",
        "        def matched(a: int) -> int:\n",
        "            return a\n",
        "    case _:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions.iter().any(|f| f.name == "matched"));
    Ok(())
}

#[test]
fn import_collected_from_if_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("if True:\n", "    import os\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.imports.iter().any(|i| i.module == "os"));
    Ok(())
}

#[test]
fn from_import_collected_from_if_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("if True:\n", "    from os import path\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved
        .imports
        .iter()
        .any(|i| i.names.contains(&"path".to_string())));
    Ok(())
}
