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
    let src = "def foo() -> None:\n    import os\n    from sys import argv\n    pass\n".to_owned();
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
    assert!(
        resolved.match_stmts[0].has_wildcard,
        "case _ must set has_wildcard"
    );
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
    let src = "x = 1\nmatch x:\n    case 1:\n        pass\n    case 2:\n        pass\n".to_owned();
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
    assert!(
        func.return_stmts.len() >= 2,
        "must find returns inside for loop"
    );
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
    assert!(
        !func.return_stmts[0].has_value,
        "return None must be has_value=false"
    );
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
    assert!(func
        .return_name_refs
        .iter()
        .any(|(name, _)| name == "result"));
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
    let src = "def foo() -> None:\n    while True:\n        x = 1\n        break\n".to_owned();
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
    let outer = resolved.functions.iter().find(|f| f.name == "outer");
    assert!(outer.is_some(), "outer must be present");
    let outer = outer.ok_or("outer not found")?;
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
    assert!(
        !resolved.calls.is_empty(),
        "AnnAssign call must be collected"
    );
    assert_eq!(resolved.calls[0].callee, "add");
    Ok(())
}

#[test]
fn collects_call_from_module_level_expr_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def side_effect() -> None:\n    pass\n\nside_effect()\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.calls.is_empty(),
        "Expr-stmt call must be collected"
    );
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
    assert!(
        !func.unhashable_keys.is_empty(),
        "list dict key must be detected"
    );
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
    let src = "def foo() -> None:\n    with open('f') as f:\n        d = {[1]: 'x'}\n".to_owned();
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
    let src = "def foo() -> None:\n    for i in range(1):\n        d = {[i]: 'x'}\n".to_owned();
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
    let bar = bar.ok_or("bar not found")?;
    assert!(!bar.decorators.is_empty());
    Ok(())
}

#[test]
fn collects_decorator_with_plain_name() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import overload\n@overload\ndef foo(x: int) -> int: ...\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(resolved.functions[0]
        .decorators
        .contains(&"overload".to_string()));
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
    assert!(cls
        .attributes
        .iter()
        .any(|a| a.name == "x" && !a.has_annotation));
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

// ---------------------------------------------------------------------------
// For/while else body: collect_all_assigns orelse branch
// ---------------------------------------------------------------------------

#[test]
fn collects_assigns_from_for_else_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    for i in range(10):\n",
        "        x = 1\n",
        "    else:\n",
        "        y = 2\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        func.all_local_assigns.contains(&"x".to_string()),
        "assign in for body must be collected"
    );
    assert!(
        func.all_local_assigns.contains(&"y".to_string()),
        "assign in for else body must be collected"
    );
    Ok(())
}

#[test]
fn collects_assigns_from_while_else_body() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    while True:\n",
        "        a = 1\n",
        "        break\n",
        "    else:\n",
        "        b = 2\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        func.all_local_assigns.contains(&"a".to_string()),
        "assign in while body must be collected"
    );
    assert!(
        func.all_local_assigns.contains(&"b".to_string()),
        "assign in while else body must be collected"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// List target unpacking: extract_target_names Expr::List branch
// ---------------------------------------------------------------------------

#[test]
fn collects_list_target_unpacking() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo() -> None:\n", "    [a, b] = [1, 2]\n",).to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        func.all_local_assigns.contains(&"a".to_string()),
        "first name in list-target must be collected"
    );
    assert!(
        func.all_local_assigns.contains(&"b".to_string()),
        "second name in list-target must be collected"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Return name refs: for/while else orelse branch
// ---------------------------------------------------------------------------

#[test]
fn collects_return_name_from_for_else() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> int:\n",
        "    for i in range(10):\n",
        "        pass\n",
        "    else:\n",
        "        return result\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        func.return_name_refs
            .iter()
            .any(|(name, _)| name == "result"),
        "return name in for-else clause must be collected"
    );
    Ok(())
}

#[test]
fn collects_return_name_from_while_else() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> int:\n",
        "    while True:\n",
        "        break\n",
        "    else:\n",
        "        return outcome\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        func.return_name_refs
            .iter()
            .any(|(name, _)| name == "outcome"),
        "return name in while-else clause must be collected"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Unhashable key: set and nested-dict keys, tuple/call traversal
// ---------------------------------------------------------------------------

#[test]
fn detects_set_key_in_dict() -> Result<(), Box<dyn std::error::Error>> {
    // {1, 2} as a dict key — set is unhashable at runtime
    let src = "def foo() -> None:\n    d = {{1, 2}: 'val'}\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        !func.unhashable_keys.is_empty(),
        "set dict key must be detected"
    );
    assert_eq!(func.unhashable_keys[0].key_type, "set");
    Ok(())
}

#[test]
fn detects_dict_key_in_dict() -> Result<(), Box<dyn std::error::Error>> {
    // {'a': 1} as a dict key — dicts are unhashable
    let src = "def foo() -> None:\n    d = {{'a': 1}: 'val'}\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        !func.unhashable_keys.is_empty(),
        "dict dict key must be detected"
    );
    assert_eq!(func.unhashable_keys[0].key_type, "dict");
    Ok(())
}

#[test]
fn detects_unhashable_key_inside_tuple_expr() -> Result<(), Box<dyn std::error::Error>> {
    // A tuple assigned as RHS — exercises the Expr::Tuple traversal path
    let src = "def foo() -> None:\n    _ = ({[1]: 2},)\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        !func.unhashable_keys.is_empty(),
        "unhashable key inside tuple element must be detected"
    );
    Ok(())
}

#[test]
fn detects_unhashable_key_inside_call_arg() -> Result<(), Box<dyn std::error::Error>> {
    // A dict with list key passed as a function argument — exercises Expr::Call traversal
    let src = "def foo() -> None:\n    _ = some_func({[1]: 2})\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        !func.unhashable_keys.is_empty(),
        "unhashable key inside call argument must be detected"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// RhsKind: IntLiteral, EmptyList, EmptyDict
// ---------------------------------------------------------------------------

#[test]
fn classifies_int_literal_rhs() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = "count = 42\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.module_vars[0].rhs_kind, RhsKind::IntLiteral);
    Ok(())
}

#[test]
fn classifies_empty_list_rhs() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = "items = []\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.module_vars[0].rhs_kind, RhsKind::EmptyList);
    Ok(())
}

#[test]
fn classifies_empty_dict_rhs() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::RhsKind;
    let src = "mapping = {}\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.module_vars[0].rhs_kind, RhsKind::EmptyDict);
    Ok(())
}

// ---------------------------------------------------------------------------
// collect_return_stmts: for/while orelse branches
// ---------------------------------------------------------------------------

#[test]
fn collects_return_from_for_else_clause() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> int:\n",
        "    for i in range(10):\n",
        "        pass\n",
        "    else:\n",
        "        return 99\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert_eq!(
        func.return_stmts.len(),
        1,
        "return in for-else clause must be collected"
    );
    assert!(func.return_stmts[0].has_value);
    Ok(())
}

#[test]
fn collects_return_from_while_else_clause() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> int:\n",
        "    while True:\n",
        "        break\n",
        "    else:\n",
        "        return 0\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert_eq!(
        func.return_stmts.len(),
        1,
        "return in while-else clause must be collected"
    );
    assert!(func.return_stmts[0].has_value);
    Ok(())
}

// ---------------------------------------------------------------------------
// visitor.rs: three remaining uncovered paths
// ---------------------------------------------------------------------------

/// Dict spread (`{**other}`) → item.key is None → exercises the "false" branch
/// of `if let Some(key) = item.key.as_ref()` (line 758 in visitor.rs).
#[test]
fn dict_spread_item_does_not_crash() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    other = {'b': 2}\n",
        "    d = {**other, [1]: 'val'}\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    // The [1] key is unhashable; the spread item (**other) has no key.
    assert!(
        !func.unhashable_keys.is_empty(),
        "list key in dict must be detected"
    );
    Ok(())
}

/// Module-level `AnnAssign` without value (`x: int`) → `node.value` is None
/// → exercises the "false" branch of `if let Some(val) = node.value.as_deref()`
/// (line 796 in visitor.rs).
#[test]
fn module_level_ann_assign_without_value() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x: int\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    // AnnAssign with no value: the important thing is this parses and resolves without crashing.
    let _ = &resolved.module_vars;
    Ok(())
}

/// Class with a docstring (`Stmt::Expr`) → exercises `_ => {}` in `class_info_from`
/// (line 361 in visitor.rs).
#[test]
fn class_with_docstring_not_collected_as_attribute() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo:\n",
        "    \"\"\"A docstring.\"\"\"\n",
        "    x: int = 0\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.classes.len(), 1);
    let cls = &resolved.classes[0];
    // Only `x` is an attribute; the docstring is not.
    assert_eq!(
        cls.attributes.len(),
        1,
        "docstring must not be collected as attribute"
    );
    assert_eq!(cls.attributes[0].name, "x");
    Ok(())
}

/// Module-level call where callee is an Attribute (not a simple Name) →
/// exercises the `?` early-return in `call_site_from_expr` (visitor.rs line 807).
#[test]
fn module_level_method_call_not_collected_as_call_site() -> Result<(), Box<dyn std::error::Error>> {
    let src = "result = obj.method(42)\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    // obj.method is an Attribute, not a simple Name → call_site_from_expr returns None
    assert!(
        resolved.calls.is_empty(),
        "method call must not be collected as a call site"
    );
    Ok(())
}

/// Module-level `AnnAssign` where target is an Attribute (not a Name) →
/// exercises the `?` early-return in `ann_assign_info_from` (visitor.rs line 923).
#[test]
fn module_level_ann_assign_with_attribute_target_not_collected(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = "x.y: int = 0\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    // x.y is an Attribute target → expr_simple_name returns None → no VariableInfo created
    assert!(
        resolved.module_vars.is_empty(),
        "attribute target must not be collected as a module var"
    );
    Ok(())
}

/// Class body with Attribute targets in both `AnnAssign` and Assign →
/// exercises the None branch of `expr_simple_name` in `class_info_from`
/// (visitor.rs lines 322 and 333).
#[test]
fn class_body_attribute_targets_skipped() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo:\n",
        "    x.y: int = 0\n", // AnnAssign with Attribute target → line 322 None branch
        "    a.b = 0\n",      // Assign with Attribute target → line 333 None branch
        "    name: str = 'ok'\n", // regular AnnAssign — should be collected
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.classes.len(), 1);
    let cls = &resolved.classes[0];
    // Only `name` is collected; x.y and a.b are skipped
    assert_eq!(
        cls.attributes.len(),
        1,
        "attribute-target assigns must not be collected"
    );
    assert_eq!(cls.attributes[0].name, "name");
    Ok(())
}

/// Function body with an `AnnAssign` whose target is an Attribute (not a Name) →
/// exercises the None branch in `collect_all_assigns` (visitor.rs line 539).
/// Also exercises `collect_unhashable_keys` None branches for AnnAssign-without-value
/// (line 676) and bare `return` (line 681).
#[test]
fn function_body_attribute_ann_assign_and_bare_return() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    x.y: int = 0\n", // AnnAssign with Attribute target (collect_all_assigns None, line 539)
        "    z: int\n",        // AnnAssign without value (collect_unhashable_keys None, line 676)
        "    return\n",        // bare return (collect_unhashable_keys None, line 681)
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.functions.len(), 1);
    // x.y is not a simple name → must not appear in local assigns
    let func = &resolved.functions[0];
    assert!(
        !func.all_local_assigns.contains(&"y".to_owned()),
        "attribute target must not be collected as a local assign"
    );
    Ok(())
}

/// Function body with a `with` statement that has no `as` clause →
/// exercises the None branch of `item.optional_vars` in `collect_all_assigns`
/// (visitor.rs line 565).
#[test]
fn function_body_with_clause_without_as() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    with open('f'):\n", // `with` without `as` → optional_vars is None
        "        pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.functions.len(), 1);
    Ok(())
}

/// Module-level bare expression statement (not a Call) →
/// exercises the None branch of `call_site_from_expr` for `Stmt::Expr`
/// in `collect_module_level_calls` (visitor.rs line 797).
#[test]
fn module_level_bare_expression_not_collected_as_call() -> Result<(), Box<dyn std::error::Error>> {
    let src = "42\n".to_owned(); // Stmt::Expr with NumberLiteral value — not a Call
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        resolved.calls.is_empty(),
        "bare integer expression must not produce a call site"
    );
    Ok(())
}

/// Function decorated with an `Attribute` expression (`@abc.abstractmethod`) →
/// exercises `Expr::Attribute` arm of `decorator_name` (visitor.rs line 984).
#[test]
fn attribute_decorator_name_extracted() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import abc\n",
        "class Base:\n",
        "    @abc.abstractmethod\n", // Attribute decorator → line 984
        "    def foo(self) -> None: pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let method = resolved
        .functions
        .iter()
        .find(|f| f.name == "foo")
        .ok_or("foo must be resolved")?;
    // decorator_name returns "abstractmethod" for the Attribute expression
    assert!(
        method.decorators.contains(&"abstractmethod".to_owned()),
        "attribute decorator name must be extracted"
    );
    Ok(())
}

/// Function decorated with a `Call(Name)` expression (`@deprecated("msg")`) →
/// exercises `Expr::Name` arm inside the `Expr::Call` branch of `decorator_name`
/// (visitor.rs line 986).
#[test]
fn call_name_decorator_name_extracted() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def deprecated(msg): pass\n",
        "@deprecated('use new_foo instead')\n", // Call(func=Name("deprecated")) → line 986
        "def foo() -> None: pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "foo")
        .ok_or("foo must be resolved")?;
    // decorator_name returns "deprecated" for Call(func=Name("deprecated"))
    assert!(
        func.decorators.contains(&"deprecated".to_owned()),
        "call-with-name decorator name must be extracted"
    );
    Ok(())
}

/// Function decorated with a `Call(Call(...))` expression (`@factory()()`) →
/// exercises the `_ => None` arm inside the `Expr::Call` branch of `decorator_name`
/// (visitor.rs line 988).
#[test]
fn call_exotic_func_decorator_returns_none() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def factory(): pass\n",
        "@factory()()\n", // Call(func=Call(func=Name("factory"))) → line 988
        "def foo() -> None: pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "foo")
        .ok_or("foo must be resolved")?;
    // decorator_name returns None → decorator not collected in the list
    assert!(
        func.decorators.is_empty(),
        "exotic call decorator must yield no decorator name"
    );
    Ok(())
}

/// Function decorated with a `Subscript` expression (`@buttons[0]`) →
/// exercises the `_ => None` arm of the outer match in `decorator_name`
/// (visitor.rs line 990).  Uses PEP 614 arbitrary decorator expressions.
#[test]
fn subscript_decorator_returns_none() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "buttons = [lambda f: f]\n",
        "@buttons[0]\n", // Subscript expression → outer `_ => None` (line 990)
        "def foo() -> None: pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "foo")
        .ok_or("foo must be resolved")?;
    assert!(
        func.decorators.is_empty(),
        "subscript decorator must yield no decorator name"
    );
    Ok(())
}
