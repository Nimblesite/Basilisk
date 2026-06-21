//! Implements [BSK-E0121] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#chkarch-diag
//! AST index for BSK-E0121 protocol conformance.
//!
//! E0121's structural checks need information the resolver's flattened
//! [`FunctionInfo`](basilisk_resolver::FunctionInfo) does not preserve:
//!
//! - **parameter kinds** (positional-only / positional-or-keyword / keyword-only)
//!   for method-signature conformance, and
//! - **`self.<attr>` assignments inside method bodies**, which provide instance
//!   variables that are invisible to class-body attribute collection.
//!
//! Both are read directly from the parsed AST, indexed once per module so the
//! per-variable conformance checks can look them up cheaply.

use std::collections::{HashMap, HashSet};

use ruff_python_ast::{Expr, Stmt, StmtClassDef, StmtFunctionDef};

/// A method's logical signature, with the implicit receiver removed for instance
/// and class methods.
pub(super) struct MethodSignature<'a> {
    /// Logical parameters in declaration order, each paired with its kind.
    pub params: Vec<(&'a str, ParamKind)>,
    /// `true` when the method is decorated `@staticmethod`.
    pub is_static: bool,
    /// `true` when the method declares `*args` or `**kwargs` (which accept
    /// arbitrary extra arguments, so strict parameter comparison does not apply).
    pub has_variadic: bool,
    /// The receiver parameter name (`self`/`cls`) for instance/class methods,
    /// before it was stripped; `None` for static methods.
    pub receiver: Option<&'a str>,
}

/// The calling convention of a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ParamKind {
    /// Before `/`: may only be passed positionally.
    PositionalOnly,
    /// The default: may be passed positionally or by keyword.
    PositionalOrKeyword,
    /// After `*`: may only be passed by keyword.
    KeywordOnly,
}

/// Per-module index of class and method definitions read from the AST.
pub(super) struct AstIndex<'a> {
    method_defs: HashMap<(&'a str, &'a str), &'a StmtFunctionDef>,
}

impl<'a> AstIndex<'a> {
    /// Build the index from a module body, recording every method (a function
    /// defined directly in a class body) keyed by `(class_name, method_name)`.
    pub(super) fn build(body: &'a [Stmt]) -> Self {
        let mut method_defs = HashMap::new();
        for stmt in body {
            let Stmt::ClassDef(class_def) = stmt else {
                continue;
            };
            for member in &class_def.body {
                if let Stmt::FunctionDef(func) = member {
                    let _ = method_defs
                        .insert((class_def.name.as_str(), func.name.as_str()), func);
                }
            }
        }
        Self { method_defs }
    }

    /// The signature of `class_name`'s `method`, if defined.
    pub(super) fn method_signature(
        &self,
        class_name: &str,
        method: &str,
    ) -> Option<MethodSignature<'a>> {
        self.method_defs
            .get(&(class_name, method))
            .map(|func| signature_of(func))
    }
}

/// Build the [`MethodSignature`] for a function definition.
fn signature_of(func: &StmtFunctionDef) -> MethodSignature<'_> {
    let is_static = func
        .decorator_list
        .iter()
        .any(|dec| matches!(&dec.expression, Expr::Name(name) if name.id.as_str() == "staticmethod"));
    let has_variadic = func.parameters.vararg.is_some() || func.parameters.kwarg.is_some();

    let mut params: Vec<(&str, ParamKind)> = func
        .parameters
        .posonlyargs
        .iter()
        .map(|p| (p.parameter.name.as_str(), ParamKind::PositionalOnly))
        .chain(
            func.parameters
                .args
                .iter()
                .map(|p| (p.parameter.name.as_str(), ParamKind::PositionalOrKeyword)),
        )
        .chain(
            func.parameters
                .kwonlyargs
                .iter()
                .map(|p| (p.parameter.name.as_str(), ParamKind::KeywordOnly)),
        )
        .collect();

    // Strip the implicit receiver (`self`/`cls`) for non-static methods: it is
    // the first declared parameter and never part of the call signature.
    let receiver = (!is_static && !params.is_empty()).then(|| params.remove(0).0);

    MethodSignature {
        params,
        is_static,
        has_variadic,
        receiver,
    }
}

/// Collect the attribute names assigned via the instance receiver (e.g.
/// `self.<name> = ...`, `self.<name>: T = ...`, `self.<name> += ...`) anywhere in
/// the bodies of `class_def`'s instance methods.
///
/// These are real instance variables of the class even though they never appear
/// as class-body attributes, so a protocol member they satisfy must not be
/// reported as missing.
pub(super) fn self_assigned_attrs(class_def: &StmtClassDef) -> HashSet<String> {
    let mut attrs = HashSet::new();
    for member in &class_def.body {
        let Stmt::FunctionDef(func) = member else {
            continue;
        };
        let is_static = func.decorator_list.iter().any(
            |dec| matches!(&dec.expression, Expr::Name(name) if name.id.as_str() == "staticmethod"),
        );
        if is_static {
            continue;
        }
        let receiver = func
            .parameters
            .posonlyargs
            .first()
            .or_else(|| func.parameters.args.first())
            .map(|p| p.parameter.name.as_str());
        let Some(receiver) = receiver else {
            continue;
        };
        collect_self_attrs(&func.body, receiver, &mut attrs);
    }
    attrs
}

/// Recursively gather `<receiver>.<attr>` assignment targets from `stmts`.
fn collect_self_attrs(stmts: &[Stmt], receiver: &str, attrs: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    record_self_attr(target, receiver, attrs);
                }
            }
            Stmt::AnnAssign(ann) => record_self_attr(&ann.target, receiver, attrs),
            Stmt::AugAssign(aug) => record_self_attr(&aug.target, receiver, attrs),
            Stmt::If(node) => {
                collect_self_attrs(&node.body, receiver, attrs);
                for clause in &node.elif_else_clauses {
                    collect_self_attrs(&clause.body, receiver, attrs);
                }
            }
            Stmt::For(node) => collect_self_attrs(&node.body, receiver, attrs),
            Stmt::While(node) => collect_self_attrs(&node.body, receiver, attrs),
            Stmt::With(node) => collect_self_attrs(&node.body, receiver, attrs),
            Stmt::Try(node) => {
                collect_self_attrs(&node.body, receiver, attrs);
                collect_self_attrs(&node.orelse, receiver, attrs);
                collect_self_attrs(&node.finalbody, receiver, attrs);
            }
            _ => {}
        }
    }
}

/// Record `target` when it is `<receiver>.<attr>`.
fn record_self_attr(target: &Expr, receiver: &str, attrs: &mut HashSet<String>) {
    let Expr::Attribute(attr) = target else {
        return;
    };
    if matches!(attr.value.as_ref(), Expr::Name(name) if name.id.as_str() == receiver) {
        let _ = attrs.insert(attr.attr.to_string());
    }
}
