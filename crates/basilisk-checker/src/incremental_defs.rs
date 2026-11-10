// ############################################################################
// # BROKEN — THIS FILE DOES NOT COMPILE. DO NOT "FIX" IT BY RESTORING TEXT   #
// # MATCHING.                                                                #
// #                                                                          #
// # Deleted helper this file called:                                         #
// #   InferredType::from_annotation (types_parsing.rs)
// #                                                                          #
// # That helper decided types from the SPELLING of source text (lowercased   #
// # annotation strings, `"int"`/`"str"`/`"object"` literal matching, `|`     #
// # splitting, `starts_with("tuple[")`). It was deleted, not replaced.       #
// #                                                                          #
// # The call sites below are LEFT BROKEN ON PURPOSE. They are the map of     #
// # what must be rebuilt on the resolved AST — resolved bindings, canonical  #
// # `TypeNode`, and `assignable`/`equivalent` — or made to abstain.          #
// #                                                                          #
// # Restoring the deleted helper, vendoring a copy of it, or re-deriving a   #
// # type from source text anywhere below is FORBIDDEN.                       #
// #                                                                          #
// # Evidence and the failing tests that pin the real behaviour:              #
// #   docs/RULE-VALIDITY-REPORT.md                                           #
// #   crates/basilisk-checker/tests/legacy_annotation_text_parser_pin_tests.rs
// #   crates/basilisk-checker/tests/pep_spelling_invariance_pin_tests.rs     #
// ############################################################################

//! Implements [TYPEINF-FUNC], [TYPEINF-INFERRED], and
//! [TYPEINF-TARGET-INCREMENTAL]. See
//! docs/specs/CHECKER-TYPE-INFERENCE-SPEC.md#TYPEINF-TARGET-INCREMENTAL
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::SalsaValue)]
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
    #[returns(copy)]
    pub file: SourceFile,
    /// The defined name.
    #[returns(ref)]
    pub name: String,
    /// Definition kind.
    #[returns(copy)]
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
#[salsa::tracked(returns(clone), cycle_fn = definition_type_cycle_recover, cycle_initial = definition_type_cycle_initial)]
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

/// The `Callable` surface of a function definition's slice: declared
/// annotations where present, with an UNANNOTATED return synthesized from
/// the body's `return` expressions — same-module return inference
/// ([NARROWPLAN-CHECKLIST] Stage 2, expression inference).
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
    let return_type = match def.returns.as_deref() {
        Some(annotation) => annotation_type(slice, Some(annotation)),
        None => synthesized_return_type(&def.body),
    };
    InferredType::Callable(crate::types::CallableInfo {
        param_types,
        return_type: Box::new(return_type),
    })
}

/// Synthesize an unannotated function's return type: the union of its
/// `return` expression types through the bidirectional engine, plus `None`
/// when a bare `return` exists or the body can fall through.
/// Functions that terminate with `raise` and have no return path synthesize
/// the bottom type `Never` ([TYPEINF-FUNC-RETURN], [TYPEINF-SPECIAL-NEVER]).
///
/// Gradual-guarantee note ([TYPEINF-TARGET-GRADUAL]): this type is inferred
/// FROM unannotated code, so when rules consume it (Integration stage) it is
/// display/assist-grade — it must never be enforced as if the user declared
/// it, or removing a return annotation could introduce new errors. The
/// differential harness (`tests/gradual_guarantee_tests.rs`) pins the
/// behavioral invariant.
fn synthesized_return_type(body: &[Stmt]) -> InferredType {
    let mut returns = Vec::new();
    let mut has_bare_return = false;
    collect_return_exprs(body, &mut returns, &mut has_bare_return);
    let last_diverges = matches!(body.last(), Some(Stmt::Return(_) | Stmt::Raise(_)));
    if returns.is_empty() && !has_bare_return {
        return if last_diverges {
            InferredType::Never
        } else {
            InferredType::None_
        };
    }
    let mut engine = BidirEngine::new(std::collections::HashMap::new());
    let types: Vec<crate::bidir::Ty> = returns.iter().map(|expr| engine.synth(expr)).collect();
    let solution = engine.finish();
    let mut result = types
        .into_iter()
        .map(|ty| ty.to_inferred(&solution.vars))
        .fold(InferredType::Never, InferredType::union);
    // Approximate fall-through: unless the body's last statement diverges,
    // an implicit `return None` path exists. Conservative in the sound
    // direction — a spurious `| None` widens, never fabricates precision.
    if has_bare_return || !last_diverges {
        result = InferredType::union(result, InferredType::None_);
    }
    result
}

/// Collect `return` expressions (and bare-`return` presence) recursively,
/// stopping at nested function/class boundaries.
fn collect_return_exprs<'a>(stmts: &'a [Stmt], out: &mut Vec<&'a Expr>, bare: &mut bool) {
    for stmt in stmts {
        match stmt {
            Stmt::Return(ret) => match ret.value.as_deref() {
                Some(value) => out.push(value),
                None => *bare = true,
            },
            Stmt::If(node) => {
                collect_return_exprs(&node.body, out, bare);
                for clause in &node.elif_else_clauses {
                    collect_return_exprs(&clause.body, out, bare);
                }
            }
            Stmt::For(node) => {
                collect_return_exprs(&node.body, out, bare);
                collect_return_exprs(&node.orelse, out, bare);
            }
            Stmt::While(node) => {
                collect_return_exprs(&node.body, out, bare);
                collect_return_exprs(&node.orelse, out, bare);
            }
            Stmt::With(node) => collect_return_exprs(&node.body, out, bare),
            Stmt::Try(node) => {
                collect_return_exprs(&node.body, out, bare);
                for handler in &node.handlers {
                    let ruff_python_ast::ExceptHandler::ExceptHandler(h) = handler;
                    collect_return_exprs(&h.body, out, bare);
                }
                collect_return_exprs(&node.orelse, out, bare);
                collect_return_exprs(&node.finalbody, out, bare);
            }
            Stmt::Match(node) => {
                for case in &node.cases {
                    collect_return_exprs(&case.body, out, bare);
                }
            }
            _ => {}
        }
    }
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
        // A bare-name alias resolves its sibling directly (the definition
        // cycle edge); any other right-hand side synthesizes with the
        // module's callables in scope, so same-module call returns
        // (`x = f()` → `f`'s declared-or-synthesized return) resolve.
        Some(Stmt::Assign(assign)) => match assign.value.as_ref() {
            Expr::Name(reference) => sibling_type(db, def, reference.id.as_str()),
            value => synth_expression(db, def, value),
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

/// Synthesize one expression with the module's CALLABLES bound in scope.
///
/// Sibling functions/classes arrive through [`callable_interface`] — a
/// `PartialEq` value — so a body-only edit to a sibling with a DECLARED
/// signature backdates and this definition's memo survives (the early
/// cutoff the checklist demands); a changed SYNTHESIZED return correctly
/// invalidates.
fn synth_expression<'db>(db: &'db dyn Db, def: Definition<'db>, expr: &Expr) -> InferredType {
    let globals: std::collections::HashMap<String, crate::bidir::Ty> =
        callable_interface(db, def.file(db))
            .0
            .iter()
            .map(|(name, ty)| (name.clone(), crate::bidir::Ty::from_inferred(ty)))
            .collect();
    let mut engine = BidirEngine::new(globals);
    engine.set_class_attributes(
        class_attribute_interface(db, def.file(db))
            .0
            .iter()
            .map(|(class, attrs)| (class.clone(), attrs.iter().cloned().collect()))
            .collect(),
    );
    let ty = engine.synth(expr);
    let solution = engine.finish();
    ty.to_inferred(&solution.vars)
}

/// A module's class-attribute schemas — the value of
/// [`class_attribute_interface`]. `PartialEq` gives backdating: attribute
/// edits that leave the schema unchanged never invalidate consumers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClassAttributeInterface(pub Vec<(String, Vec<(String, InferredType)>)>);

/// Tracked query: lowercased class name → annotated class-level attributes,
/// from each class definition's slice (plain attribute-load inference,
/// [NARROWPLAN-CHECKLIST] expression inference). `self.attr` assignments in
/// `__init__` are a follow-up; class-level `AnnAssign` covers the declared
/// schema surface.
#[salsa::tracked(returns(ref))]
pub fn class_attribute_interface(db: &dyn Db, file: SourceFile) -> ClassAttributeInterface {
    ClassAttributeInterface(
        definitions(db, file)
            .iter()
            .filter(|def| def.kind(db) == DefKind::Class)
            .filter_map(|def| {
                let slice = def.source(db);
                let attrs = class_level_attributes(slice)?;
                Some((def.name(db).to_ascii_lowercase(), attrs))
            })
            .collect(),
    )
}

/// Parse a class slice and collect its class-level annotated attributes.
fn class_level_attributes(slice: &str) -> Option<Vec<(String, InferredType)>> {
    let parsed = ruff_python_parser::parse_module(slice).ok()?;
    let Some(Stmt::ClassDef(class)) = parsed.syntax().body.first() else {
        return None;
    };
    let attrs: Vec<(String, InferredType)> = class
        .body
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::AnnAssign(assign) => match assign.target.as_ref() {
                Expr::Name(name) => Some((
                    name.id.to_string(),
                    annotation_type(slice, Some(&assign.annotation)),
                )),
                _ => None,
            },
            _ => None,
        })
        .collect();
    Some(attrs)
}

/// Guard-annotation text → the resolved narrowing target, for every
/// `TypeGuard[X]` / `TypeIs[X]` guard in one file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GuardTypes(pub std::collections::HashMap<String, InferredType>);

/// Tracked query: the file's guard texts resolved by the FULL-module
/// [TYPEINF-ANNOTATION-RESOLUTION] cascade. The per-definition narrowing pass
/// runs on a definition SLICE where module aliases and classes are invisible,
/// so the resolved targets must arrive from this file-level view (Stage 0.5
/// bidir wiring).
#[salsa::tracked(returns(ref))]
pub fn guard_type_environment(db: &dyn Db, file: SourceFile) -> GuardTypes {
    let source = file.text(db);
    let Ok(parsed) = basilisk_parser::parse_source(source.clone(), "module.py".to_owned()) else {
        return GuardTypes::default();
    };
    let Ok(module) = basilisk_resolver::resolve(&parsed) else {
        return GuardTypes::default();
    };
    let Some(resolver) = crate::annotation::AnnotationResolver::for_module(&module) else {
        return GuardTypes::default();
    };
    let mut map = std::collections::HashMap::new();
    for function in &module.functions {
        for guard in &function.narrowing_guards {
            collect_guard_types(&guard.kind, &resolver, &mut map);
        }
    }
    GuardTypes(map)
}

/// Record the resolved target of one guard kind, recursing through `assert`.
fn collect_guard_types(
    kind: &basilisk_resolver::NarrowingGuardKind,
    resolver: &crate::annotation::AnnotationResolver<'_>,
    map: &mut std::collections::HashMap<String, InferredType>,
) {
    match kind {
        basilisk_resolver::NarrowingGuardKind::TypeGuard { guard_type, .. }
        | basilisk_resolver::NarrowingGuardKind::TypeIs { guard_type, .. } => {
            if !map.contains_key(guard_type) {
                if let Some(resolved) = resolver.resolve_text(guard_type) {
                    let _ = map.insert(guard_type.clone(), resolved);
                }
            }
        }
        basilisk_resolver::NarrowingGuardKind::Assert { inner } => {
            collect_guard_types(inner, resolver, map);
        }
        _ => {}
    }
}

/// Tracked query: the `(name, type)` interface of the module's FUNCTIONS and
/// CLASSES only — the backdating boundary variable inference reads its
/// callables through (variables are excluded to keep variable↔variable
/// resolution on the direct, cycle-recovered [`definition_type`] edge).
#[salsa::tracked(returns(ref))]
pub fn callable_interface(db: &dyn Db, file: SourceFile) -> ModuleInterface {
    ModuleInterface(
        definitions(db, file)
            .iter()
            .filter(|def| matches!(def.kind(db), DefKind::Function | DefKind::Class))
            .map(|def| (def.name(db).clone(), definition_type(db, *def)))
            .collect(),
    )
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

/// Tracked query: the flow-narrowed name uses within one function
/// definition — the **Salsa-backed use-def map** of
/// [TYPEINF-TARGET-NARROWING] at definition granularity.
///
/// Re-parses and re-resolves the definition's own slice (so the query
/// depends on the tracked `source` field plus the backdated
/// [`callable_interface`]: a body-only edit elsewhere leaves this memo
/// untouched, while a sibling SIGNATURE change correctly invalidates), seeds
/// the narrowing environment from the parameter annotations, and runs the
/// flow walker ([`crate::narrow::analyse_function_in`]) with its branch
/// frames and `phi`-joins over the resolver-collected guards. The module's
/// callable interfaces flow into the [`crate::narrow::NarrowContext`] so
/// `x = f()` narrows through `f`'s return and a `Never`-returning sibling
/// call diverges (inference-driven reachability).
#[salsa::tracked(returns(ref))]
pub fn narrowed_uses<'db>(
    db: &'db dyn Db,
    def: Definition<'db>,
) -> Vec<crate::narrow::NarrowedUse> {
    if def.kind(db) != DefKind::Function {
        return Vec::new();
    }
    let slice = def.source(db);
    let Ok(parsed) = basilisk_parser::parse_source(slice.clone(), "def.py".to_owned()) else {
        return Vec::new();
    };
    let Ok(resolved) = basilisk_resolver::resolve(&parsed) else {
        return Vec::new();
    };
    let Some(function) = resolved.functions.first() else {
        return Vec::new();
    };
    let declared = declared_parameter_types(slice, function);
    let Ok(reparsed) = ruff_python_parser::parse_module(slice) else {
        return Vec::new();
    };
    let Some(Stmt::FunctionDef(function_def)) = reparsed.syntax().body.first() else {
        return Vec::new();
    };
    let ctx = crate::narrow::NarrowContext {
        callables: callable_interface(db, def.file(db))
            .0
            .iter()
            .cloned()
            .collect(),
        guard_types: guard_type_environment(db, def.file(db)).0.clone(),
        ..Default::default()
    };
    crate::narrow::analyse_function_in(
        &function_def.body,
        crate::narrow::NarrowEnv::new(declared),
        &function.narrowing_guards,
        &ctx,
    )
    .narrowed_uses
}

/// Parameter name → declared type, from a function's annotation spans.
fn declared_parameter_types(
    slice: &str,
    function: &basilisk_resolver::FunctionInfo,
) -> std::collections::HashMap<String, InferredType> {
    function
        .parameters
        .iter()
        .filter_map(|param| {
            let span = param.annotation_span?;
            let start = usize::try_from(span.start).ok()?;
            let end = usize::try_from(span.end).ok()?;
            let text = slice.get(start..end)?;
            Some((param.name.clone(), InferredType::from_annotation(text)))
        })
        .collect()
}
