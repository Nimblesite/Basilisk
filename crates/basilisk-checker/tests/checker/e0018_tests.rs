//! Tests for [BSK-E0018] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for BSK-E0018: Undefined variable in return.

use super::common::*;

#[test]
fn e0018_undefined_name_in_return_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def compute() -> int:\n    return undefined_name\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0018"),
        "undefined name in return should fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0018_defined_param_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def compute(x: int) -> int:\n    return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "returning a parameter should not fire E0018"
    );
    Ok(())
}

#[test]
fn e0018_locally_assigned_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def compute() -> int:\n    result = 42\n    return result\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "returning a locally assigned variable should not fire E0018"
    );
    Ok(())
}

#[test]
fn e0018_module_level_variable_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
_EMPTY_TEXT_MSG = \"text is required\"

def validate(text: str) -> str:
    if not text:
        return _EMPTY_TEXT_MSG
    return text
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "returning a module-level variable should not fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0018_diagnostic_has_help() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def compute() -> int:\n    return missing\n";
    let diags = run(source)?;
    let e0018 = diags.iter().find(|d| d.code.code == "BSK-E0018");
    assert!(e0018.is_some(), "should fire E0018");
    let Some(diag) = e0018 else {
        return Err("E0018 diagnostic missing after assertion".into());
    };
    assert!(diag.help.is_some(), "E0018 should have help text");
    Ok(())
}

#[test]
fn e0018_aliased_module_import_in_return_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
    // Issues #107/#64: `from <pkg> import <mod> as <alias>` binds `<alias>` at
    // module scope; using it in a nested function's return expression is valid.
    let source = r#"
from unittest.mock import patch
from nap.api import auth as auth_mod

def _patch_jwt(claims: dict[str, object]) -> object:
    return patch.object(auth_mod, "_decode_supabase_jwt", return_value=claims)
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "aliased module import used in a return expression must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0018")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0018_undefined_callee_in_return_call_fires() -> Result<(), Box<dyn std::error::Error>> {
    // The callee of a call in a return must be checked, not just bare names:
    // `return undefined_fn()` references `undefined_fn`.
    let source = "def f() -> object:\n    return undefined_fn()\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"BSK-E0018"),
        "an undefined callee in a return call should fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0018_sibling_function_call_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // Calling a sibling module-level function must not fire — it is in scope.
    let source = "def helper() -> int:\n    return 1\n\n\ndef use() -> int:\n    return helper()\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "`return helper()` for a module-level function must not fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0018_bare_sibling_function_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // Returning a sibling module-level function by name is valid (it IS defined).
    let source =
        "def helper() -> int:\n    return 1\n\n\ndef use() -> object:\n    return helper\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "`return helper` for a module-level function must not fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0018_class_instantiation_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // Instantiating a module-level class must not fire.
    let source = "class Foo:\n    pass\n\n\ndef make() -> Foo:\n    return Foo()\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "`return Foo()` for a module-level class must not fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0018_builtin_call_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // A builtin callee must not fire.
    let source = "def f() -> int:\n    return len([1, 2])\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "`return len(...)` (builtin callee) must not fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn e0018_function_local_import_in_nested_class_method_no_false_positive(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #172: a function-local import is in scope for the body of a method
    // defined on a class nested in that SAME function. Python resolves the free
    // variable through the enclosing function scope (LEGB skips the class scope
    // for method bodies, but NOT the enclosing function), so this code is valid
    // and runs without NameError — it must not fire E0018.
    let source = r#"
from __future__ import annotations


def make_store() -> object:
    from collections import OrderedDict

    class _Stub:
        def put(self, body: bytes) -> OrderedDict:
            return OrderedDict(size=len(body))

    return _Stub()
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "function-local import used in a nested class method must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0018")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0018_function_local_import_returned_bare_no_false_positive(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #172 (direct shape): a function-local import bound and returned in the
    // same function is a local binding. It must fire neither E0018 ("not defined")
    // nor E0019 ("may be unbound") — the import unconditionally binds the name.
    let source =
        "def f() -> object:\n    from collections import OrderedDict\n    return OrderedDict\n";
    let diags = run(source)?;
    let fired: Vec<&str> = codes(&diags)
        .into_iter()
        .filter(|c| *c == "BSK-E0018" || *c == "BSK-E0019")
        .collect();
    assert!(
        fired.is_empty(),
        "an unconditional function-local import returned bare must not fire E0018/E0019, got: {fired:?}"
    );
    Ok(())
}

#[test]
fn e0018_function_local_dotted_and_aliased_imports_no_false_positive(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #172, remaining import shapes: a dotted `import a.b` binds the
    // top-level package `a`; an `import a.b as d` and `from m import X as z` bind
    // their aliases. All are valid local bindings reachable from a nested method.
    let source = r#"
def make() -> object:
    import os.path
    import os.path as op
    from collections import OrderedDict as OD

    class _Stub:
        def paths(self) -> object:
            return os.path.join(op.sep, "x")

        def store(self) -> object:
            return OD()

    return _Stub()
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "dotted/aliased function-local imports used in nested methods must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0018")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0018_nested_class_returned_from_enclosing_function_no_false_positive(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #172 corollary: a class defined inside a function binds its name in
    // the enclosing scope, so `return _Stub()` from that function is valid.
    let source = "def make() -> object:\n    class _Stub:\n        pass\n    return _Stub()\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "returning a function-local class must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0018")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0018_module_level_plain_aliased_import_in_return_no_false_positive(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #180: a module-level plain `import X as Y` binds `Y` (not `X`) at
    // module scope. Referencing `Y` in a function's return expression is valid —
    // a regression of #107/#64 for the plain (non-`from`) import form, whose alias
    // the resolver previously dropped. Must not fire E0018. The non-aliased
    // `import contextlib` is the control: it was never affected.
    let source = r#"
import datetime as _dt
import contextlib
import os.path as _osp


def _now() -> _dt.datetime:
    return _dt.datetime.now(tz=_dt.timezone.utc)


def _suppressor() -> object:
    return contextlib.suppress(ValueError)


def _join() -> str:
    return _osp.join("a", "b")
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "module-level plain aliased import used in a return expression must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0018")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0018_function_local_import_attribute_call_in_return_no_false_positive(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #175 (headline repro): a plain function-local `import math` binds
    // `math` for the rest of the body, including a `return math.ceil(x)` where
    // `math` heads an attribute/call chain in the return expression. This is the
    // standard lazy-import / cycle-breaking pattern — it runs without NameError
    // and must fire neither E0018 ("not defined") nor E0019 ("may be unbound"),
    // since a top-level import binds the name unconditionally.
    let source = "def ceil_cost(x: float) -> int:\n    import math\n    return math.ceil(x)\n";
    let diags = run(source)?;
    let fired: Vec<&str> = codes(&diags)
        .into_iter()
        .filter(|c| *c == "BSK-E0018" || *c == "BSK-E0019")
        .collect();
    assert!(
        fired.is_empty(),
        "a function-local `import math` used in `return math.ceil(x)` must not fire E0018/E0019, got: {fired:?}"
    );
    Ok(())
}

#[test]
fn e0018_function_local_from_import_callee_in_return_no_false_positive(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #175 (`from ... import` callee shape): a function-local
    // `from os import getpid` binds `getpid`, which is then the callee of the
    // return expression `return getpid()`. The callee path of E0018 must see the
    // local import binding, not just bare-name references.
    let source = "def extract() -> int | None:\n    from os import getpid\n    return getpid()\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "a function-local `from ... import f` used as a return callee must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0018")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn e0018_function_local_aliased_import_attribute_in_return_no_false_positive(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #175 (aliased plain-import shape): an aliased function-local
    // `import datetime as _dt` binds `_dt`, used as the attribute base of the
    // return expression `return _dt.datetime(...)`. The alias — not the module
    // name — must be the in-scope local binding.
    let source =
        "def make_dt() -> object:\n    import datetime as _dt\n    return _dt.datetime(2020, 1, 1)\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"BSK-E0018"),
        "an aliased function-local import used as a return attribute base must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "BSK-E0018")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}
