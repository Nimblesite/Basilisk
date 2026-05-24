//! Implements [CHKARCH-ARCH-PIPELINE]. See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-ARCH-PIPELINE
//! AST walk producing [`Pep695Scoping`] facts for `BSK-E0149`.
//!
//! Every fact is derived from `ruff_python_ast` nodes, so string/comment/
//! docstring content can never be mistaken for real declarations.

use ruff_python_ast::{
    Decorator, Expr, Stmt, StmtClassDef, StmtFunctionDef, StmtTypeAlias, TypeParam,
};
use ruff_text_size::Ranged;

use crate::scope::{
    AttrAccess, DecoratorRef, GenericDefKind, Pep695AliasDef, Pep695Def, Pep695Param,
    Pep695ParamKind, Pep695Scoping,
};

use super::class_info_ext::expr_simple_name;
use super::core::{source_slice_range, text_range_to_span};
use super::function_info::{collect_name_refs_from_expr, collect_name_refs_with_spans};
use super::type_alias::type_param_name;

/// The nearest enclosing scope while walking statements.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Module,
    Class,
    Function,
}

/// Walk context threaded through the recursion.
struct Ctx<'a> {
    scope: Scope,
    /// Type-parameter names of the directly enclosing class (empty otherwise).
    enclosing_class_params: &'a [String],
}

/// Collect all PEP 695 scoping facts for a module from its AST.
pub(super) fn collect_pep695_scoping(stmts: &[Stmt], source: &str) -> Pep695Scoping {
    let mut out = Pep695Scoping::default();
    let ctx = Ctx {
        scope: Scope::Module,
        enclosing_class_params: &[],
    };
    walk(stmts, &ctx, source, &mut out);
    out
}

fn walk(stmts: &[Stmt], ctx: &Ctx<'_>, source: &str, out: &mut Pep695Scoping) {
    for stmt in stmts {
        match stmt {
            Stmt::ClassDef(cls) => walk_class(cls, ctx, source, out),
            Stmt::FunctionDef(func) => walk_function(func, ctx, source, out),
            Stmt::TypeAlias(alias) => collect_alias(alias, ctx, source, out),
            other => walk_other(other, ctx, source, out),
        }
    }
}

fn walk_class(cls: &StmtClassDef, ctx: &Ctx<'_>, source: &str, out: &mut Pep695Scoping) {
    let params = extract_params(cls.type_params.as_deref(), source);
    let param_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();

    if !params.is_empty() {
        out.defs.push(Pep695Def {
            kind: GenericDefKind::Class,
            name: cls.name.to_string(),
            name_span: text_range_to_span(cls.name.range()),
            def_span: text_range_to_span(cls.name.range()),
            params,
            decorators: decorator_refs(&cls.decorator_list),
            enclosing_class_params: enclosing_params(ctx),
        });
    }
    record_module_binding_offset(ctx, cls.name.as_str(), cls.name.range().start().to_u32(), out);

    let child = Ctx {
        scope: Scope::Class,
        enclosing_class_params: &param_names,
    };
    walk(&cls.body, &child, source, out);
}

fn walk_function(func: &StmtFunctionDef, ctx: &Ctx<'_>, source: &str, out: &mut Pep695Scoping) {
    let params = extract_params(func.type_params.as_deref(), source);

    if !params.is_empty() {
        out.defs.push(Pep695Def {
            kind: GenericDefKind::Function,
            name: func.name.to_string(),
            name_span: text_range_to_span(func.name.range()),
            def_span: text_range_to_span(func.name.range()),
            params,
            decorators: decorator_refs(&func.decorator_list),
            enclosing_class_params: enclosing_params(ctx),
        });
    }
    record_module_binding_offset(
        ctx,
        func.name.as_str(),
        func.name.range().start().to_u32(),
        out,
    );

    let child = Ctx {
        scope: Scope::Function,
        enclosing_class_params: &[],
    };
    walk(&func.body, &child, source, out);
}

fn collect_alias(alias: &StmtTypeAlias, ctx: &Ctx<'_>, source: &str, out: &mut Pep695Scoping) {
    let Some(name) = expr_simple_name(&alias.name) else {
        return;
    };
    let params = extract_params(alias.type_params.as_deref(), source);
    let mut rhs_refs = Vec::new();
    collect_name_refs_from_expr(&alias.value, &mut rhs_refs);

    out.aliases.push(Pep695AliasDef {
        name: name.clone(),
        name_span: text_range_to_span(alias.name.range()),
        self_ref_args: find_self_ref_args(&alias.value, &name),
        params,
        rhs_refs,
        in_function: ctx.scope == Scope::Function,
    });
    record_module_binding_offset(ctx, &name, alias.name.range().start().to_u32(), out);
}

/// Handle every non-def/class/alias statement: collect attribute accesses and,
/// at module scope, name references and bindings.
fn walk_other(stmt: &Stmt, ctx: &Ctx<'_>, source: &str, out: &mut Pep695Scoping) {
    let value_exprs = statement_value_exprs(stmt);
    for expr in &value_exprs {
        collect_attr_accesses(expr, out);
        if ctx.scope == Scope::Module {
            collect_name_refs_with_spans(expr, &mut out.module_name_refs);
        }
    }
    if ctx.scope == Scope::Module {
        for (name, offset) in statement_bindings(stmt) {
            out.module_bindings.push((name, offset));
        }
    }
    // Recurse into nested blocks (if/for/while/with/try) preserving scope.
    for body in nested_bodies(stmt) {
        walk(body, ctx, source, out);
    }
}

// ---------------------------------------------------------------------------
// Parameter / decorator extraction
// ---------------------------------------------------------------------------

fn extract_params(
    type_params: Option<&ruff_python_ast::TypeParams>,
    source: &str,
) -> Vec<Pep695Param> {
    let Some(tps) = type_params else {
        return Vec::new();
    };
    tps.type_params
        .iter()
        .map(|tp| param_from(tp, source))
        .collect()
}

fn param_from(tp: &TypeParam, source: &str) -> Pep695Param {
    let (kind, bound) = match tp {
        TypeParam::TypeVar(tv) => (Pep695ParamKind::TypeVar, tv.bound.as_deref()),
        TypeParam::ParamSpec(_) => (Pep695ParamKind::ParamSpec, None),
        TypeParam::TypeVarTuple(_) => (Pep695ParamKind::TypeVarTuple, None),
    };
    let mut bound_refs = Vec::new();
    let bound_text = bound.map(|expr| {
        collect_name_refs_from_expr(expr, &mut bound_refs);
        source_slice_range(source, expr.range())
            .unwrap_or_default()
            .trim()
            .to_owned()
    });
    Pep695Param {
        name: type_param_name(tp),
        span: text_range_to_span(tp.range()),
        kind,
        bound_refs,
        bound_text,
    }
}

fn decorator_refs(decorators: &[Decorator]) -> Vec<DecoratorRef> {
    decorators
        .iter()
        .map(|dec| {
            let mut refs = Vec::new();
            collect_name_refs_from_expr(&dec.expression, &mut refs);
            DecoratorRef {
                refs,
                span: text_range_to_span(dec.range()),
            }
        })
        .collect()
}

fn enclosing_params(ctx: &Ctx<'_>) -> Vec<String> {
    if ctx.scope == Scope::Class {
        ctx.enclosing_class_params.to_vec()
    } else {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Self-reference / attribute / binding helpers
// ---------------------------------------------------------------------------

/// Find the first `alias_name[args]` subscript anywhere in `expr` and return
/// the simple names of its arguments.
fn find_self_ref_args(expr: &Expr, alias_name: &str) -> Option<Vec<String>> {
    match expr {
        Expr::Subscript(sub) => {
            if expr_simple_name(&sub.value).as_deref() == Some(alias_name) {
                return Some(subscript_arg_names(&sub.slice));
            }
            find_self_ref_args(&sub.value, alias_name)
                .or_else(|| find_self_ref_args(&sub.slice, alias_name))
        }
        Expr::BinOp(bin) => find_self_ref_args(&bin.left, alias_name)
            .or_else(|| find_self_ref_args(&bin.right, alias_name)),
        Expr::Tuple(tup) => tup
            .elts
            .iter()
            .find_map(|elt| find_self_ref_args(elt, alias_name)),
        Expr::Call(call) => call
            .arguments
            .args
            .iter()
            .find_map(|arg| find_self_ref_args(arg, alias_name)),
        Expr::Starred(s) => find_self_ref_args(&s.value, alias_name),
        _ => None,
    }
}

fn subscript_arg_names(slice: &Expr) -> Vec<String> {
    match slice {
        Expr::Tuple(tup) => tup
            .elts
            .iter()
            .map(|elt| expr_simple_name(elt).unwrap_or_default())
            .collect(),
        other => vec![expr_simple_name(other).unwrap_or_default()],
    }
}

/// Recursively collect `Name.attr` accesses from an expression tree.
fn collect_attr_accesses(expr: &Expr, out: &mut Pep695Scoping) {
    if let Expr::Attribute(attr) = expr {
        if let Some(base) = expr_simple_name(&attr.value) {
            out.attr_accesses.push(AttrAccess {
                base,
                attr: attr.attr.to_string(),
                span: text_range_to_span(attr.range()),
            });
        }
    }
    for child in child_exprs(expr) {
        collect_attr_accesses(child, out);
    }
}

/// Module-scope bindings created by a non-def statement (simple-name targets).
fn statement_bindings(stmt: &Stmt) -> Vec<(String, u32)> {
    match stmt {
        Stmt::Assign(assign) => assign
            .targets
            .iter()
            .filter_map(simple_name_with_offset)
            .collect(),
        Stmt::AnnAssign(ann) => simple_name_with_offset(&ann.target).into_iter().collect(),
        Stmt::AugAssign(aug) => simple_name_with_offset(&aug.target).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn record_module_binding_offset(ctx: &Ctx<'_>, name: &str, offset: u32, out: &mut Pep695Scoping) {
    if ctx.scope == Scope::Module {
        out.module_bindings.push((name.to_owned(), offset));
    }
}

fn simple_name_with_offset(expr: &Expr) -> Option<(String, u32)> {
    expr_simple_name(expr).map(|name| (name, expr.range().start().to_u32()))
}

// ---------------------------------------------------------------------------
// Statement expression / nested-body access
// ---------------------------------------------------------------------------

/// Value-position expressions of a statement (where free names/attrs appear).
fn statement_value_exprs(stmt: &Stmt) -> Vec<&Expr> {
    match stmt {
        Stmt::Expr(e) => vec![e.value.as_ref()],
        Stmt::Assign(a) => vec![a.value.as_ref()],
        Stmt::AnnAssign(a) => {
            let mut v = vec![a.annotation.as_ref()];
            if let Some(value) = &a.value {
                v.push(value.as_ref());
            }
            v
        }
        Stmt::AugAssign(a) => vec![a.value.as_ref()],
        Stmt::Return(r) => r.value.as_deref().into_iter().collect(),
        _ => Vec::new(),
    }
}

/// Nested statement blocks of a compound statement (excluding def/class/alias,
/// which are handled by the dedicated walkers).
fn nested_bodies(stmt: &Stmt) -> Vec<&[Stmt]> {
    match stmt {
        Stmt::If(node) => {
            let mut bodies = vec![node.body.as_slice()];
            for clause in &node.elif_else_clauses {
                bodies.push(clause.body.as_slice());
            }
            bodies
        }
        Stmt::For(node) => vec![node.body.as_slice(), node.orelse.as_slice()],
        Stmt::While(node) => vec![node.body.as_slice(), node.orelse.as_slice()],
        Stmt::With(node) => vec![node.body.as_slice()],
        Stmt::Try(node) => {
            let mut bodies = vec![
                node.body.as_slice(),
                node.orelse.as_slice(),
                node.finalbody.as_slice(),
            ];
            for handler in &node.handlers {
                let ruff_python_ast::ExceptHandler::ExceptHandler(h) = handler;
                bodies.push(h.body.as_slice());
            }
            bodies
        }
        _ => Vec::new(),
    }
}

/// Immediate child expressions of an expression (for recursive walks).
fn child_exprs(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::Attribute(a) => vec![a.value.as_ref()],
        Expr::Subscript(s) => vec![s.value.as_ref(), s.slice.as_ref()],
        Expr::BinOp(b) => vec![b.left.as_ref(), b.right.as_ref()],
        Expr::BoolOp(b) => b.values.iter().collect(),
        Expr::UnaryOp(u) => vec![u.operand.as_ref()],
        Expr::Tuple(t) => t.elts.iter().collect(),
        Expr::List(l) => l.elts.iter().collect(),
        Expr::Set(s) => s.elts.iter().collect(),
        Expr::Starred(s) => vec![s.value.as_ref()],
        Expr::Compare(c) => {
            let mut v = vec![c.left.as_ref()];
            v.extend(c.comparators.iter());
            v
        }
        Expr::If(i) => vec![i.test.as_ref(), i.body.as_ref(), i.orelse.as_ref()],
        Expr::Call(c) => {
            let mut v = vec![c.func.as_ref()];
            v.extend(c.arguments.args.iter());
            v.extend(c.arguments.keywords.iter().map(|kw| &kw.value));
            v
        }
        _ => Vec::new(),
    }
}
