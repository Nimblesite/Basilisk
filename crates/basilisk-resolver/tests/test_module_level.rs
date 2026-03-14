//! Tests for resolver: `test_module_level`.

mod common;

use common::resolve_src;

#[test]
fn module_order_comparison_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = "a = 1\nb = 2\na < b\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_order_comparisons.len(), 1);
    assert_eq!(resolved.module_order_comparisons[0].left_name, "a");
    assert_eq!(resolved.module_order_comparisons[0].right_name, "b");
    Ok(())
}

#[test]
fn module_order_comparison_gte() -> Result<(), Box<dyn std::error::Error>> {
    let src = "a = 1\nb = 2\na >= b\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_order_comparisons.len(), 1);
    Ok(())
}

#[test]
fn module_order_comparison_in_if() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("a = 1\n", "b = 2\n", "if a < b:\n", "    pass\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_order_comparisons.len(), 1);
    Ok(())
}

#[test]
fn module_order_comparison_gt() -> Result<(), Box<dyn std::error::Error>> {
    let src = "a = 1\nb = 2\na > b\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_order_comparisons.len(), 1);
    Ok(())
}

#[test]
fn module_order_comparison_lte() -> Result<(), Box<dyn std::error::Error>> {
    let src = "a = 1\nb = 2\na <= b\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_order_comparisons.len(), 1);
    Ok(())
}

#[test]
fn module_bare_assignment_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x = 42\ny = 'hello'\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_bare_assignments.len(), 2);
    let names: Vec<&str> = resolved
        .module_bare_assignments
        .iter()
        .map(|a| a.name.as_str())
        .collect();
    assert!(names.contains(&"x"));
    assert!(names.contains(&"y"));
    Ok(())
}

#[test]
fn module_attr_assignment_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Foo:\n", "    pass\n", "Foo.x = 42\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.module_attr_assignments.len(), 1);
    assert_eq!(resolved.module_attr_assignments[0].object_name, "Foo");
    assert_eq!(resolved.module_attr_assignments[0].attr_name, "x");
    Ok(())
}

#[test]
fn module_attr_access_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Foo:\n", "    x: int = 1\n", "Foo.x\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.module_attr_accesses.is_empty());
    Ok(())
}

#[test]
fn module_level_calls_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = "print('hello')\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.calls.is_empty());
    Ok(())
}

#[test]
fn module_order_comparison_eq_not_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = "a = 1\nb = 2\na == b\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        resolved.module_order_comparisons.is_empty(),
        "== is not an ordering comparison"
    );
    Ok(())
}

#[test]
fn module_var_inside_if_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("if True:\n", "    x: int = 5\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.module_vars.iter().any(|v| v.name == "x"));
    Ok(())
}

#[test]
fn module_var_inside_try_except() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "try:\n",
        "    x: int = 5\n",
        "except:\n",
        "    y: int = 6\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    // try/except is NOT module-level for var collection; just verify no crash
    let _ = &resolved.module_vars;
    Ok(())
}

#[test]
fn module_order_comparison_simple_lt() -> Result<(), Box<dyn std::error::Error>> {
    // The order comparison collector requires Name on both sides.
    let src = "x = 1\ny = 2\nx < y\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.module_order_comparisons.is_empty());
    Ok(())
}

#[test]
fn module_attr_access_bare_expr() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("import os\n", "os.path\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.module_attr_accesses.is_empty());
    Ok(())
}
