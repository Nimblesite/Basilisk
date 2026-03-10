//! BSK-E0097: Protocol `__new__`/`__init__` sets self-attributes not declared in Protocol.
//!
//! When a Protocol class defines `__new__` or `__init__` that assigns to
//! `self.attr` where `attr` is not a declared member of the Protocol, this is
//! a violation: Protocol members must be explicitly declared.
//!
//! ```python
//! from typing import Protocol
//!
//! class MyProto(Protocol):
//!     x: int
//!     def __init__(self) -> None:
//!         self.y = 0  # E — `y` is not declared in the Protocol
//! ```

use std::collections::HashSet;

use basilisk_resolver::{ResolvedModule, Span};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged as _;

use super::Rule;
use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0097",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0097",
};

/// Emits BSK-E0097 when a Protocol `__new__`/`__init__` assigns to undeclared self-attributes.
pub(crate) struct ProtocolNewSelfAttrViolation;

impl Rule for ProtocolNewSelfAttrViolation {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
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
        let Ok(parsed) = basilisk_parser::parse_source(module.source.clone(), module.path.clone())
        else {
            return;
        };

        for cls_info in &protocol_classes {
            // Collect declared attribute names for this protocol.
            let declared_attrs: HashSet<&str> = cls_info
                .attributes
                .iter()
                .map(|a| a.name.as_str())
                .collect();

            // Also include method names as declared members.
            let declared_methods: HashSet<&str> =
                cls_info.method_names.iter().map(String::as_str).collect();

            // Find the corresponding class def in the AST.
            if let Some(class_def) = find_class_def(&parsed.ast.body, &cls_info.name) {
                check_class_init_new(
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

/// Check `__init__` and `__new__` methods in a protocol class for undeclared self-attr assignments.
fn check_class_init_new(
    class_def: &ruff_python_ast::StmtClassDef,
    declared_attrs: &HashSet<&str>,
    declared_methods: &HashSet<&str>,
    path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in &class_def.body {
        if let Stmt::FunctionDef(func_def) = stmt {
            let name = func_def.name.as_str();
            if name == "__init__" || name == "__new__" {
                // Get the first parameter name (usually `self` or `cls`).
                let self_name = func_def
                    .parameters
                    .args
                    .first()
                    .map(|p| p.parameter.name.as_str())
                    .or_else(|| {
                        func_def
                            .parameters
                            .posonlyargs
                            .first()
                            .map(|p| p.parameter.name.as_str())
                    });

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
    }
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
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!("Protocol member `{attr_name}` is not declared in the Protocol body"),
            span: Span {
                start: range.start().to_u32(),
                end: range.end().to_u32(),
            },
            path: path.to_owned(),
            help: Some(format!(
                "Add `{attr_name}: <type>` as a class-level annotation in the Protocol"
            )),
            note: Some(
                "Protocol members must be explicitly declared; assigning to undeclared \
                 self-attributes in `__init__`/`__new__` is not allowed"
                    .to_owned(),
            ),
        });
    }
}
