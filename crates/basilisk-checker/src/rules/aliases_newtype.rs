//! Implements [`aliases_newtype`] from [CHKARCH-DIAG-STRUCTURAL]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-STRUCTURAL
//! `aliases_newtype`: Invalid `NewType(...)` call.
//!
//! PEP 484 places restrictions on `NewType`:
//!
//! - The string name must match the variable it is assigned to
//! - The base type must be a proper concrete class
//! - `NewType` accepts exactly two arguments
//!
//! Every verdict is structural over the parsed `ruff` AST
//! ([LINESCANPLAN-AST-MIGRATION], issue #408), and every reference to a
//! builtin (`type`, `isinstance`, the base classes) resolves through the
//! module's bindings and the semantic relation layer
//! ([ASTREBUILD-LAW], [RESOLV-CANONICAL-RELATION]) — never through the
//! spelling at the use site.
//!
//! ```python
//! from typing import NewType
//! GoodName = NewType("BadName", int)  # E: name mismatch
//! BadNewType6 = NewType("BadNewType6", int, int)  # E: too many arguments
//! ```

use std::collections::{HashMap, HashSet};

use basilisk_resolver::{
    assignable, BuiltinClass, NewTypeCallInfo, ResolvedModule, Span, TypeNode, TypingForm,
};
use ruff_python_ast::{Expr, Operator};

use crate::diagnostic::{error_diagnostic, error_diagnostic_owned, Diagnostic, ErrorCode};
use crate::rules::shared::ExprIndex;

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "aliases_newtype",
    docs_url: "https://www.basilisk-python.dev/errors/aliases_newtype",
};

fn make_diagnostic(message: String, span: Span, path: &str) -> Diagnostic {
    error_diagnostic(
        CODE.clone(),
        message,
        span,
        path,
        Some("`NewType` requires exactly two arguments: a string name and a concrete base class"),
        Some("PEP 484: `NewType` accepts only proper concrete classes as the base type"),
    )
}

/// The error reason when this base-type expression is invalid for `NewType`.
fn invalid_base_reason(
    base: &Expr,
    typevar_names: &HashSet<&str>,
    typeddict_names: &HashSet<&str>,
) -> Option<&'static str> {
    if matches!(base, Expr::BinOp(binop) if binop.op == Operator::BitOr) {
        return Some("cannot use a union type as a `NewType` base");
    }
    if is_typevar_parameterized(base, typevar_names) {
        return Some("cannot use a TypeVar-parameterized generic as a `NewType` base");
    }
    if matches!(base, Expr::Name(name) if typeddict_names.contains(name.id.as_str())) {
        return Some("cannot use a `TypedDict` class as a `NewType` base");
    }
    None
}

/// Is this a subscript whose type arguments reference a `TypeVar` this module
/// actually declares? A generic parameterized over a concrete class
/// (`list[MyClass]`) is a fine `NewType` base; one that still carries an open
/// `TypeVar` (`list[T]`) is not.
fn is_typevar_parameterized(base: &Expr, typevar_names: &HashSet<&str>) -> bool {
    let Expr::Subscript(subscript) = base else {
        return false;
    };
    slice_references_typevar(&subscript.slice, typevar_names)
}

fn slice_references_typevar(expr: &Expr, typevar_names: &HashSet<&str>) -> bool {
    match expr {
        Expr::Name(name) => typevar_names.contains(name.id.as_str()),
        Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .any(|elt| slice_references_typevar(elt, typevar_names)),
        Expr::Subscript(subscript) => slice_references_typevar(&subscript.slice, typevar_names),
        Expr::BinOp(binop) => {
            slice_references_typevar(&binop.left, typevar_names)
                || slice_references_typevar(&binop.right, typevar_names)
        }
        Expr::Starred(starred) => slice_references_typevar(&starred.value, typevar_names),
        _ => false,
    }
}

fn check_newtype_call(
    info: &NewTypeCallInfo,
    index: &ExprIndex<'_>,
    path: &str,
    typevar_names: &HashSet<&str>,
    typeddict_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Wrong number of arguments
    if info.positional_arg_count != 2 {
        diagnostics.push(make_diagnostic(
            format!(
                "`NewType` takes exactly 2 arguments ({} given) for `{}`",
                info.positional_arg_count, info.lhs_name
            ),
            info.span,
            path,
        ));
        return;
    }

    // Name mismatch: first arg string != LHS name
    if let Some(declared) = &info.declared_name {
        if *declared != info.lhs_name {
            diagnostics.push(make_diagnostic(
                format!(
                    "`NewType` name `{declared}` does not match the variable name `{}`",
                    info.lhs_name
                ),
                info.span,
                path,
            ));
        }
    }

    // Validate the base type node.
    if let Some(base) = info.base_type_span.and_then(|span| index.expr(span)) {
        if let Some(reason) = invalid_base_reason(base, typevar_names, typeddict_names) {
            diagnostics.push(make_diagnostic(
                format!(
                    "Invalid base type for `NewType` `{}`: {reason}",
                    info.lhs_name
                ),
                info.span,
                path,
            ));
        }
    }
}

/// Emits `aliases_newtype` for invalid `NewType(...)` calls.
pub(crate) struct InvalidNewType;

impl Rule for InvalidNewType {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(parsed) = module.lazy_ast.get_or_parse(&module.source, &module.path) else {
            return;
        };
        let index = ExprIndex::build(&parsed.ast);
        let path = &module.path;

        let typevar_names: HashSet<&str> = module
            .typevar_calls
            .iter()
            .filter(|tv| !tv.is_paramspec && !tv.is_typevartuple)
            .map(|tv| tv.name.as_str())
            .collect();
        let typeddict_names: HashSet<&str> = module
            .classes
            .iter()
            .filter(|c| c.is_typed_dict)
            .map(|c| c.name.as_str())
            .chain(module.typeddict_calls.iter().map(|t| t.lhs_name.as_str()))
            .collect();

        for info in &module.newtype_calls {
            check_newtype_call(
                info,
                &index,
                path,
                &typevar_names,
                &typeddict_names,
                diagnostics,
            );
        }

        // Collect all NewType names defined in this module.
        let newtype_names: HashSet<&str> = module
            .newtype_calls
            .iter()
            .map(|nt| nt.lhs_name.as_str())
            .collect();

        if newtype_names.is_empty() {
            return;
        }

        // Map: newtype name → the RESOLVED node of its base type, plus the
        // base's span (used only for diagnostic message text).
        let newtype_base: HashMap<&str, (TypeNode, Span)> = module
            .newtype_calls
            .iter()
            .filter_map(|nt| {
                let span = nt.base_type_span?;
                let base = index.expr(span)?;
                let node = TypeNode::lower(&module.bindings, base);
                Some((nt.lhs_name.as_str(), (node, span)))
            })
            .collect();

        check_newtype_subclassing(module, &newtype_names, diagnostics);
        check_newtype_subscript_uses(module, &index, &newtype_names, diagnostics);
        check_newtype_assigned_to_type(module, &index, &newtype_names, diagnostics);
        check_isinstance_with_newtype(module, &index, &newtype_names, diagnostics);
        check_newtype_call_arg_types(module, &index, &newtype_base, diagnostics);
        check_newtype_var_literal_assignments(module, &index, &newtype_names, diagnostics);
    }
}

/// Subclassing a `NewType` is not allowed (PEP 484).
fn check_newtype_subclassing(
    module: &ResolvedModule,
    newtype_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = &module.path;
    for cls in &module.classes {
        for base in &cls.bases {
            if newtype_names.contains(base.as_str()) {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Class `{}` cannot subclass `{}` which is a `NewType`",
                        cls.name, base
                    ),
                    cls.def_span,
                    path,
                ));
            }
        }
    }
}

/// The `NewType` name subscripted by this annotation, if any: `UserId[int]`.
fn newtype_subscript_name<'e>(expr: &'e Expr, newtype_names: &HashSet<&str>) -> Option<&'e str> {
    let Expr::Subscript(subscript) = expr else {
        return None;
    };
    let Expr::Name(head) = subscript.value.as_ref() else {
        return None;
    };
    newtype_names
        .contains(head.id.as_str())
        .then(|| head.id.as_str())
}

/// Using a `NewType` as a generic subscript (`MyNewType[int]`) is not allowed.
fn check_newtype_subscript_uses(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    newtype_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = &module.path;
    let subscripted = |span: Option<Span>| {
        span.and_then(|span| index.expr(span))
            .and_then(|expr| newtype_subscript_name(expr, newtype_names))
    };
    for func in &module.functions {
        for param in func
            .parameters
            .iter()
            .chain(func.vararg.iter())
            .chain(func.kwarg.iter())
        {
            if subscripted(param.annotation_span).is_some() {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Parameter `{}`: `NewType` cannot be used as a generic type",
                        param.name
                    ),
                    param.name_span,
                    path,
                ));
            }
        }
    }
    for var in &module.module_vars {
        if subscripted(var.annotation_span).is_some() {
            diagnostics.push(make_diagnostic(
                format!(
                    "Variable `{}`: `NewType` cannot be used as a generic type",
                    var.name
                ),
                var.name_span,
                path,
            ));
        }
    }
    for cls in &module.classes {
        for attr in &cls.attributes {
            if subscripted(attr.annotation_span).is_some() {
                diagnostics.push(make_diagnostic(
                    format!(
                        "Attribute `{}`: `NewType` cannot be used as a generic type",
                        attr.name
                    ),
                    attr.name_span,
                    path,
                ));
            }
        }
    }
}

/// `_: type = UserId` — assigning a `NewType` to a `type`-annotated variable is invalid.
///
/// PEP 484: `NewType(...)` does not return a class object; it returns a
/// callable. The annotation is recognised by LOWERING it through the
/// module's bindings ([ASTREBUILD-LAW]): `type`, `builtins.type`, and any
/// alias of them all denote the builtin `type` class; a shadowing
/// definition of the name does not.
fn check_newtype_assigned_to_type(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    newtype_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for var in &module.module_vars {
        let annotation_is_type = var
            .annotation_span
            .and_then(|span| index.expr(span))
            .is_some_and(|expr| {
                TypeNode::lower(&module.bindings, expr) == TypeNode::Builtin(BuiltinClass::Type)
            });
        if !annotation_is_type {
            continue;
        }
        let Some(Expr::Name(rhs)) = var.rhs_span.and_then(|span| index.expr(span)) else {
            continue;
        };
        if newtype_names.contains(rhs.id.as_str()) {
            diagnostics.push(make_diagnostic(
                format!(
                    "`{}` is a `NewType`, not an instance of `type`; \
                     `NewType()` does not return a class object",
                    rhs.id.as_str()
                ),
                var.name_span,
                &module.path,
            ));
        }
    }
}

/// `isinstance(u2, UserId)` — using a `NewType` as the second argument to `isinstance` is invalid.
///
/// PEP 484: the object returned by `NewType(...)` is not a class and cannot be
/// used as the second argument to `isinstance` or `issubclass`. The callee is
/// recognised by what it RESOLVES to ([ASTREBUILD-LAW]) — an aliased import
/// of the builtin is the builtin; a module-level shadowing of its name is not.
fn check_isinstance_with_newtype(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    newtype_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for call in &module.calls {
        let is_runtime_check = index
            .expr(call.span)
            .and_then(|expr| match expr {
                Expr::Call(node) => module.bindings.form_of_with_builtins(&node.func),
                _ => None,
            })
            .is_some_and(|form| {
                matches!(
                    form,
                    TypingForm::IsinstanceFunction | TypingForm::IssubclassFunction
                )
            });
        if !is_runtime_check {
            continue;
        }
        let Some((_, second_span)) = call.args.get(1) else {
            continue;
        };
        let Some(Expr::Name(arg)) = index.expr(*second_span) else {
            continue;
        };
        if newtype_names.contains(arg.id.as_str()) {
            diagnostics.push(make_diagnostic(
                format!(
                    "`{}` is a `NewType` and cannot be used as the second argument \
                     to `isinstance`; `NewType` types are not runtime classes",
                    arg.id.as_str()
                ),
                call.span,
                &module.path,
            ));
        }
    }
}

/// Check calls to `NewType` constructors for argument type mismatches.
///
/// PEP 484: the `NewType` constructor accepts only values of the base type.
/// The argument's literal type is related to the RESOLVED base node through
/// [`assignable`] ([RESOLV-CANONICAL-RELATION]); a diagnostic is emitted
/// only on a definite `Some(false)` — unresolved bases and non-literal
/// arguments abstain.
fn check_newtype_call_arg_types(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    newtype_base: &HashMap<&str, (TypeNode, Span)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for call in &module.calls {
        let Some((base_node, base_span)) = newtype_base.get(call.callee.as_str()) else {
            continue;
        };
        let Some((_, arg_span)) = call.args.first() else {
            continue;
        };
        let Some(arg_expr) = index.expr(*arg_span) else {
            continue;
        };
        let arg_node = TypeNode::of_literal_expr(arg_expr);
        if assignable(&arg_node, base_node) == Some(false) {
            push_newtype_arg_diagnostic(module, call, *arg_span, *base_span, diagnostics);
        }
    }
}

/// The diagnostic for a constructor argument the base type cannot accept.
/// Source text appears in the MESSAGE only — never in a verdict
/// ([ASTREBUILD-LAW]).
fn push_newtype_arg_diagnostic(
    module: &ResolvedModule,
    call: &basilisk_resolver::CallSite,
    arg_span: Span,
    base_span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let arg_text = crate::span_util::slice_span(&module.source, arg_span).unwrap_or("");
    let base_text = crate::span_util::slice_span(&module.source, base_span).unwrap_or("");
    diagnostics.push(error_diagnostic_owned(
        CODE.clone(),
        format!(
            "Argument to `{}` ({}) is not compatible with its base type `{base_text}`",
            call.callee,
            arg_text.trim()
        ),
        call.span,
        &module.path,
        Some(format!(
            "Pass a value of type `{base_text}` to the `{}` constructor",
            call.callee
        )),
        Some("NewType constructors accept only values of the base type (PEP 484)".to_owned()),
    ));
}

/// Check module-level variable assignments where the annotation is a `NewType` name.
///
/// `u1: UserId = 42` is wrong because plain `int` literals are not `UserId` values.
/// Only `UserId(42)` creates a proper `UserId`.
fn check_newtype_var_literal_assignments(
    module: &ResolvedModule,
    index: &ExprIndex<'_>,
    newtype_names: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use basilisk_resolver::RhsKind;

    for var in &module.module_vars {
        let Some(Expr::Name(annotation)) = var.annotation_span.and_then(|span| index.expr(span))
        else {
            continue;
        };
        let ann = annotation.id.as_str();
        if !newtype_names.contains(ann) {
            continue;
        }

        // A literal value can never be a NewType instance — you must call the constructor.
        let is_bare_literal = matches!(
            var.rhs_kind,
            RhsKind::IntLiteral
                | RhsKind::FloatLiteral
                | RhsKind::StrLiteral
                | RhsKind::BytesLiteral
                | RhsKind::BoolLiteral
                | RhsKind::NoneValue
        );

        if is_bare_literal {
            diagnostics.push(error_diagnostic_owned(
                CODE.clone(),
                format!(
                    "Cannot assign a literal value directly to `{ann}`; \
                     use `{ann}(value)` to create a `{ann}` instance"
                ),
                var.name_span,
                &module.path,
                Some(format!("Replace the literal with `{ann}(value)`")),
                Some(
                    "NewType creates a distinct type; only the constructor call is valid"
                        .to_owned(),
                ),
            ));
        }
    }
}
