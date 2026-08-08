//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Dataclass visitor functions.

use ruff_python_ast::{Expr, ExprCall};

use crate::canonical::{BindingTable, TypingForm};


// ---------------------------------------------------------------------------
// Dataclass field specifiers
//
// The callee is recognised by resolving it to its definition, so
// `from dataclasses import field as f` and `dataclasses.field(...)` both work
// and a local `def field(...)` is correctly not the dataclass specifier.
// Keyword-argument NAMES at a call site are read directly: they need no import,
// so reading one decides nothing about typing. Implements
// [RESOLV-CANONICAL-BINDING].
// ---------------------------------------------------------------------------

/// The literal boolean passed to keyword `argument` of `call`, when present.
fn keyword_bool(call: &ExprCall, argument: &str) -> Option<bool> {
    call.arguments
        .keywords
        .iter()
        .find(|keyword| {
            keyword
                .arg
                .as_ref()
                .is_some_and(|name| name.as_str() == argument)
        })
        .and_then(|keyword| match &keyword.value {
            Expr::BooleanLiteral(literal) => Some(literal.value),
            _ => None,
        })
}

/// A dataclass field specifier call, if `value` is one.
fn field_specifier_call<'a>(bindings: &BindingTable, value: &'a Expr) -> Option<&'a ExprCall> {
    let Expr::Call(call) = value else {
        return None;
    };
    bindings
        .is_form(&call.func, TypingForm::DataclassField)
        .then_some(call)
}

/// `Some(true)`/`Some(false)` when a field specifier sets `kw_only`.
pub(super) fn field_kw_only_override(bindings: &BindingTable, value: &Expr) -> Option<bool> {
    keyword_bool(field_specifier_call(bindings, value)?, "kw_only")
}

/// Whether a field specifier sets `init=False`.
pub(super) fn field_init_is_false(bindings: &BindingTable, value: &Expr) -> bool {
    field_specifier_call(bindings, value)
        .and_then(|call| keyword_bool(call, "init"))
        .is_some_and(|init| !init)
}

// ---------------------------------------------------------------------------
// `dataclass_transform` support — DELETED
//
// The whole chain (factory discovery, field-specifier overloads, per-class
// decorator overrides, field attribute resolution) hung off a decorator parser
// that matched the DECORATOR'S SPELLING rather than resolving it, so
// `@dataclass_transform` was recognised and `@dt` from
// `from typing import dataclass_transform as dt` was not. That parser was
// deleted under [CHKARCH-TEXT-MATCHED-LOGIC]; everything that consumed it is
// deleted here with it rather than propped up.
//
// Rebuild is scoped by [ASTREBUILD-PHASE-RESOLVER] — see
// docs/plans/CHECKER-AST-RECONSTRUCTION-PLAN.md. Until then classes produced by
// a `dataclass_transform` factory carry none of the dataclass flags, and the
// rules keyed to them report nothing.
