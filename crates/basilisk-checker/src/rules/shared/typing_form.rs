//! Implements [LINESCANPLAN-AST-MIGRATION]. See
//! docs/plans/CHECKER-ELIMINATE-LINE-SCANNING-PLAN.md#LINESCANPLAN-AST-MIGRATION
//!
//! Structural accessors over annotation expression nodes.
//!
//! Everything here reads the shape of a parsed `ruff` node. Nothing here
//! decides what a name *means*: the name-taking recognition surface that once
//! lived in this module is deleted under the symbol-naming ban, and no
//! replacement may be introduced here.

use basilisk_resolver::{BindingTable, TypingForm};
use ruff_python_ast::Expr;

use crate::annotation::AnnotationResolver;

/// The `TypedDict` qualifiers wrapped around an annotation expression, and
/// the core type node they wrap.
pub(crate) struct PeeledQualifiers<'e> {
    /// `Some(true)` for `Required[...]`, `Some(false)` for `NotRequired[...]`
    /// (PEP 655), `None` when neither appears.
    pub(crate) required: Option<bool>,
    /// `true` when `ReadOnly[...]` (PEP 705) wraps the core.
    pub(crate) readonly: bool,
    /// The annotation with every qualifier wrapper removed.
    pub(crate) core: &'e Expr,
}

/// Peel nested `Required[...]` / `NotRequired[...]` / `ReadOnly[...]`
/// wrappers off an annotation expression. Qualifier identity resolves
/// through the module's bindings, so an aliased import counts and a
/// module-local definition of the same name does not.
pub(crate) fn peel_qualifiers<'e>(bindings: &BindingTable, expr: &'e Expr) -> PeeledQualifiers<'e> {
    let mut peeled = PeeledQualifiers {
        required: None,
        readonly: false,
        core: expr,
    };
    while let Expr::Subscript(subscript) = peeled.core {
        match bindings.form_of(&subscript.value) {
            Some(TypingForm::Required) => peeled.required = Some(true),
            Some(TypingForm::NotRequired) => peeled.required = Some(false),
            Some(TypingForm::ReadOnly) => peeled.readonly = true,
            _ => break,
        }
        peeled.core = &subscript.slice;
    }
    peeled
}

/// The core of an annotation expression with `TypedDict` qualifiers stripped.
pub(crate) fn strip_qualifiers<'e>(resolver: &AnnotationResolver<'_>, expr: &'e Expr) -> &'e Expr {
    peel_qualifiers(resolver.bindings(), expr).core
}

/// The comma-separated arguments of a subscript slice: `X[a, b]` yields
/// `[a, b]`, `X[a]` yields `[a]`.
pub(crate) fn subscript_args(slice: &Expr) -> Vec<&Expr> {
    match slice {
        Expr::Tuple(tuple) => tuple.elts.iter().collect(),
        single => vec![single],
    }
}
