//! Implements [`protocols_definition_2`] from [CHKARCH-DIAG]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG
//! AST index for `protocols_definition_2` protocol conformance.
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

/// A method's logical signature: its parameters with calling conventions, with
/// the implicit receiver removed for instance and class methods.
pub(super) struct MethodSignature<'a> {
    /// Logical parameters in declaration order, each paired with its kind.
    params: Vec<(&'a str, ParamKind)>,
}

/// The calling convention of a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParamKind {
    /// Before `/`: may only be passed positionally.
    PositionalOnly,
    /// The default: may be passed positionally or by keyword.
    PositionalOrKeyword,
    /// After `*`: may only be passed by keyword.
    KeywordOnly,
}

impl ParamKind {
    /// `true` when a parameter of this kind can be supplied positionally.
    fn allows_positional(self) -> bool {
        matches!(self, Self::PositionalOnly | Self::PositionalOrKeyword)
    }

    /// `true` when a parameter of this kind can be supplied by keyword.
    fn allows_keyword(self) -> bool {
        matches!(self, Self::KeywordOnly | Self::PositionalOrKeyword)
    }
}

impl MethodSignature<'_> {
    /// Compare a protocol method against an implementation method that already
    /// shares its parameter *names*, returning a human-readable reason when the
    /// implementation cannot be called every way the protocol permits.
    ///
    /// The implementation must accept each parameter in every mode (positional
    /// and/or keyword) the protocol allows: a protocol positional-or-keyword
    /// parameter cannot be satisfied by a keyword-only or positional-only one.
    /// Arity differences are reported separately by the name comparison, so a
    /// differing parameter count yields `None` here.
    pub(super) fn calling_convention_mismatch(&self, implementation: &Self) -> Option<String> {
        if self.params.len() != implementation.params.len() {
            return None;
        }
        self.params.iter().zip(&implementation.params).find_map(
            |((proto_name, proto_kind), (_, impl_kind))| {
                if proto_kind.allows_positional() && !impl_kind.allows_positional() {
                    Some(format!(
                        "parameter `{proto_name}` is positional in the protocol but \
                         keyword-only in the implementation"
                    ))
                } else if proto_kind.allows_keyword() && !impl_kind.allows_keyword() {
                    Some(format!(
                        "parameter `{proto_name}` is keyword-accessible in the protocol \
                         but positional-only in the implementation"
                    ))
                } else {
                    None
                }
            },
        )
    }
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
                    let _ = method_defs.insert((class_def.name.as_str(), func.name.as_str()), func);
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

// ##########################################################################
// # DELETED BODY — the `@staticmethod` test, which stood twice in this     #
// # file. DO NOT RESTORE IT AND DO NOT RETURN `false` IN ITS PLACE.        #
// #                                                                        #
// #   func.decorator_list.iter().any(|dec| matches!(                       #
// #       &dec.expression,                                                 #
// #       Expr::Name(name) if name.id.as_str() == "staticmethod"))         #
// #                                                                        #
// # `staticmethod` is an ordinary builtin bound to an ordinary name, and a #
// # decorator is an arbitrary expression. This matched `Expr::Name` with   #
// # one exact spelling, so:                                                #
// #                                                                        #
// #   @builtins.staticmethod        -> Expr::Attribute, never matched      #
// #   from builtins import staticmethod as sm                              #
// #   @sm                           -> the SAME object, never matched      #
// #   staticmethod = my_decorator   -> rebound, still matched              #
// #                                                                        #
// # The consequence is not cosmetic: a false answer here makes the rule    #
// # mistake the function's FIRST PARAMETER for a `self` receiver, so every #
// # parameter of a qualified-decorator staticmethod shifts by one and the  #
// # protocol comparison that follows is done against the wrong signature.  #
// #                                                                        #
// # Whether a decorator is `builtins.staticmethod` is a binding question.  #
// #                                                                        #
// # Pinned by: tests/source_text_verdict_pin_tests.rs                      #
// ##########################################################################
fn is_staticmethod_decorated(_func: &StmtFunctionDef) -> bool {
    panic!(
        "basilisk-checker: the `@staticmethod` test was DELETED because it matched one \
         exact SPELLING on an `Expr::Name` decorator, so `@builtins.staticmethod` and \
         any aliased import of the same object were not staticmethods while a rebound \
         `staticmethod` name still was. It panics because the real implementation — \
         resolving the decorator expression through the binding table — DOES NOT EXIST \
         YET. Do not restore the spelling test and do not return `false` in its place: \
         `false` makes the rule read the first parameter as a `self` receiver and \
         compare every remaining parameter against the wrong position."
    )
}

/// Build the [`MethodSignature`] for a function definition.
fn signature_of(func: &StmtFunctionDef) -> MethodSignature<'_> {
    let is_static = is_staticmethod_decorated(func);

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
    if !is_static && !params.is_empty() {
        let _ = params.remove(0);
    }

    MethodSignature { params }
}

/// Map each class in `body` to the attribute names it assigns via `self.<attr>`
/// (see [`self_assigned_attrs`]). Keys borrow from the parsed module body.
pub(super) fn self_attrs_by_class(body: &[Stmt]) -> HashMap<&str, HashSet<String>> {
    body.iter()
        .filter_map(|stmt| match stmt {
            Stmt::ClassDef(class_def) => {
                Some((class_def.name.as_str(), self_assigned_attrs(class_def)))
            }
            _ => None,
        })
        .collect()
}

/// Collect the attribute names assigned via the instance receiver (e.g.
/// `self.<name> = ...`, `self.<name>: T = ...`, `self.<name> += ...`) anywhere in
/// the bodies of `class_def`'s instance methods.
///
/// These are real instance variables of the class even though they never appear
/// as class-body attributes, so a protocol member they satisfy must not be
/// reported as missing.
fn self_assigned_attrs(class_def: &StmtClassDef) -> HashSet<String> {
    let mut attrs = HashSet::new();
    for member in &class_def.body {
        let Stmt::FunctionDef(func) = member else {
            continue;
        };
        // Second call site of the DELETED `@staticmethod` test — see the banner
        // on `is_staticmethod_decorated`.
        let is_static = is_staticmethod_decorated(func);
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
