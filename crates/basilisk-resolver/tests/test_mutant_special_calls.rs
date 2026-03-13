mod common;

use common::{resolve_src};

#[test]
fn collect_special_calls_assert_type_returns_entries() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("assert_type(1, int)\n", "assert_type('hello', str)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(
        resolved.assert_type_calls.len(),
        2,
        "both assert_type calls must be collected"
    );
    Ok(())
}

#[test]
fn collect_special_calls_only_matches_exact_name() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "print(1)\n",       // NOT assert_type
        "assert_type(1)\n", // IS assert_type
        "reveal_type(1)\n", // NOT assert_type
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(
        resolved.assert_type_calls.len(),
        1,
        "only assert_type must be collected, not print or reveal_type"
    );
    Ok(())
}

#[test]
fn collect_special_calls_inside_function_def() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo() -> None:\n", "    assert_type(1, int)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside function must be collected"
    );
    Ok(())
}

#[test]
fn collect_special_calls_inside_class_def() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("class Foo:\n", "    assert_type(1, int)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside class body must be collected"
    );
    Ok(())
}

#[test]
fn collect_special_calls_inside_if() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("x: int = 1\n", "if x > 0:\n", "    assert_type(x, int)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside if body must be collected"
    );
    Ok(())
}

#[test]
fn collect_special_calls_inside_for() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("for i in range(3):\n", "    assert_type(i, int)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside for body must be collected"
    );
    Ok(())
}

#[test]
fn collect_special_calls_inside_while() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "while x > 0:\n",
        "    assert_type(x, int)\n",
        "    x = x - 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside while body must be collected"
    );
    Ok(())
}

#[test]
fn collect_special_calls_inside_with() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "with open('f') as g:\n",
        "    assert_type(x, int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside with body must be collected"
    );
    Ok(())
}

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
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside try body must be collected"
    );
    Ok(())
}

#[test]
fn collect_special_calls_inside_match() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "match x:\n",
        "    case _:\n",
        "        assert_type(x, int)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.assert_type_calls.is_empty(),
        "assert_type inside match arm must be collected"
    );
    Ok(())
}
