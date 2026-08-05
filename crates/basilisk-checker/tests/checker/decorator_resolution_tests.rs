//! Tests for [TYPEINF-ANNOTATION-RESOLUTION] decorator resolution. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-ANNOTATION-RESOLUTION
//
// A decorator is a *name*, and whether `@ov` means `typing.overload` is the
// same binding question an annotation asks — answered by the resolver's
// import map plus its value-binding pass, never by matching the spelling
// (Refs #380). Observed through `overloads_definitions`: an `@overload`
// chain with NO implementation fires exactly when the decorator truly is
// `typing.overload`.

use super::common::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The decorator spelling denotes `typing.overload`, so the impl-less chain
/// draws `overloads_definitions`.
fn assert_recognised(source: &str, why: &str) -> TestResult {
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"overloads_definitions"),
        "{why}, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// The decorator spelling does NOT denote `typing.overload`, so no overload
/// rule may fire.
fn assert_not_overload(source: &str, why: &str) -> TestResult {
    let diags = run(source)?;
    assert!(
        !codes(&diags)
            .iter()
            .any(|code| code.starts_with("overloads_")),
        "{why}, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

/// A complete chain (two overloads + implementation) under this spelling is
/// accepted — zero diagnostics of any kind.
fn assert_chain_accepted(source: &str, why: &str) -> TestResult {
    let diags = run(source)?;
    assert!(diags.is_empty(), "{why}, got: {:?}", codes(&diags));
    Ok(())
}

// ---------------------------------------------------------------------------
// The four spellings the binding table must resolve
// ---------------------------------------------------------------------------

#[test]
fn bare_overload_import_is_recognised() -> TestResult {
    assert_recognised(
        "from typing import overload\n\n@overload\ndef f(a: int) -> int: ...\n@overload\ndef f(a: str) -> str: ...\n",
        "`from typing import overload` + `@overload` is typing.overload; an impl-less chain must fire",
    )
}

#[test]
fn aliased_overload_import_is_recognised() -> TestResult {
    assert_recognised(
        "from typing import overload as ov\n\n@ov\ndef f(a: int) -> int: ...\n@ov\ndef f(a: str) -> str: ...\n",
        "`from typing import overload as ov` binds `ov` to typing.overload (#380)",
    )
}

#[test]
fn typing_attribute_overload_is_recognised() -> TestResult {
    assert_recognised(
        "import typing\n\n@typing.overload\ndef f(a: int) -> int: ...\n@typing.overload\ndef f(a: str) -> str: ...\n",
        "`@typing.overload` is the attribute spelling of typing.overload",
    )
}

#[test]
fn aliased_module_attribute_overload_is_recognised() -> TestResult {
    assert_recognised(
        "import typing as t\n\n@t.overload\ndef f(a: int) -> int: ...\n@t.overload\ndef f(a: str) -> str: ...\n",
        "`import typing as t` makes `@t.overload` the same decorator",
    )
}

#[test]
fn value_bound_overload_is_recognised() -> TestResult {
    // The value-binding pass: `o = overload` re-binds the SAME function
    // object, so `@o` is `@overload` to the type system.
    assert_recognised(
        "from typing import overload\n\no = overload\n\n@o\ndef f(a: int) -> int: ...\n@o\ndef f(a: str) -> str: ...\n",
        "`o = overload` binds `o` to typing.overload (#380)",
    )
}

#[test]
fn value_bound_attribute_overload_is_recognised() -> TestResult {
    assert_recognised(
        "import typing\n\no = typing.overload\n\n@o\ndef f(a: int) -> int: ...\n@o\ndef f(a: str) -> str: ...\n",
        "`o = typing.overload` resolves through the value chain to typing.overload",
    )
}

// ---------------------------------------------------------------------------
// Accepted chains — the same spellings with an implementation are clean
// ---------------------------------------------------------------------------

#[test]
fn aliased_overload_chain_with_impl_is_accepted() -> TestResult {
    assert_chain_accepted(
        "from typing import overload as ov\n\n@ov\ndef f(a: int) -> int: ...\n@ov\ndef f(a: str) -> str: ...\ndef f(a: int | str) -> int | str:\n    return a\n",
        "a complete `@ov` chain is a valid overload group",
    )
}

#[test]
fn value_bound_overload_chain_with_impl_is_accepted() -> TestResult {
    assert_chain_accepted(
        "from typing import overload\n\no = overload\n\n@o\ndef f(a: int) -> int: ...\n@o\ndef f(a: str) -> str: ...\ndef f(a: int | str) -> int | str:\n    return a\n",
        "a complete `@o` chain (o = overload) is a valid overload group",
    )
}

// ---------------------------------------------------------------------------
// Discrimination — a decorator merely NAMED overload is not typing.overload
// ---------------------------------------------------------------------------

#[test]
fn foreign_overload_import_is_not_typing_overload() -> TestResult {
    // `from mymod import overload` binds SOME callable that happens to share
    // the name. Treating it as typing.overload invents an overload group —
    // and an "incomplete chain" error — out of spec-valid code.
    assert_not_overload(
        "from mymod import overload\n\n@overload\ndef f(a: int) -> int: ...\n@overload\ndef f(a: str) -> str: ...\n",
        "a foreign decorator named `overload` must not form an overload group (#380)",
    )
}

#[test]
fn foreign_module_attribute_overload_is_not_typing_overload() -> TestResult {
    assert_not_overload(
        "import mymod as t\n\n@t.overload\ndef f(a: int) -> int: ...\n@t.overload\ndef f(a: str) -> str: ...\n",
        "`t.overload` where `t` binds a foreign module is not typing.overload (#380)",
    )
}

#[test]
fn value_bound_foreign_overload_is_not_typing_overload() -> TestResult {
    assert_not_overload(
        "from mymod import overload\n\no = overload\n\n@o\ndef f(a: int) -> int: ...\n@o\ndef f(a: str) -> str: ...\n",
        "the value chain ends at a foreign name, so `@o` is not typing.overload",
    )
}
