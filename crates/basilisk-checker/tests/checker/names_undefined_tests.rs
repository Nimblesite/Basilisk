//! Tests for [`names_undefined`] from [CHKARCH-DIAG-TYPESAFETY]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPESAFETY
// Integration tests for names_undefined: Undefined variable in return.

use super::common::*;

#[test]
fn undefined_name_in_return_fires() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def compute() -> int:\n    return undefined_name\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_undefined"),
        "undefined name in return should fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn defined_param_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def compute(x: int) -> int:\n    return x\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "returning a parameter should not fire E0018"
    );
    Ok(())
}

#[test]
fn locally_assigned_no_diagnostic() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def compute() -> int:\n    result = 42\n    return result\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "returning a locally assigned variable should not fire E0018"
    );
    Ok(())
}

#[test]
fn module_level_variable_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    let source = "\
_EMPTY_TEXT_MSG = \"text is required\"

def validate(text: str) -> str:
    if not text:
        return _EMPTY_TEXT_MSG
    return text
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "returning a module-level variable should not fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn diagnostic_has_help() -> Result<(), Box<dyn std::error::Error>> {
    let source = "def compute() -> int:\n    return missing\n";
    let diags = run(source)?;
    let e0018 = diags.iter().find(|d| d.code.code == "names_undefined");
    assert!(e0018.is_some(), "should fire E0018");
    let Some(diag) = e0018 else {
        return Err("E0018 diagnostic missing after assertion".into());
    };
    assert!(diag.help.is_some(), "E0018 should have help text");
    Ok(())
}

#[test]
fn aliased_module_import_in_return_not_flagged() -> Result<(), Box<dyn std::error::Error>> {
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
        !codes(&diags).contains(&"names_undefined"),
        "aliased module import used in a return expression must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn undefined_callee_in_return_call_fires() -> Result<(), Box<dyn std::error::Error>> {
    // The callee of a call in a return must be checked, not just bare names:
    // `return undefined_fn()` references `undefined_fn`.
    let source = "def f() -> object:\n    return undefined_fn()\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_undefined"),
        "an undefined callee in a return call should fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn sibling_function_call_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // Calling a sibling module-level function must not fire — it is in scope.
    let source = "def helper() -> int:\n    return 1\n\n\ndef use() -> int:\n    return helper()\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "`return helper()` for a module-level function must not fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn bare_sibling_function_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // Returning a sibling module-level function by name is valid (it IS defined).
    let source =
        "def helper() -> int:\n    return 1\n\n\ndef use() -> object:\n    return helper\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "`return helper` for a module-level function must not fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn class_instantiation_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // Instantiating a module-level class must not fire.
    let source = "class Foo:\n    pass\n\n\ndef make() -> Foo:\n    return Foo()\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "`return Foo()` for a module-level class must not fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn builtin_call_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // A builtin callee must not fire.
    let source = "def f() -> int:\n    return len([1, 2])\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "`return len(...)` (builtin callee) must not fire E0018, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn function_local_import_in_nested_class_method_no_false_positive(
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
        !codes(&diags).contains(&"names_undefined"),
        "function-local import used in a nested class method must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn function_local_import_returned_bare_no_false_positive() -> Result<(), Box<dyn std::error::Error>>
{
    // Issue #172 (direct shape): a function-local import bound and returned in the
    // same function is a local binding. It must fire neither E0018 ("not defined")
    // nor E0019 ("may be unbound") — the import unconditionally binds the name.
    let source =
        "def f() -> object:\n    from collections import OrderedDict\n    return OrderedDict\n";
    let diags = run(source)?;
    let fired: Vec<&str> = codes(&diags)
        .into_iter()
        .filter(|c| *c == "names_undefined" || *c == "names_unbound")
        .collect();
    assert!(
        fired.is_empty(),
        "an unconditional function-local import returned bare must not fire E0018/E0019, got: {fired:?}"
    );
    Ok(())
}

#[test]
fn function_local_dotted_and_aliased_imports_no_false_positive(
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
        !codes(&diags).contains(&"names_undefined"),
        "dotted/aliased function-local imports used in nested methods must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn nested_class_returned_from_enclosing_function_no_false_positive(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #172 corollary: a class defined inside a function binds its name in
    // the enclosing scope, so `return _Stub()` from that function is valid.
    let source = "def make() -> object:\n    class _Stub:\n        pass\n    return _Stub()\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "returning a function-local class must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn module_level_plain_aliased_import_in_return_no_false_positive(
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
        !codes(&diags).contains(&"names_undefined"),
        "module-level plain aliased import used in a return expression must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn function_local_import_attribute_call_in_return_no_false_positive(
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
        .filter(|c| *c == "names_undefined" || *c == "names_unbound")
        .collect();
    assert!(
        fired.is_empty(),
        "a function-local `import math` used in `return math.ceil(x)` must not fire E0018/E0019, got: {fired:?}"
    );
    Ok(())
}

#[test]
fn function_local_from_import_callee_in_return_no_false_positive(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #175 (`from ... import` callee shape): a function-local
    // `from os import getpid` binds `getpid`, which is then the callee of the
    // return expression `return getpid()`. The callee path of E0018 must see the
    // local import binding, not just bare-name references.
    let source = "def extract() -> int | None:\n    from os import getpid\n    return getpid()\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "a function-local `from ... import f` used as a return callee must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn function_local_aliased_import_attribute_in_return_no_false_positive(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #175 (aliased plain-import shape): an aliased function-local
    // `import datetime as _dt` binds `_dt`, used as the attribute base of the
    // return expression `return _dt.datetime(...)`. The alias — not the module
    // name — must be the in-scope local binding.
    let source =
        "def make_dt() -> object:\n    import datetime as _dt\n    return _dt.datetime(2020, 1, 1)\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "an aliased function-local import used as a return attribute base must not fire E0018, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Issue #339 — the walrus operator (`:=`, PEP 572) binds its target
// ---------------------------------------------------------------------------

#[test]
fn walrus_in_if_test_binds_the_name_for_a_return_inside_the_branch(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #339 (headline repro): `if item := mapping.get(key):` binds `item`
    // for the branch body, so `return item` references a defined name. PEP 572
    // assignment expressions bind in the *enclosing* scope exactly like `=`.
    let source = "\
def get_price(prices: dict[str, float], asset: str) -> float | None:
    if item := prices.get(asset):
        return item
    return None
";
    let diags = run(source)?;
    let fired: Vec<&str> = codes(&diags)
        .into_iter()
        .filter(|c| *c == "names_undefined" || *c == "names_unbound")
        .collect();
    assert!(
        fired.is_empty(),
        "a walrus target used in a return inside the guarded branch must fire neither \
         E0018 nor E0019, got: {fired:?}"
    );
    Ok(())
}

#[test]
fn walrus_in_while_test_binds_the_name_for_a_return_inside_the_loop(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #339 (loop shape): the other canonical walrus site is a `while`
    // test, where the binding is equally visible to the loop body.
    let source = "\
def first_line(lines: list[str]) -> str | None:
    while line := lines.pop():
        return line
    return None
";
    let diags = run(source)?;
    let fired: Vec<&str> = codes(&diags)
        .into_iter()
        .filter(|c| *c == "names_undefined" || *c == "names_unbound")
        .collect();
    assert!(
        fired.is_empty(),
        "a walrus target bound by a `while` test must fire neither E0018 nor E0019, \
         got: {fired:?}"
    );
    Ok(())
}

#[test]
fn walrus_in_if_test_binds_the_name_for_a_later_top_level_return(
) -> Result<(), Box<dyn std::error::Error>> {
    // Issue #339 (post-branch shape): an `if` test always evaluates, so a walrus
    // inside it binds unconditionally — the name is live after the statement,
    // whichever way the branch went. Recognising the binding must therefore not
    // merely trade E0018 ("not defined") for E0019 ("may be unbound").
    let source = "\
def lookup(items: dict[str, int], key: str) -> int | None:
    if hit := items.get(key):
        print(hit)
    return hit
";
    let diags = run(source)?;
    let fired: Vec<&str> = codes(&diags)
        .into_iter()
        .filter(|c| *c == "names_undefined" || *c == "names_unbound")
        .collect();
    assert!(
        fired.is_empty(),
        "an `if`-test walrus binds unconditionally, so a later top-level return of the \
         name must fire neither E0018 nor E0019, got: {fired:?}"
    );
    Ok(())
}

#[test]
fn pep695_type_alias_in_return_cast_is_defined() -> Result<(), Box<dyn std::error::Error>> {
    // Issue #372: a PEP 695 `type` statement binds its name at module scope
    // (a lazily evaluated `TypeAliasType` object), so referencing the alias
    // in a return-position `cast(...)` call is NOT an undefined name.
    let source = "\
from typing import cast

type Fahrenheit = float


def to_f(celsius: float) -> Fahrenheit:
    return cast(Fahrenheit, celsius * 9 / 5 + 32)
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "a `type` statement alias used in a return cast must not fire E0018, got: {:?}",
        messages_for(&diags, "names_undefined")
    );
    Ok(())
}

#[test]
fn pep695_type_alias_returned_bare_is_defined() -> Result<(), Box<dyn std::error::Error>> {
    // Issue #372 (general form): the alias object itself is a first-class
    // runtime value — `return Alias` is a defined-name reference.
    let source = "\
type Point = tuple[float, float]


def alias() -> object:
    return Point
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "returning the alias object itself must not fire E0018, got: {:?}",
        messages_for(&diags, "names_undefined")
    );
    Ok(())
}

#[test]
fn class_scope_type_alias_is_not_visible_from_a_function() -> Result<(), Box<dyn std::error::Error>>
{
    // Class-body names do not nest: a `type` alias declared inside a class
    // is reachable only as `C.Inner`, so a bare `Inner` in a module-level
    // function is still an undefined name.
    let source = "\
class C:
    type Inner = int


def f() -> object:
    return Inner
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_undefined"),
        "a class-scope alias must not leak into module scope, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn function_scope_type_alias_is_not_visible_from_a_sibling(
) -> Result<(), Box<dyn std::error::Error>> {
    // A `type` alias declared inside one function is local to it — a
    // sibling function referencing the name is an undefined name.
    let source = "\
def g() -> None:
    type T = int


def f() -> object:
    return T
";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_undefined"),
        "a function-scope alias must not leak into sibling functions, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn function_scope_type_alias_is_visible_in_its_own_function(
) -> Result<(), Box<dyn std::error::Error>> {
    // Inside the declaring function (and its nested functions) the alias
    // is an ordinary local binding.
    let source = "\
def f() -> object:
    type T = int

    def inner() -> object:
        return T

    return T
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "a function-local alias is defined in its own scope, got: {:?}",
        messages_for(&diags, "names_undefined")
    );
    Ok(())
}

#[test]
fn self_referential_class_bases_terminate_and_flag() -> Result<(), Box<dyn std::error::Error>> {
    // GitHub #398: `class C(C[int], C[bool])` sent the resolver's transitive
    // base walk into an exponential recursion — checking must TERMINATE. And
    // per Python semantics a class name is unbound until its `class` statement
    // completes, so referencing it in its own bases list must draw
    // `names_undefined` (both classes here have no other binding).
    let (tx, rx) = std::sync::mpsc::channel();
    let _worker = std::thread::spawn(move || {
        let outcome = run("class C(C[int], C[bool]):\n    pass\n").map_err(|e| e.to_string());
        let _ = tx.send(outcome);
    });
    let received = rx.recv_timeout(std::time::Duration::from_secs(30));
    let Ok(outcome) = received else {
        return Err("resolver spun for 30s on self-referential bases (GitHub #398)".into());
    };
    let diags = outcome?;
    assert!(
        codes(&diags).contains(&"names_undefined"),
        "`class C(C[int], C[bool])` must flag the unbound self-reference, got: {:?}",
        codes(&diags)
    );

    let diags = run("class D(D):\n    pass\n")?;
    assert!(
        codes(&diags).contains(&"names_undefined"),
        "`class D(D)` must flag the unbound self-reference, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn prior_binding_and_builtin_self_named_bases_stay_clean() -> Result<(), Box<dyn std::error::Error>>
{
    // Redefining a class over a prior binding is legal Python — the base
    // names the OLD binding, not the class being defined.
    let source = "class C:\n    pass\n\n\nclass C(C):\n    pass\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "a prior binding makes `class C(C)` legal, got: {:?}",
        messages_for(&diags, "names_undefined")
    );

    // `class int(int)` derives from the BUILTIN int — also legal.
    let diags = run("class int(int):\n    pass\n")?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "a builtin base name is always bound, got: {:?}",
        messages_for(&diags, "names_undefined")
    );
    Ok(())
}

#[test]
fn module_level_undefined_callee_fires() -> Result<(), Box<dyn std::error::Error>> {
    // Issue #397 (verbatim repro): `print2` is defined nowhere, so this line
    // raises NameError at import time. "All checked. No issues found." on it
    // sent a first-time user straight to the issue tracker.
    let source = "a: int = print2(\"abc\")\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_undefined"),
        "a module-level call to an undefined name must fire names_undefined, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn module_level_defined_callees_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // Every legitimate module-scope callee source: builtin, import, def, class,
    // and a module variable holding a callable.
    let source = r#"
from functools import reduce

def local_fn() -> int:
    return 1

class LocalCls:
    pass

alias = local_fn

a = print("x")
b = reduce(lambda acc, item: acc, [1])
c = local_fn()
d = LocalCls()
e = alias()
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "defined module-level callees must not fire names_undefined, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn module_level_loop_and_unpack_targets_no_false_positive() -> Result<(), Box<dyn std::error::Error>>
{
    // Binding forms beyond plain `=`: tuple unpacking, `for` targets, `with`
    // targets, walrus, and `except ... as` names all bind at module scope.
    let source = r#"
import contextlib

first, second = print, print
for handler in [print]:
    handler()
with contextlib.suppress(ValueError) as guard:
    pass
if hit := print:
    hit()
first()
second()
"#;
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "module-level loop/unpack/walrus targets must not fire names_undefined, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn module_level_star_import_disables_the_check() -> Result<(), Box<dyn std::error::Error>> {
    // `from m import *` can bind any name, so the module-level pass must stay
    // silent rather than guess.
    let source = "from os.path import *\n\na = join(\"x\", \"y\")\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "a star import must disable the module-level undefined-callee check, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn function_body_local_callee_not_flagged_by_module_pass() -> Result<(), Box<dyn std::error::Error>>
{
    // The resolver's call collector walks into function bodies; the
    // module-level pass must not treat a parameter callee as module scope.
    let source = "\
from collections.abc import Callable

def apply(callback: Callable[[], int]) -> int:
    value = callback()
    return value
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "a parameter used as a callee inside a function must not fire names_undefined, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn module_level_full_builtin_surface_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // Builtins outside the original curated list (divmod, ascii, eval, vars,
    // globals) must be recognised at module level.
    let source = "\
a = divmod(7, 2)
b = ascii(\"x\")
c = eval(\"1\")
d = vars()
e = globals()
";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "every real builtin callee must be in scope at module level, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn self_inheriting_class_with_no_prior_binding_fires() -> Result<(), Box<dyn std::error::Error>> {
    // The minimal twin from the #398 torture case: Python evaluates the bases
    // tuple BEFORE binding the class name, so this raises NameError at import
    // time. The checker terminated (post-#398 fix) but stayed silent.
    let source = "class D(D):\n    pass\n";
    let diags = run(source)?;
    assert!(
        codes(&diags).contains(&"names_undefined"),
        "`class D(D)` with no prior `D` binding must fire names_undefined, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn self_inheriting_redefinition_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // Legal shadowing: the base `D` resolves to the PREVIOUS class binding.
    let source = "class D:\n    pass\n\n\nclass D(D):\n    pass\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "redefining `D` inheriting the previous `D` is legal, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn self_inheriting_imported_shadow_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // Legal shadowing: the base `D` resolves to the imported binding.
    let source = "from other_module import D\n\n\nclass D(D):\n    pass\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "a class shadowing an imported name may inherit from it, got: {:?}",
        diags
            .iter()
            .filter(|d| d.code.code == "names_undefined")
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn self_inheriting_builtin_shadow_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // Legal shadowing: `class int(int)` inherits the builtin `int`.
    let source = "class int(int):\n    pass\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "a class shadowing a builtin may inherit from it, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn self_inheriting_class_under_star_import_no_false_positive(
) -> Result<(), Box<dyn std::error::Error>> {
    // A star import can bind any name, `D` included — stay silent.
    let source = "from other_module import *\n\n\nclass D(D):\n    pass\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "a star import may have bound `D`, so the check must stay silent, got: {:?}",
        codes(&diags)
    );
    Ok(())
}

#[test]
fn self_inheriting_assigned_shadow_no_false_positive() -> Result<(), Box<dyn std::error::Error>> {
    // Legal shadowing: `D` is bound by assignment before the class statement.
    let source = "D = object\n\n\nclass D(D):\n    pass\n";
    let diags = run(source)?;
    assert!(
        !codes(&diags).contains(&"names_undefined"),
        "a class shadowing an assigned name may inherit from it, got: {:?}",
        codes(&diags)
    );
    Ok(())
}
