//! Implements [`constructors_callable`] from [CHKARCH-DIAG-CTOR-CALLABLE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-CTOR-CALLABLE
//! `constructors_callable`: Invalid call to a constructor-derived callable.
//!
//! Implements the typing spec rule "Converting a constructor to callable"
//! (<https://typing.readthedocs.io/en/latest/spec/constructors.html#converting-a-constructor-to-callable>).
//!
//! A variable that holds a class's *constructor-to-callable* signature must be
//! called in a way that matches that synthesized signature:
//!
//! ```python
//! r1()      # E0153: missing required argument `x`
//! r1(y=1)   # E0153: unexpected keyword argument `y`
//! ```
//!
//! The synthesized signature is derived (in priority order) from the
//! metaclass `__call__`, then `__new__` (when it returns a type other than the
//! class), then `__init__`, mirroring runtime construction.

use std::collections::HashMap;

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::{Expr, ExprCall, Number};
use ruff_text_size::Ranged as _;

use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

use super::Rule;

mod conversion;

use conversion::{CallableGroup, CallableVariant};

const CODE: ErrorCode = ErrorCode {
    code: "constructors_callable",
    docs_url: "https://www.basilisk-python.dev/errors/constructors_callable",
};

/// Emits `constructors_callable` for invalid calls to constructor-derived callables.
pub(crate) struct ConstructorCallableMisuse;

impl Rule for ConstructorCallableMisuse {
    fn check(
        &self,
        module: &ResolvedModule,
        ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        super::check_with_own_types(self, module, ctx, diagnostics);
    }

    fn check_with_types(
        &self,
        _module: &ResolvedModule,
        _types: &super::shared::module_types::ModuleTypes<'_>,
        _ctx: &super::CheckContext,
        _diagnostics: &mut Vec<Diagnostic>,
    ) {
    }
}

/// Validate a call against the synthesized signature, emitting one diagnostic.
fn validate_call(
    call: &ExprCall,
    class_name: &str,
    signatures: &[CallableGroup<'_>],
    typevars: &[&str],
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Conservative: starred positional / `**kwargs` unpacking defeats arity
    // analysis, so skip rather than risk a false positive.
    if call
        .arguments
        .args
        .iter()
        .any(|a| matches!(a, Expr::Starred(_)))
        || call.arguments.keywords.iter().any(|k| k.arg.is_none())
    {
        return;
    }

    let positional = call.arguments.args.len();
    let kw_names: Vec<&str> = call
        .arguments
        .keywords
        .iter()
        .filter_map(|k| k.arg.as_ref().map(ruff_python_ast::Identifier::as_str))
        .collect();

    let failure = signatures.iter().find_map(|group| {
        group_failure(
            call, class_name, group, positional, &kw_names, typevars, path,
        )
    });
    if let Some(failure) = failure {
        diagnostics.push(failure);
    }
}

fn group_failure(
    call: &ExprCall,
    class_name: &str,
    group: &CallableGroup<'_>,
    positional: usize,
    kw_names: &[&str],
    typevars: &[&str],
    path: &str,
) -> Option<Diagnostic> {
    let mut first_failure = None;
    for signature in &group.variants {
        let failure = check_keywords(call, class_name, signature, kw_names, path)
            .or_else(|| check_too_many(call, class_name, signature, positional, path))
            .or_else(|| check_missing(call, class_name, signature, positional, kw_names, path))
            .or_else(|| check_typevar_conflict(call, class_name, signature, typevars, path));
        let _diagnostic = failure.as_ref()?;
        first_failure = first_failure.or(failure);
    }
    first_failure
}

/// Flag the first keyword that names no parameter (when no `**kwargs`).
fn check_keywords(
    call: &ExprCall,
    class_name: &str,
    sig: &CallableVariant<'_>,
    kw_names: &[&str],
    path: &str,
) -> Option<Diagnostic> {
    if sig.has_var_keyword {
        return None;
    }
    let unknown = kw_names
        .iter()
        .find(|name| !sig.params.iter().any(|p| p.name == **name))?;
    Some(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Unexpected keyword argument `{unknown}` in call to the constructor-derived \
             callable for `{class_name}`"
        ),
        Span::from(call.range()),
        path,
        Some("Remove the keyword argument or pass it positionally".to_owned()),
        None,
    ))
}

/// Flag too many positional arguments (when no `*args`).
fn check_too_many(
    call: &ExprCall,
    class_name: &str,
    sig: &CallableVariant<'_>,
    positional: usize,
    path: &str,
) -> Option<Diagnostic> {
    if sig.has_var_positional || positional <= sig.params.len() {
        return None;
    }
    let max = sig.params.len();
    let extra = positional - max;
    Some(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Call to the constructor-derived callable for `{class_name}` has {extra} too many \
             positional argument{} (expected at most {max}, got {positional})",
            if extra == 1 { "" } else { "s" },
        ),
        Span::from(call.range()),
        path,
        None,
        None,
    ))
}

/// Flag the first required parameter left unsatisfied by position or keyword.
fn check_missing(
    call: &ExprCall,
    class_name: &str,
    sig: &CallableVariant<'_>,
    positional: usize,
    kw_names: &[&str],
    path: &str,
) -> Option<Diagnostic> {
    let missing = sig.params.iter().enumerate().find(|(idx, param)| {
        !param.has_default && *idx >= positional && !kw_names.contains(&param.name.as_str())
    })?;
    let required = sig.params.iter().filter(|p| !p.has_default).count();
    Some(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Call to the constructor-derived callable for `{class_name}` is missing required \
             argument `{}` (expected {required}, got {positional})",
            missing.1.name,
        ),
        Span::from(call.range()),
        path,
        None,
        None,
    ))
}

/// Flag a `TypeVar` bound to two incompatible types across `list[T]` params.
fn check_typevar_conflict(
    call: &ExprCall,
    class_name: &str,
    sig: &CallableVariant<'_>,
    typevars: &[&str],
    path: &str,
) -> Option<Diagnostic> {
    let mut bindings: HashMap<&str, &'static str> = HashMap::new();
    for (idx, param) in sig.params.iter().enumerate() {
        let Some(arg) = call.arguments.args.get(idx) else {
            break;
        };
        let Some(tv) = list_typevar(param.annotation_text.as_deref(), typevars) else {
            continue;
        };
        let Some(element) = list_literal_element(arg) else {
            // A non-list scalar literal can never satisfy a `list[tv]` parameter:
            // no assignment of `tv` makes it valid.
            if let Some(scalar) = non_list_scalar_literal(arg) {
                return Some(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Argument for the `list[{tv}]` parameter of the constructor-derived \
                         callable for `{class_name}` is a `{scalar}` literal — no assignment \
                         of type variable `{tv}` makes it valid"
                    ),
                    Span::from(arg.range()),
                    path,
                    None,
                    None,
                ));
            }
            continue;
        };
        if let Some(previous) = bindings.insert(tv, element) {
            if previous != element {
                return Some(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Inconsistent binding for type variable `{tv}` in call to the \
                         constructor-derived callable for `{class_name}`: `list[{previous}]` \
                         vs `list[{element}]`"
                    ),
                    Span::from(call.range()),
                    path,
                    None,
                    None,
                ));
            }
        }
    }
    None
}

/// If `annotation` is `list[T]` where `T` is a known `TypeVar`, return `T`.
fn list_typevar<'a>(annotation: Option<&'a str>, typevars: &[&str]) -> Option<&'a str> {
    let text = annotation?.trim();
    let inner = text.strip_prefix("list[")?.strip_suffix(']')?.trim();
    typevars.contains(&inner).then_some(inner)
}

/// A scalar literal that is definitely not a list (so it can never satisfy a
/// `list[...]` parameter).
fn non_list_scalar_literal(arg: &Expr) -> Option<&'static str> {
    match arg {
        Expr::List(_) => None,
        _ => literal_type_name(arg),
    }
}

/// Return the literal element type of a homogeneous list literal `[lit, ...]`.
fn list_literal_element(arg: &Expr) -> Option<&'static str> {
    let Expr::List(list) = arg else {
        return None;
    };
    let first = literal_type_name(list.elts.first()?)?;
    list.elts
        .iter()
        .all(|elt| literal_type_name(elt) == Some(first))
        .then_some(first)
}

/// Map a literal expression to its Python type name, or `None` for non-literals.
fn literal_type_name(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::BooleanLiteral(_) => Some("bool"),
        Expr::StringLiteral(_) | Expr::FString(_) => Some("str"),
        Expr::BytesLiteral(_) => Some("bytes"),
        Expr::NumberLiteral(n) => match n.value {
            Number::Int(_) => Some("int"),
            Number::Float(_) => Some("float"),
            Number::Complex { .. } => Some("complex"),
        },
        _ => None,
    }
}
