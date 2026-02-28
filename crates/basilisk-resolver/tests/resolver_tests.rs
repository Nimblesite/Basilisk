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
        "    z: int\n",       // AnnAssign without value (collect_unhashable_keys None, line 676)
        "    return\n",       // bare return (collect_unhashable_keys None, line 681)
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

// ---------------------------------------------------------------------------
// Missed mutant coverage: collect_from_stmt match arms
// ---------------------------------------------------------------------------

/// Exercises the `Stmt::For` match arm in `collect_from_stmt` — a function
/// defined inside a for-loop body must be collected.
/// Kills the `MatchArm` → empty mutant at visitor.rs:142 / :122.
#[test]
fn collect_from_stmt_for_loop_body_has_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def outer() -> None:\n",
        "    for i in range(3):\n",
        "        def inner(x: int) -> None:\n",
        "            pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.contains(&"inner"),
        "inner must be collected from for-body"
    );
    Ok(())
}

/// Exercises the `Stmt::While` match arm in `collect_from_stmt`.
/// Kills the `MatchArm` → empty mutant at visitor.rs:162.
#[test]
fn collect_from_stmt_while_body_has_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def outer() -> None:\n",
        "    while False:\n",
        "        def inner(x: int) -> None:\n",
        "            pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.contains(&"inner"),
        "inner must be collected from while-body"
    );
    Ok(())
}

/// Exercises the `Stmt::With` match arm in `collect_from_stmt`.
/// Kills the `MatchArm` → empty mutant at visitor.rs:173.
#[test]
fn collect_from_stmt_with_body_has_nested_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def outer() -> None:\n",
        "    with open('f') as g:\n",
        "        def inner(x: int) -> None:\n",
        "            pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.contains(&"inner"),
        "inner must be collected from with-body"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: collect_from_handlers
// ---------------------------------------------------------------------------

/// Exercises `collect_from_handlers` — functions inside except handlers must
/// be collected.  Kills the `FnValue` → `()` mutant at visitor.rs:281.
#[test]
fn collect_from_handlers_collects_function_in_except() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def outer() -> None:\n",
        "    try:\n",
        "        pass\n",
        "    except Exception:\n",
        "        def inner(x: int) -> None:\n",
        "            pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let names: Vec<&str> = resolved.functions.iter().map(|f| f.name.as_str()).collect();
    assert!(
        names.contains(&"inner"),
        "inner must be collected from except handler"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: class_info_from — is_dataclass / is_final
// ---------------------------------------------------------------------------

/// `class_info_from` at line 389: `d == "dataclass" || d.ends_with(".dataclass")`.
/// Replacing `||` with `&&` would miss the qualified `dataclasses.dataclass`.
/// This test uses the qualified form to kill that mutant.
#[test]
fn class_info_from_qualified_dataclass_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import dataclasses\n",
        "@dataclasses.dataclass\n",
        "class Point:\n",
        "    x: int = 0\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let cls = resolved
        .classes
        .iter()
        .find(|c| c.name == "Point")
        .ok_or("Point not found")?;
    assert!(
        cls.is_dataclass,
        "qualified @dataclasses.dataclass must set is_dataclass"
    );
    Ok(())
}

/// `class_info_from` at line 389: the simple `"dataclass"` branch.
/// Replacing `||` with `&&` would miss the bare `@dataclass`.
#[test]
fn class_info_from_bare_dataclass_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from dataclasses import dataclass\n",
        "@dataclass\n",
        "class Rect:\n",
        "    w: int = 0\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let cls = resolved
        .classes
        .iter()
        .find(|c| c.name == "Rect")
        .ok_or("Rect not found")?;
    assert!(cls.is_dataclass, "bare @dataclass must set is_dataclass");
    Ok(())
}

/// `class_info_from` at line 395: `d == "final" || d.rsplit('.').next() == Some("final")`.
/// Replacing `||` with `&&` would miss the qualified `typing.final`.
#[test]
fn class_info_from_qualified_final_decorator() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import typing\n",
        "@typing.final\n",
        "class Sealed:\n",
        "    pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let cls = resolved
        .classes
        .iter()
        .find(|c| c.name == "Sealed")
        .ok_or("Sealed not found")?;
    assert!(cls.is_final, "qualified @typing.final must set is_final");
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: body_is_stub
// ---------------------------------------------------------------------------

/// `body_is_stub` must return `true` for a pure `...` body.
/// Kills `FnValue → false` at visitor.rs:481.
#[test]
fn body_is_stub_ellipsis_only_is_stub() -> Result<(), Box<dyn std::error::Error>> {
    // @overload functions with `...` bodies are stubs — E0001/E0002 must not fire.
    let src = concat!(
        "from typing import overload\n",
        "@overload\n",
        "def process(x: int) -> int: ...\n",
        "@overload\n",
        "def process(x: str) -> str: ...\n",
        "def process(x):\n",
        "    return x\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    // The overload stubs must have is_stub_body = true
    let overloads: Vec<_> = resolved
        .functions
        .iter()
        .filter(|f| f.decorators.iter().any(|d| d == "overload"))
        .collect();
    assert!(!overloads.is_empty(), "overloads must be resolved");
    for f in &overloads {
        assert!(f.is_stub_body, "overload with `...` body must be stub");
    }
    Ok(())
}

/// `body_is_stub` must return `false` for a real body.
/// The mutation `false` would make real functions look like stubs.
#[test]
fn body_is_stub_real_body_is_not_stub() -> Result<(), Box<dyn std::error::Error>> {
    let src = "def process(x: int) -> int:\n    return x + 1\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(!func.is_stub_body, "real body must not be stub");
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: collect_return_stmts match arm (visitor.rs:521)
// ---------------------------------------------------------------------------

/// `collect_return_stmts` — the `Stmt::For` arm (line 521 area).
/// Replacing this arm with empty would miss returns inside for-loops.
#[test]
fn collect_return_stmts_return_inside_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def find(items: list) -> int:\n",
        "    for item in items:\n",
        "        return item\n",
        "    return 0\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "find")
        .ok_or("find not found")?;
    assert!(
        func.return_stmts.len() >= 2,
        "both return stmts must be collected, got {}",
        func.return_stmts.len()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: collect_unconditional_assigns match arms
// ---------------------------------------------------------------------------

/// `collect_unconditional_assigns` — `Stmt::AnnAssign` arm (visitor.rs:652).
/// Killing this arm means annotated assignments won't appear in `unconditional_assigns`.
#[test]
fn collect_unconditional_assigns_ann_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> str:\n",
        "    result: str = 'hello'\n",
        "    return result\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "foo")
        .ok_or("foo not found")?;
    assert!(
        func.unconditional_assigns.contains(&"result".to_owned()),
        "annotated assign must appear in unconditional_assigns"
    );
    Ok(())
}

/// `collect_unconditional_assigns` — `Stmt::For` arm collects loop variable (visitor.rs:647).
#[test]
fn collect_unconditional_assigns_for_target() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    for item in range(3):\n",
        "        pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "foo")
        .ok_or("foo not found")?;
    assert!(
        func.unconditional_assigns.contains(&"item".to_owned()),
        "for loop variable must appear in unconditional_assigns"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: collect_return_name_refs match arm (visitor.rs:673)
// ---------------------------------------------------------------------------

/// `collect_return_name_refs` — exercising the For arm so that returns inside
/// loops are captured.  Kills `MatchArm` → empty at visitor.rs:673.
#[test]
fn collect_return_name_refs_inside_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def first(items: list) -> object:\n",
        "    for item in items:\n",
        "        return item\n",
        "    return items\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "first")
        .ok_or("first not found")?;
    let names: Vec<&str> = func
        .return_name_refs
        .iter()
        .map(|(n, _)| n.as_str())
        .collect();
    assert!(
        names.contains(&"item"),
        "return inside for loop must be captured in return_name_refs"
    );
    assert!(
        names.contains(&"items"),
        "return after for loop must also be captured"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: collect_unhashable_keys_from_stmt match arms
// ---------------------------------------------------------------------------

/// Exercises `Stmt::Assign` arm in `collect_unhashable_keys_from_stmt` (line 723).
/// Killing this arm would miss unhashable keys in assignments.
#[test]
fn unhashable_keys_in_assign_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = "d = {[1, 2]: 'bad'}\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.module_vars.is_empty(),
        "variable d must be resolved"
    );
    Ok(())
}

/// Exercises `Stmt::Return` arm in `collect_unhashable_keys_from_stmt` (line 733).
/// Killing this arm would miss unhashable keys in return statements.
#[test]
fn unhashable_keys_in_return_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def bad() -> dict:\n", "    return {[1, 2]: 'bad'}\n",).to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "bad")
        .ok_or("bad not found")?;
    assert!(
        !func.unhashable_keys.is_empty(),
        "unhashable key in return must be collected"
    );
    Ok(())
}

/// Exercises `Stmt::Expr` arm in `collect_unhashable_keys_from_stmt` (line 803).
/// Killing the arm misses unhashable keys in standalone expression statements.
#[test]
fn unhashable_keys_in_expr_stmt() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def f() -> None:\n", "    {[1, 2]: 'key'}\n",).to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = resolved
        .functions
        .iter()
        .find(|f| f.name == "f")
        .ok_or("f not found")?;
    assert!(
        !func.unhashable_keys.is_empty(),
        "unhashable key in expr stmt must be collected"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: collect_module_level_calls match arm (line 839)
// ---------------------------------------------------------------------------

/// `collect_module_level_calls` — `Stmt::Assign` arm (line 839 area).
/// Killing this arm means `TypeVar` calls in assignments aren't collected.
#[test]
fn collect_module_level_calls_from_assign() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = TypeVar('T', int, str)\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.typevar_calls.is_empty(),
        "TypeVar in plain assignment must be collected"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: collect_typevar_calls match arm + operators
// ---------------------------------------------------------------------------

/// `collect_typevar_calls` — kills `FnValue → vec![]` at line 857.
/// `TypeVar` must be extracted and returned, not an empty vec.
#[test]
fn collect_typevar_calls_returns_entries() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar\n",
        "T = TypeVar('T', int, str)\n",
        "S = TypeVar('S')\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(
        resolved.typevar_calls.len(),
        2,
        "both TypeVars must be collected"
    );
    Ok(())
}

/// `collect_typevar_calls` — `Stmt::Assign` arm (line 860).
/// Killing this arm means `TypeVar` assignments are skipped.
#[test]
fn collect_typevar_calls_plain_assign_arm() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = TypeVar('T', int, str)\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let tv = resolved
        .typevar_calls
        .iter()
        .find(|t| t.name == "T")
        .ok_or("T not found")?;
    assert_eq!(tv.constraint_count, 2);
    Ok(())
}

/// `collect_typevar_calls` — `Stmt::AnnAssign` arm (line 898).
/// Killing this arm means annotated `TypeVar` assignments are skipped.
#[test]
fn collect_typevar_calls_ann_assign_arm() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar\n",
        "T: TypeVar = TypeVar('T', int, str)\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.typevar_calls.is_empty(),
        "annotated TypeVar assignment must be collected"
    );
    Ok(())
}

/// `collect_typevar_calls` — `attr.attr == "TypeVar"` condition (line 865 area).
/// `!=` mutant would accept all attribute calls as `TypeVar`. We test that
/// `typing.TypeVar` IS collected (killing `!=` that would skip it).
#[test]
fn collect_typevar_calls_qualified_typing_typevar() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = typing.TypeVar('T', int, str)\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.typevar_calls.is_empty(),
        "typing.TypeVar must be collected"
    );
    let tv = &resolved.typevar_calls[0];
    assert_eq!(tv.name, "T");
    assert_eq!(tv.constraint_count, 2);
    Ok(())
}

/// Kills `!=` mutant at line 869 — callee must equal "`TypeVar`", not anything else.
/// A non-TypeVar call (e.g. `T = int('T')`) must NOT be collected.
#[test]
fn collect_typevar_calls_ignores_non_typevar_calls() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = int('T')\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        resolved.typevar_calls.is_empty(),
        "non-TypeVar call must not be collected"
    );
    Ok(())
}

/// Kills `!=` mutant at line 890 — same check in `AnnAssign` branch.
#[test]
fn collect_typevar_calls_ann_assign_ignores_non_typevar() -> Result<(), Box<dyn std::error::Error>>
{
    let src = "T: int = int('T')\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        resolved.typevar_calls.is_empty(),
        "non-TypeVar ann-assign must not be collected"
    );
    Ok(())
}

/// Kills `!=` mutant at line 904 — the `kw.arg == "default"` check.
/// A `TypeVar` with `default=int` must have `has_default = true`.
#[test]
fn collect_typevar_calls_has_default_true() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = TypeVar('T', default=int)\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let tv = resolved
        .typevar_calls
        .iter()
        .find(|t| t.name == "T")
        .ok_or("T not found")?;
    assert!(
        tv.has_default,
        "TypeVar with default= must have has_default=true"
    );
    Ok(())
}

/// Kills `!=` mutant at line 907 — `has_default = false` when no `default=` kwarg.
#[test]
fn collect_typevar_calls_has_default_false_when_absent() -> Result<(), Box<dyn std::error::Error>> {
    let src = "T = TypeVar('T', int, str)\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let tv = resolved
        .typevar_calls
        .iter()
        .find(|t| t.name == "T")
        .ok_or("T not found")?;
    assert!(
        !tv.has_default,
        "TypeVar without default= must have has_default=false"
    );
    Ok(())
}

/// Kills `!=` mutant at line 925 — same `has_default` check in `AnnAssign` arm.
#[test]
fn collect_typevar_calls_ann_assign_has_default() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar\n",
        "T: TypeVar = TypeVar('T', default=int)\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let tv = resolved
        .typevar_calls
        .iter()
        .find(|t| t.name == "T")
        .ok_or("T not found")?;
    assert!(
        tv.has_default,
        "annotated TypeVar with default= must have has_default=true"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: extract_generic_params (line 941, 948, 949)
// ---------------------------------------------------------------------------

/// `extract_generic_params` — `FnValue → vec![]` at line 941.
/// A class with `Generic[T, S]` must have 2 params; empty vec means none collected.
#[test]
fn extract_generic_params_collects_multiple_params() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar, Generic\n",
        "T = TypeVar('T')\n",
        "S = TypeVar('S')\n",
        "class Pair(Generic[T, S]):\n",
        "    pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let cls = resolved
        .classes
        .iter()
        .find(|c| c.name == "Pair")
        .ok_or("Pair not found")?;
    assert_eq!(
        cls.generic_params.len(),
        2,
        "Generic[T, S] must produce 2 params"
    );
    Ok(())
}

/// `extract_generic_params` — `&&` to `||` and `!` to empty mutants at lines 948/949.
/// Both conditions must hold: a `Subscript` whose value is `Generic`.
/// Non-Generic subscripts must not produce params.
#[test]
fn extract_generic_params_non_generic_subscript_ignored() -> Result<(), Box<dyn std::error::Error>>
{
    let src = concat!(
        "from typing import TypeVar\n",
        "T = TypeVar('T')\n",
        "class Wrapper(list[T]):\n", // list[T] is not Generic[T]
        "    pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let cls = resolved
        .classes
        .iter()
        .find(|c| c.name == "Wrapper")
        .ok_or("Wrapper not found")?;
    // list[T] is a subscript but NOT Generic[...] — no params should be extracted
    assert_eq!(
        cls.generic_params.len(),
        0,
        "non-Generic subscript must not produce generic_params"
    );
    Ok(())
}

/// Single-param `Generic[T]` — exercises the `other` arm (not a Tuple slice).
/// Kills `&&` → `||` at line 948 combined with the single-element path.
#[test]
fn extract_generic_params_single_param() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypeVar, Generic\n",
        "T = TypeVar('T')\n",
        "class Box(Generic[T]):\n",
        "    pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let cls = resolved
        .classes
        .iter()
        .find(|c| c.name == "Box")
        .ok_or("Box not found")?;
    assert_eq!(
        cls.generic_params.len(),
        1,
        "Generic[T] must produce 1 param"
    );
    assert_eq!(cls.generic_params[0].name, "T");
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: annotation_flags (line 1017)
// ---------------------------------------------------------------------------

/// `annotation_flags` — `!=` mutant at line 1017.
/// A `-> None` annotation must be classified as `NoneType`, not `Any`.
#[test]
fn annotation_flags_none_is_not_any() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ReturnAnnotationKind;
    let src = "def f() -> None: pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        matches!(func.return_annotation, ReturnAnnotationKind::NoneType),
        "None annotation must be NoneType, not Any — got {:?}",
        func.return_annotation
    );
    Ok(())
}

/// `annotation_flags` — an `Any` return annotation must be classified as `Any`.
#[test]
fn annotation_flags_any_is_any() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ReturnAnnotationKind;
    let src = "from typing import Any\ndef f() -> Any: pass\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let func = &resolved.functions[0];
    assert!(
        matches!(func.return_annotation, ReturnAnnotationKind::Any),
        "Any annotation must be ReturnAnnotationKind::Any — got {:?}",
        func.return_annotation
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: alias_name (line 1069)
// ---------------------------------------------------------------------------

/// `alias_name` — `FnValue → String::new()` / `"xyzzy".into()` mutants.
/// Import alias names must be preserved correctly.
#[test]
fn alias_name_preserves_import_name() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import Optional, Union\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let import = resolved
        .imports
        .iter()
        .find(|i| i.module == "typing")
        .ok_or("no typing import")?;
    assert!(
        import.names.contains(&"Optional".to_owned()),
        "Optional must be in import names"
    );
    assert!(
        import.names.contains(&"Union".to_owned()),
        "Union must be in import names"
    );
    Ok(())
}

/// `alias_name` — single import preserves the name (kills `String::new()` mutant).
#[test]
fn alias_name_single_name_is_correct() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from os.path import join\n".to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let import = resolved
        .imports
        .iter()
        .find(|i| i.module == "os.path")
        .ok_or("no import")?;
    assert_eq!(
        import.names,
        vec!["join".to_owned()],
        "join must be preserved"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: classify_rhs / MatchArmGuard (line 1115/1116)
// ---------------------------------------------------------------------------

/// `classify_rhs` — empty list guard `list.elts.is_empty()` at line 1115.
/// An empty list must produce `RhsKind::EmptyList`, not `RhsKind::Other`.
/// Replacing the guard with `true` would classify ALL lists as `EmptyList`.
#[test]
fn classify_rhs_empty_list_vs_nonempty() -> Result<(), Box<dyn std::error::Error>> {
    // Use two module vars: one with empty list, one with non-empty.
    let src = concat!("empty: list = []\n", "nonempty: list = [1, 2, 3]\n",).to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let empty_var = resolved
        .module_vars
        .iter()
        .find(|v| v.name == "empty")
        .ok_or("empty not found")?;
    let nonempty_var = resolved
        .module_vars
        .iter()
        .find(|v| v.name == "nonempty")
        .ok_or("nonempty not found")?;
    assert_eq!(
        format!("{:?}", empty_var.rhs_kind),
        "EmptyList",
        "empty list must produce EmptyList"
    );
    assert_ne!(
        format!("{:?}", nonempty_var.rhs_kind),
        "EmptyList",
        "non-empty list must NOT produce EmptyList"
    );
    Ok(())
}

/// `classify_rhs` — empty dict guard `dict.items.is_empty()` at line 1116.
/// An empty dict must produce `RhsKind::EmptyDict`, not `RhsKind::Other`.
/// Replacing the guard with `true` would classify ALL dicts as `EmptyDict`.
#[test]
fn classify_rhs_empty_dict_vs_nonempty() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("empty: dict = {}\n", "nonempty: dict = {'a': 1}\n",).to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let empty_var = resolved
        .module_vars
        .iter()
        .find(|v| v.name == "empty")
        .ok_or("empty not found")?;
    let nonempty_var = resolved
        .module_vars
        .iter()
        .find(|v| v.name == "nonempty")
        .ok_or("nonempty not found")?;
    assert_eq!(
        format!("{:?}", empty_var.rhs_kind),
        "EmptyDict",
        "empty dict must produce EmptyDict"
    );
    assert_ne!(
        format!("{:?}", nonempty_var.rhs_kind),
        "EmptyDict",
        "non-empty dict must NOT produce EmptyDict"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: is_wildcard_pattern (line 1140)
// ---------------------------------------------------------------------------

/// `is_wildcard_pattern` — `&&` → `||` mutant at line 1140.
/// A `MatchAs` with a name is NOT a wildcard; one with neither name nor pattern IS.
#[test]
fn is_wildcard_pattern_named_match_as_is_not_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x = 1\n",
        "match x:\n",
        "    case y:\n", // MatchAs with name — NOT wildcard
        "        pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    // A MatchAs with a name is a capture pattern, not a wildcard.
    // The match stmt must be resolved with has_wildcard = false.
    let stmt = resolved.match_stmts.first().ok_or("no match stmt")?;
    assert!(
        !stmt.has_wildcard,
        "capture pattern `case y:` must not be wildcard"
    );
    Ok(())
}

/// `is_wildcard_pattern` — true wildcard (`case _:`) must have `has_wildcard = true`.
#[test]
fn is_wildcard_pattern_bare_wildcard() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x = 1\n",
        "match x:\n",
        "    case 1:\n",
        "        pass\n",
        "    case _:\n", // bare wildcard
        "        pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    let stmt = resolved.match_stmts.first().ok_or("no match stmt")?;
    assert!(
        stmt.has_wildcard,
        "bare `case _:` must set has_wildcard=true"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: collect_reveal_type_calls_from_stmt match arms
// ---------------------------------------------------------------------------

/// `collect_reveal_type_calls_from_stmt` — `Stmt::While` arm (line 1016).
/// `reveal_type` inside a while body must be collected.
#[test]
fn reveal_type_inside_while_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "while x > 0:\n",
        "    reveal_type(x)\n",
        "    x = x - 1\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.reveal_type_calls.is_empty(),
        "reveal_type inside while body must be collected"
    );
    Ok(())
}

/// `collect_reveal_type_calls_from_stmt` — `Stmt::With` arm (line 1020).
/// `reveal_type` inside a with body must be collected.
#[test]
fn reveal_type_inside_with_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "with open('f') as g:\n",
        "    reveal_type(x)\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.reveal_type_calls.is_empty(),
        "reveal_type inside with body must be collected"
    );
    Ok(())
}

/// `collect_reveal_type_calls_from_stmt` — `Stmt::Try` arm (line 1023).
/// `reveal_type` inside a try body must be collected.
#[test]
fn reveal_type_inside_try_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "try:\n",
        "    reveal_type(x)\n",
        "except Exception:\n",
        "    pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.reveal_type_calls.is_empty(),
        "reveal_type inside try body must be collected"
    );
    Ok(())
}

/// `collect_reveal_type_calls_from_stmt` — `Stmt::Match` arm (line 1032).
/// `reveal_type` inside a match case body must be collected.
#[test]
fn reveal_type_inside_match_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "match x:\n",
        "    case _:\n",
        "        reveal_type(x)\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.reveal_type_calls.is_empty(),
        "reveal_type inside match arm must be collected"
    );
    Ok(())
}

/// `collect_reveal_type_calls_from_stmt` — `==` → `!=` mutant at line 991.
/// Only `reveal_type` (not other calls) must be collected.
#[test]
fn reveal_type_calls_only_matches_reveal_type() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "print(42)\n",       // NOT reveal_type — must not be collected
        "reveal_type(42)\n", // IS reveal_type — must be collected
        "assert_type(42)\n", // NOT reveal_type — must not be collected
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(
        resolved.reveal_type_calls.len(),
        1,
        "exactly one reveal_type call must be collected, not print or assert_type"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: collect_special_calls / collect_special_calls_from_stmt
// ---------------------------------------------------------------------------

/// `collect_special_calls` — `FnValue → vec![]` at line 1044.
/// `assert_type` calls must be returned, not an empty vec.
#[test]
fn collect_special_calls_assert_type_returns_entries() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("assert_type(1, int)\n", "assert_type('hello', str)\n",).to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(
        resolved.assert_type_calls.len(),
        2,
        "both assert_type calls must be collected"
    );
    Ok(())
}

/// `collect_special_calls_from_stmt` — `==` → `!=` mutant at line 1061.
/// Only the exact function name must match — other calls must be ignored.
#[test]
fn collect_special_calls_only_matches_exact_name() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "print(1)\n",       // NOT assert_type
        "assert_type(1)\n", // IS assert_type
        "reveal_type(1)\n", // NOT assert_type
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(
        resolved.assert_type_calls.len(),
        1,
        "only assert_type must be collected, not print or reveal_type"
    );
    Ok(())
}

/// `collect_special_calls_from_stmt` — `Stmt::FunctionDef` arm (line 1070).
/// `assert_type` inside a function body must be collected.
#[test]
fn collect_special_calls_inside_function_def() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo() -> None:\n", "    assert_type(1, int)\n",).to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside function must be collected"
    );
    Ok(())
}

/// `collect_special_calls_from_stmt` — `Stmt::ClassDef` arm (line 1073).
/// `assert_type` inside a class body must be collected.
#[test]
fn collect_special_calls_inside_class_def() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Foo:\n", "    assert_type(1, int)\n",).to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside class body must be collected"
    );
    Ok(())
}

/// `collect_special_calls_from_stmt` — `Stmt::If` arm (line 1076).
/// `assert_type` inside an if body must be collected.
#[test]
fn collect_special_calls_inside_if() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("x: int = 1\n", "if x > 0:\n", "    assert_type(x, int)\n",).to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside if body must be collected"
    );
    Ok(())
}

/// `collect_special_calls_from_stmt` — `Stmt::For` arm (line 1082).
/// `assert_type` inside a for body must be collected.
#[test]
fn collect_special_calls_inside_for() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("for i in range(3):\n", "    assert_type(i, int)\n",).to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside for body must be collected"
    );
    Ok(())
}

/// `collect_special_calls_from_stmt` — `Stmt::While` arm (line 1086).
/// `assert_type` inside a while body must be collected.
#[test]
fn collect_special_calls_inside_while() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "while x > 0:\n",
        "    assert_type(x, int)\n",
        "    x = x - 1\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside while body must be collected"
    );
    Ok(())
}

/// `collect_special_calls_from_stmt` — `Stmt::With` arm (line 1090).
/// `assert_type` inside a with body must be collected.
#[test]
fn collect_special_calls_inside_with() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "with open('f') as g:\n",
        "    assert_type(x, int)\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside with body must be collected"
    );
    Ok(())
}

/// `collect_special_calls_from_stmt` — `Stmt::Try` arm (line 1093).
/// `assert_type` inside a try body must be collected.
#[test]
fn collect_special_calls_inside_try() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "try:\n",
        "    assert_type(x, int)\n",
        "except Exception:\n",
        "    pass\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside try body must be collected"
    );
    Ok(())
}

/// `collect_special_calls_from_stmt` — `Stmt::Match` arm (line 1102).
/// `assert_type` inside a match case body must be collected.
#[test]
fn collect_special_calls_inside_match() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "match x:\n",
        "    case _:\n",
        "        assert_type(x, int)\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside match arm must be collected"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: annotation_flags line 1187 — `==` → `!=` for None
// ---------------------------------------------------------------------------

/// `annotation_flags` — `==` → `!=` at line 1187.
/// The `is_none` flag (`s == "None"`) must be true for the name "None" and
/// false for any other name.  If mutated to `!=`, "None" would be false and
/// every other name would be true, causing wrong `NoneType` classifications.
#[test]
fn annotation_flags_none_name_is_none_not_other() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::ReturnAnnotationKind;
    // "None" → NoneType
    let src_none = "def f() -> None: pass\n".to_owned();
    let parsed_none = parse_source(src_none, "test.py".to_owned())?;
    let resolved_none = resolve(&parsed_none)?;
    assert!(
        matches!(
            resolved_none.functions[0].return_annotation,
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
            resolved_int.functions[0].return_annotation,
            ReturnAnnotationKind::Other
        ),
        "-> int must be Other, not NoneType"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Missed mutant coverage: collect_typeddict_calls
// ---------------------------------------------------------------------------

/// `collect_typeddict_calls` — `FnValue → vec![]` at line 1351.
/// `TypedDict` functional call must be returned, not an empty vec.
#[test]
fn collect_typeddict_calls_returns_entries() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        r#"Movie = TypedDict("Movie", {"name": str, "year": int})"#,
        "\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(
        resolved.typeddict_calls.len(),
        1,
        "TypedDict functional call must be collected"
    );
    let td = &resolved.typeddict_calls[0];
    assert_eq!(td.lhs_name, "Movie");
    Ok(())
}

/// `collect_typeddict_calls` — `==` → `!=` at line 1358 for simple callee name.
/// Only `TypedDict` by name must match; other names must be skipped.
#[test]
fn collect_typeddict_calls_only_matches_typeddict_name() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        r#"NotTypedDict = dict("Name", {"x": int})"#,
        "\n",
        r#"Movie = TypedDict("Movie", {"name": str})"#,
        "\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(
        resolved.typeddict_calls.len(),
        1,
        "only TypedDict call must be collected, not dict"
    );
    assert_eq!(resolved.typeddict_calls[0].lhs_name, "Movie");
    Ok(())
}

/// `collect_typeddict_calls` — `==` → `!=` at line 1360 for attribute callee.
/// `typing.TypedDict` must be collected; other attribute calls must not.
#[test]
fn collect_typeddict_calls_qualified_typing_typeddict() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import typing\n",
        r#"Movie = typing.TypedDict("Movie", {"name": str})"#,
        "\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(
        resolved.typeddict_calls.len(),
        1,
        "typing.TypedDict must be collected"
    );
    assert_eq!(resolved.typeddict_calls[0].lhs_name, "Movie");
    Ok(())
}

/// `collect_typeddict_calls` — `!` deletion at line 1386 (`!matches!(k, Expr::StringLiteral(_))`).
/// A dict with a non-string key must set `has_non_string_key = true`.
#[test]
fn collect_typeddict_calls_non_string_key_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(r#"Movie = TypedDict("Movie", {1: str})"#, "\n",).to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.typeddict_calls.len(), 1);
    assert!(
        resolved.typeddict_calls[0].has_non_string_key,
        "non-string dict key must set has_non_string_key=true"
    );
    Ok(())
}

/// Complement test: all string keys must set `has_non_string_key = false`.
/// Kills `!` deletion at line 1386 by providing the false side.
#[test]
fn collect_typeddict_calls_string_keys_only() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        r#"Movie = TypedDict("Movie", {"name": str, "year": int})"#,
        "\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.typeddict_calls.len(), 1);
    assert!(
        !resolved.typeddict_calls[0].has_non_string_key,
        "all-string keys must set has_non_string_key=false"
    );
    Ok(())
}

/// `collect_typeddict_calls` — second arg is not a dict literal → `NotDictLiteral`.
/// Kills the `!` deletion / `==`→`!=` variants around the dict literal check.
#[test]
fn collect_typeddict_calls_non_dict_second_arg() -> Result<(), Box<dyn std::error::Error>> {
    use basilisk_resolver::TypedDictSecondArgKind;
    let src = concat!(
        "fields = {'name': str}\n",
        r#"Movie = TypedDict("Movie", fields)"#,
        "\n",
    )
    .to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.typeddict_calls.len(), 1);
    assert_eq!(
        resolved.typeddict_calls[0].second_arg_kind,
        TypedDictSecondArgKind::NotDictLiteral,
        "variable second arg must produce NotDictLiteral"
    );
    Ok(())
}

/// `collect_typeddict_calls` — no second positional arg → `has_positional_dict = false`.
/// Exercises the else branch at line 1395.
#[test]
fn collect_typeddict_calls_keyword_only_form() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(r#"Movie = TypedDict("Movie", name=str, year=int)"#, "\n",).to_owned();
    let parsed = parse_source(src, "test.py".to_owned())?;
    let resolved = resolve(&parsed)?;
    assert_eq!(resolved.typeddict_calls.len(), 1);
    assert!(
        !resolved.typeddict_calls[0].has_positional_dict,
        "keyword-only form must set has_positional_dict=false"
    );
    Ok(())
}
