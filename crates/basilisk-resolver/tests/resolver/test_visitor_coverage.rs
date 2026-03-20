// Tests for resolver: `test_visitor_coverage`.

use super::common::resolve_src;

#[test]
fn dict_spread_item_does_not_crash() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    other = {'b': 2}\n",
        "    d = {**other, [1]: 'val'}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let func = &resolved.functions[0];
    // The [1] key is unhashable; the spread item (**other) has no key.
    assert!(
        !func.unhashable_keys.is_empty(),
        "list key in dict must be detected"
    );
    Ok(())
}

#[test]
fn module_level_ann_assign_without_value() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x: int\n".to_owned();
    let resolved = resolve_src(&src)?;
    // AnnAssign with no value: the important thing is this parses and resolves without crashing.
    let _ = &resolved.module_vars;
    Ok(())
}

#[test]
fn class_with_docstring_not_collected_as_attribute() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo:\n",
        "    \"\"\"A docstring.\"\"\"\n",
        "    x: int = 0\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
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

#[test]
fn module_level_method_call_not_collected_as_call_site() -> Result<(), Box<dyn std::error::Error>> {
    let src = "result = obj.method(42)\n".to_owned();
    let resolved = resolve_src(&src)?;
    // obj.method is an Attribute, not a simple Name → call_site_from_expr returns None
    assert!(
        resolved.calls.is_empty(),
        "method call must not be collected as a call site"
    );
    Ok(())
}

#[test]
fn module_level_ann_assign_with_attribute_target_not_collected(
) -> Result<(), Box<dyn std::error::Error>> {
    let src = "x.y: int = 0\n".to_owned();
    let resolved = resolve_src(&src)?;
    // x.y is an Attribute target → expr_simple_name returns None → no VariableInfo created
    assert!(
        resolved.module_vars.is_empty(),
        "attribute target must not be collected as a module var"
    );
    Ok(())
}

#[test]
fn class_body_attribute_targets_skipped() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "class Foo:\n",
        "    x.y: int = 0\n", // AnnAssign with Attribute target → line 322 None branch
        "    a.b = 0\n",      // Assign with Attribute target → line 333 None branch
        "    name: str = 'ok'\n", // regular AnnAssign — should be collected
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
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

#[test]
fn function_body_attribute_ann_assign_and_bare_return() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    x.y: int = 0\n", // AnnAssign with Attribute target (collect_all_assigns None, line 539)
        "    z: int\n",       // AnnAssign without value (collect_unhashable_keys None, line 676)
        "    return\n",       // bare return (collect_unhashable_keys None, line 681)
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.functions.len(), 1);
    // x.y is not a simple name → must not appear in local assigns
    let func = &resolved.functions[0];
    assert!(
        !func.all_local_assigns.contains(&"y".to_owned()),
        "attribute target must not be collected as a local assign"
    );
    Ok(())
}

#[test]
fn function_body_with_clause_without_as() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def foo() -> None:\n",
        "    with open('f'):\n", // `with` without `as` → optional_vars is None
        "        pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.functions.len(), 1);
    Ok(())
}

#[test]
fn module_level_bare_expression_not_collected_as_call() -> Result<(), Box<dyn std::error::Error>> {
    let src = "42\n".to_owned(); // Stmt::Expr with NumberLiteral value — not a Call
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.calls.is_empty(),
        "bare integer expression must not produce a call site"
    );
    Ok(())
}

#[test]
fn attribute_decorator_name_extracted() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "import abc\n",
        "class Base:\n",
        "    @abc.abstractmethod\n", // Attribute decorator → line 984
        "    def foo(self) -> None: pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
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

#[test]
fn call_name_decorator_name_extracted() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def deprecated(msg): pass\n",
        "@deprecated('use new_foo instead')\n", // Call(func=Name("deprecated")) → line 986
        "def foo() -> None: pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
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

#[test]
fn call_exotic_func_decorator_returns_none() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def factory(): pass\n",
        "@factory()()\n", // Call(func=Call(func=Name("factory"))) → line 988
        "def foo() -> None: pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
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

#[test]
fn subscript_decorator_returns_none() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "buttons = [lambda f: f]\n",
        "@buttons[0]\n", // Subscript expression → outer `_ => None` (line 990)
        "def foo() -> None: pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
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

#[test]
fn collect_return_name_refs_inside_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "def first(items: list) -> object:\n",
        "    for item in items:\n",
        "        return item\n",
        "    return items\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
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
