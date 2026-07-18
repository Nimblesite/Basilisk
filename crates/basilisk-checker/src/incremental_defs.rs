//! Implements [TYPEINF-TARGET-INCREMENTAL]. See docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-INCREMENTAL
//! Definition-level and expression-level Salsa queries — Stage 1 of
//! [NARROWPLAN-CHECKLIST](../../docs/plans/CHECKER-TYPE-NARROWING-INFERENCE-PLAN.md).
//!
//! The file-level queries in [`crate::incremental`] re-check a whole module on
//! any keystroke. This module adds ty-style granularity underneath them:
//!
//! - [`definitions`] parses a file once and creates one **tracked struct** per
//!   top-level definition, carrying the definition's own source *slice* (not
//!   offsets — an edit that only shifts a definition leaves its slice, and so
//!   every dependent memo, untouched);
//! - [`definition_type`] infers one definition's public type from its slice
//!   alone — the per-definition unit of re-execution, with **fixpoint cycle
//!   recovery** seeded by the divergent/bottom sentinel (`Unknown`) and a hard
//!   iteration cap;
//! - [`expression_types`] is the expression-level query: per-expression
//!   inferred types (assignment right-hand sides and `return` values) within
//!   one definition, for hover/inlay/diagnostic reuse;
//! - [`module_interface`] folds the per-definition types into a compact,
//!   `PartialEq` interface — the cross-file dependency boundary: a body-only
//!   edit backdates to "interface unchanged" and importers' memos survive.
#![expect(
    missing_docs,
    reason = "salsa macros generate undocumentable public constructors/accessors/setters"
)]

use basilisk_db::{Db, SourceFile};
use ruff_python_ast::{Expr, Stmt};
use ruff_text_size::Ranged;

use crate::bidir::BidirEngine;
use crate::types::InferredType;

/// Hard cap on fixpoint iterations for cyclic definitions. Reached only by
/// pathological oscillation; normal cycles converge in one or two rounds.
const CYCLE_ITERATION_CAP: u32 = 16;

/// What kind of top-level definition a [`Definition`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum DefKind {
    /// A `def`/`async def` at module level.
    Function,
    /// A `class` at module level.
    Class,
    /// A module-level assignment (`x = ...` / `x: T = ...`).
    Variable,
}

/// One top-level definition, tracked so downstream queries key on it.
///
/// `source` is the definition's own text slice: two revisions in which the
/// slice is byte-identical produce an "unchanged" tracked struct, so
/// [`definition_type`]/[`expression_types`] memos survive edits elsewhere in
/// the file — the definition-level early cutoff.
#[salsa::tracked(debug)]
pub struct Definition<'db> {
    /// The file this definition belongs to.
    pub file: SourceFile,
    /// The defined name.
    #[returns(ref)]
    pub name: String,
    /// Definition kind.
    pub kind: DefKind,
    /// The definition's own source slice (decorators included).
    #[returns(ref)]
    pub source: String,
}

/// Tracked query: the top-level definitions of one file.
///
/// The single whole-file parse per revision lives here; everything downstream
/// is per-definition.
#[salsa::tracked(returns(ref))]
pub fn definitions(db: &dyn Db, file: SourceFile) -> Vec<Definition<'_>> {
    let source = file.text(db);
    let Ok(parsed) = ruff_python_parser::parse_module(source) else {
        return Vec::new();
    };
    parsed
        .syntax()
        .body
        .iter()
        .filter_map(|stmt| create_definition(db, file, source, stmt))
        .collect()
}

/// Create the tracked struct for one top-level statement, if it defines a name.
fn create_definition<'db>(
    db: &'db dyn Db,
    file: SourceFile,
    source: &str,
    stmt: &Stmt,
) -> Option<Definition<'db>> {
    let (name, kind, range) = match stmt {
        Stmt::FunctionDef(def) => (def.name.to_string(), DefKind::Function, def.range()),
        Stmt::ClassDef(def) => (def.name.to_string(), DefKind::Class, def.range()),
        Stmt::Assign(assign) => match assign.targets.as_slice() {
            [Expr::Name(target)] => (target.id.to_string(), DefKind::Variable, assign.range()),
            _ => return None,
        },
        Stmt::AnnAssign(assign) => match assign.target.as_ref() {
            Expr::Name(target) => (target.id.to_string(), DefKind::Variable, assign.range()),
            _ => return None,
        },
        _ => return None,
    };
    let slice = source
        .get(usize::from(range.start())..usize::from(range.end()))?
        .to_owned();
    Some(Definition::new(db, file, name, kind, slice))
}

/// Tracked query with **fixpoint cycle recovery**: the public type of one
/// definition, inferred from its slice alone.
///
/// A definition whose right-hand side is a bare name reference resolves it
/// through its siblings' [`definition_type`] — mutually-referential
/// definitions therefore cycle, and salsa iterates from the divergent/bottom
/// sentinel ([`cycle_initial`] = `Unknown`, [TYPEINF-EXCEEDS-NOUNKNOWN])
/// until the types stabilise or [`CYCLE_ITERATION_CAP`] falls back to the
/// sentinel.
#[salsa::tracked(cycle_fn = definition_type_cycle_recover, cycle_initial = definition_type_cycle_initial)]
pub fn definition_type<'db>(db: &'db dyn Db, def: Definition<'db>) -> InferredType {
    match def.kind(db) {
        DefKind::Function => function_type(def.source(db)),
        DefKind::Class => InferredType::Named(def.name(db).to_ascii_lowercase()),
        DefKind::Variable => variable_type(db, def),
    }
}

/// Divergent/bottom seed for cyclic definition inference.
fn definition_type_cycle_initial<'db>(
    _db: &'db dyn Db,
    _id: salsa::Id,
    _def: Definition<'db>,
) -> InferredType {
    InferredType::Unknown
}

/// Fixpoint step: converged when the newly computed value matches the last
/// provisional one; past the hard cap, settle on the divergent sentinel.
fn definition_type_cycle_recover<'db>(
    _db: &'db dyn Db,
    cycle: &salsa::Cycle<'_>,
    last_provisional: &InferredType,
    value: InferredType,
    _def: Definition<'db>,
) -> InferredType {
    if &value == last_provisional {
        value
    } else if cycle.iteration() >= CYCLE_ITERATION_CAP {
        InferredType::Unknown
    } else {
        value
    }
}

/// The declared `Callable` surface of a function definition's slice.
fn function_type(slice: &str) -> InferredType {
    let Ok(parsed) = ruff_python_parser::parse_module(slice) else {
        return InferredType::Unknown;
    };
    let Some(Stmt::FunctionDef(def)) = parsed.syntax().body.first() else {
        return InferredType::Unknown;
    };
    let param_types = def
        .parameters
        .iter()
        .map(|param| annotation_type(slice, param.annotation()))
        .collect();
    let return_type = annotation_type(slice, def.returns.as_deref());
    InferredType::Callable(crate::types::CallableInfo {
        param_types,
        return_type: Box::new(return_type),
    })
}

/// Parse one annotation expression's text, `Unknown` when absent.
fn annotation_type(slice: &str, annotation: Option<&Expr>) -> InferredType {
    annotation
        .and_then(|expr| {
            let range = expr.range();
            slice.get(usize::from(range.start())..usize::from(range.end()))
        })
        .map_or(InferredType::Unknown, InferredType::from_annotation)
}

/// The inferred type of a module-level variable definition.
///
/// An explicit annotation wins; a bare-name right-hand side resolves through
/// the sibling definition's [`definition_type`] (the cycle edge); anything
/// else synthesizes through the bidirectional engine
/// ([TYPEINF-TARGET-BIDIRECTIONAL]).
fn variable_type<'db>(db: &'db dyn Db, def: Definition<'db>) -> InferredType {
    let slice = def.source(db);
    let Ok(parsed) = ruff_python_parser::parse_module(slice) else {
        return InferredType::Unknown;
    };
    match parsed.syntax().body.first() {
        Some(Stmt::AnnAssign(assign)) => annotation_type(slice, Some(&assign.annotation)),
        Some(Stmt::Assign(assign)) => match assign.value.as_ref() {
            Expr::Name(reference) => sibling_type(db, def, reference.id.as_str()),
            value => synth_expression(value),
        },
        _ => InferredType::Unknown,
    }
}

/// Resolve a bare-name right-hand side through the sibling definition —
/// the edge on which definition cycles (and their fixpoint) arise.
fn sibling_type<'db>(db: &'db dyn Db, def: Definition<'db>, name: &str) -> InferredType {
    definitions(db, def.file(db))
        .iter()
        .find(|sibling| sibling.name(db) == name)
        .map_or(InferredType::Unknown, |sibling| {
            definition_type(db, *sibling)
        })
}

/// Synthesize one expression through the bidirectional engine.
fn synth_expression(expr: &Expr) -> InferredType {
    let mut engine = BidirEngine::new(std::collections::HashMap::new());
    let ty = engine.synth(expr);
    let solution = engine.finish();
    ty.to_inferred(&solution.vars)
}

/// One expression-level inference result within a definition.
///
/// Offsets are relative to the definition's **slice**, so results stay valid
/// (and memos stay warm) when the definition merely moves within its file.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionType {
    /// Start offset within the definition slice.
    pub start: u32,
    /// End offset within the definition slice.
    pub end: u32,
    /// The inferred type of the expression.
    pub inferred: InferredType,
}

/// Tracked query: expression-level types within one definition — assignment
/// right-hand sides and `return` values, the positions hover/inlay hints and
/// diagnostics consume ([TYPEINF-TARGET-INCREMENTAL] expression granularity).
#[salsa::tracked(returns(ref))]
pub fn expression_types<'db>(db: &'db dyn Db, def: Definition<'db>) -> Vec<ExpressionType> {
    let slice = def.source(db);
    let Ok(parsed) = ruff_python_parser::parse_module(slice) else {
        return Vec::new();
    };
    let mut engine = BidirEngine::new(std::collections::HashMap::new());
    let mut recorded: Vec<(u32, u32, crate::bidir::Ty)> = Vec::new();
    collect_stmt_expressions(&mut engine, &parsed.syntax().body, &mut recorded);
    let solution = engine.finish();
    recorded
        .into_iter()
        .map(|(start, end, ty)| ExpressionType {
            start,
            end,
            inferred: ty.to_inferred(&solution.vars),
        })
        .collect()
}

/// Walk statements, synthesizing assignment RHS and `return` expressions.
fn collect_stmt_expressions(
    engine: &mut BidirEngine,
    stmts: &[Stmt],
    out: &mut Vec<(u32, u32, crate::bidir::Ty)>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign(assign) => record(engine, &assign.value, out),
            Stmt::AnnAssign(assign) => {
                if let Some(value) = assign.value.as_deref() {
                    record(engine, value, out);
                }
            }
            Stmt::Return(ret) => {
                if let Some(value) = ret.value.as_deref() {
                    record(engine, value, out);
                }
            }
            Stmt::FunctionDef(def) => collect_stmt_expressions(engine, &def.body, out),
            Stmt::ClassDef(def) => collect_stmt_expressions(engine, &def.body, out),
            Stmt::If(node) => {
                collect_stmt_expressions(engine, &node.body, out);
                for clause in &node.elif_else_clauses {
                    collect_stmt_expressions(engine, &clause.body, out);
                }
            }
            Stmt::For(node) => collect_stmt_expressions(engine, &node.body, out),
            Stmt::While(node) => collect_stmt_expressions(engine, &node.body, out),
            Stmt::With(node) => collect_stmt_expressions(engine, &node.body, out),
            _ => {}
        }
    }
}

/// Record one synthesized expression, slice-relative.
fn record(engine: &mut BidirEngine, expr: &Expr, out: &mut Vec<(u32, u32, crate::bidir::Ty)>) {
    let ty = engine.synth(expr);
    let range = expr.range();
    out.push((range.start().into(), range.end().into(), ty));
}

/// A module's compact inference interface: `(name, type)` per definition.
///
/// `PartialEq` makes this the **early-cutoff boundary** for cross-file work: a
/// body-only edit re-runs [`module_interface`], salsa backdates the unchanged
/// value, and importers reading it are never re-executed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModuleInterface(pub Vec<(String, InferredType)>);

/// Tracked query: the module's interface, the cross-file dependency boundary
/// ([TYPEINF-TARGET-INCREMENTAL] — Pyrefly's "Interface" idea).
#[salsa::tracked(returns(ref))]
pub fn module_interface(db: &dyn Db, file: SourceFile) -> ModuleInterface {
    ModuleInterface(
        definitions(db, file)
            .iter()
            .map(|def| (def.name(db).clone(), definition_type(db, *def)))
            .collect(),
    )
}
