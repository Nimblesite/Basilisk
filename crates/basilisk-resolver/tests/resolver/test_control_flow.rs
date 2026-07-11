//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_control_flow`.

use super::common::resolve_src;

fn assert_control_flow_keeps_lexical_scope(
    src: &str,
    module_vars: &[&str],
    module_imports: &[&str],
    local_vars: &[&str],
    local_imports: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let resolved = resolve_src(src)?;

    for name in module_vars {
        assert!(
            resolved.module_vars.iter().any(|var| var.name == *name),
            "module binding `{name}` was lost inside control flow"
        );
    }
    for name in local_vars {
        assert!(
            resolved.module_vars.iter().all(|var| var.name != *name),
            "function-local binding `{name}` leaked into module scope"
        );
    }
    for module in module_imports {
        assert!(
            resolved
                .imports
                .iter()
                .any(|import| import.module == *module),
            "module import `{module}` was lost inside control flow"
        );
    }
    for module in local_imports {
        assert!(
            resolved
                .imports
                .iter()
                .all(|import| import.module != *module),
            "function-local import `{module}` leaked into module scope"
        );
    }

    let function = resolved
        .functions
        .iter()
        .find(|function| function.name == "local_scope")
        .expect("local_scope function must resolve");
    for name in local_vars {
        assert!(
            function.local_vars.iter().any(|var| var.name == *name),
            "function-local binding `{name}` was not retained as a local"
        );
    }

    Ok(())
}

#[test]
fn for_blocks_keep_module_bindings_and_function_locals_separate(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
for _ in ():
    module_for_body: int = 1
    import module_for_body_import
else:
    module_for_else: int = 2
    import module_for_else_import

def local_scope() -> None:
    for _ in ():
        local_for_body: int = 1
        import local_for_body_import
    else:
        local_for_else: int = 2
        import local_for_else_import
";
    assert_control_flow_keeps_lexical_scope(
        src,
        &["module_for_body", "module_for_else"],
        &["module_for_body_import", "module_for_else_import"],
        &["local_for_body", "local_for_else"],
        &["local_for_body_import", "local_for_else_import"],
    )
}

#[test]
fn while_blocks_keep_module_bindings_and_function_locals_separate(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
while False:
    module_while_body: int = 1
    import module_while_body_import
else:
    module_while_else: int = 2
    import module_while_else_import

def local_scope() -> None:
    while False:
        local_while_body: int = 1
        import local_while_body_import
    else:
        local_while_else: int = 2
        import local_while_else_import
";
    assert_control_flow_keeps_lexical_scope(
        src,
        &["module_while_body", "module_while_else"],
        &["module_while_body_import", "module_while_else_import"],
        &["local_while_body", "local_while_else"],
        &["local_while_body_import", "local_while_else_import"],
    )
}

#[test]
fn with_blocks_keep_module_bindings_and_function_locals_separate(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
with context_manager():
    module_with_body: int = 1
    import module_with_body_import

def local_scope() -> None:
    with context_manager():
        local_with_body: int = 1
        import local_with_body_import
";
    assert_control_flow_keeps_lexical_scope(
        src,
        &["module_with_body"],
        &["module_with_body_import"],
        &["local_with_body"],
        &["local_with_body_import"],
    )
}

#[test]
fn try_blocks_keep_module_bindings_and_function_locals_separate(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
try:
    module_try_body: int = 1
    import module_try_body_import
except Exception:
    module_try_except: int = 2
    import module_try_except_import
else:
    module_try_else: int = 3
    import module_try_else_import
finally:
    module_try_finally: int = 4
    import module_try_finally_import

def local_scope() -> None:
    try:
        local_try_body: int = 1
        import local_try_body_import
    except Exception:
        local_try_except: int = 2
        import local_try_except_import
    else:
        local_try_else: int = 3
        import local_try_else_import
    finally:
        local_try_finally: int = 4
        import local_try_finally_import
";
    assert_control_flow_keeps_lexical_scope(
        src,
        &[
            "module_try_body",
            "module_try_except",
            "module_try_else",
            "module_try_finally",
        ],
        &[
            "module_try_body_import",
            "module_try_except_import",
            "module_try_else_import",
            "module_try_finally_import",
        ],
        &[
            "local_try_body",
            "local_try_except",
            "local_try_else",
            "local_try_finally",
        ],
        &[
            "local_try_body_import",
            "local_try_except_import",
            "local_try_else_import",
            "local_try_finally_import",
        ],
    )
}

#[test]
fn match_blocks_keep_module_bindings_and_function_locals_separate(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = r"
match subject:
    case _:
        module_match_case: int = 1
        import module_match_case_import

def local_scope() -> None:
    match subject:
        case _:
            local_match_case: int = 1
            import local_match_case_import
";
    assert_control_flow_keeps_lexical_scope(
        src,
        &["module_match_case"],
        &["module_match_case_import"],
        &["local_match_case"],
        &["local_match_case_import"],
    )
}

#[test]
fn resolves_for_loop_with_else_clause() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> None:\n    for i in range(10):\n        pass\n    else:\n        pass\n"
            .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.functions.len(), 1);
    assert_eq!(resolved.functions[0].name, "foo");
    Ok(())
}

#[test]
fn resolves_while_loop_with_else_clause() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> None:\n    while True:\n        break\n    else:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.functions.len(), 1);
    Ok(())
}

#[test]
fn resolves_with_statement() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    with open('f') as g:\n        pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.functions.len(), 1);
    Ok(())
}

#[test]
fn resolves_try_except_else_finally() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    try:\n",
        "        pass\n",
        "    except Exception:\n",
        "        pass\n",
        "    else:\n",
        "        pass\n",
        "    finally:\n",
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.functions.len(), 1);
    Ok(())
}

#[test]
fn non_module_level_imports_not_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    import os\n    from sys import argv\n    pass\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.imports.is_empty(),
        "function-level imports must not be collected"
    );
    Ok(())
}

#[test]
fn non_module_level_assigns_not_collected_as_vars() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    x = 1\n    y: int = 2\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.module_vars.is_empty(),
        "function-level assigns must not be collected as module vars"
    );
    Ok(())
}

#[test]
fn collects_return_from_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> int:\n    for i in range(10):\n        return i\n    return 0\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(
        func.return_stmts.len() >= 2,
        "must find returns inside for loop"
    );
    Ok(())
}

#[test]
fn collects_return_from_while_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    while True:\n        return 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert_eq!(func.return_stmts.len(), 1);
    assert!(func.return_stmts[0].has_value);
    Ok(())
}

#[test]
fn collects_return_from_with_statement() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    with open('f') as f:\n        return 1\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert_eq!(func.return_stmts.len(), 1);
    assert!(func.return_stmts[0].has_value);
    Ok(())
}

#[test]
fn collects_return_from_try_and_except() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> int:\n",
        "    try:\n",
        "        return 1\n",
        "    except Exception:\n",
        "        return 0\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert_eq!(
        func.return_stmts.len(),
        2,
        "return in try body + return in except body = 2"
    );
    Ok(())
}

#[test]
fn collects_return_none_as_no_value() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    return None\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert_eq!(func.return_stmts.len(), 1);
    assert!(
        !func.return_stmts[0].has_value,
        "return None must be has_value=false"
    );
    Ok(())
}

#[test]
fn collects_return_name_from_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> int:\n    for i in range(10):\n        return i\n    return 0\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(
        func.return_name_refs.iter().any(|(name, _)| name == "i"),
        "return i inside for must be collected"
    );
    Ok(())
}

#[test]
fn collects_return_name_from_while_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    while True:\n        return result\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(func
        .return_name_refs
        .iter()
        .any(|(name, _)| name == "result"));
    Ok(())
}

#[test]
fn collects_return_name_from_with_statement() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    with open('f') as ctx:\n        return val\n".to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(func.return_name_refs.iter().any(|(name, _)| name == "val"));
    Ok(())
}

#[test]
fn collects_return_name_from_try_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> int:\n",
        "    try:\n",
        "        return a\n",
        "    except Exception:\n",
        "        return b\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    assert!(func.return_name_refs.iter().any(|(name, _)| name == "a"));
    assert!(func.return_name_refs.iter().any(|(name, _)| name == "b"));
    Ok(())
}

#[test]
fn calls_collected_from_nested_if() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    pass\n",
        "if True:\n",
        "    foo()\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.calls.is_empty());
    Ok(())
}

#[test]
fn reveal_type_calls_in_if_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("x = 5\n", "if True:\n", "    reveal_type(x)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.reveal_type_calls.is_empty());
    Ok(())
}
