//! Tests for [RESOLV-CANONICAL-BINDING]: class-body descriptor calls
//! (`name = staticmethod(f)`, `name = classmethod(f)`) are recognised from
//! resolved binding identity, never from the callee's spelling.
//!
//! Pins the 2026-08-08 review finding against
//! `src/visitor/class_info.rs::rhs_callable_binding`, which compared the
//! callee's raw identifier text: an aliased `from builtins import
//! staticmethod as hidden` was invisible, and a module-local `def
//! staticmethod` shadow was wrongly recognised as the builtin descriptor.

use super::common::resolve_src;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The `(rhs_descriptor present, rhs_name)` of attribute `attr` on class
/// `class_name` after resolving `src`.
fn descriptor_binding(
    src: &str,
    class_name: &str,
    attr: &str,
) -> Result<(bool, Option<String>), Box<dyn std::error::Error>> {
    let resolved = resolve_src(src)?;
    let class = resolved
        .classes
        .iter()
        .find(|c| c.name == class_name)
        .ok_or_else(|| format!("no class `{class_name}` resolved"))?;
    let attribute = class
        .attributes
        .iter()
        .find(|a| a.name == attr)
        .ok_or_else(|| format!("no attribute `{attr}` on `{class_name}`"))?;
    Ok((attribute.rhs_descriptor.is_some(), attribute.rhs_name.clone()))
}

/// An aliased import of the builtin descriptor is the SAME descriptor: the
/// wrapper must be recognised and the wrapped callable's name carried.
#[test]
fn aliased_staticmethod_call_is_a_descriptor() -> TestResult {
    let src = concat!(
        "from builtins import staticmethod as hidden\n",
        "\n",
        "def free(x: int) -> int: ...\n",
        "\n",
        "class Widget:\n",
        "    handler = hidden(free)\n",
    );
    let (has_descriptor, rhs_name) = descriptor_binding(src, "Widget", "handler")?;
    assert!(
        has_descriptor,
        "`hidden` resolves to builtins.staticmethod; the descriptor must be recognised"
    );
    assert_eq!(rhs_name.as_deref(), Some("free"));
    Ok(())
}

/// A module-local `def staticmethod` shadows the builtin; a call to it is an
/// ordinary call, never the descriptor.
#[test]
fn shadowed_staticmethod_call_is_not_a_descriptor() -> TestResult {
    let src = concat!(
        "def staticmethod(func): ...\n",
        "\n",
        "def free(x: int) -> int: ...\n",
        "\n",
        "class Widget:\n",
        "    handler = staticmethod(free)\n",
    );
    let (has_descriptor, _) = descriptor_binding(src, "Widget", "handler")?;
    assert!(
        !has_descriptor,
        "a module-local `staticmethod` shadow is not the builtin descriptor"
    );
    Ok(())
}

/// Regression guard: the bare unshadowed builtins keep resolving.
#[test]
fn bare_builtin_descriptor_calls_still_resolve() -> TestResult {
    let src = concat!(
        "def free(x: int) -> int: ...\n",
        "\n",
        "class Widget:\n",
        "    stat = staticmethod(free)\n",
        "    cls_ = classmethod(free)\n",
        "    plain = free\n",
    );
    let (stat_descriptor, stat_name) = descriptor_binding(src, "Widget", "stat")?;
    assert!(stat_descriptor);
    assert_eq!(stat_name.as_deref(), Some("free"));
    let (cls_descriptor, cls_name) = descriptor_binding(src, "Widget", "cls_")?;
    assert!(cls_descriptor);
    assert_eq!(cls_name.as_deref(), Some("free"));
    let (plain_descriptor, plain_name) = descriptor_binding(src, "Widget", "plain")?;
    assert!(!plain_descriptor, "a bare name binds no descriptor wrapper");
    assert_eq!(plain_name.as_deref(), Some("free"));
    Ok(())
}
