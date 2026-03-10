//! BSK-E0075: Incompatible type for `Self`-typed attribute.
//!
//! When a class declares an attribute annotated with `Self` (or `Self | None`,
//! `Optional[Self]`, etc.), that attribute's type is bound to the *concrete*
//! subclass at each usage site.  Passing or assigning a parent-class instance
//! where the subclass is expected is a type error.
//!
//! ```python
//! from typing import Self, TypeVar, Generic
//! from dataclasses import dataclass
//!
//! T = TypeVar("T")
//!
//! @dataclass
//! class LinkedList(Generic[T]):
//!     value: T
//!     next: Self | None = None
//!
//! @dataclass
//! class OrdinalLinkedList(LinkedList[int]):
//!     def ordinal_value(self) -> str:
//!         return str(self.value)
//!
//! xs = OrdinalLinkedList(value=1, next=LinkedList[int](value=2))  # E
//! xs.next = LinkedList[int](value=3, next=None)                  # E
//! ```
//!
//! Specification: <https://typing.readthedocs.io/en/latest/spec/generics.html#use-in-attribute-annotations>

use std::collections::{HashMap, HashSet};

use basilisk_resolver::ResolvedModule;

use crate::diagnostic::{Diagnostic, ErrorCode, Severity};

use super::Rule;

const CODE: ErrorCode = ErrorCode {
    code: "BSK-E0075",
    docs_url: "https://www.basilisk-python.dev/errors/BSK-E0075",
};

/// Emits BSK-E0075 when a parent-class instance is used where a `Self`-typed
/// attribute expects the concrete subclass.
pub(crate) struct SelfTypeAttributeIncompatible;

impl Rule for SelfTypeAttributeIncompatible {
    fn check(&self, module: &ResolvedModule, diagnostics: &mut Vec<Diagnostic>) {
        let source = &module.source;
        let path = &module.path;

        // Step 1: Find classes with Self-typed attributes.
        // Map from class name -> set of attribute names whose annotation mentions `Self`.
        let mut self_typed_attrs: HashMap<&str, HashSet<&str>> = HashMap::new();
        for cls in &module.classes {
            for attr in &cls.attributes {
                let Some(ann_span) = attr.annotation_span else {
                    continue;
                };
                let Some(ann_text) = source.get(ann_span.start as usize..ann_span.end as usize)
                else {
                    continue;
                };
                if annotation_mentions_self(ann_text.trim()) {
                    self_typed_attrs
                        .entry(cls.name.as_str())
                        .or_default()
                        .insert(attr.name.as_str());
                }
            }
        }

        if self_typed_attrs.is_empty() {
            return;
        }

        // Step 2: Parse the source to build parent-child class relationships
        // (handles subscripted bases like `LinkedList[int]`).
        let Ok(parsed) = basilisk_parser::parse_source(source.clone(), path.clone()) else {
            return;
        };

        // Build a map: child class name -> list of parent class base names.
        let class_parent_map = build_class_parent_map(&parsed.ast.body);

        // Step 3: For each subclass, compute which Self-typed attributes it inherits.
        // Map: subclass name -> (parent class name, set of Self-typed attr names)
        let mut subclass_self_attrs: HashMap<&str, (&str, &HashSet<&str>)> = HashMap::new();
        for cls in &module.classes {
            if let Some(parents) = class_parent_map.get(cls.name.as_str()) {
                for parent_name in parents {
                    if let Some(attrs) = self_typed_attrs.get(parent_name.as_str()) {
                        subclass_self_attrs
                            .insert(cls.name.as_str(), (parent_name.as_str(), attrs));
                    }
                }
            }
        }

        if subclass_self_attrs.is_empty() {
            return;
        }

        // Step 4: Build a map of module-level variable names to their assigned class.
        // e.g., `xs = OrdinalLinkedList(...)` -> "xs" -> "OrdinalLinkedList"
        let var_class_map: HashMap<&str, &str> = module
            .module_vars
            .iter()
            .filter(|v| v.rhs_kind == basilisk_resolver::RhsKind::CallExpr)
            .filter_map(|v| {
                let rhs_span = v.rhs_span?;
                let rhs_text = source.get(rhs_span.start as usize..rhs_span.end as usize)?;
                let class_name = extract_callee_name(rhs_text)?;
                Some((v.name.as_str(), class_name))
            })
            .collect();

        // Step 5: Walk the AST to find violations.
        for stmt in &parsed.ast.body {
            check_stmt_for_violations(
                stmt,
                path,
                &subclass_self_attrs,
                &var_class_map,
                diagnostics,
            );
        }
    }
}

/// Returns `true` when an annotation text mentions `Self`.
fn annotation_mentions_self(text: &str) -> bool {
    for part in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if part == "Self" {
            return true;
        }
    }
    false
}

/// Build a map from class name -> list of parent base class names.
/// Handles both simple bases (`class A(B):`) and subscripted bases
/// (`class A(B[int]):`).
fn build_class_parent_map(stmts: &[ruff_python_ast::Stmt]) -> HashMap<String, Vec<String>> {
    use ruff_python_ast::Stmt;

    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for stmt in stmts {
        let Stmt::ClassDef(cls) = stmt else {
            continue;
        };
        let Some(args) = cls.arguments.as_ref() else {
            continue;
        };
        let mut parents = Vec::new();
        for base_expr in &args.args {
            if let Some(name) = extract_base_class_name(base_expr) {
                parents.push(name);
            }
        }
        if !parents.is_empty() {
            map.insert(cls.name.to_string(), parents);
        }
    }
    map
}

/// Extract the base class name from a base expression.
/// Handles `BaseClass` (Name) and `BaseClass[int]` (Subscript).
fn extract_base_class_name(expr: &ruff_python_ast::Expr) -> Option<String> {
    use ruff_python_ast::Expr;
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Subscript(sub) => {
            if let Expr::Name(name) = sub.value.as_ref() {
                Some(name.id.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Extract the callee name from a RHS text like `ClassName(...)` or `ClassName[T](...)`.
fn extract_callee_name(rhs_text: &str) -> Option<&str> {
    // Handle `ClassName[T](...)` by stripping everything from `[` onwards first.
    let before_bracket = rhs_text.split('[').next()?;
    let before_paren = before_bracket.split('(').next()?;
    let name = before_paren.trim();
    if name.is_empty() {
        return None;
    }
    // Class names start with uppercase (heuristic).
    if !name.starts_with(|c: char| c.is_ascii_uppercase()) {
        return None;
    }
    Some(name)
}

/// Extract the callee class name from a call expression AST node.
/// Handles `ClassName(...)` and `ClassName[T](...)`.
fn callee_class_name_from_call(call: &ruff_python_ast::ExprCall) -> Option<String> {
    use ruff_python_ast::Expr;
    match call.func.as_ref() {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Subscript(sub) => {
            if let Expr::Name(name) = sub.value.as_ref() {
                Some(name.id.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Check a statement for Self-type attribute violations.
fn check_stmt_for_violations(
    stmt: &ruff_python_ast::Stmt,
    path: &str,
    subclass_self_attrs: &HashMap<&str, (&str, &HashSet<&str>)>,
    var_class_map: &HashMap<&str, &str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::{Expr, Stmt};
    use ruff_text_size::Ranged as _;

    // Case 1: `xs = SubClass(next=ParentClass(...))` -- constructor call with
    // keyword argument for a Self-typed attribute.
    if let Stmt::Assign(assign) = stmt {
        if let Expr::Call(call) = assign.value.as_ref() {
            if let Some(constructor_class) = callee_class_name_from_call(call) {
                if let Some(&(parent_class, self_attrs)) =
                    subclass_self_attrs.get(constructor_class.as_str())
                {
                    for kw in &call.arguments.keywords {
                        let Some(kw_name) = kw.arg.as_ref() else {
                            continue;
                        };
                        if !self_attrs.contains(kw_name.as_str()) {
                            continue;
                        }
                        let Some(value_class) = call_expr_class_name(&kw.value) else {
                            continue;
                        };
                        if is_parent_not_subclass(&value_class, &constructor_class, parent_class) {
                            let range = assign.value.range();
                            let span = basilisk_resolver::Span {
                                start: range.start().to_u32(),
                                end: range.end().to_u32(),
                            };
                            diagnostics.push(Diagnostic {
                                code: CODE.clone(),
                                severity: Severity::Error,
                                message: format!(
                                    "Argument `{kw_name}` is typed as `Self` in \
                                     `{parent_class}`, which resolves to \
                                     `{constructor_class}` here, but got \
                                     `{value_class}` instance"
                                ),
                                span,
                                path: path.to_owned(),
                                help: Some(format!(
                                    "Pass a `{constructor_class}` instance instead \
                                     of `{value_class}`"
                                )),
                                note: Some(
                                    "`Self` in attribute annotations binds to the \
                                     concrete subclass, not the parent class"
                                        .to_owned(),
                                ),
                            });
                        }
                    }
                }
            }
        }

        // Case 2: `xs.attr = ParentClass(...)` -- attribute assignment.
        for target in &assign.targets {
            if let Expr::Attribute(attr_expr) = target {
                check_attr_assignment(
                    attr_expr,
                    &assign.value,
                    path,
                    subclass_self_attrs,
                    var_class_map,
                    diagnostics,
                );
            }
        }
    }

    // Handle if/elif/else blocks to find nested assignments.
    if let Stmt::If(if_stmt) = stmt {
        for body_stmt in &if_stmt.body {
            check_stmt_for_violations(
                body_stmt,
                path,
                subclass_self_attrs,
                var_class_map,
                diagnostics,
            );
        }
        for clause in &if_stmt.elif_else_clauses {
            for body_stmt in &clause.body {
                check_stmt_for_violations(
                    body_stmt,
                    path,
                    subclass_self_attrs,
                    var_class_map,
                    diagnostics,
                );
            }
        }
    }
}

/// Check an attribute assignment (`obj.attr = value`) for Self-type violations.
fn check_attr_assignment(
    attr_expr: &ruff_python_ast::ExprAttribute,
    value: &ruff_python_ast::Expr,
    path: &str,
    subclass_self_attrs: &HashMap<&str, (&str, &HashSet<&str>)>,
    var_class_map: &HashMap<&str, &str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    use ruff_python_ast::Expr;
    use ruff_text_size::Ranged as _;

    // The object must be a known variable whose class has Self-typed attrs.
    let Expr::Name(obj_name) = attr_expr.value.as_ref() else {
        return;
    };
    let Some(&var_class) = var_class_map.get(obj_name.id.as_str()) else {
        return;
    };
    let Some(&(parent_class, self_attrs)) = subclass_self_attrs.get(var_class) else {
        return;
    };

    let attr_name = attr_expr.attr.as_str();
    if !self_attrs.contains(attr_name) {
        return;
    }

    // Check if the assigned value is a parent class instance.
    let Some(value_class) = call_expr_class_name(value) else {
        return;
    };

    if is_parent_not_subclass(&value_class, var_class, parent_class) {
        let range = value.range();
        let span = basilisk_resolver::Span {
            start: range.start().to_u32(),
            end: range.end().to_u32(),
        };
        diagnostics.push(Diagnostic {
            code: CODE.clone(),
            severity: Severity::Error,
            message: format!(
                "Cannot assign `{value_class}` to attribute `{attr_name}`: \
                 `Self` resolves to `{var_class}` here, not `{value_class}`"
            ),
            span,
            path: path.to_owned(),
            help: Some(format!(
                "Assign a `{var_class}` instance instead of `{value_class}`"
            )),
            note: Some(
                "`Self` in attribute annotations binds to the concrete subclass, \
                 not the parent class"
                    .to_owned(),
            ),
        });
    }
}

/// Extract the class name from a call expression (constructor call).
/// Handles `ClassName(...)` and `ClassName[T](...)`.
fn call_expr_class_name(expr: &ruff_python_ast::Expr) -> Option<String> {
    use ruff_python_ast::Expr;
    let Expr::Call(call) = expr else {
        return None;
    };
    callee_class_name_from_call(call)
}

/// Returns `true` when `value_class` is the parent (or ancestor) of the
/// expected subclass, but not the subclass itself.
fn is_parent_not_subclass(value_class: &str, expected_subclass: &str, parent_class: &str) -> bool {
    value_class == parent_class && value_class != expected_subclass
}
