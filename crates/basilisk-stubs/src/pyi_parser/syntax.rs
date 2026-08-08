//! Conversion of parsed Python syntax into stub declaration values.

use basilisk_canonical::{BindingTable, TypingForm};
use ruff_python_ast::{self as ast, Decorator, Expr, Parameters, StmtAnnAssign, StmtFunctionDef};

use crate::types::{StubFunction, StubParam, StubParamKind, StubSpan};

/// Does any decorator on this definition denote `form`?
///
/// The question is the decorator **node**; the answer is a [`TypingForm`]. A
/// stub writing `from typing import overload as _ov` resolves identically to
/// one writing `@overload`, and a stub that defines its own `overload` does
/// not resolve at all.
pub(super) fn has_decorator_form(
    bindings: &BindingTable,
    decorators: &[Decorator],
    form: TypingForm,
) -> bool {
    decorators
        .iter()
        .any(|decorator| bindings.form_of(&decorator.expression) == Some(form))
}

/// Does any decorator on this definition denote the builtin named `builtin`?
///
/// `staticmethod` and `classmethod` need no import, so no registry entry
/// describes them and [`BindingTable::form_of`] cannot answer. Recognition is
/// therefore a bare `Name` node carrying that identifier, and only while the
/// module has not bound the name to something of its own — which
/// [`BindingTable::binds_name`] is what decides. No source text is consulted.
pub(super) fn has_builtin_decorator(
    bindings: &BindingTable,
    decorators: &[Decorator],
    builtin: &str,
) -> bool {
    if bindings.binds_name(builtin) {
        return false;
    }
    decorators.iter().any(|decorator| {
        matches!(&decorator.expression, Expr::Name(name) if name.id.as_str() == builtin)
    })
}

pub(super) fn ann_assign_target_name(ann: &StmtAnnAssign) -> Option<String> {
    if let Expr::Name(name_expr) = ann.target.as_ref() {
        Some(name_expr.id.to_string())
    } else {
        None
    }
}

pub(super) fn stub_method(
    function: &StmtFunctionDef,
    class_name: &str,
    bindings: &BindingTable,
) -> StubFunction {
    let decorators = extract_decorator_names(&function.decorator_list);
    let mut params = extract_params(&function.parameters);
    // A static method has no receiver to strip; every other method binds its
    // first parameter as one.
    let receiver = if has_builtin_decorator(bindings, &function.decorator_list, "staticmethod")
        || params.is_empty()
    {
        None
    } else {
        Some(params.remove(0))
    };
    StubFunction {
        name: function.name.to_string(),
        receiver,
        params,
        return_type: function
            .returns
            .as_ref()
            .map(|annotation| expr_to_annotation(annotation)),
        is_async: function.is_async,
        decorators,
        class_name: Some(class_name.to_owned()),
        source_span: StubSpan {
            start: function.name.range.start().into(),
            end: function.name.range.end().into(),
        },
    }
}

pub(super) fn extract_decorator_names(decorators: &[Decorator]) -> Vec<String> {
    decorators
        .iter()
        .map(|decorator| expr_to_decorator_name(&decorator.expression))
        .collect()
}

fn expr_to_decorator_name(expr: &Expr) -> String {
    match expr {
        Expr::Name(name) => name.id.to_string(),
        Expr::Attribute(attribute) => {
            let prefix = expr_to_decorator_name(&attribute.value);
            format!("{prefix}.{}", attribute.attr)
        }
        Expr::Call(call) => expr_to_decorator_name(&call.func),
        _ => String::new(),
    }
}

pub(super) fn expr_to_annotation(expr: &Expr) -> String {
    match expr {
        Expr::Name(name) => name.id.to_string(),
        Expr::NoneLiteral(_) => "None".to_owned(),
        Expr::EllipsisLiteral(_) => "...".to_owned(),
        Expr::Attribute(attribute) => {
            let prefix = expr_to_annotation(&attribute.value);
            format!("{prefix}.{}", attribute.attr)
        }
        Expr::Subscript(subscript) => {
            let base = expr_to_annotation(&subscript.value);
            let slice = expr_to_annotation(&subscript.slice);
            format!("{base}[{slice}]")
        }
        Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .map(expr_to_annotation)
            .collect::<Vec<_>>()
            .join(", "),
        Expr::List(list) => format!(
            "[{}]",
            list.elts
                .iter()
                .map(expr_to_annotation)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Expr::BinOp(binary) if matches!(binary.op, ast::Operator::BitOr) => {
            let left = expr_to_annotation(&binary.left);
            let right = expr_to_annotation(&binary.right);
            format!("{left} | {right}")
        }
        Expr::StringLiteral(string) => format!("\"{}\"", string.value),
        Expr::NumberLiteral(number) => match &number.value {
            ast::Number::Int(value) => value.to_string(),
            ast::Number::Float(value) => value.to_string(),
            ast::Number::Complex { real, imag } => format!("{real}+{imag}j"),
        },
        Expr::BooleanLiteral(boolean) => {
            if boolean.value {
                "True".to_owned()
            } else {
                "False".to_owned()
            }
        }
        Expr::Starred(starred) => format!("*{}", expr_to_annotation(&starred.value)),
        _ => "Unknown".to_owned(),
    }
}

pub(super) fn extract_params(params: &Parameters) -> Vec<StubParam> {
    let mut result = Vec::new();
    result.extend(
        params
            .posonlyargs
            .iter()
            .map(|param| param_to_stub_param(param, StubParamKind::PositionalOnly)),
    );
    result.extend(
        params
            .args
            .iter()
            .map(|param| param_to_stub_param(param, StubParamKind::Regular)),
    );
    if let Some(vararg) = &params.vararg {
        result.push(StubParam {
            name: vararg.name.to_string(),
            annotation: vararg
                .annotation
                .as_ref()
                .map(|annotation| expr_to_annotation(annotation)),
            has_default: false,
            kind: StubParamKind::Vararg,
        });
    }
    result.extend(
        params
            .kwonlyargs
            .iter()
            .map(|param| param_to_stub_param(param, StubParamKind::KeywordOnly)),
    );
    if let Some(kwarg) = &params.kwarg {
        result.push(StubParam {
            name: kwarg.name.to_string(),
            annotation: kwarg
                .annotation
                .as_ref()
                .map(|annotation| expr_to_annotation(annotation)),
            has_default: false,
            kind: StubParamKind::Kwarg,
        });
    }
    result
}

fn param_to_stub_param(param: &ast::ParameterWithDefault, kind: StubParamKind) -> StubParam {
    StubParam {
        name: param.parameter.name.to_string(),
        annotation: param
            .parameter
            .annotation
            .as_ref()
            .map(|annotation| expr_to_annotation(annotation)),
        has_default: param.default.is_some(),
        kind,
    }
}
