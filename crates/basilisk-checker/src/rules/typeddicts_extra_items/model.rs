//! Implements [`typeddicts_extra_items`] from [CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS].
//! See docs/specs/CHECKER-ARCHITECTURE-SPEC.md#CHKARCH-DIAG-TYPEDDICT-EXTRA-ITEMS
//!
//! The `TypedDict` model used by the PEP 728 `extra_items` / `closed` checks.
//!
//! Each `TypedDict` in the module — declared with the class syntax or the
//! functional `TypedDict("Name", {...}, extra_items=...)` syntax — is captured
//! as a [`TdModel`]. Inheritance is resolved transitively against a
//! `name -> &TdModel` map so callers see effective fields and the effective
//! `extra_items` pseudo-item.

use std::collections::HashMap;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::Ranged;

use basilisk_resolver::{strip_typeddict_qualifiers, Span};

use crate::rules::shared::{ann_str, expr_name};

/// Guards against cyclic `bases` (illegal Python, but must not hang).
const MAX_DEPTH: u32 = 64;

/// A wrapper qualifier that is illegal on `extra_items` (PEP 728).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Qualifier {
    Required,
    NotRequired,
}

/// An explicitly declared field of a `TypedDict`.
#[derive(Debug, Clone)]
pub(super) struct TdField {
    pub(super) name: String,
    /// Core value type with `Required`/`NotRequired`/`ReadOnly` stripped.
    pub(super) ty: String,
    pub(super) required: bool,
}

/// An explicit `extra_items=` declaration.
#[derive(Debug, Clone)]
pub(super) struct ExtraItems {
    /// Core value type with qualifiers stripped (e.g. `int | None`, `object`).
    pub(super) ty: String,
    pub(super) readonly: bool,
    /// `Some` when the value illegally wraps `Required`/`NotRequired`.
    pub(super) qualifier: Option<Qualifier>,
}

/// A `closed=` declaration. `value` is `None` when the argument is not a literal
/// `True`/`False` (which is itself an error).
#[derive(Debug, Clone)]
pub(super) struct ClosedKw {
    pub(super) value: Option<bool>,
}

/// A captured `TypedDict` definition.
#[derive(Debug, Clone)]
pub(super) struct TdModel {
    pub(super) name: String,
    pub(super) bases: Vec<String>,
    pub(super) fields: Vec<TdField>,
    pub(super) extra_items: Option<ExtraItems>,
    pub(super) closed: Option<ClosedKw>,
    /// Source span of the definition (class keyword or the assignment target).
    pub(super) span: Span,
}

/// The effective `extra_items` pseudo-item: type text and read-only flag.
pub(super) type EffectiveExtra = (String, bool);

pub(super) fn mk_span(range: ruff_text_size::TextRange) -> Span {
    Span {
        start: range.start().to_u32(),
        end: range.end().to_u32(),
    }
}

/// Split a qualified annotation text into `(core_type, readonly, qualifier)`.
fn parse_qualified(text: &str) -> (String, bool, Option<Qualifier>) {
    let trimmed = text.trim();
    let readonly = trimmed.starts_with("ReadOnly[");
    let qualifier = if trimmed.starts_with("Required[") {
        Some(Qualifier::Required)
    } else if trimmed.starts_with("NotRequired[") {
        Some(Qualifier::NotRequired)
    } else {
        None
    };
    (
        strip_typeddict_qualifiers(trimmed).to_owned(),
        readonly,
        qualifier,
    )
}

// ---------------------------------------------------------------------------
// Collection
// ---------------------------------------------------------------------------

/// Collect every `TypedDict` in declaration order. A class is a `TypedDict` when
/// it names `TypedDict` directly or inherits from a `TypedDict` collected
/// earlier (Python requires a base be defined before use).
pub(super) fn collect_models(stmts: &[Stmt], target: (u32, u32)) -> Vec<TdModel> {
    let mut models: Vec<TdModel> = Vec::new();
    collect_into(stmts, &mut models, target);
    models
}

fn collect_into(stmts: &[Stmt], models: &mut Vec<TdModel>, target: (u32, u32)) {
    for stmt in stmts {
        match stmt {
            Stmt::ClassDef(cls) => {
                if let Some(model) = model_from_class(cls, models, target) {
                    models.push(model);
                }
                collect_into(&cls.body, models, target);
            }
            Stmt::Assign(assign) => {
                if let Some(model) = model_from_functional(assign) {
                    models.push(model);
                }
            }
            _ => {}
        }
    }
}

fn is_typeddict_class(cls: &ast::StmtClassDef, known: &[TdModel]) -> bool {
    cls.arguments.as_ref().is_some_and(|args| {
        args.args.iter().any(|base| {
            expr_name(base)
                .is_some_and(|name| name == "TypedDict" || known.iter().any(|m| m.name == name))
        })
    })
}

fn model_from_class(
    cls: &ast::StmtClassDef,
    known: &[TdModel],
    target: (u32, u32),
) -> Option<TdModel> {
    if !is_typeddict_class(cls, known) {
        return None;
    }
    let total = class_total(cls);
    let mut fields = Vec::new();
    collect_td_fields(&cls.body, total, target, &mut fields);
    let bases = cls
        .arguments
        .as_ref()
        .map(|args| {
            args.args
                .iter()
                .filter_map(expr_name)
                .filter(|n| *n != "TypedDict")
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Some(TdModel {
        name: cls.name.to_string(),
        bases,
        fields,
        extra_items: extra_items_keyword(cls),
        closed: closed_keyword(cls),
        span: mk_span(cls.range()),
    })
}

/// Collect `TypedDict` fields from a class body, descending into `if` guards
/// (`sys.version_info`, `TYPE_CHECKING`) and admitting only branches that are not
/// statically false at the target version — so a version-conditional item exists
/// exactly when it would at runtime.
fn collect_td_fields(stmts: &[Stmt], total: bool, target: (u32, u32), out: &mut Vec<TdField>) {
    for stmt in stmts {
        match stmt {
            Stmt::AnnAssign(_) => {
                if let Some(field) = field_from_stmt(stmt, total) {
                    out.push(field);
                }
            }
            Stmt::If(if_stmt) => collect_td_fields_in_if(if_stmt, total, target, out),
            _ => {}
        }
    }
}

/// Admit fields from each statically-reachable branch of an `if`/`elif`/`else`.
fn collect_td_fields_in_if(
    if_stmt: &ast::StmtIf,
    total: bool,
    target: (u32, u32),
    out: &mut Vec<TdField>,
) {
    use basilisk_resolver::{evaluate, parse_static_condition, BranchTruth};
    let test = parse_static_condition(&if_stmt.test);
    if evaluate(&test, target) != BranchTruth::AlwaysFalse {
        collect_td_fields(&if_stmt.body, total, target, out);
    }
    for clause in &if_stmt.elif_else_clauses {
        let reachable = match &clause.test {
            Some(elif) => {
                evaluate(&parse_static_condition(elif), target) != BranchTruth::AlwaysFalse
            }
            // The `else` is reachable unless the `if` test is always taken.
            None => evaluate(&test, target) != BranchTruth::AlwaysTrue,
        };
        if reachable {
            collect_td_fields(&clause.body, total, target, out);
        }
    }
}

fn field_from_stmt(stmt: &Stmt, total: bool) -> Option<TdField> {
    let Stmt::AnnAssign(ann) = stmt else {
        return None;
    };
    let name = expr_name(&ann.target)?.to_owned();
    let (ty, _readonly, qualifier) = parse_qualified(&ann_str(&ann.annotation));
    let required = match qualifier {
        Some(Qualifier::Required) => true,
        Some(Qualifier::NotRequired) => false,
        None => total,
    };
    Some(TdField { name, ty, required })
}

fn class_total(cls: &ast::StmtClassDef) -> bool {
    cls.arguments.as_ref().is_none_or(|args| {
        !args.keywords.iter().any(|kw| {
            kw.arg.as_ref().is_some_and(|a| a.as_str() == "total")
                && matches!(&kw.value, Expr::BooleanLiteral(b) if !b.value)
        })
    })
}

fn extra_items_keyword(cls: &ast::StmtClassDef) -> Option<ExtraItems> {
    let args = cls.arguments.as_ref()?;
    let kw = args
        .keywords
        .iter()
        .find(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == "extra_items"))?;
    Some(extra_items_from_expr(&kw.value))
}

fn extra_items_from_expr(expr: &Expr) -> ExtraItems {
    let (ty, readonly, qualifier) = parse_qualified(&ann_str(expr));
    ExtraItems {
        ty,
        readonly,
        qualifier,
    }
}

fn closed_keyword(cls: &ast::StmtClassDef) -> Option<ClosedKw> {
    let args = cls.arguments.as_ref()?;
    let kw = args
        .keywords
        .iter()
        .find(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == "closed"))?;
    let value = match &kw.value {
        Expr::BooleanLiteral(b) => Some(b.value),
        _ => None,
    };
    Some(ClosedKw { value })
}

// ---------------------------------------------------------------------------
// Functional syntax: Name = TypedDict("Name", {...}, extra_items=..., total=...)
// ---------------------------------------------------------------------------

fn model_from_functional(assign: &ast::StmtAssign) -> Option<TdModel> {
    if assign.targets.len() != 1 {
        return None;
    }
    let name = assign.targets.first().and_then(expr_name)?.to_owned();
    let Expr::Call(call) = assign.value.as_ref() else {
        return None;
    };
    if expr_name(&call.func) != Some("TypedDict") {
        return None;
    }
    let total = functional_total(call);
    let fields = call
        .arguments
        .args
        .get(1)
        .map(|arg| functional_fields(arg, total))
        .unwrap_or_default();
    Some(TdModel {
        name,
        bases: Vec::new(),
        fields,
        extra_items: functional_extra_items(call),
        closed: functional_closed(call),
        span: mk_span(assign.range()),
    })
}

fn functional_fields(arg: &Expr, total: bool) -> Vec<TdField> {
    let Expr::Dict(dict) = arg else {
        return Vec::new();
    };
    dict.items
        .iter()
        .filter_map(|item| {
            let key = item.key.as_ref()?;
            let Expr::StringLiteral(s) = key else {
                return None;
            };
            let (ty, _readonly, qualifier) = parse_qualified(&ann_str(&item.value));
            let required = match qualifier {
                Some(Qualifier::Required) => true,
                Some(Qualifier::NotRequired) => false,
                None => total,
            };
            Some(TdField {
                name: s.value.to_str().to_owned(),
                ty,
                required,
            })
        })
        .collect()
}

fn functional_keyword<'a>(call: &'a ast::ExprCall, name: &str) -> Option<&'a Expr> {
    call.arguments
        .keywords
        .iter()
        .find(|kw| kw.arg.as_ref().is_some_and(|a| a.as_str() == name))
        .map(|kw| &kw.value)
}

fn functional_total(call: &ast::ExprCall) -> bool {
    !functional_keyword(call, "total")
        .is_some_and(|v| matches!(v, Expr::BooleanLiteral(b) if !b.value))
}

fn functional_extra_items(call: &ast::ExprCall) -> Option<ExtraItems> {
    functional_keyword(call, "extra_items").map(extra_items_from_expr)
}

fn functional_closed(call: &ast::ExprCall) -> Option<ClosedKw> {
    let value = functional_keyword(call, "closed")?;
    Some(ClosedKw {
        value: match value {
            Expr::BooleanLiteral(b) => Some(b.value),
            _ => None,
        },
    })
}

// ---------------------------------------------------------------------------
// Transitive resolution
// ---------------------------------------------------------------------------

/// Build a `name -> &TdModel` lookup.
pub(super) fn model_map(models: &[TdModel]) -> HashMap<&str, &TdModel> {
    models.iter().map(|m| (m.name.as_str(), m)).collect()
}

/// All fields visible on `name`, including transitive base fields. Nearer
/// declarations win on name collisions.
pub(super) fn transitive_fields(name: &str, map: &HashMap<&str, &TdModel>) -> Vec<TdField> {
    let mut acc: Vec<TdField> = Vec::new();
    gather_fields(name, map, 0, &mut acc);
    acc
}

fn gather_fields(name: &str, map: &HashMap<&str, &TdModel>, depth: u32, acc: &mut Vec<TdField>) {
    if depth >= MAX_DEPTH {
        return;
    }
    let Some(model) = map.get(name) else {
        return;
    };
    for base in &model.bases {
        gather_fields(base, map, depth + 1, acc);
    }
    for field in &model.fields {
        if let Some(existing) = acc.iter_mut().find(|f| f.name == field.name) {
            *existing = field.clone();
        } else {
            acc.push(field.clone());
        }
    }
}

/// The nearest *explicit* `extra_items` declaration on `name` itself or, when
/// `include_self` is false, only among its ancestors. Returns `None` when no
/// class in the chain declares `extra_items` (the implicit `ReadOnly[object]`).
pub(super) fn explicit_extra<'a>(
    name: &str,
    map: &HashMap<&'a str, &'a TdModel>,
    include_self: bool,
) -> Option<&'a ExtraItems> {
    find_extra(name, map, 0, include_self)
}

fn find_extra<'a>(
    name: &str,
    map: &HashMap<&'a str, &'a TdModel>,
    depth: u32,
    include_self: bool,
) -> Option<&'a ExtraItems> {
    if depth >= MAX_DEPTH {
        return None;
    }
    let model = map.get(name)?;
    if include_self {
        if let Some(extra) = &model.extra_items {
            return Some(extra);
        }
    }
    model
        .bases
        .iter()
        .find_map(|base| find_extra(base, map, depth + 1, true))
}

/// The effective `extra_items` pseudo-item every `TypedDict` carries: the
/// explicit declaration, or the implicit read-only `object`.
pub(super) fn effective_extra(name: &str, map: &HashMap<&str, &TdModel>) -> EffectiveExtra {
    explicit_extra(name, map, true).map_or_else(
        || ("object".to_owned(), true),
        |e| (e.ty.clone(), e.readonly),
    )
}

/// `true` when `name` or any ancestor sets `closed=True`.
pub(super) fn ancestor_closed_true(name: &str, map: &HashMap<&str, &TdModel>) -> bool {
    walk_any(name, map, 0, &|m| {
        m.closed.as_ref().is_some_and(|c| c.value == Some(true))
    })
}

fn walk_any(
    name: &str,
    map: &HashMap<&str, &TdModel>,
    depth: u32,
    pred: &dyn Fn(&TdModel) -> bool,
) -> bool {
    if depth >= MAX_DEPTH {
        return false;
    }
    map.get(name).is_some_and(|model| {
        pred(model)
            || model
                .bases
                .iter()
                .any(|base| walk_any(base, map, depth + 1, pred))
    })
}
