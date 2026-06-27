//! Implements [`directives_deprecated`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! Expression visitors and helpers for `directives_deprecated`.
//!
//! Contains `visit_expr_for_usage` and all expression-level deprecation checks.

use std::collections::HashMap;

use ruff_python_ast::{Expr, Operator};
use ruff_text_size::Ranged;

use basilisk_resolver::Span;

use crate::diagnostic::Diagnostic;

use super::collect::DeprecatedInfo;
use super::decorators::text_range_to_span;
use super::make_diagnostic;
use super::types::VarType;

/// Map a binary/augmented operator to its dunder method name.
pub(super) fn op_to_dunder(op: Operator) -> &'static str {
    match op {
        Operator::Add => "__add__",
        Operator::Sub => "__sub__",
        Operator::Mult => "__mul__",
        Operator::Div => "__truediv__",
        Operator::Mod => "__mod__",
        Operator::Pow => "__pow__",
        Operator::LShift => "__lshift__",
        Operator::RShift => "__rshift__",
        Operator::BitOr => "__or__",
        Operator::BitXor => "__xor__",
        Operator::BitAnd => "__and__",
        Operator::FloorDiv => "__floordiv__",
        Operator::MatMult => "__matmul__",
    }
}

/// Check if a dunder method on a given inferred type is deprecated; emit a diagnostic if so.
pub(super) fn check_dunder_deprecated_on_type(
    var_type: &VarType,
    dunder: &str,
    deprecated: &HashMap<String, DeprecatedInfo>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
) {
    let class_member_key = format!("{}.{}", var_type.class_name, dunder);

    // Check deprecated members from an imported module.
    if !var_type.module_alias.is_empty() {
        if let Some(members) = deprecated_members.get(&var_type.module_alias) {
            if let Some(info) = members.get(&class_member_key) {
                diagnostics.push(make_diagnostic(
                    span,
                    &info.kind,
                    &class_member_key,
                    info.message.as_deref(),
                    path,
                ));
                return;
            }
        }
    }

    // Check locally-defined deprecated members.
    if let Some(info) = deprecated.get(&class_member_key) {
        diagnostics.push(make_diagnostic(
            span,
            &info.kind,
            &class_member_key,
            info.message.as_deref(),
            path,
        ));
    }
}

/// Check if a property setter on a given inferred type is deprecated; emit a diagnostic if so.
pub(super) fn check_setter_deprecated_on_type(
    var_type: &VarType,
    member_name: &str,
    deprecated: &HashMap<String, DeprecatedInfo>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
) {
    let class_member_key = format!("{}.{}", var_type.class_name, member_name);

    // Check deprecated members from an imported module.
    if !var_type.module_alias.is_empty() {
        if let Some(members) = deprecated_members.get(&var_type.module_alias) {
            if let Some(info) = members.get(&class_member_key) {
                if info.kind == "property setter" {
                    diagnostics.push(make_diagnostic(
                        span,
                        &info.kind,
                        &class_member_key,
                        info.message.as_deref(),
                        path,
                    ));
                    return;
                }
            }
        }
    }

    // Check locally-defined deprecated members.
    if let Some(info) = deprecated.get(&class_member_key) {
        if info.kind == "property setter" {
            diagnostics.push(make_diagnostic(
                span,
                &info.kind,
                &class_member_key,
                info.message.as_deref(),
                path,
            ));
        }
    }
}

/// Visit an expression to find deprecated name usages.
#[expect(
    clippy::too_many_lines,
    reason = "expression visitor covers all expression variants"
)]
pub(super) fn visit_expr_for_usage(
    expr: &Expr,
    deprecated: &HashMap<String, DeprecatedInfo>,
    module_aliases: &HashMap<String, String>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    var_types: &HashMap<String, VarType>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        // Direct name reference: `lorem` or as the callee of `lorem()`
        Expr::Name(name) => {
            if let Some(info) = deprecated.get(name.id.as_str()) {
                diagnostics.push(make_diagnostic(
                    text_range_to_span(name.range()),
                    &info.kind,
                    name.id.as_str(),
                    info.message.as_deref(),
                    path,
                ));
            }
        }
        // Function/method call
        Expr::Call(call) => {
            match call.func.as_ref() {
                Expr::Name(name) => {
                    if let Some(info) = deprecated.get(name.id.as_str()) {
                        // Deprecated function/class called directly.
                        diagnostics.push(make_diagnostic(
                            text_range_to_span(call.range()),
                            &info.kind,
                            name.id.as_str(),
                            info.message.as_deref(),
                            path,
                        ));
                    } else {
                        // Check if calling an instance whose class has a deprecated __call__.
                        let var_name = name.id.as_str();
                        if let Some(var_type) = var_types.get(var_name) {
                            check_dunder_deprecated_on_type(
                                var_type,
                                "__call__",
                                deprecated,
                                deprecated_members,
                                path,
                                diagnostics,
                                text_range_to_span(call.range()),
                            );
                        }
                    }
                }
                Expr::Attribute(attr) => {
                    // Attribute-style call: `library.func()`, `spam.method()`, `f.foo()`
                    let mut handled = false;
                    if let Expr::Name(obj_name) = attr.value.as_ref() {
                        let var_name = obj_name.id.as_str();
                        let member_name = attr.attr.as_str();
                        if let Some(var_type) = var_types.get(var_name) {
                            // Instance method call: look up ClassName.method.
                            let key = format!("{}.{}", var_type.class_name, member_name);
                            if !var_type.module_alias.is_empty() {
                                if let Some(members) =
                                    deprecated_members.get(&var_type.module_alias)
                                {
                                    if let Some(info) = members.get(&key) {
                                        diagnostics.push(make_diagnostic(
                                            text_range_to_span(call.range()),
                                            &info.kind,
                                            member_name,
                                            info.message.as_deref(),
                                            path,
                                        ));
                                        handled = true;
                                    }
                                }
                            }
                            if !handled {
                                if let Some(info) = deprecated.get(&key) {
                                    diagnostics.push(make_diagnostic(
                                        text_range_to_span(call.range()),
                                        &info.kind,
                                        member_name,
                                        info.message.as_deref(),
                                        path,
                                    ));
                                    handled = true;
                                }
                            }
                        }
                    }
                    if !handled {
                        let first_arg_type = call
                            .arguments
                            .args
                            .first()
                            .and_then(crate::rules::shared::infer_expr_literal_type);
                        check_attribute_deprecated(
                            attr,
                            deprecated,
                            module_aliases,
                            deprecated_members,
                            var_types,
                            path,
                            diagnostics,
                            Some(text_range_to_span(call.range())),
                            first_arg_type,
                        );
                    }
                }
                _ => {
                    visit_expr_for_usage(
                        call.func.as_ref(),
                        deprecated,
                        module_aliases,
                        deprecated_members,
                        var_types,
                        path,
                        diagnostics,
                    );
                }
            }
            // Visit call arguments.
            for arg in &call.arguments.args {
                visit_expr_for_usage(
                    arg,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    var_types,
                    path,
                    diagnostics,
                );
            }
        }
        // Attribute access: `spam.greasy`, `library.norwegian_blue`
        Expr::Attribute(attr) => {
            // Check for deprecated property/method access on an inferred-type variable.
            let mut handled = false;
            if let Expr::Name(obj_name) = attr.value.as_ref() {
                let var_name = obj_name.id.as_str();
                let member_name = attr.attr.as_str();
                if let Some(var_type) = var_types.get(var_name) {
                    let key = format!("{}.{}", var_type.class_name, member_name);
                    // Only flag property getters or methods here; setters are handled on assignment.
                    if !var_type.module_alias.is_empty() {
                        if let Some(members) = deprecated_members.get(&var_type.module_alias) {
                            if let Some(info) = members.get(&key) {
                                if info.kind == "property" || info.kind == "method" {
                                    diagnostics.push(make_diagnostic(
                                        text_range_to_span(attr.range()),
                                        &info.kind,
                                        &key,
                                        info.message.as_deref(),
                                        path,
                                    ));
                                    handled = true;
                                }
                            }
                        }
                    }
                    if !handled {
                        if let Some(info) = deprecated.get(&key) {
                            if info.kind == "property" || info.kind == "method" {
                                diagnostics.push(make_diagnostic(
                                    text_range_to_span(attr.range()),
                                    &info.kind,
                                    &key,
                                    info.message.as_deref(),
                                    path,
                                ));
                                handled = true;
                            }
                        }
                    }
                }
            }
            if !handled {
                check_attribute_deprecated(
                    attr,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    var_types,
                    path,
                    diagnostics,
                    None,
                    None,
                );
            }
        }
        // Binary operations: `spam + 1` triggers `__add__`
        Expr::BinOp(binop) => {
            check_binop_deprecated(
                &binop.left,
                binop.op,
                deprecated,
                deprecated_members,
                var_types,
                path,
                diagnostics,
                text_range_to_span(binop.range()),
            );
            visit_expr_for_usage(
                &binop.left,
                deprecated,
                module_aliases,
                deprecated_members,
                var_types,
                path,
                diagnostics,
            );
            visit_expr_for_usage(
                &binop.right,
                deprecated,
                module_aliases,
                deprecated_members,
                var_types,
                path,
                diagnostics,
            );
        }
        Expr::Tuple(t) => {
            for elt in &t.elts {
                visit_expr_for_usage(
                    elt,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    var_types,
                    path,
                    diagnostics,
                );
            }
        }
        Expr::List(l) => {
            for elt in &l.elts {
                visit_expr_for_usage(
                    elt,
                    deprecated,
                    module_aliases,
                    deprecated_members,
                    var_types,
                    path,
                    diagnostics,
                );
            }
        }
        _ => {}
    }
}

/// Returns true if `info` describes a deprecated overload whose parameter type
/// does not match the call's first-argument type — meaning the call resolves to
/// a different, non-deprecated overload and must NOT be flagged (PEP 702).
///
/// Conservative: returns false (i.e. keep flagging) unless this is an overload
/// AND both the expected parameter type and the actual argument type are known
/// AND they are incompatible.
fn overload_arg_mismatch(info: &DeprecatedInfo, first_arg_type: Option<&str>) -> bool {
    if info.kind != "overload" {
        return false;
    }
    match (info.overload_param_type.as_deref(), first_arg_type) {
        (Some(expected), Some(actual)) => {
            !crate::rules::shared::is_type_compatible(actual, expected)
        }
        _ => false,
    }
}

/// Check if an attribute access refers to a deprecated member (module-level or qualified).
#[expect(
    clippy::too_many_arguments,
    reason = "attribute deprecation check requires full context"
)]
fn check_attribute_deprecated(
    attr: &ruff_python_ast::ExprAttribute,
    deprecated: &HashMap<String, DeprecatedInfo>,
    module_aliases: &HashMap<String, String>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    var_types: &HashMap<String, VarType>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    call_span: Option<Span>,
    first_arg_type: Option<&str>,
) {
    let member_name = attr.attr.as_str();
    let span = call_span.unwrap_or_else(|| text_range_to_span(attr.range()));

    if let Expr::Name(value_name) = attr.value.as_ref() {
        let alias = value_name.id.as_str();

        // Case 1: `library.func_name` where `library` is a module alias with deprecated members.
        if let Some(members) = deprecated_members.get(alias) {
            if let Some(info) = members.get(member_name) {
                // Per PEP 702, a call to an overloaded name is only deprecated
                // if it resolves to the deprecated overload. Skip when the
                // first argument's type doesn't match the deprecated overload's.
                if !overload_arg_mismatch(info, first_arg_type) {
                    diagnostics.push(make_diagnostic(
                        span,
                        &info.kind,
                        member_name,
                        info.message.as_deref(),
                        path,
                    ));
                    return;
                }
            }
        }

        // Case 2: local qualified name like `ClassName.method` matches a deprecated key directly.
        let qualified = format!("{alias}.{member_name}");
        if let Some(info) = deprecated.get(&qualified) {
            diagnostics.push(make_diagnostic(
                span,
                &info.kind,
                member_name,
                info.message.as_deref(),
                path,
            ));
            return;
        }

        // Case 3: `alias` is a typed variable; look up its class's deprecated members.
        // Only flag read-access deprecations (property getters and methods), not
        // property setters — setter deprecation is handled on assignment targets.
        if let Some(var_type) = var_types.get(alias) {
            let key = format!("{}.{}", var_type.class_name, member_name);
            if !var_type.module_alias.is_empty() {
                if let Some(members) = deprecated_members.get(&var_type.module_alias) {
                    if let Some(info) = members.get(&key) {
                        if info.kind == "property" || info.kind == "method" {
                            diagnostics.push(make_diagnostic(
                                span,
                                &info.kind,
                                member_name,
                                info.message.as_deref(),
                                path,
                            ));
                            return;
                        }
                    }
                }
            }
            if let Some(info) = deprecated.get(&key) {
                if info.kind == "property" || info.kind == "method" {
                    diagnostics.push(make_diagnostic(
                        span,
                        &info.kind,
                        member_name,
                        info.message.as_deref(),
                        path,
                    ));
                    return;
                }
            }
        }
    }

    // Recurse into the value expression for chained access.
    visit_expr_for_usage(
        attr.value.as_ref(),
        deprecated,
        module_aliases,
        deprecated_members,
        var_types,
        path,
        diagnostics,
    );
}

/// Check if a binary operation triggers a deprecated dunder method on the left operand.
#[expect(
    clippy::too_many_arguments,
    reason = "binary op deprecation check requires full context"
)]
fn check_binop_deprecated(
    left: &Expr,
    op: Operator,
    deprecated: &HashMap<String, DeprecatedInfo>,
    deprecated_members: &HashMap<String, HashMap<String, DeprecatedInfo>>,
    var_types: &HashMap<String, VarType>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
) {
    let dunder = op_to_dunder(op);
    if let Expr::Name(name) = left {
        let var_name = name.id.as_str();
        if let Some(var_type) = var_types.get(var_name) {
            check_dunder_deprecated_on_type(
                var_type,
                dunder,
                deprecated,
                deprecated_members,
                path,
                diagnostics,
                span,
            );
        }
    }
}
