mod common;

use common::{resolve_src};

#[test]
fn reveal_type_inside_while_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "while x > 0:\n",
        "    reveal_type(x)\n",
        "    x = x - 1\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.reveal_type_calls.is_empty(),
        "reveal_type inside while body must be collected"
    );
    Ok(())
}

#[test]
fn reveal_type_inside_with_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "with open('f') as g:\n",
        "    reveal_type(x)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.reveal_type_calls.is_empty(),
        "reveal_type inside with body must be collected"
    );
    Ok(())
}

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
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.reveal_type_calls.is_empty(),
        "reveal_type inside try body must be collected"
    );
    Ok(())
}

#[test]
fn reveal_type_inside_match_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x: int = 1\n",
        "match x:\n",
        "    case _:\n",
        "        reveal_type(x)\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(
        !resolved.reveal_type_calls.is_empty(),
        "reveal_type inside match arm must be collected"
    );
    Ok(())
}

#[test]
fn reveal_type_calls_only_matches_reveal_type() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "print(42)\n",       // NOT reveal_type — must not be collected
        "reveal_type(42)\n", // IS reveal_type — must be collected
        "assert_type(42)\n", // NOT reveal_type — must not be collected
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(
        resolved.reveal_type_calls.len(),
        1,
        "exactly one reveal_type call must be collected, not print or assert_type"
    );
    Ok(())
}

#[test]
fn reveal_type_call_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = "x = 42\nreveal_type(x)\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert_eq!(resolved.reveal_type_calls.len(), 1);
    Ok(())
}

#[test]
fn reveal_type_calls_in_function() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("def foo(x: int) -> None:\n", "    reveal_type(x)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.reveal_type_calls.is_empty());
    Ok(())
}

#[test]
fn reveal_type_calls_in_try_except() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x = 5\n",
        "try:\n",
        "    reveal_type(x)\n",
        "except:\n",
        "    pass\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.reveal_type_calls.is_empty());
    Ok(())
}

#[test]
fn reveal_type_calls_in_while_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "x = 5\n",
        "while True:\n",
        "    reveal_type(x)\n",
        "    break\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.reveal_type_calls.is_empty());
    Ok(())
}

#[test]
fn reveal_type_calls_in_for_loop() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("for x in [1, 2, 3]:\n", "    reveal_type(x)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.reveal_type_calls.is_empty());
    Ok(())
}

#[test]
fn reveal_type_calls_in_with_block() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!("with open('f') as fh:\n", "    reveal_type(fh)\n",).to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.reveal_type_calls.is_empty());
    Ok(())
}
