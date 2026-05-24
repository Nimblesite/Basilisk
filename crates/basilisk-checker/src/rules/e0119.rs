//! Implements [BSK-E0119] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! BSK-E0119: Protocol `isinstance`/`issubclass` violations.
//!
//! Per PEP 544:
//! - A protocol can be used as the second argument to `isinstance()` or
//!   `issubclass()` **only** if it is decorated with `@runtime_checkable`.
//! - `issubclass()` can only be used with **non-data** protocols (protocols
//!   that define only methods, not data attributes).
//! - Type checkers should reject an `isinstance()` or `issubclass()` call if
//!   there is an unsafe overlap between the type of the first argument and
//!   the protocol.
//!
//! ```python
//! from typing import Protocol, runtime_checkable
//!
//! class Proto1(Protocol):
//!     name: str
//!
//! @runtime_checkable
//! class Proto2(Protocol):
//!     name: str
//!     def method(self) -> int: ...
//!
//! isinstance(x, Proto1)            # E — not @runtime_checkable
//! issubclass(x, Proto2)            # E — data protocol in issubclass
//! isinstance(Concrete(), Proto3)   # E — unsafe overlap
//! ```

use basilisk_resolver::scope::Span;
use basilisk_resolver::ResolvedModule;
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0119",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0119",
};

/// Emits BSK-E0119 for protocol `isinstance`/`issubclass` violations:
/// not-runtime-checkable, data protocol with issubclass, and unsafe overlap.
pub(crate) struct ProtocolUnsafeOverlap;

/// Extract the class name from an expression that is either a constructor call
/// `ClassName()` or a bare name `ClassName`.
fn extract_class_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Call(call) => {
            if let Expr::Name(name) = call.func.as_ref() {
                Some(name.id.as_str())
            } else {
                None
            }
        }
        Expr::Name(name) => Some(name.id.as_str()),
        _ => None,
    }
}

/// Extract the source text for a span.
fn span_text(source: &str, span: Span) -> &str {
    let start = span.start_usize();
    let end = span.end_usize();
    source.get(start..end).unwrap_or_default()
}

/// Describes a member of a class relevant to protocol overlap checks.
#[derive(Debug)]
enum MemberKind {
    /// A method with parameter annotation texts (excluding `self`) and return annotation text.
    Method {
        param_annotations: Vec<String>,
        return_annotation: String,
    },
    /// A data attribute (not a method).
    Attribute,
}

/// Collect members of a class by name.
fn collect_class_members(
    class_name: &str,
    module: &ResolvedModule,
) -> std::collections::HashMap<String, MemberKind> {
    let mut members = std::collections::HashMap::new();

    for func in &module.functions {
        if func.class_name.as_deref() != Some(class_name) {
            continue;
        }
        let param_annotations: Vec<String> = func
            .parameters
            .iter()
            .filter(|p| p.name != "self")
            .map(|p| {
                p.annotation_span
                    .as_ref()
                    .map(|span| span_text(&module.source, *span).to_owned())
                    .unwrap_or_default()
            })
            .collect();

        let return_annotation = func
            .return_annotation_span
            .as_ref()
            .map(|span| span_text(&module.source, *span).to_owned())
            .unwrap_or_default();

        let _ = members.insert(
            func.name.clone(),
            MemberKind::Method {
                param_annotations,
                return_annotation,
            },
        );
    }

    for class in &module.classes {
        if class.name != class_name {
            continue;
        }
        for attr in &class.attributes {
            let _ = members
                .entry(attr.name.clone())
                .or_insert(MemberKind::Attribute);
        }
    }

    members
}

/// Check if a concrete class has an unsafe overlap with a protocol.
fn has_unsafe_overlap(concrete_class: &str, protocol_class: &str, module: &ResolvedModule) -> bool {
    let concrete_members = collect_class_members(concrete_class, module);
    let protocol_members = collect_class_members(protocol_class, module);

    if concrete_members.is_empty() {
        return false;
    }

    for (name, proto_kind) in &protocol_members {
        let Some(concrete_kind) = concrete_members.get(name) else {
            continue;
        };

        match (proto_kind, concrete_kind) {
            (
                MemberKind::Method {
                    param_annotations: proto_params,
                    return_annotation: proto_ret,
                },
                MemberKind::Method {
                    param_annotations: concrete_params,
                    return_annotation: concrete_ret,
                },
            ) => {
                if proto_params.len() != concrete_params.len() {
                    return true;
                }
                for (proto_p, concrete_p) in proto_params.iter().zip(concrete_params.iter()) {
                    if !proto_p.is_empty() && !concrete_p.is_empty() && proto_p != concrete_p {
                        return true;
                    }
                }
                if !proto_ret.is_empty() && !concrete_ret.is_empty() && proto_ret != concrete_ret {
                    return true;
                }
            }
            (MemberKind::Method { .. }, MemberKind::Attribute)
            | (MemberKind::Attribute, MemberKind::Method { .. }) => {
                return true;
            }
            (MemberKind::Attribute, MemberKind::Attribute) => {}
        }
    }

    false
}

/// Check if a class is a protocol (has `Protocol` in its bases).
fn is_protocol(class_name: &str, module: &ResolvedModule) -> bool {
    module
        .classes
        .iter()
        .any(|cls| cls.name == class_name && cls.bases.iter().any(|b| b == "Protocol"))
}

/// Check if a protocol is decorated with `@runtime_checkable`.
fn is_runtime_checkable(class_name: &str, module: &ResolvedModule) -> bool {
    module.classes.iter().any(|cls| {
        cls.name == class_name
            && cls
                .decorator_spans
                .iter()
                .any(|(name, _)| name == "runtime_checkable")
    })
}

/// Check if a protocol is a data protocol (has data attributes, not just methods).
fn is_data_protocol(class_name: &str, module: &ResolvedModule) -> bool {
    let members = collect_class_members(class_name, module);
    members
        .values()
        .any(|kind| matches!(kind, MemberKind::Attribute))
}

/// Convert a `ruff_text_size::TextRange` to our `Span`.
fn text_range_to_span(range: ruff_text_size::TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}

/// Check a single protocol name in the second argument of isinstance/issubclass.
/// Returns `true` if a violation was emitted (to allow early return).
fn check_single_protocol(
    proto_name: &str,
    call_name: &str,
    call_span: Span,
    first_arg: Option<&Expr>,
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !is_protocol(proto_name, module) {
        return false;
    }

    // Check 1: Not runtime_checkable.
    if !is_runtime_checkable(proto_name, module) {
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!(
                "Protocol `{proto_name}` cannot be used with `{call_name}()` \
                 because it is not decorated with `@runtime_checkable`"
            ),
            call_span,
            &module.path,
            Some(format!(
                "Add `@runtime_checkable` to the definition of `{proto_name}`"
            )),
            Some(
                "PEP 544: a Protocol can only be used as the second argument in \
                 isinstance() or issubclass() if it has the @runtime_checkable decorator"
                    .to_owned(),
            ),
        ));
        return true;
    }

    // Check 2: issubclass with data protocol.
    if call_name == "issubclass" && is_data_protocol(proto_name, module) {
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!("`issubclass()` cannot be used with data protocol `{proto_name}`"),
            call_span,
            &module.path,
            Some(format!(
                "Remove the data attributes from `{proto_name}` or \
                 use `isinstance()` instead"
            )),
            Some(
                "PEP 544: issubclass() can only be used with non-data protocols \
                 (protocols that define only methods, not data attributes)"
                    .to_owned(),
            ),
        ));
        return true;
    }

    // Check 3: Unsafe overlap with concrete class.
    if let Some(first) = first_arg {
        if let Some(concrete_class) = extract_class_name(first) {
            let concrete_exists = module.classes.iter().any(|c| c.name == concrete_class);
            if concrete_exists && has_unsafe_overlap(concrete_class, proto_name, module) {
                diagnostics.push(error_diagnostic_owned(
                    CODE.clone(),
                    format!(
                        "Unsafe overlap: `{concrete_class}` has members incompatible with \
                         protocol `{proto_name}` in `{call_name}()` call"
                    ),
                    call_span,
                    &module.path,
                    Some(format!(
                        "Ensure `{concrete_class}` correctly implements all members of \
                         `{proto_name}` with compatible signatures"
                    )),
                    Some(
                        "PEP 544: type checkers should reject isinstance()/issubclass() \
                         calls where there is an unsafe overlap between the first \
                         argument's type and the protocol"
                            .to_owned(),
                    ),
                ));
                return true;
            }
        }
    }

    false
}

/// Recursively walk statements to find isinstance/issubclass calls.
fn find_isinstance_calls(
    stmts: &[Stmt],
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    basilisk_resolver::walk_all_stmts(stmts, &mut |stmt| match stmt {
        Stmt::Expr(expr_stmt) => check_expr_for_violations(&expr_stmt.value, module, diagnostics),
        Stmt::If(if_stmt) => {
            check_expr_for_violations(&if_stmt.test, module, diagnostics);
            for elif in &if_stmt.elif_else_clauses {
                if let Some(ref test) = elif.test {
                    check_expr_for_violations(test, module, diagnostics);
                }
            }
        }
        Stmt::While(while_stmt) => {
            check_expr_for_violations(&while_stmt.test, module, diagnostics);
        }
        Stmt::Return(ret) => {
            if let Some(ref value) = ret.value {
                check_expr_for_violations(value, module, diagnostics);
            }
        }
        Stmt::Assign(assign) => check_expr_for_violations(&assign.value, module, diagnostics),
        _ => {}
    });
}

/// Check an expression for isinstance/issubclass protocol violations.
fn check_expr_for_violations(
    expr: &Expr,
    module: &ResolvedModule,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Call(call) => {
            let call_name = match call.func.as_ref() {
                Expr::Name(n) if n.id == "isinstance" || n.id == "issubclass" => n.id.as_str(),
                _ => {
                    check_expr_for_violations(&call.func, module, diagnostics);
                    for arg in &call.arguments.args {
                        check_expr_for_violations(arg, module, diagnostics);
                    }
                    return;
                }
            };

            let first_arg = call.arguments.args.first();
            let second_arg = call.arguments.args.get(1);

            let Some(second) = second_arg else {
                return;
            };

            // Use the second argument's span so multiline calls point to the
            // correct line (where `# E` annotations typically appear).
            let second_span = text_range_to_span(second.range());

            // Check each protocol in the second argument.
            match second {
                Expr::Name(name) => {
                    let _ = check_single_protocol(
                        name.id.as_str(),
                        call_name,
                        second_span,
                        first_arg,
                        module,
                        diagnostics,
                    );
                }
                Expr::Tuple(tuple) => {
                    for elt in &tuple.elts {
                        if let Expr::Name(name) = elt {
                            if check_single_protocol(
                                name.id.as_str(),
                                call_name,
                                second_span,
                                first_arg,
                                module,
                                diagnostics,
                            ) {
                                // One violation per call is enough.
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Expr::BoolOp(bool_op) => {
            for value in &bool_op.values {
                check_expr_for_violations(value, module, diagnostics);
            }
        }
        Expr::UnaryOp(unary) => {
            check_expr_for_violations(&unary.operand, module, diagnostics);
        }
        Expr::If(if_expr) => {
            check_expr_for_violations(&if_expr.test, module, diagnostics);
            check_expr_for_violations(&if_expr.body, module, diagnostics);
            check_expr_for_violations(&if_expr.orelse, module, diagnostics);
        }
        _ => {}
    }
}

impl Rule for ProtocolUnsafeOverlap {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        find_isinstance_calls(&parsed.ast.body, module, diagnostics);
    }
}
