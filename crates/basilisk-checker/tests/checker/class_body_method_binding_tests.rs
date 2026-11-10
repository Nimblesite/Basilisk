//! Tests for [TYPEINF-ANNOTATION-RESOLUTION] method binding of class-body
//! function assignments. See
//! docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
//
// A function assigned in a class body (`m = f`) is a method like any `def`:
// instance access binds the receiver, class access does not, and
// `staticmethod` / `classmethod` wrappers shift which parameter the binding
// consumes ([#382](https://github.com/Nimblesite/Basilisk/issues/382)). This is
// Python descriptor semantics consumed by the [PEP 484 callable model](https://peps.python.org/pep-0484/#callable).
// These tests pin the assigned spelling to the exact
// diagnostics the equivalent `def` in the class body draws.

use super::common::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The missing-argument diagnostics drawn by `source`.
fn arity_errors(source: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let diags = run(source)?;
    Ok(diags
        .iter()
        .filter(|d| d.message.contains("required argument"))
        .map(|d| d.message.clone())
        .collect())
}

/// A module where `C.m` is `f` assigned in the class body, next to the
/// equivalent literal `def` method `n` that serves as the behaviour baseline.
const ASSIGNED: &str = "def f(self: \"C\", a: int) -> None:\n    return None\n\n\
class C:\n    m = f\n\n    def n(self, a: int) -> None:\n        return None\n";

#[test]
fn instance_access_binds_receiver_on_assigned_method() -> TestResult {
    let errors = arity_errors(&format!("{ASSIGNED}\nC().m(1)\n"))?;
    assert!(
        errors.is_empty(),
        "instance access consumes `self`, so `C().m(1)` is complete (#382), got: {errors:?}"
    );
    Ok(())
}

#[test]
fn class_access_leaves_assigned_method_unbound() -> TestResult {
    let baseline = arity_errors(&format!("{ASSIGNED}\nC.n(1)\n"))?;
    assert!(
        !baseline.is_empty(),
        "baseline: class access to the literal `def` must be an arity error \
         (self=1, `a` missing) for the assigned spelling to be pinned against"
    );
    let errors = arity_errors(&format!("{ASSIGNED}\nC.m(1)\n"))?;
    assert!(
        !errors.is_empty(),
        "class access does not bind `self`, so `C.m(1)` misses `a` exactly \
         like `C.n(1)` does (#382)"
    );
    Ok(())
}

#[test]
fn staticmethod_wrapper_never_consumes_receiver() -> TestResult {
    let source = "def g(a: int) -> None:\n    return None\n\n\
class D:\n    s = staticmethod(g)\n\n\
D().s(1)\nD.s(1)\n";
    let errors = arity_errors(source)?;
    assert!(
        errors.is_empty(),
        "`staticmethod` takes no receiver on either access path (#382), got: {errors:?}"
    );
    Ok(())
}

#[test]
fn classmethod_wrapper_consumes_cls_on_both_access_paths() -> TestResult {
    let source = "def h(cls: type, a: int) -> None:\n    return None\n\n\
class E:\n    c = classmethod(h)\n\n\
E().c(1)\nE.c(1)\n";
    let errors = arity_errors(source)?;
    assert!(
        errors.is_empty(),
        "`classmethod` binds `cls` on instance AND class access (#382), got: {errors:?}"
    );
    Ok(())
}

#[test]
fn assigned_method_still_checks_missing_arguments_on_instance() -> TestResult {
    let errors = arity_errors(&format!("{ASSIGNED}\nC().m()\n"))?;
    assert!(
        !errors.is_empty(),
        "binding `self` must not silence real arity errors: `C().m()` still misses `a` (#382)"
    );
    Ok(())
}
