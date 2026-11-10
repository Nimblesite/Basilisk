//! Implements [LSPARCH-FEATURES-HOVER]. See docs/specs/LSP-ARCHITECTURE-SPEC.md#LSPARCH-FEATURES-HOVER
//!
//! Typing a dot receiver that is not a declared name.
//!
//! `receiver_type_name` answers for a receiver that has its own declaration —
//! an annotation or an assigned value. Two very ordinary receivers have
//! neither, and both offered no members at all (GitHub #390):
//!
//!   loop variables — `for n in range(3)` binds `n` to an ELEMENT of what it
//!   iterates. There is no assignment to read.
//!
//!   expressions — `s.upper().` has no receiver NAME to look up; the receiver
//!   is a call.
//!
//! Both are answered by typing the expression through the shared bidirectional
//! engine ([NARROWPLAN-CHECKLIST] Stage 2), with the names in view supplied as
//! its scope — the engine has no name resolution of its own.

use std::collections::HashMap;

use basilisk_checker::types::InferredType;
use basilisk_resolver::ResolvedModule;

use crate::util::span_text;

/// The names visible at `byte_offset`, mapped to their types.
///
/// Module-level bindings, plus the locals and parameters of the function that
/// encloses the offset. A name whose type cannot be established is omitted
/// rather than bound to `Unknown`, so the engine treats it as unresolved
/// instead of as a value known to be untypeable.
pub(crate) fn scope_at(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
) -> HashMap<String, InferredType> {
    let mut scope = HashMap::new();
    let types = basilisk_checker::expr_type::ModuleSpanTypes::build(resolved);

    for var in &resolved.module_vars {
        insert_variable(&mut scope, var, source, &types);
    }

    for func in &resolved.functions {
        if !encloses(func.def_span, byte_offset) {
            continue;
        }
        for param in &func.parameters {
            if let Some(annotation) = span_text(param.annotation_span, source) {
                let _ = scope.insert(
                    param.name.clone(),
                    InferredType::from_annotation(&annotation),
                );
            }
        }
        for var in func.local_vars.iter().chain(&func.local_unannotated_vars) {
            insert_variable(&mut scope, var, source, &types);
        }
    }

    scope
}

/// Bind one variable's name to its declared or inferred type. The inferred
/// side comes from the module's span-indexed oracle — the SAME engine behind
/// checker diagnostics ([NARROWPLAN-INTEGRATION] Step 5).
fn insert_variable(
    scope: &mut HashMap<String, InferredType>,
    var: &basilisk_resolver::VariableInfo,
    source: &str,
    types: &basilisk_checker::expr_type::ModuleSpanTypes<'_>,
) {
    let ty = span_text(var.annotation_span, source).map_or_else(
        || {
            var.rhs_span
                .and_then(|span| types.type_at(span))
                .unwrap_or(InferredType::Unknown)
        },
        |annotation| InferredType::from_annotation(&annotation),
    );
    if !matches!(ty, InferredType::Unknown) {
        let _ = scope.insert(var.name.clone(), ty);
    }
}

/// The zero-based line number containing `offset`.
fn line_of(source: &str, offset: usize) -> u32 {
    let before = source.get(..offset.min(source.len())).unwrap_or(source);
    u32::try_from(before.matches('\n').count()).unwrap_or(u32::MAX)
}

/// Whether `span` covers `offset`.
fn encloses(span: basilisk_resolver::Span, offset: usize) -> bool {
    span.start_usize() <= offset && offset <= span.end_usize()
}

/// The type bound by the `for` loop over `name` enclosing `byte_offset`.
///
/// The resolver records loop targets as bound NAMES for definite-assignment
/// analysis but keeps no type for them, so the loops are located in the AST and
/// their iterables typed through the shared engine. A binding's type is the
/// ELEMENT type of its iterable — `for x in xs` binds an element, not `xs`.
///
/// Every enclosing loop is resolved outermost-first, each one's bindings added
/// to the scope the next one is typed in. A nested loop usually iterates the
/// outer loop's variable (`for row in grid: for cell in row:`), which is
/// typeable only once `row` is bound.
pub(crate) fn loop_binding_type(
    resolved: &ResolvedModule,
    source: &str,
    byte_offset: usize,
    name: &str,
) -> Option<InferredType> {
    // The buffer reaching completion still holds the mid-token line that
    // triggered it (`x.`), so it does not parse. Repair it with the same patch
    // the completion resolve uses, rather than a second repair of our own.
    let repaired = match ruff_python_parser::parse_module(source) {
        Ok(_) => std::borrow::Cow::Borrowed(source),
        Err(_) => std::borrow::Cow::Owned(crate::completion::patch_cursor_line(
            source,
            line_of(source, byte_offset),
        )),
    };
    let parsed = ruff_python_parser::parse_module(&repaired).ok()?;
    // The patch rewrites the cursor's line, so clamp into the repaired text.
    let offset = byte_offset.min(repaired.len());
    let mut chain: Vec<&ruff_python_ast::StmtFor> = Vec::new();
    collect_enclosing_for(parsed.syntax().body.as_slice(), offset, &mut chain);

    let mut scope = scope_at(resolved, source, byte_offset);
    let mut bound: Option<InferredType> = None;
    for for_stmt in chain {
        let iter_range = ruff_text_size::Ranged::range(for_stmt.iter.as_ref());
        let Some(iterable) =
            repaired.get(usize::from(iter_range.start())..usize::from(iter_range.end()))
        else {
            continue;
        };
        let iterable_type =
            basilisk_checker::expr_type::infer_expression_source_in_scope(iterable.trim(), &scope);
        let Some(element) = basilisk_checker::class_naming::element_type_of(&iterable_type)
            .or_else(|| named_element_type(resolved, &iterable_type))
        else {
            continue;
        };
        // Bind everything this target introduces, so an inner loop over one of
        // these names can be typed on the next pass.
        bind_target(&for_stmt.target, &element, &mut scope);
        if let Some(component) = component_for_name(&for_stmt.target, name, &element) {
            bound = Some(component);
        }
    }
    bound
}

/// Bind every name a `for` target introduces to its own component of `element`.
fn bind_target(
    target: &ruff_python_ast::Expr,
    element: &InferredType,
    scope: &mut HashMap<String, InferredType>,
) {
    match target {
        ruff_python_ast::Expr::Name(ident) => {
            let _ = scope.insert(ident.id.to_string(), element.clone());
        }
        ruff_python_ast::Expr::Tuple(_) | ruff_python_ast::Expr::List(_) => {
            for (index, part) in unpack_targets(target).iter().enumerate() {
                if let Some(component) = tuple_component(element, index) {
                    bind_target(part, &component, scope);
                }
            }
        }
        _ => {}
    }
}

/// The component of `element` that `name` receives from an unpacking target.
///
/// `for a, b in pairs` binds `a` to the FIRST component of each element, not to
/// the element itself — typing it as the whole tuple offers tuple members on a
/// value that is an `int` (GitHub #390).
fn component_for_name(
    target: &ruff_python_ast::Expr,
    name: &str,
    element: &InferredType,
) -> Option<InferredType> {
    match target {
        ruff_python_ast::Expr::Name(ident) => (ident.id.as_str() == name).then(|| element.clone()),
        ruff_python_ast::Expr::Tuple(_) | ruff_python_ast::Expr::List(_) => unpack_targets(target)
            .iter()
            .enumerate()
            .find_map(|(index, part)| {
                let component = tuple_component(element, index)?;
                component_for_name(part, name, &component)
            }),
        _ => None,
    }
}

/// The sub-targets of a tuple or list unpacking target.
fn unpack_targets(target: &ruff_python_ast::Expr) -> &[ruff_python_ast::Expr] {
    match target {
        ruff_python_ast::Expr::Tuple(tuple) => &tuple.elts,
        ruff_python_ast::Expr::List(list) => &list.elts,
        _ => &[],
    }
}

/// The type unpacked at `index`, when the element is a tuple of known shape.
///
/// A non-tuple element yields nothing rather than a guess: `for a, b in xs`
/// where `xs` is a `list[int]` unpacks an `int`, which is a type error, not a
/// receiver we should invent members for.
fn tuple_component(element: &InferredType, index: usize) -> Option<InferredType> {
    match element {
        InferredType::Tuple(components) => components.get(index).cloned(),
        _ => None,
    }
}

/// The element type of a NAMED class, read from its own iteration protocol.
///
/// `range` yields `int` because typeshed's `range.__iter__` says
/// `-> Iterator[int]`, not because of any table here.
fn named_element_type(resolved: &ResolvedModule, iterable: &InferredType) -> Option<InferredType> {
    let InferredType::Named(class_name) = iterable else {
        return None;
    };
    let class = resolved.builtin_classes.get(class_name.as_str())?;
    let returns = class
        .declaration
        .methods
        .iter()
        .find(|method| method.name == "__iter__" || method.name == "__next__")
        .and_then(|method| method.return_type.as_deref())?;
    // `__next__` returns the element itself; `__iter__` an iterator OF it.
    let element = basilisk_checker::class_naming::annotation_type_argument(returns)
        .unwrap_or_else(|| returns.to_owned());
    let inferred = InferredType::from_annotation(&element);
    (!matches!(inferred, InferredType::Unknown)).then_some(inferred)
}

/// Record the innermost `for` statement that binds `name` and contains `offset`.
///
/// Walks compound statements so a loop nested in a function, an `if`, or
/// another loop is found. Appended OUTERMOST first: each loop's bindings must
/// be in scope before the loop nested inside it can be typed.
fn collect_enclosing_for<'a>(
    body: &'a [ruff_python_ast::Stmt],
    offset: usize,
    chain: &mut Vec<&'a ruff_python_ast::StmtFor>,
) {
    for stmt in body {
        if let ruff_python_ast::Stmt::For(for_stmt) = stmt {
            let body_range = for_stmt.body.first().map(|first| {
                (
                    usize::from(ruff_text_size::Ranged::range(first).start()),
                    usize::from(ruff_text_size::Ranged::range(for_stmt).end()),
                )
            });
            if body_range.is_some_and(|(start, end)| start <= offset && offset <= end) {
                chain.push(for_stmt);
            }
        }
        for nested in child_bodies(stmt) {
            collect_enclosing_for(nested, offset, chain);
        }
    }
}

/// The statement bodies nested inside a compound statement.
fn child_bodies(stmt: &ruff_python_ast::Stmt) -> Vec<&[ruff_python_ast::Stmt]> {
    match stmt {
        ruff_python_ast::Stmt::FunctionDef(node) => vec![node.body.as_slice()],
        ruff_python_ast::Stmt::ClassDef(node) => vec![node.body.as_slice()],
        ruff_python_ast::Stmt::For(node) => {
            vec![node.body.as_slice(), node.orelse.as_slice()]
        }
        ruff_python_ast::Stmt::While(node) => {
            vec![node.body.as_slice(), node.orelse.as_slice()]
        }
        ruff_python_ast::Stmt::With(node) => vec![node.body.as_slice()],
        ruff_python_ast::Stmt::If(node) => {
            let mut bodies = vec![node.body.as_slice()];
            bodies.extend(node.elif_else_clauses.iter().map(|c| c.body.as_slice()));
            bodies
        }
        ruff_python_ast::Stmt::Try(node) => {
            let mut bodies = vec![
                node.body.as_slice(),
                node.orelse.as_slice(),
                node.finalbody.as_slice(),
            ];
            bodies.extend(node.handlers.iter().map(|handler| match handler {
                ruff_python_ast::ExceptHandler::ExceptHandler(h) => h.body.as_slice(),
            }));
            bodies
        }
        ruff_python_ast::Stmt::Match(node) => {
            node.cases.iter().map(|case| case.body.as_slice()).collect()
        }
        _ => Vec::new(),
    }
}

/// The longest trailing slice of `before_dot` that ruff parses as ONE
/// expression — the receiver of the dot.
///
/// Found by asking the parser rather than by matching brackets by hand: the
/// earliest start offset that parses is the longest receiver, so `s.upper()`
/// is taken from `out = s.upper()` and `f(a).b` from `x = f(a).b`.
pub(crate) fn receiver_expression(before_dot: &str) -> Option<&str> {
    // A receiver never spans lines here; the cursor's own line is the search
    // space, which also bounds the number of parse attempts.
    let line = before_dot.rsplit_once('\n').map_or(before_dot, |(_, l)| l);
    line.char_indices()
        .map(|(index, _)| line.get(index..).unwrap_or(""))
        .find(|candidate| {
            !candidate.trim().is_empty()
                && ruff_python_parser::parse_expression(candidate.trim()).is_ok()
        })
        .map(str::trim)
}
