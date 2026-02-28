//! Integration tests for basilisk-resolver.

use basilisk_parser::parse_source;
use basilisk_resolver::resolve;

#[test]
fn detects_unannotated_parameter() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def process(data) -> None:\n    pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;

    assert_eq!(resolved.functions.len(), 1);
    let func = &resolved.functions[0];
    assert_eq!(func.name, "process");
    assert_eq!(func.parameters.len(), 1);
    assert!(!func.parameters[0].has_annotation);
    assert!(func.return_annotation.is_present());
    Ok(())
}

#[test]
fn detects_fully_annotated_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def greet(name: str) -> str:\n    return name\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;

    assert_eq!(resolved.functions.len(), 1);
    let func = &resolved.functions[0];
    assert!(func.parameters[0].has_annotation);
    assert!(func.return_annotation.is_present());
    Ok(())
}

#[test]
fn detects_missing_return_annotation() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def fetch(url: str):\n    pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;

    assert!(!resolved.functions[0].return_annotation.is_present());
    Ok(())
}

#[test]
fn finds_nested_functions() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def outer() -> None:\n    def inner(x: int) -> None:\n        pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;

    assert_eq!(
        resolved.functions.len(),
        2,
        "should find both outer and inner"
    );
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"outer"));
    assert!(names.contains(&"inner"));
    Ok(())
}

#[test]
fn handles_methods_in_class() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Foo:\n    def bar(self, x: int) -> None:\n        pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;

    assert_eq!(resolved.functions.len(), 1);
    assert_eq!(resolved.functions[0].name, "bar");
    Ok(())
}

#[test]
fn handles_empty_module() -> Result<(), Box<dyn std::error::Error>> {
    let src = String::new();
    let parsed = parse_source(src, "empty.py".to_owned())?;
    let resolved = resolve(&parsed)?;

    assert!(resolved.functions.is_empty());
    Ok(())
}

#[test]
fn detects_vararg_and_kwarg() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def variadic(*args: int, **kwargs: str) -> None:\n    pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;

    let func = &resolved.functions[0];
    assert!(func.vararg.as_ref().is_some_and(|p| p.has_annotation));
    assert!(func.kwarg.as_ref().is_some_and(|p| p.has_annotation));
    Ok(())
}

#[test]
fn span_start_before_end() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo(x: int) -> None:\n    pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;

    let func = &resolved.functions[0];
    assert!(func.def_span.start < func.def_span.end);
    assert!(func.name_span.start < func.name_span.end);
    Ok(())
}

// ---------------------------------------------------------------------------
// Control flow: for/while/with/try
// ---------------------------------------------------------------------------

#[test]
fn resolves_for_loop_with_else_clause() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> None:\n    for i in range(10):\n        pass\n    else:\n        pass\n"
            .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.functions.len(), 1);
    assert_eq!(resolved.functions[0].name, "foo");
    Ok(())
}

#[test]
fn resolves_while_loop_with_else_clause() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> None:\n    while True:\n        break\n    else:\n        pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.functions.len(), 1);
    Ok(())
}

#[test]
fn resolves_with_statement() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    with open('f') as g:\n        pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
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
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.functions.len(), 1);
    Ok(())
}

#[test]
fn non_module_level_imports_not_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> None:\n    import os\n    from sys import argv\n    pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        resolved.imports.is_empty(),
        "function-level imports must not be collected"
    );
    Ok(())
}

#[test]
fn non_module_level_assigns_not_collected_as_vars() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    x = 1\n    y: int = 2\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        resolved.module_vars.is_empty(),
        "function-level assigns must not be collected as module vars"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Match statements
// ---------------------------------------------------------------------------

#[test]
fn resolves_match_statement_with_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x = 1\nmatch x:\n    case 1:\n        pass\n    case _:\n        pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.match_stmts.len(), 1);
    assert!(resolved.match_stmts[0].has_wildcard, "case _ must set has_wildcard");
    Ok(())
}

#[test]
fn resolves_match_with_or_wildcard_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x = 1\nmatch x:\n    case 1 | _:\n        pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.match_stmts.len(), 1);
    assert!(
        resolved.match_stmts[0].has_wildcard,
        "case 1 | _ must be recognised as wildcard via MatchOr"
    );
    Ok(())
}

#[test]
fn match_without_wildcard_has_no_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "x = 1\nmatch x:\n    case 1:\n        pass\n    case 2:\n        pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.match_stmts.len(), 1);
    assert!(!resolved.match_stmts[0].has_wildcard);
    Ok(())
}

// ---------------------------------------------------------------------------
// RHS literal classification
// ---------------------------------------------------------------------------

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
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
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
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.module_vars.len(), 1);
    assert_eq!(resolved.module_vars[0].rhs_kind, RhsKind::CallExpr);
    Ok(())
}

#[test]
fn classifies_fstring_as_str_literal() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = "name = f\"hello\"\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.module_vars.len(), 1);
    assert_eq!(resolved.module_vars[0].rhs_kind, RhsKind::StrLiteral);
    Ok(())
}

// ---------------------------------------------------------------------------
// Annotation flags: Attribute and literal return types
// ---------------------------------------------------------------------------

#[test]
fn annotation_attribute_detected_as_any() -> Result<(), Box<dyn std::error::Error>> {
    let src = "import typing\ndef foo(x: typing.Any) -> typing.Any:\n    pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
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
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
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
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(
        resolved.functions[0].return_annotation,
        ReturnAnnotationKind::NumericLiteral
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Return statement collection from nested control flow
// ---------------------------------------------------------------------------

#[test]
fn collects_return_from_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> int:\n    for i in range(10):\n        return i\n    return 0\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(func.return_stmts.len() >= 2, "must find returns inside for loop");
    Ok(())
}

#[test]
fn collects_return_from_while_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    while True:\n        return 1\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert_eq!(func.return_stmts.len(), 1);
    assert!(func.return_stmts[0].has_value);
    Ok(())
}

#[test]
fn collects_return_from_with_statement() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    with open('f') as f:\n        return 1\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
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
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
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
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert_eq!(func.return_stmts.len(), 1);
    assert!(!func.return_stmts[0].has_value, "return None must be has_value=false");
    Ok(())
}

// ---------------------------------------------------------------------------
// Return name ref collection from nested control flow
// ---------------------------------------------------------------------------

#[test]
fn collects_return_name_from_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> int:\n    for i in range(10):\n        return i\n    return 0\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
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
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(func.return_name_refs.iter().any(|(name, _)| name == "result"));
    Ok(())
}

#[test]
fn collects_return_name_from_with_statement() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> int:\n    with open('f') as ctx:\n        return val\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
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
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(func.return_name_refs.iter().any(|(name, _)| name == "a"));
    assert!(func.return_name_refs.iter().any(|(name, _)| name == "b"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Local assign collection
// ---------------------------------------------------------------------------

#[test]
fn collects_ann_assign_in_function_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    x: int = 1\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        func.all_local_assigns.contains(&"x".to_string()),
        "annotated assign must be collected"
    );
    Ok(())
}

#[test]
fn collects_tuple_targets_from_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    for a, b in [(1, 2)]:\n        pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(func.all_local_assigns.contains(&"a".to_string()));
    assert!(func.all_local_assigns.contains(&"b".to_string()));
    Ok(())
}

#[test]
fn collects_assigns_from_while_body() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> None:\n    while True:\n        x = 1\n        break\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(func.all_local_assigns.contains(&"x".to_string()));
    Ok(())
}

#[test]
fn collects_with_statement_variable() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    with open('f') as ctx:\n        pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        func.all_local_assigns.contains(&"ctx".to_string()),
        "with-statement variable must be collected"
    );
    Ok(())
}

#[test]
fn collects_try_except_named_exception() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    try:\n",
        "        pass\n",
        "    except Exception as exc:\n",
        "        pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        func.all_local_assigns.contains(&"exc".to_string()),
        "named exception binding must be collected"
    );
    Ok(())
}

#[test]
fn collects_nested_function_name_as_local_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def outer() -> None:\n    def inner() -> None:\n        pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let outer = resolved
        .functions
        .iter()
        .find(|f| f.name == "outer")
        .expect("outer must be present");
    assert!(
        outer.all_local_assigns.contains(&"inner".to_string()),
        "nested function name must appear in enclosing scope's assigns"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Module-level call site collection
// ---------------------------------------------------------------------------

#[test]
fn collects_call_from_module_level_ann_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def add(x: int) -> int:\n    return x\n\nresult: int = add(42)\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(!resolved.calls.is_empty(), "AnnAssign call must be collected");
    assert_eq!(resolved.calls[0].callee, "add");
    Ok(())
}

#[test]
fn collects_call_from_module_level_expr_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def side_effect() -> None:\n    pass\n\nside_effect()\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(!resolved.calls.is_empty(), "Expr-stmt call must be collected");
    assert_eq!(resolved.calls[0].callee, "side_effect");
    Ok(())
}

// ---------------------------------------------------------------------------
// Unhashable key collection
// ---------------------------------------------------------------------------

#[test]
fn detects_list_key_in_dict() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    d = {[1, 2]: 'val'}\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(!func.unhashable_keys.is_empty(), "list dict key must be detected");
    assert_eq!(func.unhashable_keys[0].key_type, "list");
    Ok(())
}

#[test]
fn detects_unhashable_key_in_if_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    if True:\n        d = {[1]: 'x'}\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_unhashable_key_in_return_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> dict:\n    return {[1]: 'x'}\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_unhashable_key_in_expr_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def foo() -> None:\n    _ = {[1]: 'x'}\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_unhashable_key_in_while_body() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> None:\n    while True:\n        d = {[1]: 'x'}\n        break\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_unhashable_key_in_with_body() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> None:\n    with open('f') as f:\n        d = {[1]: 'x'}\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_unhashable_key_in_try_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    try:\n",
        "        d = {[1]: 'x'}\n",
        "    except Exception:\n",
        "        pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

#[test]
fn detects_unhashable_key_in_for_body() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "def foo() -> None:\n    for i in range(1):\n        d = {[i]: 'x'}\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(!func.unhashable_keys.is_empty());
    Ok(())
}

// ---------------------------------------------------------------------------
// Decorator collection
// ---------------------------------------------------------------------------

#[test]
fn collects_decorator_with_call_on_attribute() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import functools\n",
        "class Foo:\n",
        "    @functools.lru_cache(maxsize=128)\n",
        "    def bar(self: 'Foo') -> int:\n",
        "        return 0\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let bar = resolved.functions.iter().find(|f| f.name == "bar");
    assert!(bar.is_some(), "bar method must be resolved");
    assert!(!bar.unwrap().decorators.is_empty());
    Ok(())
}

#[test]
fn collects_decorator_with_plain_name() -> Result<(), Box<dyn std::error::Error>> {
    let src =
        "from typing import overload\n@overload\ndef foo(x: int) -> int: ...\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(resolved.functions[0].decorators.contains(&"overload".to_string()));
    Ok(())
}

// ---------------------------------------------------------------------------
// Class body variants
// ---------------------------------------------------------------------------

#[test]
fn class_assign_without_annotation_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = "class Foo:\n    x = 1\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.classes.len(), 1);
    let cls = &resolved.classes[0];
    assert!(cls.attributes.iter().any(|a| a.name == "x" && !a.has_annotation));
    Ok(())
}

#[test]
fn nested_class_methods_collected_in_functions() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Outer:\n",
        "    class Inner:\n",
        "        def inner_method(self: 'Inner') -> None:\n",
        "            pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        resolved.functions.iter().any(|f| f.name == "inner_method"),
        "nested class methods must be collected"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Import variants
// ---------------------------------------------------------------------------

#[test]
fn resolves_star_import() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ImportKind;
    let src = "from os.path import *\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.imports.len(), 1);
    assert_eq!(resolved.imports[0].kind, ImportKind::Star);
    Ok(())
}

#[test]
fn resolves_plain_import() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ImportKind;
    let src = "import os\nimport sys\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.imports.len(), 2);
    assert!(resolved.imports.iter().all(|i| i.kind == ImportKind::Plain));
    Ok(())
}

#[test]
fn resolves_from_import_without_module_name() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ImportKind;
    // Relative import: `from . import utils` — module field will be empty
    let src = "from . import utils\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.imports.len(), 1);
    assert_eq!(resolved.imports[0].kind, ImportKind::From);
    assert_eq!(resolved.imports[0].module, "");
    Ok(())
}
