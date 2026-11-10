//! Tests for [CHKARCH-TESTING]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-TESTING
// `mod common` is shared by every checker test binary, so the helpers this one
// does not call are dead code HERE, not unused code — same waiver as
// `mutation_kill_tests.rs`.
#![allow(clippy::allow_attributes, dead_code)]
//!
//! Mutation-killing tests for the metaclass `__call__` half of
//! `calls_argument_count` ([CHKARCH-TESTING-MUTATION-RATCHET]).
//!
//! A class whose metaclass defines `__call__` may never reach `__new__`/
//! `__init__` at all, so the constructor-arity judgment has to decide who
//! governs the call before it counts a single argument
//! ([metaclass `__call__`](https://typing.python.org/en/latest/spec/constructors.html#metaclass-call-method)).
//! Every test below asserts BOTH directions — the arity error that must fire
//! and the silence that must hold — because a mutant that flips the decision
//! one way is invisible to a suite that only checks the other.

use basilisk_test_macros::mutation_safe;

mod common;
use common::run;

/// A class whose `__new__` demands one argument, instantiated with none. What
/// the metaclass declares decides whether that is an error.
const CALL_WITH_NO_ARGS: &str = "\nclass C(metaclass=Meta):\n    def __new__(cls, x: int) -> \"C\":\n        return super().__new__(cls)\n\nC()\n";

/// Arity diagnostics drawn by `source`.
fn arity_errors(source: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    Ok(run(source)?
        .iter()
        .filter(|d| d.code.code == "calls_argument_count")
        .map(|d| d.message.clone())
        .collect())
}

/// Assert the constructor call draws exactly one arity error.
fn assert_reports(metaclass: &str, why: &str) -> Result<(), Box<dyn std::error::Error>> {
    let errors = arity_errors(&format!("{metaclass}{CALL_WITH_NO_ARGS}"))?;
    assert_eq!(errors.len(), 1, "{why}; got {errors:?}");
    Ok(())
}

/// Assert the constructor call is silent — the metaclass governs it.
fn assert_silent(metaclass: &str, why: &str) -> Result<(), Box<dyn std::error::Error>> {
    let errors = arity_errors(&format!("{metaclass}{CALL_WITH_NO_ARGS}"))?;
    assert!(errors.is_empty(), "{why}; got {errors:?}");
    Ok(())
}

/// Kills `metaclass_passes_through -> true`, `constructs_an_instance -> true`,
/// and both `==` → `!=` mutants in the `__call__` lookup (each of which loses
/// the method and falls back to "passes through").
///
/// A `__call__` returning `NoReturn` never yields a `C`, so `__new__` is never
/// evaluated and its signature cannot be violated.
#[mutation_safe(
    rule = "calls_argument_count",
    fns = "metaclass_passes_through|constructs_an_instance|body_delegates_construction"
)]
#[test]
fn metaclass_call_returning_noreturn_governs_the_call() -> Result<(), Box<dyn std::error::Error>> {
    assert_silent(
        "from typing import NoReturn\n\nclass Meta(type):\n    def __call__(cls, *args, **kwargs) -> NoReturn:\n        raise TypeError('no')\n",
        "a metaclass __call__ returning NoReturn never reaches __new__",
    )
}

/// Kills `metaclass_passes_through -> true` for a concrete foreign return: the
/// call evaluates to an `int`, not to a `C`.
#[mutation_safe(
    rule = "calls_argument_count",
    fns = "metaclass_passes_through|constructs_an_instance"
)]
#[test]
fn metaclass_call_returning_a_foreign_type_governs_the_call(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_silent(
        "class Meta(type):\n    def __call__(cls, *args, **kwargs) -> int:\n        return 1\n",
        "a metaclass __call__ returning int does not construct a C",
    )
}

/// Kills `metaclass_passes_through -> false`, `constructs_an_instance -> false`,
/// and `==` → `!=` on the `TypeVar` comparison (with `!=`, `T` matches no
/// declared `TypeVar` and the call is wrongly treated as metaclass-governed).
#[mutation_safe(
    rule = "calls_argument_count",
    fns = "metaclass_passes_through|constructs_an_instance"
)]
#[test]
fn metaclass_call_returning_its_typevar_still_checks_new() -> Result<(), Box<dyn std::error::Error>>
{
    assert_reports(
        "from typing import TypeVar\n\nT = TypeVar(\"T\")\n\nclass Meta(type):\n    def __call__(cls: type[T], *args, **kwargs) -> T:\n        return type.__call__(cls, *args, **kwargs)\n",
        "a `-> T` metaclass __call__ constructs the class, so __new__ governs arity",
    )
}

/// Kills `||` → `&&` and `==` → `!=` on the `Self` comparison in
/// `constructs_an_instance`: both make a `-> Self` return stop counting as
/// construction.
#[mutation_safe(rule = "calls_argument_count", fns = "constructs_an_instance")]
#[test]
fn metaclass_call_returning_self_still_checks_new() -> Result<(), Box<dyn std::error::Error>> {
    assert_reports(
        "from typing import Self\n\nclass Meta(type):\n    def __call__(cls, *args, **kwargs) -> Self:\n        return type.__call__(cls, *args, **kwargs)\n",
        "a `-> Self` metaclass __call__ constructs the class, so __new__ governs arity",
    )
}

/// Kills `body_delegates_construction -> false`: an UNANNOTATED `__call__` whose
/// body hands the call back to `type.__call__` constructs normally, so stripping
/// the annotations off a metaclass must not silence the arity error
/// ([TYPEINF-TARGET-GRADUAL]).
#[mutation_safe(
    rule = "calls_argument_count",
    fns = "metaclass_passes_through|body_delegates_construction"
)]
#[test]
fn unannotated_metaclass_call_that_delegates_still_checks_new(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_reports(
        "class Meta(type):\n    def __call__(cls, *args, **kwargs):\n        return type.__call__(cls, *args, **kwargs)\n",
        "an unannotated __call__ that delegates constructs normally",
    )
}

/// Kills `body_delegates_construction -> true` and its `&&` → `||`: an
/// unannotated `__call__` that returns a value of its own is not constructing a
/// `C`, so `__new__` is never consulted.
#[mutation_safe(
    rule = "calls_argument_count",
    fns = "metaclass_passes_through|body_delegates_construction"
)]
#[test]
fn unannotated_metaclass_call_returning_a_value_governs_the_call(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_silent(
        "class Meta(type):\n    def __call__(cls, *args, **kwargs):\n        return 1\n",
        "an unannotated __call__ returning its own value does not construct a C",
    )
}

/// Kills the `&&` → `||` mutant in `body_delegates_construction`: a body that
/// never returns has NO value-returning statement, and `all()` over nothing is
/// vacuously true — only the emptiness check keeps that from reading as
/// "delegates".
#[mutation_safe(
    rule = "calls_argument_count",
    fns = "metaclass_passes_through|body_delegates_construction"
)]
#[test]
fn unannotated_metaclass_call_that_only_raises_governs_the_call(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_silent(
        "class Meta(type):\n    def __call__(cls, *args, **kwargs):\n        raise TypeError('no')\n",
        "a __call__ that only raises never returns a C",
    )
}

/// Kills both `&&` → `||` mutants joining the pass-through conditions: a
/// `__call__` that does not forward `*args`/`**kwargs` fixes the call signature
/// itself, so `__new__`'s parameters are not what the caller must satisfy —
/// even though its return type says it constructs the class.
#[mutation_safe(rule = "calls_argument_count", fns = "metaclass_passes_through")]
#[test]
fn metaclass_call_without_kwargs_passthrough_is_not_checked(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_silent(
        "from typing import Self\n\nclass Meta(type):\n    def __call__(cls, *args) -> Self:\n        return type.__call__(cls, *args)\n",
        "a __call__ that forwards only *args does not pass the caller's arguments through unchanged",
    )
}

/// Kills `delete !` and `==` → `!=` in the metaclass-exists guard: a metaclass
/// this module cannot see into could do anything, so the judgment abstains
/// rather than assuming the default `type.__call__`.
#[mutation_safe(rule = "calls_argument_count", fns = "metaclass_passes_through")]
#[test]
fn unresolvable_metaclass_abstains() -> Result<(), Box<dyn std::error::Error>> {
    let errors = arity_errors(
        "from elsewhere import Meta\n\nclass C(metaclass=Meta):\n    def __new__(cls, x: int) -> \"C\":\n        return super().__new__(cls)\n\nC()\n",
    )?;
    assert!(
        errors.is_empty(),
        "a metaclass defined outside this module must abstain, not guess; got {errors:?}"
    );
    Ok(())
}

/// The baseline the whole rule rests on: with no metaclass in play, a
/// constructor call short of `__new__`'s required arguments still reports. A
/// mutant that silences the metaclass path must not be able to hide behind a
/// suite that never checks the ordinary case.
#[mutation_safe(rule = "calls_argument_count", fns = "metaclass_passes_through")]
#[test]
fn plain_class_still_reports_missing_constructor_argument() -> Result<(), Box<dyn std::error::Error>>
{
    let errors = arity_errors(
        "class C:\n    def __new__(cls, x: int) -> \"C\":\n        return super().__new__(cls)\n\nC()\n",
    )?;
    assert_eq!(
        errors.len(),
        1,
        "a plain class must still report its missing __new__ argument; got {errors:?}"
    );
    Ok(())
}
