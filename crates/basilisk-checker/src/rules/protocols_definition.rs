//! Implements [`protocols_definition`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! `protocols_definition`: Protocol method sets self-attributes not declared in the Protocol.
//!
//! When a Protocol class defines a method (including `__init__`/`__new__`) that
//! assigns to `self.attr` where `attr` is not a declared member of the Protocol,
//! this is a violation: per the typing spec, "additional attributes only defined
//! in the body of a method by assignment via self are not allowed". Protocol
//! members must be explicitly declared at the class level.
//!
//! ```python
//! from typing import Protocol
//!
//! class MyProto(Protocol):
//!     x: int
//!     def __init__(self) -> None:
//!         self.y = 0  # E — `y` is not declared in the Protocol
//!     def method(self) -> None:
//!         self.z: int = 0  # E — `z` is not declared in the Protocol
//! ```
//!
//! `@staticmethod`/`@classmethod` members have no instance receiver, so their
//! first parameter is not `self` and is not analysed here.

use std::collections::HashSet;

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use super::Rule;
use crate::diagnostic::{error_diagnostic_owned, Diagnostic, ErrorCode};

const CODE: ErrorCode = ErrorCode {
    code: "protocols_definition",
    docs_url: "https://www.basilisk-python.dev/errors/protocols_definition",
};

/// Emits `protocols_definition` when a Protocol `__new__`/`__init__` assigns to undeclared self-attributes.
pub(crate) struct ProtocolNewSelfAttrViolation;

impl Rule for ProtocolNewSelfAttrViolation {
    fn check(
        &self,
        module: &ResolvedModule,
        _ctx: &super::CheckContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        // Find protocol classes.
        let protocol_classes: Vec<_> = module
            .classes
            .iter()
            .filter(|cls| cls.bases.iter().any(|b| b == "Protocol"))
            .collect();

        if protocol_classes.is_empty() {
            return;
        }

        // Re-parse the source to walk the AST.
        let Some(parsed) = super::shared::parse_module(module) else {
            return;
        };

        for cls_info in &protocol_classes {
            // Collect declared attribute names for this protocol.
            let declared_attrs: HashSet<&str> =
                basilisk_resolver::collect_name_set(&cls_info.attributes);

            // Also include method names as declared members.
            let declared_methods: HashSet<&str> =
                cls_info.method_names.iter().map(String::as_str).collect();

            // Find the corresponding class def in the AST.
            if let Some(class_def) = find_class_def(&parsed.ast.body, &cls_info.name) {
                check_class_methods(
                    class_def,
                    &declared_attrs,
                    &declared_methods,
                    &module.path,
                    diagnostics,
                );
            }
        }
    }
}

/// Find a class definition by name in a list of statements.
fn find_class_def<'a>(stmts: &'a [Stmt], name: &str) -> Option<&'a ruff_python_ast::StmtClassDef> {
    for stmt in stmts {
        if let Stmt::ClassDef(class_def) = stmt {
            if class_def.name.as_str() == name {
                return Some(class_def);
            }
        }
    }
    None
}

/// Check every instance method in a protocol class for undeclared self-attr
/// assignments. `@staticmethod`/`@classmethod` members have no instance receiver
/// and are skipped to avoid mis-reading their first parameter as `self`.
fn check_class_methods(
    class_def: &ruff_python_ast::StmtClassDef,
    declared_attrs: &HashSet<&str>,
    declared_methods: &HashSet<&str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in &class_def.body {
        let Stmt::FunctionDef(func_def) = stmt else {
            continue;
        };
        if has_no_instance_receiver(func_def) {
            continue;
        }
        // Get the first parameter name (the instance receiver, usually `self`).
        let self_name = func_def
            .parameters
            .posonlyargs
            .first()
            .or_else(|| func_def.parameters.args.first())
            .map(|p| p.parameter.name.as_str());

        let Some(self_param) = self_name else {
            continue;
        };

        check_body_for_self_attrs(
            &func_def.body,
            self_param,
            declared_attrs,
            declared_methods,
            path,
            diagnostics,
        );
    }
}

/// `true` when the method is decorated `@staticmethod` or `@classmethod`, so its
/// first parameter is not an instance receiver.
fn has_no_instance_receiver(func_def: &ruff_python_ast::StmtFunctionDef) -> bool {
    func_def.decorator_list.iter().any(|dec| {
        matches!(
            &dec.expression,
            Expr::Name(name) if name.id.as_str() == "staticmethod" || name.id.as_str() == "classmethod"
        )
    })
}

/// Walk function body looking for `self.attr = ...` assignments to undeclared attributes.
fn check_body_for_self_attrs(
    stmts: &[Stmt],
    self_param: &str,
    declared_attrs: &HashSet<&str>,
    declared_methods: &HashSet<&str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    check_self_attr_target(
                        target,
                        self_param,
                        declared_attrs,
                        declared_methods,
                        path,
                        diagnostics,
                    );
                }
            }
            Stmt::AnnAssign(ann_assign) => {
                check_self_attr_target(
                    &ann_assign.target,
                    self_param,
                    declared_attrs,
                    declared_methods,
                    path,
                    diagnostics,
                );
            }
            Stmt::AugAssign(aug_assign) => {
                check_self_attr_target(
                    &aug_assign.target,
                    self_param,
                    declared_attrs,
                    declared_methods,
                    path,
                    diagnostics,
                );
            }
            // Recurse into control flow.
            Stmt::If(if_stmt) => {
                check_body_for_self_attrs(
                    &if_stmt.body,
                    self_param,
                    declared_attrs,
                    declared_methods,
                    path,
                    diagnostics,
                );
                for clause in &if_stmt.elif_else_clauses {
                    check_body_for_self_attrs(
                        &clause.body,
                        self_param,
                        declared_attrs,
                        declared_methods,
                        path,
                        diagnostics,
                    );
                }
            }
            Stmt::For(for_stmt) => {
                check_body_for_self_attrs(
                    &for_stmt.body,
                    self_param,
                    declared_attrs,
                    declared_methods,
                    path,
                    diagnostics,
                );
            }
            Stmt::While(while_stmt) => {
                check_body_for_self_attrs(
                    &while_stmt.body,
                    self_param,
                    declared_attrs,
                    declared_methods,
                    path,
                    diagnostics,
                );
            }
            _ => {}
        }
    }
}

/// Check if a target expression is `self.attr` where `attr` is not declared.
fn check_self_attr_target(
    target: &Expr,
    self_param: &str,
    declared_attrs: &HashSet<&str>,
    declared_methods: &HashSet<&str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Expr::Attribute(attr) = target else {
        return;
    };
    let Expr::Name(value_name) = attr.value.as_ref() else {
        return;
    };
    if value_name.id.as_str() != self_param {
        return;
    }

    let attr_name = attr.attr.as_str();
    if !declared_attrs.contains(attr_name) && !declared_methods.contains(attr_name) {
        let range = attr.range();
        diagnostics.push(error_diagnostic_owned(
            CODE.clone(),
            format!("Protocol member `{attr_name}` is not declared in the Protocol body"),
            Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            },
            path,
            Some(format!(
                "Add `{attr_name}: <type>` as a class-level annotation in the Protocol"
            )),
            Some(
                "Protocol members must be explicitly declared; assigning to undeclared \
                 self-attributes in a method body is not allowed"
                    .to_owned(),
            ),
        ));
    }
}
