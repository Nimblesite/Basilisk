// Tests for resolver: `test_typeddict_calls`.

use super::common::resolve_src;

#[test]
fn typeddict_total_false_detected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict, total=False):\n",
        "    name: str\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    let cls = resolved.classes.iter().find(|c| c.name == "Movie");
    assert!(cls.is_some());
    assert!(!cls.is_none_or(|c| c.is_typeddict_total));
    Ok(())
}

#[test]
fn typeddict_regular_assign_full_check() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "class Movie(TypedDict):\n",
        "    name: str\n",
        "m: Movie = {'name': 'x'}\n",
        "m = {'name': 42}\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.typeddict_key_violations.is_empty());
    Ok(())
}

#[test]
fn typeddict_functional_call_collected() -> Result<(), Box<dyn std::error::Error>> {
    let src = concat!(
        "from typing import TypedDict\n",
        "Movie = TypedDict('Movie', {'name': str, 'year': int})\n",
    )
    .to_owned();
    let resolved = resolve_src(&src)?;
    assert!(!resolved.typeddict_calls.is_empty());
    Ok(())
}
