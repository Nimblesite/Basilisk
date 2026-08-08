//! Tests for [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
// Tests for resolver: `test_decorators`.

use super::common::resolve_src;

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
    let resolved = resolve_src(&src)?;
    let bar = resolved.functions.iter().find(|f| f.name == "bar");
    assert!(bar.is_some(), "bar method must be resolved");
    let bar = bar.ok_or("bar not found")?;
    assert!(!bar.decorators.is_empty());
    Ok(())
}

#[test]
fn collects_decorator_with_plain_name() -> Result<(), Box<dyn std::error::Error>> {
    let src = "from typing import overload\n@overload\ndef foo(x: int) -> int: ...\n".to_owned();
    let resolved = resolve_src(&src)?;
    assert!(resolved.functions[0]
        .decorators
        .contains(&"overload".to_string()));
    Ok(())
}

#[test]
fn builtin_decorator_flags_do_not_depend_on_spelling() -> Result<(), Box<dyn std::error::Error>> {
    let bare = resolve_src(
        &"class C:\n    @staticmethod\n    def f() -> None: ...\n".to_owned(),
    )?;
    let aliased = resolve_src(
        &concat!(
            "from builtins import staticmethod as sm\n",
            "class C:\n",
            "    @sm\n",
            "    def f() -> None: ...\n",
        )
        .to_owned(),
    )?;
    let bare_method = bare.functions.iter().find(|function| function.name == "f");
    let aliased_method = aliased
        .functions
        .iter()
        .find(|function| function.name == "f");
    assert!(bare_method.is_some_and(|method| method.is_staticmethod));
    assert!(
        aliased_method.is_some_and(|method| method.is_staticmethod),
        "aliasing a builtin decorator must not change its resolved meaning"
    );
    Ok(())
}
