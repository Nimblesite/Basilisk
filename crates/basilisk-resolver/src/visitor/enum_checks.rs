//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! Enum Checks visitor functions.

use ruff_python_ast::{Expr, Stmt, StmtClassDef, StmtFunctionDef};
use ruff_text_size::Ranged;

use crate::scope::LiteralStringEnumMismatch;

use super::class_info_ext::expr_simple_name;
use super::core::{source_slice_range, text_range_to_span};
use super::ENUM_BASES;

pub(super) fn collect_enum_value_type_violations(
    stmts: &[Stmt],
    source: &str,
) -> Vec<crate::scope::EnumValueTypeViolationInfo> {
    let mut out = Vec::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else { continue };
        if !stmt_class_is_direct_enum(cls) {
            continue;
        }
        let Some(declared_type) = find_enum_value_annotation(cls, source) else {
            continue;
        };
        let class_name = cls.name.to_string();
        check_enum_member_values(cls, source, &declared_type, &class_name, &mut out);
        check_enum_init_value_param(cls, source, &declared_type, &class_name, &mut out);
    }
    out
}

/// Checks member value assignments in an enum class body against `_value_: T`.
pub(super) fn check_enum_member_values(
    cls: &StmtClassDef,
    _source: &str,
    declared_type: &str,
    class_name: &str,
    out: &mut Vec<crate::scope::EnumValueTypeViolationInfo>,
) {
    use crate::scope::{EnumValueTypeViolationInfo, EnumValueTypeViolationKind};
    for body_stmt in &cls.body {
        let Stmt::Assign(assign) = body_stmt else {
            continue;
        };
        if assign.targets.len() != 1 {
            continue;
        }
        let Some(Expr::Name(name_expr)) = assign.targets.first() else {
            continue;
        };
        let member_name = name_expr.id.as_str();
        // Skip dunder and special names.
        if member_name.starts_with("__") || member_name == "_value_" {
            continue;
        }
        let Some(actual_type) = infer_member_literal_type(assign.value.as_ref()) else {
            continue;
        };
        if !enum_types_compatible(declared_type, &actual_type) {
            out.push(EnumValueTypeViolationInfo {
                kind: EnumValueTypeViolationKind::MemberValueTypeMismatch,
                span: text_range_to_span(assign.value.range()),
                class_name: class_name.to_owned(),
                declared_type: declared_type.to_owned(),
                actual_type,
            });
        }
    }
}

/// Checks `self._value_ = param` assignments in `__init__` against `_value_: T`.
pub(super) fn check_enum_init_value_param(
    cls: &StmtClassDef,
    source: &str,
    declared_type: &str,
    class_name: &str,
    out: &mut Vec<crate::scope::EnumValueTypeViolationInfo>,
) {
    for body_stmt in &cls.body {
        let Stmt::FunctionDef(func) = body_stmt else {
            continue;
        };
        if func.name.as_str() != "__init__" {
            continue;
        }
        // Build parameter name -> annotation text map.
        let params: Vec<(&str, &str)> = func
            .parameters
            .posonlyargs
            .iter()
            .chain(func.parameters.args.iter())
            .filter_map(|p| {
                if p.parameter.name.as_str() == "self" {
                    return None;
                }
                let ann_expr = p.parameter.annotation.as_deref()?;
                let range = ann_expr.range();
                let ann_text = source_slice_range(source, range)?.trim();
                Some((p.parameter.name.as_str(), ann_text))
            })
            .collect();

        for init_stmt in &func.body {
            check_init_self_value_assign(
                init_stmt,
                &params,
                source,
                declared_type,
                class_name,
                out,
            );
        }
    }
}

/// Checks a single `__init__` statement for `self._value_ = param` pattern.
pub(super) fn check_init_self_value_assign<'a>(
    stmt: &Stmt,
    params: &[(&'a str, &'a str)],
    _source: &str,
    declared_type: &str,
    class_name: &str,
    out: &mut Vec<crate::scope::EnumValueTypeViolationInfo>,
) {
    use crate::scope::{EnumValueTypeViolationInfo, EnumValueTypeViolationKind};
    let Stmt::Assign(assign) = stmt else { return };
    if assign.targets.len() != 1 {
        return;
    }
    let Some(Expr::Attribute(attr)) = assign.targets.first() else {
        return;
    };
    if attr.attr.as_str() != "_value_" {
        return;
    }
    let Expr::Name(self_name) = attr.value.as_ref() else {
        return;
    };
    if self_name.id.as_str() != "self" {
        return;
    }
    // Found self._value_ = <expr>; check if RHS is a parameter name.
    let Expr::Name(rhs_name) = assign.value.as_ref() else {
        return;
    };
    let param_name = rhs_name.id.as_str();
    let Some((_, ann_text)) = params.iter().find(|(n, _)| *n == param_name) else {
        return;
    };
    let actual_type = ann_text.trim().to_owned();
    if !enum_types_compatible(declared_type, &actual_type) {
        out.push(EnumValueTypeViolationInfo {
            kind: EnumValueTypeViolationKind::InitValueParamTypeMismatch,
            span: text_range_to_span(assign.range()),
            class_name: class_name.to_owned(),
            declared_type: declared_type.to_owned(),
            actual_type,
        });
    }
}

/// Returns `true` when the class directly inherits from one of the standard enum bases.
pub(super) fn stmt_class_is_direct_enum(cls: &StmtClassDef) -> bool {
    let Some(args) = cls.arguments.as_deref() else {
        return false;
    };
    args.args.iter().any(|base| {
        if let Expr::Name(n) = base {
            ENUM_BASES.contains(&n.id.as_str())
        } else {
            false
        }
    })
}

/// Finds an `_value_: T` annotation-only declaration in the enum class body.
///
/// Returns the annotation text (e.g. `"int"`) if found, `None` otherwise.
pub(super) fn find_enum_value_annotation(cls: &StmtClassDef, source: &str) -> Option<String> {
    for stmt in &cls.body {
        let Stmt::AnnAssign(ann) = stmt else {
            continue;
        };
        if ann.value.is_some() {
            continue; // Must be annotation-only (no initializer).
        }
        let Expr::Name(n) = ann.target.as_ref() else {
            continue;
        };
        if n.id != "_value_" {
            continue;
        }
        let range = ann.annotation.range();
        let ann_text = source_slice_range(source, range)?.trim().to_owned();
        return Some(ann_text);
    }
    None
}

/// Infers the primitive type name of a literal expression.
///
/// Returns `None` for non-literal expressions (tuples, calls, names, etc.) so
/// that only clearly-typed literals are checked against `_value_: T`.
pub(super) fn infer_member_literal_type(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StringLiteral(_) => Some("str".to_owned()),
        Expr::NumberLiteral(n) => {
            if matches!(n.value, ruff_python_ast::Number::Float(_)) {
                Some("float".to_owned())
            } else if matches!(n.value, ruff_python_ast::Number::Complex { .. }) {
                Some("complex".to_owned())
            } else {
                Some("int".to_owned())
            }
        }
        Expr::BooleanLiteral(_) => Some("bool".to_owned()),
        Expr::NoneLiteral(_) => Some("None".to_owned()),
        Expr::BytesLiteral(_) => Some("bytes".to_owned()),
        _ => None,
    }
}

/// Returns `true` when `actual` is compatible with `declared` as an enum `_value_` type.
///
/// - Identical types are always compatible.
/// - `bool` is compatible with `int` (bool is a subclass of int).
/// - `bool` and `int` are compatible with `float` (numeric tower).
pub(super) fn enum_types_compatible(declared: &str, actual: &str) -> bool {
    if declared == actual {
        return true;
    }
    // bool is a subtype of int.
    if declared == "int" && actual == "bool" {
        return true;
    }
    // int and bool are compatible with float (PEP 3141 numeric tower).
    if declared == "float" && (actual == "int" || actual == "bool") {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Float parameter int-attr access collection (stub)
// ---------------------------------------------------------------------------

// Collect attribute accesses of `int`-only attributes on `float`-typed parameters.
// Full implementation is pending — returns empty for now.

// ---------------------------------------------------------------------------
// Literal string / enum member mismatch collection (stub)
// ---------------------------------------------------------------------------

// Collect `Literal["X.Y"]` vs `Literal[X.Y]` mismatches.
// Full implementation is pending — returns empty for now.

// ---------------------------------------------------------------------------
// Float parameter int-only attribute access collection
// ---------------------------------------------------------------------------

/// `int`-only attributes that are invalid to access on a `float`-typed parameter.
///
/// `numerator` and `denominator` are defined on `int` but not on `float`.
pub(super) const INT_ONLY_FLOAT_ATTRS: &[&str] = &["numerator", "denominator"];

/// Collect attribute accesses of `int`-only attributes on `float`-typed parameters.
///
/// Only top-level statements of each function body are examined — accesses inside
/// `if`/`for`/`while`/`match`/`with`/`try` blocks are excluded so that
/// `isinstance`-guarded paths (where the parameter has been narrowed to `int`) are
/// not flagged.
pub(crate) fn collect_literal_string_enum_mismatches(
    stmts: &[Stmt],
    source: &str,
) -> Vec<LiteralStringEnumMismatch> {
    let mut out = Vec::new();
    for stmt in stmts {
        if let Stmt::FunctionDef(func) = stmt {
            collect_literal_mismatches_in_function(func, source, &mut out);
            // Recurse into nested functions.
            out.extend(collect_literal_string_enum_mismatches(&func.body, source));
        }
    }
    out
}

/// Returns the inner content of `Literal[...]` if `ann` starts with `Literal[` and ends with `]`.
pub(super) fn literal_inner_content(ann: &str) -> Option<&str> {
    let ann = ann.trim();
    let inner_start = ann.find("Literal[")? + "Literal[".len();
    let body = &ann[inner_start..];
    body.strip_suffix(']')
}

/// Returns `true` when `s` is an enum member form: `Identifier.Identifier` (no spaces,
/// no quotes, no brackets, exactly one dot, both parts are simple identifiers).
pub(super) fn is_enum_member_form(s: &str) -> bool {
    let s = s.trim();
    if s.contains(' ') || s.contains('[') || s.contains('(') || s.contains('"') || s.contains('\'')
    {
        return false;
    }
    let mut parts = s.splitn(2, '.');
    let Some(obj) = parts.next() else {
        return false;
    };
    let Some(attr) = parts.next() else {
        return false;
    };
    !attr.contains('.') && is_simple_ident(obj) && is_simple_ident(attr)
}

pub(super) fn is_simple_ident(s: &str) -> bool {
    crate::is_simple_python_identifier(s)
}

/// Returns `true` when `s` is a quoted string whose body equals `enum_form`.
///
/// E.g. `is_quoted_string_of("\"Color.RED\"", "Color.RED")` → `true`.
pub(super) fn is_quoted_string_of(s: &str, enum_form: &str) -> bool {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        return inner == enum_form;
    }
    if let Some(inner) = s.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        return inner == enum_form;
    }
    false
}

pub(super) fn collect_literal_mismatches_in_function(
    func: &StmtFunctionDef,
    source: &str,
    out: &mut Vec<LiteralStringEnumMismatch>,
) {
    // Build a list of (param_name, enum_form) pairs for parameters annotated as
    // `Literal[X.Y]` where X.Y is an enum member (e.g. `Literal[Color.RED]`).
    let param_enum_literals: Vec<(&str, &str)> = super::walks::iter_all_params(&func.parameters)
        .filter_map(|p| {
            let ann_expr = p.parameter.annotation.as_deref()?;
            let range = ann_expr.range();
            let ann_text = source_slice_range(source, range)?.trim();
            let inner = literal_inner_content(ann_text)?;
            let inner = inner.trim();
            if is_enum_member_form(inner) {
                Some((p.parameter.name.as_str(), inner))
            } else {
                None
            }
        })
        .collect();

    if param_enum_literals.is_empty() {
        return;
    }

    // Walk only the top-level statements of the function body.
    for stmt in &func.body {
        let Stmt::AnnAssign(ann_assign) = stmt else {
            continue;
        };
        // The RHS must be a simple name referring to a parameter.
        let Some(value_expr) = ann_assign.value.as_deref() else {
            continue;
        };
        let Some(rhs_name) = expr_simple_name(value_expr) else {
            continue;
        };

        // Find the enum form for this parameter.
        let Some((_param, enum_form)) = param_enum_literals
            .iter()
            .find(|(param, _)| *param == rhs_name.as_str())
        else {
            continue;
        };

        // Extract the annotation text of the LHS variable.
        let ann_range = ann_assign.annotation.range();
        let Some(ann_text) = source_slice_range(source, ann_range) else {
            continue;
        };
        let ann_text = ann_text.trim();

        // The annotation must be `Literal["X.Y"]` where the inner string equals the enum form.
        let Some(inner) = literal_inner_content(ann_text) else {
            continue;
        };
        let inner = inner.trim();
        if !is_quoted_string_of(inner, enum_form) {
            continue;
        }

        // Extract the variable name span.
        let Expr::Name(lhs_name) = ann_assign.target.as_ref() else {
            continue;
        };
        out.push(LiteralStringEnumMismatch {
            var_name: lhs_name.id.as_str().to_owned(),
            annotation: ann_text.to_owned(),
            enum_form: (*enum_form).to_owned(),
            span: text_range_to_span(lhs_name.range()),
        });
    }
}

// ---------------------------------------------------------------------------
// Module-level attribute access collection
// ---------------------------------------------------------------------------
